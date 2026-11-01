use super::*;
use score_core::{NoteValue, Step, Tuplet, Voice};

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;

/// Four quarter notes in the first bar, and their ids.
fn four_quarters() -> (Editor, Vec<ElementId>) {
    let mut editor = Editor::default();
    let mut ids = Vec::new();
    for step in [Step::C, Step::D, Step::E, Step::F] {
        editor.execute(Command::Write(editor.note(step))).unwrap();
        ids.push(editor.selection().unwrap());
    }
    (editor, ids)
}

/// Each event of the first bar: its onset, duration and whether it sounds.
fn bar(editor: &Editor) -> Vec<(u32, Duration, bool)> {
    editor.score().measures()[0]
        .timed_iter()
        .filter(|(voice, ..)| *voice == Voice::One)
        .map(|(_, tick, _, event)| (tick, event.duration, !event.pitches.is_empty()))
        .collect()
}

#[test]
fn a_tuplet_on_one_note_starts_a_group_over_what_follows() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    let before = editor.score().clone();
    editor.execute(Command::SetTuplet(Some((3, 2)))).unwrap();
    // Every event of the group counts in quarters, the note's value.
    let triplet =
        |value: Duration| value.with_tuplet(Some(score_core::Tuplet::triplet(NoteValue::Quarter)));
    assert_eq!(
        bar(&editor),
        [
            (0, triplet(Duration::Quarter), true),
            (2 * Q / 3, triplet(Duration::Half), false),
            (2 * Q, Duration::Quarter, true),
            (3 * Q, Duration::Quarter, true)
        ]
    );
    let selected = editor.selection().unwrap();
    assert_eq!(editor.score().event_tick(selected), Some(0));
    editor.undo();
    assert_eq!(editor.score(), &before);
}

#[test]
fn taking_the_tuplet_away_turns_its_whole_group_into_rests() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor.execute(Command::SetTuplet(Some((5, 4)))).unwrap();
    let grouped = editor.score().clone();
    // Any event of the group will do.
    let second = editor.score().measures()[0]
        .voice_id_at(Voice::One, 1)
        .unwrap();
    editor.select(ElementId {
        event: second,
        ..ids[0]
    });
    editor.execute(Command::SetTuplet(None)).unwrap();
    // Five quarters in the time of four filled a whole bar.
    assert_eq!(bar(&editor), [(0, Duration::Whole, false)]);
    editor.undo();
    assert_eq!(editor.score(), &grouped);
}

#[test]
fn the_same_tuplet_again_or_none_on_plain_music_changes_nothing() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[1]);
    assert!(!editor.execute(Command::SetTuplet(None)).unwrap());
    editor.execute(Command::SetTuplet(Some((3, 2)))).unwrap();
    assert!(!editor.execute(Command::SetTuplet(Some((3, 2)))).unwrap());
}

#[test]
fn ratios_outside_the_supported_range_are_refused() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    let before = editor.score().clone();
    for ratio in [(3, 3), (5, 1), (10, 8)] {
        assert!(editor.execute(Command::SetTuplet(Some(ratio))).is_err());
    }
    assert_eq!(editor.score(), &before);
}

#[test]
fn a_tuplet_inside_another_group_is_refused_unless_it_starts_it() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[0]);
    editor.execute(Command::SetTuplet(Some((3, 2)))).unwrap();
    let rest = editor.score().measures()[0]
        .voice_id_at(Voice::One, 1)
        .unwrap();
    editor.select(ElementId {
        event: rest,
        ..ids[0]
    });
    assert!(editor.execute(Command::SetTuplet(Some((5, 4)))).is_err());
    let first = editor.score().measures()[0]
        .voice_id_at(Voice::One, 0)
        .unwrap();
    editor.select(ElementId {
        event: first,
        ..ids[0]
    });
    editor.execute(Command::SetTuplet(Some((5, 4)))).unwrap();
    let groups = editor.score().measures()[0].tuplet_groups(Voice::One);
    assert_eq!(groups.len(), 1);
    assert_eq!(
        groups[0].tuplet,
        score_core::Tuplet::new(5, 4, NoteValue::Quarter).unwrap()
    );
}

#[test]
fn two_dots_lengthen_a_value_by_three_quarters() {
    let (mut editor, ids) = four_quarters();
    editor.select(ids[3]);
    editor
        .execute(Command::SetDuration(NoteValue::Eighth))
        .unwrap();
    editor.select(ids[0]);
    editor
        .execute(Command::SetDuration(NoteValue::Half))
        .unwrap_err();
    editor.execute(Command::SetDots(2)).unwrap_err();
    editor.select(ids[2]);
    editor
        .execute(Command::SetDuration(NoteValue::Eighth))
        .unwrap();
    editor.select(ids[1]);
    editor.execute(Command::SetDots(2)).unwrap();
    let duration = editor.score().event(ids[1]).unwrap().duration;
    assert_eq!(duration.dots(), 2);
    assert_eq!(duration.ticks(), 7 * Q / 4);
}

#[test]
fn random_edits_keep_the_score_valid_and_undo_restores_it() {
    let values = [
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
        NoteValue::ThirtySecond,
    ];
    let ratios = [
        None,
        None,
        Some((3, 2)),
        Some((5, 4)),
        Some((6, 4)),
        Some((2, 3)),
    ];
    for seed in [1u64, 7, 42, 1234] {
        // xorshift: the same edits on every run.
        let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
        let mut roll = |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        let mut editor = Editor::default();
        let (mut applied, mut grouped) = (0, 0);
        for _ in 0..400 {
            let value = values[roll(values.len())];
            let ratio = ratios[roll(ratios.len())];
            let tuplet = ratio.and_then(|(actual, normal)| Tuplet::new(actual, normal, value));
            let event = Event {
                pitches: if roll(4) == 0 {
                    score_core::Pitches::default()
                } else {
                    score_core::Pitches::single(Pitch::new(Step::G, 4))
                },
                duration: Duration::Quarter.with_value(value).with_tuplet(tuplet),
                notation: Default::default(),
            };
            let command = match roll(8) {
                0..=2 => Command::Write(event),
                3 => Command::SetTuplet(ratio),
                4 => Command::MoveSelectedEvent {
                    measure: roll(2),
                    tick: roll(48) as u32 * Q / 12,
                },
                5 => Command::Delete,
                6 => Command::SetDots(roll(3) as u8),
                _ => {
                    editor.navigate(roll(3) != 0);
                    continue;
                }
            };
            let before = editor.score().clone();
            match editor.execute(command.clone()) {
                Ok(true) => {
                    applied += 1;
                    editor.score().validate().unwrap();
                    let after = editor.score().clone();
                    editor.undo();
                    assert_eq!(editor.score(), &before, "undo after {command:?}");
                    editor.redo();
                    assert_eq!(editor.score(), &after, "redo after {command:?}");
                }
                Ok(false) | Err(_) => assert_eq!(editor.score(), &before, "{command:?}"),
            }
            grouped += editor
                .score()
                .measures()
                .iter()
                .filter(|bar| !bar.tuplet_groups(Voice::One).is_empty())
                .count();
        }
        // Enough of the edits apply, and tuplets are among them.
        assert!(applied > 60, "seed {seed}: {applied} edits applied");
        assert!(grouped > 0, "seed {seed}: no tuplets");
    }
}
