use super::*;
use score_core::{Pitches, Step};

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
    assert_eq!(editor.score().event_tick(d), Some(old_tick + 48));
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
            tick: 96,
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
    assert_eq!(events.len(), 2);
    assert!(events[0].pitches.is_empty());
    assert_eq!(events[0].duration, Duration::Half);
    assert_eq!(events[1].pitches, event(Step::C, Duration::Quarter).pitches);
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
