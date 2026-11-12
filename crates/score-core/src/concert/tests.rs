use super::*;

fn pitch(step: Step, octave: u8, accidental: Option<Accidental>) -> Pitch {
    Pitch::new(step, octave)
        .unwrap()
        .with_accidental(accidental)
}

/// MIDI numbers, whatever the spelling.
fn sound(pitches: Pitches) -> Vec<i16> {
    pitches
        .iter()
        .map(|p| {
            (i16::from(p.octave()) + 1) * 12
                + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
                + i16::from(p.alteration())
        })
        .collect()
}

fn note(pitch: Pitch) -> Event {
    Event {
        pitches: Pitches::single(Some(pitch)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

/// One part in concert D major with one written note in its first bar.
fn one_note(instrument: Instrument, written: Pitch) -> (Score, ElementId) {
    let mut score = Score::new("Concierto".into(), &[instrument], 2).unwrap();
    score.set_key_signature(KeySignature::new(2, KeyMode::Major).unwrap());
    let part = score.parts()[0].id();
    let id = score
        .insert_on(part, StaffId::Upper, 0, 0, note(written))
        .unwrap();
    (score, id)
}

#[test]
fn transposing_parts_show_their_sound_and_the_key_of_the_piece() {
    // Written key and note, then the concert note: every one sounds D4
    // except the octave instruments, which keep their written octave.
    for (instrument, written_fifths, written, shown) in [
        (
            Instrument::ClarinetBb,
            4,
            pitch(Step::E, 4, None),
            pitch(Step::D, 4, None),
        ),
        (
            Instrument::HornF,
            3,
            pitch(Step::A, 4, None),
            pitch(Step::D, 4, None),
        ),
        (
            Instrument::ClarinetA,
            -1,
            pitch(Step::F, 4, None),
            pitch(Step::D, 4, None),
        ),
        (
            Instrument::AltoSax,
            5,
            pitch(Step::B, 4, None),
            pitch(Step::D, 4, None),
        ),
        // A major ninth: the tenor saxophone keeps its octave.
        (
            Instrument::TenorSax,
            4,
            pitch(Step::E, 5, None),
            pitch(Step::D, 5, None),
        ),
        // Octave instruments look the same in both views.
        (
            Instrument::Piccolo,
            2,
            pitch(Step::D, 5, None),
            pitch(Step::D, 5, None),
        ),
        (
            Instrument::DoubleBass,
            2,
            pitch(Step::D, 3, None),
            pitch(Step::D, 3, None),
        ),
    ] {
        let name = instrument.name();
        let (score, id) = one_note(instrument, written);
        assert_eq!(
            score.written_key_at(id.part, 0).fifths(),
            written_fifths,
            "{name}"
        );
        let view = score.in_concert_pitch();
        assert_eq!(view.written_key_at(id.part, 0).fifths(), 2, "{name}");
        assert_eq!(
            view.event(id).unwrap().pitches.first(),
            Some(shown),
            "{name}"
        );
        assert_eq!(
            view.part(id.part).unwrap().transposition(),
            instrument.standard_transposition().octaves(),
            "{name}"
        );
        // The same sound, and back to the same written notes.
        assert_eq!(
            sound(view.sounding_pitches(id).unwrap()),
            sound(score.sounding_pitches(id).unwrap()),
            "{name}"
        );
        view.validate().unwrap();
        assert_eq!(view.from_concert_pitch(&view, &score), score, "{name}");
    }
}

#[test]
fn concert_spelling_keeps_the_letter_with_double_signs() {
    let mut score = Score::new("Signos".into(), &[Instrument::ClarinetBb], 1).unwrap();
    let part = score.parts()[0].id();
    // In concert C the clarinet reads D major: a written C♭ sounds B𝄫, and
    // a written F♮ against the key's F♯ sounds E♭.
    let flat = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            note(pitch(Step::C, 5, Some(Accidental::Flat))),
        )
        .unwrap();
    let natural = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            1,
            note(pitch(Step::F, 4, Some(Accidental::Natural))),
        )
        .unwrap();
    let view = score.in_concert_pitch();
    assert_eq!(view.written_key_at(part, 0).fifths(), 0);
    assert_eq!(
        view.event(flat).unwrap().pitches.first(),
        Some(pitch(Step::B, 4, Some(Accidental::DoubleFlat)))
    );
    assert_eq!(
        view.event(natural).unwrap().pitches.first(),
        Some(pitch(Step::E, 4, Some(Accidental::Flat)))
    );
    assert_eq!(view.from_concert_pitch(&view, &score), score);
}

#[test]
fn grace_notes_and_chord_symbols_follow_the_view() {
    let (mut score, id) = one_note(Instrument::ClarinetBb, pitch(Step::E, 4, None));
    score.add_grace_note(id).unwrap();
    score
        .set_chord(id, Some(ChordSymbol::parse("E").unwrap()))
        .unwrap();
    let view = score.in_concert_pitch();
    let event = view.event(id).unwrap();
    assert_eq!(event.notation.chord.unwrap().label(), "D");
    assert_eq!(
        event.notation.grace.unwrap().note(0).map(|p| p.step),
        Some(Step::D)
    );
    assert_eq!(view.from_concert_pitch(&view, &score), score);
}

#[test]
fn notes_written_in_the_concert_view_are_stored_at_written_pitch() {
    let (score, id) = one_note(Instrument::ClarinetBb, pitch(Step::E, 4, None));
    let shown = score.in_concert_pitch();
    let mut edited = shown.clone();
    // The concert B4 that follows D major is the clarinet's C♯5, which
    // follows its E major; the untouched note keeps its written E.
    let added = edited
        .insert_on(id.part, StaffId::Upper, 0, 1, note(pitch(Step::B, 4, None)))
        .unwrap();
    edited
        .set_chord(added, Some(ChordSymbol::parse("Bm").unwrap()))
        .unwrap();
    let written = edited.from_concert_pitch(&shown, &score);
    assert_eq!(
        written.event(added).unwrap().pitches.first(),
        Some(pitch(Step::C, 5, None))
    );
    assert_eq!(
        written
            .event(added)
            .unwrap()
            .notation
            .chord
            .unwrap()
            .label(),
        "C♯m"
    );
    assert_eq!(written.event(id), score.event(id));
    assert_eq!(
        written.part(id.part).unwrap().transposition(),
        Instrument::ClarinetBb.standard_transposition()
    );
    assert_eq!(
        written.sounding_pitches(added).unwrap().first(),
        Some(pitch(Step::B, 4, Some(Accidental::Natural)))
    );
    written.validate().unwrap();
}

#[test]
fn concert_parts_and_untransposed_scores_are_left_alone() {
    let mut score = Score::new(
        "Cuerdas".into(),
        &[Instrument::Violin, Instrument::Flute, Instrument::SnareDrum],
        1,
    )
    .unwrap();
    score.set_key_signature(KeySignature::new(-3, KeyMode::Minor).unwrap());
    assert_eq!(score.in_concert_pitch(), score);
}
