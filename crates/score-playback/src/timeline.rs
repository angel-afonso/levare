//! Musical time to audio time: the tempo, with every fermata held longer.
//! A fermata stops the beat for the whole ensemble, so its span is stretched
//! for every part, and the music after it starts later.
use super::*;

/// A fermata holds its note or rest for twice its written length.
pub const FERMATA_HOLD: u64 = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timeline {
    tempo: Tempo,
    /// Merged written spans `[start, end)` in ticks under a fermata.
    holds: Vec<(u64, u64)>,
}

impl Timeline {
    pub fn new(score: &Score, tempo: Tempo) -> Self {
        let capacity = u64::from(score.time().capacity());
        let mut spans = Vec::new();
        for part in score.parts() {
            for staff in part.staves() {
                for (index, measure) in staff.measures().iter().enumerate() {
                    let mut tick = index as u64 * capacity;
                    for (_, event) in measure.iter() {
                        let end = tick + u64::from(event.duration.ticks());
                        if event.notation.fermata {
                            spans.push((tick, end));
                        }
                        tick = end;
                    }
                }
            }
        }
        spans.sort_unstable();
        // Fermatas over the same moment in several parts hold it once.
        let mut holds: Vec<(u64, u64)> = Vec::new();
        for (start, end) in spans {
            match holds.last_mut() {
                Some(last) if start < last.1 => last.1 = last.1.max(end),
                _ => holds.push((start, end)),
            }
        }
        Self { tempo, holds }
    }
    pub fn plain(tempo: Tempo) -> Self {
        Self {
            tempo,
            holds: Vec::new(),
        }
    }
    pub fn tempo(&self) -> Tempo {
        self.tempo
    }
    /// Ticks of beat that have elapsed by `tick`, counting held spans longer.
    fn held(&self, tick: u64) -> u64 {
        tick + self
            .holds
            .iter()
            .take_while(|(start, _)| *start < tick)
            .map(|(start, end)| (tick.min(*end) - start) * (FERMATA_HOLD - 1))
            .sum::<u64>()
    }
    fn held_fraction(&self, tick: f64) -> f64 {
        tick + self
            .holds
            .iter()
            .take_while(|(start, _)| (*start as f64) < tick)
            .map(|(start, end)| (tick.min(*end as f64) - *start as f64) * (FERMATA_HOLD - 1) as f64)
            .sum::<f64>()
    }
    fn ticks_per_second(&self) -> f64 {
        f64::from(self.tempo.bpm()) * TICKS_PER_QUARTER as f64 / 60.
    }
    pub fn frame_at(&self, tick: u64, sample_rate: u32) -> u64 {
        self.tempo.frame_at(self.held(tick), sample_rate)
    }
    pub fn seconds_at(&self, tick: f64) -> f64 {
        self.held_fraction(tick) / self.ticks_per_second()
    }
    /// The musical position sounding at an audio frame, the inverse of
    /// `frame_at`. Inside a hold, the position advances at half speed.
    pub fn tick_at(&self, frame: u64, sample_rate: u32) -> f64 {
        let beat = frame as f64 * self.ticks_per_second() / f64::from(sample_rate);
        let mut extra = 0.;
        for (start, end) in &self.holds {
            let (start, length) = (*start as f64, (end - start) as f64);
            let held_start = start + extra;
            if beat < held_start {
                break;
            }
            if beat < held_start + length * FERMATA_HOLD as f64 {
                return start + (beat - held_start) / FERMATA_HOLD as f64;
            }
            extra += length * (FERMATA_HOLD - 1) as f64;
        }
        beat - extra
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, Step};

    fn note(step: Step, fermata: bool) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration: Duration::Quarter,
            notation: Notation {
                fermata,
                ..Default::default()
            },
        }
    }

    #[test]
    fn a_fermata_holds_its_beat_for_every_part_and_delays_what_follows() {
        let mut score = Score::default();
        score.insert(0, 0, note(Step::C, false)).unwrap();
        score.insert(0, 1, note(Step::D, true)).unwrap();
        let after = score.insert(0, 2, note(Step::E, false)).unwrap();
        let flute = score.add_part(Instrument::Flute).unwrap();
        // A flute half note under the fermata is held along with it.
        score
            .insert_in(
                flute,
                0,
                0,
                Event {
                    duration: Duration::Half,
                    ..note(Step::G, false)
                },
            )
            .unwrap();
        let tempo = Tempo::default();
        let timeline = Timeline::new(&score, tempo);
        let q = 48; // ticks per quarter
        assert_eq!(timeline.frame_at(q, 48_000), 24_000);
        assert_eq!(timeline.frame_at(2 * q, 48_000), 72_000, "held twice");
        assert_eq!(timeline.frame_at(3 * q, 48_000), 96_000);
        assert_eq!(timeline.seconds_at(2. * q as f64), 1.5);
        for tick in [0., 30., 48., 60., 96., 120., 400.] {
            let frame = timeline.frame_at(tick as u64, 48_000);
            assert!(
                (timeline.tick_at(frame, 48_000) - tick).abs() < 1e-6,
                "{tick}"
            );
        }
        assert_eq!(timeline.tick_at(48_000, 48_000), 72.);

        let plan = PlaybackPlan::new(&score, tempo, 48_000).unwrap();
        assert_eq!(plan.frame_for_element(after), Some(72_000));
        let flute_channel = plan
            .tracks
            .iter()
            .find(|t| t.part == flute)
            .unwrap()
            .channel;
        let release = plan
            .messages
            .iter()
            .find(|m| m.channel == flute_channel && matches!(m.message, Message::NoteOff(_)))
            .unwrap();
        // The half note now lasts 1.5 s, less the bounded detached gap.
        assert_eq!(release.frame, 72_000 - 2_880);
        assert_eq!(Timeline::plain(tempo).frame_at(2 * q, 48_000), 48_000);
    }

    #[test]
    fn overlapping_fermatas_hold_the_moment_once() {
        let mut score = Score::default();
        score.insert(0, 0, note(Step::C, true)).unwrap();
        let flute = score.add_part(Instrument::Flute).unwrap();
        score.insert_in(flute, 0, 0, note(Step::E, true)).unwrap();
        let timeline = Timeline::new(&score, Tempo::default());
        assert_eq!(timeline.frame_at(48, 48_000), 48_000);
        assert_eq!(timeline.frame_at(96, 48_000), 72_000);
    }
}
