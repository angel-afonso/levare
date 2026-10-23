use super::*;
use score_core::{ChordSymbol, Duration, Event, Instrument, Notation, Pitch, Pitches};

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 5)),
        duration,
        notation: Notation::default(),
    }
}

/// (frame, key, channel, instrument) of every attack.
fn attacks(score: &Score) -> Vec<(u64, u8, u8, Instrument)> {
    let plan = PlaybackPlan::new(score, Tempo::default(), 48_000).unwrap();
    plan.messages
        .iter()
        .filter_map(|m| match m.message {
            Message::NoteOn(note) => Some((m.frame, note.key, m.channel, note.instrument)),
            _ => None,
        })
        .collect()
}

#[test]
fn chord_symbols_sound_on_a_piano_until_the_next_one_on_the_second_channel() {
    let mut score = Score::new("".into(), &[Instrument::Flute], 2).unwrap();
    let a = score.insert(0, 0, note(Step::E, Duration::Half)).unwrap();
    score.insert(0, 1, note(Step::G, Duration::Half)).unwrap();
    let c = score.insert(1, 0, note(Step::A, Duration::Whole)).unwrap();
    score
        .set_chord(a, Some(ChordSymbol::parse("C").unwrap()))
        .unwrap();
    score
        .set_chord(c, Some(ChordSymbol::parse("Am").unwrap()))
        .unwrap();
    let all = attacks(&score);
    let chords: Vec<_> = all
        .iter()
        .filter(|(_, _, channel, _)| *channel == part_channel(0) + 16)
        .collect();
    assert!(chords.iter().all(|(_, _, _, i)| *i == Instrument::Piano));
    let keys = |frame: u64| -> Vec<u8> {
        chords
            .iter()
            .filter(|(f, ..)| *f == frame)
            .map(|(_, key, ..)| *key)
            .collect()
    };
    assert_eq!(keys(0), [48, 60, 64, 67]);
    // C holds through the second half note; A minor strikes on the next bar.
    let bar = 2 * 48_000;
    assert_eq!(keys(bar), [45, 57, 60, 64]);
    assert_eq!(chords.len(), 8);
    let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
    let release = plan
        .messages
        .iter()
        .find(|m| m.channel == part_channel(0) + 16 && m.message == Message::NoteOff(48))
        .unwrap();
    assert_eq!(release.frame, bar);
    // The flute itself is untouched.
    assert_eq!(
        all.iter()
            .filter(|(_, _, channel, _)| *channel == part_channel(0))
            .count(),
        3
    );
    score.set_chords_sound(false);
    assert_eq!(attacks(&score).len(), 3);
}

#[test]
fn a_transposing_part_plays_its_written_chord_at_sounding_pitch_in_its_own_sound() {
    let mut score =
        Score::new("".into(), &[Instrument::Guitar, Instrument::ClarinetBb], 1).unwrap();
    let guitar = score.parts()[0].id();
    let clarinet = score.parts()[1].id();
    let g = score
        .insert_in(guitar, 0, 0, note(Step::E, Duration::Whole))
        .unwrap();
    let k = score
        .insert_in(clarinet, 0, 0, note(Step::D, Duration::Whole))
        .unwrap();
    for id in [g, k] {
        score
            .set_chord(id, Some(ChordSymbol::parse("D").unwrap()))
            .unwrap();
    }
    let all = attacks(&score);
    let on = |channel: u8| -> Vec<(u8, Instrument)> {
        all.iter()
            .filter(|(_, _, c, _)| *c == channel)
            .map(|(_, key, _, i)| (*key, *i))
            .collect()
    };
    // The guitar sounds an octave below its writing; the clarinet a tone below.
    let guitar_keys: Vec<_> = on(part_channel(0) + 16)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(guitar_keys, [38, 50, 54, 57]);
    assert!(
        on(part_channel(0) + 16)
            .iter()
            .all(|(_, i)| *i == Instrument::Guitar)
    );
    let clarinet_keys: Vec<_> = on(part_channel(1) + 16)
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(clarinet_keys, [48, 60, 64, 67]);
}
