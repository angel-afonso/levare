use super::*;

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    }
}

#[test]
fn point_offset_sets_reads_and_resets_without_noop_history() {
    let mut score = Score::default();
    let id = score.insert(0, 0, note(Step::C)).unwrap();
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    let mark = MarkRef::Dynamic(id);
    let offset = Offset { dx: 20, dy: -15 };
    assert_eq!(score.mark_offset(mark), Offset::ZERO);
    assert_eq!(score.set_mark_offset(mark, offset), Ok(true));
    assert_eq!(score.mark_offset(mark), offset);
    assert_eq!(score.set_mark_offset(mark, offset), Ok(false));
    assert_eq!(score.set_mark_offset(mark, Offset::ZERO), Ok(true));
    assert_eq!(score.set_mark_offset(mark, Offset::ZERO), Ok(false));
    score.validate().unwrap();
}

#[test]
fn removing_a_mark_cleans_its_offset_and_replacing_its_kind_keeps_it() {
    let mut score = Score::default();
    let id = score.insert(0, 0, note(Step::C)).unwrap();
    let mark = MarkRef::Dynamic(id);
    let offset = Offset { dx: 12, dy: 7 };
    score.set_dynamic(id, Some(Dynamic::P)).unwrap();
    score.set_mark_offset(mark, offset).unwrap();
    score.set_dynamic(id, Some(Dynamic::F)).unwrap();
    assert_eq!(score.mark_offset(mark), offset);
    score.remove_mark(mark).unwrap();
    assert_eq!(score.mark_offset(mark), Offset::ZERO);
    assert!(score.event(id).unwrap().notation.offsets.is_empty());
    score.validate().unwrap();
}

#[test]
fn missing_unsupported_and_out_of_range_offsets_are_rejected() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::C)).unwrap();
    let offset = Offset { dx: 1, dy: 0 };
    assert!(score.set_mark_offset(MarkRef::Dynamic(a), offset).is_err());
    score.set_tremolo(a, Some(Tremolo::One)).unwrap();
    assert!(score.set_mark_offset(MarkRef::Tremolo(a), offset).is_err());
    let tie = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    };
    score.add_connection(tie).unwrap();
    assert!(
        score
            .set_mark_offset(MarkRef::Connection(tie), offset)
            .is_err()
    );
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    for dx in [401, i16::MIN, i16::MAX] {
        let invalid = Offset { dx, dy: 0 };
        assert!(!invalid.is_valid());
        assert!(score.set_mark_offset(MarkRef::Dynamic(a), invalid).is_err());
    }
    score.validate().unwrap();
}

#[test]
fn connection_offset_follows_measure_and_event_rewrites_then_removal() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let connection = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    let offset = Offset { dx: 9, dy: -11 };
    score.add_connection(connection).unwrap();
    score
        .set_mark_offset(MarkRef::Connection(connection), offset)
        .unwrap();
    score.insert_measure(0).unwrap();
    let shifted = score.connections()[0];
    assert_eq!(shifted.start.measure, 1);
    assert_eq!(score.mark_offset(MarkRef::Connection(shifted)), offset);
    let moved = score.move_event(shifted.end, 1, 96).unwrap();
    let rewritten = score.connections()[0];
    assert_eq!(rewritten.end, moved);
    assert_eq!(score.mark_offset(MarkRef::Connection(rewritten)), offset);
    score.remove_connection(rewritten);
    assert!(score.connection_offsets().is_empty());
    score.validate().unwrap();
}

#[test]
fn orphan_offset_fails_validation() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let orphan = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    score.connection_offsets.push_back(ConnectionOffset {
        connection: orphan,
        offset: Offset { dx: 10, dy: 0 },
    });
    assert!(score.validate().is_err());
}

#[test]
fn stored_zero_duplicate_range_and_absent_mark_offsets_fail_validation() {
    let mut score = Score::default();
    let mut unwritten = note(Step::C);
    unwritten.notation.offsets.dynamic = Some(Offset { dx: 1, dy: 0 });
    let a = score.insert(0, 0, unwritten).unwrap();
    assert!(score.validate().is_err());
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let slur = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    score.add_connection(slur).unwrap();
    score.validate().unwrap();

    for offset in [Offset::ZERO, Offset { dx: 401, dy: 0 }] {
        let mut invalid = score.clone();
        invalid.connection_offsets.push_back(ConnectionOffset {
            connection: slur,
            offset,
        });
        assert!(invalid.validate().is_err());
    }
    let mut duplicate = score.clone();
    let entry = ConnectionOffset {
        connection: slur,
        offset: Offset { dx: 2, dy: 0 },
    };
    duplicate.connection_offsets.push_back(entry);
    duplicate.connection_offsets.push_back(entry);
    assert!(duplicate.validate().is_err());
}

#[test]
fn initial_tempo_offset_sets_resets_and_rejects_invalid_values() {
    let mut score = Score::default();
    let mark = BarMarkRef {
        measure: 0,
        mark: BarMark::Tempo,
    };
    let offset = Offset { dx: 30, dy: -20 };
    assert!(score.has_bar_mark(mark));
    assert_eq!(score.set_bar_mark_offset(mark, offset), Ok(true));
    assert_eq!(score.bar_mark_offset(mark), offset);
    assert_eq!(score.set_bar_mark_offset(mark, offset), Ok(false));
    assert_eq!(score.set_bar_mark_offset(mark, Offset::ZERO), Ok(true));
    assert!(score.bar_offsets().is_empty());
    assert!(
        score
            .set_bar_mark_offset(
                mark,
                Offset {
                    dx: i16::MIN,
                    dy: 0
                }
            )
            .is_err()
    );
    score.validate().unwrap();
}

#[test]
fn orphan_zero_empty_and_out_of_range_bar_offsets_fail_validation() {
    let mut score = Score::default();
    for offsets in [
        BarOffsets {
            rehearsal: Some(Offset { dx: 1, dy: 0 }),
            ..Default::default()
        },
        BarOffsets {
            tempo: Some(Offset::ZERO),
            ..Default::default()
        },
        BarOffsets {
            tempo: Some(Offset { dx: 401, dy: 0 }),
            ..Default::default()
        },
        BarOffsets::default(),
    ] {
        score.bar_offsets.insert(0, offsets);
        assert!(score.validate().is_err());
    }
    score.bar_offsets.clear();
    score.bar_offsets.insert(
        99,
        BarOffsets {
            tempo: Some(Offset { dx: 1, dy: 0 }),
            ..Default::default()
        },
    );
    assert!(score.validate().is_err());
}

#[test]
fn rehearsal_offset_follows_inserted_bar_then_prunes_when_removed() {
    let mut score = Score::default();
    score.toggle_rehearsal_mark(1).unwrap();
    let before = BarMarkRef {
        measure: 1,
        mark: BarMark::Rehearsal,
    };
    let after = BarMarkRef {
        measure: 2,
        mark: BarMark::Rehearsal,
    };
    let offset = Offset { dx: -11, dy: 26 };
    score.set_bar_mark_offset(before, offset).unwrap();
    score.insert_measure(0).unwrap();
    assert_eq!(score.bar_mark_offset(after), offset);
    assert_eq!(score.bar_mark_offset(before), Offset::ZERO);
    score.toggle_rehearsal_mark(2).unwrap();
    score.prune_bar_offsets();
    assert!(score.bar_offsets().is_empty());
    score.validate().unwrap();
}

#[test]
fn form_offset_prunes_only_the_removed_mark() {
    let mut score = Score::default();
    score
        .set_bar_form(
            1,
            BarForm {
                segno: true,
                fine: true,
                ..Default::default()
            },
        )
        .unwrap();
    let segno = BarMarkRef {
        measure: 1,
        mark: BarMark::Segno,
    };
    let fine = BarMarkRef {
        measure: 1,
        mark: BarMark::Fine,
    };
    let offset = Offset { dx: 5, dy: 6 };
    score.set_bar_mark_offset(segno, offset).unwrap();
    score.set_bar_mark_offset(fine, offset).unwrap();
    score
        .set_bar_form(
            1,
            BarForm {
                fine: true,
                ..Default::default()
            },
        )
        .unwrap();
    score.prune_bar_offsets();
    assert_eq!(score.bar_mark_offset(segno), Offset::ZERO);
    assert_eq!(score.bar_mark_offset(fine), offset);
    score.validate().unwrap();
}

#[test]
fn ending_offset_remaps_after_ending_start_rekeys() {
    let mut score = Score::default();
    score
        .set_ending(Ending {
            start: 1,
            end: 2,
            numbers: vec![1],
        })
        .unwrap();
    let before = BarMarkRef {
        measure: 1,
        mark: BarMark::Ending,
    };
    let after = BarMarkRef {
        measure: 2,
        mark: BarMark::Ending,
    };
    let offset = Offset { dx: 14, dy: 3 };
    score.set_bar_mark_offset(before, offset).unwrap();
    score.insert_measure(0).unwrap();
    assert_eq!(score.bar_mark_offset(after), offset);
    score.validate().unwrap();
}
