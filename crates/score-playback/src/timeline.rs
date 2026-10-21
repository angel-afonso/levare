//! Musical time to audio time: the tempo, with every fermata held longer.
//! A fermata stops the beat for the whole ensemble, so its span is stretched
//! for every part, and the music after it starts later.
use super::*;

/// A fermata holds its note or rest for twice its written length.
pub const FERMATA_HOLD: u64 = 2;

#[derive(Clone, Debug, PartialEq)]
struct Segment {
    tick: u64,
    seconds: f64,
    seconds_per_tick: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    pub bar: usize,
    /// Written tick of this bar's start.
    pub written: u64,
    /// Tick of this visit on the performed axis.
    pub performed: u64,
    pub length: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    /// Rehearsal speed relative to quarter = 120 (120 means written tempo).
    tempo: Tempo,
    segments: Vec<Segment>,
    instances: Vec<Instance>,
    linear: bool,
}

impl Timeline {
    pub fn new(score: &Score, tempo: Tempo) -> Self {
        Self::with_repeats(score, tempo, false)
    }
    pub fn with_repeats(score: &Score, tempo: Tempo, follow: bool) -> Self {
        let mut performed = 0;
        let instances: Vec<_> = score
            .play_order(follow)
            .into_iter()
            .map(|played| {
                let length = u64::from(score.time_at(played.bar).capacity());
                let instance = Instance {
                    bar: played.bar,
                    written: score.measure_start_tick(played.bar),
                    performed,
                    length,
                };
                performed += length;
                instance
            })
            .collect();
        let mut bar_holds = vec![Vec::new(); score.measures().len()];
        for part in score.parts() {
            for staff in part.staves() {
                for (index, measure) in staff.measures().iter().enumerate() {
                    for (_, onset, _, event) in measure.timed_iter() {
                        let tick = u64::from(onset);
                        let end = tick + u64::from(event.duration.ticks());
                        if event.notation.fermata {
                            bar_holds[index].push((tick, end));
                        }
                    }
                }
            }
        }
        let mut spans: Vec<_> = instances
            .iter()
            .flat_map(|instance| {
                bar_holds[instance.bar]
                    .iter()
                    .map(|&(start, end)| (instance.performed + start, instance.performed + end))
            })
            .collect();
        spans.sort_unstable();
        // Fermatas over the same moment in several parts hold it once.
        let mut holds: Vec<(u64, u64)> = Vec::new();
        for (start, end) in spans {
            match holds.last_mut() {
                Some(last) if start < last.1 => last.1 = last.1.max(end),
                _ => holds.push((start, end)),
            }
        }
        let marks: Vec<_> = instances
            .iter()
            .enumerate()
            .filter_map(|(index, instance)| {
                (index == 0
                    || instances[index - 1].bar + 1 != instance.bar
                    || score.tempo_change_at(instance.bar).is_some())
                .then_some((instance.performed, score.tempo_at(instance.bar)))
            })
            .collect();
        let mut boundaries: Vec<_> = marks
            .iter()
            .map(|(tick, _)| *tick)
            .chain(holds.iter().flat_map(|(a, b)| [*a, *b]))
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        let mut segments: Vec<Segment> = Vec::new();
        let mut mark_index = 0;
        let mut hold_index = 0;
        for tick in boundaries {
            while marks.get(mark_index + 1).is_some_and(|(at, _)| *at <= tick) {
                mark_index += 1;
            }
            while holds.get(hold_index).is_some_and(|(_, end)| *end <= tick) {
                hold_index += 1;
            }
            let seconds = segments.last().map_or(0., |last| {
                last.seconds + (tick - last.tick) as f64 * last.seconds_per_tick
            });
            let mark = marks[mark_index].1;
            let held = if holds
                .get(hold_index)
                .is_some_and(|(start, end)| *start <= tick && tick < *end)
            {
                FERMATA_HOLD as f64
            } else {
                1.
            };
            let seconds_per_tick = 60. * 120. * held
                / (f64::from(mark.bpm) * f64::from(mark.beat.ticks()) * f64::from(tempo.bpm()));
            segments.push(Segment {
                tick,
                seconds,
                seconds_per_tick,
            });
        }
        // Incomplete form may legally skip every bar (e.g. a lone second
        // ending). Keep the map total even when there is no performed visit.
        if segments.is_empty() {
            segments = Self::plain(tempo).segments;
        }
        Self {
            tempo,
            segments,
            instances,
            linear: !follow,
        }
    }
    pub fn plain(tempo: Tempo) -> Self {
        Self {
            tempo,
            instances: Vec::new(),
            linear: true,
            segments: vec![Segment {
                tick: 0,
                seconds: 0.,
                seconds_per_tick: 60. / (f64::from(tempo.bpm()) * TICKS_PER_QUARTER as f64),
            }],
        }
    }
    pub fn tempo(&self) -> Tempo {
        self.tempo
    }
    pub(crate) fn boundaries_between(
        &self,
        start: u64,
        end: u64,
    ) -> impl Iterator<Item = u64> + '_ {
        let first = self
            .segments
            .partition_point(|segment| segment.tick <= start);
        self.segments[first..]
            .iter()
            .take_while(move |segment| segment.tick < end)
            .map(|segment| segment.tick)
    }
    /// First performed visit containing this written tick, or the performed
    /// end when the written position is skipped by the form.
    pub fn frame_at(&self, tick: u64, sample_rate: u32) -> u64 {
        // A tiny tolerance prevents a mathematically integral frame rounding
        // down because of floating point accumulation between tempo segments.
        self.performed_frame_at(self.written_to_performed(tick as f64), sample_rate)
    }
    pub fn seconds_at(&self, tick: f64) -> f64 {
        self.performed_seconds(self.written_to_performed(tick))
    }
    fn written_to_performed(&self, tick: f64) -> f64 {
        let tick = tick.max(0.);
        // Legacy linear callers extrapolate beyond the score (e.g. a ruler).
        if self.linear {
            return tick;
        }
        self.instances
            .iter()
            .find(|instance| {
                instance.written as f64 <= tick
                    && tick < (instance.written + instance.length) as f64
            })
            .map_or_else(
                || self.performed_end() as f64,
                |instance| instance.performed as f64 + tick - instance.written as f64,
            )
    }
    fn performed_end(&self) -> u64 {
        self.instances
            .last()
            .map_or(0, |instance| instance.performed + instance.length)
    }
    pub fn instances(&self) -> &[Instance] {
        &self.instances
    }
    /// Frame of an offset within one specific performed visit.
    pub fn performed_frame(&self, instance: usize, offset: u64, rate: u32) -> u64 {
        self.performed_frame_at((self.instances[instance].performed + offset) as f64, rate)
    }
    pub fn total_frames(&self, rate: u32) -> u64 {
        self.performed_frame_at(self.performed_end() as f64, rate)
    }
    pub(crate) fn performed_frame_at(&self, tick: f64, rate: u32) -> u64 {
        (self.performed_seconds(tick) * f64::from(rate) + 1e-6).floor() as u64
    }
    fn performed_seconds(&self, tick: f64) -> f64 {
        let tick = tick.max(0.);
        let index = self
            .segments
            .partition_point(|s| s.tick as f64 <= tick)
            .saturating_sub(1);
        let segment = &self.segments[index];
        segment.seconds + (tick - segment.tick as f64) * segment.seconds_per_tick
    }
    /// Inverse of the complete tempo/fermata map, including fractional ticks.
    pub fn tick_at(&self, frame: u64, sample_rate: u32) -> f64 {
        if sample_rate == 0 || (!self.linear && self.instances.is_empty()) {
            return 0.;
        }
        let seconds = frame as f64 / f64::from(sample_rate);
        let index = self
            .segments
            .partition_point(|s| s.seconds <= seconds)
            .saturating_sub(1);
        let segment = &self.segments[index];
        let performed =
            segment.tick as f64 + (seconds - segment.seconds) / segment.seconds_per_tick;
        if self.linear {
            return performed;
        }
        let index = self
            .instances
            // Ownership follows the rounded scheduled frame: at a jump a
            // sub-sample rounding error must not return the previous bar.
            .partition_point(|instance| {
                self.performed_frame_at(instance.performed as f64, sample_rate) <= frame
            })
            .saturating_sub(1);
        let instance = &self.instances[index];
        instance.written as f64
            + (performed - instance.performed as f64).clamp(0., instance.length as f64)
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
    fn written_tempos_integrate_beats_meters_and_fermatas_and_invert_at_each_boundary() {
        use score_core::{TempoBeat, TempoMark, TimeSignature};
        let mut score = Score::default();
        score
            .set_time_signature_at(
                1,
                TimeSignature {
                    numerator: 3,
                    denominator: 4,
                },
            )
            .unwrap();
        score
            .set_tempo_at(
                1,
                TempoMark {
                    bpm: 60,
                    ..Default::default()
                },
            )
            .unwrap();
        score
            .set_tempo_at(
                2,
                TempoMark {
                    bpm: 120,
                    beat: TempoBeat::DottedQuarter,
                    text: "Allegro".into(),
                },
            )
            .unwrap();
        score
            .set_tempo_at(
                3,
                TempoMark {
                    bpm: 60,
                    beat: TempoBeat::Half,
                    text: String::new(),
                },
            )
            .unwrap();
        score
            .insert(
                0,
                0,
                Event {
                    duration: Duration::Whole,
                    ..note(Step::C, false)
                },
            )
            .unwrap();
        score.insert(1, 0, note(Step::D, false)).unwrap();
        score.insert(1, 1, note(Step::E, true)).unwrap();
        let after = score.insert(2, 0, note(Step::F, false)).unwrap();
        let timeline = Timeline::new(&score, Tempo::default());
        assert_eq!(timeline.frame_at(192, 48_000), 96_000);
        assert_eq!(timeline.frame_at(336, 48_000), 288_000);
        assert_eq!(timeline.frame_at(480, 48_000), 336_000);
        assert_eq!(timeline.frame_at(624, 48_000), 408_000);
        let plan = PlaybackPlan::new(&score, Tempo::default(), 48_000).unwrap();
        assert_eq!(plan.frame_for_element(after), Some(288_000));
        for rate in [44_100, 48_000, 96_000] {
            for tick in [
                0, 1, 191, 192, 239, 240, 270, 287, 288, 335, 336, 479, 480, 623, 624, 700,
            ] {
                let frame = timeline.frame_at(tick, rate);
                assert!((timeline.tick_at(frame, rate) - tick as f64).abs() < 0.003);
            }
        }
        let slower = Timeline::new(&score, Tempo::new(60).unwrap());
        assert_eq!(slower.frame_at(624, 48_000), 816_000);
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
