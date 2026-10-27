use super::*;
use score_core::{GraceKind, MarkKind, MarkRef, Pitches, Step};

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

#[test]
fn ribbon_graces_toggle_add_transpose_and_undo_one_step_each() {
    let mut score = Score::default();
    let id = score.insert(0, 0, note(Step::D)).unwrap();
    let mut editor = Editor::new(score.clone()).unwrap();
    editor.select(id);
    let acciaccatura = MarkKind::Grace(GraceKind::Acciaccatura);
    editor.execute(Command::ToggleMark(acciaccatura)).unwrap();
    assert_eq!(
        editor.score().point_mark(id, acciaccatura.family()),
        Some(acciaccatura)
    );
    editor
        .execute(Command::TransposeGraceNote(id, 0, 1))
        .unwrap();
    editor.execute(Command::AddGraceNote).unwrap();
    let grace = editor.score().event(id).unwrap().notation.grace.unwrap();
    assert_eq!(grace.len(), 2);
    assert_eq!(grace.note(1), Pitch::new(Step::E, 5));
    assert!(editor.score().has_mark(MarkRef::Grace(id, 1)));
    assert!(!editor.score().has_mark(MarkRef::Grace(id, 2)));

    editor
        .execute(Command::RemoveMark(MarkRef::Grace(id, 0)))
        .unwrap();
    assert_eq!(
        editor
            .score()
            .event(id)
            .unwrap()
            .notation
            .grace
            .unwrap()
            .len(),
        1
    );
    // Two notes became an appoggiatura group; the ribbon changes its kind,
    // and takes it off when the kind is already the one pressed.
    let grace = editor.score().event(id).unwrap().notation.grace.unwrap();
    assert_eq!(grace.kind, GraceKind::Appoggiatura);
    editor.execute(Command::ToggleMark(acciaccatura)).unwrap();
    let grace = editor.score().event(id).unwrap().notation.grace.unwrap();
    assert_eq!((grace.kind, grace.len()), (GraceKind::Acciaccatura, 1));
    editor.execute(Command::ToggleMark(acciaccatura)).unwrap();
    assert_eq!(editor.score().event(id).unwrap().notation.grace, None);
    for _ in 0..6 {
        assert!(editor.undo());
    }
    assert_eq!(editor.score(), &score);
}
