use super::*;

#[test]
fn tempo_changes_keep_a_tied_crescendos_level_on_its_musical_slope() {
    use score_core::{Connection, ConnectionKind, Dynamic, TempoMark};
    let mut score = Score::default();
    let mut ids = Vec::new();
    for bar in 0..4 {
        ids.push(
            score
                .insert(
                    bar,
                    0,
                    score_core::Event {
                        pitches: score_core::Pitches::single(score_core::Pitch::new(
                            score_core::Step::C,
                            4,
                        )),
                        duration: score_core::Duration::Whole,
                        notation: Default::default(),
                    },
                )
                .unwrap(),
        );
    }
    score.set_dynamic(ids[0], Some(Dynamic::Pp)).unwrap();
    score.set_dynamic(ids[3], Some(Dynamic::Ff)).unwrap();
    for pair in ids.windows(2) {
        score
            .add_connection(Connection {
                start: pair[0],
                end: pair[1],
                kind: ConnectionKind::Tie,
            })
            .unwrap();
    }
    score
        .add_connection(Connection {
            start: ids[0],
            end: ids[3],
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    score
        .set_tempo_at(
            1,
            TempoMark {
                bpm: 60,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_tempo_at(
            2,
            TempoMark {
                bpm: 180,
                ..Default::default()
            },
        )
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    let reference = f64::from(plan.sounds[0].reference_level);
    for tick in [96, 192, 240, 384, 480, 575] {
        let frame = plan.timeline.frame_at(tick, 48_000);
        let cc = plan
            .messages
            .iter()
            .filter(|m| m.frame <= frame)
            .filter_map(|m| {
                if let Message::Expression(value) = m.message {
                    Some(value)
                } else {
                    None
                }
            })
            .next_back()
            .unwrap();
        let level = f64::from(Dynamic::Pp.velocity())
            + (f64::from(Dynamic::Ff.velocity()) - f64::from(Dynamic::Pp.velocity())) * tick as f64
                / 576.;
        assert!(
            (f64::from(cc) - (level * 127. / reference).round()).abs() <= 1.,
            "tick {tick}: {cc}"
        );
    }
}
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

#[test]
fn local_keys_and_meters_schedule_notes_and_fermata_on_the_shared_timeline() {
    use score_core::{KeyMode, KeySignature, TimeSignature};
    let mut score = Score::default();
    score
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    score
        .set_key_signature_at(2, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let a = score
        .insert(1, 0, note(Step::F, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(2, 0, note(Step::F, Duration::Quarter))
        .unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert!(
        plan.messages
            .iter()
            .any(|m| m.frame == 96_000
                && matches!(m.message, Message::NoteOn(note) if note.key == 65))
    );
    assert!(plan.messages.iter().any(
        |m| m.frame == 168_000 && matches!(m.message, Message::NoteOn(note) if note.key == 66)
    ));
    let preview = Audition::for_element(&score, b, Tempo::default(), 48_000).unwrap();
    assert_eq!(preview.keys[0], Some(66));
    score.set_fermata(a, true).unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert!(plan.messages.iter().any(
        |m| m.frame == 192_000 && matches!(m.message, Message::NoteOn(note) if note.key == 66)
    ));
}

#[test]
fn arpeggios_roll_sounding_keys_and_seek_chases_only_attacked_tones() {
    use score_core::{Arpeggio, Connection, ConnectionKind, Duration, Event, Notation};
    for mark in Arpeggio::ALL {
        for rate in [16_000, 48_000, 192_000] {
            let mut score = Score::default();
            let event = Event {
                pitches: Pitches::new(&[
                    Pitch::new(Step::C, 4).unwrap(),
                    Pitch::new(Step::E, 4).unwrap(),
                    Pitch::new(Step::G, 4).unwrap(),
                ])
                .unwrap(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            };
            let first = score.insert(0, 0, event).unwrap();
            let second = score.insert(0, 1, event).unwrap();
            score.set_arpeggio(first, Some(mark)).unwrap();
            score
                .add_connection(Connection {
                    start: first,
                    end: second,
                    kind: ConnectionKind::Tie,
                })
                .unwrap();
            let plan = PlaybackPlan::new(&score, Tempo::default(), rate).unwrap();
            let attacks: Vec<_> = plan
                .messages
                .iter()
                .filter_map(|m| {
                    if let Message::NoteOn(note) = m.message {
                        Some((m.frame, note.key))
                    } else {
                        None
                    }
                })
                .collect();
            let expected = if mark == Arpeggio::Down {
                [67, 64, 60]
            } else {
                [60, 64, 67]
            };
            assert_eq!(
                attacks.iter().map(|(_, key)| *key).collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                attacks.iter().map(|(frame, _)| *frame).collect::<Vec<_>>(),
                [0, u64::from(rate) * 30 / 1000, u64::from(rate) * 60 / 1000]
            );
            assert_eq!(
                plan.held_notes_at(attacks[1].0 - 1)
                    .into_iter()
                    .flatten()
                    .count(),
                1
            );
            assert_eq!(
                plan.held_notes_at(attacks[2].0 + 1)
                    .into_iter()
                    .flatten()
                    .count(),
                3
            );
            let releases: Vec<_> = plan
                .messages
                .iter()
                .filter(|m| matches!(m.message, Message::NoteOff(_)))
                .map(|m| m.frame)
                .collect();
            assert_eq!(releases.len(), 3);
            assert!(
                releases
                    .iter()
                    .all(|frame| *frame == releases[0] && *frame > attacks[2].0)
            );
            let preview = Audition::for_element(&score, first, Tempo::default(), rate).unwrap();
            assert_eq!(
                preview.keys.into_iter().flatten().collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                preview.delays[..3],
                [attacks[0].0, attacks[1].0, attacks[2].0]
            );
        }
    }
}

#[test]
fn fast_arpeggios_fit_the_gate_and_enharmonic_unisons_attack_once() {
    use score_core::{Accidental, Arpeggio, Duration, Event, Notation, TempoMark};
    let mut score = Score::default();
    score
        .set_tempo_at(
            0,
            TempoMark {
                bpm: 400,
                ..Default::default()
            },
        )
        .unwrap();
    let event = Event {
        pitches: Pitches::new(&[
            Pitch::new(Step::C, 4)
                .unwrap()
                .with_accidental(Some(Accidental::Sharp)),
            Pitch::new(Step::D, 4)
                .unwrap()
                .with_accidental(Some(Accidental::Flat)),
            Pitch::new(Step::E, 4).unwrap(),
            Pitch::new(Step::G, 4).unwrap(),
        ])
        .unwrap(),
        duration: Duration::Sixteenth,
        notation: Notation {
            arpeggio: Some(Arpeggio::Up),
            articulation: Some(Articulation::Staccato),
            ..Default::default()
        },
    };
    let id = score.insert(0, 0, event).unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    let ons: Vec<_> = plan
        .messages
        .iter()
        .filter(|m| matches!(m.message, Message::NoteOn(_)))
        .collect();
    let off = plan
        .messages
        .iter()
        .find(|m| matches!(m.message, Message::NoteOff(_)))
        .unwrap()
        .frame;
    assert_eq!(ons.len(), 3);
    assert!(ons.windows(2).all(|pair| pair[0].frame < pair[1].frame));
    assert!(ons.last().unwrap().frame <= off / 2);
    let preview = Audition::for_element(&score, id, Tempo::default(), 48_000).unwrap();
    assert_eq!(preview.keys.iter().flatten().count(), 3);
    assert!(
        preview
            .keys
            .iter()
            .zip(preview.delays)
            .all(|(key, delay)| key.is_none() || delay < preview.duration_frames)
    );
}

#[test]
fn piano_hands_form_one_roll_with_their_own_levels_gates_and_tied_attacks() {
    use score_core::{
        Arpeggio, Articulation, Connection, ConnectionKind, Duration, Dynamic, Event, Notation,
    };
    for direction in Arpeggio::ALL {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let upper_event = Event {
            pitches: Pitches::new(&[
                Pitch::new(Step::C, 4).unwrap(),
                Pitch::new(Step::E, 4).unwrap(),
                Pitch::new(Step::G, 4).unwrap(),
            ])
            .unwrap(),
            duration: Duration::Quarter,
            notation: Notation::default(),
        };
        let lower_event = Event {
            pitches: Pitches::new(&[
                Pitch::new(Step::C, 3).unwrap(),
                Pitch::new(Step::E, 3).unwrap(),
                Pitch::new(Step::G, 3).unwrap(),
            ])
            .unwrap(),
            ..upper_event
        };
        let upper = score.insert(0, 0, upper_event).unwrap();
        let upper2 = score.insert(0, 1, upper_event).unwrap();
        let lower = score
            .insert_on(piano, StaffId::Lower, 0, 0, lower_event)
            .unwrap();
        let lower2 = score
            .insert_on(piano, StaffId::Lower, 0, 1, lower_event)
            .unwrap();
        score.set_dynamic(upper, Some(Dynamic::F)).unwrap();
        score.set_dynamic(lower, Some(Dynamic::P)).unwrap();
        score
            .set_articulation(lower2, Some(Articulation::Staccato))
            .unwrap();
        for (start, end) in [(upper, upper2), (lower, lower2)] {
            score
                .add_connection(Connection {
                    start,
                    end,
                    kind: ConnectionKind::Tie,
                })
                .unwrap();
        }
        score.set_arpeggio_both_hands(lower, true).unwrap();
        score.set_arpeggio(lower, Some(direction)).unwrap();
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        let attacks: Vec<_> = plan
            .messages
            .iter()
            .filter_map(|m| {
                if let Message::NoteOn(note) = m.message {
                    Some((m.frame, note))
                } else {
                    None
                }
            })
            .collect();
        let expected = if direction == Arpeggio::Down {
            vec![67, 64, 60, 55, 52, 48]
        } else {
            vec![48, 52, 55, 60, 64, 67]
        };
        assert_eq!(
            attacks.iter().map(|(_, note)| note.key).collect::<Vec<_>>(),
            expected
        );
        assert!(attacks.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(attacks.last().unwrap().0, 5760);
        assert!(attacks.iter().all(|(_, note)| note.level
            == if note.key < 60 {
                Dynamic::P.velocity()
            } else {
                Dynamic::F.velocity()
            }));
        assert_eq!(
            plan.held_notes_at(attacks[1].0 - 1)
                .into_iter()
                .flatten()
                .count(),
            1
        );
        assert_eq!(
            plan.held_notes_at(attacks[5].0 + 1)
                .into_iter()
                .flatten()
                .count(),
            6
        );
        let off = |key| {
            plan.messages
                .iter()
                .find(|m| m.message == Message::NoteOff(key))
                .unwrap()
                .frame
        };
        assert!(off(48) < off(60));
        let preview = Audition::for_element(&score, lower, Tempo::default(), 48_000).unwrap();
        assert_eq!(
            preview.keys.into_iter().flatten().collect::<Vec<_>>(),
            expected
        );
        for index in 0..6 {
            assert_eq!(
                preview.note_at(index).level,
                if preview.keys[index].unwrap() < 60 {
                    Dynamic::P.velocity()
                } else {
                    Dynamic::F.velocity()
                }
            );
            assert!(preview.delays[index] < preview.gate_at(index));
        }
        let same = Audition::for_element(&score, upper, Tempo::default(), 48_000).unwrap();
        assert_eq!(preview.keys, same.keys);
        assert_eq!(preview.delays, same.delays);
    }
}

#[test]
fn full_sixteen_tone_piano_preview_and_plan_keep_one_bounded_roll_and_separate_releases() {
    use score_core::{Arpeggio, Articulation, Duration, Event, Notation};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let make = |octave| Event {
        pitches: Pitches::new(
            &(0..8)
                .map(|i| Pitch::new(Step::C, octave).unwrap().shift(i * 2).unwrap())
                .collect::<Vec<_>>(),
        )
        .unwrap(),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    let upper = score.insert(0, 0, make(5)).unwrap();
    let lower = score
        .insert_on(piano, StaffId::Lower, 0, 0, make(2))
        .unwrap();
    score
        .set_articulation(lower, Some(Articulation::Staccato))
        .unwrap();
    score.set_arpeggio_both_hands(upper, true).unwrap();
    score.set_arpeggio(lower, Some(Arpeggio::Up)).unwrap();
    let preview = Audition::for_element(&score, lower, Tempo::default(), 48_000).unwrap();
    assert_eq!(preview.keys.iter().flatten().count(), 16);
    let lower_index = preview
        .keys
        .iter()
        .position(|key| *key == Some(36))
        .unwrap();
    let upper_index = preview
        .keys
        .iter()
        .position(|key| *key == Some(72))
        .unwrap();
    assert!(preview.gate_at(lower_index) < preview.gate_at(upper_index));
    assert!(
        preview
            .keys
            .iter()
            .enumerate()
            .all(|(index, key)| key.is_none() || preview.delays[index] < preview.gate_at(index))
    );
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.messages
            .iter()
            .filter(|m| matches!(m.message, Message::NoteOn(_)))
            .count(),
        16
    );
}

#[test]
fn shared_arpeggios_preserve_incoming_ties_and_merge_enharmonic_unisons() {
    use score_core::{Accidental, Connection, ConnectionKind, Duration, Event, Notation};
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    let first = score.insert(0, 0, event).unwrap();
    let continued = score.insert(0, 1, event).unwrap();
    score
        .add_connection(Connection {
            start: first,
            end: continued,
            kind: ConnectionKind::Tie,
        })
        .unwrap();
    score
        .insert_on(
            piano,
            StaffId::Lower,
            0,
            0,
            Event {
                pitches: Pitches::default(),
                ..event
            },
        )
        .unwrap();
    let lower = score
        .insert_on(
            piano,
            StaffId::Lower,
            0,
            1,
            Event {
                pitches: Pitches::single(Pitch::new(Step::G, 3)),
                ..event
            },
        )
        .unwrap();
    score.set_arpeggio_both_hands(continued, true).unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    let attacks: Vec<_> = plan
        .messages
        .iter()
        .filter_map(|message| {
            if let Message::NoteOn(note) = message.message {
                Some((message.frame, note.key))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(attacks, [(0, 60), (24_000, 55)]);
    assert_eq!(plan.held_notes_at(24_001).into_iter().flatten().count(), 2);
    assert_eq!(score.piano_partner(continued), Some(lower));

    let mut score = Score::default();
    let upper = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Some(
                    Pitch::new(Step::C, 4)
                        .unwrap()
                        .with_accidental(Some(Accidental::Sharp)),
                )),
                ..event
            },
        )
        .unwrap();
    let lower = score
        .insert_on(
            piano,
            StaffId::Lower,
            0,
            0,
            Event {
                pitches: Pitches::single(Some(
                    Pitch::new(Step::D, 4)
                        .unwrap()
                        .with_accidental(Some(Accidental::Flat)),
                )),
                duration: Duration::Half,
                ..event
            },
        )
        .unwrap();
    score.set_arpeggio_both_hands(lower, true).unwrap();
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        plan.messages
            .iter()
            .filter(|message| matches!(message.message, Message::NoteOn(_)))
            .count(),
        1
    );
    let preview = Audition::for_element(&score, upper, Tempo::default(), 48_000).unwrap();
    assert_eq!(
        preview.keys.iter().flatten().copied().collect::<Vec<_>>(),
        [61]
    );
    assert_eq!(preview.delays[0], 0);
}
