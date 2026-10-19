use super::*;
use score_core::{Articulation, Connection, ConnectionKind, Duration, Dynamic, Event, Notation};
fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}
#[test]
fn dynamic_persists_per_staff_staccato_releases_early_and_chase_keeps_velocity() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let b = s.insert(0, 1, note(Step::D, Duration::Quarter)).unwrap();
    let c = s.insert(0, 2, note(Step::E, Duration::Quarter)).unwrap();
    s.set_dynamic(a, Some(Dynamic::P)).unwrap();
    s.set_articulation(a, Some(Articulation::Staccato)).unwrap();
    s.set_articulation(b, Some(Articulation::Accent)).unwrap();
    s.set_dynamic(c, Some(Dynamic::Ff)).unwrap();
    s.set_articulation(c, Some(Articulation::Tenuto)).unwrap();
    let d = s.insert(0, 3, note(Step::F, Duration::Quarter)).unwrap();
    s.set_dynamic(d, Some(Dynamic::Fff)).unwrap();
    s.set_articulation(d, Some(Articulation::Marcato)).unwrap();
    s.insert_on(a.part, StaffId::Lower, 0, 0, note(Step::G, Duration::Whole))
        .unwrap();
    let plan = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    let attack = |key| {
        plan.messages
            .iter()
            .find_map(|m| match m.message {
                Message::NoteOn(note) if note.key == key => Some(note.level),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(attack(60), 56);
    assert_eq!(attack(62), 72);
    assert_eq!(attack(64), 112);
    assert_eq!(attack(67), 88);
    assert_eq!(attack(65), 127);
    assert!(plan.messages.contains(&ScheduledMessage {
        frame: 72_000,
        channel: 0,
        message: Message::NoteOff(64)
    }));
    assert!(plan.messages.contains(&ScheduledMessage {
        frame: 12_000,
        channel: 0,
        message: Message::NoteOff(60)
    }));
    assert!(plan.elements_at(18_000).contains(&Some(a))); // written length remains a quarter
    assert!(
        !plan
            .held_notes_at(18_000)
            .into_iter()
            .flatten()
            .any(|n| n.note.key == 60)
    );
    assert_eq!(
        plan.held_notes_at(30_000)
            .into_iter()
            .flatten()
            .find(|n| n.note.key == 62)
            .unwrap()
            .note
            .level,
        72
    );
}
#[test]
fn tied_chord_chains_cross_bars_without_new_attacks_and_can_start_at_continuation() {
    let mut s = Score::default();
    let mut chord = note(Step::C, Duration::Half);
    chord.pitches = chord.pitches.with(Pitch::new(Step::E, 4).unwrap()).unwrap();
    let a = s.insert(0, 0, chord).unwrap();
    let b = s.insert(0, 1, chord).unwrap();
    let c = s.insert(1, 0, chord).unwrap();
    s.set_dynamic(a, Some(Dynamic::Pp)).unwrap();
    s.set_dynamic(b, Some(Dynamic::Fff)).unwrap();
    s.set_articulation(a, Some(Articulation::Staccato)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    s.add_connection(Connection {
        start: b,
        end: c,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let plan = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(_)))
            .count(),
        2
    );
    assert!(
        plan.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOff(_)))
            .all(|m| m.frame == 144_000 - 2_880) // detached release of the chain
    );
    assert_eq!(plan.frame_for_element(c), Some(96_000));
    let held: Vec<_> = plan.held_notes_at(96_000).into_iter().flatten().collect();
    assert_eq!(held.len(), 2);
    assert!(held.iter().all(|n| n.note.level == 44));
    assert_eq!(plan.element_at(96_000), Some(c));
    assert_eq!(plan.held_notes_at(144_000).into_iter().flatten().count(), 0);
}

#[test]
fn overlapping_unisons_chase_the_original_attack_across_a_chain_of_hands() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let mut rest = note(Step::C, Duration::Quarter);
    rest.pitches = Pitches::default();
    s.insert(0, 0, rest).unwrap();
    let a = s.insert(0, 1, note(Step::C, Duration::Half)).unwrap();
    s.set_dynamic(a, Some(Dynamic::F)).unwrap();
    let lower = s
        .insert_on(part, StaffId::Lower, 0, 0, note(Step::C, Duration::Half))
        .unwrap();
    s.set_dynamic(lower, Some(Dynamic::P)).unwrap();
    // Lower attack ends at 48k, upper continues to 72k without a new attack.
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    for frame in [30_000, 60_000] {
        assert_eq!(
            p.held_notes_at(frame)
                .into_iter()
                .flatten()
                .find(|n| n.note.key == 60)
                .unwrap()
                .note
                .level,
            56
        );
    }
    assert_eq!(
        p.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(NoteEvent { key: 60, .. })))
            .count(),
        1
    );
}

#[test]
fn pizzicato_chases_original_tied_attack_and_arco_restores_each_string_timbre() {
    use score_core::{Instrument, PlayingTechnique as T};
    for instrument in [
        Instrument::Violin,
        Instrument::Viola,
        Instrument::Cello,
        Instrument::DoubleBass,
    ] {
        let mut s = Score::default();
        let piano = s.insert(0, 0, note(Step::G, Duration::Whole)).unwrap();
        let part = s.add_part(instrument).unwrap();
        let a = s
            .insert_in(part, 0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let b = s
            .insert_in(part, 0, 1, note(Step::C, Duration::Quarter))
            .unwrap();
        let c = s
            .insert_in(part, 0, 2, note(Step::E, Duration::Quarter))
            .unwrap();
        let d = s
            .insert_in(part, 0, 3, note(Step::F, Duration::Quarter))
            .unwrap();
        s.set_technique(a, Some(T::Pizzicato)).unwrap();
        s.set_technique(b, Some(T::Arco)).unwrap();
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
        s.set_dynamic(a, Some(Dynamic::P)).unwrap();
        let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
        let channel = p.tracks.iter().find(|t| t.part == part).unwrap().channel;
        assert_eq!(
            p.messages
                .iter()
                .filter(|m| m.channel == channel && matches!(m.message, Message::NoteOn(_)))
                .count(),
            3
        );
        let held = p
            .held_notes_at(p.frame_for_element(b).unwrap())
            .into_iter()
            .flatten()
            .find(|n| n.channel == channel)
            .unwrap();
        assert_eq!((held.note.technique, held.note.level), (T::Pizzicato, 56));
        let attack_at = |p: &PlaybackPlan, frame| {
            p.messages
                .iter()
                .find_map(|m| match m.message {
                    Message::NoteOn(note) if m.channel == channel && m.frame == frame => Some(note),
                    _ => None,
                })
                .unwrap()
        };
        for id in [c, d] {
            let frame = p.frame_for_element(id).unwrap();
            assert_eq!(attack_at(&p, frame).technique, T::Arco);
            assert_eq!(attack_at(&p, frame).instrument, instrument);
            assert_eq!(
                p.held_notes_at(frame + 1)
                    .into_iter()
                    .flatten()
                    .find(|n| n.channel == channel)
                    .unwrap()
                    .note
                    .technique,
                T::Arco
            );
        }
        let piano_note = p
            .held_notes_at(30_000)
            .into_iter()
            .flatten()
            .find(|n| {
                n.channel
                    == p.tracks
                        .iter()
                        .find(|t| t.part == piano.part)
                        .unwrap()
                        .channel
            })
            .unwrap()
            .note;
        assert_eq!(
            (piano_note.instrument, piano_note.technique),
            (Instrument::Piano, T::Arco)
        );
        // An instruction on a rest carries through an implicit bar remainder.
        let mut rest = note(Step::C, Duration::Quarter);
        rest.pitches = Pitches::default();
        let r = s.insert_in(part, 1, 0, rest).unwrap();
        s.set_technique(r, Some(T::Pizzicato)).unwrap();
        let next = s
            .insert_in(part, 2, 0, note(Step::A, Duration::Quarter))
            .unwrap();
        let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
        let frame = p.frame_for_element(next).unwrap();
        assert_eq!(attack_at(&p, frame).technique, T::Pizzicato);
    }
}

#[test]
fn hairpins_schedule_continuous_expression_chase_ties_and_isolate_both_piano_hands() {
    let mut s = Score::default();
    let mut hands = Vec::new();
    for (staff, kind, from, to) in [
        (
            StaffId::Upper,
            ConnectionKind::Crescendo,
            Dynamic::P,
            Dynamic::F,
        ),
        (
            StaffId::Lower,
            ConnectionKind::Diminuendo,
            Dynamic::F,
            Dynamic::P,
        ),
    ] {
        let part = s.parts()[0].id();
        let a = s
            .insert_on(part, staff, 0, 0, note(Step::C, Duration::Half))
            .unwrap();
        let b = s
            .insert_on(part, staff, 0, 1, note(Step::C, Duration::Half))
            .unwrap();
        s.set_dynamic(a, Some(from)).unwrap();
        s.set_dynamic(b, Some(to)).unwrap();
        s.add_connection(Connection {
            start: a,
            end: b,
            kind,
        })
        .unwrap();
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
        hands.push(a);
    }
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(p.tracks[0].channel, 0);
    assert_eq!(p.tracks[1].channel, 16);
    assert_eq!(p.expressions_at(24_000)[0], 99);
    assert_eq!(p.expressions_at(24_000)[16], 99);
    assert_eq!(p.expressions_at(36_000)[0], 113);
    assert_eq!(p.expressions_at(36_000)[16], 85);
    assert_eq!(p.expressions_at(72_000)[0], 127);
    assert_eq!(p.expressions_at(72_000)[16], 71);
    let held: Vec<_> = p.held_notes_at(36_000).into_iter().flatten().collect();
    assert_eq!(held.len(), 2);
    assert!(held.iter().all(|n| n.note.level == 100));
    assert_eq!(
        p.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(_)))
            .count(),
        2
    );
    for channel in [0, 16] {
        let points: Vec<_> = p
            .messages
            .iter()
            .filter_map(|m| {
                if m.channel == channel {
                    if let Message::Expression(level) = m.message {
                        Some((m.frame, level))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        assert!(points.len() >= 45);
        for pair in points.windows(2) {
            assert!(pair[0].0 <= pair[1].0);
            assert!(pair[0].1.abs_diff(pair[1].1) <= 1);
        }
        let first = p
            .messages
            .iter()
            .position(|m| {
                m.channel == channel && m.frame == 0 && matches!(m.message, Message::Expression(_))
            })
            .unwrap();
        let attack = p
            .messages
            .iter()
            .position(|m| {
                m.channel == channel && m.frame == 0 && matches!(m.message, Message::NoteOn(_))
            })
            .unwrap();
        assert!(first < attack);
    }
    // Full catalog capacity still provides a distinct route for all 28 hands.
    for _ in 1..score_core::MAX_PARTS {
        let part = s.add_part(score_core::Instrument::Piano).unwrap();
        for staff in [StaffId::Upper, StaffId::Lower] {
            let a = s
                .insert_on(part, staff, 0, 0, note(Step::E, Duration::Half))
                .unwrap();
            let b = s
                .insert_on(part, staff, 0, 1, note(Step::F, Duration::Half))
                .unwrap();
            s.add_connection(Connection {
                start: a,
                end: b,
                kind: ConnectionKind::Crescendo,
            })
            .unwrap();
        }
    }
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(p.tracks.len(), MAX_STAVES);
    assert_eq!(
        p.tracks
            .iter()
            .map(|t| t.channel)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        MAX_STAVES
    );
    assert!(
        p.tracks
            .iter()
            .all(|t| usize::from(t.channel) < PLAYBACK_CHANNELS
                && t.channel % 16 != 9
                && t.channel % 16 != 15)
    );
}

#[test]
fn controller_timing_follows_the_original_slope_through_rests_and_staccato_gaps() {
    for staccato in [false, true] {
        let mut s = Score::default();
        let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
        let mut rest = note(Step::D, Duration::Quarter);
        rest.pitches = Pitches::default();
        s.insert(0, 1, rest).unwrap();
        let b = s.insert(0, 2, note(Step::E, Duration::Quarter)).unwrap();
        s.set_dynamic(a, Some(Dynamic::P)).unwrap();
        s.set_dynamic(b, Some(Dynamic::F)).unwrap();
        if staccato {
            s.set_articulation(a, Some(Articulation::Staccato)).unwrap();
        }
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
        let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
        // Last tick before the release: staccato halves, detached keeps 92 %.
        let last_tick = if staccato { 7 } else { 14 };
        let frame = Tempo::default().frame_at(last_tick * 3, 48_000);
        let held = p.held_notes_at(frame).into_iter().flatten().next().unwrap();
        let actual = f64::from(held.note.level) * f64::from(p.expressions_at(frame)[0]) / 127.;
        let expected = 56. + 44. * last_tick as f64 / 32.;
        assert!(
            (actual - expected).abs() < 1.,
            "{actual} vs {expected}, staccato={staccato}"
        );
        assert_eq!(
            p.expressions_at(
                Tempo::default().frame_at((if staccato { 8 } else { 16 }) * 3, 48_000)
            )[0],
            127
        );
    }
}

#[test]
fn dotted_and_triplet_onsets_stay_exact_with_chords_rests_ties_and_other_hands() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let mut dotted = note(Step::C, Duration::Quarter.with_dotted(true));
    dotted.pitches = dotted
        .pitches
        .with(Pitch::new(Step::E, 4).unwrap())
        .unwrap();
    let a = s.insert(0, 0, dotted).unwrap();
    let rest = s
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Eighth,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let mut ids = Vec::new();
    for i in 2..5 {
        ids.push(
            s.insert(0, i, note(Step::G, Duration::Eighth.with_triplet(true)))
                .unwrap(),
        );
    }
    s.add_connection(Connection {
        start: ids[0],
        end: ids[1],
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let lower = s
        .insert_on(part, StaffId::Lower, 0, 0, note(Step::C, Duration::Whole))
        .unwrap();
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert_eq!(p.frame_for_element(a), Some(0));
    assert_eq!(p.frame_for_element(rest), Some(36_000));
    assert_eq!(
        ids.iter()
            .map(|id| p.frame_for_element(*id).unwrap())
            .collect::<Vec<_>>(),
        [48_000, 56_000, 64_000]
    );
    assert_eq!(p.frame_for_element(lower), Some(0));
    assert_eq!(p.duration_frames, 96_000);
    assert_eq!(
        p.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(NoteEvent { key: 67, .. })))
            .count(),
        2
    );
    assert!(
        p.held_notes_at(60_000)
            .into_iter()
            .flatten()
            .any(|n| n.note.key == 67)
    );
    // An entire long score of triplets ends on the same clock as straight beats.
    let mut s = Score::default();
    for bar in 0..96 {
        while s.measures().len() <= bar {
            s.append_measure().unwrap();
        }
        for i in 0..12 {
            s.insert(bar, i, note(Step::C, Duration::Eighth.with_triplet(true)))
                .unwrap();
        }
    }
    let tempo = Tempo::new(73).unwrap();
    let p = PlaybackPlan::new(&s, tempo, 44_100).unwrap();
    assert_eq!(
        p.duration_frames,
        tempo.frame_at(96 * u64::from(score_core::TICKS_PER_WHOLE), 44_100)
    );
}

#[test]
fn slurs_connect_notes_with_a_short_overlap_and_unslurred_notes_detach() {
    let mut s = Score::default();
    // Bar 1: C D D E under one slur; the F in bar 2 stays outside it.
    let ids: Vec<_> = [Step::C, Step::D, Step::D, Step::E]
        .into_iter()
        .enumerate()
        .map(|(i, step)| s.insert(0, i, note(step, Duration::Quarter)).unwrap())
        .collect();
    s.insert(1, 0, note(Step::F, Duration::Quarter)).unwrap();
    s.add_connection(Connection {
        start: ids[0],
        end: ids[3],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    let release = |key, after| {
        p.messages
            .iter()
            .find(|m| m.message == Message::NoteOff(key) && m.frame > after)
            .unwrap()
            .frame
    };
    // C overlaps 25 ms into D; the repeated D is re-attacked exactly on time.
    assert_eq!(release(60, 0), 24_000 + 1_200);
    assert_eq!(release(62, 0), 48_000);
    assert_eq!(release(62, 48_000), 72_000 + 1_200);
    // The last slurred note and the following unslurred one are detached.
    assert_eq!(release(64, 0), 96_000 - 1_920);
    assert_eq!(release(65, 0), 96_000 + 22_080);
    let legato: Vec<_> = p
        .messages
        .iter()
        .filter_map(|m| match m.message {
            Message::NoteOn(n) => Some((n.key, n.legato)),
            _ => None,
        })
        .collect();
    assert_eq!(
        legato,
        [(60, true), (62, true), (62, true), (64, true), (65, false)]
    );
    // A slurred staccato is portato: longer than staccato, still separated.
    s.set_articulation(ids[0], Some(Articulation::Staccato))
        .unwrap();
    let p = PlaybackPlan::new(&s, Tempo::default(), 48_000).unwrap();
    assert!(p.messages.contains(&ScheduledMessage {
        frame: 18_000,
        channel: 0,
        message: Message::NoteOff(60)
    }));
    let preview = Audition::for_element(&s, ids[0], Tempo::default(), 48_000).unwrap();
    assert!(preview.legato);
    assert_eq!(preview.duration_frames, 12_000 * 75 / 100);
    let outside = s.next_event(ids[3]).unwrap();
    assert!(
        !Audition::for_element(&s, outside, Tempo::default(), 48_000)
            .unwrap()
            .legato
    );
}
