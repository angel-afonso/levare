use super::*;
use score_core::{Accidental, KeyMode, KeySignature, Step};

fn pitch(step: Step, octave: u8, accidental: Option<Accidental>) -> Pitch {
    Pitch::new(step, octave)
        .unwrap()
        .with_accidental(accidental)
}

/// One part in concert D major, with a written note in its first bar.
fn editor(instrument: Instrument, written: Pitch) -> (Editor, ElementId) {
    let mut score = Score::new("Concierto".into(), &[instrument], 2).unwrap();
    score.set_key_signature(KeySignature::new(2, KeyMode::Major).unwrap());
    let part = score.parts()[0].id();
    let id = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: score_core::Pitches::single(Some(written)),
                duration: Duration::Quarter,
                notation: Default::default(),
            },
        )
        .unwrap();
    (Editor::new(score).unwrap(), id)
}

fn stored(editor: &Editor, id: ElementId) -> Option<Pitch> {
    editor.score().event(id)?.pitches.first()
}

#[test]
fn the_concert_view_changes_no_note_revision_or_history() {
    let (mut editor, id) = editor(Instrument::ClarinetBb, pitch(Step::E, 4, None));
    let (score, revision) = (editor.score().clone(), editor.revision());
    editor.select(id);
    editor.set_concert_pitch(true);
    assert!(editor.concert_pitch());
    assert_eq!(editor.score(), &score);
    assert_eq!(editor.revision(), revision);
    assert!(!editor.can_undo());
    assert_eq!(editor.selected_pitch(), Some(pitch(Step::D, 4, None)));
    assert_eq!(editor.shown_score().written_key_at(id.part, 0).fifths(), 2);
    // A command that changes nothing leaves the stored score as it was.
    assert!(
        !editor
            .execute(Command::SetChordsSound(editor.score().chords_sound()))
            .unwrap_or(false)
    );
    assert_eq!(editor.score(), &score);
    assert_eq!(editor.revision(), revision);
    editor.set_concert_pitch(false);
    assert_eq!(editor.selected_pitch(), Some(pitch(Step::E, 4, None)));
    assert_eq!(editor.shown_score(), &score);
}

#[test]
fn notes_entered_in_concert_pitch_are_stored_as_the_player_reads_them() {
    let (mut editor, id) = editor(Instrument::HornF, pitch(Step::A, 4, None));
    editor.set_concert_pitch(true);
    editor.select(id);
    // A concert G after the horn's concert D: the horn reads D, a fifth above.
    editor
        .execute(Command::Insert(editor.note(Step::G)))
        .unwrap();
    let added = editor.selection().unwrap();
    assert_ne!(added, id);
    assert_eq!(editor.selected_pitch().map(|p| p.step), Some(Step::G));
    assert_eq!(stored(&editor, added).map(|p| p.step), Some(Step::D));
    // The untouched note keeps its written spelling.
    assert_eq!(stored(&editor, id), Some(pitch(Step::A, 4, None)));
    // Signs act on the note as shown: a concert B♭ is the horn's F♮
    // against its A major, and the concert B♮ is the horn's F♯.
    editor.select(id);
    editor
        .execute(Command::ReplacePitch(pitch(
            Step::B,
            4,
            Some(Accidental::Flat),
        )))
        .unwrap();
    let natural = Some(pitch(Step::F, 5, Some(Accidental::Natural)));
    assert_eq!(stored(&editor, id), natural);
    editor
        .execute(Command::SetAccidental(Some(Accidental::Natural)))
        .unwrap();
    assert_eq!(
        stored(&editor, id),
        Some(pitch(Step::F, 5, Some(Accidental::Sharp)))
    );
    assert_eq!(
        editor.selected_pitch(),
        Some(pitch(Step::B, 4, Some(Accidental::Natural)))
    );
    // Undo goes back through written scores.
    assert!(editor.undo());
    assert_eq!(stored(&editor, id), natural);
    assert!(editor.undo());
    assert!(editor.undo());
    assert_eq!(editor.score().event(added), None);
    assert_eq!(
        editor.shown_score().event(id).unwrap().pitches.first(),
        Some(pitch(Step::D, 4, None))
    );
}

#[test]
fn copying_in_concert_pitch_pastes_by_sound_in_either_view() {
    let (mut editor, id) = editor(Instrument::ClarinetA, pitch(Step::F, 4, None));
    editor.set_concert_pitch(true);
    editor.select(id);
    let clip = editor.copy_selection().unwrap();
    editor.paste(&clip).unwrap();
    let pasted = editor.selection().unwrap();
    assert_ne!(pasted, id);
    assert_eq!(stored(&editor, pasted), stored(&editor, id));
    editor.set_concert_pitch(false);
    editor.paste(&clip).unwrap();
    let again = editor.selection().unwrap();
    let written = editor.score().written_key_at(again.part, again.measure);
    assert_eq!(
        stored(&editor, again).map(|p| p.in_key(written)),
        Some(pitch(Step::F, 4, Some(Accidental::Natural)))
    );
}

#[test]
fn range_warnings_read_the_same_note_in_both_views() {
    // A written D3 is below the B♭ clarinet's lowest written E3.
    let (mut editor, id) = editor(Instrument::ClarinetBb, pitch(Step::D, 3, None));
    let written = stored(&editor, id).unwrap();
    assert!(editor.score().pitch_range_warning(id, written).is_some());
    editor.set_concert_pitch(true);
    editor.select(id);
    let shown = editor.selected_pitch().unwrap();
    assert_ne!(shown, written);
    assert!(
        editor
            .shown_score()
            .pitch_range_warning(id, shown)
            .is_some()
    );
}
