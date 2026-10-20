use super::*;
use score_core::{KeyMode, KeySignature, Notation, Pitches, Step, TimeSignature};
fn note(duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::F, 4)),
        duration,
        notation: Notation::default(),
    }
}

#[test]
fn tempo_edits_preserve_music_inherit_by_bar_and_undo_without_changing_the_cursor() {
    use score_core::{TempoBeat, TempoMark};
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(note(Duration::Quarter)))
        .unwrap();
    let id = editor.selection();
    let cursor = editor.cursor();
    let initial = TempoMark {
        text: "Andante cantabile".into(),
        bpm: 84,
        ..Default::default()
    };
    let later = TempoMark {
        text: "Allegro".into(),
        beat: TempoBeat::DottedQuarter,
        bpm: 120,
    };
    editor
        .execute(Command::SetTempoAt(2, later.clone()))
        .unwrap();
    editor
        .execute(Command::SetTempoAt(0, initial.clone()))
        .unwrap();
    assert_eq!(editor.score().tempo_at(1), &initial);
    assert_eq!(editor.score().tempo_at(3), &later);
    assert_eq!(editor.selection(), id);
    assert_eq!(editor.cursor(), cursor);
    let before = editor.score().clone();
    for invalid in [
        TempoMark {
            bpm: 0,
            ..initial.clone()
        },
        TempoMark {
            text: "Dos\nlíneas".into(),
            ..initial.clone()
        },
    ] {
        assert!(editor.execute(Command::SetTempoAt(1, invalid)).is_err());
        assert_eq!(editor.score(), &before);
    }
    editor.execute(Command::RemoveTempoChange(2)).unwrap();
    assert_eq!(editor.score().tempo_at(3), &initial);
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
    assert!(editor.redo());
    assert_eq!(editor.score().tempo_at(3), &initial);
}
#[test]
fn signature_changes_preserve_cursor_notes_and_future_changes_and_undo_together() {
    let mut editor = Editor::default();
    let cursor = Cursor {
        measure: 1,
        ..editor.cursor()
    };
    editor.set_cursor(cursor);
    let time = TimeSignature {
        numerator: 3,
        denominator: 4,
    };
    let key = KeySignature::new(1, KeyMode::Major).unwrap();
    editor
        .execute(Command::SetTimeSignatureAt(1, time))
        .unwrap();
    editor.execute(Command::SetKeySignatureAt(2, key)).unwrap();
    assert_eq!(editor.cursor(), cursor);
    assert_eq!(editor.score().time_at(0), TimeSignature::default());
    assert_eq!(editor.score().time_at(3), time);
    assert_eq!(editor.score().key_signature_at(1), KeySignature::default());
    assert!(editor.undo());
    assert_eq!(editor.score().key_signature_at(2), KeySignature::default());
    assert!(editor.undo());
    assert_eq!(editor.score().time_at(3), TimeSignature::default());
    assert!(editor.redo());
    assert!(editor.redo());
    editor.execute(Command::RemoveTimeChange(1)).unwrap();
    assert_eq!(editor.score().time_at(3), TimeSignature::default());
    assert!(editor.undo());
    assert_eq!(editor.score().time_at(3), time);
}
#[test]
fn invalid_shorter_meter_never_changes_the_document_or_history() {
    let mut score = Score::default();
    score.insert(1, 0, note(Duration::Whole)).unwrap();
    let mut editor = Editor::new(score.clone()).unwrap();
    assert!(
        editor
            .execute(Command::SetTimeSignatureAt(
                1,
                TimeSignature {
                    numerator: 3,
                    denominator: 4
                }
            ))
            .is_err()
    );
    assert_eq!(editor.score(), &score);
    assert!(!editor.undo());
}
#[test]
fn copy_and_paste_across_meter_and_key_changes_preserves_time_and_sound() {
    let mut source = Score::default();
    source
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    source
        .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let a = source.insert(0, 0, note(Duration::Whole)).unwrap();
    let b = source.insert(1, 0, note(Duration::Half)).unwrap();
    let mut editor = Editor::new(source).unwrap();
    editor.select(a);
    editor.extend_selection(b);
    let clip = editor.copy_selection().unwrap();
    assert_eq!(clip.events[1].offset, 192);
    assert_eq!(
        clip.events[1].event.pitches.first().unwrap().accidental(),
        Some(score_core::Accidental::Sharp)
    );
    let mut destination = Score::default();
    destination
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    destination
        .set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let original = destination.clone();
    let mut editor = Editor::new(destination).unwrap();
    editor.paste(&clip).unwrap();
    let bar = &editor.score().measures()[1];
    assert_eq!(bar.used_ticks(), 144);
    let pitches: Vec<_> = bar
        .iter()
        .map(|(_, e)| {
            e.pitches
                .first()
                .unwrap()
                .in_key(editor.score().key_signature_at(1))
                .alteration()
        })
        .collect();
    assert_eq!(pitches, [0, 1]);
    editor.score().validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.score(), &original);
}
