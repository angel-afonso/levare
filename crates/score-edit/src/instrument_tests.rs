use super::*;
use score_core::{Connection, ConnectionKind, Step};

#[test]
fn adding_an_ensemble_is_one_transaction_and_undo_restores_the_previous_selection() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(Step::C)))
        .unwrap();
    let original = editor.score().clone();
    let selection = editor.selection();
    let cursor = editor.cursor();
    editor
        .execute(Command::AddParts(vec![
            Instrument::Violin,
            Instrument::Flute,
            Instrument::Violin,
        ]))
        .unwrap();
    let ensemble = editor.score().clone();
    assert_eq!(ensemble.parts().len(), 4);
    assert_eq!(
        ensemble
            .parts()
            .iter()
            .map(|part| part.instrument())
            .collect::<Vec<_>>(),
        [
            Instrument::Flute,
            Instrument::Piano,
            Instrument::Violin,
            Instrument::Violin
        ]
    );
    assert_eq!(ensemble.part(cursor.part).unwrap().staves().len(), 2);
    assert!(editor.undo());
    assert_eq!(editor.score(), &original);
    assert_eq!(editor.cursor(), cursor);
    assert_eq!(editor.selection(), selection);
    assert!(editor.redo());
    assert_eq!(editor.score(), &ensemble);
}

#[test]
fn invalid_or_empty_ensemble_does_not_partially_edit_the_score_or_history() {
    let mut score = Score::default();
    for _ in 1..score_core::MAX_PARTS - 1 {
        score.add_part(Instrument::Violin).unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    let original = editor.score().clone();
    let cursor = editor.cursor();
    assert!(
        editor
            .execute(Command::AddParts(vec![
                Instrument::Flute,
                Instrument::Cello
            ]))
            .is_err()
    );
    assert_eq!(editor.score(), &original);
    assert_eq!(editor.cursor(), cursor);
    assert!(!editor.can_undo());
    assert!(!editor.execute(Command::AddParts(Vec::new())).unwrap());
    assert!(!editor.can_undo());
    editor
        .execute(Command::AddParts(vec![Instrument::Flute]))
        .unwrap();
    assert_eq!(editor.score().parts().len(), score_core::MAX_PARTS);
    editor.score().validate().unwrap();
}

#[test]
fn deleting_another_row_preserves_the_selected_passage_and_undo_restores_its_part() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let flute = score.add_part(Instrument::Flute).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select_part(piano);
    editor
        .execute(Command::Insert(editor.note(Step::C)))
        .unwrap();
    let first = editor.selection().unwrap();
    editor
        .execute(Command::Insert(editor.note(Step::D)))
        .unwrap();
    let second = editor.selection().unwrap();
    editor.select(first);
    editor.extend_selection(second);
    let original = editor.score().clone();
    let cursor = editor.cursor();
    let passage = editor.selected_ids();
    editor.execute(Command::RemovePartById(flute)).unwrap();
    assert_eq!(editor.cursor(), cursor);
    assert_eq!(editor.selected_ids(), passage);
    assert!(editor.score().part(flute).is_none());
    assert!(editor.undo());
    assert_eq!(editor.score(), &original);
    assert_eq!(editor.selected_ids(), passage);
    assert!(editor.redo());
    assert_eq!(editor.selected_ids(), passage);
}

#[test]
fn deleting_another_row_keeps_the_manually_chosen_entry_octave() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let flute = score.add_part(Instrument::Flute).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select_part(piano);
    editor.octave = 7;
    editor.execute(Command::RemovePartById(flute)).unwrap();
    assert_eq!(editor.octave, 7);
    assert_eq!(editor.note(Step::D).pitches.first().unwrap().octave(), 7);
}

#[test]
fn deleting_the_active_piano_recovers_both_hands_and_connections_on_undo() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let template = Editor::default();
    let event = template.note(Step::C);
    let first = score.insert_on(piano, StaffId::Upper, 0, 0, event).unwrap();
    let second = score.insert_on(piano, StaffId::Upper, 0, 1, event).unwrap();
    let lower = score.insert_on(piano, StaffId::Lower, 0, 0, event).unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: second,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    let flute = score.add_part(Instrument::Flute).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(lower);
    let original = editor.score().clone();
    editor.execute(Command::RemovePartById(piano)).unwrap();
    assert_eq!(editor.cursor().part, flute);
    assert_eq!(editor.cursor().staff, StaffId::Upper);
    assert!(editor.selection().is_none());
    assert!(editor.score().connections().is_empty());
    assert!(editor.execute(Command::RemovePartById(flute)).is_err());
    assert!(editor.undo());
    assert_eq!(editor.score(), &original);
    assert_eq!(editor.selection(), Some(lower));
    editor.score().validate().unwrap();
}
