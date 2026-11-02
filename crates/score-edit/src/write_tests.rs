use super::*;
use score_core::{Pitches, Step};

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;

fn event(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Default::default(),
    }
}

#[test]
fn insert_in_the_middle_still_shifts_later_notes() {
    let mut score = Score::default();
    score
        .insert(0, 0, event(Step::C, Duration::Quarter))
        .unwrap();
    let d = score
        .insert(0, 1, event(Step::D, Duration::Quarter))
        .unwrap();
    let old_tick = score.event_tick(d).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.set_cursor(Cursor {
        index: 1,
        ..Cursor::default()
    });
    editor
        .execute(Command::Insert(event(Step::E, Duration::Quarter)))
        .unwrap();
    let bar = &editor.score().measures()[0];
    let steps: Vec<_> = bar
        .voice_iter(score_core::Voice::One)
        .map(|(_, e)| e.pitches.iter().next().unwrap().step)
        .collect();
    assert_eq!(steps, [Step::C, Step::E, Step::D]);
    assert_eq!(editor.score().event_tick(d), Some(old_tick + u64::from(Q)));
}

#[test]
fn insert_into_a_full_bar_before_its_end_is_rejected_atomically() {
    let mut score = Score::default();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        score
            .insert(0, index, event(step, Duration::Quarter))
            .unwrap();
    }
    let mut editor = Editor::new(score.clone()).unwrap();
    editor.set_cursor(Cursor {
        index: 1,
        ..Cursor::default()
    });
    assert!(
        editor
            .execute(Command::Insert(event(Step::G, Duration::Quarter)))
            .is_err()
    );
    assert_eq!(editor.score(), &score);
    assert!(!editor.can_undo());
}

#[test]
fn insert_at_the_end_of_a_full_bar_moves_to_a_new_bar() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        score
            .insert(0, index, event(step, Duration::Quarter))
            .unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    editor.set_cursor(Cursor {
        index: 4,
        ..Cursor::default()
    });
    editor
        .execute(Command::Insert(event(Step::G, Duration::Quarter)))
        .unwrap();
    assert_eq!(editor.score().measures().len(), 2);
    assert_eq!(editor.selection().unwrap().measure, 1);
    assert_eq!(editor.cursor().measure, 1);
}

#[test]
fn write_over_a_rest_bar_keeps_one_history_step() {
    let mut score = Score::default();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        score
            .insert(0, index, event(step, Duration::Quarter))
            .unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    assert!(editor.select_all());
    editor.execute(Command::Erase).unwrap();
    let erased = editor.score().clone();
    editor.set_cursor(Cursor::default());
    editor
        .execute(Command::Write(event(Step::C, Duration::Quarter)))
        .unwrap();
    let events: Vec<_> = editor.score().measures()[0]
        .voice_iter(score_core::Voice::One)
        .map(|(_, e)| *e)
        .collect();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].pitches, event(Step::C, Duration::Quarter).pitches);
    assert!(
        events[1..]
            .iter()
            .all(|e| e.pitches.is_empty() && e.duration == Duration::Quarter)
    );
    assert!(editor.undo());
    assert_eq!(editor.score(), &erased);
}

#[test]
fn write_replaces_following_notes_and_advances_the_cursor() {
    let mut score = Score::default();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        score
            .insert(0, index, event(step, Duration::Quarter))
            .unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    editor.set_cursor(Cursor {
        index: 1,
        ..Cursor::default()
    });
    editor
        .execute(Command::Write(event(Step::G, Duration::Half)))
        .unwrap();
    let events: Vec<_> = editor.score().measures()[0]
        .voice_iter(score_core::Voice::One)
        .map(|(_, e)| *e)
        .collect();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].pitches, event(Step::C, Duration::Quarter).pitches);
    assert_eq!(events[1].pitches, event(Step::G, Duration::Half).pitches);
    assert_eq!(events[2].pitches, event(Step::F, Duration::Quarter).pitches);
    assert_eq!(
        editor.selection().unwrap().event,
        editor.score().measures()[0]
            .voice_id_at(score_core::Voice::One, 1)
            .unwrap()
    );
    assert_eq!(editor.cursor().index, 2);
}

#[test]
fn write_in_a_full_bar_at_its_end_continues_in_the_next_bar() {
    let mut score = Score::new("".into(), &[Instrument::Violin], 1).unwrap();
    for (index, step) in [Step::C, Step::D, Step::E, Step::F].into_iter().enumerate() {
        score
            .insert(0, index, event(step, Duration::Quarter))
            .unwrap();
    }
    let mut editor = Editor::new(score).unwrap();
    editor.set_cursor(Cursor {
        index: 4,
        ..Cursor::default()
    });
    editor
        .execute(Command::Write(event(Step::G, Duration::Quarter)))
        .unwrap();
    assert_eq!(editor.score().measures().len(), 2);
    assert_eq!(editor.selection().unwrap().measure, 1);
    assert_eq!(editor.cursor().measure, 1);
    assert_eq!(editor.cursor().index, 1);
}

#[test]
fn write_at_tick_uses_the_cursor_part_staff_and_voice() {
    let mut score = Score::new("".into(), &[Instrument::Piano], 1).unwrap();
    let lower = score
        .insert_in_voice(
            score_core::PartId(0),
            StaffId::Lower,
            0,
            score_core::Voice::Two,
            0,
            event(Step::E, Duration::Quarter),
        )
        .unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.set_cursor(Cursor {
        staff: StaffId::Lower,
        voice: score_core::Voice::One,
        ..Cursor::default()
    });
    editor
        .execute(Command::WriteAt {
            measure: 0,
            tick: 2 * Q,
            event: event(Step::C, Duration::Quarter),
        })
        .unwrap();
    let bar = &editor
        .score()
        .part(score_core::PartId(0))
        .unwrap()
        .staff(StaffId::Lower)
        .unwrap()
        .measures()[0];
    let events: Vec<_> = bar
        .voice_iter(score_core::Voice::One)
        .map(|(_, e)| *e)
        .collect();
    // A half rest before it and a quarter rest to end the bar.
    assert_eq!(events.len(), 3);
    assert!(events[0].pitches.is_empty());
    assert_eq!(events[0].duration, Duration::Half);
    assert_eq!(events[1].pitches, event(Step::C, Duration::Quarter).pitches);
    assert!(events[2].pitches.is_empty());
    assert_eq!(events[2].duration, Duration::Quarter);
    assert!(editor.score().event(lower).is_some());
    assert_eq!(editor.selection().unwrap().staff, StaffId::Lower);
}

#[test]
fn chord_entry_after_write_still_adds_pitches() {
    let score = Score::default();
    let mut editor = Editor::new(score).unwrap();
    editor
        .execute(Command::Write(event(Step::C, Duration::Quarter)))
        .unwrap();
    editor
        .execute(Command::AddPitch(Pitch::new(Step::E, 4).unwrap()))
        .unwrap();
    let selected = editor.selection().unwrap();
    assert_eq!(editor.score().event(selected).unwrap().pitches.len(), 2);
}

#[test]
fn noteheads_and_beams_toggle_on_the_notes_of_a_passage_and_undo() {
    use score_core::{Beam, Notehead};
    let mut score = Score::default();
    let a = score
        .insert(0, 0, event(Step::C, Duration::Quarter))
        .unwrap();
    score
        .insert(
            0,
            1,
            Event {
                pitches: Pitches::default(),
                ..event(Step::C, Duration::Quarter)
            },
        )
        .unwrap();
    let b = score
        .insert(0, 2, event(Step::E, Duration::Quarter))
        .unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(a);
    editor.extend_selection(b);
    let before = editor.score().clone();
    editor
        .execute(Command::ToggleNotehead(Notehead::Cross))
        .unwrap();
    for id in [a, b] {
        assert_eq!(
            editor.score().event(id).unwrap().notation.notehead,
            Some(Notehead::Cross)
        );
    }
    // The same head again restores the oval on every note.
    editor
        .execute(Command::ToggleNotehead(Notehead::Cross))
        .unwrap();
    assert_eq!(editor.score(), &before);
    editor.execute(Command::ToggleBeam(Beam::Off)).unwrap();
    assert_eq!(
        editor.score().event(b).unwrap().notation.beam,
        Some(Beam::Off)
    );
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.select(
        editor.score().measures()[0]
            .id_at(1)
            .map(|event| ElementId { event, ..a })
            .unwrap(),
    );
    assert!(
        editor
            .execute(Command::ToggleNotehead(Notehead::Slash))
            .is_err()
    );
}

#[test]
fn bar_repeats_toggle_on_the_selected_bars_and_writing_replaces_them() {
    let mut score = Score::default();
    let ids: Vec<_> = (0..3)
        .map(|bar| {
            score
                .insert(bar, 0, event(Step::C, Duration::Quarter))
                .unwrap()
        })
        .collect();
    let (second, third) = (ids[1], ids[2]);
    let mut editor = Editor::new(score).unwrap();
    let before = editor.score().clone();
    editor.select(second);
    editor.extend_selection(third);
    editor.execute(Command::ToggleBarRepeat).unwrap();
    let bars = |editor: &Editor| {
        editor
            .score()
            .measures()
            .iter()
            .map(|m| m.repeats_previous())
            .collect::<Vec<_>>()
    };
    assert_eq!(bars(&editor), [false, true, true, false]);
    assert!(editor.score().measures()[1].is_empty());
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.redo();
    // Writing into a % bar puts music back in it.
    editor.set_cursor(Cursor {
        measure: 2,
        index: 0,
        ..Cursor::default()
    });
    editor
        .execute(Command::Insert(event(Step::D, Duration::Quarter)))
        .unwrap();
    assert_eq!(bars(&editor), [false, true, false, false]);
    // The first bar has nothing to repeat.
    editor.set_cursor(Cursor::default());
    assert!(editor.execute(Command::ToggleBarRepeat).is_err());
}

#[test]
fn a_passage_transposes_by_an_interval_and_undoes_in_one_step() {
    let mut score = Score::default();
    let ids: Vec<_> = [Step::C, Step::E, Step::G]
        .into_iter()
        .enumerate()
        .map(|(index, step)| {
            score
                .insert(0, index, event(step, Duration::Quarter))
                .unwrap()
        })
        .collect();
    let mut editor = Editor::new(score).unwrap();
    assert!(
        editor
            .execute(Command::TransposeBy(score_core::Interval::new(1, 2)))
            .is_err()
    );
    editor.select(ids[0]);
    editor.extend_selection(ids[2]);
    let before = editor.score().clone();
    // A minor third down: C–E–G becomes A–C♯–E.
    editor
        .execute(Command::TransposeBy(score_core::Interval::new(2, 3).down()))
        .unwrap();
    let steps: Vec<_> = ids
        .iter()
        .map(|id| editor.score().event(*id).unwrap().pitches.first().unwrap())
        .map(|pitch| (pitch.step, pitch.octave(), pitch.alteration()))
        .collect();
    assert_eq!(steps, [(Step::A, 3, 0), (Step::C, 4, 1), (Step::E, 4, 0)]);
    // The passage stays selected for the next step, and one undo restores it.
    assert_eq!(editor.selected_ids(), ids);
    editor.undo();
    assert_eq!(editor.score(), &before);
}

#[test]
fn stems_and_curves_flip_by_hand_save_and_undo() {
    use score_core::{Connection, ConnectionKind, Stem};
    let mut score = Score::default();
    let a = score
        .insert(0, 0, event(Step::C, Duration::Quarter))
        .unwrap();
    let b = score
        .insert(0, 1, event(Step::C, Duration::Quarter))
        .unwrap();
    let tie = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    };
    score.add_connection(tie).unwrap();
    let mut editor = Editor::new(score).unwrap();
    let before = editor.score().clone();
    editor
        .execute(Command::SetStems(vec![
            (a, Some(Stem::Down)),
            (b, Some(Stem::Down)),
        ]))
        .unwrap();
    editor.execute(Command::FlipCurve(tie)).unwrap();
    assert_eq!(
        editor.score().event(a).unwrap().notation.stem,
        Some(Stem::Down)
    );
    assert!(editor.score().curve_flipped(tie));
    let saved = editor.score().clone();
    assert_eq!(
        serde_json::from_value::<Score>(serde_json::to_value(&saved).unwrap()).unwrap(),
        saved
    );
    assert!(editor.execute(Command::SetStems(Vec::new())).is_err());
    editor.undo();
    editor.undo();
    assert_eq!(editor.score(), &before);
}

#[test]
fn respelling_a_chord_tone_leaves_the_others() {
    let mut score = Score::default();
    let chord = Event {
        pitches: Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::E, 4).unwrap(),
            Pitch::new(Step::G, 4)
                .unwrap()
                .with_accidental(Some(score_core::Accidental::Sharp)),
        ])
        .unwrap(),
        ..event(Step::C, Duration::Quarter)
    };
    let id = score.insert(0, 0, chord).unwrap();
    let mut editor = Editor::new(score).unwrap();
    editor.select(id);
    editor.select_tone(2);
    editor.execute(Command::Respell).unwrap();
    let names: Vec<_> = editor
        .score()
        .event(id)
        .unwrap()
        .pitches
        .iter()
        .map(|p| (p.step, p.alteration()))
        .collect();
    assert_eq!(names, [(Step::C, 0), (Step::E, 0), (Step::A, -1)]);
    editor.undo();
    assert!(editor.execute(Command::Respell).is_ok());
}
