mod clipboard;
mod follow;
mod keyboard;
mod menu_typography;
mod music_font;
mod paint;
mod playback;
mod playhead;
mod selection;
mod shortcuts;
mod sound_banks;
mod ui;

use gpui::{prelude::*, *};
use score_core::{
    Accidental, Articulation, Connection, ConnectionKind, Dynamic, ElementId, Instrument, KeyMode,
    KeySignature, PartId, PlayingTechnique, StaffId,
};
use score_document::Document;
use score_edit::{Command, Cursor};
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

type QuitSnapshot = Rc<RefCell<Option<score_core::Score>>>;

/// A key held longer than this without its key-up is a new press, not a repeat
/// (covers a key-up lost while the window was in the background).
const REPEAT_WINDOW: Elapsed = Elapsed::from_millis(700);

#[derive(Clone)]
enum Next {
    New,
    Open,
    OpenPath(PathBuf),
    Close,
}

struct AppView {
    document: Document,
    ui: ui::UiState,
    focus: FocusHandle,
    path: Option<PathBuf>,
    status: String,
    busy: bool,
    confirming: bool,
    instrument_picker: Option<ui::InstrumentPicker>,
    key_picker: bool,
    key_mode: KeyMode,
    chord_entry: bool,
    entry_accidental: Option<Accidental>,
    pending_slur: Option<ElementId>,
    pending_hairpin: Option<(ElementId, ConnectionKind)>,
    reveal_selection: bool,
    canvas_origin: Point<Pixels>,
    selection_drag: Option<selection::PassageDrag>,
    mixer_drag: Option<ui::MixerDrag>,
    viewport: follow::PlaybackViewport,
    layout: score_layout::Layout,
    music_font: Rc<music_font::MusicFont>,
    quit_snapshot: QuitSnapshot,
    playback: playback::Playback,
    shortcuts: Shortcuts,
    preferences: Option<ui::Preferences>,
    /// Set by the keystroke interceptor when the key being dispatched is an
    /// auto-repeat; consumed by the next `Run`.
    repeat_at: Option<Instant>,
    held: HashMap<String, Instant>,
    recent: score_io::RecentFiles,
    recent_path: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl AppView {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        quit_snapshot: QuitSnapshot,
        mut shortcuts: Shortcuts,
    ) -> Self {
        let (mut document, status) = match recovery_path().filter(|path| path.exists()) {
            Some(path) => match score_io::load(&path)
                .map_err(|error| error.to_string())
                .and_then(|score| Document::recover(score).map_err(|error| error.to_string()))
            {
                Ok(document) => (
                    document,
                    "Partitura recuperada del cierre anterior. Guárdala para conservarla."
                        .to_owned(),
                ),
                Err(error) => (
                    Document::default(),
                    format!("No se pudo recuperar la sesión: {error}"),
                ),
            },
            None => (
                Document::default(),
                "Escribe tu primera nota con las teclas A–G".to_owned(),
            ),
        };
        let recent_path = data_path("recientes.json");
        let recent = recent_path
            .as_deref()
            .and_then(|path| score_io::RecentFiles::load(path).ok())
            .unwrap_or_default();
        shortcuts.recent = recent.files().to_vec();
        shortcuts.install(cx);
        let layout = document.layout(960.);
        let focus = cx.focus_handle();
        focus.focus(window);
        let view = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            view.update(cx, |view, cx| {
                if view.busy || view.confirming {
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
        *quit_snapshot.borrow_mut() = document.is_dirty().then(|| document.editor.score().clone());
        cx.spawn(async move |view, cx| {
            loop {
                Timer::after(std::time::Duration::from_millis(16)).await;
                if view
                    .update(cx, |view: &mut Self, cx| {
                        if view.playback.tick() {
                            let cursor = view.document.editor.cursor();
                            let score = view.document.editor.score();
                            let position = view.playback.position(score, cursor);
                            view.viewport.follow(
                                &view.layout,
                                view.playback
                                    .playhead
                                    .filter(|id| id.measure == position.measure),
                                playhead::tracking_bounds(
                                    &view.layout,
                                    cursor,
                                    position,
                                    score.time().capacity(),
                                ),
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
                view.shortcuts.refresh_layout();
                view.shortcuts.install(cx);
                cx.notify();
            });
        });
        Self {
            document,
            ui: ui::UiState::default(),
            focus,
            path: None,
            status,
            busy: false,
            confirming: false,
            instrument_picker: None,
            key_picker: false,
            key_mode: KeyMode::Major,
            chord_entry: false,
            entry_accidental: None,
            pending_slur: None,
            pending_hairpin: None,
            reveal_selection: false,
            canvas_origin: Point::default(),
            selection_drag: None,
            mixer_drag: None,
            viewport: follow::PlaybackViewport::default(),
            layout,
            music_font: Rc::new(music_font::MusicFont::new()),
            quit_snapshot,
            playback: playback::Playback::new(),
            shortcuts,
            preferences: None,
            repeat_at: None,
            held: HashMap::new(),
            recent,
            recent_path,
            _subscriptions: vec![interceptor, layout_change],
        }
    }

    fn sync_quit_snapshot(&self) {
        *self.quit_snapshot.borrow_mut() = self
            .document
            .is_dirty()
            .then(|| self.document.editor.score().clone());
    }

    fn toggle_articulation(&mut self, mark: Articulation, cx: &mut Context<Self>) {
        let current = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .and_then(|event| event.notation.articulation);
        self.edit(
            Command::SetArticulation(if current == Some(mark) {
                None
            } else {
                Some(mark)
            }),
            cx,
        );
    }
    /// The ribbon has no remove buttons: pressing the selected note's active
    /// mark again removes it, as articulations and accidentals already do.
    fn toggle_dynamic(&mut self, mark: Dynamic, cx: &mut Context<Self>) {
        let current = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .and_then(|event| event.notation.dynamic);
        self.edit(
            Command::SetDynamic((current != Some(mark)).then_some(mark)),
            cx,
        );
    }
    fn toggle_technique(&mut self, mark: PlayingTechnique, cx: &mut Context<Self>) {
        let current = self
            .document
            .editor
            .selection()
            .and_then(|id| self.document.editor.score().event(id))
            .and_then(|event| event.notation.technique);
        self.edit(
            Command::SetTechnique((current != Some(mark)).then_some(mark)),
            cx,
        );
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
    fn toggle_tie(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
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
    fn toggle_hairpin(&mut self, kind: ConnectionKind, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
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
            self.status = format!(
                "{}: selecciona el final y pulsa el mismo regulador. Esc cancela",
                kind.label()
            );
            cx.notify();
        }
    }
    fn toggle_slur(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
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
            self.status =
                "Selecciona la nota final y pulsa Terminar expresión (L). Esc cancela".into();
            cx.notify();
        } else {
            self.status = "Elige una nota o acorde, no un silencio".into();
            cx.notify();
        }
    }

    fn edit(&mut self, command: Command, cx: &mut Context<Self>) {
        self.selection_drag = None;
        if self.busy || self.confirming || self.instrument_picker.is_some() {
            return;
        }
        self.status = match self.document.execute(command.clone()) {
            Ok(true) => {
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.playback.stop();
                self.reveal_selection = true;
                if matches!(
                    command,
                    Command::Insert(_)
                        | Command::Transpose(_)
                        | Command::AddPitch(_)
                        | Command::RemoveHighestPitch
                        | Command::SetAccidental(_)
                        | Command::SetKeySignature(_)
                        | Command::SetTechnique(_)
                        | Command::SetDynamic(_)
                        | Command::SetArticulation(_)
                ) && let Some(id) = self.document.editor.selection()
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
                    return;
                }
                match command {
                    Command::RemovePart | Command::RemovePartById(_) => {
                        "Instrumento eliminado. Puedes recuperarlo con Deshacer.".into()
                    }
                    Command::AddPart(_) => "Instrumento añadido y colocado en el score.".into(),
                    Command::AddParts(parts) => format!(
                        "{} instrumentos añadidos. Puedes deshacerlos juntos.",
                        parts.len()
                    ),
                    _ => "Partitura actualizada".into(),
                }
            }
            Ok(false) => self.status.clone(),
            Err(error) => error.to_string(),
        };
        self.sync_quit_snapshot();
        cx.notify();
    }

    /// Runs before any binding: marks auto-repeats and feeds the recorder in
    /// Preferences, which must see keys that are already bound.
    fn intercept(&mut self, event: &KeystrokeEvent, _: &mut Window, cx: &mut Context<Self>) {
        let stroke = &event.keystroke;
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
            modifiers.control,
            modifiers.alt,
            modifiers.shift,
            modifiers.platform,
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
        self.selection_drag = None;
        let Some(spec) = self.shortcuts.spec(command).copied() else {
            return;
        };
        if self.busy || self.confirming {
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
            id::NOTE_C,
            id::NOTE_D,
            id::NOTE_E,
            id::NOTE_F,
            id::NOTE_G,
            id::NOTE_A,
            id::NOTE_B,
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
        if note_input.contains(&command) && !self.ui.note_entry {
            self.status = match self.shortcuts.label(id::TOGGLE_NOTE_INPUT) {
                Some(keys) => format!("Activa la entrada de notas con {keys} para escribir"),
                None => "Activa la entrada de notas para escribir".into(),
            };
            cx.notify();
            return;
        }
        use score_core::Step;
        match command {
            id::NEW => self.request_next(Next::New, window, cx),
            id::OPEN => self.request_next(Next::Open, window, cx),
            id::SAVE => self.save(false, cx),
            id::SAVE_AS => self.save(true, cx),
            id::CLOSE | id::QUIT => self.request_next(Next::Close, window, cx),
            id::PREFERENCES => self.open_preferences(cx),
            id::CLEAR_RECENT => {
                self.recent.clear();
                self.save_recent(cx);
                self.status = "Lista de recientes borrada".into();
            }
            id::CLOSE_PANEL => self.close_preferences(cx),

            id::COPY => self.copy_music(false, cx),
            id::CUT => self.copy_music(true, cx),
            id::PASTE => self.paste_music(cx),
            id::SELECT_ALL => {
                if self.document.editor.select_all() {
                    self.status =
                        "Pentagrama seleccionado. Puedes copiar o cortar el pasaje".into();
                    self.reveal_selection = true;
                } else {
                    self.status = "Este pentagrama no tiene notas ni silencios escritos".into();
                }
            }
            id::EXTEND_PREVIOUS | id::EXTEND_NEXT => {
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
            id::DELETE => self.edit(Command::Delete, cx),
            id::ESCAPE => self.escape(cx),
            id::REPEAT => self.repeat_selection(cx),

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
            id::REST => self.edit(Command::Insert(self.document.editor.rest()), cx),
            id::CHORD_MODE => self.toggle_chord(cx),
            id::REMOVE_HIGHEST => self.edit(Command::RemoveHighestPitch, cx),
            id::ENTRY_OCTAVE_UP => self.change_entry_octave(1, cx),
            id::ENTRY_OCTAVE_DOWN => self.change_entry_octave(-1, cx),

            id::DOT => self.toggle_rhythm(false, cx),
            id::TRIPLET => self.toggle_rhythm(true, cx),
            id::SIXTEENTH => self.choose_duration(score_core::Duration::Sixteenth, cx),
            id::EIGHTH => self.choose_duration(score_core::Duration::Eighth, cx),
            id::QUARTER => self.choose_duration(score_core::Duration::Quarter, cx),
            id::HALF => self.choose_duration(score_core::Duration::Half, cx),
            id::WHOLE => self.choose_duration(score_core::Duration::Whole, cx),

            id::NATURAL => self.toggle_accidental(Accidental::Natural, cx),
            id::SHARP => self.toggle_accidental(Accidental::Sharp, cx),
            id::FLAT => self.toggle_accidental(Accidental::Flat, cx),
            id::FOLLOW_KEY => self.apply_accidental(None, cx),

            id::TIE => self.toggle_tie(cx),
            id::SLUR => self.toggle_slur(cx),
            id::REMOVE_TIE => self.edit(Command::RemoveConnections(ConnectionKind::Tie), cx),
            id::REMOVE_SLUR => self.edit(Command::RemoveConnections(ConnectionKind::Slur), cx),
            id::STACCATO => self.toggle_articulation(Articulation::Staccato, cx),
            id::TENUTO => self.toggle_articulation(Articulation::Tenuto, cx),
            id::ACCENT => self.toggle_articulation(Articulation::Accent, cx),
            id::MARCATO => self.toggle_articulation(Articulation::Marcato, cx),
            id::CRESCENDO => self.toggle_hairpin(ConnectionKind::Crescendo, cx),
            id::DIMINUENDO => self.toggle_hairpin(ConnectionKind::Diminuendo, cx),
            id::REMOVE_CRESCENDO => {
                self.edit(Command::RemoveConnections(ConnectionKind::Crescendo), cx)
            }
            id::REMOVE_DIMINUENDO => {
                self.edit(Command::RemoveConnections(ConnectionKind::Diminuendo), cx)
            }
            id::PIZZICATO => self.toggle_technique(PlayingTechnique::Pizzicato, cx),
            id::ARCO => self.toggle_technique(PlayingTechnique::Arco, cx),
            id::CLEAR_TECHNIQUE => self.edit(Command::SetTechnique(None), cx),
            id::CLEAR_ARTICULATION => self.edit(Command::SetArticulation(None), cx),
            id::DYNAMIC_PPP => self.toggle_dynamic(Dynamic::Ppp, cx),
            id::DYNAMIC_PP => self.toggle_dynamic(Dynamic::Pp, cx),
            id::DYNAMIC_P => self.toggle_dynamic(Dynamic::P, cx),
            id::DYNAMIC_MP => self.toggle_dynamic(Dynamic::Mp, cx),
            id::DYNAMIC_MF => self.toggle_dynamic(Dynamic::Mf, cx),
            id::DYNAMIC_F => self.toggle_dynamic(Dynamic::F, cx),
            id::DYNAMIC_FF => self.toggle_dynamic(Dynamic::Ff, cx),
            id::DYNAMIC_FFF => self.toggle_dynamic(Dynamic::Fff, cx),
            id::CLEAR_DYNAMIC => self.edit(Command::SetDynamic(None), cx),

            id::KEY_SIGNATURE => {
                self.ui.show_instruments = true;
                self.toggle_key_picker(cx);
            }
            id::BAR_AT_END => self.edit(Command::AppendMeasure, cx),

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
                self.document.editor.navigate(command == id::NEXT);
                self.reveal_selection = true;
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
            id::TEMPO_UP => self.playback.change_tempo(5),
            id::TEMPO_DOWN => self.playback.change_tempo(-5),
            id::MIXER => {
                self.ui.show_inspector = true;
                self.ui.inspector = "Mezclador";
            }

            id::ZOOM_IN => self.set_zoom(self.ui.zoom + 0.1),
            id::ZOOM_OUT => self.set_zoom(self.ui.zoom - 0.1),
            id::ZOOM_ACTUAL => self.set_zoom(1.),
            id::FULL_SCREEN => window.toggle_fullscreen(),
            id::TOGGLE_INSTRUMENTS_PANEL => {
                self.ui.show_instruments = !self.ui.show_instruments;
                self.reveal_selection = true;
            }
            id::TOGGLE_INSPECTOR => {
                self.ui.show_inspector = !self.ui.show_inspector;
                self.reveal_selection = true;
            }
            _ => self.status = format!("Sin acción para «{}»", spec.label),
        }
        cx.notify();
    }

    fn write_note(&mut self, step: score_core::Step, cx: &mut Context<Self>) {
        let note = self.entry_note(step);
        let adding = self.chord_entry
            && self
                .document
                .editor
                .selection()
                .and_then(|id| self.document.editor.score().event(id))
                .is_some_and(|event| !event.pitches.is_empty());
        self.edit(
            if adding {
                Command::AddPitch(note.pitches.first().expect("entry note"))
            } else {
                Command::Insert(note)
            },
            cx,
        );
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
            Some(event) => self.edit(Command::Insert(event), cx),
            None => self.status = "Selecciona una nota, acorde o silencio para repetirlo".into(),
        }
    }

    fn escape(&mut self, cx: &mut Context<Self>) {
        self.selection_drag = None;
        if self.pending_hairpin.take().is_some() {
            self.status = "Regulador cancelado".into();
        } else if self.pending_slur.take().is_some() {
            self.status = "Ligadura cancelada".into();
        } else if self.playback.is_playing() || self.playback.is_paused() {
            self.stop_playback(cx);
        } else {
            self.document
                .editor
                .set_cursor(self.document.editor.cursor());
        }
    }

    fn toggle_rhythm(&mut self, triplet: bool, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
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
    fn choose_duration(&mut self, duration: score_core::Duration, _: &mut Context<Self>) {
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

    /// ⌥↑/⌥↓: the next note of the chord, then the adjacent staff.
    fn move_vertically(&mut self, up: bool, cx: &mut Context<Self>) {
        let editor = &mut self.document.editor;
        if let Some(event) = editor.selection().and_then(|id| editor.score().event(id)) {
            let pitches = event.pitches;
            if let Some(index) = pitches
                .iter()
                .position(|pitch| Some(pitch) == editor.selected_pitch())
            {
                let next = if up { index + 1 } else { index.wrapping_sub(1) };
                if next < pitches.len() {
                    editor.select_tone(next);
                    return;
                }
            }
        }
        let staves: Vec<(PartId, StaffId)> = editor
            .score()
            .parts()
            .iter()
            .flat_map(|part| {
                part.staves()
                    .iter()
                    .map(move |staff| (part.id(), staff.id()))
            })
            .collect();
        let cursor = editor.cursor();
        let Some(index) = staves
            .iter()
            .position(|staff| *staff == (cursor.part, cursor.staff))
        else {
            return;
        };
        let target = if up {
            index.checked_sub(1)
        } else {
            Some(index + 1).filter(|next| *next < staves.len())
        };
        if let Some((part, staff)) = target.map(|index| staves[index]) {
            if part != cursor.part {
                editor.select_part(part);
            }
            editor.select_staff(staff);
            self.playback.follow_staff(part, staff);
            self.reveal_selection = true;
            cx.notify();
        }
    }

    fn set_zoom(&mut self, zoom: f32) {
        self.ui.zoom = (zoom * 10.).round().clamp(5., 15.) / 10.;
        self.reveal_selection = true;
    }

    fn open_preferences(&mut self, cx: &mut Context<Self>) {
        self.preferences = Some(ui::Preferences::default());
        self.playback.stop();
        cx.notify();
    }

    fn open_instrument_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.preferences.is_some() {
            return;
        }
        self.instrument_picker = Some(ui::InstrumentPicker::default());
        self.selection_drag = None;
        self.key_picker = false;
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
        if self.busy || self.confirming {
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
        if keys.to_platform(&self.shortcuts.layout).is_none() {
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
        if self.busy || self.confirming || self.instrument_picker.is_some() {
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
                clear_recovery();
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.document = Document::default();
                self.playback.reset_part_mix(
                    self.document.editor.score(),
                    self.document.editor.cursor().part,
                );
                self.mixer_drag = None;
                self.viewport.scroll.set_offset(Point::default());
                self.instrument_picker = None;
                self.key_picker = false;
                self.chord_entry = false;
                self.entry_accidental = None;
                self.path = None;
                self.status = "Nueva partitura · clave de sol · 4/4".into();
            }
            Next::Open => self.open(None, cx),
            Next::OpenPath(path) => self.open(Some(path), cx),
            Next::Close => {
                clear_recovery();
                *self.quit_snapshot.borrow_mut() = None;
                window.remove_window();
            }
        }
        if !closing {
            self.sync_quit_snapshot();
        }
        cx.notify();
    }

    /// Opens `known`, or asks for a file when it is `None`.
    fn open(&mut self, known: Option<PathBuf>, cx: &mut Context<Self>) {
        self.busy = true;
        let picker = known.is_none().then(|| {
            cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Abrir partitura".into()),
            })
        });
        cx.spawn(async move |view, cx| {
            let path = match (known, picker) {
                (Some(path), _) => Ok(Some(path)),
                (None, Some(picker)) => match picker.await {
                    Ok(Ok(paths)) => Ok(paths.and_then(|paths| paths.into_iter().next())),
                    Ok(Err(error)) => Err(error.to_string()),
                    Err(error) => Err(error.to_string()),
                },
                (None, None) => Ok(None),
            };
            let result = match path {
                Ok(Some(path)) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            score_io::load(&path)
                                .map(|score| (score, path))
                                .map_err(|error| error.to_string())
                        })
                        .await,
                ),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Some(Ok((score, path))) => match Document::new(score) {
                        Ok(document) => {
                            clear_recovery();
                            view.pending_slur = None;
                            view.pending_hairpin = None;
                            view.document = document;
                            view.playback.reset_part_mix(
                                view.document.editor.score(),
                                view.document.editor.cursor().part,
                            );
                            view.mixer_drag = None;
                            view.viewport.scroll.set_offset(Point::default());
                            view.instrument_picker = None;
                            view.key_picker = false;
                            view.chord_entry = false;
                            view.entry_accidental = None;
                            view.remember_recent(&path, cx);
                            view.path = Some(path);
                            view.status = "Partitura abierta".into();
                            view.sync_quit_snapshot();
                        }
                        Err(error) => view.status = error.to_string(),
                    },
                    Some(Err(error)) => view.status = format!("No se pudo abrir: {error}"),
                    None => {}
                }
                cx.notify();
            });
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

    fn save(&mut self, save_as: bool, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.busy = true;
        let existing = if save_as { None } else { self.path.clone() };
        let picker = existing.is_none().then(|| {
            let directory = self
                .path
                .as_ref()
                .and_then(|path| path.parent())
                .map(|p| p.to_owned())
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            cx.prompt_for_new_path(&directory, Some("Sin título.partitura"))
        });
        let score = self.document.editor.score().clone();
        cx.spawn(async move |view, cx| {
            let path = match (existing, picker) {
                (Some(path), _) => Ok(Some(path)),
                (_, Some(picker)) => picker
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|result| result.map_err(|e| e.to_string())),
                _ => Ok(None),
            };
            let result = match path {
                // Preserve exactly the path whose replacement the native panel approved.
                Ok(Some(path)) => Some(
                    cx.background_executor()
                        .spawn(async move {
                            score_io::save(&path, &score)
                                .map(|_| path)
                                .map_err(|error| error.to_string())
                        })
                        .await,
                ),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            };
            let _ = view.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Some(Ok(path)) => {
                        clear_recovery();
                        view.document.mark_saved();
                        view.status = format!("Guardado: {}", path.display());
                        view.remember_recent(&path, cx);
                        view.path = Some(path);
                        view.sync_quit_snapshot();
                    }
                    Some(Err(error)) => view.status = format!("No se pudo guardar: {error}"),
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
        if self.busy || self.confirming {
            return;
        }
        let changed = if redo {
            self.document.editor.redo()
        } else {
            self.document.editor.undo()
        };
        if changed {
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
        if self.busy || self.confirming {
            return;
        }
        self.document.editor.select_part(part);
        self.playback
            .follow_staff(part, self.document.editor.cursor().staff);
        self.reveal_selection = true;
        cx.notify();
    }

    fn toggle_key_picker(&mut self, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.key_picker = !self.key_picker;
        if self.key_picker {
            self.key_mode = self.document.editor.score().key_signature().mode();
            self.instrument_picker = None;
        }
        cx.notify();
    }
    fn choose_key(&mut self, key: KeySignature, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
        self.edit(Command::SetKeySignature(key), cx);
        self.key_picker = false;
        self.status = format!(
            "{} · las notas sin alteración propia siguen la armadura",
            key.label()
        );
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
        if self.busy || self.confirming {
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
        if self.busy || self.confirming {
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
        if self.busy || self.confirming {
            return;
        }
        if self
            .document
            .editor
            .score()
            .part(part)
            .is_none_or(|part| part.instrument() != Instrument::Piano)
        {
            self.status = "Selecciona un piano para cambiar de mano".into();
            cx.notify();
            return;
        }
        if self.document.editor.cursor().part != part {
            self.document.editor.select_part(part);
        }
        self.document.editor.select_staff(staff);
        self.playback.follow_staff(part, staff);
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
        if self.busy || self.confirming {
            return;
        }
        self.ui.note_entry = !self.ui.note_entry;
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
    fn seek_playback(&mut self, tick: u64, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            return;
        }
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
                        self.document.editor.score().time().capacity(),
                    ),
                    true,
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
                Ok(()) => format!(
                    "Sonido: {} · Listo para reproducir",
                    self.playback.sound_name()
                ),
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
        };
        self.layout = self.document.layout_compact(
            ((f32::from(window.viewport_size().width) - panels - 128.) / self.ui.zoom).max(500.),
        );
        self.layout.scale = self.ui.zoom;
        for label in &mut self.layout.labels {
            if label.staff == Some(StaffId::Upper)
                && self
                    .document
                    .editor
                    .score()
                    .part(label.part)
                    .is_some_and(|part| part.instrument() != Instrument::Piano)
            {
                label.text = ui::instrument_name(self.document.editor.score(), label.part);
            }
        }
        let title = self
            .path
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.document.editor.score().title().to_owned());
        window.set_window_title(&format!(
            "{}{} — Partitura",
            title,
            if self.document.is_dirty() { " •" } else { "" }
        ));
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(ui::BG))
            .text_color(rgb(ui::TEXT))
            .font_family(".SystemUIFont")
            .track_focus(&self.focus)
            .key_context(
                if self.preferences.is_some() || self.instrument_picker.is_some() {
                    "Partitura Modal"
                } else {
                    "Partitura"
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
            .child(ui::workspace(self, title, cx))
    }
}

fn data_path(name: &str) -> Option<PathBuf> {
    dirs::data_local_dir().map(|directory| directory.join("Partitura").join(name))
}

fn recovery_path() -> Option<PathBuf> {
    data_path("recovery.partitura")
}

fn clear_recovery() {
    if let Some(path) = recovery_path() {
        let _ = std::fs::remove_file(path);
    }
}

fn main() {
    Application::new().run(|cx| {
        // App-level ownership survives window/view teardown during a native
        // Dock quit. GPUI's Context::on_app_quit would only hold a weak view.
        let quit_snapshot: QuitSnapshot = Rc::new(RefCell::new(None));
        let pending = quit_snapshot.clone();
        cx.on_app_quit(move |_| {
            if let Some(score) = pending.borrow().as_ref()
                && let Some(path) = recovery_path()
            {
                let result = std::fs::create_dir_all(path.parent().expect("recovery parent"))
                    .map_err(|error| error.to_string())
                    .and_then(|_| score_io::save(&path, score).map_err(|error| error.to_string()));
                if let Err(error) = result {
                    eprintln!("No se pudo guardar la recuperación: {error}");
                }
            }
            async {}
        })
        .detach();
        let shortcuts = Shortcuts::load();
        shortcuts.install(cx);
        let bounds = Bounds::centered(None, size(px(1440.), px(1000.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.), px(680.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Partitura".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(16.), px(16.))),
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| AppView::new(window, cx, quit_snapshot, shortcuts)),
        )
        .expect("open Partitura window");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
