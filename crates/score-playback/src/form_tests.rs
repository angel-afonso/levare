use super::*;
use score_core::{BarForm, Connection, ConnectionKind, Duration, Ending, Event, Notation};

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

fn score(bars: usize) -> Score {
    Score::new("Form".into(), &[Instrument::Violin], bars).unwrap()
}

#[test]
fn linear_score_frames_are_unchanged() {
    let mut score = score(4);
    let a = score
        .insert(0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(
            0,
            1,
            Event {
                notation: Notation {
                    fermata: true,
                    ..Default::default()
                },
                ..note(Step::D, Duration::Quarter)
            },
        )
        .unwrap();
    let c = score
        .insert(1, 0, note(Step::E, Duration::Quarter))
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        [a, b, c].map(|id| plan.frame_for_element(id)),
        [Some(0), Some(24_000), Some(120_000)]
    );
    assert_eq!(
        plan.duration_frames, 144_000,
        "legacy plans stop after the last event, excluding empty trailing bars"
    );
}

fn repeated_score() -> Score {
    let mut score = score(2);
    for (bar, step) in [(0, Step::C), (1, Step::D)] {
        score.insert(bar, 0, note(step, Duration::Whole)).unwrap();
    }
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    score
}

#[test]
fn repeat_plays_the_bars_twice() {
    let score = repeated_score();
    let linear = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, false).unwrap();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(
        plan.spans
            .iter()
            .map(|span| span.element.measure)
            .collect::<Vec<_>>(),
        [0, 1, 0, 1]
    );
    assert_eq!(
        plan.spans.iter().map(|span| span.start).collect::<Vec<_>>(),
        [0, 96_000, 192_000, 288_000]
    );
    assert_eq!(plan.duration_frames, linear.duration_frames * 2);
    assert_eq!(
        plan.timeline.frame_at(score.total_ticks(), 48_000),
        plan.duration_frames
    );
    assert_eq!(
        plan.timeline.instances()[2],
        Instance {
            bar: 0,
            written: 0,
            performed: 384,
            length: 192
        }
    );
}

#[test]
fn second_ending_plays_after_the_jump() {
    let mut score = score(3);
    for bar in 0..3 {
        score
            .insert(bar, 0, note(Step::C, Duration::Whole))
            .unwrap();
    }
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
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(
        plan.spans
            .iter()
            .map(|span| span.element.measure)
            .collect::<Vec<_>>(),
        [0, 1, 0, 2]
    );
}

#[test]
fn tick_at_returns_written_time_on_the_second_pass() {
    let score = repeated_score();
    for rate in [44_100, 48_000, 96_000] {
        let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), rate, true).unwrap();
        let frame = plan.spans[2].start;
        assert_eq!(plan.timeline.tick_at(frame, rate), 0.);
        assert_eq!(plan.element_at(frame), Some(plan.spans[0].element));
        assert!((plan.timeline.tick_at(frame + u64::from(rate) / 4, rate) - 24.).abs() < 0.003);
        assert_eq!(plan.frame_for_element(plan.spans[2].element), Some(0));
        assert_eq!(plan.timeline.frame_at(192, rate), plan.spans[1].start);
    }
}

#[test]
fn tempo_resets_when_jumping_back() {
    let mut score = repeated_score();
    score
        .set_tempo_at(
            0,
            score_core::TempoMark {
                bpm: 60,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_tempo_at(
            1,
            score_core::TempoMark {
                bpm: 120,
                ..Default::default()
            },
        )
        .unwrap();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(
        plan.spans
            .iter()
            .map(|span| span.end - span.start)
            .collect::<Vec<_>>(),
        [192_000, 96_000, 192_000, 96_000]
    );
}

#[test]
fn tie_into_a_skipped_bar_reattacks() {
    let mut score = score(3);
    let ids: Vec<_> = (0..3)
        .map(|bar| {
            score
                .insert(bar, 0, note(Step::C, Duration::Whole))
                .unwrap()
        })
        .collect();
    score
        .add_connection(Connection {
            start: ids[1],
            end: ids[2],
            kind: ConnectionKind::Tie,
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
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    let end = plan
        .spans
        .iter()
        .find(|span| span.element == ids[2])
        .unwrap();
    assert!(
        plan.messages
            .iter()
            .any(|message| message.frame == end.start
                && matches!(message.message, Message::NoteOn(_)))
    );
    assert_eq!(plan.sounds.len(), 4);
}

#[test]
fn follow_false_is_the_written_order() {
    let score = repeated_score();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, false).unwrap();
    assert_eq!(
        plan.spans
            .iter()
            .map(|span| span.element.measure)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        plan.messages,
        PlaybackPlan::new(&score, Tempo::default(), 48_000)
            .unwrap()
            .messages
    );
    assert!(
        plan.timeline
            .instances()
            .iter()
            .all(|instance| instance.written == instance.performed)
    );
}

#[test]
fn repeated_hairpin_restores_each_attack_and_continuous_expression() {
    use score_core::Dynamic;
    let mut score = score(2);
    let a = score
        .insert(
            0,
            0,
            Event {
                notation: Notation {
                    fermata: true,
                    ..Default::default()
                },
                ..note(Step::C, Duration::Whole)
            },
        )
        .unwrap();
    let b = score.insert(1, 0, note(Step::D, Duration::Whole)).unwrap();
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    score.set_dynamic(b, Some(Dynamic::F)).unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
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
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    let repeat = plan.spans[2].start;
    assert_eq!(repeat, 288_000);
    assert_eq!(plan.sounds[0].level, plan.sounds[2].level);
    assert_eq!(plan.sounds[1].level, plan.sounds[3].level);
    for frame in [0, 24_000, 96_000, 150_000, 191_999, 192_000, 240_000] {
        assert_eq!(
            plan.expressions_at(frame),
            plan.expressions_at(repeat + frame),
            "frame {frame}"
        );
    }
    assert!(plan.expressions_at(96_000)[0] > plan.expressions_at(0)[0]);
    assert!(
        plan.expression
            .windows(2)
            .all(|points| points[0].frame <= points[1].frame)
    );
}

#[test]
fn a_repeated_shared_arpeggio_uses_only_its_own_occurrence() {
    let mut score = Score::new("Roll".into(), &[Instrument::Piano], 3).unwrap();
    let first = score.insert(0, 0, note(Step::C, Duration::Whole)).unwrap();
    let upper = score.insert(1, 0, note(Step::C, Duration::Whole)).unwrap();
    let lower = score
        .insert_on(
            first.part,
            StaffId::Lower,
            1,
            0,
            note(Step::G, Duration::Whole),
        )
        .unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: upper,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    score.set_arpeggio_both_hands(lower, true).unwrap();
    score
        .set_arpeggio(lower, Some(score_core::Arpeggio::Up))
        .unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    let attacks: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|message| match message.message {
            Message::NoteOn(note) if note.key == 67 => Some(message.frame),
            _ => None,
        })
        .collect();
    let visits: Vec<_> = plan
        .spans
        .iter()
        .filter(|span| span.element == lower)
        .map(|span| span.start)
        .collect();
    assert_eq!(attacks.len(), 2);
    assert_eq!(
        attacks[0], visits[0],
        "the first upper chord is tied in and adds no new roll attack"
    );
    assert!(
        attacks[1] > visits[1],
        "the second upper chord reattacks below the lower hand's crossed note"
    );
}

#[test]
fn jump_and_fine_keep_performed_duration_and_written_cursor() {
    let mut score = score(4);
    for bar in 0..4 {
        score
            .insert(bar, 0, note(Step::C, Duration::Whole))
            .unwrap();
    }
    score
        .set_bar_form(
            1,
            BarForm {
                fine: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            3,
            BarForm {
                jump: Some(score_core::Jump::DaCapoAlFine),
                ..Default::default()
            },
        )
        .unwrap();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(
        plan.spans
            .iter()
            .map(|span| span.element.measure)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 0, 1]
    );
    assert_eq!(plan.duration_frames, 6 * 96_000);
    assert_eq!(plan.timeline.tick_at(plan.spans[4].start, 48_000), 0.);
    assert_eq!(plan.timeline.tick_at(plan.duration_frames, 48_000), 384.);
}

#[test]
fn outgoing_slur_does_not_connect_across_a_repeat_jump() {
    let mut score = score(3);
    let ids: Vec<_> = [Step::C, Step::D, Step::E]
        .into_iter()
        .enumerate()
        .map(|(bar, step)| score.insert(bar, 0, note(step, Duration::Whole)).unwrap())
        .collect();
    score
        .add_connection(Connection {
            start: ids[1],
            end: ids[2],
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
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(
        plan.sounds[1].end, plan.spans[2].start,
        "the backward jump must release the previous slurred note"
    );
    assert!(
        plan.sounds[3].end > plan.spans[4].start,
        "the final pass still follows the written slur"
    );
}

#[test]
fn an_ending_that_skips_the_entire_score_has_an_empty_timeline() {
    let mut score = score(2);
    score
        .set_ending(Ending {
            start: 0,
            end: 1,
            numbers: vec![2],
        })
        .unwrap();
    assert!(score.play_order(true).is_empty());
    let timeline = Timeline::with_repeats(&score, Tempo::default(), true);
    assert!(timeline.instances().is_empty());
    assert_eq!(timeline.total_frames(48_000), 0);
    assert_eq!(timeline.frame_at(48, 48_000), 0);
    assert_eq!(timeline.tick_at(48_000, 48_000), 0.);
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(plan.duration_frames, 0);
    assert!(plan.messages.is_empty());
    assert!(plan.spans.is_empty());
}

#[test]
fn scheduled_repeat_boundaries_map_back_at_nonintegral_sample_frames() {
    let mut score = repeated_score();
    score
        .set_tempo_at(
            0,
            score_core::TempoMark {
                bpm: 137,
                ..Default::default()
            },
        )
        .unwrap();
    for rate in [44_100, 48_000, 96_000] {
        let timeline = Timeline::with_repeats(&score, Tempo::default(), true);
        for (index, instance) in timeline.instances().iter().enumerate() {
            let frame = timeline.performed_frame(index, 0, rate);
            assert!(
                (timeline.tick_at(frame, rate) - instance.written as f64).abs() < 0.003,
                "instance {index} at rate {rate}"
            );
        }
    }
}
