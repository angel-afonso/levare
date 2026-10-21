use super::*;
use crate::{
    Accidental, Duration, Event, KeyMode, KeySignature, Notation, Pitches, StaffId, Step,
    Transposition, Voice,
};

fn pitch(step: Step, octave: u8) -> Pitch {
    Pitch::new(step, octave).unwrap()
}

fn insert(score: &mut Score, pitches: &[Pitch], staff: StaffId, voice: Voice) -> ElementId {
    score
        .insert_in_voice(
            score.parts()[0].id(),
            staff,
            0,
            voice,
            0,
            Event {
                pitches: Pitches::new(pitches).unwrap(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap()
}

#[test]
fn catalog_covers_pitched_instruments_and_excludes_unpitched_percussion() {
    for instrument in Instrument::ALL {
        let range = instrument.sounding_range();
        assert_eq!(
            range.is_none(),
            instrument.percussion_key().is_some(),
            "{}",
            instrument.name()
        );
        if let Some(range) = range {
            assert!(range.lowest < range.highest);
            assert!(range.contains(i16::from(range.lowest)));
            assert!(range.contains(i16::from(range.highest)));
            assert!(!range.contains(i16::from(range.lowest) - 1));
            assert!(!range.contains(i16::from(range.highest) + 1));
        }
    }
    assert_eq!(
        Instrument::Piano.sounding_range().unwrap().to_string(),
        "A0–C8"
    );
    assert_eq!(
        Instrument::ClarinetBb.sounding_range().unwrap().to_string(),
        "D3–B♭6"
    );
}

#[test]
fn custom_ranges_are_per_part_validated_and_override_the_harmonic_envelope() {
    let mut score = Score::new(
        "Rangos propios".into(),
        &[Instrument::Violin, Instrument::Violin],
        1,
    )
    .unwrap();
    let first = score.parts()[0].id();
    let second = score.parts()[1].id();
    let low = pitch(Step::G, 3);
    let id = insert(&mut score, &[low], StaffId::Upper, Voice::One);
    let range = InstrumentRange {
        lowest: 60,
        highest: 84,
    };
    assert!(score.set_instrument_range(first, Some(range)).unwrap());
    assert!(!score.set_instrument_range(first, Some(range)).unwrap());
    assert!(score.pitch_range_warning(id, low).is_some());
    assert_eq!(score.part(first).unwrap().sounding_range(), Some(range));
    assert_eq!(score.part(second).unwrap().range_override(), None);
    score.set_harmonic(id, Some(Harmonic::Artificial)).unwrap();
    assert!(score.pitch_range_warning(id, low).is_none()); // G5 lies inside the custom bounds.
    let before = score.clone();
    for invalid in [
        InstrumentRange {
            lowest: 84,
            highest: 60,
        },
        InstrumentRange {
            lowest: 12,
            highest: 128,
        },
        InstrumentRange {
            lowest: 0,
            highest: 60,
        },
    ] {
        assert!(score.set_instrument_range(first, Some(invalid)).is_err());
        assert_eq!(score, before);
    }
    assert!(score.set_instrument_range(first, None).unwrap());
    assert_eq!(score.part(first).unwrap().range_override(), None);
    let percussion = score.add_part(Instrument::SnareDrum).unwrap();
    assert!(score.set_instrument_range(percussion, Some(range)).is_err());
}

#[test]
fn chord_tones_use_chromatic_boundaries_and_explicit_accidentals() {
    let mut score = Score::new("Violín".into(), &[Instrument::Violin], 1).unwrap();
    let flat = pitch(Step::G, 3).with_accidental(Some(Accidental::Flat));
    let low = pitch(Step::G, 3);
    let high = pitch(Step::A, 7);
    let sharp = high.with_accidental(Some(Accidental::Sharp));
    let id = insert(&mut score, &[flat, high], StaffId::Upper, Voice::One);
    assert!(score.pitch_range_warning(id, flat).is_some());
    assert!(score.pitch_range_warning(id, high).is_none());
    score.replace_pitch(id, 0, low).unwrap();
    assert!(score.pitch_range_warning(id, low).is_none());
    score.replace_pitch(id, 1, sharp).unwrap();
    assert!(score.pitch_range_warning(id, sharp).is_some());
    score.validate().unwrap(); // Warnings never invalidate the score.
}

#[test]
fn key_changes_and_naturals_update_warnings_without_editing_the_note() {
    let mut score = Score::new("Violín".into(), &[Instrument::Violin], 1).unwrap();
    let low = pitch(Step::G, 3);
    let id = insert(&mut score, &[low], StaffId::Upper, Voice::Two);
    assert!(score.pitch_range_warning(id, low).is_none());
    score.set_key_signature(KeySignature::new(-6, KeyMode::Major).unwrap()); // G-flat
    assert!(score.pitch_range_warning(id, low).is_some());
    let natural = low.with_accidental(Some(Accidental::Natural));
    score.replace_pitch(id, 0, natural).unwrap();
    assert!(score.pitch_range_warning(id, natural).is_none());
    score.set_key_signature(KeySignature::default());
    assert!(score.pitch_range_warning(id, natural).is_none());
}

#[test]
fn transposing_instruments_and_legacy_concert_parts_compare_sounding_pitch() {
    for (instrument, low, below) in [
        (
            Instrument::ClarinetBb,
            pitch(Step::E, 3),
            pitch(Step::E, 3).with_accidental(Some(Accidental::Flat)),
        ),
        (
            Instrument::Piccolo,
            pitch(Step::D, 4),
            pitch(Step::D, 4).with_accidental(Some(Accidental::Flat)),
        ),
        (
            Instrument::PiccoloTrumpet,
            pitch(Step::E, 3),
            pitch(Step::E, 3).with_accidental(Some(Accidental::Flat)),
        ),
        (
            Instrument::DoubleBass,
            pitch(Step::B, 1),
            pitch(Step::B, 1).with_accidental(Some(Accidental::Flat)),
        ),
        (
            Instrument::Guitar,
            pitch(Step::E, 3),
            pitch(Step::E, 3).with_accidental(Some(Accidental::Flat)),
        ),
        (
            Instrument::Glockenspiel,
            pitch(Step::G, 3),
            pitch(Step::G, 3).with_accidental(Some(Accidental::Flat)),
        ),
    ] {
        let mut score = Score::new("Transpositor".into(), &[instrument], 1).unwrap();
        let part = score.parts()[0].id();
        let id = insert(&mut score, &[low], StaffId::Upper, Voice::One);
        assert!(
            score.pitch_range_warning(id, low).is_none(),
            "{}",
            instrument.name()
        );
        score.replace_pitch(id, 0, below).unwrap();
        assert!(
            score.pitch_range_warning(id, below).is_some(),
            "{}",
            instrument.name()
        );
        if instrument == Instrument::ClarinetBb {
            score
                .set_part_transposition(part, Transposition::default())
                .unwrap();
            let real_low = pitch(Step::D, 3);
            score.replace_pitch(id, 0, real_low).unwrap();
            assert!(score.pitch_range_warning(id, real_low).is_none());
            let real_below = real_low.with_accidental(Some(Accidental::Flat));
            score.replace_pitch(id, 0, real_below).unwrap();
            assert!(score.pitch_range_warning(id, real_below).is_some());
        }
    }
}

#[test]
fn piccolo_trumpet_fourth_valve_and_upper_limit_use_sounding_pitch() {
    let mut score = Score::new("Piccolo".into(), &[Instrument::PiccoloTrumpet], 1).unwrap();
    let low = pitch(Step::E, 3);
    let high = pitch(Step::A, 5);
    let id = insert(&mut score, &[low, high], StaffId::Upper, Voice::One);
    assert!(score.pitch_range_warning(id, low).is_none()); // D4 sounding
    assert!(score.pitch_range_warning(id, high).is_none()); // G6 sounding
    let above = high.with_accidental(Some(Accidental::Sharp));
    score.replace_pitch(id, 1, above).unwrap();
    assert!(score.pitch_range_warning(id, above).is_some()); // A-flat6 sounding
}

#[test]
fn rests_percussion_and_nonexistent_tones_have_no_warning() {
    for instrument in [Instrument::Flute, Instrument::SnareDrum] {
        let mut score = Score::new("Sin aviso".into(), &[instrument], 1).unwrap();
        let low = pitch(Step::C, 1);
        let id = insert(&mut score, &[], StaffId::Upper, Voice::One);
        assert!(score.pitch_range_warning(id, low).is_none());
        score.set_pitches(id, Pitches::single(Some(low))).unwrap();
        assert_eq!(
            score.pitch_range_warning(id, low).is_some(),
            instrument == Instrument::Flute
        );
        assert!(score.pitch_range_warning(id, pitch(Step::D, 1)).is_none());
        assert!(
            score
                .pitch_range_warning(ElementId { measure: 10, ..id }, low)
                .is_none()
        );
    }
}

#[test]
fn both_piano_hands_share_the_same_range() {
    let mut score = Score::new("Piano".into(), &[Instrument::Piano], 1).unwrap();
    let low = pitch(Step::A, 0);
    let id = insert(&mut score, &[low], StaffId::Lower, Voice::Two);
    assert!(score.pitch_range_warning(id, low).is_none());
    let below = pitch(Step::A, 0).with_accidental(Some(Accidental::Flat));
    score.replace_pitch(id, 0, below).unwrap();
    assert!(score.pitch_range_warning(id, below).is_some());
    let high = pitch(Step::C, 8);
    let id = insert(&mut score, &[high], StaffId::Upper, Voice::One);
    assert!(score.pitch_range_warning(id, high).is_none());
    let above = high.with_accidental(Some(Accidental::Sharp));
    score.replace_pitch(id, 0, above).unwrap();
    assert!(score.pitch_range_warning(id, above).is_some());
}

#[test]
fn harmonics_use_their_sounding_register_and_ignore_the_touch_diamond() {
    let mut score = Score::new("Armónicos".into(), &[Instrument::Violin], 1).unwrap();
    let high = pitch(Step::D, 8);
    let id = insert(&mut score, &[high], StaffId::Upper, Voice::One);
    assert!(score.pitch_range_warning(id, high).is_some());
    score.set_harmonic(id, Some(Harmonic::Natural)).unwrap();
    assert!(score.pitch_range_warning(id, high).is_none());
    let low = pitch(Step::G, 3);
    score.replace_pitch(id, 0, low).unwrap();
    assert!(score.pitch_range_warning(id, low).is_some()); // Not a sounding harmonic.
    score.set_harmonic(id, Some(Harmonic::Artificial)).unwrap();
    assert!(score.pitch_range_warning(id, low).is_none()); // Sounds G5.
    let touch = score
        .event(id)
        .unwrap()
        .harmonic_touch(score.written_key_at(id.part, 0))
        .unwrap()
        .0;
    assert!(score.pitch_range_warning(id, touch).is_none());
    let below = low.with_accidental(Some(Accidental::Flat));
    score.replace_pitch(id, 0, below).unwrap();
    assert!(score.pitch_range_warning(id, below).is_some());
}
