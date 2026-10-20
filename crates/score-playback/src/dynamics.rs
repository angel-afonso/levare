//! Per-attack expression calibration. Ordinary passages keep their exact velocity
//! and full expression. A sustained crescendo reserves only the headroom it needs.
use super::*;
use score_core::DynamicEnvelope;

struct LevelSegment {
    start: u64,
    end: u64,
    from: f64,
    to: f64,
}
impl LevelSegment {
    fn at(&self, frame: u64) -> f64 {
        self.from
            + (self.to - self.from)
                * (frame.saturating_sub(self.start).min(self.end - self.start)) as f64
                / (self.end - self.start) as f64
    }
}
pub(super) fn prepare(
    envelope: DynamicEnvelope,
    sounds: &mut [SoundSpan],
    timeline: &Timeline,
    rate: u32,
) -> Vec<ExpressionPoint> {
    let levels: Vec<_> = envelope
        .segments
        .into_iter()
        .flat_map(|s| {
            let ticks: Vec<_> = std::iter::once(s.start)
                .chain(
                    timeline
                        .boundaries()
                        .filter(|tick| *tick > s.start && *tick < s.end),
                )
                .chain(std::iter::once(s.end))
                .collect();
            let level = |tick: u64| {
                f64::from(s.from)
                    + (f64::from(s.to) - f64::from(s.from)) * (tick - s.start) as f64
                        / (s.end - s.start) as f64
            };
            ticks
                .windows(2)
                .map(|pair| LevelSegment {
                    start: timeline.frame_at(pair[0], rate),
                    end: timeline.frame_at(pair[1], rate),
                    from: level(pair[0]),
                    to: level(pair[1]),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    for sound in sounds.iter_mut() {
        let first = levels.partition_point(|s| s.end <= sound.start);
        let peak = levels[first..]
            .iter()
            .take_while(|s| s.start < sound.end)
            .map(|s| {
                s.at(sound.start.max(s.start))
                    .max(s.at(sound.end.min(s.end)))
            })
            .fold(1f64, f64::max)
            .ceil() as u8;
        sound.reference_level = peak;
        sound.level = peak.saturating_add(sound.boost).min(127);
    }
    // A channel has one expression controller. Overlapping sounds must reserve
    // the same headroom, including chains connected through a long second voice.
    let mut order: Vec<_> = (0..sounds.len()).collect();
    order.sort_by_key(|i| sounds[*i].start);
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        let mut release = sounds[order[start]].end;
        let mut reference = sounds[order[start]].reference_level;
        while end < order.len() && sounds[order[end]].start < release {
            release = release.max(sounds[order[end]].end);
            reference = reference.max(sounds[order[end]].reference_level);
            end += 1;
        }
        for index in &order[start..end] {
            sounds[*index].reference_level = reference;
            sounds[*index].level = reference.saturating_add(sounds[*index].boost).min(127);
        }
        start = end;
    }
    let mut points: Vec<ExpressionPoint> = Vec::new();
    let mut sound_index = 0;
    let mut reference = 127;
    for segment in levels {
        let mut cursor = segment.start;
        while cursor < segment.end {
            while order
                .get(sound_index)
                .is_some_and(|i| sounds[*i].start <= cursor)
            {
                reference = sounds[order[sound_index]].reference_level;
                sound_index += 1;
            }
            let end = order
                .get(sound_index)
                .map_or(segment.end, |i| sounds[*i].start.min(segment.end));
            let cc = |frame| {
                (segment.at(frame) * 127. / f64::from(reference))
                    .round()
                    .clamp(0., 127.) as u8
            };
            let from = cc(cursor);
            let to = cc(end);
            let steps = u64::from(from.abs_diff(to));
            for step in 0..=steps {
                let value = if to >= from {
                    from + step as u8
                } else {
                    from - step as u8
                };
                // Invert the original level slope at each rounded controller
                // transition. Interpolating clamped endpoints would stretch a
                // crescendo across a rest after the sounding interval ends.
                let frame = if step == 0 {
                    cursor
                } else {
                    let threshold = (f64::from(value) + if to > from { -0.5 } else { 0.5 })
                        * f64::from(reference)
                        / 127.;
                    let fraction = ((threshold - segment.at(cursor))
                        / (segment.at(end) - segment.at(cursor)))
                    .clamp(0., 1.);
                    cursor + ((end - cursor) as f64 * fraction).ceil() as u64
                };
                if points
                    .last()
                    .is_some_and(|p| p.frame == frame && p.value == value)
                {
                    continue;
                }
                points.push(ExpressionPoint { frame, value });
            }
            cursor = end;
        }
    }
    points
}
