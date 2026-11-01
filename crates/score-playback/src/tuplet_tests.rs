use super::*;
use score_core::{
    Connection, ConnectionKind, Duration, ElementId, Event, NoteValue, PartId, Pitch, Pitches,
    StaffId, Step, Tuplet, Voice,
};

/// A quarter note.
const Q: u32 = score_core::TICKS_PER_QUARTER;
/// A quarter note at the default 120 beats per minute and 48 kHz.
const QUARTER_FRAMES: u64 = 24_000;

fn write(score: &mut Score, tick: u32, step: Step, duration: Duration) -> ElementId {
    let event = Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration,
        notation: Default::default(),
    };
    score
        .overwrite(PartId(0), StaffId::Upper, 0, Voice::One, tick, event)
        .unwrap()[0]
}

/// Each attack's frame and key.
fn attacks(score: &Score) -> Vec<(u64, u8)> {
    let plan = PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap();
    plan.messages
        .iter()
        .filter_map(|message| match message.message {
            Message::NoteOn(note) => Some((message.frame, note.key)),
            _ => None,
        })
        .collect()
}

const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];

#[test]
fn a_quintuplet_divides_its_beat_in_five_equal_parts() {
    let mut score = Score::default();
    let five = Duration::Sixteenth.with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth));
    for i in 0..5 {
        write(&mut score, i * Q / 5, STEPS[i as usize], five);
    }
    let frames: Vec<_> = attacks(&score).iter().map(|a| a.0).collect();
    assert_eq!(
        frames,
        (0..5).map(|i| i * QUARTER_FRAMES / 5).collect::<Vec<_>>()
    );
}

#[test]
fn a_septuplet_spreads_seven_notes_over_two_beats() {
    let mut score = Score::default();
    let seven = Duration::Sixteenth.with_tuplet(Tuplet::new(7, 8, NoteValue::Sixteenth));
    for i in 0..7 {
        write(&mut score, i * 2 * Q / 7, STEPS[i as usize], seven);
    }
    // Whole frames, each at or just before its exact time.
    let frames: Vec<_> = attacks(&score).iter().map(|a| a.0).collect();
    assert_eq!(
        frames,
        (0..7)
            .map(|i| i * 2 * QUARTER_FRAMES / 7)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_double_dotted_quarter_and_a_sixteenth_fill_a_half_note() {
    let mut score = Score::default();
    write(&mut score, 0, Step::C, Duration::Quarter.with_dots(2));
    write(&mut score, 7 * Q / 4, Step::D, Duration::Sixteenth);
    write(&mut score, 2 * Q, Step::E, Duration::Quarter);
    assert_eq!(
        attacks(&score),
        [
            (0, 72),
            (7 * QUARTER_FRAMES / 4, 74),
            (2 * QUARTER_FRAMES, 76)
        ]
    );
}

#[test]
fn sixty_fourths_sound_a_sixteenth_of_a_beat_apart() {
    let mut score = Score::default();
    for i in 0..4 {
        write(
            &mut score,
            i * Q / 16,
            STEPS[i as usize],
            Duration::SixtyFourth,
        );
    }
    let frames: Vec<_> = attacks(&score).iter().map(|a| a.0).collect();
    assert_eq!(frames, [0, 1_500, 3_000, 4_500]);
}

#[test]
fn ties_into_and_out_of_a_triplet_sound_as_one_note_each() {
    let mut score = Score::default();
    let triplet = Duration::Eighth.with_triplet(true);
    let before = write(&mut score, 0, Step::C, Duration::Quarter);
    let first = write(&mut score, Q, Step::C, triplet);
    write(&mut score, Q + Q / 3, Step::D, triplet);
    let last = write(&mut score, Q + 2 * Q / 3, Step::E, triplet);
    let after = write(&mut score, 2 * Q, Step::E, Duration::Quarter);
    for (start, end) in [(before, first), (last, after)] {
        score
            .add_connection(Connection {
                start,
                end,
                kind: ConnectionKind::Tie,
            })
            .unwrap();
    }
    assert_eq!(
        attacks(&score),
        [
            (0, 72),
            (QUARTER_FRAMES + QUARTER_FRAMES / 3, 74),
            (QUARTER_FRAMES + 2 * QUARTER_FRAMES / 3, 76)
        ]
    );
}
