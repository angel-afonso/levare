use super::*;
use score_core::{Connection, ConnectionKind, Duration, Dynamic, Event, Notation, Voice};
fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}
#[test]
fn both_voices_play_together_and_chase_each_held_note() {
    let mut s = Score::default();
    let p = s.parts()[0].id();
    let one = s.insert(0, 0, note(Step::C, Duration::Whole)).unwrap();
    let a = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::E, Duration::Half),
        )
        .unwrap();
    let b = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            1,
            note(Step::G, Duration::Half),
        )
        .unwrap();
    let plan = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.tracks
            .iter()
            .filter(|t| t.staff == StaffId::Upper)
            .count(),
        2
    );
    let held = plan
        .held_notes_at(12_000)
        .into_iter()
        .flatten()
        .map(|n| n.note.key)
        .collect::<Vec<_>>();
    assert_eq!(held, [60, 64]);
    let active = plan
        .elements_at(12_000)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert!(active.contains(&one) && active.contains(&a));
    let later = plan
        .elements_at(60_000)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert!(later.contains(&one) && later.contains(&b) && !later.contains(&a));
    assert_eq!(plan.duration_frames, 96_000);
}
#[test]
fn dividing_and_reuniting_preserve_shared_dynamics_techniques_hairpins_and_timing() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let mut first = note(Step::C, Duration::Half);
    first.notation.dynamic = Some(Dynamic::P);
    first.notation.technique = Some(PlayingTechnique::Pizzicato);
    let a = s.insert_in(p, 0, 0, first).unwrap();
    let mut last = note(Step::D, Duration::Half);
    last.notation.dynamic = Some(Dynamic::F);
    let b = s.insert_in(p, 0, 1, last).unwrap();
    let lower = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::E, Duration::Whole),
        )
        .unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let before = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    let level = score_core::DynamicEnvelope::new(&s, p, StaffId::Upper)
        .unwrap()
        .level_at(48);
    s.set_part_divisi(p, true).unwrap();
    let moved = ElementId {
        staff: StaffId::Lower,
        ..lower
    };
    assert_eq!(s.technique_at(moved), Some(PlayingTechnique::Pizzicato));
    assert_eq!(
        score_core::DynamicEnvelope::new(&s, p, StaffId::Lower)
            .unwrap()
            .level_at(48),
        level
    );
    let after = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(before.messages, after.messages);
    s.set_part_divisi(p, false).unwrap();
    assert_eq!(
        before.messages,
        PlaybackPlan::new(&s, Tempo::default(), 48_000)
            .unwrap()
            .messages
    );
}

#[test]
fn maximum_two_voice_ensemble_tracks_and_chases_all_448_tones_without_truncation() {
    let mut s = Score::default();
    for _ in 1..score_core::MAX_PARTS {
        s.add_part(Instrument::Piano).unwrap();
    }
    let parts: Vec<_> = s.parts().iter().map(|p| p.id()).collect();
    for part in parts {
        for (staff, voice, octave) in [
            (StaffId::Upper, Voice::One, 0),
            (StaffId::Upper, Voice::Two, 2),
            (StaffId::Lower, Voice::One, 4),
            (StaffId::Lower, Voice::Two, 6),
        ] {
            let steps = [
                Step::C,
                Step::D,
                Step::E,
                Step::F,
                Step::G,
                Step::A,
                Step::B,
                Step::C,
            ];
            let pitches: Vec<_> = steps
                .into_iter()
                .enumerate()
                .map(|(i, step)| Pitch::new(step, octave + (i / 7) as u8).unwrap())
                .collect();
            s.insert_in_voice(
                part,
                staff,
                0,
                voice,
                0,
                Event {
                    pitches: Pitches::new(&pitches).unwrap(),
                    duration: Duration::Whole,
                    notation: Default::default(),
                },
            )
            .unwrap();
        }
    }
    let plan = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(plan.tracks.len(), score_core::MAX_STAVES * 2);
    assert_eq!(
        plan.elements_at(12_000).into_iter().flatten().count(),
        score_core::MAX_STAVES * 2
    );
    assert_eq!(
        plan.held_notes_at(12_000).into_iter().flatten().count(),
        score_core::MAX_SOUNDING_NOTES
    );
}

#[test]
fn passage_divisi_changes_presentation_without_changing_any_playback_message() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    while s.measures().len() < 6 {
        s.append_measure().unwrap();
    }
    let mut lower = Vec::new();
    for bar in 0..6 {
        s.insert_in(p, bar, 0, note(Step::G, Duration::Whole))
            .unwrap();
        lower.push(
            s.insert_in_voice(
                p,
                StaffId::Upper,
                bar,
                Voice::Two,
                0,
                note(Step::C, Duration::Whole),
            )
            .unwrap(),
        );
    }
    s.set_dynamic(lower[1], Some(Dynamic::P)).unwrap();
    s.add_connection(Connection {
        start: lower[0],
        end: lower[5],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    s.add_connection(Connection {
        start: lower[0],
        end: lower[5],
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let before = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    s.set_passage_divisi(p, 1, Some(3), true).unwrap();
    let after = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(before.messages, after.messages);
    assert_eq!(before.duration_frames, after.duration_frames);
    assert_eq!(before.held_notes_at(240_000), after.held_notes_at(240_000));
}

#[test]
fn sustained_midbar_divisi_and_unis_keep_exact_audio_including_expression() {
    use score_core::{Articulation, DivisiPoint};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    for i in 0..4 {
        s.insert_in(p, 0, i, note(Step::G, Duration::Quarter))
            .unwrap();
    }
    let held = s
        .insert_in_voice(
            p,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::C, Duration::Whole),
        )
        .unwrap();
    s.set_dynamic(held, Some(Dynamic::P)).unwrap();
    s.set_articulation(held, Some(Articulation::Tenuto))
        .unwrap();
    s.set_fermata(held, true).unwrap();
    let before = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let after = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(before.messages, after.messages);
    assert_eq!(before.duration_frames, after.duration_frames);
    assert_eq!(s.event(held).unwrap().duration, Duration::Whole);
}
