//! Grace notes at cue size before their note: stems up, a flag (slashed for
//! an acciaccatura) on a lone note, two beams over a group.
use super::*;
use score_core::Event;

/// Grace notes are engraved at two thirds of the normal size.
pub const CUE: f32 = 0.66;
const STEP: f32 = 13.;
const ACCIDENTAL: f32 = 9.;
const STEM: f32 = 25.;

/// A glyph drawn at cue size, with the grace note it belongs to.
#[derive(Clone, Debug)]
pub struct CueGlyph {
    pub glyph: Glyph,
    pub mark: MarkRef,
}

/// Room the grace notes of `event` take before its accidentals.
pub(super) fn width(event: &Event) -> f32 {
    event.notation.grace.map_or(0., |grace| {
        grace
            .notes()
            .map(|pitch| {
                STEP + if pitch.accidental().is_some() {
                    ACCIDENTAL
                } else {
                    0.
                }
            })
            .sum::<f32>()
            + 4.
    })
}

/// Engrave the grace notes of `event`, ending at `right`.
pub(super) fn engrave(
    result: &mut MeasureLayout,
    event: &Event,
    element: ElementId,
    right: f32,
    clef: Clef,
    metrics: &Metrics,
) {
    let Some(grace) = event.notation.grace else {
        return;
    };
    let head = metrics.head * CUE;
    let mut x = right - width(event);
    let mut heads = Vec::new();
    let cue = |result: &mut MeasureLayout, code: u32, x: f32, y: f32, index: usize| {
        result.cue.push(CueGlyph {
            glyph: Glyph {
                rotated: false,
                text: char::from_u32(code).expect("SMuFL glyph").to_string(),
                position: Point { x, y },
                element: Some(element),
            },
            mark: MarkRef::Grace(element, index as u8),
        });
    };
    for (index, pitch) in grace.notes().enumerate() {
        let y = if clef == Clef::Percussion {
            20.
        } else {
            40. - f32::from(pitch.diatonic() - clef.bottom_diatonic()) * 5.
        };
        if let Some(accidental) = pitch.accidental() {
            let code = match accidental.semitones() {
                -2 => 0xe264,
                -1 => 0xe260,
                1 => 0xe262,
                2 => 0xe263,
                _ => 0xe261,
            };
            cue(result, code, x, y, index);
            x += ACCIDENTAL;
        }
        cue(result, 0xe0a4, x, y, index);
        let mut ledger = |y: f32| {
            result.lines.push(Line {
                from: Point { x: x - 2., y },
                to: Point {
                    x: x + head + 2.,
                    y,
                },
                thickness: metrics.ledger * CUE,
                element: None,
            });
        };
        let mut above = -10.;
        while above >= y {
            ledger(above);
            above -= 10.;
        }
        let mut below = 50.;
        while below <= y {
            ledger(below);
            below += 10.;
        }
        result.marks.push(MarkBox {
            mark: MarkRef::Grace(element, index as u8),
            glyph: None,
            center: Point {
                x: x + head / 2.,
                y,
            },
            half: (head / 2. + 2., 5.),
        });
        heads.push((x, y));
        x += STEP;
    }
    let stem_x = |x: f32| x + head - metrics.stem * CUE / 2.;
    let stroke = |result: &mut MeasureLayout, from: Point, to: Point, thickness: f32| {
        result.lines.push(Line {
            from,
            to,
            thickness,
            element: Some(element),
        });
    };
    let top = if let [(x, y)] = heads[..] {
        let top = y - STEM;
        stroke(
            result,
            Point { x: stem_x(x), y },
            Point {
                x: stem_x(x),
                y: top,
            },
            metrics.stem * CUE,
        );
        cue(result, 0xe240, stem_x(x), top, 0);
        top
    } else {
        let beam = heads.iter().map(|(_, y)| *y).fold(f32::INFINITY, f32::min) - STEM;
        for (x, y) in &heads {
            stroke(
                result,
                Point {
                    x: stem_x(*x),
                    y: *y,
                },
                Point {
                    x: stem_x(*x),
                    y: beam,
                },
                metrics.stem * CUE,
            );
        }
        let (first, last) = (heads[0].0, heads[heads.len() - 1].0);
        for level in 0..2 {
            let y = beam + 1.6 + level as f32 * 5.;
            stroke(
                result,
                Point {
                    x: stem_x(first) - metrics.stem * CUE / 2.,
                    y,
                },
                Point {
                    x: stem_x(last) + metrics.stem * CUE / 2.,
                    y,
                },
                3.3,
            );
        }
        beam
    };
    if grace.kind == score_core::GraceKind::Acciaccatura {
        let x = stem_x(heads[0].0);
        stroke(
            result,
            Point {
                x: x - 5.,
                y: top + 14.,
            },
            Point {
                x: x + 7.,
                y: top + 5.,
            },
            metrics.stem * 1.2,
        );
    }
    result.top = result.top.min(top - 4.);
    result.ink_top = result.ink_top.min(top - 2.);
}
