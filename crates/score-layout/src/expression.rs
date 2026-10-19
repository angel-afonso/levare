use super::*;
use score_core::{Connection, ConnectionKind, MarkRef};

/// Where a point mark can be clicked, in measure coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MarkBox {
    pub mark: MarkRef,
    /// The mark's glyph in `MeasureLayout::glyphs`; techniques are labels.
    pub glyph: Option<usize>,
    pub center: Point,
    pub half: (f32, f32),
}
impl MarkBox {
    fn distance(&self, x: f32, y: f32) -> Option<f32> {
        let dx = (x - self.center.x).abs();
        let dy = (y - self.center.y).abs();
        (dx <= self.half.0 && dy <= self.half.1).then_some(dx * dx + dy * dy)
    }
}

/// What a click on the score lands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Note(ElementId, Option<score_core::Pitch>),
    Mark(MarkRef),
}

/// Pointer tolerance around curves and hairpin strokes, in layout units.
const STROKE_REACH: f32 = 5.;

fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length == 0. {
        0.
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0., 1.)
    };
    let (x, y) = (a.x + t * dx - p.x, a.y + t * dy - p.y);
    (x * x + y * y).sqrt()
}

impl Curve {
    /// The same cubic the canvas strokes, sampled as a polyline.
    pub fn distance(&self, p: Point) -> f32 {
        let span = self.to.x - self.from.x;
        let controls = [
            self.from,
            Point {
                x: self.from.x + span / 3.,
                y: self.from.y + self.bend,
            },
            Point {
                x: self.to.x - span / 3.,
                y: self.to.y + self.bend,
            },
            self.to,
        ];
        let at = |t: f32| {
            let u = 1. - t;
            let w = [u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t];
            Point {
                x: controls.iter().zip(w).map(|(c, w)| c.x * w).sum(),
                y: controls.iter().zip(w).map(|(c, w)| c.y * w).sum(),
            }
        };
        (0..16)
            .map(|i| segment_distance(p, at(i as f32 / 16.), at((i + 1) as f32 / 16.)))
            .fold(f32::INFINITY, f32::min)
    }
}

impl Hairpin {
    /// Zero inside the wedge, else the distance to its nearer stroke.
    pub fn distance(&self, p: Point) -> f32 {
        let (left, right) = (self.upper.from.x, self.upper.to.x);
        if (left..=right).contains(&p.x) {
            let t = (p.x - left) / (right - left).max(1.);
            let top = self.upper.from.y + (self.upper.to.y - self.upper.from.y) * t;
            let bottom = self.lower.from.y + (self.lower.to.y - self.lower.from.y) * t;
            if (top..=bottom).contains(&p.y) {
                return 0.;
            }
        }
        segment_distance(p, self.upper.from, self.upper.to).min(segment_distance(
            p,
            self.lower.from,
            self.lower.to,
        ))
    }
}

impl Layout {
    /// Notes win over the marks around them only on the notehead itself, so
    /// an articulation just above a note is still easy to pick.
    pub fn hit(&self, point: Point) -> Option<Hit> {
        let p = Point {
            x: point.x / self.scale,
            y: point.y / self.scale,
        };
        let note = self.hit_pitch(point);
        let on_head = note.is_some_and(|(id, pitch)| {
            self.measures.iter().any(|m| {
                m.content.events.iter().any(|e| {
                    e.id == id
                        && e.pitch == pitch
                        && (p.x - m.origin.x - e.center.x).abs() <= 7.
                        && (p.y - m.origin.y - e.center.y).abs() <= 5.
                })
            })
        });
        if on_head {
            return note.map(|(id, pitch)| Hit::Note(id, pitch));
        }
        let point_mark = self
            .measures
            .iter()
            .flat_map(|m| {
                m.content.marks.iter().filter_map(move |mark| {
                    mark.distance(p.x - m.origin.x, p.y - m.origin.y)
                        .map(|d| (mark.mark, d))
                })
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(mark, _)| mark);
        if let Some(mark) = point_mark {
            return Some(Hit::Mark(mark));
        }
        if let Some((id, pitch)) = note {
            return Some(Hit::Note(id, pitch));
        }
        self.curves
            .iter()
            .map(|curve| (curve.connection, curve.distance(p)))
            .chain(
                self.hairpins
                    .iter()
                    .map(|hairpin| (hairpin.connection, hairpin.distance(p))),
            )
            .filter(|(_, distance)| *distance <= STROKE_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(connection, _)| Hit::Mark(MarkRef::Connection(connection)))
    }
}

#[derive(Clone, Debug)]
pub struct Curve {
    pub from: Point,
    pub to: Point,
    pub bend: f32,
    pub connection: Connection,
}

/// Place local marks outside the complete note/chord, including its stem.
pub(super) fn marks(result: &mut MeasureLayout, event: score_core::Event, id: ElementId) {
    let heads: Vec<_> = result.events.iter().filter(|head| head.id == id).collect();
    let low = heads.iter().map(|head| head.center.y).fold(20f32, f32::max);
    let high = heads.iter().map(|head| head.center.y).fold(20f32, f32::min);
    let x = heads.first().map_or(0., |head| head.center.x);
    let down = result
        .beam_stem(id)
        .map_or((low + high) / 2. <= 20., |s| s.down);
    let stem = if event.duration.value() != score_core::NoteValue::Whole {
        35.
    } else {
        0.
    };
    let top = result
        .beam_stem(id)
        .map_or(high - if down { 0. } else { stem }, |s| high.min(s.to.y));
    let bottom = result
        .beam_stem(id)
        .map_or(low + if down { stem } else { 0. }, |s| low.max(s.to.y));
    let articulation_y = (-10f32).min(top - 12.);
    if let Some(mark) = event.notation.articulation {
        let y = articulation_y;
        result.marks.push(MarkBox {
            mark: MarkRef::Articulation(id),
            glyph: Some(result.glyphs.len()),
            // Bravura's above-articulations rise up to one space from the
            // baseline and reach 1.4 spaces right of the drawing point.
            center: Point {
                x: x + 2.,
                y: y - 5.,
            },
            half: (9., 8.),
        });
        result.glyphs.push(Glyph {
            text: mark.glyph().into(),
            position: Point { x: x - 4., y },
            element: Some(id),
        });
        result.top = result.top.min(y - 16.);
    }
    // The fermata sits outside the articulation, centered over the note.
    let fermata_y = if event.notation.articulation.is_some() {
        articulation_y - 14.
    } else {
        (-12f32).min(top - 12.)
    };
    if event.notation.fermata {
        let kind = score_core::MarkKind::Fermata;
        result.marks.push(MarkBox {
            mark: MarkRef::Fermata(id),
            glyph: Some(result.glyphs.len()),
            // Bravura's fermataAbove: 2.4 spaces wide, 1.3 high.
            center: Point {
                x,
                y: fermata_y - 6.,
            },
            half: (14., 9.),
        });
        result.glyphs.push(Glyph {
            text: kind.glyph().expect("fermata glyph").into(),
            position: Point {
                x: x - 12.,
                y: fermata_y,
            },
            element: Some(id),
        });
        result.top = result.top.min(fermata_y - 18.);
    }
    if let Some(mark) = event.notation.technique {
        // A separate lane above stems and articulation, also used by slur envelopes.
        let y = (-18f32).min(top - 20.);
        let y = if event.notation.articulation.is_some() {
            y - 20.
        } else {
            y
        };
        let y = if event.notation.fermata {
            y.min(fermata_y - 22.)
        } else {
            y
        };
        let width = mark.label().chars().count() as f32 * 7.;
        result.marks.push(MarkBox {
            mark: MarkRef::Technique(id),
            glyph: None,
            center: Point {
                x: x - 5. + width / 2.,
                y: y - 5.,
            },
            half: (width / 2. + 3., 9.),
        });
        result.techniques.push(TechniqueLabel {
            text: mark.label(),
            position: Point { x: x - 5., y },
            element: id,
        });
        result.top = result.top.min(y - 16.);
    }
    if let Some(mark) = event.notation.dynamic {
        // Keep forte ascenders below the entire lower tie lane.
        let y = 40f32.max(bottom) + 40.;
        // Bravura's dynamic letters are up to 1.8 spaces wide; f rises 1.8
        // spaces and p descends 0.6 below the baseline.
        let width = mark.label().chars().count() as f32 * 14.;
        result.marks.push(MarkBox {
            mark: MarkRef::Dynamic(id),
            glyph: Some(result.glyphs.len()),
            center: Point {
                x: x - 6. + width / 2.,
                y: y - 5.,
            },
            half: (width / 2. + 3., 11.),
        });
        result.glyphs.push(Glyph {
            text: mark.glyph().into(),
            position: Point { x: x - 6., y },
            element: Some(id),
        });
        result.bottom = result.bottom.max(y + 12.);
    }
}

pub(super) fn curves(score: &Score, measures: &[PlacedMeasure]) -> Vec<Curve> {
    let mut result = Vec::new();
    let mut staves: HashMap<(PartId, StaffId), Vec<&PlacedMeasure>> = HashMap::new();
    for measure in measures {
        staves
            .entry((measure.part, measure.staff))
            .or_default()
            .push(measure);
    }
    for c in score.connections().iter().filter(|c| !c.kind.is_hairpin()) {
        let Some(staff) = staves.get(&(c.start.part, c.start.staff)) else {
            continue;
        };
        let (Some(start), Some(end)) = (staff.get(c.start.measure), staff.get(c.end.measure))
        else {
            continue;
        };
        let start_heads: Vec<_> = start
            .content
            .events
            .iter()
            .filter(|e| e.id == c.start)
            .collect();
        let end_heads: Vec<_> = end
            .content
            .events
            .iter()
            .filter(|e| e.id == c.end)
            .collect();
        let mut systems: Vec<Vec<&PlacedMeasure>> = Vec::new();
        for &m in &staff[c.start.measure..=c.end.measure] {
            if systems
                .last()
                .is_none_or(|row| row[0].origin.y != m.origin.y)
            {
                systems.push(Vec::new())
            }
            systems.last_mut().unwrap().push(m);
        }
        let count = if c.kind == ConnectionKind::Tie {
            start_heads.len()
        } else {
            1
        };
        for tone in 0..count {
            let a = start_heads[tone];
            let b = if c.kind == ConnectionKind::Tie {
                end_heads
                    .iter()
                    .find(|head| head.pitch == a.pitch)
                    .copied()
                    .unwrap_or(end_heads[tone])
            } else {
                end_heads[0]
            };
            // Opposite the stem for single notes, outward for chord tones.
            let below = if count > 1 {
                a.center.y
                    >= (start_heads
                        .iter()
                        .map(|h| h.center.y)
                        .fold(f32::INFINITY, f32::min)
                        + start_heads
                            .iter()
                            .map(|h| h.center.y)
                            .fold(f32::NEG_INFINITY, f32::max))
                        / 2.
            } else {
                start
                    .content
                    .beam_stem(c.start)
                    .map_or(a.center.y > 20., |stem| !stem.down)
            };
            for (row_index, row) in systems.iter().enumerate() {
                let first = row[0];
                let last = row[row.len() - 1];
                let x1 = if row_index == 0 {
                    start.origin.x + a.center.x + 7.
                } else {
                    first.origin.x + music_prefix(true, score.key_signature()) - 12.
                };
                let x2 = if row_index + 1 == systems.len() {
                    end.origin.x + b.center.x - 7.
                } else {
                    last.origin.x + last.width - 5.
                };
                let (y1, y2, bend) = if c.kind == ConnectionKind::Slur {
                    // Follow the envelope of every glyph/stem on this system.
                    let top = row
                        .iter()
                        .map(|m| m.origin.y + m.content.ink_top)
                        .fold(f32::INFINITY, f32::min)
                        - 8.;
                    (top, top, -14.)
                } else {
                    let offset = if below { 8. } else { -8. };
                    (
                        first.origin.y + a.center.y + offset,
                        first.origin.y + b.center.y + offset,
                        if below { 12. } else { -12. },
                    )
                };
                if x2 > x1 {
                    result.push(Curve {
                        from: Point { x: x1, y: y1 },
                        to: Point { x: x2, y: y2 },
                        bend,
                        connection: *c,
                    });
                }
            }
        }
    }
    result
}

#[derive(Clone, Debug)]
pub struct Hairpin {
    pub staff_origin: f32,
    pub upper: Line,
    pub lower: Line,
    pub connection: Connection,
}
pub(super) fn hairpins(score: &Score, measures: &[PlacedMeasure]) -> Vec<Hairpin> {
    let mut result = Vec::new();
    for c in score.connections().iter().filter(|c| c.kind.is_hairpin()) {
        let staff: Vec<_> = measures
            .iter()
            .filter(|m| m.part == c.start.part && m.staff == c.start.staff)
            .collect();
        let start = staff[c.start.measure];
        let end = staff[c.end.measure];
        let a = start
            .content
            .events
            .iter()
            .find(|e| e.id == c.start)
            .unwrap()
            .center
            .x;
        let b = end
            .content
            .events
            .iter()
            .find(|e| e.id == c.end)
            .unwrap()
            .center
            .x;
        let mut rows: Vec<Vec<&PlacedMeasure>> = Vec::new();
        for &m in &staff[c.start.measure..=c.end.measure] {
            if rows.last().is_none_or(|r| r[0].origin.y != m.origin.y) {
                rows.push(Vec::new());
            }
            rows.last_mut().unwrap().push(m);
        }
        let start_tick = score.event_tick(c.start).unwrap() as f32;
        let end_tick = score.event_tick(c.end).unwrap() as f32;
        let capacity = f32::from(score.time().capacity());
        let opening = |tick: f32| {
            let progress = ((tick - start_tick) / (end_tick - start_tick)).clamp(0., 1.);
            6. * if c.kind == ConnectionKind::Crescendo {
                progress
            } else {
                1. - progress
            }
        };
        for (i, row) in rows.iter().enumerate() {
            let first = row[0];
            let last = row[row.len() - 1];
            let x1 = if i == 0 {
                start.origin.x + a + 8.
            } else {
                first.origin.x + music_prefix(true, score.key_signature()) - 12.
            };
            let x2 = if i + 1 == rows.len() {
                end.origin.x + b - 8.
            } else {
                last.origin.x + last.width - 5.
            };
            // Shared lane below every local mark and lower tie on this staff row.
            let y = staff
                .iter()
                .filter(|m| m.origin.y == first.origin.y)
                .map(|m| m.origin.y + m.content.ink_bottom)
                .fold(f32::NEG_INFINITY, f32::max)
                + 28.;
            let h1 = opening(if i == 0 {
                start_tick
            } else {
                first.index as f32 * capacity
            });
            let h2 = opening(if i + 1 == rows.len() {
                end_tick
            } else {
                (last.index + 1) as f32 * capacity
            });
            if x1 < x2 {
                result.push(Hairpin {
                    staff_origin: first.origin.y,
                    upper: Line {
                        from: Point { x: x1, y: y - h1 },
                        to: Point { x: x2, y: y - h2 },
                        thickness: 1.2,
                        element: None,
                    },
                    lower: Line {
                        from: Point { x: x1, y: y + h1 },
                        to: Point { x: x2, y: y + h2 },
                        thickness: 1.2,
                        element: None,
                    },
                    connection: *c,
                });
            }
        }
    }
    result
}
