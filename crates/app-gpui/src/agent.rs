//! The agent's turn over the open score: while it works the score is locked
//! to the user, its tool calls run here on the UI thread, and everything it
//! did folds into one undo step when the turn ends.
use std::collections::HashMap;
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use agent_host::{
    AgentEvent, AgentKind, LaunchCommand, McpServer, ResolvedTools, Runtime, SessionHandle,
    ToolInvocation,
};
use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures::stream;
use gpui::{Context, ScrollHandle, Task, Timer, Window};
use score_core::{Accidental, BarMarkRef, Duration, ElementId, MarkRef, Score};
use score_edit::{Cursor, Editor, HistoryMark};
use score_keymap::{Category, CommandId, Registry, id};

use crate::AppView;
use crate::app_paths::data_path;

/// What the user is told when a command waits for the agent.
pub const LOCKED: &str = "El agente está trabajando: detenlo para editar la partitura.";
pub const LOCKED_CLOSE: &str = "Detén al agente antes de cerrar la ventana.";
const WORKING: &str =
    "El agente está trabajando · puedes reproducir, desplazarte y hacer zoom mientras tanto";
/// Where a status names the Undo key, which differs per platform and user.
pub const UNDO_KEY: &str = "{deshacer}";
pub const FINISHED: &str = "El agente terminó · {deshacer} deshace todo lo que hizo";
pub const STOPPED: &str = "Agente detenido · {deshacer} deshace lo que llegó a hacer";
/// The agent did not end its turn after Stop, so its process was closed.
pub const STOPPED_HARD: &str =
    "Se cerró el agente: no respondió a Detener · {deshacer} deshace lo que llegó a hacer";
pub const FAILED: &str = "El agente se detuvo por un error";
pub const NO_START: &str = "El agente no pudo empezar";
/// How long a stopped turn may take to report its end before the agent's
/// process is closed.
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
const NO_TURN: &str = "El turno ya terminó: Levare no aplica más cambios hasta el próximo mensaje.";
const BUSY: &str = "Levare está ocupado con un diálogo; vuelve a intentarlo en un momento.";
const NO_FOLDER: &str = "No se encontró la carpeta de datos de Levare para el agente.";
const REFUSED: &str = "Rechazado";
pub const UNANSWERED: &str = "Sin respuesta: el turno terminó";
const MODAL: &str = "Levare tiene abierta una ventana del usuario (preferencias o selector de instrumentos) que no deja ejecutar comandos; pídele que la cierre.";

/// Whether a command of `category` reaches the user's score: during the
/// agent's turn only playback and the view do.
pub fn user_may_run(category: Category, turn_active: bool) -> bool {
    !turn_active || matches!(category, Category::Playback | Category::View)
}

/// What the user is told when `command` waits for the agent: closing and
/// quitting say so, like the window's close button.
pub fn lock_message(command: CommandId) -> &'static str {
    if [score_keymap::id::CLOSE, score_keymap::id::QUIT].contains(&command) {
        LOCKED_CLOSE
    } else {
        LOCKED
    }
}

/// Where the selection and the cursor are, to tell whether a tool call moved
/// them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    ids: Vec<ElementId>,
    cursor: Cursor,
}

impl Selected {
    pub fn of(editor: &Editor) -> Self {
        Self {
            ids: editor.selected_ids(),
            cursor: editor.cursor(),
        }
    }
}

/// Whether the score view follows the agent after one tool call: when the
/// call moved the selection, or a reveal was already due. Read-only calls
/// leave the user's scrolling alone.
pub fn follows_agent(due: bool, before: &Selected, after: &Selected) -> bool {
    due || before != after
}

/// What the agent hears when a command it ran changed nothing it can see:
/// the status the command left, which says why. Playback and review
/// commands, and copying, only report, so their status is no refusal.
pub fn refusal(
    category: Category,
    command: CommandId,
    changed: bool,
    status_before: &str,
    status_after: &str,
) -> Option<String> {
    let reports = matches!(category, Category::Playback | Category::Review)
        || command == score_keymap::id::COPY;
    (!changed && !reports && status_after != status_before).then(|| status_after.to_string())
}

/// One turn of the agent: everything it does between `start` and `finish`
/// is one undo step.
pub struct Turn {
    mark: HistoryMark,
}

impl Turn {
    pub fn start(editor: &mut Editor) -> Turn {
        Turn {
            mark: editor.mark(),
        }
    }

    /// Folds the turn's edits into one undo step and puts the cursor and the
    /// selection back where the user left them.
    pub fn finish(self, editor: &mut Editor) {
        editor.squash(&self.mark);
        editor.restore_selection(&self.mark);
    }
}

/// One line of the conversation, in order, for the panel to show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    User(String),
    /// The agent's answer; consecutive message pieces are joined.
    Agent(String),
    Tool {
        id: String,
        title: String,
        done: bool,
    },
    /// The agent asks before going on; `options` are `(id, label)`.
    Permission {
        request: u64,
        title: String,
        options: Vec<(String, String)>,
    },
    /// What Levare tells the user went wrong, already explained.
    Error(String),
    /// A note from Levare between the turns, like a new session.
    Notice(String),
}

/// The conversation as the panel shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Log(Vec<Entry>);

impl Log {
    pub fn entries(&self) -> &[Entry] {
        &self.0
    }

    pub fn push(&mut self, entry: Entry) {
        self.0.push(entry);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Adds what the session of a `kind` agent reported: message pieces join
    /// the answer they continue, a tool call already shown is updated, and a
    /// failure is explained.
    pub fn apply(&mut self, kind: AgentKind, event: AgentEvent) {
        match event {
            AgentEvent::Message(more) => match self.0.last_mut() {
                Some(Entry::Agent(text)) => text.push_str(&more),
                _ => self.0.push(Entry::Agent(more)),
            },
            AgentEvent::Tool { id, title, done } => {
                let shown = self.0.iter_mut().rev().find_map(|entry| match entry {
                    Entry::Tool {
                        id: shown,
                        title,
                        done,
                    } if *shown == id => Some((title, done)),
                    _ => None,
                });
                match shown {
                    Some((shown_title, shown_done)) => {
                        // Updates may leave the title out.
                        if !title.is_empty() {
                            *shown_title = title;
                        }
                        *shown_done = done;
                    }
                    None => self.0.push(Entry::Tool { id, title, done }),
                }
            }
            AgentEvent::Permission {
                request,
                title,
                options,
            } => self.0.push(Entry::Permission {
                request,
                title,
                options,
            }),
            AgentEvent::TurnEnded => {}
            AgentEvent::Failed(raw) => self
                .0
                .push(Entry::Error(agent_host::explain_error(kind, &raw))),
        }
    }
}

/// The note-entry state the agent's commands may change; it goes back as
/// the user left it when the turn ends, like the selection.
struct UserContext {
    note_entry: bool,
    chord_entry: bool,
    entry_accidental: Option<Accidental>,
    duration: Duration,
    octave: u8,
    clicked_mark: Option<MarkRef>,
    clicked_bar_mark: Option<BarMarkRef>,
}

impl UserContext {
    fn save(view: &AppView) -> Self {
        Self {
            note_entry: view.ui.note_entry,
            chord_entry: view.chord_entry,
            entry_accidental: view.entry_accidental,
            duration: view.document.editor.duration,
            octave: view.document.editor.octave,
            clicked_mark: view.clicked_mark,
            clicked_bar_mark: view.clicked_bar_mark,
        }
    }

    fn restore(self, view: &mut AppView) {
        view.ui.note_entry = self.note_entry;
        view.chord_entry = self.chord_entry;
        view.entry_accidental = self.entry_accidental;
        view.document.editor.duration = self.duration;
        view.document.editor.octave = self.octave;
        view.clicked_mark = self.clicked_mark;
        view.clicked_bar_mark = self.clicked_bar_mark;
    }
}

/// What the view does once the agent state took in an event or a click.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    /// End the turn in flight: fold its edits and give the score back.
    pub finish_turn: bool,
    /// What the status line says now.
    pub status: Option<&'static str>,
    /// A stop to watch: after `STOP_WAIT`, call `stop_timeout` with it.
    pub watch_stop: Option<u64>,
}

/// Where a message went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sending {
    /// To the session.
    Prompted,
    /// It waits for a session: start one.
    Start,
    /// It waits for the session already starting.
    Wait,
}

/// A running session and the MCP server it reaches Levare's tools through.
/// Fields drop in order: the agent's process first, then the server.
struct Session {
    handle: SessionHandle,
    _server: McpServer,
    /// Whether the session's first prompt, with the instructions, went out.
    instructed: bool,
    /// Receives the tool calls and the session's events; dropping it stops
    /// listening.
    listener: Task<()>,
}

/// The agent panel's state, one conversation per open document. The
/// runtime, the server and the session start with the first message.
pub struct AgentState {
    pub kind: AgentKind,
    /// What `probe` found, once it ran.
    pub tools: Option<ResolvedTools>,
    /// The conversation so far, for the agent panel to draw.
    pub log: Log,
    /// What the user answered to each permission of `log`, by its index.
    answers: HashMap<usize, String>,
    /// The conversation's scroll, kept at its end as entries arrive.
    pub scroll: ScrollHandle,
    turn: Option<Turn>,
    user: Option<UserContext>,
    session: Option<Session>,
    runtime: Option<Runtime>,
    /// The message of a turn whose session is still starting.
    pending: Option<String>,
    starting: bool,
    probing: bool,
    /// Stop was pressed and the cancelled prompt has not reported its end.
    stopping: bool,
    /// Counts the stops, so a late timer leaves a later stop alone.
    stops: u64,
    /// Whether a session of this conversation already started (it reported
    /// something other than a failure), so the next one is marked in the
    /// log.
    started_one: bool,
    /// Bumped whenever the session is dropped, so the tasks of an earlier
    /// one leave the new one alone.
    generation: u64,
    registry: Rc<Registry>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            kind: AgentKind::Claude,
            tools: None,
            log: Log::default(),
            answers: HashMap::new(),
            scroll: ScrollHandle::new(),
            turn: None,
            user: None,
            session: None,
            runtime: None,
            pending: None,
            starting: false,
            probing: false,
            stopping: false,
            stops: 0,
            started_one: false,
            generation: 0,
            registry: Rc::new(Registry::builtin()),
        }
    }
}

impl AgentState {
    pub fn turn_active(&self) -> bool {
        self.turn.is_some()
    }

    /// Whether a new message may go out: no turn in flight, and a stopped
    /// one has reported its end.
    pub fn can_send(&self) -> bool {
        self.turn.is_none() && !self.stopping
    }

    /// Stop was pressed and the agent has not ended the turn yet.
    pub fn stopping(&self) -> bool {
        self.stopping
    }

    /// The answer the user gave to the permission at `index` of the log,
    /// or why it got none.
    pub fn answer(&self, index: usize) -> Option<&str> {
        self.answers.get(&index).map(String::as_str)
    }

    fn say(&mut self, entry: Entry) {
        self.log.push(entry);
        self.scroll.scroll_to_bottom();
    }

    /// Adds what the session reported. A permission asked when no turn
    /// waits for it is shown already expired; its request comes back to be
    /// refused on the session, so the agent is not left waiting.
    pub(crate) fn hear(&mut self, event: AgentEvent) -> Option<u64> {
        let late = match &event {
            AgentEvent::Permission { request, .. } if !self.turn_active() || self.stopping => {
                Some(*request)
            }
            _ => None,
        };
        // Only an open session reports anything but a failure.
        if !matches!(event, AgentEvent::Failed(_)) {
            self.started_one = true;
        }
        self.log.apply(self.kind, event);
        if late.is_some() {
            let index = self.log.entries().len() - 1;
            self.answers.insert(index, UNANSWERED.to_string());
        }
        self.scroll.scroll_to_bottom();
        late
    }

    /// Notes the answer to the permission card at `index` of the log and
    /// returns its session request. Only a card still open answers: every
    /// card of an earlier turn or session was expired with it.
    pub(crate) fn record_answer(&mut self, index: usize, option_id: Option<&str>) -> Option<u64> {
        if !self.turn_active() || self.stopping || self.answers.contains_key(&index) {
            return None;
        }
        let Some(Entry::Permission {
            request, options, ..
        }) = self.log.entries().get(index)
        else {
            return None;
        };
        let answer = option_id
            .and_then(|id| options.iter().find(|(option, _)| option == id))
            .map_or_else(|| REFUSED.to_string(), |(_, label)| label.clone());
        let request = *request;
        self.answers.insert(index, answer);
        Some(request)
    }

    pub(crate) fn begin_turn(&mut self, turn: Turn) {
        self.turn = Some(turn);
    }

    /// Ends the turn in flight, if any; its open permissions expire with it.
    pub(crate) fn end_turn(&mut self) -> Option<Turn> {
        let turn = self.turn.take()?;
        self.pending = None;
        self.expire_permissions();
        Some(turn)
    }

    /// Permissions still waiting when the turn ends were cancelled with it.
    fn expire_permissions(&mut self) {
        for (index, entry) in self.log.entries().iter().enumerate() {
            if matches!(entry, Entry::Permission { .. }) {
                self.answers
                    .entry(index)
                    .or_insert_with(|| UNANSWERED.to_string());
            }
        }
    }

    /// Ends the session; the next message starts a new one.
    pub(crate) fn drop_session(&mut self) {
        // The new session's request ids start over.
        self.expire_permissions();
        self.session = None;
        self.pending = None;
        self.starting = false;
        self.stopping = false;
        self.generation += 1;
    }

    /// Starts the turn of the user's message and sends it, or keeps it for
    /// the session to start.
    pub(crate) fn send(&mut self, turn: Turn, text: String) -> Sending {
        let new_session = self.session.is_none() && !self.starting;
        if new_session && self.started_one {
            self.say(Entry::Notice(format!(
                "Conversación nueva con {}",
                self.kind.label()
            )));
        }
        self.say(Entry::User(text.clone()));
        self.begin_turn(turn);
        if let Some(session) = &mut self.session {
            prompt(session, text);
            return Sending::Prompted;
        }
        self.pending = Some(text);
        if !new_session {
            return Sending::Wait;
        }
        self.starting = true;
        Sending::Start
    }

    /// The message a starting session sends once it is open.
    pub(crate) fn take_pending(&mut self) -> Option<String> {
        self.pending.take()
    }

    /// Stop: the first asks the agent to end the turn, which is finished at
    /// once; a second, or the agent's silence, closes the agent.
    pub(crate) fn stop(&mut self) -> Reaction {
        if self.stopping {
            self.drop_session();
            return Reaction {
                status: Some(STOPPED_HARD),
                ..Reaction::default()
            };
        }
        if !self.turn_active() {
            return Reaction::default();
        }
        let stopped = Reaction {
            finish_turn: true,
            status: Some(STOPPED),
            watch_stop: None,
        };
        // Still starting: the message never reached an agent.
        if self.pending.is_some() {
            return stopped;
        }
        if let Some(session) = &self.session {
            session.handle.cancel();
        }
        self.stopping = true;
        self.stops += 1;
        Reaction {
            watch_stop: Some(self.stops),
            ..stopped
        }
    }

    /// The wait after `stop` number `stop` is over.
    pub(crate) fn stop_timeout(&mut self, stop: u64) -> Reaction {
        if !self.stopping || stop != self.stops {
            return Reaction::default();
        }
        self.drop_session();
        Reaction {
            status: Some(STOPPED_HARD),
            ..Reaction::default()
        }
    }

    /// The prompt in flight ended: the turn's, or that of a stopped one.
    pub(crate) fn on_turn_ended(&mut self) -> Reaction {
        self.started_one = true;
        if self.stopping {
            self.stopping = false;
            return Reaction::default();
        }
        Reaction {
            finish_turn: self.turn_active(),
            status: self.turn_active().then_some(FINISHED),
            watch_stop: None,
        }
    }

    /// The session failed, which ends the prompt in flight.
    pub(crate) fn on_failed(&mut self, raw: String) -> Reaction {
        self.stopping = false;
        let _ = self.hear(AgentEvent::Failed(raw));
        Reaction {
            finish_turn: self.turn_active(),
            status: self.turn_active().then_some(FAILED),
            watch_stop: None,
        }
    }

    /// The session's events ended: the session is gone.
    pub(crate) fn on_closed(&mut self) -> Reaction {
        if let Some(session) = self.session.take() {
            // The listener is the task that reports this; it ends by itself.
            session.listener.detach();
        }
        self.drop_session();
        Reaction {
            finish_turn: self.turn_active(),
            ..Reaction::default()
        }
    }

    /// The session could not start: says why and unlocks the score.
    pub(crate) fn fail_start(&mut self, error: String) -> Reaction {
        self.starting = false;
        self.say(Entry::Error(error));
        Reaction {
            finish_turn: self.turn_active(),
            status: Some(NO_START),
            watch_stop: None,
        }
    }
}

/// What reaches the UI thread from the session.
enum Incoming {
    Tool(ToolInvocation),
    Event(AgentEvent),
    /// The session's events ended: the session is gone.
    Closed,
}

/// What a command may change that the agent can tell apart: the score, the
/// selection and the note-entry state.
struct Seen {
    score: Score,
    selected: Selected,
    note_entry: bool,
    chord_entry: bool,
    entry_accidental: Option<Accidental>,
    duration: Duration,
    octave: u8,
}

impl Seen {
    fn of(view: &AppView) -> Self {
        let editor = &view.document.editor;
        Self {
            score: editor.score().clone(),
            selected: Selected::of(editor),
            note_entry: view.ui.note_entry,
            chord_entry: view.chord_entry,
            entry_accidental: view.entry_accidental,
            duration: editor.duration,
            octave: editor.octave,
        }
    }

    /// Whether `view` still shows this, without copying its score again.
    fn still(&self, view: &AppView) -> bool {
        let editor = &view.document.editor;
        *editor.score() == self.score
            && Selected::of(editor) == self.selected
            && view.ui.note_entry == self.note_entry
            && view.chord_entry == self.chord_entry
            && view.entry_accidental == self.entry_accidental
            && editor.duration == self.duration
            && editor.octave == self.octave
    }
}

/// The tools' view of the app while one call runs. Commands go through
/// `AppView::run` as a shortcut would, past the lock.
struct AgentHost<'a, 'b> {
    view: &'a mut AppView,
    window: &'a mut Window,
    cx: &'a mut Context<'b, AppView>,
}

impl agent_tools::Host for AgentHost<'_, '_> {
    fn editor(&self) -> &Editor {
        &self.view.document.editor
    }

    fn editor_mut(&mut self) -> &mut Editor {
        &mut self.view.document.editor
    }

    /// `AppView::run` refuses through the status line; a refusal comes back
    /// as the error, so the agent learns why nothing changed.
    fn run(&mut self, command: CommandId) -> Result<(), String> {
        let registry = Rc::clone(&self.view.agent.registry);
        let Some(spec) = agent_tools::allowed_commands(&registry)
            .into_iter()
            .find(|spec| spec.id == command)
        else {
            return Err(format!(
                "El comando «{command}» no está disponible para el agente."
            ));
        };
        let view = &mut *self.view;
        if view.busy || view.confirming {
            return Err(BUSY.into());
        }
        if view.instrument_picker.is_some() || view.preferences.is_some() {
            return Err(MODAL.into());
        }
        let before = Seen::of(view);
        // Emptied, so a refusal repeated word for word still counts as new.
        let status = std::mem::take(&mut view.status);
        view.agent_acting = true;
        view.run(command, self.window, self.cx);
        view.agent_acting = false;
        let changed = !before.still(view);
        let refused = refusal(spec.category, command, changed, "", &view.status);
        if view.status.is_empty() {
            view.status = status;
        }
        match refused {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
}

/// Starts the MCP server on the agent's runtime; the result is awaited from
/// the UI's executor.
fn serve(
    runtime: &Runtime,
    tools: UnboundedSender<ToolInvocation>,
) -> impl Future<Output = Result<McpServer, String>> + use<> {
    let started = runtime.handle().spawn(McpServer::start(tools));
    async move {
        match started.await {
            Ok(Ok(server)) => Ok(server),
            Ok(Err(error)) => Err(format!(
                "No se pudo abrir el servidor de herramientas: {error}"
            )),
            Err(error) => Err(format!(
                "No se pudo abrir el servidor de herramientas: {error}"
            )),
        }
    }
}

/// The agent's working folder, created if missing.
fn working_folder() -> Result<PathBuf, String> {
    let agent = data_path("agente").ok_or_else(|| NO_FOLDER.to_string())?;
    working_folder_in(&agent)
}

/// A folder of the agent's own inside `agent`, empty until it writes there,
/// so the agent never starts next to Levare's files (the recovery copy, the
/// settings) that share the data folder.
pub(crate) fn working_folder_in(agent: &Path) -> Result<PathBuf, String> {
    let folder = agent.join("espacio");
    fs::create_dir_all(&folder)
        .map_err(|error| format!("No se pudo crear la carpeta del agente: {error}"))?;
    Ok(folder)
}

/// A session to open once its MCP server is up.
struct Opening {
    launch: LaunchCommand,
    cwd: PathBuf,
    tools: UnboundedReceiver<ToolInvocation>,
}

/// What the start-up found off the UI thread.
type Prepared = (
    ResolvedTools,
    Result<LaunchCommand, String>,
    Result<PathBuf, String>,
);

impl AppView {
    /// The user's input may not touch the score: the agent's turn is on and
    /// the command at hand is not the agent's own.
    pub(crate) fn agent_locked(&self) -> bool {
        self.agent.turn_active() && !self.agent_acting
    }

    /// Whether the agent's turn holds the user's input back; if so the status
    /// says why.
    pub(crate) fn agent_blocks(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.agent_locked() {
            return false;
        }
        self.status = LOCKED.into();
        cx.notify();
        true
    }

    /// Whether the window may close now; during a turn it says why not.
    pub(crate) fn agent_allows_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.agent_locked() {
            return true;
        }
        self.status = LOCKED_CLOSE.into();
        cx.notify();
        false
    }

    /// The open document is replaced: its conversation and session end.
    pub(crate) fn end_agent_conversation(&mut self, cx: &mut Context<Self>) {
        self.finish_turn(cx);
        self.agent.drop_session();
        self.agent.log.clear();
        self.agent.answers.clear();
        self.agent.started_one = false;
    }

    /// Does what the agent state decided.
    fn react(&mut self, reaction: Reaction, cx: &mut Context<Self>) {
        if reaction.finish_turn {
            self.finish_turn(cx);
        }
        if let Some(status) = reaction.status {
            self.status = status.replace(UNDO_KEY, &self.shortcuts.hint(id::UNDO));
        }
        if let Some(stop) = reaction.watch_stop {
            cx.spawn(async move |view, cx| {
                Timer::after(STOP_WAIT).await;
                let _ = view.update(cx, |view, cx| {
                    let reaction = view.agent.stop_timeout(stop);
                    view.react(reaction, cx);
                });
            })
            .detach();
        }
        cx.notify();
    }
}

/// What the agent panel calls.
impl AppView {
    /// Sends the user's message and starts the agent's turn, starting the
    /// session first if there is none.
    pub(crate) fn send_agent_message(
        &mut self,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = text.trim().to_string();
        if text.is_empty() || !self.agent.can_send() || self.busy || self.confirming {
            return;
        }
        // Text fields would take the keys the user may still press, and an
        // open instrument picker would refuse the agent's edits.
        window.focus(&self.focus);
        if self.instrument_picker.is_some() {
            self.close_instrument_picker(cx);
        }
        let turn = self.start_turn();
        if self.agent.send(turn, text) == Sending::Start {
            self.start_agent(window, cx);
        }
        self.status = WORKING.into();
        cx.notify();
    }

    /// Stops the turn in flight. What the agent already did stays, as one
    /// undo step. Pressed again while the agent has not stopped, it closes
    /// the agent.
    pub(crate) fn stop_agent(&mut self, cx: &mut Context<Self>) {
        let reaction = self.agent.stop();
        self.react(reaction, cx);
    }

    /// Chooses the agent for the next message; a different one starts a new
    /// session. Not during a turn.
    pub(crate) fn select_agent(&mut self, kind: AgentKind, cx: &mut Context<Self>) {
        if self.agent.turn_active() || self.agent.kind == kind {
            return;
        }
        self.agent.kind = kind;
        self.agent.drop_session();
        cx.notify();
    }

    /// Answers the permission card at `index` of the log with one of its
    /// option ids, or `None` to refuse. A card no longer open is ignored.
    pub(crate) fn answer_agent_permission(
        &mut self,
        index: usize,
        option_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self.agent.record_answer(index, option_id.as_deref()) else {
            return;
        };
        if let Some(session) = &self.agent.session {
            session.handle.answer_permission(request, option_id);
        }
        cx.notify();
    }

    /// Finds which agents are installed, off the UI thread, once.
    pub(crate) fn probe_agents(&mut self, cx: &mut Context<Self>) {
        if self.agent.tools.is_some() || self.agent.probing {
            return;
        }
        self.agent.probing = true;
        cx.spawn(async move |view, cx| {
            let tools = cx
                .background_executor()
                .spawn(async { agent_host::probe() })
                .await;
            let _ = view.update(cx, |view, cx| {
                let agent = &mut view.agent;
                agent.probing = false;
                let tools = agent.tools.get_or_insert(tools);
                // Before the first message, an agent that cannot start gives
                // way to one that can.
                let usable = |kind| agent_host::launch_command(kind, tools).is_ok();
                if agent.log.entries().is_empty()
                    && !usable(agent.kind)
                    && let Some(kind) = AgentKind::ALL.into_iter().find(|&kind| usable(kind))
                {
                    agent.kind = kind;
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl AppView {
    fn start_turn(&mut self) -> Turn {
        self.agent.user = Some(UserContext::save(self));
        // What the user had picked or half-drawn must not steer the agent's
        // commands.
        self.clicked_mark = None;
        self.clicked_bar_mark = None;
        self.pending_slur = None;
        self.pending_hairpin = None;
        self.open_line = None;
        self.selection_drag = None;
        self.note_drag = None;
        self.mark_drag = None;
        self.connection_drag = None;
        self.ghost = None;
        Turn::start(&mut self.document.editor)
    }

    fn finish_turn(&mut self, cx: &mut Context<Self>) {
        let Some(turn) = self.agent.end_turn() else {
            return;
        };
        turn.finish(&mut self.document.editor);
        self.pending_slur = None;
        self.pending_hairpin = None;
        self.open_line = None;
        if let Some(user) = self.agent.user.take() {
            user.restore(self);
        }
        self.sync_quit_snapshot();
        cx.notify();
    }

    /// The turn could not start: says why and unlocks the score.
    fn fail_start(&mut self, error: String, cx: &mut Context<Self>) {
        let reaction = self.agent.fail_start(error);
        self.react(reaction, cx);
    }

    /// Finds the agent, starts the runtime, the MCP server and the session,
    /// then sends the pending message. Probing and the folder run off the
    /// UI thread.
    fn start_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let generation = self.agent.generation;
        let kind = self.agent.kind;
        let known = self.agent.tools.clone();
        cx.spawn_in(window, async move |view, cx| {
            let prepared: Prepared = cx
                .background_executor()
                .spawn(async move {
                    let tools = known.unwrap_or_else(agent_host::probe);
                    let launch = agent_host::launch_command(kind, &tools);
                    (tools, launch, working_folder())
                })
                .await;
            let Ok(Some((server, opening))) =
                view.update(cx, |view, cx| view.begin_server(generation, prepared, cx))
            else {
                return;
            };
            let server = server.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.open_session(generation, server, opening, window, cx)
            });
        })
        .detach();
    }

    fn begin_server(
        &mut self,
        generation: u64,
        (tools, launch, cwd): Prepared,
        cx: &mut Context<Self>,
    ) -> Option<(
        impl Future<Output = Result<McpServer, String>> + use<>,
        Opening,
    )> {
        if generation != self.agent.generation {
            return None;
        }
        self.agent.tools.get_or_insert(tools);
        // Stopped while the agent was being looked for.
        if self.agent.pending.is_none() {
            self.agent.starting = false;
            return None;
        }
        let (launch, cwd) = match (launch, cwd) {
            (Ok(launch), Ok(cwd)) => (launch, cwd),
            (Err(error), _) | (_, Err(error)) => {
                self.fail_start(error, cx);
                return None;
            }
        };
        if self.agent.runtime.is_none() {
            match Runtime::new() {
                Ok(runtime) => self.agent.runtime = Some(runtime),
                Err(error) => {
                    self.fail_start(format!("No se pudo preparar el agente: {error}"), cx);
                    return None;
                }
            }
        }
        let runtime = self.agent.runtime.as_ref().expect("created above");
        let (sender, receiver) = mpsc::unbounded();
        let opening = Opening {
            launch,
            cwd,
            tools: receiver,
        };
        Some((serve(runtime, sender), opening))
    }

    fn open_session(
        &mut self,
        generation: u64,
        server: Result<McpServer, String>,
        Opening { launch, cwd, tools }: Opening,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.agent.generation {
            return;
        }
        let server = match server {
            Ok(server) => server,
            Err(error) => return self.fail_start(error, cx),
        };
        self.agent.starting = false;
        let runtime = self
            .agent
            .runtime
            .as_ref()
            .expect("started with the server");
        let (handle, events) = agent_host::start_session(runtime, launch, cwd, &server);
        let incoming = stream::select(
            tools.map(Incoming::Tool),
            events
                .map(Incoming::Event)
                .chain(stream::iter([Incoming::Closed])),
        );
        let listener = cx.spawn_in(window, async move |view, cx| {
            let mut incoming = incoming;
            while let Some(next) = incoming.next().await {
                let closed = matches!(next, Incoming::Closed);
                let received = view.update_in(cx, |view, window, cx| {
                    view.receive(generation, next, window, cx)
                });
                if closed || received.is_err() {
                    break;
                }
            }
        });
        let mut session = Session {
            handle,
            _server: server,
            instructed: false,
            listener,
        };
        if let Some(text) = self.agent.take_pending() {
            prompt(&mut session, text);
        }
        self.agent.session = Some(session);
        cx.notify();
    }

    fn receive(
        &mut self,
        generation: u64,
        incoming: Incoming,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A call of an earlier session goes unanswered, which tells its
        // agent Levare is gone.
        if generation != self.agent.generation {
            return;
        }
        match incoming {
            Incoming::Tool(invocation) => self.answer_tool(invocation, window, cx),
            Incoming::Event(event) => self.agent_event(event, cx),
            Incoming::Closed => {
                let reaction = self.agent.on_closed();
                self.react(reaction, cx);
            }
        }
    }

    /// Runs one tool call on the score and answers it. Only during a turn,
    /// so everything the agent changes is in that turn's undo step.
    fn answer_tool(
        &mut self,
        invocation: ToolInvocation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The agent gave up on it already.
        if invocation.reply.is_canceled() {
            return;
        }
        let result = if !self.agent.turn_active() {
            Err(NO_TURN.to_string())
        } else if self.busy || self.confirming {
            Err(BUSY.to_string())
        } else {
            let registry = Rc::clone(&self.agent.registry);
            let (due, before) = (self.reveal_selection, Selected::of(&self.document.editor));
            let result = agent_tools::dispatch(
                &mut AgentHost {
                    view: self,
                    window,
                    cx,
                },
                &registry,
                invocation.call,
            );
            // Every call that moved the selection shows it, whether it went
            // through a command or straight to the editor.
            self.reveal_selection =
                follows_agent(due, &before, &Selected::of(&self.document.editor));
            self.sync_quit_snapshot();
            cx.notify();
            result
        };
        let _ = invocation.reply.send(result);
    }

    fn agent_event(&mut self, event: AgentEvent, cx: &mut Context<Self>) {
        let reaction = match event {
            AgentEvent::TurnEnded => self.agent.on_turn_ended(),
            AgentEvent::Failed(raw) => self.agent.on_failed(raw),
            event => {
                let asks = matches!(event, AgentEvent::Permission { .. });
                match self.agent.hear(event) {
                    Some(request) => {
                        if let Some(session) = &self.agent.session {
                            session.handle.answer_permission(request, None);
                        }
                    }
                    // A card waits for the user: they must see it.
                    None if asks && !self.ui.show_agent => {
                        self.ui.show_agent = true;
                        self.reveal_selection = true;
                    }
                    None => {}
                }
                Reaction::default()
            }
        };
        self.react(reaction, cx);
    }
}

/// Sends a turn's message, after the instructions in a session's first one.
fn prompt(session: &mut Session, text: String) {
    let text = if session.instructed {
        text
    } else {
        session.instructed = true;
        format!("{}\n\n{text}", agent_tools::INSTRUCTIONS)
    };
    session.handle.prompt(text);
}
