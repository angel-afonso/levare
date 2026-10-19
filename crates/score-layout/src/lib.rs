//! Small, incremental, multi-staff engraver. All coordinates are UI-independent.
use score_core::{
    Accidental, Clef, ElementId, Instrument, InstrumentFamily, KeySignature, Measure, PartId,
    Score, StaffId, TimeSignature,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[cfg(test)]
use score_core::Duration;
mod beaming;
pub use beaming::{BeamGroup, BeamSegment, BeamStem};
mod rhythm;
pub use rhythm::TupletGroup;
mod expression;
pub use expression::{Curve, Hairpin, Hit, MarkBox};

pub const STAFF_SPACE: f32 = 10.;
pub const MUSIC_SIZE: f32 = STAFF_SPACE * 4.;

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
    pub cursor_x: Vec<f32>,
    /// Shared rhythmic onsets, including bar end, for a continuous playhead.
    pub time_positions: Vec<(u16, f32)>,
    pub top: f32,
    pub bottom: f32,
    pub ink_top: f32,
    pub ink_bottom: f32,
}

impl MeasureLayout {
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

#[derive(Clone, Debug)]
pub struct PlacedMeasure {
    pub staff: StaffId,
    pub part: PartId,
    pub index: usize,
    pub origin: Point,
    pub width: f32,
    pub content: Arc<MeasureLayout>,
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
    pub width: f32,
    pub height: f32,
    pub measures: Vec<PlacedMeasure>,
    pub labels: Vec<StaffLabel>,
    pub connectors: Vec<Line>,
    pub braces: Vec<Brace>,
    pub brackets: Vec<Bracket>,
    pub curves: Vec<Curve>,
    pub hairpins: Vec<Hairpin>,
}

impl Layout {
    pub fn hit_test(&self, point: Point) -> Option<ElementId> {
        self.hit_pitch(point).map(|(id, _)| id)
    }
    pub fn hit_pitch(&self, point: Point) -> Option<(ElementId, Option<score_core::Pitch>)> {
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
                (dx.abs() <= 15. && dy.abs() <= 18.)
                    .then_some(((event.id, event.pitch), dx * dx + dy * dy))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(target, _)| target)
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
    pub fn cursor(
        &self,
        part: PartId,
        staff: StaffId,
        measure: usize,
        index: usize,
    ) -> Option<Point> {
        let placed = self.measure_on(part, staff, measure)?;
        Some(Point {
            x: (placed.origin.x + *placed.content.cursor_x.get(index)?) * self.scale,
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
                point.x >= measure.origin.x
                    && point.x <= measure.origin.x + measure.width
                    && point.y >= measure.origin.y - 30.
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
    fn ticks(score: &Score, index: usize) -> (Vec<u16>, Vec<f32>, f32) {
        let mut ticks = vec![0, score.time().capacity()];
        let mut wide = HashSet::new();
        let mut left = HashMap::<u16, f32>::new();
        for part in score.parts() {
            for staff in part.staves() {
                let mut tick = 0;
                let signs = measure_accidentals(&staff.measures()[index], score.key_signature());
                for (id, event) in staff.measures()[index].iter() {
                    if event.duration.is_dotted()
                        || event.notation.dynamic.is_some()
                        || event.notation.technique.is_some()
                    {
                        wide.insert(tick);
                    }
                    let padding = accidental_padding(&signs[&id]);
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
                    tick += event.duration.ticks();
                    ticks.push(tick);
                }
            }
        }
        ticks.sort_unstable();
        ticks.dedup();
        let gaps = ticks
            .windows(2)
            .map(|pair| {
                let normal: f32 = if wide.contains(&pair[0]) || wide.contains(&pair[1]) {
                    72.
                } else {
                    28.
                };
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
    measure: Measure,
    time: TimeSignature,
    width: f32,
    first: bool,
    clef: Clef,
    key_signature: KeySignature,
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
    rhythm: &'a Rhythm,
}

struct Metrics {
    staff: f32,
    bar: f32,
    ledger: f32,
    extension: f32,
    head: f32,
    whole: f32,
    stem: f32,
    glyph_bounds: HashMap<char, (f32, f32)>,
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
            .chain([0xe883])
            .filter_map(|code| {
                let character = char::from_u32(code)?;
                let bounds = face.glyph_bounding_box(face.glyph_index(character)?)?;
                Some((
                    character,
                    (
                        -f32::from(bounds.y_max) * scale,
                        -f32::from(bounds.y_min) * scale,
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
            glyph_bounds,
        }
    }
}

pub struct LayoutEngine {
    cache: HashMap<(PartId, StaffId, usize), CacheEntry>,
    metrics: Metrics,
    pub recomputed_measures: usize,
}
impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            cache: HashMap::new(),
            metrics: Metrics::bravura(),
            recomputed_measures: 0,
        }
    }
}

impl LayoutEngine {
    pub fn layout(&mut self, score: &Score, width: f32) -> Layout {
        self.layout_with_spacing(score, width, false)
    }
    pub fn layout_compact(&mut self, score: &Score, width: f32) -> Layout {
        self.layout_with_spacing(score, width, true)
    }
    fn layout_with_spacing(&mut self, score: &Score, width: f32, compact: bool) -> Layout {
        let width = if width.is_finite() {
            width.max(500.)
        } else {
            960.
        };
        let left = 160.;
        let available = width - left - 32.;
        self.cache.retain(|(part, staff, index), _| {
            score
                .part(*part)
                .and_then(|part| part.staff(*staff))
                .is_some()
                && *index < score.measures().len()
        });
        self.recomputed_measures = 0;
        let mut placed = Vec::new();
        let mut labels = Vec::new();
        let mut connectors = Vec::new();
        let mut braces = Vec::new();
        let mut brackets = Vec::new();
        // Index connection lanes once; avoid scanning every link for every staff row.
        let lanes: HashSet<_> = score
            .connections()
            .iter()
            .filter(|c| !c.kind.is_hairpin())
            .flat_map(|c| {
                (c.start.measure..=c.end.measure)
                    .map(move |index| (c.start.part, c.start.staff, index))
            })
            .collect();
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
            let mut columns = Vec::new();
            let mut occupied = 0.;
            for index in row_start..(row_start + 4).min(score.measures().len()) {
                let prefix = music_prefix(columns.is_empty(), score.key_signature());
                let ticks = Rhythm::ticks(score, index);
                let minimum = (prefix + ticks.2 + ticks.1.iter().sum::<f32>() + 22.).max(155.);
                if !columns.is_empty() && occupied + minimum > available {
                    break;
                }
                occupied += minimum;
                columns.push((ticks, minimum));
            }
            let extra = (available - occupied).max(0.) / columns.len() as f32;
            let rhythms: Vec<_> = columns
                .iter()
                .enumerate()
                .map(|(offset, (ticks, minimum))| {
                    Arc::new(Rhythm::new(
                        ticks.clone(),
                        music_prefix(offset == 0, score.key_signature()),
                        minimum + extra,
                    ))
                })
                .collect();
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
                for staff in part.staves() {
                    let first_placed = placed.len();
                    let mut x = left;
                    let mut top = if compact { -24f32 } else { -42f32 };
                    let mut bottom = if compact { 64f32 } else { 80f32 };
                    for (offset, (_, minimum)) in columns.iter().enumerate() {
                        let index = row_start + offset;
                        let width = minimum + extra;
                        let first = offset == 0;
                        let measure = &staff.measures()[index];
                        let clef = part.clef(staff.id());
                        let rhythm = &rhythms[offset];
                        let key = (part.id(), staff.id(), index);
                        if !self.cache.get(&key).is_some_and(|cached| {
                            cached.measure == *measure
                                && cached.time == score.time()
                                && cached.width == width
                                && cached.first == first
                                && cached.clef == clef
                                && cached.key_signature == score.key_signature()
                                && cached.rhythm == *rhythm
                        }) {
                            let content = Arc::new(engrave(
                                measure,
                                Engraving {
                                    part: part.id(),
                                    staff: staff.id(),
                                    index,
                                    width,
                                    first,
                                    time: score.time(),
                                    clef,
                                    key_signature: score.key_signature(),
                                    rhythm,
                                },
                                &self.metrics,
                            ));
                            let mut ink_top = 0f32;
                            let mut ink_bottom = 40f32;
                            for line in &content.lines {
                                ink_top =
                                    ink_top.min(line.from.y.min(line.to.y) - line.thickness / 2.);
                                ink_bottom = ink_bottom
                                    .max(line.from.y.max(line.to.y) + line.thickness / 2.);
                            }
                            for glyph in &content.glyphs {
                                for character in glyph.text.chars() {
                                    let (top, bottom) = self
                                        .metrics
                                        .glyph_bounds
                                        .get(&character)
                                        .copied()
                                        .unwrap_or((-44., 44.));
                                    ink_top = ink_top.min(glyph.position.y + top);
                                    ink_bottom = ink_bottom.max(glyph.position.y + bottom);
                                }
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
                            let mut content = content;
                            Arc::make_mut(&mut content).ink_top = ink_top;
                            Arc::make_mut(&mut content).ink_bottom = ink_bottom;
                            self.cache.insert(
                                key,
                                CacheEntry {
                                    measure: measure.clone(),
                                    time: score.time(),
                                    width,
                                    first,
                                    clef,
                                    key_signature: score.key_signature(),
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
                            part: part.id(),
                            staff: staff.id(),
                            index,
                            origin: Point { x, y: 0. },
                            width,
                            content,
                        });
                        x += width;
                    }
                    // Reserve curved-connection lanes separately from cached local ink.
                    let has_connection = (row_start..row_start + columns.len())
                        .any(|index| lanes.contains(&(part.id(), staff.id(), index)));
                    if has_connection {
                        top -= 30.;
                        bottom += 22.;
                    }
                    if (row_start..row_start + columns.len())
                        .any(|index| hairpin_lanes.contains(&(part.id(), staff.id(), index)))
                    {
                        bottom += 40.;
                    }
                    let origin_y = y - top;
                    for measure in &mut placed[first_placed..] {
                        measure.origin.y = origin_y;
                    }
                    labels.push(StaffLabel {
                        staff: Some(staff.id()),
                        text: if part.instrument() == Instrument::Piano {
                            match staff.id() {
                                StaffId::Upper => "M. derecha",
                                StaffId::Lower => "M. izquierda",
                            }
                            .into()
                        } else {
                            part.instrument().name().into()
                        },
                        position: Point {
                            x: 24.,
                            y: origin_y + 24.,
                        },
                        part: part.id(),
                    });
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
                if part.instrument() == Instrument::Piano {
                    let top = part_top.expect("piano upper staff");
                    braces.push(Brace {
                        x: left - 16.,
                        top,
                        bottom: part_bottom,
                    });
                    labels.push(StaffLabel {
                        staff: None,
                        text: "Piano".into(),
                        part: part.id(),
                        position: Point {
                            x: 24.,
                            y: (top + part_bottom) / 2.,
                        },
                    });
                    for x in std::iter::once(left).chain(
                        placed[part_start..part_start + columns.len()]
                            .iter()
                            .map(|measure| measure.origin.x + measure.width),
                    ) {
                        connectors.push(Line {
                            from: Point { x, y: top },
                            to: Point { x, y: part_bottom },
                            thickness: 1.,
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
                                thickness: 1.,
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
                    thickness: 1.5,
                    element: None,
                });
            }
            y += 32.;
            row_start += columns.len();
        }
        let curves = expression::curves(score, &placed);
        let hairpins = expression::hairpins(score, &placed);
        Layout {
            curves,
            hairpins,
            scale: 1.,
            width: placed
                .iter()
                .map(|m| m.origin.x + m.width + 32.)
                .fold(width, f32::max),
            height: y + 24.,
            measures: placed,
            labels,
            connectors,
            braces,
            brackets,
        }
    }
}

fn key_width(key: KeySignature) -> f32 {
    if key.count() == 0 {
        0.
    } else {
        key.count() as f32 * 11. + 8.
    }
}
fn music_prefix(first: bool, key: KeySignature) -> f32 {
    if first { 88. + key_width(key) } else { 24. }
}
fn key_positions(clef: Clef, flats: bool) -> [f32; 7] {
    match (clef, flats) {
        (Clef::Treble, false) => [0., 15., -5., 10., 25., 5., 20.],
        (Clef::Treble, true) => [20., 5., 25., 10., 30., 15., 35.],
        (Clef::Alto, false) => [5., 20., 0., 15., 30., 10., 25.],
        (Clef::Alto, true) => [25., 10., 30., 15., 35., 20., 40.],
        (Clef::Bass, false) => [10., 25., 5., 20., 35., 15., 30.],
        (Clef::Bass, true) => [30., 15., 35., 20., 40., 25., 10.],
    }
}

#[derive(Clone, Copy)]
struct PlacedAccidental {
    pitch: score_core::Pitch,
    sign: Accidental,
    column: usize,
}

fn measure_accidentals(
    measure: &Measure,
    key: KeySignature,
) -> HashMap<score_core::EventId, Vec<PlacedAccidental>> {
    // Unmarked pitches follow the key; explicit signs override it. Remember local
    // notation per octave and reset to the signature at each measure boundary.
    let mut state = [0i8; 63];
    for step in key.steps() {
        for octave in 0..9 {
            state[octave * 7 + step.index() as usize] = key.accidental(*step).unwrap().semitones();
        }
    }
    measure
        .iter()
        .map(|(id, event)| {
            let mut signs = Vec::<PlacedAccidental>::new();
            let mut columns = Vec::<i16>::new();
            for pitch in event.pitches.iter().rev() {
                let position = pitch.diatonic() as usize;
                let alteration = pitch.in_key(key).alteration();
                let sign = pitch.accidental().or_else(|| {
                    (state[position] != alteration)
                        .then_some(key.accidental(pitch.step).unwrap_or(Accidental::Natural))
                });
                state[position] = alteration;
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
        let y = result
            .events
            .iter()
            .find(|head| head.id == id && head.pitch == Some(sign.pitch))
            .expect("engraved pitch")
            .center
            .y;
        let code = match sign.sign {
            Accidental::Flat => 0xe260,
            Accidental::Natural => 0xe261,
            Accidental::Sharp => 0xe262,
        };
        result.glyphs.push(Glyph {
            text: char::from_u32(code).unwrap().to_string(),
            position: Point {
                x: left - 8. - (sign.column + 1) as f32 * 14.,
                y,
            },
            element: Some(id),
        });
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
        rhythm,
    } = options;
    let mut result = MeasureLayout {
        lines: Vec::new(),
        glyphs: Vec::new(),
        techniques: Vec::new(),
        marks: Vec::new(),
        tuplets: Vec::new(),
        beams: Vec::new(),
        events: Vec::new(),
        cursor_x: Vec::new(),
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
            text: match clef {
                Clef::Treble => "\u{e050}",
                Clef::Alto => "\u{e05c}",
                Clef::Bass => "\u{e062}",
            }
            .into(),
            position: Point {
                x: 10.,
                y: match clef {
                    Clef::Treble => 30.,
                    Clef::Alto => 20.,
                    Clef::Bass => 10.,
                },
            },
            element: None,
        });
        let positions = key_positions(clef, key_signature.fifths() < 0);
        for (index, _) in key_signature.steps().iter().enumerate() {
            result.glyphs.push(Glyph {
                text: if key_signature.fifths() < 0 {
                    "\u{e260}"
                } else {
                    "\u{e262}"
                }
                .into(),
                position: Point {
                    x: 42. + index as f32 * 11.,
                    y: positions[index],
                },
                element: None,
            });
        }
        if index == 0 {
            for (number, y) in [(time.numerator, 10.), (time.denominator, 30.)] {
                let text = number
                    .to_string()
                    .chars()
                    .map(|c| char::from_u32(0xe080 + c.to_digit(10).unwrap()).unwrap())
                    .collect();
                result.glyphs.push(Glyph {
                    text,
                    position: Point {
                        x: 48. + key_width(key_signature),
                        y,
                    },
                    element: None,
                });
            }
        }
    }
    let start = music_prefix(first, key_signature);
    let mut tick = 0;
    let signs = measure_accidentals(measure, key_signature);
    let beams = beaming::Plan::new(measure, part, staff, index, time, clef, rhythm);
    for (id, event) in measure.iter() {
        result.cursor_x.push(rhythm.x(tick) - 8.);
        let element = ElementId {
            staff,
            part,
            measure: index,
            event: id,
        };
        if event.pitches.len() > 1 {
            engrave_chord(
                &mut result,
                *event,
                element,
                rhythm.x(tick),
                clef,
                metrics,
                beams.direction(element),
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
            rhythm::dots(&mut result, *event, element, metrics);
            tick += event.duration.ticks();
            continue;
        }
        let y = event
            .pitches
            .first()
            .map(|pitch| 40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.)
            .unwrap_or(20.);
        let head_width = if event.duration.value() == score_core::NoteValue::Whole {
            metrics.whole
        } else {
            metrics.head
        };
        // Align notehead centers, including the wider whole-note glyph.
        let x = rhythm.x(tick) + (metrics.head - head_width) / 2.;
        let code = if beams.direction(element).is_some() {
            0xe0a4 // A beamed note has a separate stem and no flag.
        } else if event.pitches.first().is_some() {
            let down = u32::from(y <= 20.);
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
            10.
        } else {
            y
        };
        result.glyphs.push(Glyph {
            text: char::from_u32(code).unwrap().to_string(),
            position: Point { x, y: glyph_y },
            element: Some(element),
        });
        result.events.push(EventPosition {
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
        tick += event.duration.ticks();
    }
    result.cursor_x.push(rhythm.x(tick) - 8.);
    if measure.is_empty() {
        result.glyphs.push(Glyph {
            text: "\u{e4e3}".into(),
            position: Point {
                x: start + (width - start) * 0.4,
                y: 10.,
            },
            element: None,
        });
    }
    beams.engrave(&mut result, metrics);
    for (event, value) in measure.iter() {
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

fn engrave_chord(
    result: &mut MeasureLayout,
    event: score_core::Event,
    id: ElementId,
    onset: f32,
    clef: Clef,
    metrics: &Metrics,
    forced_direction: Option<bool>,
) {
    let y_at =
        |pitch: score_core::Pitch| 40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.;
    let low = y_at(event.pitches.first().expect("chord"));
    let high = y_at(event.pitches.last().expect("chord"));
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
    let mut pitches: Vec<_> = event.pitches.iter().collect();
    if down {
        pitches.reverse();
    }
    let mut previous: Option<i16> = None;
    let mut shifted = false;
    for pitch in pitches {
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
        result.glyphs.push(Glyph {
            text: char::from_u32(head).unwrap().to_string(),
            position: Point { x, y },
            element: Some(id),
        });
        result.events.push(EventPosition {
            id,
            pitch: Some(pitch),
            center: Point {
                x: x + head_width / 2.,
                y,
            },
        });
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
    if forced_direction.is_none() && event.duration.value() != score_core::NoteValue::Whole {
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
