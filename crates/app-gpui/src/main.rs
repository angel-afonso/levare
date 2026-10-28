// Release builds on Windows open no console window next to Levare.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod agent;
#[cfg(test)]
mod agent_tests;
mod app_icon;
mod app_paths;
mod autosave;
mod clipboard;
mod comments;
mod connection_input;
mod diagnostics;
mod file_dialogs;
mod follow;
mod fonts;
mod ghost;
mod keyboard;
mod mark_drag;
mod menu_typography;
mod music_font;
mod note_drag;
mod paint;
mod panels;
#[cfg(test)]
mod panels_tests;
mod playback;
mod playhead;
mod practice;
mod review;
mod selection;
mod shortcuts;
mod sound_banks;
mod staff_text;
#[cfg(feature = "vsco")]
mod store;
mod system_open;
mod ui;
mod updates;

use app_paths::data_path;
use gpui::{prelude::*, *};
use score_core::{
    Accidental, BarMarkRef, Barline, Connection, ConnectionKind, ElementId, Instrument, Jump,
    KeyMode, KeySignature, MarkKind, MarkRef, PartId, StaffId,
};
use score_document::Document;
use score_edit::{Command, Cursor, FormEdit};
use score_keymap::{CommandId, KeySequence, Keystroke as Key, Status, id};
use shortcuts::{
    AddInstrument, ChooseDynamic, ChooseKeySignature, MenuRun, OpenRecent, Run, Shortcuts,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    time::{Duration as Elapsed, Instant},
};

type QuitSnapshot = Option<score_io::Recovery>;

/// A key held longer than this without its key-up is a new press, not a repeat
/// (covers a key-up lost while the window was in the background).
const REPEAT_WINDOW: Elapsed = Elapsed::from_millis(700);
/// Minimum spacing between navigation auditions while a key is held.
const AUDITION_GAP: Elapsed = Elapsed::from_millis(90);

/// Held arrows sound at most every `AUDITION_GAP`, never as a burst.
fn audition_due(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|last| now.duration_since(last) >= AUDITION_GAP)
}

#[derive(Clone)]
enum Next {
    New,
    Open,
    OpenPath(PathBuf),
    Close,
}

struct AppView {
    #[cfg(feature = "vsco")]
    store: store::Store,
    document: Document,
    ui: ui::UiState,
    focus: FocusHandle,
    /// The file the score came from. Save replaces it only when it is a
    /// `.levare` document; see `file_dialogs::save_in_place`.
    path: Option<PathBuf>,
    status: String,
    busy: bool,
    confirming: bool,
    instrument_picker: Option<ui::InstrumentPicker>,
    export_picker: Option<ui::ExportPicker>,
    /// Acerca de Levare.
    about: bool,
    key_picker: bool,
    time_picker: bool,
    clef_picker: bool,
    tempo_picker: Option<ui::TempoEditor>,
    command_search: Option<ui::CommandSearch>,
    /// The bar number field of «Ir al compás», open in the transport.
    go_to_bar: Option<Entity<ui::text_input::TextInput>>,
    recent_searches: Vec<CommandId>,
    header_editor: Option<ui::HeaderEditor>,
    range_editor: Option<ui::RangeEditor>,
    comment_editor: Option<ui::CommentEditor>,
    text_editor: Option<ui::StaffTextEditor>,
    lyric_editor: Option<ui::LyricEditor>,
    chord_editor: Option<ui::ChordEditor>,
    comments: comments::CommentState,
    review: review::ReviewState,
    signature_from_start: bool,
    pending_time: score_core::TimeSignature,
    key_mode: KeyMode,
    chord_entry: bool,
    entry_accidental: Option<Accidental>,
    pending_slur: Option<ElementId>,
    pending_hairpin: Option<(ElementId, ConnectionKind)>,
    /// The line from the pending start to the last note typed in entry mode;
    /// it grows with each note until S, H or Esc ends it.
    open_line: Option<Connection>,
    connection_drag: Option<connection_input::ConnectionDrag>,
    /// A mark clicked in the score. Its note is the editor's selection, so
    /// the ribbon's toggles change or remove it; see `selected_mark`.
    clicked_mark: Option<MarkRef>,
    clicked_bar_mark: Option<BarMarkRef>,
    hover: Option<selection::Hover>,
    zoom_wheel: f32,
    reveal_selection: bool,
    pending_page: Option<usize>,
    canvas_origin: Point<Pixels>,
    selection_drag: Option<selection::PassageDrag>,
    note_drag: Option<note_drag::NoteDrag>,
    mark_drag: Option<mark_drag::MarkDrag>,
    ghost: Option<ghost::Ghost>,
    mixer_drag: Option<ui::MixerDrag>,
    viewport: follow::PlaybackViewport,
    /// Shared with the score canvas, which keeps it for painting.
    layout: Rc<score_layout::Layout>,
    /// Playback position of the frame being rendered, computed once per render.
    position: playhead::Position,
    music_font: Rc<music_font::MusicFont>,
    quit_snapshot: QuitSnapshot,
    playback: playback::Playback,
    shortcuts: Shortcuts,
    preferences: Option<ui::Preferences>,
    /// Set by the keystroke interceptor when the key being dispatched is an
    /// auto-repeat; consumed by the next `Run`.
    repeat_at: Option<Instant>,
    last_audition: Option<Instant>,
    held: HashMap<String, Instant>,
    recent: score_io::RecentFiles,
    recent_path: Option<PathBuf>,
    panels_path: Option<PathBuf>,
    /// What `paneles.json` holds, to write it again only when something changed.
    panels_saved: panels::PanelMemory,
    agent: agent::AgentState,
    /// The agent panel's message field.
    agent_field: Entity<ui::text_input::TextInput>,
    /// «Buscar parte» in the left panel.
    parts_search: Entity<ui::text_input::TextInput>,
    /// Set while one of the agent's tool calls runs a command, which lets it
    /// past the lock that holds the user's input during the turn.
    agent_acting: bool,
    updates: updates::Updates,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        recovery: Result<score_io::Recovery, String>,
        mut shortcuts: Shortcuts,
    ) -> Self {
        let recovery_file = recovery_path();
        let recovered = match &recovery {
            Ok(_) => recovery_file
                .as_deref()
                .map(|path| score_io::load_recovery(path).map_err(|e| e.to_string()))
                .transpose()
                .map(|value| value.flatten()),
            Err(reason) => Err(reason.clone()),
        };
        let mut quit_snapshot: QuitSnapshot = recovery.ok();
        let nothing_recovered = matches!(recovered, Ok(None));
        let mut unrestored = None;
        let (mut document, path, mut status) = match recovered {
            Ok(Some(recovered)) => match Document::recover(recovered.score) {
                Ok(document) => (
                    document,
                    recovered.original_path,
                    "Partitura recuperada. Revisa los cambios y guárdala para conservarlos."
                        .to_owned(),
                ),
                Err(error) => {
                    unrestored = Some(error.to_string());
                    (Document::default(), None, String::new())
                }
            },
            Ok(None) => (
                Document::default(),
                None,
                "Escribe tu primera nota con las teclas A–G".into(),
            ),
            Err(error) if quit_snapshot.is_some() => {
                unrestored = Some(error);
                (Document::default(), None, String::new())
            }
            Err(error) => (
                Document::default(),
                None,
                format!("Recuperación no disponible: {error}"),
            ),
        };
        // The file that failed may be the only copy of unsaved work: it moves
        // aside before the first edit autosaves over it.
        let mut kept_recovery = None;
        if let Some(error) = unrestored {
            let set_aside = recovery_file
                .as_deref()
                .map(|path| score_io::set_aside_recovery(path).map_err(|e| e.to_string()))
                .unwrap_or_else(|| Err("no se encontró la carpeta de datos de Levare".into()));
            status = match set_aside {
                Ok(kept) => {
                    let name = kept.file_name().unwrap_or_default().to_string_lossy();
                    let message = format!(
                        "No se pudo recuperar: {error}. El archivo de recuperación se guardó \
                         aparte como «{name}»."
                    );
                    kept_recovery = Some(kept);
                    message
                }
                Err(why) => {
                    eprintln!("No se pudo apartar la recuperación: {why}");
                    quit_snapshot = None;
                    format!(
                        "No se pudo recuperar: {error}. El autoguardado queda desactivado para \
                         no reemplazar el archivo de recuperación."
                    )
                }
            };
        }
        let recent_path = data_path("recientes.json");
        let recent = recent_path
            .as_deref()
            .and_then(|path| score_io::RecentFiles::load(path).ok())
            .unwrap_or_default();
        let panels_path = data_path("paneles.json");
        let panels_saved = panels_path.as_deref().map(panels::load).unwrap_or_default();
        shortcuts.recent = recent.files().to_vec();
        shortcuts.install(cx);
        let layout = Rc::new(document.layout(960.));
        let playback = playback::Playback::new();
        let position = playback.position(document.editor.score(), document.editor.cursor());
        let focus = cx.focus_handle();
        focus.focus(window);
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            view.update(cx, |view, cx| {
                if view.busy || view.confirming || !view.agent_allows_close(cx) {
                    return false;
                }
                if view.document.is_dirty() {
                    view.request_next(Next::Close, window, cx);
                    false
                } else {
                    true
                }
            })
            .unwrap_or(true)
        });

        if let Some(kept) = kept_recovery {
            cx.defer_in(window, move |view, window, cx| {
                view.show_kept_recovery(kept, window, cx)
            });
        }

        let updates = updates::Updates::new();
        updates::start(&updates, cx);
        // After an update's restart: the saved score that was open. Unsaved
        // work comes back through the autosave above instead.
        if let Some(score) = updates::take_reopen().filter(|_| nothing_recovered) {
            cx.defer_in(window, move |view, window, cx| {
                view.open(Some(score), window, cx)
            });
        }

        cx.spawn(async move |view, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(16)).await;
                if view
                    .update(cx, |view: &mut Self, cx| {
                        if let Some(error) =
                            view.quit_snapshot.as_ref().and_then(|r| r.take_error())
                        {
                            view.status = format!("No se pudo autoguardar: {error}");
                            cx.notify();
                        }
                        #[cfg(feature = "vsco")]
                        if view.store.moved() {
                            cx.notify();
                        }
                        if view.playback.tick() {
                            let cursor = view.document.editor.cursor();
                            let position = view.playback.frame_position(&view.document.editor);
                            view.viewport.follow(
                                &view.layout,
                                view.playback
                                    .playhead
                                    .filter(|id| id.measure == position.measure),
                                playhead::tracking_bounds(&view.layout, cursor, position),
                                view.playback.is_playing(),
                            );
                            if view.playback.just_finished {
                                view.status = "Reproducción finalizada".into();
                            }
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let view = cx.weak_entity();
        let interceptor = cx.intercept_keystrokes(move |event, window, cx| {
            let _ = view.update(cx, |view, cx| view.intercept(event, window, cx));
        });
        let view = cx.weak_entity();
        let layout_change = cx.on_keyboard_layout_change(move |cx| {
            let _ = view.update(cx, |view: &mut Self, cx| {
                view.shortcuts.refresh_layout(cx);
                view.shortcuts.install(cx);
                cx.notify();
            });
        });
        Self {
            document,
            ui: ui::UiState::restore(panels_saved.clone()),
            focus,
            path,
            status,
            busy: false,
            confirming: false,
            instrument_picker: None,
            export_picker: None,
            about: false,
            key_picker: false,
            time_picker: false,
            clef_picker: false,
            tempo_picker: None,
            command_search: None,
            go_to_bar: None,
            recent_searches: Vec::new(),
            header_editor: None,
            range_editor: None,
            comment_editor: None,
            text_editor: None,
            lyric_editor: None,
            chord_editor: None,
            comments: comments::CommentState::default(),
            review: review::ReviewState::default(),
            signature_from_start: false,
            pending_time: Default::default(),
            key_mode: KeyMode::Major,
            chord_entry: false,
            entry_accidental: None,
            pending_slur: None,
            pending_hairpin: None,
            open_line: None,
            connection_drag: None,
            clicked_mark: None,
            clicked_bar_mark: None,
            hover: None,
            zoom_wheel: 0.,
            reveal_selection: false,
            pending_page: None,
            canvas_origin: Point::default(),
            selection_drag: None,
            note_drag: None,
            mark_drag: None,
            ghost: None,
            mixer_drag: None,
            #[cfg(feature = "vsco")]
            store: store::Store::new(),
            viewport: follow::PlaybackViewport::default(),
            layout,
            position,
            music_font: Rc::new(music_font::MusicFont::new()),
            quit_snapshot,
            playback,
            shortcuts,
            preferences: None,
            repeat_at: None,
            last_audition: None,
            held: HashMap::new(),
            recent,
            recent_path,
            panels_path,
            panels_saved,
            agent: agent::AgentState::default(),
            agent_field: ui::agent_panel::field(cx),
            parts_search: ui::parts_search_field(cx),
            agent_acting: false,
            updates,
            _subscriptions: vec![interceptor, layout_change],
        }
    }

    fn clear_recovery(&self) {
        if let Some(recovery) = &self.quit_snapshot {
            recovery.update(None);
        }
    }

    fn sync_quit_snapshot(&self) {
        if let Some(recovery) = &self.quit_snapshot {
            recovery.update(self.unsaved_snapshot());
        }
    }

    /// What the autosave holds: the score while it has unsaved changes.
    fn unsaved_snapshot(&self) -> Option<score_io::RecoveredScore> {
        self.document.is_dirty().then(|| score_io::RecoveredScore {
            score: self.document.editor.score().clone(),
            original_path: self.path.clone(),
        })
    }

    /// Says where the recovery file that could not be restored went, so the
    /// musician can keep it or send it on instead of wondering about the work.
    fn show_kept_recovery(&mut self, kept: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.confirming = true;
        let name = kept.file_name().unwrap_or_default().to_string_lossy();
        let folder = kept.parent().unwrap_or(&kept).display();
        let detail = format!(
            "Levare guardó el archivo de recuperación aparte, sin cambios, como «{name}» en \
             {folder}."
        );
        let answer = window.prompt(
            PromptLevel::Warning,
            "No se pudo recuperar la partitura sin guardar",
            Some(&detail),
            &["Mostrar el archivo", "Aceptar"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let reveal = answer.await == Ok(0);
            let _ = view.update(cx, |view, cx| {
                view.confirming = false;
                if reveal {
                    cx.reveal_path(&kept);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Every catalog mark, from the ribbon, a menu or a shortcut. Point marks
    /// toggle on the selection: the active one is removed, another one of its
    /// family replaces it. Connections are entered from both ends.
    fn apply_mark(&mut self, kind: MarkKind, cx: &mut Context<Self>) {
        match kind {
            MarkKind::Connection(ConnectionKind::Tie) => self.toggle_tie(cx),
            MarkKind::Connection(ConnectionKind::Slur) => self.toggle_slur(cx),
            MarkKind::Connection(kind) if kind.is_line() => self.toggle_line(kind, cx),
            MarkKind::Connection(kind) => self.toggle_hairpin(kind, cx),
            kind => self.edit(Command::ToggleMark(kind), cx),
        }
    }
    /// A single selected note inside an existing connection of this kind. Its
    /// final note is excluded, so a new phrase or hairpin can still start there.
    fn connection_at_selection(&self, kind: ConnectionKind) -> Option<Connection> {
        let editor = &self.document.editor;
        let id = editor
            .selection()
            .filter(|_| editor.selected_ids().len() == 1)?;
        editor
            .score()
            .connections()
            .iter()
            .copied()
            .find(|c| c.kind == kind && c.end != id && editor.score().connection_contains(*c, id))
    }
    /// A line over the selected passage. A single note takes an octave line,
    /// pedal or trill of its own, and a glissando to the next note; the same
    /// command on a written line removes it.
    fn toggle_line(&mut self, kind: ConnectionKind, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if let Some(MarkRef::Connection(old)) = self.selected_mark()
            && old.kind == kind
        {
            self.edit(Command::RemoveConnection(old), cx);
            return;
        }
        let editor = &self.document.editor;
        let score = editor.score();
        let ids = editor.selected_ids();
        let (Some(&start), Some(&last)) = (ids.first(), ids.last()) else {
            self.status = format!("{}: selecciona primero sus notas", kind.label());
            cx.notify();
            return;
        };
        let end = if kind == ConnectionKind::Glissando && start == last {
            score.next_event(start)
        } else {
            Some(last)
        };
        let written = score.connections().iter().copied().find(|c| {
            c.kind == kind
                && ((c.start == start && Some(c.end) == end)
                    || (ids.len() == 1 && kind.is_range() && score.connection_contains(*c, start)))
        });
        match (written, end) {
            (Some(connection), _) => self.edit(Command::RemoveConnection(connection), cx),
            (None, Some(end)) => {
                self.edit(Command::AddConnection(Connection { start, end, kind }), cx)
            }
            (None, None) => {
                self.status = "Escribe primero la nota en la que acaba el glissando".into();
                cx.notify();
            }
        }
    }
    fn toggle_tie(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let score = self.document.editor.score();
        let Some(start) = self.document.editor.selection() else {
            self.status = "Selecciona el inicio de la ligadura de unión".into();
            cx.notify();
            return;
        };
        if let Some(connection) = score
            .connections()
            .iter()
            .find(|c| c.kind == ConnectionKind::Tie && c.start == start)
            .copied()
        {
            self.edit(Command::RemoveConnection(connection), cx);
        } else if let Some(end) = score.next_event(start) {
            self.edit(
                Command::AddConnection(Connection {
                    start,
                    end,
                    kind: ConnectionKind::Tie,
                }),
                cx,
            );
        } else {
            self.status = "Escribe primero la siguiente nota o acorde de igual altura".into();
            cx.notify();
        }
    }
    /// The clicked mark while it still exists and its note stays selected
    /// alone; any other selection or edit that removes it lets it go.
    pub(crate) fn selected_mark(&self) -> Option<MarkRef> {
        let editor = &self.document.editor;
        self.clicked_mark.filter(|mark| {
            editor.score().has_mark(*mark)
                && editor.selection() == Some(mark.anchor())
                && editor.selected_ids().len() == 1
        })
    }
    /// The line whose ends show drag handles: the one being dragged, else a
    /// picked slur or hairpin.
    pub(crate) fn line_handles(&self) -> Option<Connection> {
        if let Some(drag) = &self.connection_drag {
            return drag.candidate.or(drag.original);
        }
        match self.selected_mark() {
            Some(MarkRef::Connection(line)) if line.kind != ConnectionKind::Tie => Some(line),
            _ => None,
        }
    }
    pub(crate) fn selected_bar_mark(&self) -> Option<BarMarkRef> {
        let editor = &self.document.editor;
        self.clicked_bar_mark.filter(|mark| {
            editor.score().has_bar_mark(*mark)
                && editor.selection().is_none()
                && editor.cursor().measure == mark.measure
        })
    }

    fn nudge_bar_mark(&mut self, mark: BarMarkRef, dx: i16, dy: i16, cx: &mut Context<Self>) {
        let base = self.document.editor.score().bar_mark_offset(mark);
        let max = i32::from(score_core::Offset::MAX);
        let offset = score_core::Offset {
            dx: (i32::from(base.dx) + i32::from(dx)).clamp(-max, max) as i16,
            dy: (i32::from(base.dy) + i32::from(dy)).clamp(-max, max) as i16,
        };
        if offset == base {
            return;
        }
        if self.try_edit(Command::SetBarMarkOffset(mark, offset), cx) {
            self.clicked_bar_mark = Some(mark);
            self.reveal_selection = false;
            self.status = format!(
                "{} · {} la devuelve a su sitio",
                mark_drag::DragTarget::Bar(mark).label(self.document.editor.score()),
                self.shortcuts.hint(id::RESET_POSITION)
            );
            cx.notify();
        }
    }
    /// Take a mark that follows time to another note; it then sounds there.
    fn move_mark(
        &mut self,
        from: MarkRef,
        to: MarkRef,
        offset: score_core::Offset,
        cx: &mut Context<Self>,
    ) {
        let label = self
            .document
            .editor
            .score()
            .mark_label(from)
            .unwrap_or_default();
        if self.try_edit(Command::MoveMark(from, to, offset), cx) {
            self.document.editor.select(to.anchor());
            self.clicked_mark = Some(to);
            self.status = format!(
                "{label}: {} · {} para deshacer",
                mark_drag::place_label(self.document.editor.score(), to.anchor()),
                self.shortcuts.hint(id::UNDO)
            );
        }
        cx.notify();
    }
    /// ⌥← and ⌥→ move a mark that follows time one note earlier or later.
    fn step_mark(&mut self, mark: MarkRef, later: bool, cx: &mut Context<Self>) {
        let score = self.document.editor.score();
        let Some(to) = mark_drag::stepped(score, mark, later) else {
            self.status = if matches!(mark, MarkRef::Connection(_)) {
                "El regulador no cabe ahí".into()
            } else if later {
                "No hay más notas después en esta voz".into()
            } else {
                "No hay más notas antes en esta voz".into()
            };
            cx.notify();
            return;
        };
        let offset = score_core::Offset {
            dx: 0,
            dy: score.mark_offset(mark).dy,
        };
        self.move_mark(mark, to, offset, cx);
    }
    fn nudge_mark(&mut self, mark: MarkRef, dx: i16, dy: i16, cx: &mut Context<Self>) {
        let base = self.document.editor.score().mark_offset(mark);
        let max = i32::from(score_core::Offset::MAX);
        let offset = score_core::Offset {
            dx: (i32::from(base.dx) + i32::from(dx)).clamp(-max, max) as i16,
            dy: (i32::from(base.dy) + i32::from(dy)).clamp(-max, max) as i16,
        };
        if self.try_edit(Command::SetMarkOffset(mark, offset), cx) {
            self.clicked_mark = Some(mark);
            let label = self
                .document
                .editor
                .score()
                .mark_label(mark)
                .unwrap_or_default();
            self.status = format!(
                "{label} · {} la devuelve a su sitio",
                self.shortcuts.hint(id::RESET_POSITION)
            );
            cx.notify();
        }
    }
    /// The line following the typed notes, while its start is still pending.
    fn following_line(&self, kind: ConnectionKind) -> Option<Connection> {
        let start = if kind == ConnectionKind::Slur {
            self.pending_slur
        } else {
            self.pending_hairpin
                .filter(|(_, pending)| *pending == kind)
                .map(|(start, _)| start)
        }?;
        self.open_line
            .filter(|line| line.kind == kind && line.start == start)
            .filter(|line| self.document.editor.score().connections().contains(line))
    }
    /// S or H on a line that follows the typed notes ends it where it is.
    fn close_open_line(&mut self, kind: ConnectionKind, cx: &mut Context<Self>) -> bool {
        if self.following_line(kind).is_none() {
            return false;
        }
        self.pending_slur = None;
        self.pending_hairpin = None;
        self.open_line = None;
        let (name, ending) = connection_words(kind);
        self.status = format!("{name} terminad{ending}");
        cx.notify();
        true
    }
    /// After a note typed in entry mode, stretch the pending line to it. One
    /// undo removes the note and gives the line back its previous end. A line
    /// that cannot reach the note ends where it was; the message says why.
    fn extend_open_line(&mut self) -> Option<String> {
        let (start, kind) = match (self.pending_slur, self.pending_hairpin) {
            (Some(start), _) => (start, ConnectionKind::Slur),
            (None, Some(pending)) => pending,
            (None, None) => return None,
        };
        let end = self.document.editor.selection()?;
        let (next, command) = connection_input::follow_command(
            self.document.editor.score(),
            start,
            end,
            kind,
            self.following_line(kind),
        )?;
        match self.document.editor.execute_joined(command) {
            Ok(_) => {
                self.open_line = Some(next);
                None
            }
            Err(error) => {
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.open_line = None;
                let (name, ending) = connection_words(kind);
                Some(format!("{name} terminad{ending}: {}", error.0))
            }
        }
    }
    fn line_prompt(&self, kind: ConnectionKind) -> String {
        let command = match kind {
            ConnectionKind::Slur => id::SLUR,
            ConnectionKind::Crescendo => id::CRESCENDO,
            _ => id::DIMINUENDO,
        };
        let key = self
            .shortcuts
            .label(command)
            .map_or_else(String::new, |key| format!(" ({key})"));
        let (name, ending) = connection_words(kind);
        if self.ui.note_entry {
            format!(
                "{name} abiert{ending}: sigue a las notas que escribas; pulsa de nuevo{key} o Esc para terminar"
            )
        } else {
            format!("{name}: selecciona la nota final y pulsa de nuevo{key}. Esc cancela")
        }
    }
    fn toggle_hairpin(&mut self, kind: ConnectionKind, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.close_open_line(kind, cx) {
            return;
        }
        // A hairpin picked in the score: the same kind removes it, the other
        // one turns it around in place.
        if let Some(MarkRef::Connection(old)) = self.selected_mark()
            && old.kind.is_hairpin()
        {
            if old.kind == kind {
                self.edit(Command::RemoveConnection(old), cx);
            } else {
                let new = Connection { kind, ..old };
                self.edit(Command::ReplaceConnection(old, new), cx);
                if self.document.editor.score().connections().contains(&new) {
                    self.clicked_mark = Some(MarkRef::Connection(new));
                }
            }
            return;
        }
        if self
            .pending_hairpin
            .is_none_or(|(_, pending)| pending != kind)
            && let Some(connection) = self.connection_at_selection(kind)
        {
            self.edit(Command::RemoveConnection(connection), cx);
            return;
        }
        let ids = self.document.editor.selected_ids();
        let Some(&selected) = ids.last() else {
            self.status =
                "Selecciona el inicio del regulador o un pasaje de dos o más notas".into();
            cx.notify();
            return;
        };
        let endpoints = if ids.len() > 1 {
            Some((ids[0], selected))
        } else if let Some((start, _)) = self.pending_hairpin.filter(|(_, k)| *k == kind) {
            if start == selected {
                self.pending_hairpin = None;
                self.status = "Regulador cancelado".into();
                cx.notify();
                return;
            }
            Some((start, selected))
        } else {
            None
        };
        if let Some((start, end)) = endpoints {
            self.edit(Command::AddConnection(Connection { start, end, kind }), cx);
        } else {
            self.pending_slur = None;
            self.pending_hairpin = Some((selected, kind));
            self.open_line = None;
            self.status = self.line_prompt(kind);
            cx.notify();
        }
    }
    fn toggle_slur(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.close_open_line(ConnectionKind::Slur, cx) {
            return;
        }
        let ids = self.document.editor.selected_ids();
        if ids.len() > 1 {
            let score = self.document.editor.score();
            match connection_input::passage_endpoints(score, &ids, ConnectionKind::Slur) {
                Some((start, end)) => {
                    let slur = Connection {
                        start,
                        end,
                        kind: ConnectionKind::Slur,
                    };
                    // The same passage again takes its slur away.
                    let command = if score.connections().contains(&slur) {
                        Command::RemoveConnection(slur)
                    } else {
                        Command::AddConnection(slur)
                    };
                    self.edit(command, cx);
                }
                None => {
                    self.status = "El pasaje necesita al menos dos notas o acordes".into();
                    cx.notify();
                }
            }
            return;
        }
        let Some(id) = self.document.editor.selection() else {
            self.status = "Selecciona una nota para la ligadura de expresión".into();
            cx.notify();
            return;
        };
        if self.pending_slur.is_none()
            && let Some(connection) = self.connection_at_selection(ConnectionKind::Slur)
        {
            self.edit(Command::RemoveConnection(connection), cx);
            return;
        }
        if let Some(start) = self.pending_slur {
            if id == start {
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.status = "Ligadura cancelada".into();
                cx.notify();
                return;
            }
            self.edit(
                Command::AddConnection(Connection {
                    start,
                    end: id,
                    kind: ConnectionKind::Slur,
                }),
                cx,
            );
        } else if self
            .document
            .editor
            .score()
            .event(id)
            .is_some_and(|e| !e.pitches.is_empty())
        {
            self.pending_slur = Some(id);
            self.pending_hairpin = None;
            self.open_line = None;
            self.status = self.line_prompt(ConnectionKind::Slur);
            cx.notify();
        } else {
            self.status = "Elige una nota o acorde, no un silencio".into();
            cx.notify();
        }
    }

    fn edit(&mut self, command: Command, cx: &mut Context<Self>) {
        self.try_edit(command, cx);
    }

    fn try_edit(&mut self, command: Command, cx: &mut Context<Self>) -> bool {
        self.selection_drag = None;
        self.note_drag = None;
        self.mark_drag = None;
        self.connection_drag = None;
        self.ghost = None;
        if self.busy || self.confirming || self.agent_locked() || self.instrument_picker.is_some() {
            return false;
        }
        self.clicked_bar_mark = None;
        let tempo_tick = (matches!(
            command,
            Command::SetTempoAt(_, _) | Command::RemoveTempoChange(_)
        ) && (self.playback.has_ruler_position()
            || self.playback.is_playing()
            || self.playback.is_paused()))
        .then(|| {
            self.playback
                .position(self.document.editor.score(), self.document.editor.cursor())
                .tick
                .floor() as u64
        });
        let mut changed = false;
        let mut line_message = None;
        self.status = match self.document.execute(command.clone()) {
            Ok(true) => {
                changed = true;
                // Typing in entry mode carries a pending slur or hairpin along.
                let typed = self.ui.note_entry
                    && matches!(
                        command,
                        Command::Insert(_)
                            | Command::Write(_)
                            | Command::WriteAt { .. }
                            | Command::AddPitch(_)
                    );
                if typed {
                    line_message = self.extend_open_line();
                } else {
                    self.pending_slur = None;
                    self.pending_hairpin = None;
                    self.open_line = None;
                }
                if !matches!(
                    command,
                    Command::SetHeader(..)
                        | Command::SetInstrumentRange(..)
                        | Command::SaveComment { .. }
                        | Command::SetCommentResolved(..)
                        | Command::RemoveComment(_)
                ) && !self.agent_acting
                {
                    // The agent's edits leave the user's playback running.
                    self.playback.stop();
                }
                if let Some(tick) = tempo_tick {
                    let _ = self.playback.seek(self.document.editor.score(), tick);
                }

                self.reveal_selection = true;
                if !self.agent_acting
                    && matches!(
                        command,
                        Command::Insert(_)
                            | Command::Write(_)
                            | Command::WriteAt { .. }
                            | Command::ReplacePitch(_)
                            | Command::Transpose(_)
                            | Command::AddPitch(_)
                            | Command::RemoveHighestPitch
                            | Command::SetAccidental(_)
                            | Command::SetKeySignature(_)
                            | Command::SetKeySignatureAt(_, _)
                            | Command::RemoveKeyChange(_)
                            | Command::SetTechnique(_)
                            | Command::SetDynamic(_)
                            | Command::SetArticulation(_)
                            | Command::ToggleMark(_)
                            | Command::SetArpeggioBothHands(_)
                    )
                    && let Some(id) = self.document.editor.selection()
                    && self
                        .document
                        .editor
                        .score()
                        .event(id)
                        .is_some_and(|event| !event.pitches.is_empty())
                    && let Err(error) = self.playback.preview(self.document.editor.score(), id)
                {
                    self.status = format!("Nota escrita. Audición: {error}");
                    self.sync_quit_snapshot();
                    cx.notify();
                    return true;
                }
                match command {
                    Command::SetPassageDivisi(true) => "Divisi desde la posición seleccionada; si elegiste un pasaje, solo en ese tramo. Puedes deshacer.".into(),
                    Command::SetPassageDivisi(false) => "Unis. desde la posición seleccionada: ambas voces en un pentagrama.".into(),
                    Command::SetPartDivisi(true) => "Parte dividida: voz 1 en el pentagrama I y voz 2 en el II. Puedes deshacer.".into(),
                    Command::SetPartDivisi(false) => "Pentagramas reunidos en voces 1 y 2. La música se conserva.".into(),
                    Command::RemovePart | Command::RemovePartById(_) => {
                        "Instrumento eliminado. Puedes recuperarlo con Deshacer.".into()
                    }
                    Command::AddPart(_) => "Instrumento añadido y colocado en el score.".into(),
                    Command::AddParts(parts) => format!(
                        "{} instrumentos añadidos. Puedes deshacerlos juntos.",
                        parts.len()
                    ),
                    Command::SetHeader(..) => "Cabecera actualizada. Puedes deshacer.".into(),
                    Command::SetPlayers(part, _) => {
                        let score = self.document.editor.score();
                        format!(
                            "{}: grabaciones de {} · {} para deshacer",
                            ui::instrument_name(score, part),
                            match score.players(part) {
                                score_core::Players::Solo => "solista",
                                score_core::Players::Section => "sección",
                            },
                            self.shortcuts.hint(id::UNDO)
                        )
                    }
                    _ => "Partitura actualizada".into(),
                }
            }
            Ok(false) => self.status.clone(),
            Err(error) => error.to_string(),
        };
        if let Some(message) = line_message {
            self.status = message;
        }
        self.sync_quit_snapshot();
        cx.notify();
        changed
    }

    /// Runs before any binding: marks auto-repeats and feeds the recorder in
    /// Preferences, which must see keys that are already bound.
    fn intercept(&mut self, event: &KeystrokeEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ghost_cleared = self.ghost.take().is_some();
        let hover_cleared = self.hover.take().is_some();
        if ghost_cleared || hover_cleared {
            cx.notify();
        }
        let stroke = &event.keystroke;
        if let Some(drag) = self.note_drag.take() {
            self.selection_drag = None;
            cx.notify();
            if stroke.key == "escape" {
                if drag.active {
                    self.status = "Cambio de nota cancelado".into();
                }
                cx.stop_propagation();
                return;
            }
        }
        if let Some(drag) = self.mark_drag.take() {
            cx.notify();
            if stroke.key == "escape" {
                if drag.active {
                    self.status = "Movimiento de la marca cancelado".into();
                }
                cx.stop_propagation();
                return;
            }
        }
        if let Some(drag) = self.connection_drag.take() {
            self.selection_drag = None;
            if let Some(line) = drag.original {
                self.clicked_mark = Some(MarkRef::Connection(line));
            }
            cx.notify();
            if stroke.key == "escape" {
                self.status = "Dibujo cancelado".into();
                cx.stop_propagation();
                return;
            }
        }
        if self.export_picker.is_some() {
            cx.stop_propagation();
            if !stroke.modifiers.modified() {
                if stroke.key == "escape" {
                    self.close_export_picker(cx);
                } else if stroke.key == "enter" {
                    self.confirm_export(window, cx);
                }
            }
            return;
        }
        if self.about {
            cx.stop_propagation();
            if !stroke.modifiers.modified() && matches!(stroke.key.as_str(), "escape" | "enter") {
                self.close_about(cx);
            }
            return;
        }
        let now = Instant::now();
        let repeated = self
            .held
            .insert(stroke.key.clone(), now)
            .is_some_and(|last| now.duration_since(last) < REPEAT_WINDOW);
        self.repeat_at = repeated.then_some(now);
        if self.instrument_picker.is_some() && stroke.key == "enter" && !stroke.modifiers.modified()
        {
            cx.stop_propagation();
            if !repeated {
                self.confirm_instruments(cx);
            }
            return;
        }
        if self
            .command_search
            .as_ref()
            .is_some_and(|search| search.focused(window, cx))
        {
            let count = self.command_results(cx).len();
            match stroke.key.as_str() {
                "down" | "up" => {
                    cx.stop_propagation();
                    if let Some(search) = &mut self.command_search {
                        search.selected = if stroke.key == "down" {
                            (search.selected + 1).min(count.saturating_sub(1))
                        } else {
                            search.selected.saturating_sub(1)
                        };
                    }
                }
                "enter" if !stroke.modifiers.modified() => {
                    cx.stop_propagation();
                    let selected = self.command_search.as_ref().map_or(0, |s| s.selected);
                    if let Some(result) = self.command_results(cx).get(selected) {
                        self.run_search_result(result.item.command, window, cx);
                    }
                }
                "escape" => {
                    cx.stop_propagation();
                    self.command_search = None;
                    window.focus(&self.focus);
                }
                _ => {
                    if let Some(search) = &mut self.command_search {
                        search.selected = 0;
                    }
                }
            }
            cx.notify();
            return;
        }
        if self.agent_field_focused(window, cx) {
            match stroke.key.as_str() {
                "enter" if !stroke.modifiers.modified() => {
                    cx.stop_propagation();
                    if !repeated {
                        self.send_agent_field(window, cx);
                    }
                }
                "escape" => {
                    cx.stop_propagation();
                    window.focus(&self.focus);
                    cx.notify();
                }
                _ => {}
            }
            return;
        }
        if self.parts_search_focused(window, cx) {
            match stroke.key.as_str() {
                "enter" if !stroke.modifiers.modified() => {
                    cx.stop_propagation();
                    self.choose_found_part(window, cx);
                }
                "escape" => {
                    cx.stop_propagation();
                    self.clear_parts_search(window, cx);
                }
                _ => {}
            }
            return;
        }
        if self
            .go_to_bar
            .as_ref()
            .is_some_and(|field| field.focus_handle(cx).is_focused(window))
        {
            match stroke.key.as_str() {
                "enter" if !stroke.modifiers.modified() => {
                    cx.stop_propagation();
                    self.confirm_go_to_bar(window, cx);
                }
                "escape" => {
                    cx.stop_propagation();
                    self.close_go_to_bar(window, cx);
                }
                _ => {}
            }
            return;
        }
        if self
            .lyric_editor
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
        {
            let plain = !stroke.modifiers.modified();
            let join = match stroke.key.as_str() {
                "space" if plain => Some(score_core::Join::Word),
                "-" if stroke.modifiers.shift => Some(score_core::Join::Melisma),
                "-" if plain => Some(score_core::Join::Hyphen),
                _ if stroke.key_char.as_deref() == Some("_") => Some(score_core::Join::Melisma),
                _ => None,
            };
            if let Some(join) = join {
                cx.stop_propagation();
                self.apply_syllable(join, true, cx);
            } else if stroke.key == "enter" && plain {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_syllable(score_core::Join::Word, false, cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.lyric_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if self
            .chord_editor
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
        {
            if stroke.key == "tab" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                self.apply_chord(true, cx);
            } else if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_chord(false, cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.chord_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if self
            .text_editor
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
        {
            if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_text(cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.text_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if self
            .tempo_picker
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
        {
            if stroke.key == "tab" {
                cx.stop_propagation();
                if let Some(editor) = &self.tempo_picker {
                    let focus = if editor.text.focus_handle(cx).is_focused(window) {
                        editor.bpm.focus_handle(cx)
                    } else {
                        editor.text.focus_handle(cx)
                    };
                    focus.focus(window);
                }
                return;
            }
            if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_tempo(cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.tempo_picker = None;
                self.header_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if let Some(editor) = self
            .header_editor
            .as_ref()
            .filter(|editor| editor.focused(window, cx))
        {
            if stroke.key == "tab" {
                cx.stop_propagation();
                editor.focus_next(stroke.modifiers.shift, window, cx);
            } else if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_header(cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.header_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if let Some(editor) = self
            .range_editor
            .as_ref()
            .filter(|editor| editor.focused(window, cx))
        {
            if stroke.key == "tab" {
                cx.stop_propagation();
                editor.focus_next(window, cx);
            } else if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_range(cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.range_editor = None;
                window.focus(&self.focus);
                cx.notify();
            }
            return;
        }
        if self
            .comment_editor
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
        {
            if stroke.key == "enter" && !stroke.modifiers.modified() {
                cx.stop_propagation();
                window.focus(&self.focus);
                self.apply_comment(cx);
            } else if stroke.key == "escape" {
                cx.stop_propagation();
                self.comment_editor = None;
                window.focus(&self.focus);
                cx.notify();
            } else if stroke.key == "tab" {
                cx.stop_propagation();
                window.focus(&self.focus);
            }
            return;
        }
        if let Some(mark) = self.selected_mark()
            && mark.follows_time()
            && self
                .preferences
                .as_ref()
                .is_none_or(|p| p.recording.is_none())
            && let Some(later) = mark_drag::step(&stroke.key, stroke.modifiers)
        {
            cx.stop_propagation();
            self.step_mark(mark, later, cx);
            return;
        }
        // A selected mark is nudged by physical arrows, leaving Tab for navigation.
        if let Some(mark) = self.selected_mark()
            && mark.can_move()
            && self
                .preferences
                .as_ref()
                .is_none_or(|p| p.recording.is_none())
            && let Some((dx, dy)) = mark_drag::nudge(&stroke.key, stroke.modifiers)
        {
            cx.stop_propagation();
            self.nudge_mark(mark, dx, dy, cx);
            return;
        }
        if let Some(mark) = self.selected_bar_mark()
            && self
                .preferences
                .as_ref()
                .is_none_or(|p| p.recording.is_none())
            && let Some((dx, dy)) = mark_drag::nudge(&stroke.key, stroke.modifiers)
        {
            cx.stop_propagation();
            self.nudge_bar_mark(mark, dx, dy, cx);
            return;
        }
        let Some(preferences) = &self.preferences else {
            return;
        };
        if preferences.recording.is_none() {
            return;
        }
        cx.stop_propagation();
        if repeated {
            return;
        }
        let modifiers = stroke.modifiers;
        if stroke.key == "escape" && !modifiers.modified() {
            self.cancel_recording(cx);
            return;
        }
        let key = Key::from_platform(
            &stroke.key,
            modifiers.control && !modifiers.secondary(),
            modifiers.alt,
            modifiers.shift,
            modifiers.secondary(),
            &self.shortcuts.layout,
        );
        self.record(KeySequence::single(key), cx);
    }

    fn key_up(&mut self, event: &KeyUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.held.remove(&event.keystroke.key);
    }

    /// A key binding fired. Held keys repeat only commands marked as repeatable.
    fn run_key(&mut self, command: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        let repeated = self
            .repeat_at
            .take()
            .is_some_and(|at| at.elapsed() < Elapsed::from_millis(100));
        if repeated
            && self
                .shortcuts
                .spec(command)
                .is_some_and(|spec| !spec.repeat)
        {
            return;
        }
        self.run(command, window, cx);
    }

    /// The single entry point for keys, menus and toolbar buttons.
    fn run(&mut self, command: CommandId, window: &mut Window, cx: &mut Context<Self>) {
        self.note_drag = None;
        self.mark_drag = None;
        if self
            .tempo_picker
            .as_ref()
            .is_some_and(|editor| editor.focused(window, cx))
            || self
                .header_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .range_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .comment_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .text_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .lyric_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .chord_editor
                .as_ref()
                .is_some_and(|editor| editor.focused(window, cx))
            || self
                .command_search
                .as_ref()
                .is_some_and(|search| search.focused(window, cx))
            || self.agent_field_focused(window, cx)
            || self.parts_search_focused(window, cx)
        {
            // TextInput owns writing/navigation/clipboard keys while it has focus;
            // the agent's commands are not keys, and the agent panel, which has
            // no key, closes from the menu while its field is in use.
            if !self.agent_acting && command != id::TOGGLE_AGENT_PANEL {
                return;
            }
        }
        self.selection_drag = None;
        let Some(spec) = self.shortcuts.spec(command).copied() else {
            return;
        };
        if self.busy || self.confirming {
            return;
        }
        if !agent::user_may_run(spec.category, self.agent_locked()) {
            self.status = agent::lock_message(command).into();
            cx.notify();
            return;
        }
        if self.instrument_picker.is_some() {
            if [id::CLOSE_PANEL, id::ESCAPE, id::ADD_INSTRUMENT].contains(&command) {
                self.close_instrument_picker(cx);
            }
            return;
        }
        if self.preferences.is_some()
            && [
                id::COPY,
                id::CUT,
                id::PASTE,
                id::SELECT_ALL,
                id::EXTEND_PREVIOUS,
                id::EXTEND_NEXT,
            ]
            .contains(&command)
        {
            return;
        }
        if spec.status == Status::Pending {
            self.status = match self.shortcuts.label(command) {
                Some(keys) => format!("Pendiente: {} ({keys})", spec.label),
                None => format!("Pendiente: {}", spec.label),
            };
            cx.notify();
            return;
        }
        let note_input = [
            id::ADD_C,
            id::ADD_D,
            id::ADD_E,
            id::ADD_F,
            id::ADD_G,
            id::ADD_A,
            id::ADD_B,
            id::INTERVAL_1,
            id::INTERVAL_2,
            id::INTERVAL_3,
            id::INTERVAL_4,
            id::INTERVAL_5,
            id::INTERVAL_6,
            id::INTERVAL_7,
            id::INTERVAL_8,
            id::INTERVAL_9,
            id::REST,
            id::REPEAT,
        ];
        if note_input.contains(&command)
            && !self.ui.note_entry
            && self.document.editor.selection().is_none()
        {
            self.status = match self.shortcuts.label(id::TOGGLE_NOTE_INPUT) {
                Some(keys) => format!("Activa la entrada de notas con {keys} para escribir"),
                None => "Activa la entrada de notas para escribir".into(),
            };
            cx.notify();
            return;
        }
        match score_toolbar::marks::kind(command) {
            Some(MarkKind::Text(kind)) => return self.open_text_editor(kind, window, cx),
            Some(MarkKind::Lyric) => return self.open_lyric_editor(window, cx),
            Some(MarkKind::Chord) => return self.open_chord_editor(window, cx),
            _ => {}
        }
        if let Some(kind) = score_toolbar::marks::kind(command) {
            self.apply_mark(kind, cx);
            cx.notify();
            return;
        }
        if let Some(edit) = form_command(&self.document.editor, command) {
            self.edit(edit, cx);
            return;
        }
        use score_core::Step;
        let moves_selection = matches!(
            command,
            id::PREVIOUS
                | id::NEXT
                | id::EXTEND_PREVIOUS
                | id::EXTEND_NEXT
                | id::UPPER_NOTE_OR_STAFF
                | id::LOWER_NOTE_OR_STAFF
                | id::BAR_START
                | id::NEXT_BAR
                | id::NEXT_INSTRUMENT
                | id::PREVIOUS_INSTRUMENT
        );
        match command {
            id::NEW_COMMENT => self.open_comment_editor(None, window, cx),
            id::SHOW_COMMENTS => {
                self.show_review();
                self.comments.open = true;
            }
            id::RESOLVE_COMMENT => {
                if let Some(comment) = self
                    .comments
                    .active
                    .and_then(|id| self.document.editor.score().comment(id))
                {
                    self.set_comment_status(comment.id, !comment.resolved, cx);
                }
            }
            id::CHECK_SCORE => self.check_review(false, cx),
            id::CHECK_SELECTION => self.check_review(true, cx),
            id::REVIEW_ISSUES => self.show_review(),
            id::REVIEW_PREVIOUS => self.navigate_review(false, cx),
            id::REVIEW_NEXT => self.navigate_review(true, cx),
            id::SHOW_RANGES => self.review.show_ranges = !self.review.show_ranges,
            id::EDIT_RANGE => self.open_range_editor(cx),
            id::RESET_RANGE => self.edit(
                Command::SetInstrumentRange(self.document.editor.cursor().part, None),
                cx,
            ),
            id::REHEARSAL_MARK => self.edit(Command::ToggleRehearsalMark, cx),
            id::SYSTEM_BREAK => self.edit(
                Command::ToggleLayoutBreak(score_core::LayoutBreak::System),
                cx,
            ),
            id::PAGE_BREAK => self.edit(
                Command::ToggleLayoutBreak(score_core::LayoutBreak::Page),
                cx,
            ),
            id::REHEARSAL_LETTERS => self.edit(
                Command::SetRehearsalStyle(score_core::RehearsalStyle::Letters),
                cx,
            ),
            id::REHEARSAL_NUMBERS => self.edit(
                Command::SetRehearsalStyle(score_core::RehearsalStyle::Numbers),
                cx,
            ),
            id::REHEARSAL_BAR_NUMBERS => self.edit(
                Command::SetRehearsalStyle(score_core::RehearsalStyle::BarNumbers),
                cx,
            ),
            id::NEW => self.request_next(Next::New, window, cx),
            id::OPEN => self.request_next(Next::Open, window, cx),
            id::SAVE => self.save(false, window, cx),
            id::SAVE_AS => self.save(true, window, cx),
            id::EXPORT => self.export_pdf(cx),
            id::EXPORT_MUSICXML => self.export_musicxml(cx),
            id::CLOSE | id::QUIT => self.request_next(Next::Close, window, cx),
            id::PREFERENCES => self.open_preferences(cx),
            id::CLEAR_RECENT => {
                self.recent.clear();
                self.save_recent(cx);
                self.status = "Lista de recientes borrada".into();
            }
            id::CLOSE_PANEL => self.close_preferences(cx),
            id::CHECK_FOR_UPDATES => self.check_for_updates(true, cx),
            id::RESTART_TO_UPDATE => self.restart_to_update(cx),
            id::AUTOMATIC_UPDATES => self.toggle_automatic_updates(cx),
            id::ABOUT => self.open_about(cx),

            id::COPY => self.copy_music(false, cx),
            id::CUT => self.copy_music(true, cx),
            id::PASTE => self.paste_music(cx),
            id::SELECT_ALL => {
                self.clicked_bar_mark = None;
                if self.document.editor.select_all() {
                    self.status =
                        "Pentagrama seleccionado. Puedes copiar o cortar el pasaje".into();
                    self.reveal_selection = true;
                } else {
                    self.status = "Este pentagrama no tiene notas ni silencios escritos".into();
                }
            }
            id::EXTEND_PREVIOUS | id::EXTEND_NEXT => {
                self.clicked_bar_mark = None;
                self.document
                    .editor
                    .extend_navigation(command == id::EXTEND_NEXT);
                self.status = format!(
                    "{} elementos seleccionados",
                    self.document.editor.selected_ids().len()
                );
                self.reveal_selection = true;
            }
            id::UNDO => self.history(false, cx),
            id::REDO => self.history(true, cx),
            id::DELETE if self.selected_bar_mark().is_some() => {
                self.status = "Usa el comando de la marca de compás para quitarla".into();
            }
            id::DELETE => match self.selected_mark() {
                Some(mark) => self.edit(Command::RemoveMark(mark), cx),
                None => self.edit(Command::Erase, cx),
            },
            id::REMOVE_TIME => self.edit(Command::Delete, cx),
            id::RESET_POSITION => {
                if let Some(mark) = self.selected_bar_mark() {
                    self.edit(Command::ResetBarMarkOffset(mark), cx);
                    self.clicked_bar_mark = Some(mark);
                    self.reveal_selection = false;
                } else {
                    let mark = self.selected_mark();
                    self.edit(Command::ResetMarkOffsets(mark), cx);
                    if let Some(mark) = mark {
                        self.clicked_mark = Some(mark);
                    }
                }
            }
            id::ESCAPE => self.escape(cx),
            id::REPEAT => self.repeat_selection(cx),

            id::VOICE_1 => self.choose_voice(score_core::Voice::One, cx),
            id::VOICE_2 => self.choose_voice(score_core::Voice::Two, cx),
            id::DIVIDE_PART => self.edit(Command::SetPassageDivisi(true), cx),
            id::JOIN_PART => self.edit(Command::SetPassageDivisi(false), cx),
            id::TOGGLE_NOTE_INPUT => self.toggle_note_entry(cx),
            id::STEP_TIME => self.status = "La entrada paso a paso está activa".into(),
            id::NOTE_C => self.write_note(Step::C, cx),
            id::NOTE_D => self.write_note(Step::D, cx),
            id::NOTE_E => self.write_note(Step::E, cx),
            id::NOTE_F => self.write_note(Step::F, cx),
            id::NOTE_G => self.write_note(Step::G, cx),
            id::NOTE_A => self.write_note(Step::A, cx),
            id::NOTE_B => self.write_note(Step::B, cx),
            id::ADD_C => self.add_pitch(Step::C, cx),
            id::ADD_D => self.add_pitch(Step::D, cx),
            id::ADD_E => self.add_pitch(Step::E, cx),
            id::ADD_F => self.add_pitch(Step::F, cx),
            id::ADD_G => self.add_pitch(Step::G, cx),
            id::ADD_A => self.add_pitch(Step::A, cx),
            id::ADD_B => self.add_pitch(Step::B, cx),
            id::INTERVAL_1 => self.add_interval(1, cx),
            id::INTERVAL_2 => self.add_interval(2, cx),
            id::INTERVAL_3 => self.add_interval(3, cx),
            id::INTERVAL_4 => self.add_interval(4, cx),
            id::INTERVAL_5 => self.add_interval(5, cx),
            id::INTERVAL_6 => self.add_interval(6, cx),
            id::INTERVAL_7 => self.add_interval(7, cx),
            id::INTERVAL_8 => self.add_interval(8, cx),
            id::INTERVAL_9 => self.add_interval(9, cx),
            id::REST => self.edit(
                if self.ui.note_entry {
                    Command::Write(self.document.editor.rest())
                } else {
                    Command::Erase
                },
                cx,
            ),
            id::CHORD_MODE => self.toggle_chord(cx),
            id::REMOVE_HIGHEST => self.edit(Command::RemoveHighestPitch, cx),
            id::ENTRY_OCTAVE_UP => self.change_entry_octave(1, cx),
            id::ENTRY_OCTAVE_DOWN => self.change_entry_octave(-1, cx),

            id::DOT => self.toggle_rhythm(false, cx),
            id::TRIPLET => self.toggle_rhythm(true, cx),
            id::SIXTEENTH => self.choose_duration(score_core::Duration::Sixteenth, cx),
            id::THIRTY_SECOND => self.choose_duration(score_core::Duration::ThirtySecond, cx),
            id::EIGHTH => self.choose_duration(score_core::Duration::Eighth, cx),
            id::QUARTER => self.choose_duration(score_core::Duration::Quarter, cx),
            id::HALF => self.choose_duration(score_core::Duration::Half, cx),
            id::WHOLE => self.choose_duration(score_core::Duration::Whole, cx),

            id::NATURAL => self.toggle_accidental(Accidental::Natural, cx),
            id::SHARP => self.toggle_accidental(Accidental::Sharp, cx),
            id::FLAT => self.toggle_accidental(Accidental::Flat, cx),
            id::FOLLOW_KEY => self.apply_accidental(None, cx),

            id::REMOVE_TIE => self.edit(Command::RemoveConnections(ConnectionKind::Tie), cx),
            id::REMOVE_SLUR => self.edit(Command::RemoveConnections(ConnectionKind::Slur), cx),
            id::REMOVE_CRESCENDO => {
                self.edit(Command::RemoveConnections(ConnectionKind::Crescendo), cx)
            }
            id::REMOVE_DIMINUENDO => {
                self.edit(Command::RemoveConnections(ConnectionKind::Diminuendo), cx)
            }
            id::CLEAR_TECHNIQUE => self.edit(Command::SetTechnique(None), cx),
            id::CLEAR_ARTICULATION => self.edit(Command::SetArticulation(None), cx),
            id::CLEAR_DYNAMIC => self.edit(Command::SetDynamic(None), cx),

            id::TIME_SIGNATURE => self.toggle_time_picker(cx),
            id::CLEF => self.toggle_clef_picker(cx),
            id::ARPEGGIO_BOTH_HANDS => {
                let enabled = self
                    .document
                    .editor
                    .selection()
                    .and_then(|id| self.document.editor.score().arpeggio_at(id))
                    .is_some_and(|mark| mark.other_hand.is_some());
                self.edit(Command::SetArpeggioBothHands(!enabled), cx);
            }
            id::TEMPO_TEXT => self.toggle_tempo_picker(cx),
            id::COMMAND_SEARCH => self.open_command_search(window, cx),
            id::REPEATS => {
                self.playback.toggle_repeats();
                self.status = if self.playback.follow_repeats {
                    "La reproducción sigue repeticiones y saltos".into()
                } else {
                    "La reproducción ignora repeticiones y saltos".into()
                };
                cx.notify();
            }
            id::SCORE_INFO => self.toggle_header_editor(window, cx),
            id::KEY_SIGNATURE => self.toggle_key_picker(cx),
            id::BAR_AT_END => self.edit(Command::AppendMeasure, cx),
            id::SINGLE_BAR => self.edit(Command::InsertMeasure, cx),
            id::DELETE_BAR => self.edit(Command::DeleteMeasures, cx),

            id::ADD_INSTRUMENT => {
                self.open_instrument_picker(cx);
            }
            id::REMOVE_INSTRUMENT => self.edit(Command::RemovePart, cx),
            id::ADD_PIANO => self.edit(Command::AddPart(Instrument::Piano), cx),
            id::NEXT_INSTRUMENT => self.cycle_part(true, cx),
            id::PREVIOUS_INSTRUMENT => self.cycle_part(false, cx),
            id::RIGHT_HAND => {
                self.select_hand(self.document.editor.cursor().part, StaffId::Upper, cx)
            }
            id::LEFT_HAND => {
                self.select_hand(self.document.editor.cursor().part, StaffId::Lower, cx)
            }

            id::PREVIOUS | id::NEXT => {
                self.clicked_bar_mark = None;
                self.document.editor.navigate(command == id::NEXT);
                self.reveal_selection = true;
                self.audition_selection();
            }
            // A picked grace note moves alone, not the note it decorates.
            id::STEP_UP | id::STEP_DOWN | id::OCTAVE_UP | id::OCTAVE_DOWN
                if matches!(self.selected_mark(), Some(MarkRef::Grace(..))) =>
            {
                let Some(MarkRef::Grace(id, index)) = self.selected_mark() else {
                    unreachable!("checked grace mark");
                };
                let steps = match command {
                    id::STEP_UP => 1,
                    id::STEP_DOWN => -1,
                    id::OCTAVE_UP => 7,
                    _ => -7,
                };
                self.edit(Command::TransposeGraceNote(id, index, steps), cx);
                self.clicked_mark = Some(MarkRef::Grace(id, index));
            }
            id::GRACE_NOTE => {
                self.edit(Command::AddGraceNote, cx);
                if let Some(id) = self.document.editor.selection()
                    && let Some(grace) = self
                        .document
                        .editor
                        .score()
                        .event(id)
                        .and_then(|event| event.notation.grace)
                {
                    // The new grace note is picked, so the arrows tune it at once.
                    self.clicked_mark = Some(MarkRef::Grace(id, (grace.len() - 1) as u8));
                    self.status = format!(
                        "{} de {} notas · ↑/↓ cambian la altura de la nueva; Backspace la borra",
                        grace.kind.label(),
                        grace.len()
                    );
                }
            }
            // A mark picked in the score must not move its note by surprise.
            id::STEP_UP | id::STEP_DOWN | id::OCTAVE_UP | id::OCTAVE_DOWN
                if self.selected_mark().is_some() || self.selected_bar_mark().is_some() =>
            {
                self.status =
                    "Esta marca no se puede mover · Esc vuelve a su nota para transportarla".into();
            }
            id::STEP_UP => self.edit(Command::Transpose(1), cx),
            id::STEP_DOWN => self.edit(Command::Transpose(-1), cx),
            id::OCTAVE_UP => self.edit(Command::Transpose(7), cx),
            id::OCTAVE_DOWN => self.edit(Command::Transpose(-7), cx),
            id::UPPER_NOTE_OR_STAFF => self.move_vertically(true, cx),
            id::LOWER_NOTE_OR_STAFF => self.move_vertically(false, cx),

            id::PLAY => self.toggle_playback(cx),
            id::PLAY_FROM_SELECTION => {
                self.playback.stop();
                self.toggle_playback(cx);
            }
            id::PLAY_FROM_START => self.playback_from_start(cx),
            id::PLAYLINE_TO_START => self.playline_to_start(cx),
            id::LOOP => self.toggle_loop(cx),
            id::REPLAY => self.replay(cx),
            id::HALF_SPEED => self.toggle_half_speed(cx),
            id::CHORD_PLAYBACK => {
                let sound = !self.document.editor.score().chords_sound();
                self.edit(Command::SetChordsSound(sound), cx);
                self.status = if sound {
                    "Los cifrados suenan al reproducir"
                } else {
                    "Los cifrados solo se escriben"
                }
                .into();
            }
            id::BAR_START | id::NEXT_BAR => self.move_by_bar(command == id::NEXT_BAR, cx),
            id::GO_TO_BAR => self.open_go_to_bar(window, cx),
            id::STOP => self.stop_playback(cx),
            id::AUDITION => {
                self.playback.audition = !self.playback.audition;
                self.status = if self.playback.audition {
                    "Audición al escribir activada"
                } else {
                    "Audición al escribir desactivada"
                }
                .into();
            }
            id::TEMPO_UP => self.change_written_tempo(5, cx),
            id::TEMPO_DOWN => self.change_written_tempo(-5, cx),
            // Each part's mix lives in the left panel's Mezcla; again goes back to Lista.
            id::MIXER => {
                if self.ui.show_instruments && self.ui.parts_mode == panels::PartsMode::Mix {
                    self.ui.parts_mode = panels::PartsMode::List;
                } else {
                    self.reveal_selection |= !self.ui.show_instruments;
                    self.ui.show_instruments = true;
                    self.ui.parts_mode = panels::PartsMode::Mix;
                }
            }
            // Like MIXER: again goes back to Selección.
            id::SOUNDS => {
                if self.ui.show_inspector && self.ui.right == panels::RightTab::Sound {
                    self.ui.right = panels::RightTab::Selection;
                } else {
                    self.reveal_selection |= !self.ui.show_inspector;
                    self.ui.show_inspector = true;
                    self.ui.right = panels::RightTab::Sound;
                }
            }

            id::ZOOM_IN => self.set_zoom(self.ui.zoom + 0.1),
            id::ZOOM_OUT => self.set_zoom(self.ui.zoom - 0.1),
            id::ZOOM_ACTUAL => self.set_zoom(1.),
            id::PAGE_VIEW => self.set_view_mode(score_layout::ViewMode::Page, cx),
            id::CONTINUOUS_VIEW => self.set_view_mode(score_layout::ViewMode::Continuous, cx),
            id::SINGLE_SYSTEM_VIEW => self.set_view_mode(score_layout::ViewMode::SingleSystem, cx),
            id::ZOOM_FIT => {
                self.set_view_mode(score_layout::ViewMode::Page, cx);
                // Pages ignore the width and zoom only rescales them, so
                // this reuses the canvas's engraving instead of replacing it.
                let layout =
                    self.document
                        .layout_view(1000., score_layout::ViewMode::Page, self.ui.zoom);
                let page = &layout.pages[0];
                let viewport = self.viewport.scroll.bounds().size;
                self.ui.zoom = ((f32::from(viewport.width) - 128.) / page.width)
                    .min((f32::from(viewport.height) - 48.) / page.height)
                    .clamp(0.02, 2.);
                self.reveal_selection = true;
            }
            id::FULL_SCREEN => window.toggle_fullscreen(),
            id::TAB_NOTE_INPUT => self.show_tab(score_toolbar::TabId::Write),
            id::TAB_NOTATION => self.show_tab(score_toolbar::TabId::Articulations),
            id::TAB_DYNAMICS => self.show_tab(score_toolbar::TabId::Dynamics),
            id::TAB_TEXT => self.show_tab(score_toolbar::TabId::Text),
            // Breaks moved to Partitura; the old Disposición shortcut follows them.
            id::TAB_HOME | id::TAB_LAYOUT => self.show_tab(score_toolbar::TabId::Score),
            id::TAB_PLAYBACK => self.show_tab(score_toolbar::TabId::Playback),
            id::TAB_REVIEW => self.show_tab(score_toolbar::TabId::Review),
            id::TAB_VIEW => self.show_tab(score_toolbar::TabId::View),
            id::TOGGLE_RIBBON => {
                self.ui.ribbon_collapsed = !self.ui.ribbon_collapsed;
                self.ui.ribbon_peek = false;
            }
            id::TOGGLE_INSTRUMENTS_PANEL => {
                self.ui.show_instruments = !self.ui.show_instruments;
                self.reveal_selection = true;
            }
            id::TOGGLE_INSPECTOR => {
                self.ui.show_inspector = !self.ui.show_inspector;
                self.reveal_selection = true;
            }
            id::TOGGLE_AGENT_PANEL => {
                self.ui.show_agent = !self.ui.show_agent;
                self.reveal_selection = true;
                if self.ui.show_agent {
                    self.probe_agents(cx);
                    self.agent_field.focus_handle(cx).focus(window);
                } else if self.agent_field_focused(window, cx) {
                    window.focus(&self.focus);
                }
            }
            _ => self.status = format!("«{}» no está disponible en esta versión", spec.label),
        }
        if moves_selection {
            self.close_moved_editors();
        }
        cx.notify();
    }

    fn show_tab(&mut self, tab: score_toolbar::TabId) {
        self.ui.tab = tab;
        self.ui.open_group = None;
        if self.ui.ribbon_collapsed {
            self.ui.ribbon_peek = true;
        }
    }

    fn choose_voice(&mut self, voice: score_core::Voice, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.document.editor.select_voice(voice);
        let cursor = self.document.editor.cursor();
        let bar = &self
            .document
            .editor
            .score()
            .part(cursor.part)
            .unwrap()
            .staff(cursor.staff)
            .unwrap()
            .measures()[cursor.measure];
        let first = bar.voice_id_at(voice, 0);
        if let Some(event) = first {
            self.document.editor.select(ElementId {
                part: cursor.part,
                staff: cursor.staff,
                measure: cursor.measure,
                event,
            });
        }
        self.pending_slur = None;
        self.pending_hairpin = None;
        self.status = format!(
            "Voz {} · {}",
            voice.number(),
            if self.ui.note_entry {
                "entrada activa"
            } else {
                "escribe una letra para añadir notas"
            }
        );
        self.reveal_selection = true;
        cx.notify();
    }

    fn write_note(&mut self, step: score_core::Step, cx: &mut Context<Self>) {
        let note = self.entry_note(step);
        let command =
            match letter_action(&self.document.editor, self.ui.note_entry, self.chord_entry) {
                LetterAction::Insert => Command::Write(note),
                LetterAction::AddToChord => {
                    Command::AddPitch(note.pitches.first().expect("entry note"))
                }
                LetterAction::FillRest => {
                    Command::ReplacePitch(note.pitches.first().expect("entry note"))
                }
            };
        if self.try_edit(command, cx) {
            self.ui.note_entry = true;
            cx.notify();
        }
    }

    fn add_pitch(&mut self, step: score_core::Step, cx: &mut Context<Self>) {
        let pitch = self.entry_note(step).pitches.first().expect("entry note");
        self.edit(Command::AddPitch(pitch), cx);
    }

    /// Sibelius' Shift+1–9: a diatonic interval above the chord's top note,
    /// spelled by the key signature.
    fn add_interval(&mut self, interval: i16, cx: &mut Context<Self>) {
        let top = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .and_then(|event| event.pitches.last());
        let Some(top) = top else {
            self.status = "Selecciona una nota para añadirle un intervalo".into();
            return;
        };
        match top.shift(interval - 1) {
            Some(pitch) => self.edit(Command::AddPitch(pitch.with_accidental(None)), cx),
            None => self.status = "El intervalo queda fuera del registro".into(),
        }
    }

    /// Sibelius' R: repeats the selected note, chord or rest after it.
    fn repeat_selection(&mut self, cx: &mut Context<Self>) {
        let event = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .copied();
        match event {
            Some(event) => self.edit(Command::Write(event), cx),
            None => self.status = "Selecciona una nota, acorde o silencio para repetirlo".into(),
        }
    }

    fn escape(&mut self, cx: &mut Context<Self>) {
        self.selection_drag = None;
        if self.comment_editor.take().is_some() {
            cx.notify();
            return;
        }
        if self.range_editor.take().is_some() {
            cx.notify();
            return;
        }
        if self.ui.context_menu.take().is_some() {
            cx.notify();
            return;
        }
        if self.open_editor().is_some() && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        if self.cancel_editor() {
            cx.notify();
            return;
        }
        if self.clicked_bar_mark.take().is_some() {
            self.status = "Marca de compás deseleccionada".into();
            cx.notify();
        } else if self.clicked_mark.take().is_some() {
            self.status = "Nota seleccionada".into();
            cx.notify();
        } else if let Some(kind) = self
            .pending_hairpin
            .map(|(_, kind)| kind)
            .or(self.pending_slur.map(|_| ConnectionKind::Slur))
        {
            // A line that already follows the typed notes stays where it is.
            let (name, ending) = connection_words(kind);
            self.status = if self.following_line(kind).is_some() {
                format!("{name} terminad{ending}")
            } else if kind.is_hairpin() {
                "Regulador cancelado".into()
            } else {
                "Ligadura cancelada".into()
            };
            self.pending_hairpin = None;
            self.pending_slur = None;
            self.open_line = None;
        } else if self.playback.is_playing() || self.playback.is_paused() {
            self.stop_playback(cx);
        } else {
            match escape_step(self.ui.note_entry) {
                EscapeStep::LeaveEntry => {
                    self.ui.note_entry = false;
                    self.status =
                        "Entrada de notas desactivada · Esc otra vez quita la selección".into();
                    cx.notify();
                }
                EscapeStep::Deselect => self
                    .document
                    .editor
                    .set_cursor(self.document.editor.cursor()),
            }
        }
    }

    fn toggle_rhythm(&mut self, triplet: bool, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let current = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .map_or(self.document.editor.duration, |e| e.duration);
        let enabled = !(if triplet {
            current.is_triplet()
        } else {
            current.is_dotted()
        });
        if self.document.editor.selection().is_some() {
            self.edit(
                if triplet {
                    Command::SetTriplet(enabled)
                } else {
                    Command::SetDotted(enabled)
                },
                cx,
            );
            let now = self
                .document
                .editor
                .selection()
                .and_then(|id| self.document.editor.score().event(id))
                .unwrap()
                .duration;
            if (if triplet {
                now.is_triplet()
            } else {
                now.is_dotted()
            }) != enabled
            {
                return;
            }
        }
        let entry = self.document.editor.duration;
        self.document.editor.duration = if triplet {
            entry.with_triplet(enabled)
        } else {
            entry.with_dotted(enabled)
        };
        self.status = format!("Entrada: {}", self.document.editor.duration.label());
        cx.notify();
    }
    fn choose_duration(&mut self, duration: score_core::Duration, cx: &mut Context<Self>) {
        if !self.ui.note_entry && self.document.editor.selection().is_some() {
            self.edit(Command::SetDuration(duration.value()), cx);
            return;
        }
        self.document.editor.duration = self.document.editor.duration.with_value(duration.value());
        self.status = format!(
            "Duración de entrada: {}",
            self.document.editor.duration.label()
        );
    }

    fn change_entry_octave(&mut self, delta: i8, _: &mut Context<Self>) {
        self.document.editor.octave = (self.document.editor.octave as i8 + delta).clamp(0, 8) as u8;
        self.status = format!("Octava de entrada: {}", self.document.editor.octave);
    }

    /// Sibelius' 7/8/9 toggle: pressing the sign a note already has removes it.
    fn toggle_accidental(&mut self, sign: Accidental, cx: &mut Context<Self>) {
        let current = match self.document.editor.selected_pitch() {
            Some(pitch) => pitch.accidental(),
            None => self.entry_accidental,
        };
        self.apply_accidental((current != Some(sign)).then_some(sign), cx);
    }

    fn audition_selection(&mut self) {
        // The agent's moves are silent.
        if self.playback.is_playing() || self.agent_acting {
            return;
        }
        let now = Instant::now();
        if !audition_due(self.last_audition, now) {
            return;
        }
        if let Some(id) = self.document.editor.selection()
            && self
                .document
                .editor
                .score()
                .event(id)
                .is_some_and(|event| !event.pitches.is_empty())
        {
            let result = self.playback.preview(self.document.editor.score(), id);
            self.last_audition = Some(now);
            if let Err(error) = result {
                self.status = format!("No se pudo escuchar la nota: {error}");
            }
        }
    }

    /// ⌥↑/⌥↓: the next note of the chord, then the adjacent staff.
    fn move_vertically(&mut self, up: bool, cx: &mut Context<Self>) {
        self.clicked_bar_mark = None;
        let next_tone = {
            let editor = &self.document.editor;
            editor
                .selection()
                .and_then(|id| editor.score().event(id))
                .and_then(|event| {
                    let pitches = event.pitches;
                    let index = pitches
                        .iter()
                        .position(|pitch| Some(pitch) == editor.selected_pitch())?;
                    let next = if up { index + 1 } else { index.wrapping_sub(1) };
                    (next < pitches.len()).then_some(next)
                })
        };
        if let Some(next) = next_tone {
            self.document.editor.select_tone(next);
            self.audition_selection();
            return;
        }
        let editor = &mut self.document.editor;
        let cursor = editor.cursor();
        let staves: Vec<(PartId, StaffId)> = editor
            .score()
            .parts()
            .iter()
            .flat_map(|part| {
                part.display_staves(cursor.measure)
                    .map(move |staff| (part.id(), staff))
            })
            .collect();
        let at = editor.divisi_position();
        let Some(index) = staves.iter().position(|(part, staff)| {
            *part == cursor.part
                && *staff
                    == editor.score().part(*part).unwrap().displayed_staff(
                        cursor.staff,
                        cursor.voice,
                        cursor.measure,
                        at.tick,
                    )
        }) else {
            return;
        };
        let target = if up {
            index.checked_sub(1)
        } else {
            Some(index + 1).filter(|next| *next < staves.len())
        };
        if let Some((part, staff)) = target.map(|index| staves[index]) {
            editor.select_display_staff_in(part, staff);
            self.playback.follow_staff(part, editor.cursor().staff);
            self.reveal_selection = true;
            cx.notify();
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        self.ui.zoom = (zoom * 10.).round().clamp(1., 20.) / 10.;
        self.ghost = None;
        self.hover = None;
        self.reveal_selection = true;
    }

    fn zoom_at(&mut self, zoom: f32, pointer: Point<Pixels>) {
        let old = self.ui.zoom;
        let new = (zoom * 10.).round().clamp(1., 20.) / 10.;
        if new == old {
            return;
        }
        self.ui.zoom = new;
        self.reveal_selection = false;
        let within_canvas = pointer - self.canvas_origin;
        self.viewport.scroll.set_offset(selection::anchored_offset(
            self.viewport.scroll.offset(),
            within_canvas,
            new / old,
        ));
    }

    fn set_view_mode(&mut self, mode: score_layout::ViewMode, cx: &mut Context<Self>) {
        if self.ui.view_mode != mode {
            self.ui.view_mode = mode;
            self.ghost = None;
            self.pending_page = None;
            self.selection_drag = None;
            self.reveal_selection = true;
            self.status = match mode {
                score_layout::ViewMode::Page => "Vista de página · Sistemas repartidos en hojas",
                score_layout::ViewMode::Continuous => "Vista continua · Desplazamiento vertical",
                score_layout::ViewMode::SingleSystem => {
                    "Vista de pentagrama · Desplazamiento horizontal"
                }
            }
            .into();
            cx.notify();
        }
    }

    fn open_preferences(&mut self, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        self.preferences = Some(ui::Preferences::default());
        self.playback.stop();
        cx.notify();
    }

    fn open_instrument_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() || self.preferences.is_some() {
            return;
        }
        self.instrument_picker = Some(ui::InstrumentPicker::default());
        self.selection_drag = None;
        self.key_picker = false;
        self.time_picker = false;
        self.clef_picker = false;
        self.tempo_picker = None;
        self.header_editor = None;
        self.ui.file_menu = false;
        self.ui.open_group = None;
        self.playback.stop();
        cx.notify();
    }

    fn close_instrument_picker(&mut self, cx: &mut Context<Self>) {
        self.instrument_picker = None;
        cx.notify();
    }

    fn confirm_instruments(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let Some(mut picker) = self.instrument_picker.take() else {
            return;
        };
        let instruments = picker.instruments();
        if instruments.is_empty() {
            self.instrument_picker = Some(picker);
            return;
        }
        let before = self.document.editor.score().parts().len();
        self.edit(Command::AddParts(instruments), cx);
        if self.document.editor.score().parts().len() == before {
            picker.error = Some(self.status.clone());
            self.instrument_picker = Some(picker);
        } else {
            self.ui.show_instruments = true;
        }
        cx.notify();
    }

    fn close_preferences(&mut self, cx: &mut Context<Self>) {
        match &mut self.preferences {
            Some(preferences)
                if preferences.recording.is_some() || preferences.conflict.is_some() =>
            {
                preferences.recording = None;
                preferences.conflict = None;
            }
            _ => self.preferences = None,
        }
        cx.notify();
    }

    fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        if let Some(preferences) = &mut self.preferences {
            preferences.recording = None;
            preferences.message = Some("Grabación cancelada".into());
        }
        cx.notify();
    }

    /// A combination arrived while Preferences waits for one.
    fn record(&mut self, keys: KeySequence, cx: &mut Context<Self>) {
        let Some(preferences) = &mut self.preferences else {
            return;
        };
        let Some(recording) = preferences.recording.take() else {
            return;
        };
        let Some(spec) = self.shortcuts.spec(recording.command).copied() else {
            return;
        };
        if keys.is_altgr_for(score_keymap::Platform::current()) {
            preferences.message = Some(format!(
                "{} no sirve como atajo: Ctrl+Alt es AltGr en Windows, que escribe @ o #, y un \
                 atajo del escritorio en Linux; prueba con {}",
                keys.label(),
                keys.without_altgr().label()
            ));
        } else if keys.to_platform(&self.shortcuts.layout).is_none() {
            preferences.message = Some(format!(
                "{} no se puede detectar en este teclado",
                keys.label()
            ));
        } else if let Some(other) =
            self.shortcuts
                .keymap
                .conflict(&spec, &keys, &self.shortcuts.layout)
        {
            preferences.message = None;
            preferences.conflict = Some(ui::PreferencesConflict {
                command: recording.command,
                keys,
                other,
                add: recording.add,
            });
        } else {
            self.assign(recording.command, keys, recording.add, None, cx);
        }
        cx.notify();
    }

    /// Gives `keys` to `command`, taking them from `other` if it had them.
    fn assign(
        &mut self,
        command: CommandId,
        keys: KeySequence,
        add: bool,
        other: Option<CommandId>,
        cx: &mut Context<Self>,
    ) {
        if let Some(other) = other {
            let remaining = self.shortcuts.without_keys(other, &keys);
            self.shortcuts.user.set(other, &remaining);
        }
        let list = self.shortcuts.with_keys(command, keys.clone(), add);
        self.shortcuts.user.set(command, &list);
        self.save_shortcuts(format!("{} asignado", keys.label()), cx);
    }

    fn save_shortcuts(&mut self, done: String, cx: &mut Context<Self>) {
        let result = self.shortcuts.save();
        self.shortcuts.install(cx);
        if let Some(preferences) = &mut self.preferences {
            preferences.conflict = None;
            preferences.message = Some(match result {
                Ok(()) => done,
                Err(error) => format!("{done}, pero no se pudo guardar: {error}"),
            });
        }
        cx.notify();
    }

    fn request_next(&mut self, next: Next, window: &mut Window, cx: &mut Context<Self>) {
        self.note_drag = None;
        self.mark_drag = None;
        if self.busy || self.confirming || self.agent_locked() || self.instrument_picker.is_some() {
            return;
        }
        self.playback.stop();
        if !self.document.is_dirty() {
            self.perform_next(next, window, cx);
            return;
        }
        self.confirming = true;
        let answer = window.prompt(
            PromptLevel::Warning,
            "Hay cambios sin guardar",
            Some("Cancela para guardar tu partitura, o descarta los cambios para continuar."),
            &["Cancelar", "Descartar cambios"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let discard = answer.await == Ok(1);
            let _ = view.update_in(cx, |view, window, cx| {
                view.confirming = false;
                if discard {
                    view.perform_next(next, window, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn perform_next(&mut self, next: Next, window: &mut Window, cx: &mut Context<Self>) {
        let closing = matches!(next, Next::Close);
        match next {
            Next::New => {
                self.clear_recovery();
                self.clicked_mark = None;
                self.clicked_bar_mark = None;
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.end_agent_conversation(cx);
                self.document = Document::default();
                self.review.reset_document();
                self.comments = comments::CommentState::default();
                self.comment_editor = None;
                self.range_editor = None;
                self.playback.reset_part_mix(
                    self.document.editor.score(),
                    self.document.editor.cursor().part,
                );
                self.mixer_drag = None;
                self.viewport.scroll.set_offset(Point::default());
                self.instrument_picker = None;
                self.cancel_editor();
                self.chord_entry = false;
                self.entry_accidental = None;
                self.path = None;
                self.status = "Nueva partitura · clave de sol · 4/4".into();
            }
            Next::Open => self.open(None, window, cx),
            Next::OpenPath(path) => self.open(Some(path), window, cx),
            Next::Close => {
                self.clear_recovery();
                if let Some(recovery) = &self.quit_snapshot {
                    let _ = recovery.flush();
                }
                window.remove_window();
            }
        }
        if !closing {
            self.sync_quit_snapshot();
        }
        cx.notify();
    }

    /// Opens `known`, or asks for a file when it is `None`.
    fn open(&mut self, known: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        self.busy = true;
        let picker = known.is_none().then(|| {
            cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Abrir partitura".into()),
            })
        });
        cx.spawn_in(window, async move |view, cx| {
            let path = match (known, picker) {
                (Some(path), _) => Ok(Some(path)),
                (None, Some(picker)) => match picker.await {
                    Ok(Ok(paths)) => Ok(paths.and_then(|paths| paths.into_iter().next())),
                    Ok(Err(error)) => Err((None, file_dialogs::dialog_error(error))),
                    Err(error) => Err((None, file_dialogs::dialog_error(error))),
                },
                (None, None) => Ok(None),
            };
            let result = match path {
                Ok(Some(path)) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            match score_io::open(&path) {
                                Ok(score) => Ok((score, path)),
                                Err(error) => Err((Some(path), file_dialogs::file_error(&error))),
                            }
                        })
                        .await,
                ),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Some(Ok((opened, path))) => match if opened.imported { Document::recover(opened.score) } else { Document::new(opened.score) } {
                        Ok(document) => {
                            view.clear_recovery();
                            view.clicked_mark = None;
                            view.clicked_bar_mark = None;
                            view.pending_slur = None;
                            view.pending_hairpin = None;
                            view.end_agent_conversation(cx);
                            view.document = document;
                            view.review.reset_document();
                            view.comments = comments::CommentState::default();
                            view.comment_editor = None;
                            view.range_editor = None;
                            view.playback.reset_part_mix(
                                view.document.editor.score(),
                                view.document.editor.cursor().part,
                            );
                            view.mixer_drag = None;
                            view.viewport.scroll.set_offset(Point::default());
                            view.instrument_picker = None;
                            view.cancel_editor();
                            view.chord_entry = false;
                            view.entry_accidental = None;
                            view.remember_recent(&path, cx);
                            view.path = Some(path);
                            view.status = if opened.imported { "MusicXML importado · guarda una copia .levare para conservar los cambios" } else { "Partitura abierta" }.into();
                            view.sync_quit_snapshot();
                            if !opened.warnings.is_empty() {
                                view.confirming = true;
                                let details = opened.warnings.join("\n\n");
                                let answer = window.prompt(PromptLevel::Warning, "MusicXML importado con avisos", Some(&details), &["Aceptar"], cx);
                                cx.spawn_in(window, async move |view, cx| {
                                    let _ = answer.await;
                                    let _ = view.update(cx, |view, cx| {view.confirming=false;cx.notify();});
                                }).detach();
                            }
                        }
                        Err(error) => {
                            view.report_file_error("abrir", Some(&path), error.to_string(), window, cx)
                        }
                    },
                    Some(Err((path, error))) => {
                        view.report_file_error("abrir", path.as_deref(), error, window, cx)
                    }
                    None => {}
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// A failed open, save or export stays on the status line and is also
    /// explained in a window, which names the file.
    fn report_file_error(
        &mut self,
        action: &str,
        path: Option<&std::path::Path>,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let title = file_dialogs::failure_title(action, path);
        self.status = format!("{title}: {message}");
        // The export picker, reopened behind the window, keeps its own hold.
        let release = !self.confirming;
        self.confirming = true;
        let answer = window.prompt(
            PromptLevel::Critical,
            &title,
            Some(&file_dialogs::sentence(&message)),
            &["Aceptar"],
            cx,
        );
        cx.spawn_in(window, async move |view, cx| {
            let _ = answer.await;
            if release {
                let _ = view.update(cx, |view, cx| {
                    view.confirming = false;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Archivo › Abrir reciente. A file that disappeared leaves the list.
    fn open_recent(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if path.exists() {
            self.request_next(Next::OpenPath(path), window, cx);
        } else {
            self.recent.remove(&path);
            self.save_recent(cx);
            self.status = format!("Ya no existe: {}", path.display());
            cx.notify();
        }
    }

    fn remember_recent(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        self.recent.touch(path);
        self.save_recent(cx);
    }

    /// Persists the list and refreshes Archivo in the native menu bar.
    fn save_recent(&mut self, cx: &mut Context<Self>) {
        if let Some(path) = &self.recent_path
            && let Err(error) = self.recent.save(path)
        {
            self.status = format!("No se pudo guardar la lista de recientes: {error}");
        }
        self.shortcuts.recent = self.recent.files().to_vec();
        self.shortcuts.install(cx);
    }

    /// Writes `paneles.json` when a remembered panel field changed since the last write.
    fn remember_panels(&mut self) {
        let memory = self.ui.panel_memory();
        if memory == self.panels_saved {
            return;
        }
        if let Some(path) = &self.panels_path
            && let Err(error) = panels::save(path, &memory)
        {
            self.status = format!("No se pudo guardar el estado de los paneles: {error}");
        }
        self.panels_saved = memory;
    }

    fn export_pdf(&mut self, cx: &mut Context<Self>) {
        self.export_score(false, cx);
    }

    fn export_musicxml(&mut self, cx: &mut Context<Self>) {
        self.export_score(true, cx);
    }

    fn export_score(&mut self, musicxml: bool, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.playback.stop();
        self.selection_drag = None;
        self.key_picker = false;
        self.time_picker = false;
        self.clef_picker = false;
        self.tempo_picker = None;
        self.header_editor = None;
        self.ui.file_menu = false;
        self.ui.open_group = None;
        self.export_picker = Some(ui::ExportPicker {
            selection: score_export::Selection::all(self.document.editor.score(), musicxml),
            error: None,
        });
        self.confirming = true;
        cx.notify();
    }

    fn close_export_picker(&mut self, cx: &mut Context<Self>) {
        self.export_picker = None;
        self.confirming = false;
        cx.notify();
    }

    fn confirm_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Some(picker) = &mut self.export_picker else {
            return;
        };
        if let Err(error) = picker.selection.validate(self.document.editor.score()) {
            picker.error = Some(error);
            cx.notify();
            return;
        }
        let selection = picker.selection.clone();
        self.export_picker = None;
        self.confirming = false;
        self.busy = true;
        let directory = file_dialogs::start_folder(self.path.as_deref());
        let score = self.document.editor.score().clone();
        let suggested = score_export::folder_name(score.title());
        let picker = cx.prompt_for_new_path(&directory, Some(&suggested));
        cx.spawn_in(window, async move |view, cx| {
            let retry = selection.clone();
            let result = match picker.await {
                Ok(Ok(Some(path))) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            match score_export::export(&score, &selection, &path) {
                                Ok(files) => Ok((path, files.len())),
                                Err(error) => Err((Some(path), error)),
                            }
                        })
                        .await,
                ),
                Ok(Ok(None)) => None,
                Ok(Err(error)) => Some(Err((None, file_dialogs::dialog_error(error)))),
                Err(error) => Some(Err((None, file_dialogs::dialog_error(error)))),
            };
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Some(Ok((path, count))) => {
                        view.status = format!(
                            "Exportación completa · {count} archivos · {}",
                            path.display()
                        );
                    }
                    other => {
                        let failure = other.and_then(Result::err);
                        view.export_picker = Some(ui::ExportPicker {
                            selection: retry,
                            error: failure
                                .as_ref()
                                .map(|(_, error)| format!("No se pudo exportar: {error}")),
                        });
                        view.confirming = true;
                        if let Some((path, error)) = failure {
                            view.report_file_error("exportar", path.as_deref(), error, window, cx);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn save(&mut self, save_as: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.busy = true;
        let existing = if save_as {
            None
        } else {
            file_dialogs::save_in_place(self.path.as_deref()).map(PathBuf::from)
        };
        let picker = existing.is_none().then(|| {
            let (directory, name) = file_dialogs::suggested_save(
                self.path.as_deref(),
                self.document.editor.score().title(),
            );
            cx.prompt_for_new_path(&directory, Some(&name))
        });
        let score = self.document.editor.score().clone();
        cx.spawn_in(window, async move |view, cx| {
            let path = match (existing, picker) {
                (Some(path), _) => Ok(Some(path)),
                (_, Some(picker)) => picker
                    .await
                    .map_err(file_dialogs::dialog_error)
                    .and_then(|result| result.map_err(file_dialogs::dialog_error))
                    .map_err(|error| (None, error)),
                _ => Ok(None),
            };
            // The native panel approved replacing the name the musician typed,
            // not the one with the extension added, so that one asks again.
            let path = match path {
                Ok(Some(chosen)) => match file_dialogs::with_levare_extension(&chosen) {
                    Some(named) if named.exists() => {
                        let answer = view.update_in(cx, |view, window, cx| {
                            view.confirming = true;
                            let name = named.file_name().unwrap_or_default().to_string_lossy();
                            window.prompt(
                                PromptLevel::Warning,
                                &format!("«{name}» ya existe"),
                                Some("Si lo reemplazas, se perderá su contenido actual."),
                                &["Cancelar", "Reemplazar"],
                                cx,
                            )
                        });
                        let replace = match answer {
                            Ok(answer) => answer.await == Ok(1),
                            Err(_) => false,
                        };
                        let _ = view.update(cx, |view, _| view.confirming = false);
                        Ok(replace.then_some(named))
                    }
                    Some(named) => Ok(Some(named)),
                    None => Ok(Some(chosen)),
                },
                other => other,
            };
            let result = match path {
                Ok(Some(path)) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            match score_io::save(&path, &score) {
                                Ok(()) => Ok(path),
                                Err(error) => Err((Some(path), file_dialogs::file_error(&error))),
                            }
                        })
                        .await,
                ),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
            let _ = view.update_in(cx, |view, window, cx| {
                view.busy = false;
                match result {
                    Some(Ok(path)) => {
                        view.clear_recovery();
                        view.document.mark_saved();
                        view.status = format!("Guardado: {}", path.display());
                        view.remember_recent(&path, cx);
                        view.path = Some(path);
                        view.sync_quit_snapshot();
                    }
                    Some(Err((path, error))) => {
                        view.report_file_error("guardar", path.as_deref(), error, window, cx)
                    }
                    None => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn history(&mut self, redo: bool, cx: &mut Context<Self>) {
        self.selection_drag = None;
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let changed = if redo {
            self.document.editor.redo()
        } else {
            self.document.editor.undo()
        };
        if changed {
            self.clicked_bar_mark = None;
            self.pending_slur = None;
            self.pending_hairpin = None;
            self.playback.stop();
            self.reveal_selection = true;
            self.status = if redo {
                "Cambio rehecho"
            } else {
                "Cambio deshecho"
            }
            .into();
        }
        self.sync_quit_snapshot();
        cx.notify();
    }

    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.ghost = None;
        let was_paused = self.playback.is_paused();
        let selection = self.document.editor.selection();
        self.status = match self.playback.toggle(
            self.document.editor.score(),
            selection,
            self.document.editor.cursor().part,
            self.document.editor.cursor().staff,
        ) {
            Ok(()) if self.playback.is_paused() => {
                "En pausa · Espacio para continuar · Esc para detener".into()
            }
            Ok(()) if self.playback.is_playing() && was_paused => {
                "Reproducción reanudada · Espacio para pausar".into()
            }
            Ok(()) if self.playback.is_playing() && self.playback.has_ruler_position() => {
                "Reproduciendo desde la regla · Espacio para pausar".into()
            }
            Ok(()) if self.playback.is_playing() && selection.is_some() => {
                "Reproduciendo desde la selección · Espacio para pausar".into()
            }
            Ok(()) if self.playback.is_playing() => {
                "Reproduciendo desde el inicio · Espacio para pausar".into()
            }
            Ok(()) => "Reproducción detenida".into(),
            Err(error) => error,
        };
        cx.notify();
    }

    fn select_part(&mut self, part: PartId, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.document.editor.select_part(part);
        self.clicked_bar_mark = None;
        self.playback
            .follow_staff(part, self.document.editor.cursor().staff);
        self.reveal_selection = true;
        cx.notify();
    }

    fn signature_measure(&self) -> usize {
        if self.signature_from_start {
            0
        } else {
            self.tempo_picker
                .as_ref()
                .map_or(self.document.editor.cursor().measure, |editor| {
                    editor.measure
                })
        }
    }
    /// The signature editors write from the first bar instead of the cursor's.
    fn sign_from_start(&mut self, cx: &mut Context<Self>) {
        self.signature_from_start = true;
        self.sync_signature_controls(cx);
    }
    fn sync_signature_controls(&mut self, cx: &mut Context<Self>) {
        let bar = self.signature_measure();
        if let Some(editor) = &self.tempo_picker {
            self.tempo_picker = Some(ui::TempoEditor::new(
                self.document.editor.score().tempo_at(bar),
                editor.measure,
                cx,
            ));
        }
        self.pending_time = self.document.editor.score().time_at(bar);
        self.key_mode = self.document.editor.score().key_signature_at(bar).mode();
    }
    fn open_command_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let search = ui::CommandSearch::new(cx);
        search.field.focus_handle(cx).focus(window);
        self.command_search = Some(search);
        self.ui.file_menu = false;
        self.ui.open_group = None;
        self.ui.context_menu = None;
        cx.notify();
    }

    /// Ranked results for the typed query; the last commands run from the
    /// search while it is empty.
    fn command_results(&self, cx: &App) -> Vec<score_toolbar::SearchResult> {
        let Some(search) = &self.command_search else {
            return Vec::new();
        };
        let query = search.field.read(cx).value();
        if query.trim().is_empty() {
            return self
                .recent_searches
                .iter()
                .map(|command| score_toolbar::SearchResult {
                    item: score_toolbar::item_view(
                        *command,
                        &self.shortcuts.registry,
                        &self.shortcuts.keymap,
                        self,
                    ),
                    place: score_toolbar::ribbon_place(*command).unwrap_or_else(|| {
                        self.shortcuts
                            .spec(*command)
                            .map_or_else(String::new, |spec| spec.category.label().to_owned())
                    }),
                })
                .collect();
        }
        score_toolbar::search(
            query,
            &self.shortcuts.registry,
            &self.shortcuts.keymap,
            self,
            10,
        )
    }

    fn run_search_result(
        &mut self,
        command: CommandId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.command_search = None;
        window.focus(&self.focus);
        self.recent_searches.retain(|recent| *recent != command);
        self.recent_searches.insert(0, command);
        self.recent_searches.truncate(5);
        cx.notify();
        self.run(command, window, cx);
    }

    /// The editor open in the Selection tab. Opening one closes the others,
    /// so at most one is set.
    fn open_editor(&self) -> Option<ui::EditorKind> {
        use ui::EditorKind as Kind;
        [
            (self.header_editor.is_some(), Kind::ScoreInfo),
            (self.tempo_picker.is_some(), Kind::Tempo),
            (self.clef_picker, Kind::Clef),
            (self.key_picker, Kind::Key),
            (self.time_picker, Kind::Time),
            (self.text_editor.is_some(), Kind::Text),
            (self.lyric_editor.is_some(), Kind::Lyric),
            (self.chord_editor.is_some(), Kind::Chord),
        ]
        .into_iter()
        .find_map(|(open, kind)| open.then_some(kind))
    }
    /// Editors live in the right panel's Selection tab: opening one, from
    /// anywhere, shows it there. The words editors are closed here, the
    /// signature, tempo and header editors by the function that opens one.
    fn show_editor_tab(&mut self) {
        self.text_editor = None;
        self.lyric_editor = None;
        self.chord_editor = None;
        self.reveal_editor_tab();
    }
    fn reveal_editor_tab(&mut self) {
        self.ui.show_inspector = true;
        self.ui.right = panels::RightTab::Selection;
    }
    /// An open editor the user cannot see: asking for it again shows it
    /// instead of closing it.
    fn editor_hidden(&self) -> bool {
        !self.ui.show_inspector || self.ui.right != panels::RightTab::Selection
    }
    /// Closes the open editor without writing, as Esc does. Returns whether
    /// one was open.
    fn cancel_editor(&mut self) -> bool {
        if self.text_editor.take().is_some()
            | self.lyric_editor.take().is_some()
            | self.chord_editor.take().is_some()
        {
            return true;
        }
        if self.clef_picker || self.key_picker || self.time_picker {
            self.clef_picker = false;
            self.key_picker = false;
            self.time_picker = false;
            return true;
        }
        if self.tempo_picker.take().is_some() {
            self.status = "Edición de tempo cancelada".into();
            return true;
        }
        if self.header_editor.take().is_some() {
            self.status = "Edición de la cabecera cancelada".into();
            return true;
        }
        false
    }
    /// A click or a navigation key moved the selection: the editors of what
    /// was selected close as Esc closes them, after the move's own status.
    /// Returns whether one closed.
    fn close_moved_editors(&mut self) -> bool {
        self.open_editor()
            .is_some_and(|kind| !kind.survives_selection_move())
            && self.cancel_editor()
    }
    /// The editors' Cancelar button.
    fn close_editors(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.cancel_editor() {
            cx.notify();
        }
    }
    /// Opens the header editor, if needed, on one of its fields.
    fn edit_header_field(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.header_editor.is_none() {
            self.toggle_header_editor(window, cx);
        }
        if let Some(editor) = &self.header_editor {
            editor.focus_field(index, window, cx);
        }
    }
    fn toggle_tempo_picker(&mut self, cx: &mut Context<Self>) {
        self.toggle_tempo_at(self.document.editor.cursor().measure, cx);
    }
    fn toggle_tempo_at(&mut self, measure: usize, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.tempo_picker.is_some() && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        if self.tempo_picker.is_some() {
            self.tempo_picker = None;
            self.header_editor = None;
        } else {
            self.signature_from_start = false;
            self.tempo_picker = Some(ui::TempoEditor::new(
                self.document.editor.score().tempo_at(measure),
                measure,
                cx,
            ));
        }
        self.header_editor = None;
        self.key_picker = false;
        self.time_picker = false;
        self.clef_picker = false;
        self.instrument_picker = None;
        if self.tempo_picker.is_some() {
            self.show_editor_tab();
        }
        cx.notify();
    }
    fn toggle_header_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.header_editor.is_some() && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        if self.header_editor.take().is_none() {
            let editor = ui::HeaderEditor::new(self.document.editor.score(), cx);
            editor.focus_first(window, cx);
            self.header_editor = Some(editor);
            self.tempo_picker = None;
            self.key_picker = false;
            self.time_picker = false;
            self.clef_picker = false;
            self.instrument_picker = None;
            self.show_editor_tab();
        }
        cx.notify();
    }
    fn apply_header(&mut self, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(editor) = &self.header_editor else {
            return;
        };
        let (title, credits) = editor.header(cx);
        // Checked on a copy first so an invalid field keeps the editor open.
        match self
            .document
            .editor
            .score()
            .clone()
            .set_header(&title, &credits)
        {
            Err(error) => self.status = error.to_string(),
            Ok(changed) => {
                self.header_editor = None;
                if changed {
                    self.edit(Command::SetHeader(title, credits), cx);
                }
            }
        }
        cx.notify();
    }
    fn apply_tempo(&mut self, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(editor) = &self.tempo_picker else {
            return;
        };
        let mark = match editor.mark(cx) {
            Ok(mark) => mark,
            Err(error) => {
                self.status = error.to_string();
                cx.notify();
                return;
            }
        };
        let bar = self.signature_measure();
        self.edit(Command::SetTempoAt(bar, mark.clone()), cx);
        if self.document.editor.score().tempo_at(bar) == &mark {
            self.tempo_picker = None;
            self.header_editor = None;
            self.status = format!(
                "{} desde el compás {} · hasta el siguiente cambio",
                mark.label(),
                bar + 1
            );
        }
        cx.notify();
    }
    fn change_written_tempo(&mut self, delta: i16, cx: &mut Context<Self>) {
        let bar = self
            .playback
            .position(self.document.editor.score(), self.document.editor.cursor())
            .measure;
        let mut mark = self.document.editor.score().tempo_at(bar).clone();
        mark.bpm = (mark.bpm as i16 + delta).clamp(20, 400) as u16;
        self.edit(Command::SetTempoAt(bar, mark), cx);
    }
    fn toggle_clef_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.clef_picker && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        self.clef_picker = !self.clef_picker;
        self.time_picker = false;
        self.key_picker = false;
        self.tempo_picker = None;
        self.header_editor = None;
        self.instrument_picker = None;
        self.signature_from_start = false;
        if self.clef_picker {
            self.show_editor_tab();
        }
        cx.notify();
    }
    fn choose_clef(&mut self, clef: Option<score_core::Clef>, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let cursor = self.document.editor.cursor();
        let bar = self.signature_measure();
        let command = match clef {
            Some(clef) => Command::SetClefAt(cursor.part, cursor.staff, bar, clef),
            None => Command::RemoveClefChange(cursor.part, cursor.staff, bar),
        };
        self.edit(command, cx);
        let part = self.document.editor.score().part(cursor.part).unwrap();
        if clef.is_none() || clef == Some(part.clef_at(cursor.staff, bar)) {
            self.clef_picker = false;
            self.status = format!(
                "{} desde el compás {}",
                part.clef_at(cursor.staff, bar).label(),
                bar + 1
            );
        }
        cx.notify();
    }
    fn toggle_key_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.key_picker && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        self.key_picker = !self.key_picker;
        self.time_picker = false;
        self.clef_picker = false;
        self.tempo_picker = None;
        self.header_editor = None;
        self.signature_from_start = false;
        self.sync_signature_controls(cx);
        self.instrument_picker = None;
        if self.key_picker {
            self.show_editor_tab();
        }
        cx.notify();
    }
    fn toggle_time_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.time_picker && self.editor_hidden() {
            self.reveal_editor_tab();
            cx.notify();
            return;
        }
        self.time_picker = !self.time_picker;
        self.clef_picker = false;
        self.tempo_picker = None;
        self.header_editor = None;
        self.key_picker = false;
        self.signature_from_start = false;
        self.sync_signature_controls(cx);
        self.instrument_picker = None;
        if self.time_picker {
            self.show_editor_tab();
        }
        cx.notify();
    }
    fn choose_key(&mut self, key: KeySignature, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if !self.key_picker {
            self.signature_from_start = false;
        }
        let bar = self.signature_measure();
        self.edit(Command::SetKeySignatureAt(bar, key), cx);
        if self.document.editor.score().key_signature_at(bar) == key {
            self.key_picker = false;
            self.status = format!(
                "{} desde el compás {} · hasta el siguiente cambio",
                key.label(),
                bar + 1
            );
        }
        cx.notify();
    }
    fn choose_time(&mut self, time: score_core::TimeSignature, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        let bar = self.signature_measure();
        self.edit(Command::SetTimeSignatureAt(bar, time), cx);
        if self.document.editor.score().time_at(bar) == time {
            self.time_picker = false;
            self.clef_picker = false;
            self.tempo_picker = None;
            self.header_editor = None;
            self.status = format!(
                "Métrica {}/{} desde el compás {} · hasta el siguiente cambio",
                time.numerator,
                time.denominator,
                bar + 1
            );
        }
        cx.notify();
    }
    fn entry_note(&self, step: score_core::Step) -> score_core::Event {
        let mut note = self.document.editor.note(step);
        note.pitches = score_core::Pitches::single(
            note.pitches
                .first()
                .map(|pitch| pitch.with_accidental(self.entry_accidental)),
        );
        note
    }
    fn apply_accidental(&mut self, accidental: Option<Accidental>, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.document.editor.selection().is_some() {
            self.edit(Command::SetAccidental(accidental), cx);
        } else {
            self.entry_accidental = accidental;
            self.status = "Alteración de entrada ajustada".into();
            cx.notify();
        }
    }
    fn toggle_chord(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.chord_entry = !self.chord_entry;
        self.status = if self.chord_entry {
            "Modo acorde: añade notas con A–G; desactívalo para avanzar"
        } else {
            "Entrada de notas consecutivas"
        }
        .into();
        cx.notify();
    }

    fn select_hand(&mut self, part: PartId, staff: StaffId, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.document.editor.score().part(part).is_none_or(|part| {
            part.display_staves(self.document.editor.cursor().measure)
                .count()
                != 2
        }) {
            self.status = "Selecciona una parte con dos pentagramas".into();
            cx.notify();
            return;
        }
        if self.document.editor.cursor().part != part {
            self.document.editor.select_part(part);
        }
        self.document.editor.select_display_staff(staff);
        self.clicked_bar_mark = None;
        self.playback
            .follow_staff(part, self.document.editor.cursor().staff);
        self.reveal_selection = true;
        cx.notify();
    }

    fn cycle_part(&mut self, forward: bool, cx: &mut Context<Self>) {
        let parts = self.document.editor.score().parts();
        let index = parts
            .iter()
            .position(|part| part.id() == self.document.editor.cursor().part)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % parts.len()
        } else {
            (index + parts.len() - 1) % parts.len()
        };
        self.select_part(parts[next].id(), cx);
    }

    fn toggle_note_entry(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        self.ui.note_entry = !self.ui.note_entry;
        if !self.ui.note_entry {
            self.ghost = None;
        }
        self.status = if self.ui.note_entry {
            "Entrada de notas activa"
        } else {
            "Entrada de notas desactivada · N para activar"
        }
        .into();
        cx.notify();
    }
    fn playback_from_start(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.ghost = None;
        self.playback.stop();
        self.status = match self.playback.toggle(
            self.document.editor.score(),
            None,
            self.document.editor.cursor().part,
            self.document.editor.cursor().staff,
        ) {
            Ok(()) => "Reproduciendo desde el inicio".into(),
            Err(error) => error,
        };
        cx.notify();
    }
    fn seek_playback(&mut self, tick: u64, reveal: bool, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.ghost = None;
        self.selection_drag = None;
        self.status = match self.playback.seek(self.document.editor.score(), tick) {
            Ok(()) => {
                let position = self
                    .playback
                    .position(self.document.editor.score(), self.document.editor.cursor());
                self.viewport.follow(
                    &self.layout,
                    None,
                    playhead::tracking_bounds(
                        &self.layout,
                        self.document.editor.cursor(),
                        position,
                    ),
                    reveal,
                );
                format!(
                    "Compás {} · Tiempo {} · Espacio para {}",
                    position.measure + 1,
                    position.beat.round() as u16,
                    if self.playback.is_playing() {
                        "pausar"
                    } else {
                        "reproducir"
                    },
                )
            }
            Err(error) => error,
        };
        cx.notify();
    }
    fn stop_playback(&mut self, cx: &mut Context<Self>) {
        self.playback.stop();
        self.status = "Reproducción detenida".into();
        cx.notify();
    }
    fn choose_sound(&mut self, bank: Option<usize>, cx: &mut Context<Self>) {
        if self.busy
            || self.confirming
            || self.instrument_picker.is_some()
            || self.preferences.is_some()
        {
            return;
        }
        let score = self.document.editor.score();
        self.status =
            match self
                .playback
                .choose_sound(bank, score, self.document.editor.cursor().part)
            {
                Ok(()) => format!("Sonido: {}", self.playback.sound_name()),
                Err(error) => {
                    self.playback.sound_error = Some(error.clone());
                    error
                }
            };
        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.remember_panels();
        self.review.follow(&self.document.editor);
        self.comments.refresh(self.document.editor.score());
        if self
            .range_editor
            .as_ref()
            .is_some_and(|editor| self.document.editor.score().part(editor.part).is_none())
        {
            self.range_editor = None;
        }
        if !window.is_window_active() {
            self.note_drag = None;
            self.mark_drag = None;
            self.selection_drag = None;
            self.ghost = None;
            self.hover = None;
        }
        if self
            .mark_drag
            .as_ref()
            .is_some_and(|drag| drag.zoom_changed(self.ui.zoom))
        {
            self.mark_drag = None;
        }
        self.ui.compact = f32::from(window.viewport_size().width) < 1250.;
        self.ui.width = f32::from(window.viewport_size().width);
        let panels = if self.ui.show_instruments {
            ui::LEFT_WIDTH
        } else {
            0.
        } + if self.ui.show_inspector {
            ui::RIGHT_WIDTH
        } else {
            0.
        } + if self.ui.show_agent {
            ui::AGENT_WIDTH
        } else {
            0.
        };
        let width =
            ((f32::from(window.viewport_size().width) - panels - 128.) / self.ui.zoom).max(500.);
        let layout = if let Some(preview) = self
            .mark_drag
            .as_ref()
            .and_then(|drag| drag.preview.as_ref())
        {
            Rc::clone(preview)
        } else if let Some(drag) = &mut self.connection_drag
            && let Some(layout) = drag.shown(self.ui.zoom)
        {
            layout
        } else if let Some(drag) = &mut self.note_drag
            && let Some(score) = &drag.preview
        {
            let mut layout = drag.layout.layout_pitch_preview(score, &drag.geometry);
            layout.scale = self.ui.zoom;
            Rc::new(layout)
        } else {
            self.document
                .layout_view(width, self.ui.view_mode, self.ui.zoom)
        };
        if !Rc::ptr_eq(&self.layout, &layout) {
            self.ghost = None;
            self.hover = None;
        }
        self.layout = layout;
        self.position = self.playback.frame_position(&self.document.editor);
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.document.editor.score().title().to_owned());
        self.ui.frame.sync(
            window,
            format!(
                "{}{} · Levare",
                title,
                if self.document.is_dirty() { " •" } else { "" }
            ),
        );
        let root = div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(ui::BG))
            .text_color(rgb(ui::TEXT))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .key_context(
                if self.preferences.is_some()
                    || self.instrument_picker.is_some()
                    || self.export_picker.is_some()
                    || self.about
                {
                    "Levare Modal"
                } else {
                    "Levare"
                },
            )
            .on_key_up(cx.listener(Self::key_up))
            .on_action(
                cx.listener(|this, action: &Run, window, cx| this.run_key(action.0, window, cx)),
            )
            .on_action(
                cx.listener(|this, action: &MenuRun, window, cx| this.run(action.0, window, cx)),
            )
            .on_action(cx.listener(|this, action: &ChooseDynamic, _, cx| {
                this.edit(Command::SetDynamic(action.0), cx)
            }))
            .on_action(cx.listener(|this, action: &OpenRecent, window, cx| {
                this.open_recent(action.0.clone(), window, cx)
            }))
            .on_action(cx.listener(|this, action: &AddInstrument, _, cx| {
                this.edit(Command::AddPart(action.0), cx)
            }))
            .on_action(
                cx.listener(|this, action: &ChooseKeySignature, _, cx| {
                    this.choose_key(action.0, cx)
                }),
            )
            .child(ui::workspace(self, title, cx));
        ui::window_frame::frame(&self.ui.frame, root)
    }
}

fn recovery_path() -> Option<PathBuf> {
    data_path("recovery.partitura")
}

fn main() {
    diagnostics::install();
    let app = Application::new();
    let to_open = system_open::listen(&app);
    app.run(|cx| {
        app_icon::install();
        fonts::register(cx);
        // App-level ownership survives window/view teardown during a native
        // Dock quit. GPUI's Context::on_app_quit would only hold a weak view.
        let recovery = recovery_path()
            .ok_or_else(|| "no se encontró la carpeta de datos de Levare".to_owned())
            .and_then(|path| score_io::Recovery::open(&path).map_err(|e| e.to_string()));
        if let Err(error) = &recovery {
            eprintln!("Recuperación no disponible: {error}");
        }
        let pending = recovery.as_ref().ok().cloned();
        let shortcuts = Shortcuts::load(cx);
        shortcuts.install(cx);
        let (bounds, min_size) = ui::window_frame::first_window(cx);
        let opened = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(min_size),
                titlebar: Some(TitlebarOptions {
                    title: Some("Levare".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(16.))),
                }),
                // Linux: Levare's title bar replaces the compositor's, when it
                // agrees (`ui::window_frame`). macOS and Windows ignore it.
                window_decorations: Some(WindowDecorations::Client),
                // The macOS bundle id: Linux desktops match the window to
                // Levare's desktop entry and icon by it.
                app_id: Some("dev.partitura.editor".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| AppView::new(window, cx, recovery, shortcuts)),
        );
        let window = opened.unwrap_or_else(|error| diagnostics::window_failed(error));
        cx.on_app_quit(move |cx| {
            // The window is gone after a close, which already asked about
            // unsaved changes; the writer still finishes what it holds.
            if window
                .update(cx, |view, _, _| view.keep_unsaved_on_quit())
                .is_err()
                && let Some(recovery) = &pending
                && let Err(error) = recovery.flush()
            {
                eprintln!("No se pudo guardar la recuperación: {error}");
            }
            async {}
        })
        .detach();
        diagnostics::announce_unseen_crash(window.into(), cx);
        system_open::deliver(to_open, cx);
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}

/// What a letter A–G does: it writes after a note or fills a selected rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LetterAction {
    Insert,
    AddToChord,
    FillRest,
}

fn letter_action(editor: &score_edit::Editor, note_entry: bool, chord_entry: bool) -> LetterAction {
    let selected = editor.selection().and_then(|id| editor.score().event(id));
    match selected {
        Some(event) if event.pitches.is_empty() && !note_entry => LetterAction::FillRest,
        Some(event) if !event.pitches.is_empty() && chord_entry => LetterAction::AddToChord,
        _ => LetterAction::Insert,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EscapeStep {
    LeaveEntry,
    Deselect,
}

fn escape_step(note_entry: bool) -> EscapeStep {
    if note_entry {
        EscapeStep::LeaveEntry
    } else {
        EscapeStep::Deselect
    }
}

/// Status-bar name of a line and its gender ending: «ligadura terminada»,
/// «crescendo terminado».
fn connection_words(kind: ConnectionKind) -> (&'static str, &'static str) {
    match kind {
        ConnectionKind::Slur => ("Ligadura", "a"),
        ConnectionKind::Tie => ("Unión", "a"),
        ConnectionKind::Crescendo => ("Crescendo", "o"),
        ConnectionKind::Diminuendo => ("Diminuendo", "o"),
        ConnectionKind::OttavaAlta | ConnectionKind::OttavaBassa | ConnectionKind::Pedal => {
            (kind.label(), "o")
        }
        ConnectionKind::Trill => ("Trino", "o"),
        ConnectionKind::Glissando => ("Glissando", "o"),
    }
}

/// The same bar bounds used by score-edit for every form transaction.
fn form_bounds(editor: &score_edit::Editor) -> (usize, usize) {
    let ids = editor.selected_ids();
    let start = ids
        .iter()
        .map(|id| id.measure)
        .min()
        .unwrap_or(editor.cursor().measure);
    let end = ids.iter().map(|id| id.measure).max().unwrap_or(start);
    (start, end)
}

/// Resolve ribbon/menu commands before entering the normal editor transaction.
fn form_command(editor: &score_edit::Editor, command: CommandId) -> Option<Command> {
    let edit = match command {
        id::REPEAT_START => FormEdit::ToggleRepeatStart,
        id::REPEAT_END => FormEdit::ToggleRepeatEnd,
        id::ENDING_1 => return Some(Command::SetEnding(vec![1])),
        id::ENDING_2 => return Some(Command::SetEnding(vec![2])),
        id::REMOVE_ENDING => return Some(Command::RemoveEndings),
        id::DOUBLE_BARLINE | id::FINAL_BARLINE => {
            let style = if command == id::DOUBLE_BARLINE {
                Barline::Double
            } else {
                Barline::Final
            };
            let (_, end) = form_bounds(editor);
            FormEdit::Barline(if editor.score().bar_form(end).barline == style {
                Barline::Single
            } else {
                style
            })
        }
        id::SEGNO => FormEdit::ToggleSegno,
        id::CODA => FormEdit::ToggleCoda,
        id::TO_CODA => FormEdit::ToggleToCoda,
        id::FINE => FormEdit::ToggleFine,
        id::DA_CAPO => FormEdit::Jump(Some(Jump::DaCapo)),
        id::DA_CAPO_AL_FINE => FormEdit::Jump(Some(Jump::DaCapoAlFine)),
        id::DA_CAPO_AL_CODA => FormEdit::Jump(Some(Jump::DaCapoAlCoda)),
        id::DAL_SEGNO => FormEdit::Jump(Some(Jump::DalSegno)),
        id::DAL_SEGNO_AL_FINE => FormEdit::Jump(Some(Jump::DalSegnoAlFine)),
        id::DAL_SEGNO_AL_CODA => FormEdit::Jump(Some(Jump::DalSegnoAlCoda)),
        id::CLEAR_FORM => FormEdit::Clear,
        _ => return None,
    };
    Some(Command::EditForm(edit))
}

#[cfg(test)]
mod entry_mode_tests {
    use super::{EscapeStep, LetterAction, escape_step, letter_action};
    use score_core::{Pitch, Step};
    use score_edit::{Command, Editor};

    #[test]
    fn letter_fills_a_selected_rest_when_entry_is_off() {
        let mut editor = Editor::default();
        editor.execute(Command::Insert(editor.rest())).unwrap();
        assert_eq!(letter_action(&editor, false, false), LetterAction::FillRest);
    }

    #[test]
    fn letter_after_a_selected_note_inserts_when_entry_is_off() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        assert_eq!(letter_action(&editor, false, false), LetterAction::Insert);
    }

    #[test]
    fn chord_mode_adds_to_a_selected_note() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        for note_entry in [false, true] {
            assert_eq!(
                letter_action(&editor, note_entry, true),
                LetterAction::AddToChord
            );
        }
    }

    #[test]
    fn letter_with_nothing_selected_inserts() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        editor.set_cursor(editor.cursor());
        assert_eq!(letter_action(&editor, false, false), LetterAction::Insert);
    }

    #[test]
    fn entry_on_keeps_inserting_after_a_rest() {
        let mut editor = Editor::default();
        editor.execute(Command::Insert(editor.rest())).unwrap();
        assert_eq!(letter_action(&editor, true, false), LetterAction::Insert);
    }

    #[test]
    fn escape_leaves_entry_before_deselecting() {
        assert_eq!(escape_step(true), EscapeStep::LeaveEntry);
        assert_eq!(escape_step(false), EscapeStep::Deselect);
    }

    #[test]
    fn letter_after_a_selected_note_keeps_the_existing_note() {
        let mut editor = Editor::default();
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        let first = editor.selection().unwrap();
        let command = match letter_action(&editor, false, false) {
            LetterAction::Insert => Command::Insert(editor.note(Step::D)),
            LetterAction::AddToChord => {
                Command::AddPitch(editor.note(Step::D).pitches.first().unwrap())
            }
            LetterAction::FillRest => {
                Command::ReplacePitch(editor.note(Step::D).pitches.first().unwrap())
            }
        };
        editor.execute(command).unwrap();
        assert_ne!(editor.selection(), Some(first));
        assert_eq!(
            editor.score().event(first).unwrap().pitches.first(),
            Pitch::new(Step::C, 4)
        );
        let cursor = editor.cursor();
        assert_eq!(
            editor
                .score()
                .part(cursor.part)
                .unwrap()
                .staff(cursor.staff)
                .unwrap()
                .measures()[cursor.measure]
                .len(),
            2
        );
    }
}

#[cfg(test)]
mod audition_tests {
    use super::audition_due;
    use std::time::{Duration, Instant};

    #[test]
    fn first_audition_is_always_due() {
        assert!(audition_due(None, Instant::now()));
    }

    #[test]
    fn held_arrows_do_not_burst() {
        let last = Instant::now();
        assert!(!audition_due(Some(last), last + Duration::from_millis(30)));
        assert!(audition_due(Some(last), last + Duration::from_millis(90)));
    }
}

#[cfg(test)]
mod form_command_tests {
    use super::{form_bounds, form_command};
    use score_core::{Barline, Duration, Event, Jump, Notation, Pitch, Pitches, Score, Step};
    use score_keymap::{CommandId, id};

    fn run(editor: &mut score_edit::Editor, command: CommandId) {
        let edit = form_command(editor, command).expect("form dispatch");
        editor.execute(edit).unwrap();
    }

    #[test]
    fn repeat_end_dispatch_is_one_undoable_editor_transaction() {
        let mut editor = score_edit::Editor::default();
        run(&mut editor, id::REPEAT_END);
        assert_eq!(editor.score().bar_form(0).repeat_end, Some(2));
        assert!(editor.undo());
        assert!(editor.score().bar_form(0).is_plain());
        assert!(editor.redo());
        run(&mut editor, id::REPEAT_END);
        assert!(editor.score().bar_form(0).is_plain());
    }

    #[test]
    fn selected_range_dispatch_targets_start_end_and_barline_toggle() {
        let mut score = Score::default();
        let event = Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
            notation: Notation::default(),
        };
        let a = score.insert(1, 0, event).unwrap();
        let b = score.insert(3, 0, event).unwrap();
        let mut editor = score_edit::Editor::new(score).unwrap();
        editor.select(b);
        editor.extend_selection(a);
        assert_eq!(form_bounds(&editor), (1, 3));
        for command in [id::REPEAT_START, id::SEGNO, id::CODA] {
            run(&mut editor, command);
        }
        for command in [id::REPEAT_END, id::TO_CODA, id::FINE, id::DA_CAPO_AL_FINE] {
            run(&mut editor, command);
        }
        let first = editor.score().bar_form(1);
        assert!(first.repeat_start && first.segno && first.coda);
        assert!(first.repeat_end.is_none());
        let last = editor.score().bar_form(3);
        assert!(last.repeat_end == Some(2) && last.to_coda && last.fine);
        assert_eq!(last.jump, Some(Jump::DaCapoAlFine));
        for (command, style) in [
            (id::DOUBLE_BARLINE, Barline::Double),
            (id::FINAL_BARLINE, Barline::Final),
        ] {
            run(&mut editor, command);
            assert_eq!(editor.score().bar_form(3).barline, style);
            run(&mut editor, command);
            assert_eq!(editor.score().bar_form(3).barline, Barline::Single);
        }
        run(&mut editor, id::ENDING_1);
        let ending = editor.score().ending_at(2).unwrap();
        assert_eq!((ending.start, ending.end), (1, 3));
        run(&mut editor, id::CLEAR_FORM);
        assert!((1..=3).all(|bar| editor.score().bar_form(bar).is_plain()
            && editor.score().ending_at(bar).is_none()));
    }
}
