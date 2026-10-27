use crate::agent::{
    AgentState, Entry, FAILED, FINISHED, LOCKED, LOCKED_CLOSE, Log, NO_START, Reaction, STOPPED,
    STOPPED_HARD, Selected, Sending, Turn, UNANSWERED, follows_agent, lock_message, refusal,
    user_may_run,
};
use agent_host::{AgentEvent, AgentKind};
use agent_tools::{EscribirArgs, Host, LeerArgs, Position, SeleccionarArgs, ToolCall, dispatch};
use score_core::{Duration, ElementId, Event, Instrument, Pitch, Pitches, Score, Step};
use score_edit::{Command, Editor};
use score_keymap::{Category, CommandId, Registry, id};

/// The tools over a bare editor; no registry command reaches it.
struct EditorHost(Editor);

impl Host for EditorHost {
    fn editor(&self) -> &Editor {
        &self.0
    }
    fn editor_mut(&mut self) -> &mut Editor {
        &mut self.0
    }
    fn run(&mut self, id: CommandId) -> Result<(), String> {
        Err(format!("«{id}» no está disponible en la prueba"))
    }
}

fn note(step: Step) -> Event {
    Event {
        notation: Default::default(),
        pitches: Pitches::single(Some(Pitch::new(step, 4).unwrap())),
        duration: Duration::Quarter,
    }
}

/// A violin with three bars; the first is C4 D4 E4 F4 in quarters, with
/// the last one written selected.
fn violin() -> Editor {
    let score = Score::new("Prueba".into(), &[Instrument::Violin], 3).unwrap();
    let mut editor = Editor::new(score).unwrap();
    for (beat, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        editor
            .execute(Command::WriteAt {
                measure: 0,
                tick: beat as u16 * 48,
                event: note(step),
            })
            .unwrap();
    }
    editor
}

fn first_event(editor: &Editor, measure: usize) -> ElementId {
    let score = editor.score();
    let part = &score.parts()[0];
    let staff = part.staff(score_core::StaffId::Upper).unwrap();
    let event = staff.measures()[measure]
        .voice_id_at(score_core::Voice::One, 0)
        .unwrap();
    ElementId {
        part: part.id(),
        staff: score_core::StaffId::Upper,
        measure,
        event,
    }
}

#[test]
fn lock_allows_playback_and_view_only() {
    assert!(user_may_run(Category::Playback, true));
    assert!(user_may_run(Category::View, true));
    for category in [Category::Edit, Category::NoteInput, Category::Expression] {
        assert!(!user_may_run(category, true), "{category:?}");
    }
    for category in Category::ALL {
        assert!(user_may_run(category, false), "{category:?}");
    }
}

#[test]
fn turn_finish_after_failure_leaves_single_undo() {
    let mut host = EditorHost(violin());
    let first = first_event(&host.0, 0);
    host.0.select(first);
    let before = host.0.score().clone();
    let (selection, cursor) = (host.0.selected_ids(), host.0.cursor());

    let turn = Turn::start(&mut host.0);
    for (tick, step) in [(0, Step::G), (48, Step::A)] {
        host.0
            .execute(Command::WriteAt {
                measure: 1,
                tick,
                event: note(step),
            })
            .unwrap();
    }
    let edited = host.0.score().clone();
    // The tie joins different pitches: refused after bar 1 was written.
    let error = dispatch(
        &mut host,
        &Registry::builtin(),
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 1,
            compases: vec!["G4:1~".into(), "A4:1".into()],
        }),
    )
    .unwrap_err();
    assert!(error.contains("No se ha escrito nada"), "{error}");
    assert_eq!(
        host.0.score(),
        &edited,
        "the failed write rolled back alone"
    );
    host.0.select(first_event(&host.0, 1));
    turn.finish(&mut host.0);

    assert_eq!(host.0.score(), &edited);
    assert_eq!(host.0.selected_ids(), selection);
    assert_eq!(host.0.cursor(), cursor);
    assert!(host.0.undo());
    assert_eq!(host.0.score(), &before);
    // The setup's own writes are still there to undo, one by one.
    assert!(host.0.undo());
    assert_ne!(host.0.score(), &before);
}

#[test]
fn turn_without_edits_adds_no_undo() {
    let mut editor = violin();
    let (selection, cursor) = (editor.selected_ids(), editor.cursor());
    let first = first_event(&editor, 0);
    assert_ne!(selection, [first]);
    let could_undo = editor.can_undo();

    let turn = Turn::start(&mut editor);
    editor.select(first);
    turn.finish(&mut editor);

    assert_eq!(editor.can_undo(), could_undo);
    assert_eq!(editor.selected_ids(), selection);
    assert_eq!(editor.cursor(), cursor);
    // Only the setup's four writes are there to undo.
    for _ in 0..4 {
        assert!(editor.undo());
    }
    assert!(!editor.can_undo());
}

#[test]
fn refusal_is_the_status_of_a_command_that_changed_nothing() {
    let why = "Selecciona el inicio de la ligadura de unión";
    let refused = |category, command, changed, after: &str| {
        refusal(category, command, changed, "Partitura actualizada", after)
    };
    assert_eq!(
        refused(Category::Expression, id::TIE, false, why),
        Some(why.to_string())
    );
    // An edit that went through, or a command that said nothing new.
    assert_eq!(refused(Category::Expression, id::TIE, true, why), None);
    assert_eq!(
        refused(
            Category::Expression,
            id::TIE,
            false,
            "Partitura actualizada"
        ),
        None
    );
    // Playback, review and copying only report.
    assert_eq!(
        refused(Category::Playback, id::PLAY, false, "Reproduciendo"),
        None
    );
    assert_eq!(
        refused(Category::Review, id::CHECK_SCORE, false, "Sin avisos"),
        None
    );
    assert_eq!(
        refused(Category::Edit, id::COPY, false, "Copiados 4 elementos"),
        None
    );
}

#[test]
fn view_follows_only_calls_that_move_the_selection() {
    let mut host = EditorHost(violin());
    let registry = Registry::builtin();
    let call = |host: &mut EditorHost, call| {
        let before = Selected::of(&host.0);
        dispatch(host, &registry, call).unwrap();
        follows_agent(false, &before, &Selected::of(&host.0))
    };
    assert!(!call(&mut host, ToolCall::Resumen));
    assert!(!call(
        &mut host,
        ToolCall::Leer(LeerArgs {
            partes: None,
            desde: 1,
            hasta: 3,
        })
    ));
    let first = Position {
        parte: "1".into(),
        compas: 1,
        tiempo: 1.,
        pentagrama: None,
        voz: None,
    };
    assert!(call(
        &mut host,
        ToolCall::Seleccionar(SeleccionarArgs {
            desde: first,
            hasta: None,
        })
    ));
    assert!(call(
        &mut host,
        ToolCall::Escribir(EscribirArgs {
            parte: "1".into(),
            pentagrama: None,
            voz: None,
            desde_compas: 2,
            compases: vec!["G4:1".into()],
        })
    ));
    // A reveal already due is kept.
    let here = Selected::of(&host.0);
    assert!(follows_agent(true, &here, &here));
}

fn tool(id: &str, title: &str, done: bool) -> AgentEvent {
    AgentEvent::Tool {
        id: id.into(),
        title: title.into(),
        done,
    }
}

#[test]
fn log_merges_message_chunks() {
    let mut log = Log::default();
    log.push(Entry::User("Pon un trino".into()));
    log.apply(AgentKind::Claude, AgentEvent::Message("Hola ".into()));
    log.apply(AgentKind::Claude, AgentEvent::Message("mundo".into()));
    assert_eq!(
        log.entries(),
        [
            Entry::User("Pon un trino".into()),
            Entry::Agent("Hola mundo".into())
        ]
    );
    // A tool call in between starts a new answer.
    log.apply(AgentKind::Claude, tool("1", "leer", true));
    log.apply(AgentKind::Claude, AgentEvent::Message("Listo".into()));
    assert_eq!(log.entries().last(), Some(&Entry::Agent("Listo".into())));
    assert_eq!(log.entries().len(), 4);
}

#[test]
fn log_updates_tool_by_id() {
    let mut log = Log::default();
    log.apply(AgentKind::Codex, tool("1", "escribir", false));
    log.apply(AgentKind::Codex, tool("2", "leer", false));
    log.apply(AgentKind::Codex, tool("1", "", true));
    assert_eq!(
        log.entries(),
        [
            Entry::Tool {
                id: "1".into(),
                title: "escribir".into(),
                done: true
            },
            Entry::Tool {
                id: "2".into(),
                title: "leer".into(),
                done: false
            }
        ]
    );
}

#[test]
fn log_explains_failure() {
    let mut log = Log::default();
    log.apply(AgentKind::Codex, AgentEvent::TurnEnded);
    log.apply(
        AgentKind::Codex,
        AgentEvent::Failed("Authentication required (-32000)".into()),
    );
    let [Entry::Error(text)] = log.entries() else {
        panic!("{:?}", log.entries());
    };
    assert!(text.contains("`codex login`"), "{text}");
}

fn asks(request: u64) -> AgentEvent {
    AgentEvent::Permission {
        request,
        title: "Editar archivo".into(),
        options: vec![("allow".into(), "Permitir".into())],
    }
}

#[test]
fn permission_after_stop_is_born_expired() {
    let mut editor = violin();
    let mut agent = AgentState::default();
    agent.begin_turn(Turn::start(&mut editor));
    // Stop ends the turn before the cancelled prompt reports its end.
    agent.end_turn().unwrap().finish(&mut editor);
    assert_eq!(agent.hear(asks(0)), Some(0), "refused on the session");
    assert_eq!(agent.answer(0), Some(UNANSWERED));
    // The next turn does not bring it back.
    agent.begin_turn(Turn::start(&mut editor));
    assert_eq!(agent.record_answer(0, Some("allow")), None);
    assert_eq!(agent.answer(0), Some(UNANSWERED));
}

#[test]
fn old_card_cannot_answer_the_new_sessions_request() {
    let mut editor = violin();
    let mut agent = AgentState::default();
    agent.begin_turn(Turn::start(&mut editor));
    assert_eq!(agent.hear(asks(0)), None);
    // A new session, whose request ids start over.
    agent.drop_session();
    assert_eq!(agent.answer(0), Some(UNANSWERED));
    assert_eq!(agent.hear(asks(0)), None);
    assert_eq!(agent.record_answer(0, Some("allow")), None);
    assert_eq!(agent.answer(1), None, "the new card still waits");
    assert_eq!(agent.record_answer(1, Some("allow")), Some(0));
    assert_eq!(agent.answer(1), Some("Permitir"));
    assert_eq!(agent.answer(0), Some(UNANSWERED));
}

#[test]
fn answering_by_index_marks_exactly_that_card() {
    let mut editor = violin();
    let mut agent = AgentState::default();
    agent.begin_turn(Turn::start(&mut editor));
    agent.hear(asks(4));
    agent.hear(asks(5));
    assert_eq!(agent.record_answer(0, None), Some(4));
    assert_eq!(agent.answer(0), Some("Rechazado"));
    assert_eq!(agent.answer(1), None);
    // Answered once only.
    assert_eq!(agent.record_answer(0, Some("allow")), None);
    assert_eq!(agent.answer(0), Some("Rechazado"));
    // An index that is not a permission answers nothing.
    agent.hear(AgentEvent::Message("Hecho".into()));
    assert_eq!(agent.record_answer(2, None), None);
}

#[test]
fn closing_during_a_turn_asks_to_stop_the_agent() {
    assert_eq!(lock_message(id::QUIT), LOCKED_CLOSE);
    assert_eq!(lock_message(id::CLOSE), LOCKED_CLOSE);
    assert_eq!(lock_message(id::STACCATO), LOCKED);
}

/// The agent state and an editor for its turns. The view's part of each
/// reaction is played here: finishing folds the turn on the editor.
struct Panel {
    agent: AgentState,
    editor: Editor,
}

impl Panel {
    fn new() -> Self {
        Self {
            agent: AgentState::default(),
            editor: violin(),
        }
    }

    fn send(&mut self, text: &str) -> Sending {
        let turn = Turn::start(&mut self.editor);
        self.agent.send(turn, text.into())
    }

    /// The session opened and took the waiting message.
    fn open(&mut self) -> Option<String> {
        self.agent.take_pending()
    }

    fn react(&mut self, reaction: Reaction) -> Reaction {
        if reaction.finish_turn {
            self.agent
                .end_turn()
                .expect("a turn to finish")
                .finish(&mut self.editor);
        }
        reaction
    }

    fn stop(&mut self) -> Reaction {
        let reaction = self.agent.stop();
        self.react(reaction)
    }
}

fn status(status: &'static str) -> Reaction {
    Reaction {
        status: Some(status),
        ..Reaction::default()
    }
}

fn finished(status: &'static str) -> Reaction {
    Reaction {
        finish_turn: true,
        status: Some(status),
        watch_stop: None,
    }
}

#[test]
fn turn_ends_and_the_next_message_goes() {
    let mut panel = Panel::new();
    assert_eq!(panel.send("hola"), Sending::Start);
    assert!(!panel.agent.can_send());
    assert_eq!(panel.open().as_deref(), Some("hola"));
    let reaction = panel.agent.on_turn_ended();
    assert_eq!(panel.react(reaction), finished(FINISHED));
    assert!(panel.agent.can_send());
}

#[test]
fn stop_then_turn_ended_lets_the_next_message_go() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    let stop = panel.stop();
    assert!(stop.finish_turn);
    assert_eq!(stop.status, Some(STOPPED));
    assert!(stop.watch_stop.is_some());
    // The score is back, but the agent has not ended the turn yet.
    assert!(!panel.agent.turn_active());
    assert!(panel.agent.stopping());
    assert!(!panel.agent.can_send());
    assert_eq!(panel.agent.on_turn_ended(), Reaction::default());
    assert!(panel.agent.can_send());
    // The timer that fires later leaves the panel alone.
    assert_eq!(
        panel.agent.stop_timeout(stop.watch_stop.unwrap()),
        Reaction::default()
    );
    assert!(panel.agent.can_send());
}

#[test]
fn stop_then_failure_lets_the_next_message_go() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    panel.stop();
    let reaction = panel.agent.on_failed("se rompió".into());
    assert_eq!(panel.react(reaction), Reaction::default());
    assert!(panel.agent.can_send());
    assert!(matches!(
        panel.agent.log.entries().last(),
        Some(Entry::Error(_))
    ));
}

#[test]
fn failure_mid_turn_finishes_it() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    let reaction = panel.agent.on_failed("se rompió".into());
    assert_eq!(panel.react(reaction), finished(FAILED));
    assert!(panel.agent.can_send());
}

#[test]
fn closed_mid_turn_finishes_it_and_the_next_message_starts_anew() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    panel.agent.hear(AgentEvent::Message("Voy.".into()));
    let reaction = panel.agent.on_closed();
    assert!(panel.react(reaction).finish_turn);
    assert!(panel.agent.can_send());
    assert_eq!(panel.send("otra vez"), Sending::Start);
    let entries = panel.agent.log.entries();
    assert_eq!(
        entries[entries.len() - 2],
        Entry::Notice("Conversación nueva con Claude Code".into())
    );
    assert_eq!(entries[entries.len() - 1], Entry::User("otra vez".into()));
}

#[test]
fn start_failure_unlocks_and_the_next_message_starts_again() {
    let mut panel = Panel::new();
    assert_eq!(panel.send("hola"), Sending::Start);
    let reaction = panel.agent.fail_start("No se encontró `npx`.".into());
    // A session that fails before saying anything never started either.
    panel
        .agent
        .on_failed("Authentication required (-32000)".into());
    assert_eq!(panel.react(reaction), finished(NO_START));
    assert!(panel.agent.can_send());
    assert_eq!(panel.send("hola"), Sending::Start);
    // No session ever started, so nothing marks a new one.
    assert!(
        !panel
            .agent
            .log
            .entries()
            .iter()
            .any(|entry| matches!(entry, Entry::Notice(_)))
    );
}

#[test]
fn stop_during_startup_then_send_again() {
    let mut panel = Panel::new();
    assert_eq!(panel.send("hola"), Sending::Start);
    // Nothing reached an agent: there is nothing to wait for.
    assert_eq!(panel.stop(), finished(STOPPED));
    assert!(!panel.agent.stopping());
    assert!(panel.agent.can_send());
    // The session still starting takes the new message, not the old one.
    assert_eq!(panel.send("otra"), Sending::Wait);
    assert_eq!(panel.open().as_deref(), Some("otra"));
    let reaction = panel.agent.on_turn_ended();
    assert_eq!(panel.react(reaction), finished(FINISHED));
}

#[test]
fn second_stop_closes_the_agent() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    panel.agent.hear(tool("1", "mcp__levare__resumen", false));
    let first = panel.stop();
    assert_eq!(panel.stop(), status(STOPPED_HARD));
    assert!(!panel.agent.stopping());
    assert!(panel.agent.can_send());
    assert_eq!(
        panel.agent.stop_timeout(first.watch_stop.unwrap()),
        Reaction::default()
    );
    // A new session, marked in the log.
    assert_eq!(panel.send("sigue"), Sending::Start);
    assert!(
        panel
            .agent
            .log
            .entries()
            .contains(&Entry::Notice("Conversación nueva con Claude Code".into()))
    );
}

#[test]
fn silent_agent_is_closed_after_the_wait() {
    let mut panel = Panel::new();
    panel.send("hola");
    panel.open();
    let watch = panel.stop().watch_stop.unwrap();
    assert_eq!(panel.agent.stop_timeout(watch + 1), Reaction::default());
    assert!(panel.agent.stopping());
    assert_eq!(panel.agent.stop_timeout(watch), status(STOPPED_HARD));
    assert!(panel.agent.can_send());
}

#[test]
fn new_agent_marks_a_new_conversation() {
    let mut panel = Panel::new();
    panel.send("hola");
    // The first conversation has no mark.
    assert!(
        !panel
            .agent
            .log
            .entries()
            .iter()
            .any(|entry| matches!(entry, Entry::Notice(_)))
    );
    panel.open();
    let reaction = panel.agent.on_turn_ended();
    panel.react(reaction);
    panel.agent.kind = AgentKind::Codex;
    panel.agent.drop_session();
    panel.send("y ahora tú");
    assert!(
        panel
            .agent
            .log
            .entries()
            .contains(&Entry::Notice("Conversación nueva con Codex".into()))
    );
}

#[test]
fn stop_without_a_turn_does_nothing() {
    let mut panel = Panel::new();
    assert_eq!(panel.stop(), Reaction::default());
    assert!(panel.agent.can_send());
}
