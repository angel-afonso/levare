use super::*;
use score_core::{Pitches, Step};

fn event(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

fn write_three(editor: &mut Editor) {
    for step in [Step::C, Step::D, Step::E] {
        editor.execute(Command::Write(event(step))).unwrap();
    }
}

#[test]
fn squash_turns_many_edits_into_one_undo() {
    let mut editor = Editor::default();
    editor.execute(Command::Write(event(Step::G))).unwrap();
    let mark = editor.mark();
    let before = editor.score().clone();
    write_three(&mut editor);
    assert!(editor.squash(&mark));
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
    // The edit before the mark is still the only thing left to undo.
    assert!(editor.can_undo());
    assert!(editor.undo());
    assert!(!editor.can_undo());
}

#[test]
fn squash_without_changes_adds_nothing() {
    let mut editor = Editor::default();
    let mark = editor.mark();
    assert!(!editor.squash(&mark));
    assert!(!editor.can_undo());
    editor.execute(Command::Write(event(Step::G))).unwrap();
    let mark = editor.mark();
    assert!(!editor.squash(&mark));
    assert!(editor.can_undo());
    assert!(editor.undo());
    assert!(!editor.can_undo());
}

#[test]
fn rollback_discards_partial_edits() {
    let mut editor = Editor::default();
    editor.execute(Command::Write(event(Step::G))).unwrap();
    editor.undo();
    let mark = editor.mark();
    let before = editor.score().clone();
    let cursor = editor.cursor();
    editor.execute(Command::Write(event(Step::C))).unwrap();
    editor.execute(Command::Write(event(Step::D))).unwrap();
    editor.rollback(&mark);
    assert_eq!(editor.score(), &before);
    assert_eq!(editor.cursor(), cursor);
    assert!(!editor.can_undo());
    assert!(!editor.can_redo());
}

#[test]
fn restore_selection_keeps_score_and_history() {
    let mut editor = Editor::default();
    editor.execute(Command::Write(event(Step::G))).unwrap();
    let selected = editor.selection();
    let cursor = editor.cursor();
    let mark = editor.mark();
    let before = editor.score().clone();
    write_three(&mut editor);
    assert_ne!(editor.selection(), selected);
    let after = editor.score().clone();
    assert!(editor.squash(&mark));
    editor.restore_selection(&mark);
    assert_eq!(editor.selection(), selected);
    assert_eq!(editor.cursor(), cursor);
    assert_eq!(editor.score(), &after);
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
}
