//! Chord symbols heard as a soft accompaniment: keyboards, guitar and harp
//! play them in their own sound, every other part with a piano, on the part's
//! second channel so they never cut its notes. A chord holds until the next
//! symbol on its staff, and strikes again after a repeat or jump.
use super::*;
use score_core::{ChordSymbol, InstrumentFamily};

pub(crate) fn sounds(score: &Score, timeline: &Timeline, rate: u32, out: &mut Vec<SoundSpan>) {
    if !score.chords_sound() {
        return;
    }
    for (index, part) in score.parts().iter().enumerate() {
        if part.instrument().percussion_key().is_some() {
            continue;
        }
        let instrument = match part.instrument().family() {
            InstrumentFamily::Keyboards | InstrumentFamily::PluckedStrings => part.instrument(),
            _ => Instrument::Piano,
        };
        let channel = part_channel(index) + 16;
        let chromatic = part.transposition().chromatic;
        for staff in part.staves() {
            let mut chords = std::collections::BTreeMap::new();
            for (bar, measure) in staff.measures().iter().enumerate() {
                for (_, onset, event, written) in measure.timed_iter() {
                    if let Some(chord) = written.notation.chord {
                        let id = ElementId {
                            part: part.id(),
                            staff: staff.id(),
                            measure: bar,
                            event,
                        };
                        chords.insert(
                            score.measure_start_tick(bar) + u64::from(onset),
                            (id, chord),
                        );
                    }
                }
            }
            if chords.is_empty() {
                continue;
            }
            let Some(envelope) = score_core::DynamicEnvelope::new(score, part.id(), staff.id())
            else {
                continue;
            };
            let mut open: Option<SoundSpan> = None;
            let mut previous: Option<usize> = None;
            for (visit, instance) in timeline.instances().iter().enumerate() {
                let (begin, end) = (instance.written, instance.written + instance.length);
                let contiguous = previous.is_some_and(|bar| bar + 1 == instance.bar);
                previous = Some(instance.bar);
                let segments: Vec<_> = chords
                    .range(..=begin)
                    .next_back()
                    .into_iter()
                    .chain(chords.range(begin + 1..end))
                    .map(|(&tick, &chord)| (tick, chord))
                    .collect();
                if !contiguous {
                    out.extend(open.take());
                }
                for (k, &(tick, (id, chord))) in segments.iter().enumerate() {
                    let from = tick.max(begin);
                    let to = segments.get(k + 1).map_or(end, |next| next.0);
                    let start = timeline.performed_frame(visit, from - begin, rate);
                    let stop = timeline.performed_frame(visit, to - begin, rate);
                    if let Some(sound) = open
                        .as_mut()
                        .filter(|sound| tick < begin && sound.element == id)
                    {
                        sound.end = stop;
                        sound.written_end = stop;
                        sound.written_range.1 = to;
                        continue;
                    }
                    out.extend(open.take());
                    let level = envelope.level_at(from);
                    open = attack(
                        chord,
                        chromatic,
                        (id, instrument, channel),
                        (start, stop),
                        (from, to),
                        level,
                    );
                }
            }
            out.extend(open.take());
        }
    }
}

fn attack(
    chord: ChordSymbol,
    chromatic: i8,
    (element, instrument, channel): (ElementId, Instrument, u8),
    (start, end): (u64, u64),
    written: (u64, u64),
    level: u8,
) -> Option<SoundSpan> {
    let mut pitches = Pitches::default();
    for key in chord.keys() {
        let key = (i16::from(key) + i16::from(chromatic)).clamp(21, 108) as u8;
        pitches = pitches.with(interpretation::pitch_of_key(key)).ok()?;
    }
    (start < end).then_some(SoundSpan {
        element,
        arpeggio: None,
        tremolo: None,
        tremolo_repeat: false,
        tremolo_attack: None,
        grace: false,
        harmony: true,
        written_ticks: written.1 - written.0,
        attack_duration: end - start,
        articulation: None,
        boost: 0,
        reference_level: level,
        technique: PlayingTechnique::Arco,
        harmonic: None,
        instrument,
        legato: false,
        slurred: false,
        start,
        end,
        written_end: end,
        written_range: written,
        channel,
        pitches,
        // An accompaniment sits a little under the written dynamic.
        level: (u16::from(level) * 4 / 5).max(1) as u8,
        muted: false,
    })
}
