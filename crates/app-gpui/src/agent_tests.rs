use crate::agent::{Turn, user_may_run};
use agent_tools::{EscribirArgs, Host, ToolCall, dispatch};
use score_core::{Duration, ElementId, Event, Instrument, Pitch, Pitches, Score, Step};
use score_edit::{Command, Editor};
use score_keymap::{Category, CommandId, Registry};

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
