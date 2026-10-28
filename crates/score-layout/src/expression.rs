use super::*;
use score_core::{Connection, ConnectionKind, MarkRef, TextKind};

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
pub(super) const STROKE_REACH: f32 = 5.;

pub(super) fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
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
            .chain(self.piano_arpeggios.iter().filter_map(|mark| {
                mark.bounds
                    .distance(p.x, p.y)
                    .map(|d| (mark.bounds.mark, d))
            }))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(mark, _)| mark);
        if let Some(mark) = point_mark {
            return Some(Hit::Mark(mark));
        }
        // Lines stand clear of the heads, often within a note's reach.
        let line = self
            .line_marks
            .iter()
            .map(|line| (line.connection, line.distance(p)))
            .filter(|(_, distance)| *distance <= STROKE_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((connection, _)) = line {
            return Some(Hit::Mark(MarkRef::Connection(connection)));
        }
        if let Some((id, pitch)) = note {
            return Some(Hit::Note(id, pitch));
        }
        self.curves
            .iter()
            .filter(|curve| curve.connection.start != curve.connection.end)
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
    /// Display staff owning this segment, including projected divisi voices.
    pub staff_origin: f32,
    pub from: Point,
    pub to: Point,
    pub bend: f32,
    pub connection: Connection,
}

/// Compose an arbitrarily tall arpeggio from unchanged Bravura line segments.
/// SMuFL segments run horizontally; the canvas turns the whole run upwards.
pub(super) fn arpeggio(
    result: &mut MeasureLayout,
    event: score_core::Event,
    id: ElementId,
    metrics: &Metrics,
) {
    let Some(mark) = event.notation.arpeggio else {
        return;
    };
    let heads: Vec<_> = result.events.iter().filter(|head| head.id == id).collect();
    let high = heads
        .iter()
        .map(|h| h.center.y)
        .fold(f32::INFINITY, f32::min);
    let low = heads
        .iter()
        .map(|h| h.center.y)
        .fold(f32::NEG_INFINITY, f32::max);
    let head_width = if event.duration.value() == score_core::NoteValue::Whole {
        metrics.whole
    } else {
        metrics.head
    };
    let mut left = heads
        .iter()
        .map(|h| h.center.x - head_width / 2.)
        .fold(f32::INFINITY, f32::min);
    for glyph in result.glyphs.iter().filter(|g| g.element == Some(id)) {
        left = left.min(glyph.position.x + metrics.bounds(&glyph.text, false).0);
    }
    let (glyph, bounds) = arpeggio_glyph(mark, id, high, low, left, metrics);
    result.top = result.top.min(bounds.center.y - bounds.half.1 - 4.);
    result.bottom = result.bottom.max(bounds.center.y + bounds.half.1 + 4.);
    result.marks.push(MarkBox {
        glyph: Some(result.glyphs.len()),
        ..bounds
    });
    result.glyphs.push(glyph);
}

fn arpeggio_glyph(
    mark: score_core::Arpeggio,
    id: ElementId,
    high: f32,
    low: f32,
    left: f32,
    metrics: &Metrics,
) -> (Glyph, MarkBox) {
    let arrow = match mark {
        score_core::Arpeggio::Normal => "",
        score_core::Arpeggio::Up => "\u{eaad}",
        score_core::Arpeggio::Down => "\u{eaae}",
    };
    let arrow_length = if arrow.is_empty() {
        0.
    } else {
        metrics.bounds(arrow, false).2
    };
    let repeats = ((low - high + 12. - arrow_length) / metrics.arpeggio_repeat)
        .ceil()
        .max(1.) as usize;
    let segment = if mark == score_core::Arpeggio::Down {
        "\u{eaaa}"
    } else {
        "\u{eaa9}"
    };
    let text = if mark == score_core::Arpeggio::Down {
        format!("{arrow}{}", segment.repeat(repeats))
    } else {
        format!("{}{arrow}", segment.repeat(repeats))
    };
    let (l, t, r, b) = metrics.bounds(&text, true);
    let position = Point {
        x: left - 7. - r,
        y: (high + low - t - b) / 2.,
    };
    let bounds = MarkBox {
        mark: MarkRef::Arpeggio(id),
        glyph: None,
        center: Point {
            x: position.x + (l + r) / 2.,
            y: position.y + (t + b) / 2.,
        },
        half: ((r - l) / 2. + 3., (b - t) / 2. + 2.),
    };
    (
        Glyph {
            rotated: true,
            text,
            position,
            element: Some(id),
        },
        bounds,
    )
}

/// A single mark in final score coordinates, after staff spacing and pagination.
#[derive(Clone, Debug)]
pub struct PianoArpeggioLayout {
    pub glyph: Glyph,
    pub bounds: MarkBox,
    pub other_hand: ElementId,
}

pub(super) fn piano_arpeggios(
    score: &Score,
    measures: &[PlacedMeasure],
    metrics: &Metrics,
) -> Vec<PianoArpeggioLayout> {
    let lookup: HashMap<_, _> = measures
        .iter()
        .map(|bar| ((bar.part, bar.staff, bar.index), bar))
        .collect();
    score
        .piano_arpeggios()
        .iter()
        .filter_map(|mark| {
            let mut high = f32::INFINITY;
            let mut low = f32::NEG_INFINITY;
            let mut left = f32::INFINITY;
            for id in [mark.upper, mark.lower] {
                let bar = lookup.get(&(id.part, id.staff, id.measure))?;
                for head in bar.content.events.iter().filter(|head| head.id == id) {
                    high = high.min(bar.origin.y + head.center.y);
                    low = low.max(bar.origin.y + head.center.y);
                }
                for glyph in bar.content.glyphs.iter().filter(|g| {
                    g.element == Some(id)
                        && g.text.chars().next().is_some_and(
                            |c| matches!(c,'\u{e0a2}'..='\u{e0a4}'|'\u{e1d2}'..='\u{e1db}'|'\u{e260}'..='\u{e262}'),
                        )
                }) {
                    left = left.min(
                        bar.origin.x + glyph.position.x + metrics.bounds(&glyph.text, false).0,
                    );
                }
            }
            if !high.is_finite() || !left.is_finite() {
                return None;
            }
            let (glyph, bounds) = arpeggio_glyph(mark.kind, mark.upper, high, low, left, metrics);
            Some(PianoArpeggioLayout {
                glyph,
                bounds,
                other_hand: mark.lower,
            })
        })
        .collect()
}

/// Place local marks outside the complete note/chord, including its stem.
pub(super) fn marks(result: &mut MeasureLayout, event: score_core::Event, id: ElementId) {
    let heads: Vec<_> = result.events.iter().filter(|head| head.id == id).collect();
    let low = heads.iter().map(|head| head.center.y).fold(20f32, f32::max);
    let high = heads
        .iter()
        .map(|head| head.center.y)
        .chain(
            result
                .marks
                .iter()
                .filter(|m| m.mark == MarkRef::Harmonic(id))
                .map(|m| m.center.y),
        )
        .fold(20f32, f32::min);
    let x = heads.first().map_or(0., |head| head.center.x);
    let poly = result.events.iter().any(|head| head.voice == Voice::Two);
    let below = poly && heads.first().is_some_and(|head| head.voice == Voice::Two);
    let down = if poly {
        below
    } else {
        result
            .beam_stem(id)
            .map_or((low + high) / 2. <= 20., |s| s.down)
    };
    let staff_top = result.ink_top;
    let staff_bottom = result.ink_bottom;
    let stem = if event.duration.value() != score_core::NoteValue::Whole {
        35.
    } else {
        0.
    };
    let mut top = result
        .beam_stem(id)
        .map_or(high - if down { 0. } else { stem }, |s| high.min(s.to.y));
    let mut bottom = result
        .beam_stem(id)
        .map_or(low + if down { stem } else { 0. }, |s| low.max(s.to.y));
    if event.notation.harmonic == Some(score_core::Harmonic::Natural) {
        let y = if below {
            52f32.max(bottom + 14.)
        } else {
            (-12f32).min(top - 12.)
        };
        result.marks.push(MarkBox {
            mark: MarkRef::Harmonic(id),
            glyph: Some(result.glyphs.len()),
            center: Point { x, y: y - 4. },
            half: (7., 7.),
        });
        result.glyphs.push(Glyph {
            rotated: false,
            text: score_core::Harmonic::Natural.glyph().into(),
            position: Point { x: x - 4., y },
            element: Some(id),
        });
        if below {
            bottom = y + 4.;
            result.bottom = result.bottom.max(y + 10.);
        } else {
            top = y - 10.;
            result.top = result.top.min(y - 14.);
        }
    }
    let articulation_y = if below {
        (50f32).max(bottom + 12.)
    } else {
        (-10f32).min(top - 12.)
    };
    if let Some(mark) = event.notation.articulation {
        let y = articulation_y;
        result.marks.push(MarkBox {
            mark: MarkRef::Articulation(id),
            glyph: Some(result.glyphs.len()),
            // Bravura's above-articulations rise up to one space from the
            // baseline and reach 1.4 spaces right of the drawing point.
            center: Point {
                x: x + 2.,
                y: y + if below { 5. } else { -5. },
            },
            half: (9., 8.),
        });
        result.glyphs.push(Glyph {
            rotated: false,
            text: if below {
                mark.glyph_below()
            } else {
                mark.glyph()
            }
            .into(),
            position: Point { x: x - 4., y },
            element: Some(id),
        });
        if below {
            result.bottom = result.bottom.max(y + 16.);
        } else {
            result.top = result.top.min(y - 16.);
        }
    }
    // The fermata sits outside the articulation, centered over the note.
    let fermata_y = if event.notation.articulation.is_some() {
        articulation_y + if below { 18. } else { -14. }
    } else {
        if below {
            (52f32).max(bottom + 12.)
        } else {
            (-12f32).min(top - 12.)
        }
    };
    if event.notation.fermata {
        let kind = score_core::MarkKind::Fermata;
        result.marks.push(MarkBox {
            mark: MarkRef::Fermata(id),
            glyph: Some(result.glyphs.len()),
            // Bravura's fermataAbove: 2.4 spaces wide, 1.3 high.
            center: Point {
                x,
                y: fermata_y + if below { 6. } else { -6. },
            },
            half: (14., 9.),
        });
        result.glyphs.push(Glyph {
            rotated: false,
            text: if below {
                "\u{e4c1}"
            } else {
                kind.glyph().expect("fermata glyph")
            }
            .into(),
            position: Point {
                x: x - 12.,
                y: fermata_y,
            },
            element: Some(id),
        });
        if below {
            result.bottom = result.bottom.max(fermata_y + 18.);
        } else {
            result.top = result.top.min(fermata_y - 18.);
        }
    }
    if let Some(mark) = event.notation.division {
        let y = (-24f32).min(if poly { staff_top } else { top } - 24.)
            - if event.notation.technique.is_some() {
                22.
            } else {
                0.
            };
        let y = if event.notation.fermata && !below {
            y.min(fermata_y - 24.)
        } else {
            y
        };
        let width = mark.label().chars().count() as f32 * 7.;
        result.marks.push(MarkBox {
            mark: MarkRef::Division(id),
            glyph: None,
            center: Point {
                x: x - 5. + width / 2.,
                y: y - 5.,
            },
            half: (width / 2. + 3., 9.),
        });
        result.techniques.push(TechniqueLabel {
            text: label_words(mark.label()),
            position: Point { x: x - 5., y },
            element: id,
            mark: MarkRef::Division(id),
        });
        result.top = result.top.min(y - 16.);
    }
    if let Some(mark) = event.notation.technique {
        // A separate lane above stems and articulation, also used by slur envelopes.
        let y = (-18f32).min(if poly { staff_top } else { top } - 20.);
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
            text: label_words(mark.label()),
            position: Point { x: x - 5., y },
            element: id,
            mark: MarkRef::Technique(id),
        });
        result.top = result.top.min(y - 16.);
    }
    // Written instructions share the technique lane; the row arranges them.
    if let Some(words) = event.notation.technique_text {
        let y = (-18f32).min(if poly { staff_top } else { top } - 20.)
            - if event.notation.articulation.is_some() {
                20.
            } else {
                0.
            };
        let y = if event.notation.fermata {
            y.min(fermata_y - 22.)
        } else {
            y
        };
        let label = TechniqueLabel {
            text: words,
            position: Point { x: x - 5., y },
            element: id,
            mark: MarkRef::Text(id, TextKind::Technique),
        };
        result.marks.push(MarkBox {
            mark: label.mark,
            glyph: None,
            center: Point {
                x: x - 5. + label.width() / 2.,
                y: y - 5.,
            },
            half: (label.width() / 2. + 3., 9.),
        });
        result.techniques.push(label);
        result.top = result.top.min(y - 16.);
    }
    // A chord symbol starts above the note; the row gives it the outer lane.
    if let Some(chord) = event.notation.chord {
        let y = (-18f32).min(if poly { staff_top } else { top } - 20.);
        let label = TechniqueLabel {
            text: score_core::Words::new(&chord.label()).expect("chord symbols are short"),
            position: Point { x: x - 6., y },
            element: id,
            mark: MarkRef::Chord(id),
        };
        result.marks.push(MarkBox {
            mark: label.mark,
            glyph: None,
            center: Point {
                x: x - 6. + label.width() / 2.,
                y: y - 6.,
            },
            half: (label.width() / 2. + 3., 10.),
        });
        result.techniques.push(label);
        result.top = result.top.min(y - 18.);
    }
    let dynamic_lane = 40f32
        .max(bottom)
        .max(if poly { staff_bottom } else { bottom })
        + 40.;
    if let Some(mark) = event.notation.dynamic {
        // Keep forte ascenders below the entire lower tie lane.
        let y = dynamic_lane;
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
            rotated: false,
            text: mark.glyph().into(),
            position: Point { x: x - 6., y },
            element: Some(id),
        });
        result.bottom = result.bottom.max(y + 12.);
    }
    // An expression follows the dynamic on its baseline: «p dolce».
    if let Some(words) = event.notation.expression_text {
        let after = event
            .notation
            .dynamic
            .map_or(0., |mark| mark.label().chars().count() as f32 * 14. + 6.);
        let label = TechniqueLabel {
            text: words,
            position: Point {
                x: x - 6. + after,
                y: dynamic_lane,
            },
            element: id,
            mark: MarkRef::Text(id, TextKind::Expression),
        };
        result.marks.push(MarkBox {
            mark: label.mark,
            glyph: None,
            center: Point {
                x: label.position.x + label.width() / 2.,
                y: dynamic_lane - 5.,
            },
            half: (label.width() / 2. + 3., 9.),
        });
        result.expressions.push(label);
        result.bottom = result.bottom.max(dynamic_lane + 12.);
    }
    // Syllables wait for the row to put each verse on one line.
    for (verse, syllable) in event.notation.lyrics.verses() {
        let mut label = LyricLabel {
            syllable,
            verse: verse as u8,
            position: Point { x: 0., y: 0. },
            element: id,
        };
        let width = label.width();
        // A held syllable starts at its note; the others centre on it.
        let left = if syllable.join == score_core::Join::Melisma {
            x - 6.
        } else {
            x - width / 2.
        };
        label.position = Point {
            x: left,
            y: lyrics::FIRST + verse as f32 * lyrics::LINE,
        };
        result.marks.push(MarkBox {
            mark: MarkRef::Lyric(id, verse as u8),
            glyph: None,
            center: Point {
                x: left + width / 2.,
                y: label.position.y - 5.,
            },
            half: (width / 2. + 2., 9.),
        });
        result.lyrics.push(label);
    }
}

/// Placed measures indexed once per layout, so each slur, tie and hairpin
/// finds its notes and rows without scanning every bar of the score.
pub(super) struct PlacedIndex<'a> {
    /// The first displayed measure holding each event's heads.
    events: HashMap<ElementId, &'a PlacedMeasure>,
    /// Displayed measures of each part's source staff, in layout order.
    staves: HashMap<(PartId, StaffId), Vec<&'a PlacedMeasure>>,
}
impl<'a> PlacedIndex<'a> {
    pub(super) fn new(measures: &'a [PlacedMeasure]) -> Self {
        let mut events = HashMap::new();
        let mut staves = HashMap::<_, Vec<_>>::new();
        for m in measures {
            for head in &m.content.events {
                events.entry(head.id).or_insert(m);
            }
            staves.entry((m.part, m.source_staff)).or_default().push(m);
        }
        Self { events, staves }
    }
    pub(super) fn measure_of(&self, id: ElementId) -> Option<&'a PlacedMeasure> {
        self.events.get(&id).copied()
    }
}

pub(super) fn curves(score: &Score, index: &PlacedIndex, metrics: &Metrics) -> Vec<Curve> {
    let mut result = Vec::new();
    for c in score.connections().iter().filter(|c| c.kind.is_curve()) {
        let Some(start) = index.measure_of(c.start) else {
            continue;
        };
        let Some(end) = index.measure_of(c.end) else {
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
        let systems = connection_rows(score, *c, index);
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
            let below = if score
                .part(c.start.part)
                .unwrap()
                .staff(c.start.staff)
                .unwrap()
                .measures()[c.start.measure]
                .has_two_voices()
            {
                score.voice_of(c.start) == Some(Voice::Two)
            } else if count > 1 {
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
            if c.kind == ConnectionKind::Tie
                && start.system_start == end.system_start
                && start.staff != end.staff
            {
                let offset = if below { 8. } else { -8. };
                result.push(Curve {
                    staff_origin: start.origin.y,
                    from: Point {
                        x: start.origin.x + a.center.x + 7.,
                        y: start.origin.y + a.center.y + offset,
                    },
                    to: Point {
                        x: end.origin.x + b.center.x - 7.,
                        y: end.origin.y + b.center.y + offset,
                    },
                    bend: if below { 12. } else { -12. },
                    connection: *c,
                });
                continue;
            }
            for (row_index, row) in systems.iter().enumerate() {
                let first = row.measures[0];
                let x1 = if row_index == 0 {
                    start.origin.x + a.center.x + 7.
                } else {
                    row.start_x()
                };
                let x2 = if row_index + 1 == systems.len() {
                    end.origin.x + b.center.x - 7.
                } else {
                    row.end_x(score)
                };
                let (y1, y2, bend) = if c.kind == ConnectionKind::Slur {
                    // Keep the original shallow, level arch. Indication text
                    // has its own lane above it and cannot raise this envelope.
                    let (top, bottom) = row
                        .measures
                        .iter()
                        .map(|bar| {
                            let (top, bottom) = notation_bounds(&bar.content, metrics);
                            (bar.origin.y + top, bar.origin.y + bottom)
                        })
                        .fold((f32::INFINITY, f32::NEG_INFINITY), |a, b| {
                            (a.0.min(b.0), a.1.max(b.1))
                        });
                    if score.voice_of(c.start) == Some(Voice::Two) {
                        (bottom + 8., bottom + 8., 14.)
                    } else {
                        (top - 8., top - 8., -14.)
                    }
                } else {
                    let offset = if below { 8. } else { -8. };
                    let clearance = |bar: &PlacedMeasure, head: &EventPosition| {
                        let tick = score.event_tick(head.id);
                        bar.content
                            .events
                            .iter()
                            .filter(|other| {
                                other.voice != head.voice && score.event_tick(other.id) == tick
                            })
                            .map(|other| other.center.y)
                            .fold(head.center.y, |y, other| {
                                if below { y.max(other) } else { y.min(other) }
                            })
                    };
                    (
                        first.origin.y + clearance(start, a) + offset,
                        first.origin.y + clearance(end, b) + offset,
                        if below { 12. } else { -12. },
                    )
                };
                if x2 > x1 {
                    result.push(Curve {
                        staff_origin: first.origin.y,
                        from: Point { x: x1, y: y1 },
                        to: Point { x: x2, y: y2 },
                        bend: if row.start == row.end {
                            bend * 0.2
                        } else {
                            bend
                        },
                        connection: *c,
                    });
                }
            }
        }
    }
    result
}

/// Musical symbols occupy the inner lane; text and dynamics are separate.
pub(super) fn notation_bounds(content: &MeasureLayout, metrics: &Metrics) -> (f32, f32) {
    let (mut top, mut bottom) = (0f32, 40f32);
    for (index, glyph) in content.glyphs.iter().enumerate() {
        if content
            .marks
            .iter()
            .any(|mark| mark.glyph == Some(index) && matches!(mark.mark, MarkRef::Dynamic(_)))
        {
            continue;
        }
        let (_, a, _, b) = metrics.bounds(&glyph.text, glyph.rotated);
        top = top.min(glyph.position.y + a);
        bottom = bottom.max(glyph.position.y + b);
    }
    for line in &content.lines {
        top = top.min(line.from.y.min(line.to.y) - line.thickness / 2.);
        bottom = bottom.max(line.from.y.max(line.to.y) + line.thickness / 2.);
    }
    for point in content
        .beams
        .iter()
        .flat_map(|g| &g.segments)
        .flat_map(|s| &s.corners)
    {
        top = top.min(point.y);
        bottom = bottom.max(point.y);
    }
    for mark in &content.marks {
        if !matches!(
            mark.mark,
            MarkRef::Dynamic(_)
                | MarkRef::Technique(_)
                | MarkRef::Division(_)
                | MarkRef::Text(..)
                | MarkRef::Chord(_)
                | MarkRef::Lyric(..)
        ) {
            top = top.min(mark.center.y - mark.half.1);
            bottom = bottom.max(mark.center.y + mark.half.1);
        }
    }
    for tuplet in &content.tuplets {
        top = top.min(tuplet.top);
    }
    (top, bottom)
}

/// Arrange text after the complete staff row is known, without changing cached
/// engraving. Inner to outer: notation, slur, technique, division. Text boxes
/// have ten units of clear space; coincident indications use another lane.
pub(super) fn arrange_indications(
    measures: &mut [PlacedMeasure],
    metrics: &Metrics,
    above_slur: bool,
) -> (f32, f32, f32) {
    let bounds: Vec<_> = measures
        .iter()
        .map(|bar| notation_bounds(&bar.content, metrics))
        .collect();
    let top = bounds.iter().map(|b| b.0).fold(0f32, f32::min);
    let bottom = bounds.iter().map(|b| b.1).fold(40f32, f32::max);
    let has = |lane: u8| {
        measures
            .iter()
            .any(|bar| bar.content.techniques.iter().any(|t| t.lane() == lane))
    };
    let technique_y = top - if above_slur { 22. } else { 0. } - 14.;
    let division_y = technique_y - if has(0) { 30. } else { 0. };
    let chord_y = division_y - if has(1) { 30. } else { 0. };
    let mut labels: Vec<_> = measures
        .iter()
        .enumerate()
        .flat_map(|(m, bar)| {
            bar.content
                .techniques
                .iter()
                .enumerate()
                .map(move |(i, label)| (label.lane(), bar.origin.x + label.position.x, m, i))
        })
        .collect();
    labels.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
    let mut occupied: Vec<(f32, f32, f32)> = Vec::new();
    for (lane, x, m, i) in labels {
        let content = Arc::make_mut(&mut measures[m].content);
        let label = &mut content.techniques[i];
        let width = label.width();
        let mut y = [technique_y, division_y, chord_y][usize::from(lane)];
        while let Some(&(_, _, other_y)) = occupied.iter().find(|&&(left, right, other_y)| {
            x < right + 10. && x + width + 10. > left && (y - other_y).abs() < 30.
        }) {
            y = other_y - 30.;
        }
        let shift = y - label.position.y;
        let owner = label.mark;
        label.position.y = y;
        for mark in content.marks.iter_mut().filter(|mark| mark.mark == owner) {
            mark.center.y += shift;
        }
        occupied.push((x, x + width, y));
    }
    for (bar, bounds) in measures.iter_mut().zip(bounds) {
        if bar.content.techniques.is_empty() {
            continue;
        }
        let content = Arc::make_mut(&mut bar.content);
        content.ink_top = content
            .techniques
            .iter()
            .map(|label| label.position.y - 16.)
            .fold(bounds.0, f32::min);
        content.top = content.ink_top.min(-42.);
    }
    (top, bottom, division_y)
}

#[derive(Clone, Debug)]
pub struct Hairpin {
    pub staff_origin: f32,
    pub upper: Line,
    pub lower: Line,
    pub connection: Connection,
}
/// Shared expression baseline, with a floor below every sounding note and
/// lower articulation. Dynamics alone cannot pull the wedge through low notes.
pub(super) fn hairpin_y<'a>(
    measures: impl Iterator<Item = &'a MeasureLayout>,
    below_curve: bool,
) -> f32 {
    let mut floor = 40f32;
    let mut dynamic = f32::NEG_INFINITY;
    for content in measures {
        floor = floor.max(content.music_ink_bottom);
        if below_curve {
            floor = floor.max(content.ink_bottom);
        }
        for mark in &content.marks {
            if matches!(
                mark.mark,
                MarkRef::Dynamic(_) | MarkRef::Text(_, TextKind::Expression)
            ) {
                dynamic = dynamic.max(mark.center.y);
            } else if matches!(mark.mark, MarkRef::Text(_, TextKind::Technique)) {
                // Above the staff: no floor for the lane below.
            } else {
                floor = floor.max(mark.center.y + mark.half.1);
            }
        }
    }
    dynamic.max(floor + 28.)
}

/// The sides of the staff, `(above, below)`, that slurs and ties take in each
/// bar. Indexed once, so rows and hairpins don't scan every connection.
pub(super) fn curve_lanes(score: &Score) -> HashMap<(PartId, StaffId, usize), (bool, bool)> {
    let mut lanes = HashMap::<_, (bool, bool)>::new();
    for c in score.connections().iter().filter(|c| c.kind.is_curve()) {
        let below = score.voice_of(c.start) == Some(Voice::Two);
        for index in c.start.measure..=c.end.measure {
            let lanes = lanes
                .entry((c.start.part, c.start.staff, index))
                .or_default();
            if c.kind == ConnectionKind::Tie {
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
    lanes
}

pub(super) fn hairpins(score: &Score, index: &PlacedIndex, thickness: f32) -> Vec<Hairpin> {
    let mut result = Vec::new();
    let curve_lanes = curve_lanes(score);
    for c in score.connections().iter().filter(|c| c.kind.is_hairpin()) {
        let Some(start) = index.measure_of(c.start) else {
            continue;
        };
        let Some(end) = index.measure_of(c.end) else {
            continue;
        };
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
        let rows = connection_rows(score, *c, index);
        let start_tick = score.event_tick(c.start).unwrap() as f32;
        let end_tick = score.event_tick(c.end).unwrap() as f32;
        let opening = |tick: f32| {
            let progress = ((tick - start_tick) / (end_tick - start_tick)).clamp(0., 1.);
            6. * if c.kind == ConnectionKind::Crescendo {
                progress
            } else {
                1. - progress
            }
        };
        for (i, row) in rows.iter().enumerate() {
            let first = row.measures[0];
            let dynamics: Vec<_> = row
                .measures
                .iter()
                .flat_map(|m| {
                    m.content.marks.iter().filter_map(move |mark| {
                        matches!(
                            mark.mark,
                            MarkRef::Dynamic(_) | MarkRef::Text(_, TextKind::Expression)
                        )
                        .then_some((
                            m.origin.x + mark.center.x - mark.half.0,
                            m.origin.x + mark.center.x + mark.half.0,
                            m.origin.y + mark.center.y,
                        ))
                    })
                })
                .collect();
            let mut x1 = if i == 0 {
                start.origin.x + a + 8.
            } else {
                row.start_x()
            };
            let mut x2 = if i + 1 == rows.len() {
                end.origin.x + b - 8.
            } else {
                row.end_x(score)
            };
            // Dynamics and hairpins share one expression lane. Leave horizontal
            // clearance around the letters rather than stacking another lane.
            for &(left, right, _) in &dynamics {
                if left <= x1 && right >= x1 - 6. {
                    x1 = right + 6.;
                }
                if left <= x2 + 6. && right >= x2 {
                    x2 = left - 6.;
                }
            }
            let below_curve = row.measures.iter().any(|m| {
                curve_lanes
                    .get(&(first.part, first.source_staff, m.index))
                    .is_some_and(|&(_, below)| below)
            });
            let y = first.origin.y
                + hairpin_y(row.measures.iter().map(|m| m.content.as_ref()), below_curve);
            let h1 = opening(if i == 0 { start_tick } else { row.start as f32 });
            let h2 = opening(if i + 1 == rows.len() {
                end_tick
            } else {
                row.end as f32
            });
            if x1 < x2 {
                let mut spans = vec![(x1, x2)];
                for &(left, right, _) in &dynamics {
                    spans = spans
                        .into_iter()
                        .flat_map(|(a, b)| {
                            if right + 6. <= a || left - 6. >= b {
                                vec![(a, b)]
                            } else {
                                [(a, left - 6.), (right + 6., b)]
                                    .into_iter()
                                    .filter(|(a, b)| a < b)
                                    .collect()
                            }
                        })
                        .collect();
                }
                let opening_at = |x: f32| h1 + (h2 - h1) * (x - x1) / (x2 - x1);
                for (left, right) in spans {
                    result.push(Hairpin {
                        staff_origin: first.origin.y,
                        upper: Line {
                            from: Point {
                                x: left,
                                y: y - opening_at(left),
                            },
                            to: Point {
                                x: right,
                                y: y - opening_at(right),
                            },
                            thickness,
                            element: None,
                        },
                        lower: Line {
                            from: Point {
                                x: left,
                                y: y + opening_at(left),
                            },
                            to: Point {
                                x: right,
                                y: y + opening_at(right),
                            },
                            thickness,
                            element: None,
                        },
                        connection: *c,
                    });
                }
            }
        }
    }
    result
}

pub(super) struct ConnectionRow<'a> {
    pub(super) measures: Vec<&'a PlacedMeasure>,
    pub(super) start: u64,
    pub(super) end: u64,
}
impl ConnectionRow<'_> {
    pub(super) fn start_x(&self) -> f32 {
        let first = self.measures[0];
        let local = self.start - first.start_tick;
        // Use this placed bar's actual timing grid. A voice returning to the
        // upper staff within a system does not repeat its clef/signature.
        first.origin.x + first.content.x_at_tick(local as f64) - 4.
    }
    pub(super) fn end_x(&self, score: &Score) -> f32 {
        let last = self.measures[self.measures.len() - 1];
        let local = self.end - last.start_tick;
        last.origin.x
            + if local == u64::from(score.time_at(last.index).capacity()) {
                last.width - 5.
            } else {
                last.content.x_at_tick(local as f64) + 2.
            }
    }
}
pub(super) fn connection_rows<'a>(
    score: &Score,
    c: Connection,
    index: &PlacedIndex<'a>,
) -> Vec<ConnectionRow<'a>> {
    let voice = score.voice_of(c.start).unwrap_or(Voice::One);
    let start = score.event_tick(c.start).unwrap();
    let end = score.event_tick(c.end).unwrap();
    let mut pieces = Vec::new();
    for &m in index
        .staves
        .get(&(c.start.part, c.start.staff))
        .into_iter()
        .flatten()
    {
        for r in m.regions.iter().filter(|r| r.voice == voice) {
            let a = m.start_tick + u64::from(r.start);
            let b = m.start_tick + u64::from(r.end);
            if b > start && a <= end {
                pieces.push((a.max(start), b.min(end), m));
            }
        }
    }
    pieces.sort_by_key(|(a, b, _)| (*a, *b));
    let mut rows: Vec<ConnectionRow<'_>> = Vec::new();
    for (start, end, m) in pieces {
        if let Some(row) = rows
            .last_mut()
            .filter(|row| row.measures[0].origin.y == m.origin.y && row.end == start)
        {
            row.measures.push(m);
            row.end = end;
        } else {
            rows.push(ConnectionRow {
                measures: vec![m],
                start,
                end,
            });
        }
    }
    rows
}
