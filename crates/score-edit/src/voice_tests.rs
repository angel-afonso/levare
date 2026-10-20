use super::*;
use score_core::{Connection, ConnectionKind, Division, Duration, MarkKind, Voice};
fn note(step: score_core::Step, value: Duration) -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(step, 4)),
        duration: value,
        notation: Default::default(),
    }
}
#[test]
fn voices_have_independent_capacity_navigation_selection_clipboard_and_history() {
    use score_core::Step::*;
    let mut e = Editor::default();
    for step in [C, D, E, F] {
        e.execute(Command::Insert(note(step, Duration::Quarter)))
            .unwrap();
    }
    let one = e.score().clone();
    e.select_voice(Voice::Two);
    e.execute(Command::Insert(note(C, Duration::Half))).unwrap();
    let first = e.selection().unwrap();
    assert_eq!(e.score().event_tick(first), Some(0));
    e.execute(Command::Insert(note(G, Duration::Half))).unwrap();
    let second = e.selection().unwrap();
    assert_eq!(e.score().event_tick(second), Some(96));
    assert_eq!(e.score().measures()[0].len(), 6);
    assert_eq!(e.score().measures()[0].used_ticks(), 192);
    e.select_all();
    assert_eq!(e.selected_ids(), [first, second]);
    let clip = e.copy_selection().unwrap();
    assert_eq!(clip.events.len(), 2);
    assert_eq!(clip.events[1].offset, 96);
    e.execute(Command::Erase).unwrap();
    assert!(e.score().event(first).unwrap().pitches.is_empty());
    assert_eq!(
        e.score().measures()[0]
            .voice_iter(Voice::One)
            .collect::<Vec<_>>(),
        one.measures()[0].voice_iter(Voice::One).collect::<Vec<_>>()
    );
    e.undo();
    assert_eq!(e.selected_ids(), [first, second]);
    e.set_cursor(Cursor {
        voice: Voice::Two,
        measure: 1,
        index: 0,
        ..e.cursor()
    });
    e.paste(&clip).unwrap();
    assert_eq!(e.score().measures()[1].voice_len(Voice::Two), 2);
    assert_eq!(e.score().measures()[1].voice_len(Voice::One), 0);
    assert_eq!(e.cursor().voice, Voice::Two);
    e.score().validate().unwrap();
    e.undo();
    assert!(e.score().measures()[1].is_empty());
    e.select(first);
    e.navigate(true);
    assert_eq!(e.selection(), Some(second));
    assert_eq!(e.cursor().voice, Voice::Two);
}
#[test]
fn independent_overlapping_slurs_remain_voice_local_and_divisi_preserves_music() {
    use score_core::Step::*;
    let mut score = Score::default();
    let violin = score.add_part(Instrument::Violin).unwrap();
    let mut e = Editor::new(score).unwrap();
    e.select_part(violin);
    let mut voices = Vec::new();
    for voice in Voice::ALL {
        e.select_voice(voice);
        e.execute(Command::Insert(note(C, Duration::Half))).unwrap();
        let a = e.selection().unwrap();
        e.execute(Command::Insert(note(D, Duration::Half))).unwrap();
        let b = e.selection().unwrap();
        e.execute(Command::AddConnection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Slur,
        }))
        .unwrap();
        voices.push((a, b));
    }
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    let before = e.score().clone();
    let cursor = e.cursor();
    e.select_all();
    e.execute(Command::SetPartDivisi(true)).unwrap();
    let divided = e.score().part(violin).unwrap();
    assert!(divided.is_divisi());
    assert_eq!(divided.staves().len(), 2);
    assert_eq!(divided.clef(StaffId::Lower), score_core::Clef::Treble);
    assert_eq!(
        divided.staff(StaffId::Upper).unwrap().measures()[0].voice_len(Voice::One),
        2
    );
    assert_eq!(
        divided.staff(StaffId::Lower).unwrap().measures()[0].voice_len(Voice::One),
        2
    );
    assert_eq!(e.cursor().voice, Voice::One);
    assert_eq!(e.cursor().staff, StaffId::Lower);
    let moved = ElementId {
        staff: StaffId::Lower,
        ..voices[1].1
    };
    assert_eq!(e.selection(), Some(moved));
    assert_eq!(
        e.score().event(moved).unwrap().notation.division,
        Some(Division::Divisi)
    );
    e.score().validate().unwrap();
    e.execute(Command::SetPartDivisi(false)).unwrap();
    assert_eq!(e.cursor().voice, Voice::Two);
    assert_eq!(e.cursor().staff, StaffId::Upper);
    assert_eq!(e.score().connections().len(), 2);
    assert_eq!(e.score().measures()[0].len(), 0); // Unrelated piano remains untouched.
    assert_eq!(
        e.score().part(violin).unwrap().measures()[0].voice_len(Voice::Two),
        2
    );
    e.score().validate().unwrap();
    e.undo();
    e.undo();
    assert_eq!(e.score(), &before);
    assert_eq!(e.cursor(), cursor);
    e.redo();
    assert!(e.score().part(violin).unwrap().is_divisi());
}
#[test]
fn rejoining_rejects_unrepresentable_extra_voices_without_losing_work() {
    use score_core::Step::*;
    let mut s = Score::default();
    let part = s.add_part(Instrument::Cello).unwrap();
    s.set_part_divisi(part, true).unwrap();
    s.insert_in_voice(
        part,
        StaffId::Upper,
        0,
        Voice::Two,
        0,
        note(C, Duration::Whole),
    )
    .unwrap();
    s.insert_on(part, StaffId::Lower, 0, 0, note(G, Duration::Whole))
        .unwrap();
    let before = s.clone();
    assert!(s.set_part_divisi(part, false).is_err());
    assert_eq!(s, before);
    s.validate().unwrap();
}
#[test]
fn duration_edits_never_consume_the_other_voice_capacity_or_break_its_links() {
    use score_core::Step::*;
    let mut e = Editor::default();
    e.execute(Command::Insert(note(C, Duration::Whole)))
        .unwrap();
    let upper = e.selection().unwrap();
    e.select_voice(Voice::Two);
    e.execute(Command::Insert(note(E, Duration::Quarter)))
        .unwrap();
    let lower = e.selection().unwrap();
    e.execute(Command::SetDuration(score_core::NoteValue::Whole))
        .unwrap();
    assert_eq!(e.score().event_tick(lower), Some(0));
    assert_eq!(e.score().event(upper).unwrap().duration, Duration::Whole);
    assert_eq!(e.score().event(lower).unwrap().duration, Duration::Whole);
    e.score().validate().unwrap();
}

#[test]
fn removing_time_stays_in_voice_two_and_copy_keeps_a_simultaneous_shared_dynamic() {
    use score_core::{Dynamic, Step::*};
    let mut e = Editor::default();
    let mut one = note(C, Duration::Whole);
    one.notation.dynamic = Some(Dynamic::Pp);
    e.execute(Command::Insert(one)).unwrap();
    e.select_voice(Voice::Two);
    e.execute(Command::Insert(note(E, Duration::Quarter)))
        .unwrap();
    let first = e.selection().unwrap();
    e.execute(Command::Insert(note(F, Duration::Quarter)))
        .unwrap();
    e.execute(Command::Delete).unwrap();
    assert_eq!(e.cursor().voice, Voice::Two);
    e.execute(Command::Insert(note(G, Duration::Quarter)))
        .unwrap();
    assert_eq!(e.score().measures()[0].voice_len(Voice::One), 1);
    assert_eq!(e.score().measures()[0].voice_len(Voice::Two), 2);
    e.select(first);
    assert_eq!(e.copy_selection().unwrap().initial_dynamic, Dynamic::Pp);
}

#[test]
fn selected_divisi_passage_and_unis_boundaries_keep_both_voices_and_undo() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    while s.measures().len() < 6 {
        s.append_measure().unwrap();
    }
    let mut ids = Vec::new();
    for bar in 0..6 {
        for voice in Voice::ALL {
            ids.push(
                s.insert_in_voice(
                    p,
                    StaffId::Upper,
                    bar,
                    voice,
                    0,
                    note(score_core::Step::C, Duration::Whole),
                )
                .unwrap(),
            );
        }
    }
    s.add_connection(Connection {
        start: ids[1],
        end: ids[11],
        kind: ConnectionKind::Slur,
    })
    .unwrap();
    let before = s.clone();
    let mut e = Editor::new(s).unwrap();
    e.select(ids[2]);
    e.extend_selection(ids[6]);
    e.execute(Command::SetPassageDivisi(true)).unwrap();
    assert_eq!(
        (0..6)
            .map(|bar| e.score().part(p).unwrap().divisi_at(bar))
            .collect::<Vec<_>>(),
        [false, true, true, true, false, false]
    );
    for &id in &ids {
        assert_eq!(
            e.score().event(id).unwrap().pitches,
            before.event(id).unwrap().pitches
        );
    }
    assert_eq!(e.score().connections(), before.connections());
    assert_eq!(e.selected_ids().len(), 3);
    e.undo();
    assert_eq!(e.score(), &before);
    e.redo();
    e.select(ids[4]);
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Unison)))
        .unwrap();
    assert!(!e.score().part(p).unwrap().divisi_at(2));
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Unison)))
        .unwrap();
    assert!(e.score().part(p).unwrap().divisi_at(2));
    // Empty bars can also start and close a passage, without inserting fake notes.
    e.set_cursor(Cursor {
        part: p,
        measure: 5,
        ..Default::default()
    });
    e.execute(Command::SetPassageDivisi(true)).unwrap();
    assert!(e.score().part(p).unwrap().divisi_at(5));
    e.score().validate().unwrap();
}

#[test]
fn overwriting_a_divisi_boundary_never_resurrects_old_note_labels() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    let a = s
        .insert_in(p, 0, 0, note(score_core::Step::C, Duration::Whole))
        .unwrap();
    let b = s
        .insert_in(p, 1, 0, note(score_core::Step::C, Duration::Whole))
        .unwrap();
    let c = s
        .insert_in(p, 2, 0, note(score_core::Step::C, Duration::Whole))
        .unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(b);
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    e.select(a);
    e.extend_selection(c);
    e.execute(Command::SetPassageDivisi(false)).unwrap();
    assert_eq!(e.score().mark_kind(score_core::MarkRef::Division(b)), None);
    e.select(a);
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Unison)))
        .unwrap();
    e.set_cursor(Cursor {
        part: p,
        measure: 3,
        ..Default::default()
    });
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap(); // Replaces the restore boundary, if present.
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap(); // Removes it.
    assert_eq!(e.score().mark_kind(score_core::MarkRef::Division(b)), None);
    assert!(e.score().event(b).unwrap().notation.division.is_none());
}

#[test]
fn midbar_divisi_unis_and_selected_passages_use_exact_note_positions_and_undo() {
    use score_core::{DivisiPoint, Step};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let mut ids = Vec::new();
    for i in 0..4 {
        ids.push(
            s.insert_in(p, 0, i, note(Step::G, Duration::Quarter))
                .unwrap(),
        );
    }
    s.insert_in_voice(
        p,
        StaffId::Upper,
        0,
        Voice::Two,
        0,
        note(Step::C, Duration::Whole),
    )
    .unwrap();
    let before = s.clone();
    let mut e = Editor::new(s).unwrap();
    e.select(ids[1]);
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    e.select(ids[3]);
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Unison)))
        .unwrap();
    let part = e.score().part(p).unwrap();
    assert_eq!(
        part.divisi_changes_in(0).collect::<Vec<_>>(),
        [(48, Division::Divisi), (144, Division::Unison)]
    );
    assert!(!part.divisi_at_tick(0, 47));
    assert!(part.divisi_at_tick(0, 48));
    assert!(!part.divisi_at_tick(0, 144));
    assert_eq!(part.measures()[0].voice_len(Voice::Two), 1);
    e.undo();
    e.undo();
    assert_eq!(e.score(), &before);
    e.select(ids[1]);
    e.extend_selection(ids[2]);
    e.execute(Command::SetPassageDivisi(true)).unwrap();
    assert_eq!(
        e.score()
            .part(p)
            .unwrap()
            .divisi_changes_in(0)
            .collect::<Vec<_>>(),
        [(48, Division::Divisi), (144, Division::Unison)]
    );
    assert_eq!(
        e.score().part(p).unwrap().measures()[0].voice_len(Voice::Two),
        1
    );
    // A filled bar's insertion cursor belongs to the following bar for toggles.
    e.set_cursor(Cursor {
        part: p,
        index: 4,
        ..Default::default()
    });
    assert_eq!(e.divisi_position(), DivisiPoint::new(1, 0));
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    assert!(e.score().part(p).unwrap().divisi_change_at(1).is_none());
}

#[test]
fn removing_a_midbar_boundary_after_rhythm_edits_never_resurrects_mirror_labels() {
    use score_core::{DivisiPoint, MarkRef, Step};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    let a = s
        .insert_in(p, 0, 0, note(Step::C, Duration::Quarter))
        .unwrap();
    let b = s
        .insert_in(p, 0, 1, note(Step::D, Duration::Quarter))
        .unwrap();
    s.set_division(b, Some(Division::Divisi)).unwrap();
    s.set_durations(&[(a, Duration::Half)]).unwrap();
    s.remove_divisi_change_at(p, DivisiPoint::new(0, 48))
        .unwrap();
    assert!(!s.part(p).unwrap().has_passage_divisi());
    assert_eq!(s.mark_kind(MarkRef::Division(b)), None);
    let before = s.clone();
    s.insert_in_voice(
        p,
        StaffId::Upper,
        0,
        Voice::Two,
        0,
        note(Step::C, Duration::Whole),
    )
    .unwrap();
    let before_cut = s.clone();
    assert!(
        s.set_divisi_span(p, DivisiPoint::new(0, 4), None, true)
            .is_err()
    );
    assert_eq!(s, before_cut);
    assert!(
        before
            .part(p)
            .unwrap()
            .divisi_change_at_tick(0, 48)
            .is_none()
    );
}

#[test]
fn selecting_a_held_voice_keeps_exact_midbar_positions_for_editing_and_undo() {
    use score_core::{DivisiPoint, Step};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
    let mut upper = Vec::new();
    for i in 0..4 {
        upper.push(
            s.insert_in(p, 0, i, note(Step::G, Duration::Quarter))
                .unwrap(),
        );
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
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(upper[3]);
    e.select_display_staff(StaffId::Lower);
    assert_eq!(e.selection(), Some(held));
    assert_eq!(e.divisi_position(), DivisiPoint::new(0, 144));
    e.select_divisi_position(DivisiPoint::new(0, 48));
    assert_eq!(e.divisi_position(), DivisiPoint::new(0, 48));
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Unison)))
        .unwrap();
    assert_eq!(
        e.score().part(p).unwrap().divisi_change_at_tick(0, 48),
        Some(Division::Unison)
    );
    e.undo();
    assert_eq!(e.divisi_position(), DivisiPoint::new(0, 48));
    e.select(held);
    e.select_divisi_position(DivisiPoint::new(0, 144));
    e.execute(Command::ToggleMark(MarkKind::Division(Division::Divisi)))
        .unwrap();
    assert_eq!(
        e.score().part(p).unwrap().divisi_change_at_tick(0, 144),
        Some(Division::Divisi)
    );
    assert_eq!(e.score().event(held).unwrap().duration, Duration::Whole);
}

#[test]
fn paste_clears_continuation_position_and_undo_restores_it() {
    use score_core::{DivisiPoint, Step};
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(s.parts()[0].id()).unwrap();
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
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(held);
    e.select_divisi_position(DivisiPoint::new(0, 48));
    let clip = e.copy_selection().unwrap();
    e.paste(&clip).unwrap();
    assert_eq!(e.selection().unwrap().measure, 1);
    assert_eq!(e.divisi_position(), DivisiPoint::new(1, 0));
    e.undo();
    assert_eq!(e.divisi_position(), DivisiPoint::new(0, 48));
    e.redo();
    assert_eq!(e.divisi_position(), DivisiPoint::new(1, 0));
}

#[test]
fn lower_staff_navigation_preserves_visible_notes_outside_midbar_divisi() {
    use score_core::{DivisiPoint, Step};
    let mut s = Score::default();
    let piano = s.parts()[0].id();
    let p = s.add_part(Instrument::Violin).unwrap();
    s.remove_part(piano).unwrap();
    let upper = s
        .insert_in(p, 0, 0, note(Step::G, Duration::Whole))
        .unwrap();
    let mut lower = vec![];
    for i in 0..4 {
        lower.push(
            s.insert_in_voice(
                p,
                StaffId::Upper,
                0,
                Voice::Two,
                i,
                note(Step::C, Duration::Quarter),
            )
            .unwrap(),
        );
    }
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(upper);
    e.select_display_staff(StaffId::Lower);
    assert_eq!(e.selection(), Some(lower[0]));
    assert_eq!(e.divisi_position(), DivisiPoint::new(0, 0));
}
