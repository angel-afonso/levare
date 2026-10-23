//! Automatic, staff-local beaming. Grouping is derived from written rhythm and
//! meter; it never changes musical events or their playback durations.
use super::*;
use score_core::NoteValue;

const THICKNESS: f32 = 5.;
const SEPARATION: f32 = 8.;

#[derive(Clone, Debug)]
pub struct BeamSegment {
    pub corners: [Point; 4],
    /// One is the main beam; two is a secondary beam or hook.
    pub level: u8,
}
#[derive(Clone, Debug)]
pub struct BeamStem {
    pub element: ElementId,
    pub from: Point,
    pub to: Point,
    pub down: bool,
}
#[derive(Clone, Debug)]
pub struct BeamGroup {
    pub elements: Vec<ElementId>,
    pub stems: Vec<BeamStem>,
    pub segments: Vec<BeamSegment>,
}
impl MeasureLayout {
    pub fn beam_stem(&self, id: ElementId) -> Option<&BeamStem> {
        self.beams
            .iter()
            .flat_map(|g| &g.stems)
            .find(|s| s.element == id)
    }
}
struct Note {
    element: ElementId,
    tick: u16,
    onset: f32,
    high: f32,
    low: f32,
    levels: u8,
    triplet: bool,
}
struct Group {
    notes: Vec<Note>,
    down: bool,
}
pub(super) struct Plan {
    groups: Vec<Group>,
    members: HashMap<ElementId, bool>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Beat(usize),
    Tuplet(usize),
}

fn levels(value: NoteValue) -> u8 {
    match value {
        NoteValue::Eighth => 1,
        NoteValue::Sixteenth => 2,
        NoteValue::ThirtySecond => 3,
        _ => 0,
    }
}
/// Simple meters follow their denominator beats. Compound meters use three
/// denominator units; odd short meters use explicit, documented defaults.
fn beats(time: TimeSignature) -> Vec<u16> {
    let n = time.numerator;
    let units: Vec<u8> = if n.is_multiple_of(3) && (n > 3 || time.denominator >= 8) {
        vec![3; usize::from(n / 3)]
    } else if time.denominator >= 8 {
        match n {
            5 => vec![3, 2],
            7 => vec![2, 2, 3],
            _ => {
                let mut groups = vec![2; usize::from(n / 2)];
                if !n.is_multiple_of(2) {
                    groups.push(1);
                }
                groups
            }
        }
    } else {
        vec![1; usize::from(n)]
    };
    let unit = score_core::TICKS_PER_WHOLE / u16::from(time.denominator);
    let mut boundaries = vec![0];
    for group in units {
        boundaries.push(boundaries.last().unwrap() + u16::from(group) * unit);
    }
    boundaries
}
impl Plan {
    pub(super) fn new(
        measure: &Measure,
        part: PartId,
        staff: StaffId,
        index: usize,
        time: TimeSignature,
        clef: Clef,
        rhythm: &Rhythm,
    ) -> Self {
        let tuplets: HashMap<_, _> = rhythm::groups(measure, part, staff, index)
            .into_iter()
            .enumerate()
            .flat_map(|(i, group)| group.into_iter().map(move |id| (id, i)))
            .collect();
        let boundaries = beats(time);
        let mut groups = Vec::new();
        let mut notes: Vec<Note> = Vec::new();
        for voice in Voice::ALL {
            let mut previous = None;
            let mut tick = 0;
            let flush = |notes: &mut Vec<Note>, groups: &mut Vec<Group>| {
                if notes.len() >= 2 {
                    let high = notes.iter().map(|n| n.high).fold(f32::INFINITY, f32::min);
                    let low = notes
                        .iter()
                        .map(|n| n.low)
                        .fold(f32::NEG_INFINITY, f32::max);
                    groups.push(Group {
                        notes: std::mem::take(notes),
                        down: if measure.has_two_voices() {
                            voice == Voice::Two
                        } else {
                            (high + low) / 2. <= 20.
                        },
                    });
                } else {
                    notes.clear();
                }
            };
            for (event, value) in measure.voice_iter(voice) {
                let id = ElementId {
                    part,
                    staff,
                    measure: index,
                    event,
                };
                let end = tick + value.duration.ticks();
                let level = levels(value.duration.value());
                let beat = boundaries.partition_point(|b| *b <= tick).saturating_sub(1);
                let key = if level == 0 || value.pitches.is_empty() {
                    None
                } else if value.duration.is_triplet() {
                    Some(Boundary::Tuplet(tuplets[&id]))
                } else if end <= boundaries[beat + 1] {
                    Some(Boundary::Beat(beat))
                } else {
                    None
                };
                if key.is_none() || key != previous {
                    flush(&mut notes, &mut groups);
                }
                if key.is_some() {
                    let y = |pitch: score_core::Pitch| {
                        40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.
                    };
                    notes.push(Note {
                        element: id,
                        tick,
                        onset: rhythm.x(tick) + voice_offset(measure, voice, tick),
                        high: y(value
                            .harmonic_touch(KeySignature::default())
                            .map(|(pitch, _)| pitch)
                            .or_else(|| value.pitches.last())
                            .unwrap()),
                        low: y(value.pitches.first().unwrap()),
                        levels: level,
                        triplet: value.duration.is_triplet(),
                    });
                }
                previous = key;
                tick = end;
            }
            flush(&mut notes, &mut groups);
        }
        let members = groups
            .iter()
            .flat_map(|g| g.notes.iter().map(move |n| (n.element, g.down)))
            .collect();
        Self { groups, members }
    }
    pub(super) fn direction(&self, id: ElementId) -> Option<bool> {
        self.members.get(&id).copied()
    }
    pub(super) fn engrave(self, result: &mut MeasureLayout, metrics: &Metrics) {
        for group in self.groups {
            let notes = &group.notes;
            let first = &notes[0];
            let last = &notes[notes.len() - 1];
            let stem_x = |note: &Note| note.onset + if group.down { 0. } else { metrics.head };
            let x1 = stem_x(first);
            let x2 = stem_x(last);
            let height = |n: &Note| if group.down { n.low } else { n.high };
            let slope = ((height(last) - height(first)) * 0.25 / (x2 - x1))
                .clamp(-0.125, 0.125)
                .clamp(-10. / (x2 - x1), 10. / (x2 - x1));
            let deepest = notes.iter().map(|n| n.levels).max().unwrap_or(1);
            // Each beam past the second lengthens the stems by its own spacing.
            let length = 35. + SEPARATION * f32::from(deepest.saturating_sub(2));
            let candidates = notes.iter().map(|n| {
                height(n) + if group.down { length } else { -length } - slope * (stem_x(n) - x1)
            });
            let origin = if group.down {
                candidates.fold(f32::NEG_INFINITY, f32::max)
            } else {
                candidates.fold(f32::INFINITY, f32::min)
            };
            let beam_y = |x: f32| origin + slope * (x - x1);
            let toward_heads = if group.down { -1. } else { 1. };
            let segment = |left: f32, right: f32, level: u8| {
                let offset = toward_heads * SEPARATION * f32::from(level - 1);
                let y1 = beam_y(left) + offset;
                let y2 = beam_y(right) + offset;
                BeamSegment {
                    level,
                    corners: [
                        Point { x: left, y: y1 },
                        Point { x: right, y: y2 },
                        Point {
                            x: right,
                            y: y2 + toward_heads * THICKNESS,
                        },
                        Point {
                            x: left,
                            y: y1 + toward_heads * THICKNESS,
                        },
                    ],
                }
            };
            let mut segments = vec![segment(x1 - metrics.stem / 2., x2 + metrics.stem / 2., 1)];
            // Every inner level joins the runs of notes that short; a lone one
            // gets a hook toward the note it pairs with in its metric unit.
            for level in 2..=deepest {
                let mut i = 0;
                while i < notes.len() {
                    if notes[i].levels < level {
                        i += 1;
                        continue;
                    }
                    let start = i;
                    while i + 1 < notes.len() && notes[i + 1].levels >= level {
                        i += 1;
                    }
                    if i > start {
                        segments.push(segment(
                            stem_x(&notes[start]) - metrics.stem / 2.,
                            stem_x(&notes[i]) + metrics.stem / 2.,
                            level,
                        ));
                    } else {
                        let unit = if notes[i].triplet {
                            score_core::TICKS_PER_QUARTER / 3
                        } else {
                            score_core::TICKS_PER_QUARTER / 2
                        };
                        let pair_ticks = unit >> (level - 2);
                        let prefer_right = notes[i].tick.is_multiple_of(pair_ticks);
                        let right = (prefer_right && i + 1 < notes.len()) || i == 0;
                        let x = stem_x(&notes[i]);
                        let neighbor = stem_x(&notes[if right { i + 1 } else { i - 1 }]);
                        let hook = ((neighbor - x).abs() * 0.45).min(12.);
                        segments.push(if right {
                            segment(x - metrics.stem / 2., x + hook, level)
                        } else {
                            segment(x - hook, x + metrics.stem / 2., level)
                        });
                    }
                    i += 1;
                }
            }
            let stems: Vec<_> = notes
                .iter()
                .map(|n| BeamStem {
                    element: n.element,
                    from: Point {
                        x: stem_x(n),
                        y: if group.down { n.high } else { n.low },
                    },
                    to: Point {
                        x: stem_x(n),
                        y: beam_y(stem_x(n)),
                    },
                    down: group.down,
                })
                .collect();
            for stem in &stems {
                result.lines.push(Line {
                    from: stem.from,
                    to: stem.to,
                    thickness: metrics.stem,
                    element: Some(stem.element),
                });
            }
            for point in segments.iter().flat_map(|s| s.corners) {
                result.top = result.top.min(point.y - 1.);
                result.bottom = result.bottom.max(point.y + 1.);
            }
            result.beams.push(BeamGroup {
                elements: notes.iter().map(|n| n.element).collect(),
                stems,
                segments,
            });
        }
    }
}
