use super::*;

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

#[test]
fn moving_into_free_time_leaves_rests_and_keeps_all_other_onsets() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let b = s.insert(0, 1, note(Step::D, Duration::Quarter)).unwrap();
    let moved = s.move_event(a, 0, 96).unwrap();
    assert_eq!(moved, a);
    assert_eq!(s.event_tick(a), Some(96));
    assert_eq!(s.event_tick(b), Some(48));
    assert_eq!(s.measures()[0].voice_ticks(Voice::One), 144);
    assert!(
        s.measures()[0]
            .voice_iter(Voice::One)
            .next()
            .unwrap()
            .1
            .pitches
            .is_empty()
    );
    s.validate().unwrap();
}

#[test]
fn occupied_equal_slots_exchange_complete_chords_and_annotations_across_bars() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    s.add_pitch(a, Pitch::new(Step::E, 4).unwrap()).unwrap();
    s.set_dynamic(a, Some(Dynamic::P)).unwrap();
    s.set_arpeggio(a, Some(Arpeggio::Up)).unwrap();
    let b = s.insert(1, 0, note(Step::G, Duration::Quarter)).unwrap();
    let original_a = *s.event(a).unwrap();
    let original_b = *s.event(b).unwrap();
    let moved = s.move_event(a, 1, 0).unwrap();
    assert_eq!(s.event(moved), Some(&original_a));
    let new_b = ElementId {
        measure: 0,
        event: s.measures()[0].voice_id_at(Voice::One, 0).unwrap(),
        ..a
    };
    assert_eq!(s.event(new_b), Some(&original_b));
    assert_eq!(s.event_tick(moved), Some(192));
    assert_eq!(s.event_tick(new_b), Some(0));
    s.validate().unwrap();
}

#[test]
fn silence_splitting_and_self_overlapping_moves_keep_exact_ticks() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let b = s
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Half,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let c = s.insert(0, 2, note(Step::D, Duration::Quarter)).unwrap();
    let a = s.move_event(a, 0, 12).unwrap();
    assert_eq!(s.event_tick(a), Some(12));
    assert_eq!(s.event_tick(c), Some(144));
    assert!(s.event(b).is_none());
    assert_eq!(s.measures()[0].voice_ticks(Voice::One), 192);
    s.validate().unwrap();
}

#[test]
fn rejects_incompatible_destinations_atomically_and_no_op_is_identical() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    s.insert(1, 0, note(Step::D, Duration::Half)).unwrap();
    let before = s.clone();
    assert!(s.move_event(a, 1, 0).is_err());
    assert!(s.move_event(a, 3, 168).is_err());
    assert!(s.move_event(a, usize::MAX, 0).is_err());
    assert!(s.move_event(a, 1, u16::MAX).is_err());
    assert!(s.move_event(a, 2, 2).is_err());
    assert_eq!(s, before);
    assert_eq!(s.move_event(a, 0, 0).unwrap(), a);
    assert_eq!(s, before);
}

#[test]
fn keys_voices_and_other_hands_remain_independent() {
    let mut s = Score::default();
    let part = s.parts()[0].id();
    let a = s
        .insert_in_voice(
            part,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::F, Duration::Eighth.with_triplet(true)),
        )
        .unwrap();
    s.insert_on(part, StaffId::Lower, 0, 0, note(Step::D, Duration::Whole))
        .unwrap();
    s.set_key_signature_at(1, KeySignature::new(1, KeyMode::Major).unwrap())
        .unwrap();
    let lower = s.part(part).unwrap().staff(StaffId::Lower).unwrap().clone();
    let moved = s.move_event(a, 1, 16).unwrap();
    assert_eq!(s.voice_of(moved), Some(Voice::Two));
    assert_eq!(s.event_tick(moved), Some(208));
    assert_eq!(
        s.event(moved)
            .unwrap()
            .pitches
            .first()
            .unwrap()
            .accidental(),
        Some(Accidental::Natural)
    );
    assert_eq!(s.part(part).unwrap().staff(StaffId::Lower).unwrap(), &lower);
    assert!(s.measures()[1].voice_iter(Voice::One).next().is_none());
    s.validate().unwrap();
}

#[test]
fn links_follow_the_moved_event_and_invalid_ties_are_pruned() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let b = s.insert(0, 1, note(Step::C, Duration::Quarter)).unwrap();
    let c = s.insert(3, 0, note(Step::G, Duration::Quarter)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    s.add_connection(Connection {
        start: a,
        end: c,
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let moved = s.move_event(a, 1, 0).unwrap();
    assert!(
        !s.connections()
            .iter()
            .any(|c| c.kind == ConnectionKind::Tie)
    );
    assert!(s.connections().iter().any(|link| link.start == moved
        && link.end == c
        && link.kind == ConnectionKind::Crescendo));
    s.validate().unwrap();
}

#[test]
fn silent_destination_marks_are_protected() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let mut rest = note(Step::D, Duration::Half);
    rest.pitches = Pitches::default();
    rest.notation.fermata = true;
    s.insert(1, 0, rest).unwrap();
    let before = s.clone();
    assert!(s.move_event(a, 1, 0).is_err());
    assert_eq!(s, before);
}

#[test]
fn silent_destination_hairpin_anchors_are_protected_when_consumed_or_split() {
    let mut s = Score::default();
    let a = s.insert(0, 0, note(Step::C, Duration::Quarter)).unwrap();
    let mut rest = note(Step::C, Duration::Half);
    rest.pitches = Pitches::default();
    let r = s.insert(1, 0, rest).unwrap();
    let b = s.insert(1, 1, note(Step::D, Duration::Quarter)).unwrap();
    s.add_connection(Connection {
        start: r,
        end: b,
        kind: ConnectionKind::Crescendo,
    })
    .unwrap();
    let before = s.clone();
    assert!(s.move_event(a, 1, 0).is_err());
    assert!(s.move_event(a, 1, 24).is_err());
    assert_eq!(s, before);
}
