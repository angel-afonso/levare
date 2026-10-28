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
fn squash_of_edits_that_cancel_out_leaves_no_step() {
    let mut editor = Editor::default();
    editor.execute(Command::Write(event(Step::G))).unwrap();
    let mark = editor.mark();
    let before = editor.score().clone();
    editor.execute(Command::ToggleRehearsalMark).unwrap();
    editor.execute(Command::ToggleRehearsalMark).unwrap();
    // Moving the selection does not count as a change.
    editor.navigate(false);
    assert_eq!(editor.score(), &before);
    assert!(!editor.squash(&mark));
    assert_eq!(editor.score(), &before);
    // The edit before the mark is the only one left to undo.
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

#[test]
fn restore_selection_keeps_the_current_one_when_the_marked_is_gone() {
    let mut editor = Editor::default();
    write_three(&mut editor);
    let last = editor.selection().unwrap();
    editor.extend_navigation(false);
    editor.extend_navigation(false);
    assert_eq!(editor.selected_ids().len(), 3);
    let mark = editor.mark();
    // Removing the passage takes away both ends of the marked range.
    editor.execute(Command::Delete).unwrap();
    assert!(editor.score().event(last).is_none());
    let (selection, cursor) = (editor.selection(), editor.cursor());
    editor.restore_selection(&mark);
    assert_eq!(editor.selection(), selection);
    assert_eq!(editor.cursor(), cursor);
    assert!(editor.selected_ids().len() <= 1);
}

#[test]
fn the_revision_names_the_score_through_edits_undo_redo_rollback_and_squash() {
    let mut editor = Editor::default();
    let opened = editor.revision();
    assert_ne!(Editor::default().revision(), opened);
    editor.execute(Command::Write(event(Step::G))).unwrap();
    let written = editor.revision();
    assert_ne!(written, opened);
    editor.navigate(false);
    assert_eq!(editor.revision(), written);
    assert!(editor.undo());
    assert_eq!(editor.revision(), opened);
    assert!(editor.redo());
    assert_eq!(editor.revision(), written);

    let mark = editor.mark();
    write_three(&mut editor);
    assert_ne!(editor.revision(), written);
    editor.rollback(&mark);
    assert_eq!(editor.revision(), written);

    editor.execute(Command::ToggleRehearsalMark).unwrap();
    assert_ne!(editor.revision(), written);
    editor.execute(Command::ToggleRehearsalMark).unwrap();
    assert!(!editor.squash(&mark));
    assert_eq!(editor.revision(), written);

    write_three(&mut editor);
    let squashed = editor.revision();
    assert!(editor.squash(&mark));
    assert_eq!(editor.revision(), squashed);
    assert!(editor.undo());
    assert_eq!(editor.revision(), written);
}
