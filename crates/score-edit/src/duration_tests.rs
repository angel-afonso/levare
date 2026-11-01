use super::*;
use score_core::{Articulation, ConnectionKind, NoteValue, Step, Voice};

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;

/// C D E F in quarters, and their ids.
fn four_quarters() -> (Editor, Vec<ElementId>) {
    let mut editor = Editor::default();
    let mut ids = Vec::new();
    for step in [Step::C, Step::D, Step::E, Step::F] {
        editor.execute(Command::Write(editor.note(step))).unwrap();
        ids.push(editor.selection().unwrap());
    }
    (editor, ids)
}

/// Each event of a bar: onset, duration and its note, if any.
fn bar(editor: &Editor, measure: usize) -> Vec<(u32, Duration, Option<Step>)> {
    editor.score().measures()[measure]
        .timed_iter()
        .filter(|(voice, ..)| *voice == Voice::One)
        .map(|(_, tick, _, event)| (tick, event.duration, event.pitches.first().map(|p| p.step)))
        .collect()
}

#[test]
fn lengthening_a_note_writes_over_what_follows() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor
        .execute(Command::SetDuration(NoteValue::Half))
        .unwrap();
    assert_eq!(
        bar(&editor, 0),
        [
            (0, Duration::Half, Some(Step::C)),
            (2 * Q, Duration::Quarter, Some(Step::E)),
            (3 * Q, Duration::Quarter, Some(Step::F))
        ]
    );
    // The changed note stays selected.
    let selected = editor.selection().unwrap();
    assert_eq!(
        editor.score().event(selected).unwrap().duration,
        Duration::Half
    );
}

#[test]
fn shortening_a_note_leaves_a_rest_and_later_notes_in_place() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor
        .execute(Command::SetDuration(NoteValue::Eighth))
        .unwrap();
    assert_eq!(
        bar(&editor, 0),
        [
            (0, Duration::Eighth, Some(Step::C)),
            (Q / 2, Duration::Eighth, None),
            (Q, Duration::Quarter, Some(Step::D)),
            (2 * Q, Duration::Quarter, Some(Step::E)),
            (3 * Q, Duration::Quarter, Some(Step::F))
        ]
    );
}

#[test]
fn a_dot_takes_half_of_the_next_note_and_leaves_its_rest() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor.execute(Command::SetDotted(true)).unwrap();
    assert_eq!(
        bar(&editor, 0)[..3],
        [
            (0, Duration::Quarter.with_dotted(true), Some(Step::C)),
            (3 * Q / 2, Duration::Eighth, None),
            (2 * Q, Duration::Quarter, Some(Step::E))
        ]
    );
    editor.select(editor.selection().unwrap());
    editor.execute(Command::SetDots(0)).unwrap();
    // Two eighth rests: like Sibelius and MuseScore, rests are not merged.
    assert_eq!(
        bar(&editor, 0)[1..3],
        [
            (Q, Duration::Eighth, None),
            (3 * Q / 2, Duration::Eighth, None)
        ]
    );
}

#[test]
fn a_long_value_at_the_end_of_a_bar_is_tied_into_the_next() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[3]);
    editor
        .execute(Command::SetDuration(NoteValue::Half))
        .unwrap();
    assert_eq!(
        bar(&editor, 0)[3],
        (3 * Q, Duration::Quarter, Some(Step::F))
    );
    assert_eq!(bar(&editor, 1)[0], (0, Duration::Quarter, Some(Step::F)));
    let ties = editor
        .score()
        .connections()
        .iter()
        .filter(|c| c.kind == ConnectionKind::Tie)
        .count();
    assert_eq!(ties, 1);
}

#[test]
fn the_note_keeps_its_marks_and_undo_restores_the_bar() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor
        .execute(Command::SetArticulation(Some(Articulation::Staccato)))
        .unwrap();
    let before = editor.score().clone();
    editor
        .execute(Command::SetDuration(NoteValue::Half))
        .unwrap();
    let selected = editor.selection().unwrap();
    assert_eq!(
        editor
            .score()
            .event(selected)
            .unwrap()
            .notation
            .articulation,
        Some(Articulation::Staccato)
    );
    editor.undo();
    assert_eq!(editor.score(), &before);
}
