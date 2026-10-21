use super::*;
use score_core::{InstrumentRange, Step};

#[test]
fn range_edits_restore_limits_notes_and_passage_selection_in_one_undo_step() {
    let mut score = Score::new("Rangos".into(), &[Instrument::Flute], 1).unwrap();
    let part = score.parts()[0].id();
    let note = Event {
        pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    };
    let first = score.insert_on(part, StaffId::Upper, 0, 0, note).unwrap();
    let last = score.insert_on(part, StaffId::Upper, 0, 1, note).unwrap();
    let mut editor = Editor::new(score.clone()).unwrap();
    editor.select(first);
    editor.extend_selection(last);
    let selected = editor.selected_ids();
    let range = InstrumentRange {
        lowest: 72,
        highest: 96,
    };
    assert!(
        editor
            .execute(Command::SetInstrumentRange(part, Some(range)))
            .unwrap()
    );
    assert_eq!(editor.selected_ids(), selected);
    assert_eq!(editor.score().event(first), score.event(first));
    assert_eq!(editor.score().review(None).len(), 2);
    assert!(
        !editor
            .execute(Command::SetInstrumentRange(part, Some(range)))
            .unwrap()
    );
    assert!(editor.undo());
    assert_eq!(editor.score(), &score);
    assert_eq!(editor.selected_ids(), selected);
    assert!(editor.redo());
    assert_eq!(
        editor.score().part(part).unwrap().range_override(),
        Some(range)
    );
    let before = editor.score().clone();
    assert!(
        editor
            .execute(Command::SetInstrumentRange(
                part,
                Some(InstrumentRange {
                    lowest: 96,
                    highest: 60
                })
            ))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
    assert!(
        editor
            .execute(Command::SetInstrumentRange(part, None))
            .unwrap()
    );
    assert!(editor.score().review(None).is_empty());
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
}

#[test]
fn range_changes_keep_the_exact_selected_position_inside_a_held_note() {
    let mut editor =
        Editor::new(Score::new("Posición".into(), &[Instrument::Violin], 1).unwrap()).unwrap();
    let part = editor.cursor().part;
    let mut event = editor.note(Step::G);
    event.duration = Duration::Whole;
    editor.execute(Command::Insert(event)).unwrap();
    let point = score_core::DivisiPoint::new(0, Duration::Quarter.ticks());
    editor.select_divisi_position(point);
    assert_eq!(editor.divisi_position(), point);
    editor
        .execute(Command::SetInstrumentRange(
            part,
            Some(InstrumentRange {
                lowest: 55,
                highest: 90,
            }),
        ))
        .unwrap();
    assert_eq!(editor.divisi_position(), point);
    assert!(editor.undo());
    assert_eq!(editor.divisi_position(), point);
}
