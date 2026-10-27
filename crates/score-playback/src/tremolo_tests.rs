use super::*;
use score_core::{Connection, ConnectionKind, Duration, Event, Notation, Tremolo};
fn event(duration: Duration, tremolo: Tremolo) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration,
        notation: Notation {
            tremolo: Some(tremolo),
            ..Default::default()
        },
    }
}
fn attacks(plan: &PlaybackPlan) -> Vec<&ScheduledMessage> {
    plan.messages
        .iter()
        .filter(|message| matches!(message.message, Message::NoteOn(_)))
        .collect()
}
#[test]
fn quarter_with_two_strokes_plays_four_sixteenths() {
    let mut score = Score::default();
    score
        .insert(0, 0, event(Duration::Quarter, Tremolo::Two))
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        attacks(&plan)
            .iter()
            .map(|attack| attack.frame)
            .collect::<Vec<_>>(),
        [0, 6000, 12000, 18000]
    );
    assert_eq!(plan.spans.len(), 1);
    for sound in &plan.sounds {
        assert!(sound.end < sound.written_end);
    }
}
#[test]
fn tied_half_notes_with_one_stroke_play_eighths_across_the_tie() {
    let mut score = Score::default();
    let a = score
        .insert(0, 0, event(Duration::Half, Tremolo::One))
        .unwrap();
    let b = score
        .insert(0, 1, event(Duration::Half, Tremolo::One))
        .unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(attacks(&plan).len(), 8);
    assert_eq!(attacks(&plan).last().unwrap().frame, 84_000);
}
#[test]
fn eighth_with_one_stroke_is_a_single_attack() {
    let mut score = Score::default();
    score
        .insert(0, 0, event(Duration::Eighth, Tremolo::One))
        .unwrap();
    assert_eq!(
        attacks(&PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap()).len(),
        1
    );
}
#[test]
fn fermata_stretches_the_repetitions() {
    let mut score = Score::default();
    let mut note = event(Duration::Quarter, Tremolo::Two);
    note.notation.fermata = true;
    score.insert(0, 0, note).unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        attacks(&plan)
            .iter()
            .map(|attack| attack.frame)
            .collect::<Vec<_>>(),
        [0, 12000, 24000, 36000]
    );
}
#[test]
fn only_the_first_tremolo_attack_keeps_its_accent() {
    for dynamic in [score_core::Dynamic::Mf, score_core::Dynamic::Fff] {
        let mut score = Score::default();
        let mut note = event(Duration::Quarter, Tremolo::Two);
        note.notation.articulation = Some(Articulation::Accent);
        note.notation.dynamic = Some(dynamic);
        score.insert(0, 0, note).unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        let levels: Vec<_> = attacks(&plan)
            .iter()
            .map(|message| match message.message {
                Message::NoteOn(note) => note.level,
                _ => unreachable!(),
            })
            .collect();
        let base = dynamic.velocity();
        assert_eq!(levels, [base.saturating_add(16).min(127), base, base, base]);
    }
}
#[test]
fn tremolos_repeat_with_form_and_chase_only_current_attack() {
    let mut score = Score::new("Repeated tremolo".into(), &[Instrument::Violin], 1).unwrap();
    score
        .insert(0, 0, event(Duration::Quarter, Tremolo::Two))
        .unwrap();
    score
        .set_bar_form(
            0,
            score_core::BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let plan = PlaybackPlan::with_repeats(&score, Tempo::default(), 48_000, true).unwrap();
    assert_eq!(attacks(&plan).len(), 8);
    assert_eq!(plan.held_notes_at(1000).into_iter().flatten().count(), 1);
    assert_eq!(plan.held_notes_at(5900).into_iter().flatten().count(), 0);
    assert_eq!(plan.held_notes_at(97_000).into_iter().flatten().count(), 1);
}

#[test]
fn maximum_ensemble_tremolos_keep_the_existing_simultaneous_voice_limit() {
    let mut score = Score::new(
        "Tremolo orchestra".into(),
        &[Instrument::Piano; score_core::MAX_PARTS],
        1,
    )
    .unwrap();
    let parts: Vec<_> = score.parts().iter().map(|part| part.id()).collect();
    for part in parts {
        for (staff, voice, octave) in [
            (StaffId::Upper, score_core::Voice::One, 0),
            (StaffId::Upper, score_core::Voice::Two, 2),
            (StaffId::Lower, score_core::Voice::One, 4),
            (StaffId::Lower, score_core::Voice::Two, 6),
        ] {
            let pitches: Vec<_> = [
                Step::C,
                Step::D,
                Step::E,
                Step::F,
                Step::G,
                Step::A,
                Step::B,
                Step::C,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, step)| Pitch::new(step, octave + (index / 7) as u8).unwrap())
            .collect();
            score
                .insert_in_voice(
                    part,
                    staff,
                    0,
                    voice,
                    0,
                    Event {
                        pitches: Pitches::new(&pitches).unwrap(),
                        ..event(Duration::Quarter, Tremolo::Three)
                    },
                )
                .unwrap();
        }
    }
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.held_notes_at(1000).into_iter().flatten().count(),
        MAX_SOUNDING_NOTES
    );
    assert_eq!(plan.held_notes_at(2900).into_iter().flatten().count(), 0);
    assert_eq!(
        plan.held_notes_at(4000).into_iter().flatten().count(),
        MAX_SOUNDING_NOTES
    );
}
#[test]
fn three_strokes_mark_the_first_repeated_and_last_attacks_and_two_strokes_do_not() {
    let roles = |strokes| {
        let mut score = Score::default();
        score
            .insert(0, 0, event(Duration::Quarter, strokes))
            .unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        attacks(&plan)
            .iter()
            .map(|attack| match attack.message {
                Message::NoteOn(note) => note.tremolo,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>()
    };
    let unmeasured = roles(Tremolo::Three);
    assert_eq!(unmeasured.len(), 8);
    assert_eq!(unmeasured[0], Some(TremoloAttack::First));
    assert!(
        unmeasured[1..7]
            .iter()
            .all(|role| *role == Some(TremoloAttack::Repeat))
    );
    assert_eq!(unmeasured[7], Some(TremoloAttack::Last));
    assert!(roles(Tremolo::Two).iter().all(Option::is_none));
}
