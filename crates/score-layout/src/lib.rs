//! Small, incremental, multi-staff engraver. All coordinates are UI-independent.
#[cfg(test)]
use score_core::Instrument;
use score_core::{
    Clef, ElementId, InstrumentFamily, KeySignature, MarkRef, Measure, PartId, Score, StaffId,
    TimeSignature, Voice,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[cfg(test)]
use score_core::{Accidental, Duration};
mod repertoire;
pub use repertoire::{TextRun, bravura_character, glyph_codes, text_runs};
mod beaming;
mod divisi;
pub use beaming::{BeamGroup, BeamSegment, BeamStem};
mod views;
pub use views::{Page, System, ViewMode};
mod rhythm;
pub use rhythm::TupletGroup;
mod expression;
pub use expression::{Curve, Hairpin, Hit, MarkBox, PianoArpeggioLayout};

pub const STAFF_SPACE: f32 = 10.;
pub const MUSIC_SIZE: f32 = STAFF_SPACE * 4.;
pub const STAFF_LABEL_SIZE: f32 = 16.;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
#[derive(Clone, Debug)]
pub struct Line {
    pub from: Point,
    pub to: Point,
    pub thickness: f32,
    /// Note-owned strokes (stems); staff and ledger lines stay neutral.
    pub element: Option<ElementId>,
}
#[derive(Clone, Debug)]
pub struct Glyph {
    /// Turn the original outlines 90° counterclockwise (SMuFL line segments).
    pub rotated: bool,
    pub text: String,
    pub position: Point,
    pub element: Option<ElementId>,
}
#[derive(Clone, Debug)]
pub struct TechniqueLabel {
    pub text: &'static str,
    pub position: Point,
    pub element: ElementId,
}
#[derive(Clone, Debug)]
pub struct EventPosition {
    pub voice: Voice,
    pub tick: u16,
    pub end_tick: u16,
    pub id: ElementId,
    pub pitch: Option<score_core::Pitch>,
    pub center: Point,
}

#[derive(Clone, Debug)]
pub struct MeasureLayout {
    pub lines: Vec<Line>,
    pub glyphs: Vec<Glyph>,
    pub techniques: Vec<TechniqueLabel>,
    /// Clickable area of each point mark (articulation, dynamic, technique).
    pub marks: Vec<MarkBox>,
    pub tuplets: Vec<TupletGroup>,
    pub beams: Vec<BeamGroup>,
    pub events: Vec<EventPosition>,
    pub staff_spans: Vec<(f32, f32)>,
    pub cursor_x: Vec<f32>,
    pub second_cursor_x: Vec<f32>,
    /// Shared rhythmic onsets, including bar end, for a continuous playhead.
    pub time_positions: Vec<(u16, f32)>,
    pub top: f32,
    pub bottom: f32,
    pub ink_top: f32,
    pub ink_bottom: f32,
    /// Note/rest, stem and beam ink, before expression marks.
    pub music_ink_bottom: f32,
}

impl MeasureLayout {
    pub fn tick_at_x(&self, x: f32) -> f64 {
        for pair in self.time_positions.windows(2) {
            let [(a, x1), (b, x2)] = pair else {
                unreachable!()
            };
            if x <= *x2 {
                return f64::from(*a)
                    + f64::from(*b - *a)
                        * f64::from(((x - *x1) / (*x2 - *x1).max(0.001)).clamp(0., 1.));
            }
        }
        f64::from(self.time_positions.last().unwrap().0)
    }
    pub fn x_at_tick(&self, tick: f64) -> f32 {
        let Some(&(first, x)) = self.time_positions.first() else {
            return 0.;
        };
        if tick <= f64::from(first) {
            return x;
        }
        for pair in self.time_positions.windows(2) {
            let [(a, x1), (b, x2)] = pair else {
                unreachable!()
            };
            if tick <= f64::from(*b) {
                let t = ((tick - f64::from(*a)) / f64::from(b - a)).clamp(0., 1.) as f32;
                return x1 + (x2 - x1) * t;
            }
        }
        self.time_positions.last().unwrap().1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VoiceRegion {
    pub voice: Voice,
    pub start: u16,
    pub end: u16,
}
impl VoiceRegion {
    pub fn contains(&self, voice: Voice, tick: u16) -> bool {
        self.voice == voice && self.start <= tick && tick < self.end
    }
}
#[derive(Clone, Debug)]
pub struct PlacedMeasure {
    pub system_start: usize,
    pub start_tick: u64,
    pub staff: StaffId,
    /// Canonical musical source of this displayed row.
    pub source_staff: StaffId,
    pub source_voice: Option<Voice>,
    pub regions: Vec<VoiceRegion>,
    pub source_onsets: [Vec<u16>; 2],
    pub part: PartId,
    pub index: usize,
    pub origin: Point,
    pub width: f32,
    pub content: Arc<MeasureLayout>,
}

impl PlacedMeasure {
    pub fn insertion_near(&self, preferred: Voice, x: f32) -> (Voice, usize) {
        let tick = self.content.tick_at_x(x);
        let voice = self.source_voice.unwrap_or_else(|| {
            if self.regions.iter().any(|r| {
                r.voice == preferred && f64::from(r.start) <= tick && tick < f64::from(r.end)
            }) {
                preferred
            } else {
                Voice::One
            }
        });
        let onsets = &self.source_onsets[usize::from(voice == Voice::Two)];
        let capacity = self.content.time_positions.last().unwrap().0;
        let index = onsets
            .iter()
            .enumerate()
            .filter(|(_, t)| {
                self.regions
                    .iter()
                    .any(|r| r.contains(voice, (**t).min(capacity - 1)))
            })
            .min_by(|(_, a), (_, b)| {
                (self.content.x_at_tick(f64::from(**a)) - x)
                    .abs()
                    .total_cmp(&(self.content.x_at_tick(f64::from(**b)) - x).abs())
            })
            .map(|(i, _)| i)
            .unwrap_or(onsets.len() - 1);
        (voice, index)
    }
}
#[derive(Clone, Debug)]
pub struct TempoLabel {
    pub measure: usize,
    pub mark: score_core::TempoMark,
    pub position: Point,
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct StaffLabel {
    pub staff: Option<StaffId>,
    pub text: String,
    pub position: Point,
    pub part: PartId,
}
#[derive(Clone, Debug)]
pub struct Brace {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
}
/// Family bracket at the beginning of a system; piano braces remain separate.
#[derive(Clone, Debug)]
pub struct Bracket {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
    pub family: InstrumentFamily,
}
#[derive(Clone, Debug)]
pub struct Layout {
    /// Presentation scale; musical geometry and cached measures remain unscaled.
    pub scale: f32,
    pub mode: ViewMode,
    pub pages: Vec<Page>,
    pub systems: Vec<System>,
    pub width: f32,
    pub height: f32,
    pub measures: Vec<PlacedMeasure>,
    pub labels: Vec<StaffLabel>,
    pub tempos: Vec<TempoLabel>,
    pub connectors: Vec<Line>,
    pub braces: Vec<Brace>,
    pub brackets: Vec<Bracket>,
    pub curves: Vec<Curve>,
    pub hairpins: Vec<Hairpin>,
    pub piano_arpeggios: Vec<PianoArpeggioLayout>,
}

impl Layout {
    pub fn hit_tempo(&self, point: Point) -> Option<usize> {
        let p = Point {
            x: point.x / self.scale,
            y: point.y / self.scale,
        };
        self.tempos
            .iter()
            .find(|mark| {
                p.x >= mark.position.x - 4.
                    && p.x <= mark.position.x + mark.width
                    && p.y >= mark.position.y - 30.
                    && p.y <= mark.position.y + 5.
            })
            .map(|mark| mark.measure)
    }
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.hit_pitch(point).map(|(id, _)| id)
    }
    pub fn hit_pitch(&self, point: Point) -> Option<(ElementId, Option<score_core::Pitch>)> {
        self.hit_pitch_in_voice(point, Voice::One)
    }
    pub fn hit_pitch_in_voice(
        &self,
        point: Point,
        preferred: Voice,
    ) -> Option<(ElementId, Option<score_core::Pitch>)> {
        self.hit_pitch_with_position(point, preferred)
            .map(|(id, pitch, _)| (id, pitch))
    }
    pub fn hit_pitch_with_position(
        &self,
        point: Point,
        preferred: Voice,
    ) -> Option<(ElementId, Option<score_core::Pitch>, u16)> {
        let point = Point {
            x: point.x / self.scale,
            y: point.y / self.scale,
        };
        self.measures
            .iter()
            .flat_map(|measure| {
                measure
                    .content
                    .events
                    .iter()
                    .map(move |event| (measure, event))
            })
            .filter_map(|(measure, event)| {
                let dx = point.x - measure.origin.x - event.center.x;
                let dy = point.y - measure.origin.y - event.center.y;
                (dx.abs() <= 15. && dy.abs() <= 18.).then_some((
                    (event.id, event.pitch, event.tick),
                    dx * dx + dy * dy,
                    event.voice,
                ))
            })
            .min_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| (a.2 != preferred).cmp(&(b.2 != preferred)))
            })
            .map(|(target, _, _)| target)
    }

    pub fn measure(&self, part: PartId, measure: usize) -> Option<&PlacedMeasure> {
        self.measure_on(part, StaffId::Upper, measure)
    }
    pub fn measure_on(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
    ) -> Option<&PlacedMeasure> {
        self.measures
            .iter()
            .find(|placed| placed.part == part && placed.staff == staff && placed.index == measure)
    }
    pub fn event_measure(&self, id: ElementId) -> Option<&PlacedMeasure> {
        self.measures
            .iter()
            .filter_map(|m| {
                m.content
                    .events
                    .iter()
                    .filter(|e| e.id == id)
                    .map(|e| e.tick)
                    .min()
                    .map(|tick| (m, tick))
            })
            .min_by_key(|(_, tick)| *tick)
            .map(|(m, _)| m)
    }
    pub fn voice_measure(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        voice: Voice,
    ) -> Option<&PlacedMeasure> {
        self.voice_measure_at_tick(part, staff, measure, voice, 0.)
    }
    pub fn voice_measure_at_tick(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        voice: Voice,
        tick: f64,
    ) -> Option<&PlacedMeasure> {
        self.measures.iter().find(|m| {
            m.part == part
                && m.source_staff == staff
                && m.index == measure
                && m.regions.iter().any(|r| {
                    r.voice == voice && f64::from(r.start) <= tick && tick < f64::from(r.end)
                })
        })
    }
    pub fn cursor(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        index: usize,
    ) -> Option<Point> {
        self.voice_cursor(part, staff, measure, Voice::One, index)
    }
    pub fn voice_cursor(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        voice: Voice,
        index: usize,
    ) -> Option<Point> {
        let source = self
            .measures
            .iter()
            .find(|m| m.part == part && m.source_staff == staff && m.index == measure)?;
        let tick = f64::from(*source.source_onsets[usize::from(voice == Voice::Two)].get(index)?);
        let capacity = source.content.time_positions.last()?.0;
        let placed = self.voice_measure_at_tick(
            part,
            staff,
            measure,
            voice,
            tick.min(f64::from(capacity) - 0.001),
        )?;
        Some(Point {
            x: (placed.origin.x
                + *if voice == Voice::One || placed.source_voice.is_some() {
                    placed.content.cursor_x.get(index)?
                } else {
                    placed.content.second_cursor_x.get(index)?
                })
                * self.scale,
            y: placed.origin.y * self.scale,
        })
    }

    pub fn measure_at(&self, point: Point) -> Option<(PartId, StaffId, usize)> {
        let point = Point {
            x: point.x / self.scale,
            y: point.y / self.scale,
        };
        self.measures
            .iter()
            .find(|measure| {
                measure.content.staff_spans.iter().any(|(left, right)| {
                    point.x >= measure.origin.x + left && point.x <= measure.origin.x + right
                }) && point.y >= measure.origin.y - 30.
                    && point.y <= measure.origin.y + 70.
            })
            .map(|measure| (measure.part, measure.staff, measure.index))
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Rhythm {
    ticks: Vec<u16>,
    positions: Vec<f32>,
}
impl Rhythm {
    fn ticks(score: &Score, index: usize, metrics: &Metrics) -> (Vec<u16>, Vec<f32>, f32) {
        let mut ticks = vec![0, score.time_at(index).capacity()];
        let mut wide = HashSet::new();
        let mut left = HashMap::<u16, f32>::new();
        let mut right = HashMap::<u16, f32>::new();
        let piano_arpeggios: HashSet<_> = score
            .piano_arpeggios()
            .iter()
            .filter(|mark| mark.upper.measure == index)
            .flat_map(|mark| [mark.upper, mark.lower])
            .collect();
        for part in score.parts() {
            for (tick, _) in part.divisi_changes_in(index) {
                ticks.push(tick);
                wide.insert(tick);
            }
            for staff in part.staves() {
                let signs = measure_accidentals(
                    &staff.measures()[index],
                    score.written_key_at(part.id(), index),
                );
                for (_, tick, id, event) in staff.measures()[index].timed_iter() {
                    if staff.measures()[index].has_two_voices()
                        || event.duration.is_dotted()
                        || event.notation.dynamic.is_some()
                        || event.notation.technique.is_some()
                        || event.notation.harmonic.is_some()
                        || event.notation.fermata
                    {
                        wide.insert(tick);
                    }
                    if let Some(mark) = event.notation.dynamic {
                        let (l, _, r, _) = metrics.bounds(mark.glyph(), false);
                        let width =
                            (r - l).max(mark.label().chars().count() as f32 * 14. + 6.) + 6.;
                        right
                            .entry(tick)
                            .and_modify(|v| *v = v.max(width))
                            .or_insert(width);
                    }
                    let padding = accidental_padding(&signs[&id])
                        + if event.notation.arpeggio.is_some()
                            || piano_arpeggios.contains(&ElementId {
                                part: part.id(),
                                staff: staff.id(),
                                measure: index,
                                event: id,
                            })
                        {
                            20.
                        } else {
                            0.
                        };
                    left.entry(tick)
                        .and_modify(|value| *value = value.max(padding))
                        .or_insert(padding);
                    ticks.push(tick);
                    if event
                        .pitches
                        .iter()
                        .zip(event.pitches.iter().skip(1))
                        .any(|(a, b)| b.diatonic() - a.diatonic() == 1)
                    {
                        wide.insert(tick);
                    }
                    ticks.push(tick + event.duration.ticks());
                }
            }
        }
        ticks.sort_unstable();
        ticks.dedup();
        let gaps = ticks
            .windows(2)
            .map(|pair| {
                let normal: f32 = if wide.contains(&pair[0]) || wide.contains(&pair[1]) {
                    40.
                } else {
                    28.
                };
                let normal = normal.max(right.get(&pair[0]).copied().unwrap_or(0.));
                let padding = left.get(&pair[1]).copied().unwrap_or(0.);
                if padding > 0. {
                    normal.max(35. + padding)
                } else {
                    normal
                }
            })
            .collect();
        (ticks, gaps, left.get(&0).copied().unwrap_or(0.))
    }
    fn new(grid: (Vec<u16>, Vec<f32>, f32), prefix: f32, width: f32) -> Self {
        let (ticks, gaps, leading) = grid;
        let prefix = prefix + leading;
        let weights: Vec<f32> = ticks
            .windows(2)
            .map(|pair| f32::from(pair[1] - pair[0]).sqrt())
            .collect();
        let minimum = gaps.iter().sum::<f32>();
        let scale = (width - prefix - 22. - minimum).max(0.) / weights.iter().sum::<f32>();
        let mut x = prefix;
        let mut positions = vec![x];
        for (weight, gap) in weights.into_iter().zip(gaps) {
            x += gap + weight * scale;
            positions.push(x);
        }
        Self { ticks, positions }
    }
    fn x(&self, tick: u16) -> f32 {
        self.positions[self
            .ticks
            .binary_search(&tick)
            .expect("shared musical onset")]
    }
}
struct CacheEntry {
    regions: Vec<VoiceRegion>,
    boundaries: Vec<(u16, score_core::Division)>,
    source: (StaffId, Option<Voice>),
    measure: Measure,
    time: TimeSignature,
    width: f32,
    first: bool,
    clef: Clef,
    key_signature: KeySignature,
    previous_key: Option<KeySignature>,
    time_changed: bool,
    rhythm: Arc<Rhythm>,
    content: Arc<MeasureLayout>,
    ink_top: f32,
    ink_bottom: f32,
}
struct Engraving<'a> {
    part: PartId,
    staff: StaffId,
    index: usize,
    width: f32,
    first: bool,
    time: TimeSignature,
    clef: Clef,
    key_signature: KeySignature,
    previous_key: Option<KeySignature>,
    time_changed: bool,
    rhythm: &'a Rhythm,
    hidden: &'a HashSet<score_core::EventId>,
    accidental_resets: &'a [u16],
}

struct Metrics {
    staff: f32,
    bar: f32,
    ledger: f32,
    extension: f32,
    head: f32,
    whole: f32,
    stem: f32,
    hairpin: f32,
    glyph_bounds: HashMap<char, (f32, f32, f32, f32, f32)>,
    arpeggio_repeat: f32,
}

impl Metrics {
    fn bravura() -> Self {
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../../../assets/fonts/bravura_metadata.json"))
                .expect("bundled Bravura metadata");
        let value = |key: &str| {
            data["engravingDefaults"][key]
                .as_f64()
                .expect("engraving metric") as f32
                * STAFF_SPACE
        };
        let width = |key: &str| {
            data["glyphBBoxes"][key]["bBoxNE"][0]
                .as_f64()
                .expect("glyph width") as f32
                * STAFF_SPACE
        };
        let face = ttf_parser::Face::parse(include_bytes!("../../../assets/fonts/Bravura.otf"), 0)
            .expect("bundled Bravura");
        let scale = MUSIC_SIZE / f32::from(face.units_per_em());
        let glyph_bounds = (0xe000..=0xe5ff)
            .chain([
                0xe614, 0xe883, 0xe634, 0xe635, 0xe63c, 0xeaa9, 0xeaaa, 0xeaad, 0xeaae,
            ])
            .filter_map(|code| {
                let character = char::from_u32(code)?;
                let bounds = face.glyph_bounding_box(face.glyph_index(character)?)?;
                Some((
                    character,
                    (
                        f32::from(bounds.x_min) * scale,
                        -f32::from(bounds.y_max) * scale,
                        f32::from(bounds.x_max) * scale,
                        -f32::from(bounds.y_min) * scale,
                        f32::from(face.glyph_hor_advance(face.glyph_index(character)?)?) * scale,
                    ),
                ))
            })
            .collect();
        Self {
            staff: value("staffLineThickness"),
            bar: value("thinBarlineThickness"),
            ledger: value("legerLineThickness"),
            extension: value("legerLineExtension"),
            head: width("noteheadBlack"),
            whole: width("noteheadWhole"),
            stem: value("stemThickness"),
            hairpin: value("hairpinThickness"),
            glyph_bounds,
            arpeggio_repeat: data["glyphsWithAnchors"]["wiggleArpeggiatoUp"]["repeatOffset"][0]
                .as_f64()
                .expect("Bravura arpeggio repeat") as f32
                * STAFF_SPACE,
        }
    }
    fn bounds(&self, text: &str, rotated: bool) -> (f32, f32, f32, f32) {
        let (mut left, mut top, mut right, mut bottom) = (
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        let mut advance = 0.;
        for character in text.chars() {
            let (l, t, r, b, width) = self
                .glyph_bounds
                .get(&character)
                .copied()
                .unwrap_or((0., -44., 20., 44., 20.));
            let (l, t, r, b) = if rotated {
                (t, -r - advance, b, -l - advance)
            } else {
                (l + advance, t, r + advance, b)
            };
            left = left.min(l);
            top = top.min(t);
            right = right.max(r);
            bottom = bottom.max(b);
            advance += width;
        }
        (left, top, right, bottom)
    }
}

pub struct LayoutEngine {
    cache: HashMap<(PartId, StaffId, usize), CacheEntry>,
    metrics: Metrics,
    orchestra_page: Option<(Score, f32)>,
    pub recomputed_measures: usize,
}
impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            cache: HashMap::new(),
            metrics: Metrics::bravura(),
            orchestra_page: None,
            recomputed_measures: 0,
        }
    }
}

impl LayoutEngine {
    pub fn layout(&mut self, score: &Score, width: f32) -> Layout {
        self.layout_with_spacing(score, width, false, false, None, false)
    }
    pub fn layout_compact(&mut self, score: &Score, width: f32) -> Layout {
        self.layout_with_spacing(score, width, true, false, None, false)
    }
    fn layout_with_spacing(
        &mut self,
        score: &Score,
        width: f32,
        compact: bool,
        unwrapped: bool,
        pinned: Option<&Layout>,
        part_layout: bool,
    ) -> Layout {
        let width = if width.is_finite() {
            width.max(500.)
        } else {
            960.
        };
        let left =
            if part_layout {
                if score.parts().iter().any(|p| {
                    p.instrument().is_grand_staff() || p.has_passage_divisi() || p.is_divisi()
                }) {
                    96.
                } else {
                    56.
                }
            } else {
                160.
            };
        let available = width - left - 32.;
        self.cache.retain(|(part, staff, index), _| {
            *index < score.measures().len()
                && score
                    .part(*part)
                    .is_some_and(|part| part.display_staves(*index).any(|id| id == *staff))
        });
        self.recomputed_measures = 0;
        let mut systems = Vec::new();
        let mut placed = Vec::new();
        let mut labels = Vec::new();
        let mut connectors = Vec::new();
        let mut braces = Vec::new();
        let mut brackets = Vec::new();
        // Index connection lanes once; avoid scanning every link for every staff row.
        let mut curve_lanes = HashMap::<_, (bool, bool)>::new();
        for c in score.connections().iter().filter(|c| !c.kind.is_hairpin()) {
            let below = score.voice_of(c.start) == Some(Voice::Two);
            for index in c.start.measure..=c.end.measure {
                let lanes = curve_lanes
                    .entry((c.start.part, c.start.staff, index))
                    .or_default();
                if c.kind == score_core::ConnectionKind::Tie {
                    // Ties can turn either way, depending on the chord/stem.
                    lanes.0 = true;
                    lanes.1 = true;
                } else if below {
                    lanes.1 = true;
                } else {
                    lanes.0 = true;
                }
            }
        }
        let hairpin_lanes: HashSet<_> = score
            .connections()
            .iter()
            .filter(|c| c.kind.is_hairpin())
            .flat_map(|c| {
                (c.start.measure..=c.end.measure)
                    .map(move |index| (c.start.part, c.start.staff, index))
            })
            .collect();
        let mut row_start = 0;
        let mut y = if compact { 20. } else { 40. };
        while row_start < score.measures().len() {
            let system_top = y;
            let mut columns = Vec::new();
            let mut occupied = 0.;
            let mut tempo_right = f32::NEG_INFINITY;
            let pinned_system = pinned
                .and_then(|layout| layout.systems.iter().find(|s| s.first_measure == row_start));
            let row_end = if let Some(system) = pinned_system {
                system.last_measure + 1
            } else if unwrapped {
                score.measures().len()
            } else {
                (row_start + if part_layout { 6 } else { 4 }).min(score.measures().len())
            };
            for index in row_start..row_end {
                let prefix = signature_prefix(score, index, columns.is_empty());
                let ticks = Rhythm::ticks(score, index, &self.metrics);
                let minimum = pinned
                    .and_then(|layout| layout.measures.iter().find(|m| m.index == index))
                    .map(|m| m.width)
                    .unwrap_or_else(|| {
                        (prefix + ticks.2 + ticks.1.iter().sum::<f32>() + 22.)
                            .max(155.)
                            .max(score.tempo_change_at(index).map_or(0., |mark| {
                                let label = prefix + tempo_width(mark) + 20.;
                                if !part_layout || label > available {
                                    label
                                } else {
                                    0.
                                }
                            }))
                    });
                if pinned.is_none()
                    && !unwrapped
                    && !columns.is_empty()
                    && occupied + minimum > available
                {
                    break;
                }
                // A tempo heading may span several bars in a part. Break
                // before a heading that would overlap another or leave the row.
                if part_layout
                    && !unwrapped
                    && pinned.is_none()
                    && let Some(mark) = score.tempo_change_at(index)
                {
                    let start = occupied + prefix + ticks.2;
                    let right = start + tempo_width(mark) + 20.;
                    if !columns.is_empty() && (start < tempo_right || right > available) {
                        break;
                    }
                    tempo_right = right;
                }
                occupied += minimum;
                columns.push((ticks, minimum));
            }
            let extra = if unwrapped || pinned.is_some() {
                0.
            } else {
                (available - occupied).max(0.) / columns.len() as f32
            };
            let mut column_extra = vec![extra; columns.len()];
            if part_layout {
                let mut preceding_width = 0.;
                for (offset, (ticks, minimum)) in columns.iter().enumerate() {
                    if let Some(mark) = score.tempo_change_at(row_start + offset) {
                        let label_start = preceding_width
                            + signature_prefix(score, row_start + offset, offset == 0)
                            + ticks.2;
                        let allowance = (available - label_start - tempo_width(mark) - 20.).max(0.);
                        let before: f32 = column_extra[..offset].iter().sum();
                        if before > allowance {
                            // Keep the heading inside the page while preserving
                            // the justified right edge: stretch its own bar instead.
                            for stretch in &mut column_extra[..offset] {
                                *stretch *= allowance / before;
                            }
                            column_extra[offset] += before - allowance;
                        }
                    }
                    preceding_width += minimum;
                }
            }
            let rhythms: Vec<_> = columns
                .iter()
                .enumerate()
                .map(|(offset, (ticks, minimum))| {
                    if let Some(measure) = pinned.and_then(|layout| {
                        layout
                            .measures
                            .iter()
                            .find(|m| m.index == row_start + offset)
                    }) {
                        return Arc::new(Rhythm {
                            ticks: measure
                                .content
                                .time_positions
                                .iter()
                                .map(|(t, _)| *t)
                                .collect(),
                            positions: measure
                                .content
                                .time_positions
                                .iter()
                                .map(|(_, x)| x + 8.)
                                .collect(),
                        });
                    }
                    Arc::new(Rhythm::new(
                        ticks.clone(),
                        signature_prefix(score, row_start + offset, offset == 0),
                        minimum + column_extra[offset],
                    ))
                })
                .collect();
            if (row_start..row_start + columns.len())
                .any(|bar| score.tempo_change_at(bar).is_some())
            {
                y += 40.;
            }
            let mut groups: Vec<(InstrumentFamily, f32, f32, usize)> = Vec::new();
            let row_first = placed.len();
            let mut first_staff_y = None;
            let mut last_staff_y = 0.;
            for part in score.parts() {
                let family = part.instrument().family();
                if groups.last().is_some_and(|group| group.0 != family) {
                    y += 16.;
                }
                let part_start = placed.len();
                let mut part_top = None;
                let mut part_bottom = 0.;
                let row_divisi =
                    (row_start..row_start + columns.len()).any(|bar| part.bar_has_divisi(bar));
                let row_staves = std::iter::once(StaffId::Upper).chain(
                    (part.instrument().is_grand_staff() || row_divisi).then_some(StaffId::Lower),
                );
                for staff in row_staves {
                    let first_placed = placed.len();
                    let mut x = left;
                    let mut top = if compact { -24f32 } else { -42f32 };
                    let mut bottom = if compact { 64f32 } else { 80f32 };
                    for (offset, (_, minimum)) in columns.iter().enumerate() {
                        let index = row_start + offset;
                        let width = minimum + column_extra[offset];
                        if staff == StaffId::Lower
                            && !part.instrument().is_grand_staff()
                            && !part.bar_has_divisi(index)
                        {
                            x += width;
                            continue;
                        }
                        let first = offset == 0
                            || (staff == StaffId::Lower
                                && !part.instrument().is_grand_staff()
                                && !part.bar_has_divisi(index - 1));
                        let previous_key = previous_signature(score, part.id(), index);
                        let time_changed = score.time_change_at(index).is_some();
                        let measure = part.display_measure(staff, index);
                        let hidden = part.hidden_display_events(staff, index);
                        let resets = Vec::new();
                        let regions =
                            divisi::regions(part, staff, index, score.time_at(index).capacity());
                        let (source_staff, source_voice) = part.display_source(staff, index);
                        let clef = part.clef(staff);
                        let rhythm = &rhythms[offset];
                        let key = (part.id(), staff, index);
                        if !self.cache.get(&key).is_some_and(|cached| {
                            cached.boundaries == part.divisi_changes_in(index).collect::<Vec<_>>()
                                && cached.regions == regions
                                && cached.source == (source_staff, source_voice)
                                && cached.measure == measure
                                && cached.time == score.time_at(index)
                                && cached.width == width
                                && cached.first == first
                                && cached.clef == clef
                                && cached.key_signature == score.written_key_at(part.id(), index)
                                && cached.previous_key == previous_key
                                && cached.time_changed == time_changed
                                && cached.rhythm == *rhythm
                        }) {
                            let mut content = Arc::new(engrave(
                                &measure,
                                Engraving {
                                    part: part.id(),
                                    staff: source_staff,
                                    index,
                                    width,
                                    first,
                                    time: score.time_at(index),
                                    clef,
                                    key_signature: score.written_key_at(part.id(), index),
                                    previous_key,
                                    time_changed,
                                    rhythm,
                                    hidden: &hidden,
                                    accidental_resets: &resets,
                                },
                                &self.metrics,
                            ));
                            if let Some(voice) = source_voice {
                                for event in &mut Arc::make_mut(&mut content).events {
                                    event.voice = voice;
                                }
                            }
                            divisi::finish(Arc::make_mut(&mut content), part, staff, index, rhythm);
                            let mut ink_top = 0f32;
                            let mut ink_bottom = 40f32;
                            for line in &content.lines {
                                ink_top =
                                    ink_top.min(line.from.y.min(line.to.y) - line.thickness / 2.);
                                ink_bottom = ink_bottom
                                    .max(line.from.y.max(line.to.y) + line.thickness / 2.);
                            }
                            for glyph in &content.glyphs {
                                let (_, top, _, bottom) =
                                    self.metrics.bounds(&glyph.text, glyph.rotated);
                                ink_top = ink_top.min(glyph.position.y + top);
                                ink_bottom = ink_bottom.max(glyph.position.y + bottom);
                            }
                            for point in content
                                .beams
                                .iter()
                                .flat_map(|g| &g.segments)
                                .flat_map(|s| s.corners)
                            {
                                ink_top = ink_top.min(point.y);
                                ink_bottom = ink_bottom.max(point.y);
                            }
                            for text in &content.techniques {
                                ink_top = ink_top.min(text.position.y - 16.);
                                ink_bottom = ink_bottom.max(text.position.y + 4.);
                            }
                            Arc::make_mut(&mut content).ink_top = ink_top;
                            Arc::make_mut(&mut content).ink_bottom = ink_bottom;
                            self.cache.insert(
                                key,
                                CacheEntry {
                                    regions: regions.clone(),
                                    boundaries: part.divisi_changes_in(index).collect(),
                                    source: (source_staff, source_voice),
                                    measure: measure.clone(),
                                    time: score.time_at(index),
                                    width,
                                    first,
                                    clef,
                                    key_signature: score.written_key_at(part.id(), index),
                                    previous_key,
                                    time_changed,
                                    rhythm: rhythm.clone(),
                                    content,
                                    ink_top,
                                    ink_bottom,
                                },
                            );
                            self.recomputed_measures += 1;
                        }
                        let content = self.cache[&key].content.clone();
                        let cached = &self.cache[&key];
                        top = top.min(if compact { cached.ink_top } else { content.top });
                        bottom = bottom.max(if compact {
                            cached.ink_bottom
                        } else {
                            content.bottom
                        });
                        placed.push(PlacedMeasure {
                            system_start: row_start,
                            start_tick: score.measure_start_tick(index),
                            part: part.id(),
                            staff,
                            source_staff,
                            source_voice,
                            regions,
                            source_onsets: std::array::from_fn(|v| {
                                let voice = Voice::ALL[v];
                                let bar = &part.staff(source_staff).unwrap().measures()[index];
                                bar.timed_iter()
                                    .filter(|(v, _, _, _)| *v == voice)
                                    .map(|(_, tick, _, _)| tick)
                                    .chain(std::iter::once(bar.voice_ticks(voice)))
                                    .collect()
                            }),
                            index,
                            origin: Point { x, y: 0. },
                            width,
                            content,
                        });
                        x += width;
                    }
                    if staff == StaffId::Upper {
                        for measure in &placed[first_placed..] {
                            for (tick, mark) in part.divisi_changes_in(measure.index) {
                                if !part.measures()[measure.index]
                                    .timed_iter()
                                    .any(|(_, t, _, _)| t == tick)
                                {
                                    labels.push(StaffLabel {
                                        staff: None,
                                        text: mark.label().into(),
                                        part: part.id(),
                                        position: Point {
                                            x: measure.origin.x
                                                + measure.content.x_at_tick(f64::from(tick)),
                                            y: 0.,
                                        },
                                    });
                                }
                            }
                        }
                    }
                    // Reserve curved-connection lanes separately from cached local ink.
                    let (above, below) = (row_start..row_start + columns.len())
                        .filter_map(|index| {
                            curve_lanes.get(&(
                                part.id(),
                                part.display_source(staff, index).0,
                                index,
                            ))
                        })
                        .fold((false, false), |a, b| (a.0 || b.0, a.1 || b.1));
                    let (notation_top, notation_bottom, division_y) =
                        expression::arrange_indications(
                            &mut placed[first_placed..],
                            &self.metrics,
                            above,
                        );
                    top = placed[first_placed..]
                        .iter()
                        .map(|bar| {
                            if compact {
                                bar.content.ink_top
                            } else {
                                bar.content.top
                            }
                        })
                        .fold(if compact { -24f32 } else { -42f32 }, f32::min);
                    if labels
                        .iter()
                        .any(|label| label.part == part.id() && label.position.y == 0.)
                    {
                        top = top.min(division_y - 16.);
                    }
                    if above {
                        top = top.min(notation_top - 22.);
                    }
                    if below {
                        bottom = bottom.max(notation_bottom + 22.);
                    }
                    if (row_start..row_start + columns.len()).any(|index| {
                        hairpin_lanes.contains(&(
                            part.id(),
                            part.display_source(staff, index).0,
                            index,
                        ))
                    }) {
                        bottom = bottom.max(
                            expression::hairpin_y(
                                placed[first_placed..].iter().map(|m| m.content.as_ref()),
                                below,
                            ) + 6.,
                        );
                    }
                    let origin_y = pinned
                        .and_then(|layout| {
                            layout.measures.iter().find(|m| {
                                m.part == part.id()
                                    && m.staff == staff
                                    && m.system_start == row_start
                            })
                        })
                        .map_or(y - top, |m| m.origin.y);
                    for label in labels
                        .iter_mut()
                        .filter(|label| label.part == part.id() && label.position.y == 0.)
                    {
                        label.position.y = origin_y + division_y;
                    }
                    for measure in &mut placed[first_placed..] {
                        measure.origin.y = origin_y;
                    }
                    if !part_layout || row_divisi || part.instrument().is_grand_staff() {
                        labels.push(StaffLabel {
                            staff: Some(staff),
                            text: if row_divisi {
                                match staff {
                                    StaffId::Upper => "div. I",
                                    StaffId::Lower => "div. II",
                                }
                                .into()
                            } else if part.instrument().is_grand_staff() {
                                match staff {
                                    StaffId::Upper => "M. derecha",
                                    StaffId::Lower => "M. izquierda",
                                }
                                .into()
                            } else {
                                score.part_name(part.id())
                            },
                            position: Point {
                                x: 24.,
                                y: origin_y + 24.,
                            },
                            part: part.id(),
                        });
                    }
                    first_staff_y.get_or_insert(origin_y);
                    last_staff_y = origin_y;
                    part_top.get_or_insert(origin_y);
                    part_bottom = origin_y + 40.;
                    y += bottom - top + if compact { 16. } else { 28. };
                }
                let top = part_top.expect("part upper staff");
                if let Some(group) = groups.last_mut().filter(|group| group.0 == family) {
                    group.2 = part_bottom;
                    group.3 += 1;
                } else {
                    groups.push((family, top, part_bottom, 1));
                }
                if part.instrument().is_grand_staff() || row_divisi {
                    let top = part_top.expect("two-staff part");
                    if part.instrument().is_grand_staff() {
                        braces.push(Brace {
                            x: left - 16.,
                            top,
                            bottom: part_bottom,
                        });
                    } else if score.parts().len() == 1 && row_divisi {
                        brackets.push(Bracket {
                            x: left - 12.,
                            top,
                            bottom: part_bottom,
                            family,
                        });
                    }
                    if !part_layout {
                        labels.push(StaffLabel {
                            staff: None,
                            text: score.part_name(part.id()),
                            part: part.id(),
                            position: Point {
                                x: 24.,
                                y: (top + part_bottom) / 2.,
                            },
                        });
                    }
                    for x in std::iter::once(left).chain(
                        placed[part_start..part_start + columns.len()]
                            .iter()
                            .map(|measure| measure.origin.x + measure.width),
                    ) {
                        let connected = part.instrument().is_grand_staff()
                            || placed[part_start..part_start + columns.len()]
                                .iter()
                                .any(|m| {
                                    (x == left && part.divisi_at(m.index))
                                        || (x == m.origin.x + m.width
                                            && part.divisi_at_tick(
                                                m.index,
                                                score.time_at(m.index).capacity() - 1,
                                            ))
                                });
                        if !connected {
                            continue;
                        }
                        connectors.push(Line {
                            from: Point { x, y: top },
                            to: Point { x, y: part_bottom },
                            thickness: self.metrics.bar,
                            element: None,
                        });
                    }
                }
            }
            if score.parts().len() > 1 {
                for (family, top, bottom, parts) in groups {
                    // A single piano already has its own two-staff brace.
                    if family != InstrumentFamily::Keyboards || parts > 1 {
                        brackets.push(Bracket {
                            x: left
                                - if family == InstrumentFamily::Keyboards {
                                    30.
                                } else {
                                    12.
                                },
                            top,
                            bottom,
                            family,
                        });
                    }
                    if parts > 1 {
                        for measure in &placed[row_first..row_first + columns.len()] {
                            connectors.push(Line {
                                from: Point {
                                    x: measure.origin.x + measure.width,
                                    y: top,
                                },
                                to: Point {
                                    x: measure.origin.x + measure.width,
                                    y: bottom,
                                },
                                thickness: self.metrics.bar,
                                element: None,
                            });
                        }
                    }
                }
                connectors.push(Line {
                    from: Point {
                        x: left,
                        y: first_staff_y.unwrap(),
                    },
                    to: Point {
                        x: left,
                        y: last_staff_y + 40.,
                    },
                    thickness: self.metrics.bar,
                    element: None,
                });
            }
            systems.push(System {
                first_measure: row_start,
                last_measure: row_start + columns.len() - 1,
                top: system_top,
                bottom: y,
            });
            y += 32.;
            row_start += columns.len();
        }
        let tempos = score
            .tempo_marks()
            .filter_map(|(measure, mark)| {
                let bar = placed.iter().find(|bar| bar.index == measure)?;
                let system = systems.iter().find(|system| {
                    (system.first_measure..=system.last_measure).contains(&measure)
                })?;
                Some(TempoLabel {
                    measure,
                    mark: mark.clone(),
                    width: tempo_width(mark),
                    position: Point {
                        x: bar.origin.x + bar.content.x_at_tick(0.),
                        y: system.top + 28.,
                    },
                })
            })
            .collect();
        let index = expression::PlacedIndex::new(&placed);
        let curves = expression::curves(score, &index, &self.metrics);
        let hairpins = expression::hairpins(score, &index, self.metrics.hairpin);
        let piano_arpeggios = expression::piano_arpeggios(score, &placed, &self.metrics);
        Layout {
            curves,
            hairpins,
            piano_arpeggios,
            scale: 1.,
            mode: ViewMode::Continuous,
            pages: Vec::new(),
            systems,
            width: placed
                .iter()
                .map(|m| m.origin.x + m.width + 32.)
                .fold(width, f32::max),
            height: y + 24.,
            measures: placed,
            labels,
            tempos,
            connectors,
            braces,
            brackets,
        }
    }
}

fn tempo_width(mark: &score_core::TempoMark) -> f32 {
    // Conservative advances at 18px: ordinary words stay compact while wide
    // letters and non-Latin text still leave clearance before the music.
    mark.text
        .chars()
        .map(|c| match c {
            'W' | 'M' | '@' => 18.,
            'm' | 'w' => 15.,
            'i' | 'l' | 'j' | 'I' | 'f' | 't' | 'r' => 7.,
            ' ' => 5.,
            c if c.is_ascii_uppercase() => 14.,
            c if c.is_ascii_alphanumeric() => 10.,
            c if c.is_ascii_punctuation() => 7.,
            _ => 18.,
        })
        .sum::<f32>()
        + if mark.text.is_empty() { 0. } else { 12. }
        + 86.
}

fn key_width(key: KeySignature) -> f32 {
    if key.count() == 0 {
        0.
    } else {
        key.count() as f32 * 11. + 8.
    }
}
fn previous_signature(score: &Score, part: PartId, index: usize) -> Option<KeySignature> {
    (index > 0 && score.key_change_at(index).is_some())
        .then(|| score.written_key_at(part, index - 1))
}
fn cancel_steps(old: KeySignature, key: KeySignature) -> Vec<score_core::Step> {
    old.steps()
        .iter()
        .copied()
        .filter(|step| old.accidental(*step) != key.accidental(*step))
        .collect()
}
fn prefix_for(
    first: bool,
    key: KeySignature,
    previous: Option<KeySignature>,
    time_changed: bool,
) -> f32 {
    let cancellations = previous.map_or(0, |old| cancel_steps(old, key).len());
    let cancel_width = if cancellations == 0 {
        0.
    } else {
        cancellations as f32 * 11. + 8.
    };
    if first {
        88. + key_width(key) + cancel_width
    } else {
        24. + if previous.is_some() {
            key_width(key) + cancel_width + 8.
        } else {
            0.
        } + if time_changed { 40. } else { 0. }
    }
}
fn signature_prefix(score: &Score, index: usize, first: bool) -> f32 {
    let entering_staff = index > 0
        && score
            .parts()
            .iter()
            .any(|part| part.bar_has_divisi(index) && !part.bar_has_divisi(index - 1));
    score
        .parts()
        .iter()
        .map(|part| {
            prefix_for(
                first || entering_staff,
                score.written_key_at(part.id(), index),
                previous_signature(score, part.id(), index),
                score.time_change_at(index).is_some(),
            )
        })
        .fold(0., f32::max)
}
fn key_positions(clef: Clef, flats: bool) -> [f32; 7] {
    match (clef, flats) {
        (Clef::Treble, false) => [0., 15., -5., 10., 25., 5., 20.],
        (Clef::Treble, true) => [20., 5., 25., 10., 30., 15., 35.],
        (Clef::Alto, false) => [5., 20., 0., 15., 30., 10., 25.],
        (Clef::Alto, true) => [25., 10., 30., 15., 35., 20., 40.],
        (Clef::Bass, false) => [10., 25., 5., 20., 35., 15., 30.],
        (Clef::Bass, true) => [30., 15., 35., 20., 40., 25., 10.],
        (Clef::Percussion, _) => [20.; 7],
    }
}

#[derive(Clone, Copy)]
struct PlacedAccidental {
    pitch: score_core::Pitch,
    sign: i8,
    touch: bool,
    column: usize,
}

fn written_pitches(
    event: score_core::Event,
    key: KeySignature,
) -> Vec<(score_core::Pitch, i8, bool, bool)> {
    event
        .pitches
        .iter()
        .map(|pitch| {
            (
                pitch,
                pitch.in_key(key).alteration(),
                false,
                pitch.accidental().is_some(),
            )
        })
        .chain(
            event
                .harmonic_touch(key)
                .map(|(pitch, alteration)| (pitch, alteration, true, false)),
        )
        .collect()
}

fn measure_accidentals(
    measure: &Measure,
    key: KeySignature,
) -> HashMap<score_core::EventId, Vec<PlacedAccidental>> {
    measure_accidentals_with_resets(measure, key, &[])
}
fn measure_accidentals_with_resets(
    measure: &Measure,
    key: KeySignature,
    resets: &[u16],
) -> HashMap<score_core::EventId, Vec<PlacedAccidental>> {
    // Unmarked pitches follow the key; explicit signs override it. Remember local
    // notation per octave and reset to the signature at each measure boundary.
    let mut state = [0i8; 63];
    for step in key.steps() {
        for octave in 0..9 {
            state[octave * 7 + step.index() as usize] = key.accidental(*step).unwrap().semitones();
        }
    }
    let initial = state;
    let mut previous = None;
    let mut events: Vec<_> = measure.timed_iter().collect();
    events.sort_by_key(|(voice, tick, _, _)| (*tick, *voice));
    let mut alterations = HashMap::new();
    let mut conflicts = HashSet::new();
    for (_, tick, _, event) in &events {
        for (pitch, alteration, _, _) in written_pitches(**event, key) {
            let position = (*tick, pitch.diatonic());
            if alterations
                .insert(position, alteration)
                .is_some_and(|old| old != alteration)
            {
                conflicts.insert(position);
            }
        }
    }
    events
        .into_iter()
        .map(|(_, tick, id, event)| {
            if resets
                .iter()
                .any(|&at| at <= tick && previous.is_none_or(|before| at > before))
            {
                state = initial;
            }
            previous = Some(tick);
            let mut signs = Vec::<PlacedAccidental>::new();
            let mut columns = Vec::<i16>::new();
            for (pitch, alteration, touch, explicit) in
                written_pitches(*event, key).into_iter().rev()
            {
                let position = pitch.diatonic() as usize;
                let sign = (explicit
                    || state[position] != alteration
                    || conflicts.contains(&(tick, pitch.diatonic())))
                .then_some(alteration);
                state[position] = if conflicts.contains(&(tick, pitch.diatonic())) {
                    i8::MAX
                } else {
                    alteration
                };
                if let Some(sign) = sign {
                    // Seven diatonic steps leave 35 px between the tallest signs.
                    let column = columns
                        .iter()
                        .position(|previous| (pitch.diatonic() - previous).abs() >= 7)
                        .unwrap_or(columns.len());
                    if column == columns.len() {
                        columns.push(pitch.diatonic());
                    } else {
                        columns[column] = pitch.diatonic();
                    }
                    signs.push(PlacedAccidental {
                        pitch,
                        sign,
                        touch,
                        column,
                    });
                }
            }
            (id, signs)
        })
        .collect()
}
fn accidental_padding(signs: &[PlacedAccidental]) -> f32 {
    signs
        .iter()
        .map(|sign| (sign.column + 1) as f32 * 14. + 22.)
        .fold(0., f32::max)
}
fn engrave_accidentals(
    result: &mut MeasureLayout,
    id: ElementId,
    signs: &[PlacedAccidental],
    head_width: f32,
) {
    let left = result
        .events
        .iter()
        .filter(|head| head.id == id)
        .map(|head| head.center.x - head_width / 2.)
        .fold(f32::INFINITY, f32::min);
    for sign in signs {
        let y = if sign.touch {
            result
                .marks
                .iter()
                .find(|mark| mark.mark == MarkRef::Harmonic(id))
                .expect("engraved touch head")
                .center
                .y
        } else {
            result
                .events
                .iter()
                .find(|head| head.id == id && head.pitch == Some(sign.pitch))
                .expect("engraved pitch")
                .center
                .y
        };
        let code = match sign.sign {
            -2 => 0xe264,
            -1 => 0xe260,
            0 => 0xe261,
            1 => 0xe262,
            2 => 0xe263,
            _ => unreachable!("supported accidental"),
        };
        result.glyphs.push(Glyph {
            rotated: false,
            text: char::from_u32(code).unwrap().to_string(),
            position: Point {
                x: left - 8. - (sign.column + 1) as f32 * 14.,
                y,
            },
            element: Some(id),
        });
    }
}

fn voice_offset(measure: &Measure, voice: Voice, tick: u16) -> f32 {
    if voice != Voice::Two {
        return 0.;
    }
    let one = measure
        .timed_iter()
        .find(|(v, t, _, _)| *v == Voice::One && *t == tick);
    let two = measure
        .timed_iter()
        .find(|(v, t, _, _)| *v == Voice::Two && *t == tick);
    match (one, two) {
        (Some((_, _, _, a)), Some((_, _, _, b)))
            if a.pitches == b.pitches
                && a.duration == b.duration
                && a.notation.harmonic == b.notation.harmonic
                && a.notation.harmonic != Some(score_core::Harmonic::Artificial) =>
        {
            0.
        }
        (Some((_, _, _, a)), Some((_, _, _, b)))
            if written_pitches(*a, KeySignature::default())
                .iter()
                .any(|(a, _, _, _)| {
                    written_pitches(*b, KeySignature::default())
                        .iter()
                        .any(|(b, _, _, _)| (a.diatonic() - b.diatonic()).abs() <= 1)
                }) =>
        {
            14.
        }
        _ => 0.,
    }
}

fn engrave(measure: &Measure, options: Engraving<'_>, metrics: &Metrics) -> MeasureLayout {
    let Engraving {
        part,
        staff,
        index,
        width,
        first,
        time,
        clef,
        key_signature,
        previous_key,
        time_changed,
        rhythm,
        hidden,
        accidental_resets,
    } = options;
    let mut result = MeasureLayout {
        lines: Vec::new(),
        glyphs: Vec::new(),
        techniques: Vec::new(),
        marks: Vec::new(),
        tuplets: Vec::new(),
        beams: Vec::new(),
        events: Vec::new(),
        staff_spans: vec![(0., width)],
        cursor_x: Vec::new(),
        second_cursor_x: Vec::new(),
        time_positions: rhythm
            .ticks
            .iter()
            .copied()
            .zip(rhythm.positions.iter().map(|x| x - 8.))
            .collect(),
        top: -42.,
        bottom: 80.,
        ink_top: 0.,
        ink_bottom: 40.,
        music_ink_bottom: 40.,
    };
    let mut line = |x1, y1, x2, y2, thickness| {
        result.lines.push(Line {
            from: Point { x: x1, y: y1 },
            to: Point { x: x2, y: y2 },
            thickness,
            element: None,
        })
    };
    for n in 0..5 {
        line(
            0.,
            n as f32 * STAFF_SPACE,
            width,
            n as f32 * STAFF_SPACE,
            metrics.staff,
        );
    }
    line(width, 0., width, 40., metrics.bar);
    if first {
        line(0., 0., 0., 40., metrics.bar);
    }
    if first {
        result.glyphs.push(Glyph {
            rotated: false,
            text: match clef {
                Clef::Treble => "\u{e050}",
                Clef::Alto => "\u{e05c}",
                Clef::Bass => "\u{e062}",
                Clef::Percussion => "\u{e069}",
            }
            .into(),
            position: Point {
                x: 10.,
                y: match clef {
                    Clef::Treble => 30.,
                    Clef::Alto => 20.,
                    Clef::Bass => 10.,
                    Clef::Percussion => 20.,
                },
            },
            element: None,
        });
    }
    let key_x = if first { 42. } else { 10. };
    let cancellations = previous_key.map_or_else(Vec::new, |old| cancel_steps(old, key_signature));
    let cancel_width = if cancellations.is_empty() {
        0.
    } else {
        cancellations.len() as f32 * 11. + 8.
    };
    if first || previous_key.is_some() {
        if let Some(old) = previous_key {
            let positions = key_positions(clef, old.fifths() < 0);
            for (column, step) in cancellations.iter().enumerate() {
                let row = old.steps().iter().position(|s| s == step).unwrap();
                result.glyphs.push(Glyph {
                    rotated: false,
                    text: "\u{e261}".into(),
                    position: Point {
                        x: key_x + column as f32 * 11.,
                        y: positions[row],
                    },
                    element: None,
                });
            }
        }
        let positions = key_positions(clef, key_signature.fifths() < 0);
        for (column, _) in key_signature.steps().iter().enumerate() {
            result.glyphs.push(Glyph {
                rotated: false,
                text: if key_signature.fifths() < 0 {
                    "\u{e260}"
                } else {
                    "\u{e262}"
                }
                .into(),
                position: Point {
                    x: key_x + cancel_width + column as f32 * 11.,
                    y: positions[column],
                },
                element: None,
            });
        }
    }
    if time_changed {
        let key_space = if first || previous_key.is_some() {
            key_width(key_signature) + cancel_width
        } else {
            0.
        };
        for (number, y) in [(time.numerator, 10.), (time.denominator, 30.)] {
            let text = number
                .to_string()
                .chars()
                .map(|c| char::from_u32(0xe080 + c.to_digit(10).unwrap()).unwrap())
                .collect();
            result.glyphs.push(Glyph {
                rotated: false,
                text,
                position: Point {
                    x: if first { 48. } else { 10. } + key_space,
                    y,
                },
                element: None,
            });
        }
    }
    let start = prefix_for(first, key_signature, previous_key, time_changed);
    let signs = measure_accidentals_with_resets(measure, key_signature, accidental_resets);
    let beams = beaming::Plan::new(measure, part, staff, index, time, clef, rhythm);
    for (voice, tick, id, event) in measure.timed_iter() {
        let onset = rhythm.x(tick) + voice_offset(measure, voice, tick);
        let forced = measure.has_two_voices().then_some(voice == Voice::Two);
        if voice == Voice::One {
            result.cursor_x.push(onset - 8.);
        } else {
            result.second_cursor_x.push(onset - 8.);
        }
        if hidden.contains(&id) {
            continue;
        }
        let element = ElementId {
            staff,
            part,
            measure: index,
            event: id,
        };
        if event.pitches.len() > 1
            || event.notation.harmonic == Some(score_core::Harmonic::Artificial)
        {
            engrave_chord(
                &mut result,
                *event,
                element,
                onset,
                clef,
                metrics,
                forced.or(beams.direction(element)),
                beams.direction(element).is_some(),
                voice,
                key_signature,
            );
            engrave_accidentals(
                &mut result,
                element,
                &signs[&id],
                if event.duration.value() == score_core::NoteValue::Whole {
                    metrics.whole
                } else {
                    metrics.head
                },
            );
            expression::arpeggio(&mut result, *event, element, metrics);
            rhythm::dots(&mut result, *event, element, metrics);
            continue;
        }
        let y = event
            .pitches
            .first()
            .map(|pitch| {
                if clef == Clef::Percussion {
                    20.
                } else {
                    40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.
                }
            })
            .unwrap_or(if measure.has_two_voices() {
                if voice == Voice::One { 5. } else { 35. }
            } else {
                20.
            });
        let head_width = if event.duration.value() == score_core::NoteValue::Whole {
            metrics.whole
        } else {
            metrics.head
        };
        // Align notehead centers, including the wider whole-note glyph.
        let x = onset + (metrics.head - head_width) / 2.;
        let code = if beams.direction(element).is_some() {
            0xe0a4 // A beamed note has a separate stem and no flag.
        } else if event.pitches.first().is_some() {
            let down = u32::from(forced.unwrap_or(y <= 20.));
            match event.duration.value() {
                score_core::NoteValue::Whole => 0xe1d2,
                score_core::NoteValue::Half => 0xe1d3 + down,
                score_core::NoteValue::Quarter => 0xe1d5 + down,
                score_core::NoteValue::Eighth => 0xe1d7 + down,
                score_core::NoteValue::Sixteenth => 0xe1d9 + down,
            }
        } else {
            match event.duration.value() {
                score_core::NoteValue::Whole => 0xe4e3,
                score_core::NoteValue::Half => 0xe4e4,
                score_core::NoteValue::Quarter => 0xe4e5,
                score_core::NoteValue::Eighth => 0xe4e6,
                score_core::NoteValue::Sixteenth => 0xe4e7,
            }
        };
        let glyph_y = if event.pitches.first().is_none()
            && event.duration.value() == score_core::NoteValue::Whole
        {
            if measure.has_two_voices() {
                if voice == Voice::One { 0. } else { 30. }
            } else {
                10.
            }
        } else {
            y
        };
        result.glyphs.push(Glyph {
            rotated: false,
            text: char::from_u32(code).unwrap().to_string(),
            position: Point { x, y: glyph_y },
            element: Some(element),
        });
        result.events.push(EventPosition {
            tick: 0,
            end_tick: 0,
            voice,
            id: element,
            pitch: event.pitches.first(),
            center: Point {
                x: x + head_width / 2.,
                y,
            },
        });
        engrave_accidentals(&mut result, element, &signs[&id], head_width);
        result.top = result.top.min(y - 44.);
        result.bottom = result.bottom.max(y + 44.);
        if event.pitches.first().is_some() {
            let mut ledger_y = -10.;
            while ledger_y >= y {
                result.lines.push(Line {
                    from: Point {
                        x: x - metrics.extension,
                        y: ledger_y,
                    },
                    to: Point {
                        x: x + head_width + metrics.extension,
                        y: ledger_y,
                    },
                    thickness: metrics.ledger,
                    element: None,
                });
                ledger_y -= 10.;
            }
            let mut ledger_y = 50.;
            while ledger_y <= y {
                result.lines.push(Line {
                    from: Point {
                        x: x - metrics.extension,
                        y: ledger_y,
                    },
                    to: Point {
                        x: x + head_width + metrics.extension,
                        y: ledger_y,
                    },
                    thickness: metrics.ledger,
                    element: None,
                });
                ledger_y += 10.;
            }
        }
        rhythm::dots(&mut result, *event, element, metrics);
    }
    result
        .cursor_x
        .push(rhythm.x(measure.voice_ticks(Voice::One)) - 8.);
    result
        .second_cursor_x
        .push(rhythm.x(measure.voice_ticks(Voice::Two)) - 8.);
    if measure.voice_len(Voice::One) == 0 {
        result.glyphs.push(Glyph {
            rotated: false,
            text: "\u{e4e3}".into(),
            position: Point {
                x: start + (width - start) * 0.4,
                y: if measure.has_two_voices() { 0. } else { 10. },
            },
            element: None,
        });
    }
    beams.engrave(&mut result, metrics);
    let timing: HashMap<_, _> = measure
        .timed_iter()
        .map(|(_, tick, id, event)| (id, (tick, tick + event.duration.ticks())))
        .collect();
    for head in &mut result.events {
        let (start, end) = timing[&head.id.event];
        head.tick = start;
        head.end_tick = end;
    }
    // Measure the actual musical ink before adding expression lanes. A head
    // does not occupy a hypothetical 44 units on both sides of the staff.
    result.ink_top = 0.;
    result.ink_bottom = 40.;
    for glyph in result.glyphs.iter().filter(|g| g.element.is_some()) {
        let (_, top, _, bottom) = metrics.bounds(&glyph.text, glyph.rotated);
        result.ink_top = result.ink_top.min(glyph.position.y + top);
        result.ink_bottom = result.ink_bottom.max(glyph.position.y + bottom);
    }
    for line in result.lines.iter().filter(|l| l.element.is_some()) {
        result.ink_top = result.ink_top.min(line.from.y.min(line.to.y));
        result.ink_bottom = result.ink_bottom.max(line.from.y.max(line.to.y));
    }
    for point in result
        .beams
        .iter()
        .flat_map(|g| &g.segments)
        .flat_map(|s| s.corners)
    {
        result.ink_top = result.ink_top.min(point.y);
        result.ink_bottom = result.ink_bottom.max(point.y);
    }
    result.music_ink_bottom = result.ink_bottom;
    for (event, value) in measure.iter().filter(|(id, _)| !hidden.contains(id)) {
        expression::marks(
            &mut result,
            *value,
            ElementId {
                part,
                staff,
                measure: index,
                event,
            },
        );
    }
    rhythm::tuplets(&mut result, measure, part, staff, index, metrics);
    result
}

#[allow(clippy::too_many_arguments)]
fn engrave_chord(
    result: &mut MeasureLayout,
    event: score_core::Event,
    id: ElementId,
    onset: f32,
    clef: Clef,
    metrics: &Metrics,
    forced_direction: Option<bool>,
    beamed: bool,
    voice: Voice,
    key: KeySignature,
) {
    let y_at = |pitch: score_core::Pitch| {
        if clef == Clef::Percussion {
            20.
        } else {
            40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.
        }
    };
    let low = y_at(event.pitches.first().expect("chord"));
    let touch = event.harmonic_touch(key).map(|(pitch, _)| pitch);
    let high = y_at(touch.or_else(|| event.pitches.last()).expect("chord"));
    let down = forced_direction.unwrap_or((low + high) / 2. <= 20.);
    let head_width = if event.duration.value() == score_core::NoteValue::Whole {
        metrics.whole
    } else {
        metrics.head
    };
    let base = onset + (metrics.head - head_width) / 2.;
    let head = match event.duration.value() {
        score_core::NoteValue::Whole => 0xe0a2,
        score_core::NoteValue::Half => 0xe0a3,
        _ => 0xe0a4,
    };
    let mut pitches: Vec<_> = event
        .pitches
        .iter()
        .map(|pitch| (pitch, false))
        .chain(touch.map(|pitch| (pitch, true)))
        .collect();
    if down {
        pitches.reverse();
    }
    let mut previous: Option<i16> = None;
    let mut shifted = false;
    for (pitch, generated) in pitches {
        shifted = if previous.is_some_and(|previous| (pitch.diatonic() - previous).abs() == 1) {
            !shifted
        } else {
            false
        };
        previous = Some(pitch.diatonic());
        let x = base
            + if shifted {
                if down { -head_width } else { head_width }
            } else {
                0.
            };
        let y = y_at(pitch);
        // Keep the touched diamond attached to the same stem. Whole notes
        // have no stem, so center it over the stopped note instead.
        let diamond = score_core::Harmonic::Artificial.glyph();
        let (l, _, r, _) = metrics.bounds(diamond, false);
        let x = if generated {
            x + if event.duration.value() == score_core::NoteValue::Whole {
                (head_width - (r - l)) / 2. - l
            } else if down {
                -l
            } else {
                head_width - r
            }
        } else {
            x
        };
        let glyph_index = result.glyphs.len();
        result.glyphs.push(Glyph {
            rotated: false,
            text: if generated {
                diamond.into()
            } else {
                char::from_u32(head).unwrap().to_string()
            },
            position: Point { x, y },
            element: Some(id),
        });
        if generated {
            result.marks.push(expression::MarkBox {
                mark: MarkRef::Harmonic(id),
                glyph: Some(glyph_index),
                center: Point {
                    x: x + (l + r) / 2.,
                    y,
                },
                half: ((r - l) / 2. + 3., 7.),
            });
        } else {
            result.events.push(EventPosition {
                tick: 0,
                end_tick: 0,
                voice,
                id,
                pitch: Some(pitch),
                center: Point {
                    x: x + head_width / 2.,
                    y,
                },
            });
        }
        result.top = result.top.min(y - 44.);
        result.bottom = result.bottom.max(y + 44.);
        let mut ledger = -10.;
        while ledger >= y {
            result.lines.push(Line {
                from: Point {
                    x: x - metrics.extension,
                    y: ledger,
                },
                to: Point {
                    x: x + head_width + metrics.extension,
                    y: ledger,
                },
                thickness: metrics.ledger,
                element: None,
            });
            ledger -= 10.;
        }
        let mut ledger = 50.;
        while ledger <= y {
            result.lines.push(Line {
                from: Point {
                    x: x - metrics.extension,
                    y: ledger,
                },
                to: Point {
                    x: x + head_width + metrics.extension,
                    y: ledger,
                },
                thickness: metrics.ledger,
                element: None,
            });
            ledger += 10.;
        }
    }
    if !beamed && event.duration.value() != score_core::NoteValue::Whole {
        let stem_x = if down { onset } else { onset + metrics.head };
        let end_y = if down { low + 35. } else { high - 35. };
        result.lines.push(Line {
            from: Point {
                x: stem_x,
                y: if down { high } else { low },
            },
            to: Point {
                x: stem_x,
                y: end_y,
            },
            thickness: metrics.stem,
            element: Some(id),
        });
        let flag = match event.duration.value() {
            score_core::NoteValue::Eighth => Some(if down { 0xe241 } else { 0xe240 }),
            score_core::NoteValue::Sixteenth => Some(if down { 0xe243 } else { 0xe242 }),
            _ => None,
        };
        if let Some(flag) = flag {
            result.glyphs.push(Glyph {
                rotated: false,
                text: char::from_u32(flag).unwrap().to_string(),
                position: Point {
                    x: stem_x,
                    y: end_y,
                },
                element: Some(id),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Event, Pitch, Step};

    #[test]
    fn presentation_scale_keeps_chord_hit_targets_cursor_and_staff_selection_aligned() {
        let mut score = Score::default();
        let pitches = score_core::Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::E, 4).unwrap(),
            Pitch::new(Step::G, 4).unwrap(),
        ])
        .unwrap();
        let id = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let mut layout = LayoutEngine::default().layout_compact(&score, 960.);
        let cursor = layout.cursor(id.part, id.staff, 0, 1).unwrap();
        for scale in [0.5, 0.75, 1.5] {
            layout.scale = scale;
            let bar = layout.measure_on(id.part, id.staff, 0).unwrap();
            for head in &bar.content.events {
                let point = Point {
                    x: (bar.origin.x + head.center.x) * scale,
                    y: (bar.origin.y + head.center.y) * scale,
                };
                assert_eq!(layout.hit_pitch(point), Some((id, head.pitch)));
            }
            let at = layout.cursor(id.part, id.staff, 0, 1).unwrap();
            assert_eq!(
                at,
                Point {
                    x: cursor.x * scale,
                    y: cursor.y * scale
                }
            );
            assert_eq!(
                layout.measure_at(Point {
                    x: (bar.origin.x + bar.width / 2.) * scale,
                    y: (bar.origin.y + 20.) * scale
                }),
                Some((id.part, id.staff, 0))
            );
        }
    }

    #[test]
    fn compact_spacing_uses_actual_ink_and_keeps_extreme_chords_separate() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        score.add_part(Instrument::Viola).unwrap();
        let normal = LayoutEngine::default().layout(&score, 960.);
        let mut engine = LayoutEngine::default();
        let compact = engine.layout_compact(&score, 960.);
        assert!(compact.height < normal.height);
        for (staff, octave) in [(StaffId::Upper, 8), (StaffId::Lower, 0)] {
            let pitches = score_core::Pitches::new(&[
                Pitch::new(Step::C, octave).unwrap(),
                Pitch::new(Step::E, octave).unwrap(),
                Pitch::new(Step::G, octave).unwrap(),
            ])
            .unwrap();
            score
                .insert_on(
                    piano,
                    staff,
                    0,
                    0,
                    Event {
                        notation: Default::default(),
                        pitches,
                        duration: Duration::Eighth,
                    },
                )
                .unwrap();
        }
        let layout = engine.layout_compact(&score, 960.);
        let mut previous_bottom = f32::NEG_INFINITY;
        for bar in layout.measures.iter().filter(|bar| bar.index == 0) {
            let ink = &engine.cache[&(bar.part, bar.staff, bar.index)];
            assert!(bar.origin.y + ink.ink_top >= previous_bottom + 15.9);
            previous_bottom = bar.origin.y + ink.ink_bottom;
        }
        engine.layout_compact(&score, 960.);
        assert_eq!(engine.recomputed_measures, 0);
    }

    #[test]
    fn signatures_repeat_at_system_starts_in_all_clefs_with_room_before_time_and_notes() {
        use score_core::KeyMode;
        for fifths in [-7, -1, 0, 1, 7] {
            let mut score = Score::default();
            score.set_key_signature(KeySignature::new(fifths, KeyMode::Major).unwrap());
            score.add_part(Instrument::Viola).unwrap();
            for _ in 0..2 {
                score.append_measure().unwrap();
            }
            let parts: Vec<_> = score.parts().iter().map(|part| part.id()).collect();
            for part in parts {
                score
                    .insert_in(
                        part,
                        0,
                        0,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap();
            }
            let layout = LayoutEngine::default().layout(&score, 700.);
            let first_x = layout.measures[0].origin.x;
            for bar in &layout.measures {
                let is_first = bar.origin.x == first_x;
                let signs: Vec<_> = bar
                    .content
                    .glyphs
                    .iter()
                    .filter(|g| {
                        g.element.is_none() && ["\u{e260}", "\u{e262}"].contains(&g.text.as_str())
                    })
                    .collect();
                assert_eq!(
                    signs.len(),
                    if is_first {
                        fifths.unsigned_abs() as usize
                    } else {
                        0
                    }
                );
                if !signs.is_empty() {
                    let last = signs.last().unwrap().position.x + 10.;
                    assert!(bar.content.cursor_x[0] > last);
                    if bar.index == 0 {
                        let time = bar
                            .content
                            .glyphs
                            .iter()
                            .find(|g| g.text == "\u{e084}")
                            .unwrap();
                        assert!(time.position.x > last);
                    }
                    let clef = score.part(bar.part).unwrap().clef(bar.staff);
                    let positions = key_positions(clef, fifths < 0);
                    assert_eq!(
                        signs.iter().map(|g| g.position.y).collect::<Vec<_>>(),
                        positions[..signs.len()]
                    );
                }
            }
            assert!(
                layout
                    .measures
                    .iter()
                    .any(|bar| bar.index > 0 && bar.origin.x == first_x)
            );
        }
    }

    #[test]
    fn explicit_natural_cancels_the_key_and_unmarked_note_restores_it_and_cache_updates() {
        use score_core::KeyMode;
        let mut score = Score::default();
        score.set_key_signature(KeySignature::new(1, KeyMode::Major).unwrap());
        let f = Pitch::new(Step::F, 4).unwrap();
        let mut ids = Vec::new();
        for (index, pitch) in [f.with_accidental(Some(Accidental::Natural)), f]
            .into_iter()
            .enumerate()
        {
            ids.push(
                score
                    .insert(
                        0,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Some(pitch)),
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap(),
            );
        }
        let next = score
            .insert(
                1,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Some(f)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let mut engine = LayoutEngine::default();
        let layout = engine.layout(&score, 960.);
        for (id, code) in ids.into_iter().zip(["\u{e261}", "\u{e262}"]) {
            assert!(
                layout
                    .measure_on(id.part, id.staff, id.measure)
                    .unwrap()
                    .content
                    .glyphs
                    .iter()
                    .any(|g| g.element == Some(id) && g.text == code)
            );
        }
        assert!(
            !layout
                .measure_on(next.part, next.staff, next.measure)
                .unwrap()
                .content
                .glyphs
                .iter()
                .any(|g| g.element == Some(next)
                    && ["\u{e260}", "\u{e261}", "\u{e262}"].contains(&g.text.as_str()))
        );
        engine.layout(&score, 960.);
        assert_eq!(engine.recomputed_measures, 0);
        // Same number of signs and same spacing; only the signature changes.
        score.set_key_signature(KeySignature::new(-1, KeyMode::Major).unwrap());
        let changed = engine.layout(&score, 960.);
        assert_eq!(engine.recomputed_measures, score.measures().len() * 2);
        assert!(
            changed.measures[0]
                .content
                .glyphs
                .iter()
                .any(|g| g.element.is_none() && g.text == "\u{e260}")
        );
        assert!(
            !changed.measures[0]
                .content
                .glyphs
                .iter()
                .any(|g| g.element.is_none() && g.text == "\u{e262}")
        );
    }

    #[test]
    fn accidentals_cancel_within_a_measure_and_reset_between_hands_and_bars() {
        let c = Pitch::new(Step::C, 4).unwrap();
        let mut score = Score::default();
        let signs = [
            Some(Accidental::Sharp),
            None,
            Some(Accidental::Flat),
            Some(Accidental::Natural),
        ];
        let mut ids = Vec::new();
        for (index, sign) in signs.into_iter().enumerate() {
            ids.push(
                score
                    .insert(
                        0,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Some(c.with_accidental(sign))),
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap(),
            );
        }
        let next = score
            .insert(
                1,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Some(c)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let left = score
            .insert_on(
                ids[0].part,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Some(c)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let layout = LayoutEngine::default().layout(&score, 960.);
        let expected = ["\u{e262}", "\u{e261}", "\u{e260}", "\u{e261}"];
        for (id, code) in ids.into_iter().zip(expected) {
            let bar = layout.measure_on(id.part, id.staff, id.measure).unwrap();
            let sign = bar
                .content
                .glyphs
                .iter()
                .find(|g| g.element == Some(id) && g.text == code)
                .unwrap();
            assert!(sign.position.x > 0.);
            let head = bar
                .content
                .events
                .iter()
                .find(|head| head.id == id)
                .unwrap();
            assert!(sign.position.x + 12. < head.center.x);
        }
        for id in [next, left] {
            let bar = layout.measure_on(id.part, id.staff, id.measure).unwrap();
            assert!(!bar.content.glyphs.iter().any(|g| g.element == Some(id)
                && ["\u{e260}", "\u{e261}", "\u{e262}"].contains(&g.text.as_str())));
        }
    }

    #[test]
    fn dense_chord_signs_have_columns_clearance_and_individual_hit_targets() {
        let pitches = score_core::Pitches::new(
            &[
                Step::C,
                Step::D,
                Step::E,
                Step::F,
                Step::G,
                Step::A,
                Step::B,
            ]
            .map(|step| {
                Pitch::new(step, 4)
                    .unwrap()
                    .with_accidental(Some(Accidental::Sharp))
            }),
        )
        .unwrap();
        let mut score = Score::default();
        let mut ids = Vec::new();
        for index in 0..4 {
            ids.push(
                score
                    .insert(
                        0,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches,
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap(),
            );
        }
        let layout = LayoutEngine::default().layout(&score, 500.);
        let bar = layout.measure_on(ids[0].part, StaffId::Upper, 0).unwrap();
        let mut previous_right = 68.; // First clef/time ends before this point.
        for id in ids {
            let signs: Vec<_> = bar
                .content
                .glyphs
                .iter()
                .filter(|g| g.element == Some(id) && g.text == "\u{e262}")
                .collect();
            assert_eq!(signs.len(), 7);
            let left = signs
                .iter()
                .map(|g| g.position.x)
                .fold(f32::INFINITY, f32::min);
            assert!(
                left > previous_right,
                "signs must clear the preceding figure"
            );
            for a in &signs {
                for b in &signs {
                    if !std::ptr::eq(*a, *b) {
                        assert!(
                            (a.position.x - b.position.x).abs() >= 14.
                                || (a.position.y - b.position.y).abs() >= 35.
                        );
                    }
                }
            }
            previous_right = bar
                .content
                .events
                .iter()
                .filter(|head| head.id == id)
                .map(|head| head.center.x + 18.)
                .fold(0., f32::max);
            for head in bar.content.events.iter().filter(|head| head.id == id) {
                let target = layout.hit_pitch(Point {
                    x: bar.origin.x + head.center.x,
                    y: bar.origin.y + head.center.y,
                });
                assert_eq!(target, Some((id, head.pitch)));
            }
        }
        assert!(bar.width > 500.); // Horizontal scrolling instead of overlapping signs.
    }

    #[test]
    fn chord_heads_share_one_stem_and_each_head_selects_the_chord() {
        for duration in Duration::ALL {
            let mut score = Score::default();
            let pitches = score_core::Pitches::new(&[
                Pitch::new(Step::C, 4).unwrap(),
                Pitch::new(Step::E, 4).unwrap(),
                Pitch::new(Step::G, 4).unwrap(),
            ])
            .unwrap();
            let id = score
                .insert(
                    0,
                    0,
                    Event {
                        notation: Default::default(),
                        pitches,
                        duration,
                    },
                )
                .unwrap();
            let layout = LayoutEngine::default().layout(&score, 960.);
            let measure = layout.measure_on(id.part, id.staff, id.measure).unwrap();
            let heads: Vec<_> = measure
                .content
                .events
                .iter()
                .filter(|event| event.id == id)
                .collect();
            assert_eq!(heads.len(), 3);
            assert_eq!(measure.content.cursor_x.len(), 2); // One rhythmic event.
            assert!(heads.iter().all(|head| head.center.x == heads[0].center.x));
            let stems = measure
                .content
                .lines
                .iter()
                .filter(|line| {
                    line.from.x == line.to.x
                        && line.from.x > 80.
                        && line.from.x < measure.width - 20.
                })
                .count();
            assert_eq!(stems, usize::from(duration != Duration::Whole));
            for head in heads {
                assert_eq!(
                    layout.hit_test(Point {
                        x: measure.origin.x + head.center.x,
                        y: measure.origin.y + head.center.y
                    }),
                    Some(id)
                );
            }
            let flags = measure
                .content
                .glyphs
                .iter()
                .filter(|glyph| {
                    glyph
                        .text
                        .chars()
                        .any(|c| (0xe240..=0xe243).contains(&(c as u32)))
                })
                .count();
            assert_eq!(
                flags,
                usize::from(matches!(duration, Duration::Eighth | Duration::Sixteenth))
            );
        }
    }

    #[test]
    fn close_chord_tones_alternate_sides_and_get_enough_rhythmic_space() {
        let mut score = Score::default();
        let pitches = score_core::Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::D, 4).unwrap(),
            Pitch::new(Step::E, 4).unwrap(),
        ])
        .unwrap();
        let first = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let second = score
            .insert(
                0,
                1,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let layout = LayoutEngine::default().layout(&score, 500.);
        let measure = layout.measure_on(first.part, first.staff, 0).unwrap();
        let a: Vec<_> = measure
            .content
            .events
            .iter()
            .filter(|head| head.id == first)
            .collect();
        let b: Vec<_> = measure
            .content
            .events
            .iter()
            .filter(|head| head.id == second)
            .collect();
        assert!(
            a.windows(2)
                .all(|pair| (pair[1].center.x - pair[0].center.x).abs() >= 11.)
        );
        let a_right = a
            .iter()
            .map(|head| head.center.x)
            .fold(f32::NEG_INFINITY, f32::max);
        let b_left = b
            .iter()
            .map(|head| head.center.x)
            .fold(f32::INFINITY, f32::min);
        assert!(b_left - a_right > 35.);
        assert!(measure.content.bottom >= 94.);
    }

    #[test]
    fn grand_staff_aligns_the_hands_and_connects_them_with_a_brace() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let right = score
            .insert_on(
                piano,
                StaffId::Upper,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let left = score
            .insert_on(
                piano,
                StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let mut engine = LayoutEngine::default();
        let layout = engine.layout(&score, 960.);
        let upper = layout.measure_on(piano, StaffId::Upper, 0).unwrap();
        let lower = layout.measure_on(piano, StaffId::Lower, 0).unwrap();
        assert_eq!(upper.origin.x, lower.origin.x);
        assert_eq!(upper.width, lower.width);
        assert_eq!(
            upper.content.events[0].center.x,
            lower.content.events[0].center.x
        );
        assert_eq!(upper.content.events[0].center.y, 50.);
        assert_eq!(lower.content.events[0].center.y, 25.);
        assert!(
            upper
                .content
                .glyphs
                .iter()
                .any(|glyph| glyph.text == "\u{e050}")
        );
        assert!(
            lower
                .content
                .glyphs
                .iter()
                .any(|glyph| glyph.text == "\u{e062}")
        );
        assert!(
            layout
                .braces
                .iter()
                .any(|brace| brace.top == upper.origin.y && brace.bottom == lower.origin.y + 40.)
        );
        assert!(
            layout
                .connectors
                .iter()
                .any(|line| line.from.x == upper.origin.x + upper.width
                    && line.from.y == upper.origin.y
                    && line.to.y == lower.origin.y + 40.)
        );
        for id in [right, left] {
            let placed = layout.measure_on(id.part, id.staff, id.measure).unwrap();
            let event = &placed.content.events[0];
            assert_eq!(
                layout.hit_test(Point {
                    x: placed.origin.x + event.center.x,
                    y: placed.origin.y + event.center.y
                }),
                Some(id)
            );
            assert!(layout.cursor(id.part, id.staff, id.measure, 1).is_some());
        }
        assert_eq!(
            layout.measure_at(Point {
                x: lower.origin.x + 5.,
                y: lower.origin.y + 20.
            }),
            Some((piano, StaffId::Lower, 0))
        );
        let upper_content = upper.content.clone();
        score.transpose(left, -1).unwrap();
        let updated = engine.layout(&score, 960.);
        assert_eq!(engine.recomputed_measures, 1);
        assert!(Arc::ptr_eq(
            &upper_content,
            &updated
                .measure_on(piano, StaffId::Upper, 0)
                .unwrap()
                .content
        ));
    }

    #[test]
    fn short_notes_keep_clearance_before_long_implicit_remainders() {
        let mut score = Score::default();
        for measure in 0..4 {
            for index in 0..2 {
                score
                    .insert(
                        measure,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Sixteenth,
                        },
                    )
                    .unwrap();
            }
        }
        for denominator in [1, 4] {
            let mut data = serde_json::to_value(&score).unwrap();
            data["time"] = serde_json::json!({ "numerator": 16, "denominator": denominator });
            let long_score: Score = serde_json::from_value(data).unwrap();
            long_score.validate().unwrap();
            let layout = LayoutEngine::default().layout(&long_score, 960.);
            for measure in &layout.measures {
                for pair in measure.content.events.windows(2) {
                    assert!(pair[1].center.x - pair[0].center.x >= 27.99);
                }
            }
        }
    }

    #[test]
    fn whole_and_quarter_noteheads_share_the_same_center_at_a_shared_onset() {
        let mut score = Score::default();
        let quarter = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let cello = score.add_part(score_core::Instrument::Cello).unwrap();
        let whole = score
            .insert_in(
                cello,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let layout = LayoutEngine::default().layout(&score, 960.);
        let center = |id: ElementId| {
            layout
                .measure(id.part, id.measure)
                .unwrap()
                .content
                .events
                .iter()
                .find(|event| event.id == id)
                .unwrap()
                .center
                .x
        };
        assert_eq!(center(quarter), center(whole));
    }

    #[test]
    fn ensemble_aligns_musical_onsets_and_keeps_part_hit_testing_separate() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let cello = score.add_part(score_core::Instrument::Cello).unwrap();
        let half = Event {
            notation: Default::default(),
            pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Half,
        };
        let first = score.insert_in(piano, 0, 0, half).unwrap();
        let later = score.insert_in(piano, 0, 1, half).unwrap();
        let mut cello_notes = Vec::new();
        for index in 0..4 {
            cello_notes.push(
                score
                    .insert_in(
                        cello,
                        0,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap(),
            );
        }
        let mut engine = LayoutEngine::default();
        let layout = engine.layout(&score, 960.);
        let p = layout.measure(piano, 0).unwrap();
        let c = layout.measure(cello, 0).unwrap();
        assert_eq!(p.origin.x, c.origin.x);
        assert_eq!(p.width, c.width);
        assert_eq!(p.content.events[0].center.x, c.content.events[0].center.x);
        assert_eq!(p.content.events[1].center.x, c.content.events[2].center.x);
        assert!(c.origin.y + c.content.top > p.origin.y + p.content.bottom);
        assert!(
            c.content
                .glyphs
                .iter()
                .any(|glyph| glyph.text == "\u{e062}")
        );
        for (part, ids) in [(piano, vec![first, later]), (cello, cello_notes)] {
            let placed = layout.measure(part, 0).unwrap();
            for id in ids {
                let event = placed
                    .content
                    .events
                    .iter()
                    .find(|event| event.id == id)
                    .unwrap();
                assert_eq!(
                    layout.hit_test(Point {
                        x: placed.origin.x + event.center.x,
                        y: placed.origin.y + event.center.y
                    }),
                    Some(id)
                );
            }
            assert_eq!(
                layout.measure_at(Point {
                    x: placed.origin.x + 5.,
                    y: placed.origin.y + 20.
                }),
                Some((part, StaffId::Upper, 0))
            );
        }
        let cached_piano = p.content.clone();
        score.transpose(c.content.events[0].id, 1).unwrap();
        let updated = engine.layout(&score, 960.);
        assert_eq!(engine.recomputed_measures, 1);
        assert!(Arc::ptr_eq(
            &cached_piano,
            &updated.measure(piano, 0).unwrap().content
        ));
    }

    #[test]
    fn alto_clef_places_middle_c_on_the_middle_line() {
        let mut score = Score::default();
        let viola = score.add_part(score_core::Instrument::Viola).unwrap();
        score
            .insert_in(
                viola,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let layout = LayoutEngine::default().layout(&score, 960.);
        let measure = layout.measure(viola, 0).unwrap();
        assert_eq!(measure.content.events[0].center.y, 20.);
        assert!(
            measure
                .content
                .glyphs
                .iter()
                .any(|glyph| glyph.text == "\u{e05c}")
        );
        assert!(
            layout
                .labels
                .iter()
                .any(|label| label.part == viola && label.text == "Viola")
        );
        assert!(!layout.connectors.is_empty());
    }
    #[test]
    fn edits_recompute_only_changed_measure_and_hit_test_tracks_ids() {
        let mut score = Score::default();
        let mut engine = LayoutEngine::default();
        engine.layout(&score, 1000.);
        assert_eq!(engine.recomputed_measures, 8);
        engine.layout(&score, 1000.);
        assert_eq!(engine.recomputed_measures, 0);
        let id = score
            .insert(
                1,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let layout = engine.layout(&score, 1000.);
        assert_eq!(engine.recomputed_measures, 2);
        let measure = &layout.measures[1];
        let note = &measure.content.events[0];
        assert_eq!(note.center.y, 50.); // Middle C: first ledger line below treble staff.
        assert_eq!(
            layout.hit_test(Point {
                x: measure.origin.x + note.center.x,
                y: measure.origin.y + note.center.y
            }),
            Some(id)
        );
        assert_eq!(layout.hit_test(Point { x: -10., y: -10. }), None);
    }

    #[test]
    fn dense_measures_wrap_without_collisions_and_extreme_notes_expand_rows() {
        let mut score = Score::default();
        for index in 0..16 {
            score
                .insert(
                    0,
                    index,
                    score_core::Event {
                        notation: Default::default(),
                        pitches: score_core::Pitches::single(score_core::Pitch::new(
                            score_core::Step::C,
                            0,
                        )),
                        duration: Duration::Sixteenth,
                    },
                )
                .unwrap();
        }
        let layout = LayoutEngine::default().layout(&score, 500.);
        let first = &layout.measures[0];
        for pair in first.content.events.windows(2) {
            assert!(pair[1].center.x - pair[0].center.x >= 25.);
        }
        assert!(layout.measures[1].origin.y > first.origin.y + first.content.bottom);
        assert!(layout.width >= first.origin.x + first.width);
    }
}

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod grouping_tests;

#[cfg(test)]
mod beaming_tests;

#[cfg(test)]
mod views_tests;

#[cfg(test)]
mod voice_tests;

#[cfg(test)]
mod engraving_regression_tests;

#[cfg(test)]
mod harmonic_tests;

#[cfg(test)]
mod part_tests;

mod spacing;

#[cfg(test)]
mod spacing_tests;

#[cfg(test)]
mod orchestra_tests;
