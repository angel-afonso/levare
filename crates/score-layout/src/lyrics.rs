//! Lyrics under the staff: every verse on its own line across the row,
//! dynamics below them, hyphens between the syllables of a word and an
//! extender under a syllable held over several notes.
use super::*;
use score_core::{Join, MarkRef, TextKind};

/// Provisional baseline of the first verse, before the row is known.
pub(super) const FIRST: f32 = 64.;
/// Distance between the baselines of two verses.
pub(super) const LINE: f32 = 20.;

/// Put the row's verses on common lines under its notes and slurs, and move
/// the dynamics and expressions of the row under them. Returns the new bottom
/// of the row, if it has words.
pub(super) fn arrange(
    measures: &mut [PlacedMeasure],
    metrics: &Metrics,
    below_curve: bool,
) -> Option<f32> {
    let verses = measures
        .iter()
        .flat_map(|bar| bar.content.lyrics.iter().map(|lyric| lyric.verse))
        .max()?;
    let notes = measures
        .iter()
        .map(|bar| expression::notation_bounds(&bar.content, metrics).1)
        .fold(40f32, f32::max);
    let first = notes + if below_curve { 22. } else { 0. } + 20.;
    let last = first + f32::from(verses) * LINE + 6.;
    let lane = |mark: &MarkBox| {
        matches!(
            mark.mark,
            MarkRef::Dynamic(_) | MarkRef::Text(_, TextKind::Expression)
        )
    };
    let dynamics = measures
        .iter()
        .flat_map(|bar| bar.content.marks.iter().filter(|mark| lane(mark)))
        .map(|mark| mark.center.y - mark.half.1)
        .fold(f32::INFINITY, f32::min);
    let shift = (last + 6. - dynamics).max(0.);
    let mut bottom = last;
    for bar in measures.iter_mut() {
        let has_dynamics = bar.content.marks.iter().any(lane);
        if bar.content.lyrics.is_empty() && !(has_dynamics && shift > 0.) {
            continue;
        }
        let content = Arc::make_mut(&mut bar.content);
        for lyric in &mut content.lyrics {
            lyric.position.y = first + f32::from(lyric.verse) * LINE;
            let reference = MarkRef::Lyric(lyric.element, lyric.verse);
            for mark in content
                .marks
                .iter_mut()
                .filter(|mark| mark.mark == reference)
            {
                mark.center.y = lyric.position.y - 5.;
            }
        }
        if shift > 0. {
            for index in 0..content.marks.len() {
                if lane(&content.marks[index]) {
                    content.marks[index].center.y += shift;
                    if let Some(glyph) = content.marks[index].glyph {
                        content.glyphs[glyph].position.y += shift;
                    }
                }
            }
            for label in &mut content.expressions {
                label.position.y += shift;
            }
        }
        let ink = content
            .marks
            .iter()
            .filter(|mark| lane(mark))
            .map(|mark| mark.center.y + mark.half.1 + 4.)
            .fold(last, f32::max);
        content.ink_bottom = content.ink_bottom.max(ink);
        content.bottom = content.bottom.max(ink + 6.);
        bottom = bottom.max(ink + 6.);
    }
    Some(bottom)
}

/// Hyphens and extenders for every staff, once the rows are placed.
pub(super) fn lines(measures: &[PlacedMeasure], thickness: f32) -> Vec<Line> {
    let mut staves: Vec<((PartId, StaffId), Vec<&PlacedMeasure>)> = Vec::new();
    for bar in measures.iter().filter(|bar| !bar.content.lyrics.is_empty()) {
        let key = (bar.part, bar.source_staff);
        match staves.iter_mut().find(|(other, _)| *other == key) {
            Some((_, bars)) => bars.push(bar),
            None => staves.push((key, vec![bar])),
        }
    }
    let mut result = Vec::new();
    for (_, bars) in staves {
        for verse in 0..score_core::MAX_VERSES as u8 {
            let mut words: Vec<(&PlacedMeasure, &LyricLabel)> = bars
                .iter()
                .flat_map(|bar| {
                    bar.content
                        .lyrics
                        .iter()
                        .filter(move |lyric| lyric.verse == verse)
                        .map(move |lyric| (*bar, lyric))
                })
                .collect();
            words.sort_by(|a, b| {
                (a.0.index, a.0.origin.x + a.1.position.x)
                    .partial_cmp(&(b.0.index, b.0.origin.x + b.1.position.x))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for (k, &(bar, lyric)) in words.iter().enumerate() {
                let end = bar.origin.x + lyric.position.x + lyric.width();
                let y = bar.origin.y + lyric.position.y;
                let next = words.get(k + 1).copied();
                match lyric.syllable.join {
                    Join::Word => {}
                    Join::Hyphen => {
                        let row_end = |bar: &PlacedMeasure| bar.origin.x + bar.width;
                        let (from, to) = match next {
                            Some((other, word)) if other.origin.y == bar.origin.y => {
                                (end, other.origin.x + word.position.x)
                            }
                            // The word goes on in the next system: hyphen at the row end.
                            _ => (end, (end + 14.).min(row_end(bar))),
                        };
                        let gap = to - from;
                        if gap > 5. {
                            let length = (gap * 0.5).min(8.);
                            let middle = (from + to) / 2.;
                            result.push(Line {
                                from: Point {
                                    x: middle - length / 2.,
                                    y: y - 4.,
                                },
                                to: Point {
                                    x: middle + length / 2.,
                                    y: y - 4.,
                                },
                                thickness,
                                element: None,
                            });
                        }
                    }
                    Join::Melisma => {
                        // Under every later note of the voice in this row
                        // before the next syllable.
                        let Some(voice) = bar
                            .content
                            .events
                            .iter()
                            .find(|e| e.id == lyric.element)
                            .map(|e| e.voice)
                        else {
                            continue;
                        };
                        let stop = next.map(|(other, word)| {
                            (other.index, other.origin.x + word.position.x - 4.)
                        });
                        let last = bars
                            .iter()
                            .filter(|other| {
                                other.origin.y == bar.origin.y && other.index >= bar.index
                            })
                            .flat_map(|other| {
                                other.content.events.iter().filter_map(move |e| {
                                    (e.voice == voice && e.pitch.is_some())
                                        .then_some((other.index, other.origin.x + e.center.x + 6.))
                                })
                            })
                            .filter(|&(index, x)| {
                                x > end && stop.is_none_or(|stop| (index, x) < stop)
                            })
                            .map(|(_, x)| x)
                            .fold(f32::NEG_INFINITY, f32::max);
                        if last > end + 4. {
                            result.push(Line {
                                from: Point { x: end + 2., y },
                                to: Point { x: last, y },
                                thickness,
                                element: None,
                            });
                        }
                    }
                }
            }
        }
    }
    result
}
