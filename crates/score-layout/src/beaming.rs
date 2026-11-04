//! Automatic, staff-local beaming. Grouping is derived from written rhythm and
//! meter; it never changes musical events or their playback durations.
use super::*;
use score_core::{Beam, Duration, NoteValue};

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
#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub element: ElementId,
    tick: u32,
    onset: f32,
    pub high: f32,
    pub low: f32,
    levels: u8,
    /// How long an eighth lasts here, in or out of a tuplet.
    eighth: u32,
    /// A stem turned down (`true`) or up by hand.
    down: Option<bool>,
    /// Drawn on the other staff of a piano.
    pub cross: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Group {
    pub notes: Vec<Note>,
    pub down: bool,
    /// The direction chosen by hand or by the voice, before the heads'.
    pub forced: Option<bool>,
}
impl Group {
    /// The direction its heads ask for, once they are where they are drawn.
    pub fn settle(&mut self) {
        let high = self
            .notes
            .iter()
            .map(|n| n.high)
            .fold(f32::INFINITY, f32::min);
        let low = self
            .notes
            .iter()
            .map(|n| n.low)
            .fold(f32::NEG_INFINITY, f32::max);
        self.down = self.forced.unwrap_or((high + low) / 2. <= 20.);
    }
    /// Whether it joins notes of both staves.
    pub fn spans_staves(&self) -> bool {
        self.notes.iter().any(|n| n.cross) && self.notes.iter().any(|n| !n.cross)
    }
}
/// Where the highest and lowest heads of `event` sit on a staff in `clef`.
pub(crate) fn heads(event: &score_core::Event, clef: Clef) -> (f32, f32) {
    let y =
        |pitch: score_core::Pitch| 40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.;
    (
        y(event
            .harmonic_touch(KeySignature::default())
            .map(|(pitch, _)| pitch)
            .or_else(|| event.pitches.last())
            .unwrap()),
        y(event.pitches.first().unwrap()),
    )
}
pub(super) struct Plan {
    groups: Vec<Group>,
    /// Groups with a note on the other staff, drawn once both are placed.
    crossing: Vec<Group>,
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
        NoteValue::SixtyFourth => 4,
        _ => 0,
    }
}
/// Simple meters follow their denominator beats. Compound meters use three
/// denominator units; odd short meters use explicit, documented defaults.
fn beats(time: TimeSignature) -> Vec<u32> {
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
    let unit = score_core::TICKS_PER_WHOLE / u32::from(time.denominator);
    let mut boundaries = vec![0];
    for group in units {
        boundaries.push(boundaries.last().unwrap() + u32::from(group) * unit);
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
            .flat_map(|(i, group)| group.elements.into_iter().map(move |id| (id, i)))
            .collect();
        let boundaries = beats(time);
        let mut groups = Vec::new();
        let mut notes: Vec<Note> = Vec::new();
        for voice in Voice::ALL {
            let mut previous = None;
            let mut joined = None;
            let mut tick = 0;
            let flush = |notes: &mut Vec<Note>, groups: &mut Vec<Group>| {
                if notes.len() >= 2 {
                    // A stem turned by hand turns its whole group.
                    let forced = notes
                        .iter()
                        .find_map(|note| note.down)
                        .or(measure.has_several_voices().then_some(voice.is_lower()));
                    let mut group = Group {
                        notes: std::mem::take(notes),
                        down: false,
                        forced,
                    };
                    group.settle();
                    groups.push(group);
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
                let beam = value.notation.beam;
                let natural = if level == 0 || value.pitches.is_empty() || beam == Some(Beam::Off) {
                    None
                } else if value.duration.tuplet().is_some() {
                    Some(Boundary::Tuplet(tuplets[&id]))
                } else if end <= boundaries[beat + 1] || beam == Some(Beam::Start) {
                    // Started by hand, a note across the beat may still lead a beam.
                    Some(Boundary::Beat(beat))
                } else {
                    None
                };
                // A joined note carries the rest of its beat into the previous beam.
                let key = if beam == Some(Beam::Join) && natural.is_some() && previous.is_some() {
                    joined = natural.map(|natural| (natural, previous.unwrap()));
                    previous
                } else {
                    match joined {
                        Some((from, to)) if natural == Some(from) => Some(to),
                        _ => natural,
                    }
                };
                if key.is_none() || key != previous || beam == Some(Beam::Start) {
                    flush(&mut notes, &mut groups);
                }
                if key.is_some() {
                    let (high, low) = heads(value, clef);
                    notes.push(Note {
                        down: value
                            .notation
                            .stem
                            .map(|stem| stem == score_core::Stem::Down),
                        element: id,
                        tick,
                        onset: rhythm.x(tick) + voice_offset(measure, voice, tick),
                        high,
                        low,
                        levels: level,
                        eighth: Duration::Eighth
                            .with_tuplet(value.duration.tuplet())
                            .ticks(),
                        cross: value.notation.cross,
                    });
                }
                previous = key;
                tick = end;
            }
            flush(&mut notes, &mut groups);
        }
        let (crossing, groups): (Vec<_>, Vec<_>) = groups
            .into_iter()
            .partition(|g| g.notes.iter().any(|n| n.cross));
        // A beam between the staves takes this staff's stems toward the other.
        let members = groups
            .iter()
            .flat_map(|g| g.notes.iter().map(move |n| (n.element, g.down)))
            .chain(
                crossing
                    .iter()
                    .flat_map(|g| &g.notes)
                    .filter(|n| !n.cross)
                    .map(|n| (n.element, staff == StaffId::Upper)),
            )
            .collect();
        Self {
            groups,
            crossing,
            members,
        }
    }
    pub(super) fn direction(&self, id: ElementId) -> Option<bool> {
        self.members.get(&id).copied()
    }
    pub(super) fn engrave(self, result: &mut MeasureLayout, metrics: &Metrics) {
        result.crossing_beams = self.crossing;
        for group in self.groups {
            engrave_group(group, result, metrics);
        }
    }
}

/// Draw a beam over notes of one staff, on the side `group.down` says.
pub(crate) fn engrave_group(group: Group, result: &mut MeasureLayout, metrics: &Metrics) {
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
    let candidates = notes
        .iter()
        .map(|n| height(n) + if group.down { length } else { -length } - slope * (stem_x(n) - x1));
    let origin = if group.down {
        candidates.fold(f32::NEG_INFINITY, f32::max)
    } else {
        candidates.fold(f32::INFINITY, f32::min)
    };
    let beam_y = |x: f32| origin + slope * (x - x1);
    let toward_heads = if group.down { -1. } else { 1. };
    let segments = segments(notes, &stem_x, &beam_y, toward_heads, metrics);
    let stems = notes
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
    push(result, notes, stems, segments, metrics);
}

/// Draw a beam between the two staves of a piano: notes above it, already
/// placed in `result`'s coordinates by their `upper` side, stem down to it
/// and notes below it stem up.
pub(crate) fn engrave_between(
    group: Group,
    upper: impl Fn(&Note) -> bool,
    result: &mut MeasureLayout,
    metrics: &Metrics,
) {
    let notes = &group.notes;
    let stem_x = |note: &Note| note.onset + if upper(note) { 0. } else { metrics.head };
    let deepest = notes.iter().map(|n| n.levels).max().unwrap_or(1);
    let depth = SEPARATION * f32::from(deepest - 1) + THICKNESS;
    let above = notes
        .iter()
        .filter(|n| upper(n))
        .map(|n| n.low)
        .fold(f32::NEG_INFINITY, f32::max);
    let below = notes
        .iter()
        .filter(|n| !upper(n))
        .map(|n| n.high)
        .fold(f32::INFINITY, f32::min);
    // Level, halfway between the lowest head above and the highest below.
    let origin = (above + below) / 2. - depth / 2.;
    let beam_y = |_: f32| origin;
    let segments = segments(notes, &stem_x, &beam_y, 1., metrics);
    let stems = notes
        .iter()
        .map(|n| {
            let down = upper(n);
            BeamStem {
                element: n.element,
                from: Point {
                    x: stem_x(n),
                    y: if down { n.high } else { n.low },
                },
                to: Point {
                    x: stem_x(n),
                    y: if down { origin + depth } else { origin },
                },
                down,
            }
        })
        .collect();
    push(result, notes, stems, segments, metrics);
}

/// The beams of every level over `notes`, from the main one on `beam_y`,
/// each further level a step `toward` (1 down, -1 up).
fn segments(
    notes: &[Note],
    stem_x: &impl Fn(&Note) -> f32,
    beam_y: &impl Fn(f32) -> f32,
    toward: f32,
    metrics: &Metrics,
) -> Vec<BeamSegment> {
    let segment = |left: f32, right: f32, level: u8| {
        let offset = toward * SEPARATION * f32::from(level - 1);
        let y1 = beam_y(left) + offset;
        let y2 = beam_y(right) + offset;
        BeamSegment {
            level,
            corners: [
                Point { x: left, y: y1 },
                Point { x: right, y: y2 },
                Point {
                    x: right,
                    y: y2 + toward * THICKNESS,
                },
                Point {
                    x: left,
                    y: y1 + toward * THICKNESS,
                },
            ],
        }
    };
    let first = stem_x(&notes[0]);
    let last = stem_x(&notes[notes.len() - 1]);
    let deepest = notes.iter().map(|n| n.levels).max().unwrap_or(1);
    let mut segments = vec![segment(
        first - metrics.stem / 2.,
        last + metrics.stem / 2.,
        1,
    )];
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
                let pair_ticks = notes[i].eighth >> (level - 2);
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
    segments
}

fn push(
    result: &mut MeasureLayout,
    notes: &[Note],
    stems: Vec<BeamStem>,
    segments: Vec<BeamSegment>,
    metrics: &Metrics,
) {
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
