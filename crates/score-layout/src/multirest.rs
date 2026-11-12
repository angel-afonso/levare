//! Multi-measure rests in parts (`Score::multirests` decides the runs).
//!
//! Every bar of a run stays placed, so bar indices, the caret, the playhead
//! and hit testing keep working: the first bar draws the H-bar and the count
//! across the room of the whole run, and the bars after it lie on top of it
//! with the same place and width but draw nothing.
use super::*;

/// Room the H-bar takes after the bar's clef, key and time signature.
pub(crate) const WIDTH: f32 = 180.;
/// Baseline of the count, centred one and a half spaces above the staff.
const COUNT_Y: f32 = -15.;

/// The first bar of a run of `bars` bars: its rests give way to the H-bar,
/// and the upper staff (`counted`) shows how many bars it stands for.
pub(crate) fn engrave(
    content: &MeasureLayout,
    bars: usize,
    counted: bool,
    metrics: &Metrics,
) -> Arc<MeasureLayout> {
    let mut result = content.clone();
    // An empty bar's own ink is its rests (with their dots): every glyph
    // tied to an event, and the whole rest of a bar with no events at all.
    result
        .glyphs
        .retain(|glyph| glyph.element.is_none() && glyph.text != "\u{e4e3}");
    result.lines.retain(|line| line.element.is_none());
    result.events.clear();
    result.marks.clear();
    result.tuplets.clear();
    result.beams.clear();
    result.cue.clear();
    let left = content.time_positions.first().map_or(0., |(_, x)| x + 8.);
    let right = content
        .time_positions
        .last()
        .map_or(left + WIDTH, |(_, x)| *x)
        .max(left + 2. * STAFF_SPACE);
    let middle = 2. * STAFF_SPACE;
    result.lines.push(Line {
        from: Point { x: left, y: middle },
        to: Point {
            x: right,
            y: middle,
        },
        thickness: STAFF_SPACE,
        element: None,
    });
    for x in [left, right] {
        result.lines.push(Line {
            from: Point {
                x,
                y: middle - STAFF_SPACE,
            },
            to: Point {
                x,
                y: middle + STAFF_SPACE,
            },
            thickness: metrics.bar * 1.5,
            element: None,
        });
    }
    if counted {
        let text = count_text(bars);
        let (l, top, r, _) = metrics.bounds(&text, false);
        result.glyphs.push(Glyph {
            rotated: false,
            text,
            position: Point {
                x: (left + right) / 2. - (l + r) / 2.,
                y: COUNT_Y,
            },
            element: None,
        });
        result.ink_top = result.ink_top.min(COUNT_Y + top);
    }
    Arc::new(result)
}

/// A bar inside a run: it keeps the run's place, its rhythmic grid and its
/// caret positions, but its ink is the run's first bar.
pub(crate) fn hide(content: &MeasureLayout) -> Arc<MeasureLayout> {
    let mut result = content.clone();
    result.lines.clear();
    result.glyphs.clear();
    result.techniques.clear();
    result.expressions.clear();
    result.lyrics.clear();
    result.marks.clear();
    result.tuplets.clear();
    result.beams.clear();
    result.events.clear();
    result.cue.clear();
    Arc::new(result)
}

/// The count in Bravura's time signature digits, as engravers print it.
pub(crate) fn count_text(bars: usize) -> String {
    bars.to_string()
        .chars()
        .filter_map(|digit| char::from_u32(0xe080 + digit.to_digit(10)?))
        .collect()
}
