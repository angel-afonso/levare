use super::*;
use score_core::{
    Connection, ConnectionKind, Duration, KeyMode, KeySignature, NoteValue, StaffId, Step,
    TempoMark, TimeSignature,
};

fn selected_pitch(editor: &Editor) -> Option<Pitch> {
    editor
        .score()
        .event(editor.selection().unwrap())
        .unwrap()
        .pitches
        .first()
}

#[test]
fn letter_entry_picks_the_nearest_octave() {
    for (first, second, expected) in [
        (Step::B, Step::C, (4, 5)),
        (Step::C, Step::B, (4, 3)),
        (Step::C, Step::G, (4, 3)),
        (Step::C, Step::F, (4, 4)),
    ] {
        let mut editor = Editor::default();
        editor.execute(Command::Insert(editor.note(first))).unwrap();
        assert_eq!(selected_pitch(&editor).unwrap().octave(), expected.0);
        editor
            .execute(Command::Insert(editor.note(second)))
            .unwrap();
        assert_eq!(selected_pitch(&editor).unwrap().octave(), expected.1);
    }
}

#[test]
fn same_letter_repeats_the_same_pitch() {
    let mut editor = Editor::default();
    for _ in 0..3 {
        editor
            .execute(Command::Insert(editor.note(Step::E)))
            .unwrap();
        assert_eq!(selected_pitch(&editor), Pitch::new(Step::E, 4));
    }
}

#[test]
fn entry_octave_override_moves_the_reference() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(Step::C)))
        .unwrap();
    editor.octave = 5;
    editor
        .execute(Command::Insert(editor.note(Step::D)))
        .unwrap();
    assert_eq!(selected_pitch(&editor), Pitch::new(Step::D, 5));
}

#[test]
fn rest_or_empty_selection_uses_the_staff_octave() {
    let mut editor = Editor::default();
    editor.execute(Command::Insert(editor.rest())).unwrap();
    assert_eq!(selected_pitch(&editor), None);
    assert_eq!(
        editor.note(Step::C).pitches.first(),
        Pitch::new(Step::C, editor.octave)
    );
}

#[test]
fn nearest_octave_never_leaves_the_pitch_range() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(Step::B)))
        .unwrap();
    editor.octave = 8;
    let pitch = editor.note(Step::C).pitches.first().unwrap();
    assert!(pitch.octave() <= 8);
    assert_eq!(pitch, Pitch::new(Step::C, 8).unwrap());

    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(Step::C)))
        .unwrap();
    editor.octave = 0;
    assert_eq!(editor.note(Step::B).pitches.first(), Pitch::new(Step::B, 0));
}

#[test]
fn replace_selected_tone_erase_preserves_time_and_ripple_delete_undoes() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(editor.note(Step::C)))
        .unwrap();
    let first = editor.selection().unwrap();
    editor
        .execute(Command::AddPitch(Pitch::new(Step::E, 4).unwrap()))
        .unwrap();
    editor
        .execute(Command::AddPitch(Pitch::new(Step::G, 4).unwrap()))
        .unwrap();
    editor.select_pitch(first, Pitch::new(Step::E, 4));
    editor
        .execute(Command::ReplacePitch(Pitch::new(Step::F, 4).unwrap()))
        .unwrap();
    assert_eq!(
        editor
            .score()
            .event(first)
            .unwrap()
            .pitches
            .iter()
            .map(|p| p.step)
            .collect::<Vec<_>>(),
        [Step::C, Step::F, Step::G]
    );
    let before = editor.score().clone();
    assert!(
        editor
            .execute(Command::ReplacePitch(Pitch::new(Step::C, 4).unwrap()))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
    editor
        .execute(Command::Insert(editor.note(Step::D)))
        .unwrap();
    let second = editor.selection().unwrap();
    let onset = editor.score().event_tick(second);
    editor.select(first);
    let before = editor.score().clone();
    editor.execute(Command::Erase).unwrap();
    assert!(editor.score().event(first).unwrap().pitches.is_empty());
    assert_eq!(editor.score().event_tick(second), onset);
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.execute(Command::Delete).unwrap();
    assert!(editor.score().event_tick(second) < onset);
    editor.undo();
    assert_eq!(editor.score(), &before);
}
#[test]
fn duration_edit_rejects_overflow_atomically_and_preserves_chord_identity() {
    let mut editor = Editor::default();
    for step in [Step::C, Step::D, Step::E, Step::F] {
        editor.execute(Command::Insert(editor.note(step))).unwrap();
    }
    let last = editor.selection().unwrap();
    let before = editor.score().clone();
    assert!(
        editor
            .execute(Command::SetDuration(NoteValue::Half))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
    editor
        .execute(Command::SetDuration(NoteValue::Eighth))
        .unwrap();
    assert_eq!(editor.selection(), Some(last));
    assert_eq!(
        editor.score().event(last).unwrap().duration,
        Duration::Eighth
    );
    editor.undo();
    assert_eq!(editor.score(), &before);
}
#[test]
fn structural_edits_remap_all_staves_connections_and_carry_local_signatures() {
    let mut score = Score::default();
    score
        .remove_measures(1, score.measures().len() - 1)
        .unwrap();
    let mut editor = Editor::new(score).unwrap();
    for _ in 0..3 {
        editor.execute(Command::AppendMeasure).unwrap();
    }
    let part = editor.cursor().part;
    let mut ids = Vec::new();
    for bar in 0..4 {
        editor.set_cursor(Cursor {
            voice: Default::default(),
            part,
            staff: StaffId::Upper,
            measure: bar,
            index: 0,
        });
        editor
            .execute(Command::Insert(editor.note(Step::C)))
            .unwrap();
        ids.push(editor.selection().unwrap());
    }
    let connection = Connection {
        kind: ConnectionKind::Slur,
        start: ids[2],
        end: ids[3],
    };
    editor.execute(Command::AddConnection(connection)).unwrap();
    editor
        .execute(Command::SetTimeSignatureAt(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        ))
        .unwrap();
    let key = KeySignature::new(-3, KeyMode::Minor).unwrap();
    editor.execute(Command::SetKeySignatureAt(1, key)).unwrap();
    let tempo = TempoMark {
        bpm: 88,
        ..Default::default()
    };
    editor
        .execute(Command::SetTempoAt(1, tempo.clone()))
        .unwrap();
    let before = editor.score().clone();
    editor.set_cursor(Cursor {
        voice: Default::default(),
        part,
        staff: StaffId::Upper,
        measure: 1,
        index: 0,
    });
    editor.execute(Command::InsertMeasure).unwrap();
    assert_eq!(editor.score().measures().len(), 5);
    assert!(
        editor
            .score()
            .parts()
            .iter()
            .flat_map(|p| p.staves())
            .all(|s| s.measures()[1].is_empty())
    );
    assert_eq!(editor.score().connections()[0].start.measure, 3);
    assert_eq!(
        editor.score().time_at(2),
        TimeSignature {
            numerator: 3,
            denominator: 4
        }
    );
    assert_eq!(editor.score().time_at(1), TimeSignature::default());
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.set_cursor(Cursor {
        voice: Default::default(),
        part,
        staff: StaffId::Upper,
        measure: 1,
        index: 0,
    });
    editor.execute(Command::DeleteMeasures).unwrap();
    assert_eq!(editor.score().connections()[0].start.measure, 1);
    assert_eq!(
        editor.score().time_at(1),
        TimeSignature {
            numerator: 3,
            denominator: 4
        }
    );
    assert_eq!(editor.score().key_signature_at(1), key);
    assert_eq!(editor.score().tempo_at(1), &tempo);
    editor.score().validate().unwrap();
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.select_all();
    editor.execute(Command::DeleteMeasures).unwrap();
    assert_eq!(editor.score().measures().len(), 1);
    assert!(editor.score().measures()[0].is_empty());
    editor.score().validate().unwrap();
    editor.undo();
    assert_eq!(editor.score(), &before);
}
