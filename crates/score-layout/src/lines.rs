//! Lines over a passage: octave lines, pedal, trills and glissandi. They are
//! cut at system breaks like hairpins, and each one stands outside the notes,
//! slurs, dynamics and inner lines it would otherwise cross.
use super::*;
use expression::{ConnectionRow, PlacedIndex, connection_rows, segment_distance};
use score_core::{Connection, ConnectionKind as K};

const DASH: f32 = 6.;
const GAP: f32 = 4.;
const HOOK: f32 = 10.;
/// Clear space between a line and the ink it stands over.
const CLEARANCE: f32 = 4.;
/// Room one lane of lines takes outside the staff ink.
pub(super) const LANE: f32 = 26.;

/// One system's piece of a line.
#[derive(Clone, Debug)]
pub struct LineMark {
    pub staff_origin: f32,
    /// The line from its first to its last note within this system.
    pub from: Point,
    pub to: Point,
    pub strokes: Vec<Line>,
    pub glyphs: Vec<Glyph>,
    /// Ink of each glyph: left, top, right and bottom.
    pub areas: Vec<(f32, f32, f32, f32)>,
    pub connection: Connection,
}

impl LineMark {
    /// Zero over a label or the wiggle, else the distance to a stroke.
    pub fn distance(&self, p: Point) -> f32 {
        if self
            .areas
            .iter()
            .any(|&(l, t, r, b)| (l..=r).contains(&p.x) && (t..=b).contains(&p.y))
        {
            return 0.;
        }
        self.strokes
            .iter()
            .map(|line| segment_distance(p, line.from, line.to))
            .fold(f32::INFINITY, f32::min)
    }

    /// Left, top, right and bottom of everything painted.
    pub fn ink(&self) -> (f32, f32, f32, f32) {
        let strokes = self.strokes.iter().map(|line| {
            let half = line.thickness / 2.;
            (
                line.from.x.min(line.to.x) - half,
                line.from.y.min(line.to.y) - half,
                line.from.x.max(line.to.x) + half,
                line.from.y.max(line.to.y) + half,
            )
        });
        strokes.chain(self.areas.iter().copied()).fold(
            (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
        )
    }
}

/// Above or below the staff; glissandi run between their notes.
pub(super) const fn above(kind: K) -> Option<bool> {
    match kind {
        K::Trill | K::OttavaAlta => Some(true),
        K::OttavaBassa | K::Pedal => Some(false),
        _ => None,
    }
}

fn dashes(strokes: &mut Vec<Line>, from: f32, to: f32, y: f32, thickness: f32) {
    let mut x = from;
    while x < to {
        let end = (x + DASH).min(to);
        strokes.push(Line {
            from: Point { x, y },
            to: Point { x: end, y },
            thickness,
            element: None,
        });
        x = end + GAP;
    }
}

fn stroke(from: Point, to: Point, thickness: f32) -> Line {
    Line {
        from,
        to,
        thickness,
        element: None,
    }
}

/// The first written line ends at the end of its last note, not its onset.
fn row_end(row: &ConnectionRow, score: &Score) -> f32 {
    let last = row.measures[row.measures.len() - 1];
    let local = row.end - last.start_tick;
    if local >= u64::from(score.time_at(last.index).capacity()) {
        last.origin.x + last.width - 5.
    } else {
        last.origin.x + last.content.x_at_tick(local as f64) - 6.
    }
}

/// Ink of this staff row between `x1` and `x2`: notes, text, slurs, hairpins
/// and the lines already placed nearer the staff.
fn clearance(
    row: &ConnectionRow,
    (x1, x2): (f32, f32),
    curves: &[Curve],
    hairpins: &[Hairpin],
    placed: &[LineMark],
) -> (f32, f32) {
    let first = row.measures[0];
    let (mut top, mut bottom) =
        row.measures
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(top, bottom), m| {
                (
                    top.min(m.origin.y + m.content.ink_top),
                    bottom.max(m.origin.y + m.content.ink_bottom),
                )
            });
    let here = |origin: f32, part: PartId, left: f32, right: f32| {
        origin == first.origin.y && part == first.part && left < x2 && right > x1
    };
    for curve in curves
        .iter()
        .filter(|c| here(c.staff_origin, c.connection.start.part, c.from.x, c.to.x))
    {
        top = top.min(curve.from.y.min(curve.to.y) + curve.bend.min(0.) * 0.75);
        bottom = bottom.max(curve.from.y.max(curve.to.y) + curve.bend.max(0.) * 0.75);
    }
    for hairpin in hairpins.iter().filter(|h| {
        here(
            h.staff_origin,
            h.connection.start.part,
            h.upper.from.x,
            h.upper.to.x,
        )
    }) {
        top = top.min(hairpin.upper.from.y.min(hairpin.upper.to.y) - 2.);
        bottom = bottom.max(hairpin.lower.from.y.max(hairpin.lower.to.y) + 2.);
    }
    for line in placed {
        let (left, line_top, right, line_bottom) = line.ink();
        if here(line.staff_origin, line.connection.start.part, left, right) {
            top = top.min(line_top);
            bottom = bottom.max(line_bottom);
        }
    }
    (top, bottom)
}

pub(super) fn lines(
    score: &Score,
    index: &PlacedIndex,
    curves: &[Curve],
    hairpins: &[Hairpin],
    metrics: &Metrics,
) -> Vec<LineMark> {
    let mut result: Vec<LineMark> = Vec::new();
    // Inner lanes first: a trill sits under an 8va, an 8vb over the pedal.
    for kind in [
        K::Trill,
        K::OttavaAlta,
        K::OttavaBassa,
        K::Pedal,
        K::Glissando,
    ] {
        for c in score.connections().iter().filter(|c| c.kind == kind) {
            let (Some(start), Some(end)) = (index.measure_of(c.start), index.measure_of(c.end))
            else {
                continue;
            };
            let heads = |m: &PlacedMeasure, id: ElementId| {
                m.content
                    .events
                    .iter()
                    .filter(|e| e.id == id)
                    .min_by(|a, b| a.center.y.total_cmp(&b.center.y))
                    .cloned()
            };
            let (Some(a), Some(b)) = (heads(start, c.start), heads(end, c.end)) else {
                continue;
            };
            let mut rows = connection_rows(score, *c, index);
            if kind == K::Glissando {
                result.extend(glissando(score, *c, &rows, (start, a), (end, b), metrics));
                continue;
            }
            let Some(last) = rows.last_mut() else {
                continue;
            };
            last.end = score.event_tick(c.end).unwrap()
                + u64::from(score.event(c.end).unwrap().duration.ticks());
            let count = rows.len();
            for (i, row) in rows.iter().enumerate() {
                let x1 = if i == 0 {
                    start.origin.x + a.center.x - 7.
                } else {
                    row.start_x()
                };
                let x2 = if i + 1 == count {
                    row_end(row, score)
                } else {
                    row.end_x(score)
                }
                .max(x1 + 12.);
                let (top, bottom) = clearance(row, (x1, x2), curves, hairpins, &result);
                let mark = |strokes, glyphs, areas, y| LineMark {
                    staff_origin: row.measures[0].origin.y,
                    from: Point { x: x1, y },
                    to: Point { x: x2, y },
                    strokes,
                    glyphs,
                    areas,
                    connection: *c,
                };
                let label = |text: &str, x: f32, y: f32| {
                    let (l, t, r, b) = metrics.bounds(text, false);
                    (
                        Glyph {
                            rotated: false,
                            text: text.into(),
                            position: Point { x, y },
                            element: None,
                        },
                        (x + l, y + t, x + r, y + b),
                    )
                };
                let base = |text: &str| {
                    let (_, t, _, b) = metrics.bounds(text, false);
                    if above(kind) == Some(true) {
                        top - CLEARANCE - b
                    } else {
                        bottom + CLEARANCE - t
                    }
                };
                let last_row = i + 1 == count;
                result.push(match kind {
                    K::OttavaAlta | K::OttavaBassa => {
                        let text = if kind == K::OttavaAlta {
                            "\u{e511}"
                        } else {
                            "\u{e51c}"
                        };
                        let y = base(text);
                        let (glyph, area) = label(text, x1, y);
                        let middle = (area.1 + area.3) / 2.;
                        let mut strokes = Vec::new();
                        dashes(&mut strokes, area.2 + 3., x2, middle, metrics.octave);
                        if last_row {
                            let hook = if kind == K::OttavaAlta { HOOK } else { -HOOK };
                            strokes.push(stroke(
                                Point { x: x2, y: middle },
                                Point {
                                    x: x2,
                                    y: middle + hook,
                                },
                                metrics.octave,
                            ));
                        }
                        mark(strokes, vec![glyph], vec![area], middle)
                    }
                    K::Trill => {
                        let y = base("\u{e566}");
                        let (sign, area) = label("\u{e566}", x1, y);
                        let middle = (area.1 + area.3) / 2.;
                        let wiggle = if i == 0 { area.2 + 2. } else { x1 };
                        let mut glyphs = Vec::new();
                        let mut areas = Vec::new();
                        if i == 0 {
                            glyphs.push(sign);
                            areas.push(area);
                        }
                        let repeats = ((x2 - wiggle) / metrics.advance('\u{eaa4}')) as usize;
                        // A trill on one note shorter than a half is the sign alone.
                        let alone = c.start == c.end
                            && score.event(c.start).unwrap().duration.ticks()
                                < score_core::TICKS_PER_WHOLE / 2;
                        if !alone && (repeats >= 1 || i > 0) {
                            let (glyph, area) =
                                label(&"\u{eaa4}".repeat(repeats.max(1)), wiggle, y - 1.5);
                            glyphs.push(glyph);
                            areas.push(area);
                        }
                        mark(Vec::new(), glyphs, areas, middle)
                    }
                    _ => {
                        let y = base("\u{e650}");
                        let mut glyphs = Vec::new();
                        let mut areas = Vec::new();
                        let mut left = x1;
                        if i == 0 {
                            let (glyph, area) = label("\u{e650}", x1, y);
                            left = area.2 + 3.;
                            glyphs.push(glyph);
                            areas.push(area);
                        }
                        let mut strokes = Vec::new();
                        if left < x2 {
                            strokes.push(stroke(
                                Point { x: left, y },
                                Point { x: x2, y },
                                metrics.pedal,
                            ));
                        }
                        if last_row {
                            strokes.push(stroke(
                                Point { x: x2, y },
                                Point { x: x2, y: y - HOOK },
                                metrics.pedal,
                            ));
                        }
                        mark(strokes, glyphs, areas, y)
                    }
                });
            }
        }
    }
    result
}

/// A straight slide from the top head of one note to the top head of the next.
fn glissando(
    score: &Score,
    c: Connection,
    rows: &[ConnectionRow],
    (start, a): (&PlacedMeasure, EventPosition),
    (end, b): (&PlacedMeasure, EventPosition),
    metrics: &Metrics,
) -> Vec<LineMark> {
    let from = Point {
        x: start.origin.x + a.center.x + 8.,
        y: start.origin.y + a.center.y,
    };
    let to = Point {
        x: end.origin.x + b.center.x - 9.,
        y: end.origin.y + b.center.y,
    };
    let rise = b.center.y - a.center.y;
    let count = rows.len();
    rows.iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let origin = row.measures[0].origin.y;
            let left = if i == 0 {
                from
            } else {
                Point {
                    x: row.start_x(),
                    y: origin + b.center.y - rise / 2.,
                }
            };
            let right = if i + 1 == count {
                to
            } else {
                Point {
                    x: row.end_x(score),
                    y: origin + a.center.y + rise / 2.,
                }
            };
            (right.x > left.x).then(|| LineMark {
                staff_origin: origin,
                from: left,
                to: right,
                strokes: vec![stroke(left, right, metrics.staff * 1.2)],
                glyphs: Vec::new(),
                areas: Vec::new(),
                connection: c,
            })
        })
        .collect()
}
