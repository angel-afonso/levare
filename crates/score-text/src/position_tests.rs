use super::*;
use score_core::{
    Duration, Event, Instrument, Pitch, Pitches, Score, StaffId, Step, TimeSignature, Voice,
};
use score_edit::{Command, Editor};

fn time(numerator: u8, denominator: u8) -> TimeSignature {
    TimeSignature {
        numerator,
        denominator,
    }
}

fn score_in(time_signature: TimeSignature, measures: usize) -> Score {
    let mut score = Score::new("".into(), &[Instrument::Violin], measures).unwrap();
    score.set_time_signature_at(0, time_signature).unwrap();
    score
}

fn quarter() -> Event {
    Event {
        notation: Default::default(),
        pitches: Pitches::single(Some(Pitch::new(Step::C, 4).unwrap())),
        duration: Duration::Quarter,
    }
}

#[test]
fn resolves_part_by_number_and_name() {
    let score = Score::new(
        "".into(),
        &[Instrument::Flute, Instrument::Viola, Instrument::Cello],
        1,
    )
    .unwrap();
    let id = |index: usize| score.parts()[index].id();
    assert_eq!(resolve_part(&score, "2"), Ok(id(1)));
    assert_eq!(resolve_part(&score, " 3 "), Ok(id(2)));
    assert_eq!(resolve_part(&score, "viola"), Ok(id(1)));
    assert_eq!(resolve_part(&score, "VIOLAS"), Ok(id(1)));
    // Prefix, and accents are optional.
    assert_eq!(resolve_part(&score, "viola"), Ok(id(1)));
    assert!(resolve_part(&score, "vio").is_err());
    assert_eq!(resolve_part(&score, "FLAU"), Ok(id(0)));
    assert_eq!(resolve_part(&score, "violoncellos"), Ok(id(2)));
    let score = Score::new("".into(), &[Instrument::Violin, Instrument::Trombone], 1).unwrap();
    assert_eq!(resolve_part(&score, "violín"), Ok(score.parts()[0].id()));
    assert_eq!(resolve_part(&score, "VIOLIN"), Ok(score.parts()[0].id()));
    assert_eq!(resolve_part(&score, "trombones"), Ok(score.parts()[1].id()));
}

#[test]
fn unknown_part_lists_the_available_ones() {
    let score = Score::new("".into(), &[Instrument::Flute, Instrument::Viola], 1).unwrap();
    for query in ["9", "0", "arpa", ""] {
        let error = resolve_part(&score, query).unwrap_err();
        assert!(error.contains("1 Flauta"), "{query}: {error}");
        assert!(error.contains("2 Viola"), "{query}: {error}");
    }
}

#[test]
fn ambiguous_part_lists_candidates() {
    let score = Score::new(
        "".into(),
        &[Instrument::Violin, Instrument::Violin, Instrument::Viola],
        1,
    )
    .unwrap();
    for query in ["violín", "violines"] {
        let error = resolve_part(&score, query).unwrap_err();
        assert!(error.contains("1 Violín I"), "{error}");
        assert!(error.contains("2 Violín II"), "{error}");
        assert!(!error.contains("Viola"), "{error}");
        // The user chooses, not the agent.
        assert!(error.contains("Pregunta al usuario cuál quiere"), "{error}");
        assert!(!error.contains("Indica el número"), "{error}");
    }
    // The exact name wins over longer names sharing the prefix.
    assert_eq!(resolve_part(&score, "violín II"), Ok(score.parts()[1].id()));
    assert_eq!(resolve_part(&score, "viola"), Ok(score.parts()[2].id()));
}

#[test]
fn beat_ticks_in_simple_and_compound() {
    let simple = score_in(time(4, 4), 1);
    assert_eq!(beat_tick(&simple, 0, 1.), Ok(0));
    assert_eq!(beat_tick(&simple, 0, 3.), Ok(96));
    assert_eq!(beat_tick(&simple, 0, 2.5), Ok(72));
    let compound = score_in(time(6, 8), 1);
    assert_eq!(beat_tick(&compound, 0, 4.), Ok(72));
    assert_eq!(beat_tick(&compound, 0, 6.), Ok(120));
}

#[test]
fn beat_follows_the_time_signature_of_its_own_bar() {
    let mut score = score_in(time(4, 4), 3);
    score.set_time_signature_at(1, time(6, 8)).unwrap();
    assert_eq!(beat_tick(&score, 0, 4.), Ok(144));
    assert_eq!(beat_tick(&score, 1, 4.), Ok(72));
}

#[test]
fn beat_outside_measure_fails() {
    let score = score_in(time(3, 4), 2);
    let error = beat_tick(&score, 0, 4.).unwrap_err();
    assert!(error.contains("3/4"), "{error}");
    assert!(error.contains("compás 1"), "{error}");
    for beat in [0., -1., 0.5, f32::NAN, f32::INFINITY] {
        assert!(beat_tick(&score, 0, beat).is_err(), "{beat}");
    }
    assert!(beat_tick(&score, 2, 1.).is_err());
}

#[test]
fn beat_must_land_on_a_tick() {
    let score = score_in(time(4, 4), 1);
    let error = beat_tick(&score, 0, 1.01).unwrap_err();
    assert!(error.contains("1.01"), "{error}");
    // A third of a beat is exactly 16 ticks.
    assert!(beat_tick(&score, 0, 1.0 + 1. / 3.).is_ok());
}

#[test]
fn event_at_reports_onsets() {
    let score = score_in(time(4, 4), 1);
    let mut editor = Editor::new(score).unwrap();
    for _ in 0..4 {
        editor.execute(Command::Write(quarter())).unwrap();
    }
    let part = editor.score().parts()[0].id();
    let found = event_at(editor.score(), part, StaffId::Upper, Voice::One, 0, 96).unwrap();
    assert_eq!(
        (found.part, found.staff, found.measure),
        (part, StaffId::Upper, 0)
    );
    let expected = editor.score().measures()[0]
        .voice_id_at(Voice::One, 2)
        .unwrap();
    assert_eq!(found.event, expected);
    let error = event_at(editor.score(), part, StaffId::Upper, Voice::One, 0, 24).unwrap_err();
    assert!(error.contains("1, 2, 3, 4"), "{error}");
    let error = event_at(editor.score(), part, StaffId::Upper, Voice::Two, 0, 0).unwrap_err();
    assert!(error.contains("voz 2"), "{error}");
    assert!(event_at(editor.score(), part, StaffId::Lower, Voice::One, 0, 0).is_err());
    assert!(event_at(editor.score(), part, StaffId::Upper, Voice::One, 5, 0).is_err());
}

#[test]
fn pickup_bar_numbering() {
    // MusicXML import models a pickup as a short first bar with its own signature.
    let mut score = score_in(time(1, 4), 4);
    score.set_time_signature_at(1, time(4, 4)).unwrap();
    // Levare prints `index + 1` on every bar (app-gpui/src/paint.rs), pickup included.
    assert_eq!(bar_index(&score, 1), Ok(0));
    assert_eq!(bar_index(&score, 2), Ok(1));
    assert_eq!(bar_index(&score, 4), Ok(3));
    assert_eq!(beat_tick(&score, 0, 1.), Ok(0));
    assert!(beat_tick(&score, 0, 2.).is_err());
    assert_eq!(beat_tick(&score, 1, 4.), Ok(144));
    let error = bar_index(&score, 5).unwrap_err();
    assert!(error.contains("4 compases"), "{error}");
    assert!(bar_index(&score, 0).is_err());
}

#[test]
fn fractional_onsets_round_trip() {
    let mut editor = Editor::new(score_in(time(4, 4), 1)).unwrap();
    let event = |duration| Event {
        duration,
        ..quarter()
    };
    let (mut ticks, mut at) = (Vec::new(), 0);
    for (duration, count) in [
        (Duration::Eighth.with_triplet(true), 3),
        (Duration::Sixteenth.with_triplet(true), 6),
        (Duration::ThirtySecond, 8),
        (Duration::Quarter, 1),
    ] {
        for _ in 0..count {
            ticks.push(at);
            at += duration.ticks();
            editor.execute(Command::Write(event(duration))).unwrap();
        }
    }
    let part = editor.score().parts()[0].id();
    let error = event_at(editor.score(), part, StaffId::Upper, Voice::One, 0, 1).unwrap_err();
    let list = error.rsplit_once("Empiezan en ").unwrap().1;
    let labels: Vec<f32> = list
        .trim_end_matches('.')
        .split(", ")
        .map(|label| label.parse().unwrap())
        .collect();
    assert_eq!(labels.len(), ticks.len(), "{error}");
    for (label, tick) in labels.into_iter().zip(ticks) {
        assert_eq!(beat_tick(editor.score(), 0, label), Ok(tick), "{label}");
    }
}

#[test]
fn beat_between_two_ticks_is_rejected() {
    let score = score_in(time(4, 4), 1);
    assert!(beat_tick(&score, 0, 1.25).is_ok());
    assert!(beat_tick(&score, 0, 1. + 0.5 / 48.).is_err());
    assert!(beat_tick(&score, 0, 1.01).is_err());
}
