use super::*;
use score_core::{
    Arpeggio, Articulation, BarForm, Connection, ConnectionKind, Duration, Dynamic, Ending, Event,
    Notation, TimeSignature, Tremolo, Voice,
};

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

fn orchestral_fixture() -> Score {
    let mut score = Score::new(
        "Orquesta".into(),
        &[
            Instrument::ClarinetBb,
            Instrument::HornF,
            Instrument::Violin,
        ],
        2,
    )
    .unwrap();
    let parts: Vec<_> = score.parts().iter().map(|part| part.id()).collect();
    for part in parts {
        for measure in 0..2 {
            score
                .insert_on(
                    part,
                    StaffId::Upper,
                    measure,
                    0,
                    Event {
                        pitches: Pitches::single(Pitch::new(
                            if measure == 0 { Step::C } else { Step::F },
                            4,
                        )),
                        duration: Duration::Whole,
                        notation: Notation::default(),
                    },
                )
                .unwrap();
        }
    }
    score
}

#[test]
fn literal_style_keeps_the_orchestral_plan_exact() {
    let score = orchestral_fixture();
    let previous = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    let explicit =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, true, Style::Literal).unwrap();
    assert_eq!(previous.messages, explicit.messages);
    assert_eq!(previous.spans, explicit.spans);
    assert_eq!(previous.duration_frames, explicit.duration_frames);
}

#[test]
fn metric_offset_follows_simple_and_compound_beats() {
    let mut score = Score::default();
    for (tick, expected) in [(0, 4), (48, 1), (96, 2), (24, -2)] {
        assert_eq!(
            interpretation::metric_offset(&score, tick),
            expected,
            "4/4 tick {tick}"
        );
    }
    score
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    assert_eq!(interpretation::metric_offset(&score, 48), 1);
    score
        .set_time_signature_at(
            0,
            TimeSignature {
                numerator: 6,
                denominator: 8,
            },
        )
        .unwrap();
    assert_eq!(interpretation::metric_offset(&score, 72), 1);
    assert_eq!(interpretation::metric_offset(&score, 24), -2);
    score.append_measure().unwrap();
    score
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    let next_bar = score.measure_start_tick(1);
    assert_eq!(interpretation::metric_offset(&score, next_bar), 4);
    assert_eq!(interpretation::metric_offset(&score, next_bar + 48), 1);
}

#[test]
fn natural_metric_downbeat_outweighs_the_second_quarter() {
    let mut score = Score::default();
    for index in 0..4 {
        score
            .insert(0, index, note(Step::C, Duration::Quarter))
            .unwrap();
    }
    let plan =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    let levels: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|message| match message.message {
            Message::NoteOn(note) => Some(note.level),
            _ => None,
        })
        .collect();
    assert_eq!(levels.len(), 4);
    assert!(levels[0] > levels[1]);
}

#[test]
fn phrase_contour_peaks_at_the_highest_written_attack() {
    let offsets = interpretation::phrase_offsets(&[60, 62, 64, 65, 67], None);
    assert_eq!(offsets[0], 0);
    assert!(offsets[4] > offsets[0]);
    assert_eq!(offsets[4], 1); // Peak +4, final release −3.

    let mut score = Score::default();
    for (bar, step) in [Step::C, Step::D, Step::E, Step::F, Step::G]
        .into_iter()
        .enumerate()
    {
        while score.measures().len() <= bar {
            score.append_measure().unwrap();
        }
        score.insert(bar, 0, note(step, Duration::Whole)).unwrap();
    }
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
    let contour: Vec<_> = natural
        .sounds
        .iter()
        .zip(&literal.sounds)
        .map(|(new, old)| {
            let (_, variation) = interpretation::human_variation(0, new.written_range.0);
            i16::from(new.level)
                - i16::from(old.level)
                - i16::from(interpretation::metric_offset(&score, new.written_range.0))
                - variation
        })
        .collect();
    assert_eq!(contour, [0, 1, 2, 3, 1]);
}

#[test]
fn phrase_final_release_needs_three_notes_and_respects_marcato() {
    assert_eq!(interpretation::phrase_offsets(&[60, 60], None), [0, 0]);
    assert_eq!(
        interpretation::phrase_offsets(&[60, 60, 60, 60], None),
        [0, 0, 0, -3]
    );
    assert_eq!(
        interpretation::phrase_offsets(&[60, 60, 60, 60], Some(Articulation::Marcato)),
        [0, 0, 0, 0]
    );
}

#[test]
fn phrase_contours_do_not_mix_two_voices_on_one_staff() {
    let mut score = Score::default();
    let part = score.parts()[0].id();
    for (index, step) in [Step::C, Step::D, Step::E].into_iter().enumerate() {
        score
            .insert(0, index, note(step, Duration::Quarter))
            .unwrap();
    }
    for index in 0..2 {
        score
            .insert_in_voice(
                part,
                StaffId::Upper,
                0,
                Voice::Two,
                index,
                note(Step::G, Duration::Half),
            )
            .unwrap();
    }
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
    let contour: Vec<_> = natural
        .sounds
        .iter()
        .zip(&literal.sounds)
        .map(|(new, old)| {
            let (_, variation) = interpretation::human_variation(0, new.written_range.0);
            i16::from(new.level)
                - i16::from(old.level)
                - i16::from(interpretation::metric_offset(&score, new.written_range.0))
                - variation
        })
        .collect();
    assert_eq!(contour, [0, 2, 1, 0, 0]);
}

#[test]
fn phrase_slur_connects_notes_across_written_rests() {
    let mut score = Score::default();
    let first = score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    score
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                ..note(Step::C, Duration::Quarter)
            },
        )
        .unwrap();
    let second = score
        .insert(0, 2, note(Step::D, Duration::Quarter))
        .unwrap();
    let third = score
        .insert(0, 3, note(Step::E, Duration::Quarter))
        .unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: third,
            kind: ConnectionKind::Slur,
        })
        .unwrap();
    let plan =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    assert_eq!(plan.sounds.len(), 3);
    assert_eq!(plan.sounds[1].written_range.0, 96);
    let (_, variation) = interpretation::human_variation(0, plan.sounds[1].written_range.0);
    let metric = interpretation::metric_offset(&score, plan.sounds[1].written_range.0);
    assert_eq!(
        i16::from(plan.sounds[1].level)
            - i16::from(plan.sounds[1].reference_level)
            - i16::from(metric)
            - variation,
        2
    );
    assert_eq!(score.event_tick(second), Some(96));
}

#[test]
fn phrase_breaks_at_a_repeat_jump() {
    let mut score = Score::new("Repeat".into(), &[Instrument::Violin], 2).unwrap();
    score.insert(0, 0, note(Step::C, Duration::Whole)).unwrap();
    score.insert(1, 0, note(Step::D, Duration::Whole)).unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, true, Style::Natural).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, true, Style::Literal).unwrap();
    assert_eq!(natural.sounds.len(), 4);
    for (new, old) in natural.sounds.iter().zip(&literal.sounds) {
        let (_, variation) = interpretation::human_variation(0, new.written_range.0);
        assert_eq!(i16::from(new.level) - i16::from(old.level), 4 + variation);
    }
}

#[test]
fn phrase_slur_does_not_cross_a_skipped_ending() {
    let mut score = Score::new("Endings".into(), &[Instrument::Violin], 3).unwrap();
    let first = score.insert(0, 0, note(Step::C, Duration::Half)).unwrap();
    score.insert(0, 1, note(Step::D, Duration::Half)).unwrap();
    score.insert(1, 0, note(Step::E, Duration::Whole)).unwrap();
    let last = score.insert(2, 0, note(Step::F, Duration::Whole)).unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: last,
            kind: ConnectionKind::Slur,
        })
        .unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    for (bar, number) in [(1, 1), (2, 2)] {
        score
            .set_ending(Ending {
                start: bar,
                end: bar,
                numbers: vec![number],
            })
            .unwrap();
    }
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, true, Style::Natural).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, true, Style::Literal).unwrap();
    assert_eq!(natural.sounds.len(), 6);
    assert_eq!(natural.sounds.last().unwrap().element, last);
    let new = natural.sounds.last().unwrap();
    let old = literal.sounds.last().unwrap();
    let (_, variation) = interpretation::human_variation(0, new.written_range.0);
    let metric = interpretation::metric_offset(&score, new.written_range.0);
    assert_eq!(
        i16::from(new.level) - i16::from(old.level) - variation - i16::from(metric),
        0
    );
}

#[test]
fn natural_style_is_deterministic_and_preserves_expression_reference() {
    let mut score = Score::default();
    let first = score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let last = score
        .insert(0, 1, note(Step::D, Duration::Quarter))
        .unwrap();
    score.set_dynamic(first, Some(Dynamic::P)).unwrap();
    score.set_dynamic(last, Some(Dynamic::F)).unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: last,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    let make =
        |style| PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, style).unwrap();
    let natural = make(Style::Natural);
    let second = make(Style::Natural);
    let literal = make(Style::Literal);
    assert_eq!(natural.messages, second.messages);
    assert_eq!(
        natural
            .sounds
            .iter()
            .map(|sound| (sound.start, sound.end, sound.level))
            .collect::<Vec<_>>(),
        second
            .sounds
            .iter()
            .map(|sound| (sound.start, sound.end, sound.level))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        natural
            .expression
            .iter()
            .map(|point| (point.frame, point.value))
            .collect::<Vec<_>>(),
        literal
            .expression
            .iter()
            .map(|point| (point.frame, point.value))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        natural
            .messages
            .iter()
            .filter(|message| matches!(message.message, Message::Expression(_)))
            .collect::<Vec<_>>(),
        literal
            .messages
            .iter()
            .filter(|message| matches!(message.message, Message::Expression(_)))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        natural
            .sounds
            .iter()
            .map(|sound| sound.reference_level)
            .collect::<Vec<_>>(),
        literal
            .sounds
            .iter()
            .map(|sound| sound.reference_level)
            .collect::<Vec<_>>()
    );
}

#[test]
fn human_delay_is_bounded_for_each_orchestral_attack() {
    let score = orchestral_fixture();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    for (old, new) in literal.sounds.iter().zip(&natural.sounds) {
        assert!((old.start..=old.start + 240).contains(&new.start));
        assert!(new.start <= natural.duration_frames);
        assert!((1..=127).contains(&new.level));
        assert_eq!(new.end - new.start, old.end - old.start);
    }
    assert_ne!(
        natural.sounds[0].start, natural.sounds[2].start,
        "different parts can have separate attack timing"
    );
}

#[test]
fn tremolo_repetitions_do_not_receive_fresh_metric_or_phrase_accents() {
    let mut score = Score::default();
    let mut first = note(Step::C, Duration::Quarter);
    first.notation.tremolo = Some(Tremolo::Two);
    score.insert(0, 0, first).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    assert_eq!(natural.sounds.len(), 4);
    let (_, variation) = interpretation::human_variation(0, 0);
    assert_eq!(
        i16::from(natural.sounds[0].level) - i16::from(literal.sounds[0].level),
        4 + variation
    );
    for (new, old) in natural.sounds.iter().zip(&literal.sounds).skip(1) {
        assert!(new.tremolo_repeat);
        assert_eq!(i16::from(new.level) - i16::from(old.level), variation);
    }
}

#[test]
fn chords_and_both_piano_hands_share_delay_without_flattening_arpeggios() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let upper = Event {
        pitches: Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::E, 4).unwrap(),
        ])
        .unwrap(),
        ..note(Step::C, Duration::Quarter)
    };
    let lower = Event {
        pitches: Pitches::new(&[
            Pitch::new(Step::C, 3).unwrap(),
            Pitch::new(Step::G, 3).unwrap(),
        ])
        .unwrap(),
        ..upper
    };
    let upper_id = score.insert(0, 0, upper).unwrap();
    score.insert(0, 1, upper).unwrap(); // An ordinary simultaneous chord after the roll.
    let lower_id = score.insert_on(piano, StaffId::Lower, 0, 0, lower).unwrap();
    score.set_arpeggio_both_hands(lower_id, true).unwrap();
    score.set_arpeggio(lower_id, Some(Arpeggio::Up)).unwrap();
    let literal =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Literal).unwrap();
    let natural =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    let shift = natural.sounds[0].start - literal.sounds[0].start;
    let lower_index = natural
        .sounds
        .iter()
        .position(|sound| sound.element == lower_id)
        .unwrap();
    assert_eq!(
        natural.sounds[lower_index].start - literal.sounds[lower_index].start,
        shift
    );
    assert_eq!(natural.sounds[0].element, upper_id);
    let attacks = |plan: &PlaybackPlan| -> Vec<_> {
        plan.messages
            .iter()
            .filter_map(|m| match m.message {
                Message::NoteOn(note) => Some((note.key, m.frame)),
                _ => None,
            })
            .collect()
    };
    let literal_attacks = attacks(&literal);
    let natural_attacks = attacks(&natural);
    assert_eq!(literal_attacks.len(), 6);
    for ((old_key, old_frame), (new_key, new_frame)) in literal_attacks
        .iter()
        .take(4)
        .zip(natural_attacks.iter().take(4))
    {
        assert_eq!(old_key, new_key);
        assert_eq!(*new_frame - *old_frame, shift);
    }
    assert!(
        literal_attacks
            .iter()
            .take(4)
            .copied()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|pair| pair[0].1 != pair[1].1)
    );
    assert_eq!(literal_attacks[4].1, literal_attacks[5].1);
    assert_eq!(natural_attacks[4].1, natural_attacks[5].1);
    assert_eq!(
        natural_attacks[4].1 - literal_attacks[4].1,
        natural_attacks[5].1 - literal_attacks[5].1
    );
}

#[test]
fn repeated_keys_release_before_the_next_natural_attack() {
    let mut score = Score::default();
    for index in 0..4 {
        let id = score
            .insert(0, index, note(Step::C, Duration::Quarter))
            .unwrap();
        score
            .set_articulation(id, Some(Articulation::Tenuto))
            .unwrap();
    }
    let plan =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    assert!(
        plan.sounds
            .windows(2)
            .all(|pair| pair[0].end <= pair[1].start && pair[0].end >= pair[0].start)
    );
    let attacks: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|m| matches!(m.message, Message::NoteOn(_)).then_some(m.frame))
        .collect();
    let releases: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|m| matches!(m.message, Message::NoteOff(60)).then_some(m.frame))
        .collect();
    assert_eq!(attacks.len(), 4);
    assert_eq!(releases.len(), 4);
    assert!(
        releases
            .iter()
            .zip(attacks.iter().skip(1))
            .all(|(off, on)| off <= on)
    );
}

#[test]
fn natural_velocity_saturates_at_both_midi_limits() {
    let mut score = Score::default();
    score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::default(),
                ..note(Step::C, Duration::Eighth)
            },
        )
        .unwrap();
    let quiet = score.insert(0, 1, note(Step::C, Duration::Eighth)).unwrap();
    let loud = score
        .insert(0, 2, note(Step::D, Duration::Quarter))
        .unwrap();
    score.set_dynamic(quiet, Some(Dynamic::Ppp)).unwrap();
    score.set_dynamic(loud, Some(Dynamic::Fff)).unwrap();
    score
        .set_articulation(loud, Some(Articulation::Marcato))
        .unwrap();
    let plan =
        PlaybackPlan::with_style(&score, Tempo::default(), 48_000, false, Style::Natural).unwrap();
    assert_eq!(
        interpretation::metric_offset(&score, plan.sounds[0].written_range.0),
        -2
    );
    assert!((1..=127).contains(&plan.sounds[0].level));
    assert_eq!(plan.sounds[1].level, 127);
}
