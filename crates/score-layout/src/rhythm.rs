//! Local rhythmic marks. Incomplete tuplets remain visible while composing.
use super::*;

#[derive(Clone, Debug)]
pub struct TupletGroup {
    pub elements: Vec<ElementId>,
    pub top: f32,
    pub left: f32,
    pub right: f32,
    pub bracketed: bool,
}

/// From one augmentation dot to the next.
pub const DOT_STEP: f32 = 9.;

pub(super) fn dots(
    result: &mut MeasureLayout,
    event: score_core::Event,
    id: ElementId,
    metrics: &Metrics,
) {
    if !event.duration.is_dotted() {
        return;
    }
    let heads: Vec<_> = result.events.iter().filter(|h| h.id == id).collect();
    let half_width = if event.duration.value() == score_core::NoteValue::Whole {
        metrics.whole
    } else {
        metrics.head
    } / 2.;
    let right = heads
        .iter()
        .map(|h| h.center.x + half_width)
        .fold(0f32, f32::max);
    let flagged = matches!(
        event.duration.value(),
        score_core::NoteValue::Eighth
            | score_core::NoteValue::Sixteenth
            | score_core::NoteValue::ThirtySecond
            | score_core::NoteValue::SixtyFourth
    );
    let x = right
        + if flagged && !event.pitches.is_empty() {
            16.
        } else {
            6.
        };
    let mut ys: Vec<_> = heads.iter().map(|h| h.center.y).collect();
    if event.notation.harmonic == Some(score_core::Harmonic::Artificial)
        && let Some(mark) = result
            .marks
            .iter()
            .find(|mark| mark.mark == MarkRef::Harmonic(id))
    {
        ys.push(mark.center.y);
    }
    ys.sort_by(f32::total_cmp);
    let mut occupied = Vec::new();
    for y in ys {
        // A dot on a staff line moves to the adjacent space. Chord seconds
        // get distinct spaces in one common dot column.
        let mut slot = if (y / 5.).round() as i32 % 2 == 0 {
            y - 5.
        } else {
            y
        };
        while occupied.contains(&(slot as i32)) {
            slot += 10.;
        }
        occupied.push(slot as i32);
        for dot in 0..event.duration.dots() {
            result.glyphs.push(Glyph {
                rotated: false,
                text: "\u{e1e7}".into(),
                position: Point {
                    x: x + f32::from(dot) * DOT_STEP,
                    y: slot,
                },
                element: Some(id),
            });
        }
        result.top = result.top.min(slot - 6.);
        result.bottom = result.bottom.max(slot + 6.);
    }
}

/// One voice's tuplet groups as `score_core` derives them.
pub(super) struct Group {
    pub elements: Vec<ElementId>,
    pub actual: u8,
    pub complete: bool,
}

pub(super) fn groups(measure: &Measure, part: PartId, staff: StaffId, index: usize) -> Vec<Group> {
    Voice::ALL
        .into_iter()
        .flat_map(|voice| measure.tuplet_groups(voice))
        .map(|group| Group {
            elements: group
                .events
                .iter()
                .map(|&event| ElementId {
                    part,
                    staff,
                    measure: index,
                    event,
                })
                .collect(),
            actual: group.tuplet.actual(),
            complete: group.is_complete(),
        })
        .collect()
}
pub(super) fn tuplets(
    result: &mut MeasureLayout,
    measure: &Measure,
    part: PartId,
    staff: StaffId,
    index: usize,
    metrics: &Metrics,
) {
    let groups = groups(measure, part, staff, index);
    if groups.is_empty() {
        return;
    }
    let note_lines = result.lines.clone();
    for group in groups {
        let visible: std::collections::HashSet<_> = result.events.iter().map(|e| e.id).collect();
        let runs: Vec<_> = group
            .elements
            .split(|id| !visible.contains(id))
            .filter(|run| !run.is_empty())
            .collect();
        // A group shown whole may lose its bracket to a beam.
        let whole = group.complete && runs.len() == 1 && runs[0].len() == group.elements.len();
        for run in runs {
            bracket(
                result,
                run,
                group.actual,
                whole,
                &note_lines,
                metrics,
                measure.has_two_voices() && measure.voice_of(run[0].event) == Some(Voice::Two),
            );
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn bracket(
    result: &mut MeasureLayout,
    ids: &[ElementId],
    actual: u8,
    whole: bool,
    note_lines: &[Line],
    metrics: &Metrics,
    below: bool,
) {
    if ids.is_empty() {
        return;
    }
    let heads: Vec<_> = result
        .events
        .iter()
        .filter(|e| ids.contains(&e.id))
        .collect();
    let left = heads
        .iter()
        .map(|h| h.center.x)
        .fold(f32::INFINITY, f32::min)
        - 10.;
    let right = heads
        .iter()
        .map(|h| h.center.x)
        .fold(f32::NEG_INFINITY, f32::max)
        + 12.;
    let mut top = 0f32;
    for glyph in result
        .glyphs
        .iter()
        .filter(|g| g.element.is_some_and(|id| ids.contains(&id)))
    {
        for c in glyph.text.chars() {
            top = top.min(glyph.position.y + metrics.glyph_bounds.get(&c).map_or(-44., |b| b.0));
        }
    }
    // Technique and division text is arranged outside the musical symbols
    // after the row is known; it must not lift the tuplet or its slur.
    for line in note_lines
        .iter()
        .filter(|l| l.from.x >= left && l.from.x <= right && l.to.x >= left && l.to.x <= right)
    {
        top = top.min(line.from.y.min(line.to.y));
    }
    let bottom = heads
        .iter()
        .map(|head| head.center.y)
        .fold(40., f32::max)
        .max(
            note_lines
                .iter()
                .filter(|l| l.element.is_some_and(|id| ids.contains(&id)))
                .map(|l| l.from.y.max(l.to.y))
                .fold(40., f32::max),
        );
    let y = if below { bottom + 20. } else { top - 14. };
    let center = (left + right) / 2.;
    // A complete group under one beam of its own shows only its number.
    let bracketed = !(whole && result.beams.iter().any(|g| g.elements == ids));
    if bracketed {
        for (x1, y1, x2, y2) in [
            (left, y + if below { -6. } else { 6. }, left, y),
            (left, y, center - 8., y),
            (center + 8., y, right, y),
            (right, y, right, y + if below { -6. } else { 6. }),
        ] {
            result.lines.push(Line {
                from: Point { x: x1, y: y1 },
                to: Point { x: x2, y: y2 },
                thickness: 1.,
                element: None,
            });
        }
    }
    result.glyphs.push(Glyph {
        rotated: false,
        // SMuFL tuplet digits start at U+E880.
        text: char::from_u32(0xe880 + u32::from(actual)).unwrap().into(),
        position: Point {
            x: center - 4.,
            y: y + 5.,
        },
        element: Some(ids[0]),
    });
    if below {
        result.bottom = result.bottom.max(y + 18.);
    } else {
        result.top = result.top.min(y - 16.);
    }
    result.tuplets.push(TupletGroup {
        elements: ids.to_vec(),
        top: y - 16.,
        left,
        right,
        bracketed,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Pitch, Pitches, Step};
    #[test]
    fn dots_clear_staff_lines_chord_seconds_and_flags_and_triplets_reserve_ink() {
        let mut s = Score::default();
        let chord = Event {
            pitches: Pitches::new(&[
                Pitch::new(Step::E, 4).unwrap(),
                Pitch::new(Step::F, 4).unwrap(),
                Pitch::new(Step::G, 4).unwrap(),
            ])
            .unwrap(),
            duration: Duration::Eighth.with_dotted(true),
            notation: Default::default(),
        };
        let a = s.insert(0, 0, chord).unwrap();
        let mut ids = Vec::new();
        for i in 1..4 {
            ids.push(
                s.insert(
                    0,
                    i,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 6)),
                        duration: Duration::Eighth.with_triplet(true),
                        notation: Default::default(),
                    },
                )
                .unwrap(),
            );
        }
        let mut engine = LayoutEngine::default();
        let layout = engine.layout_compact(&s, 960.);
        let m = layout.measure(a.part, 0).unwrap();
        let dots: Vec<_> = m
            .content
            .glyphs
            .iter()
            .filter(|g| g.element == Some(a) && g.text == "\u{e1e7}")
            .collect();
        assert_eq!(dots.len(), 3);
        assert!(
            dots.iter()
                .all(|g| ((g.position.y / 5.).round() as i32) % 2 != 0)
        );
        let mut ys: Vec<_> = dots.iter().map(|g| g.position.y as i32).collect();
        ys.sort();
        ys.dedup();
        assert_eq!(ys.len(), 3);
        assert_eq!(m.content.tuplets.len(), 1);
        assert_eq!(m.content.tuplets[0].elements, ids);
        assert!(m.content.glyphs.iter().any(|g| g.text == "\u{e883}"));
        assert!(m.origin.y + m.content.ink_top >= 0.);
        engine.layout_compact(&s, 960.);
        assert_eq!(engine.recomputed_measures, 0);
        s.set_duration(a, Duration::Eighth).unwrap();
        engine.layout_compact(&s, 960.);
        assert_eq!(engine.recomputed_measures, 2);
    }
    #[test]
    fn consecutive_triplets_group_in_threes_and_incomplete_groups_remain_visible() {
        let mut s = Score::default();
        for i in 0..7 {
            s.insert(
                0,
                i,
                Event {
                    pitches: Pitches::default(),
                    duration: Duration::Eighth.with_triplet(true),
                    notation: Default::default(),
                },
            )
            .unwrap();
        }
        let layout = LayoutEngine::default().layout_compact(&s, 960.);
        let m = layout.measure(s.parts()[0].id(), 0).unwrap();
        assert_eq!(
            m.content
                .tuplets
                .iter()
                .map(|g| g.elements.len())
                .collect::<Vec<_>>(),
            [3, 3, 1]
        );
        for g in &m.content.tuplets {
            assert!(g.top >= m.content.ink_top - 20.);
        }
    }
}

#[cfg(test)]
mod tuplet_tests {
    use super::*;
    use score_core::{Duration, Event, NoteValue, Pitch, Pitches, Step, Tuplet};

    fn note(duration: Duration) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(Step::B, 4)),
            duration,
            notation: Default::default(),
        }
    }

    fn bar(durations: &[Duration]) -> (Score, Vec<ElementId>) {
        let mut score = Score::default();
        let ids = durations
            .iter()
            .enumerate()
            .map(|(index, &duration)| score.insert(0, index, note(duration)).unwrap())
            .collect();
        (score, ids)
    }

    fn digit(actual: u32) -> String {
        char::from_u32(0xe880 + actual).unwrap().into()
    }

    #[test]
    fn a_beamed_quintuplet_shows_its_5_without_a_bracket() {
        let sixteenth = Duration::Sixteenth.with_tuplet(Tuplet::usual(5, NoteValue::Sixteenth));
        let (score, ids) = bar(&[sixteenth; 5]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        assert_eq!(m.tuplets.len(), 1);
        assert_eq!(m.tuplets[0].elements, ids);
        assert!(!m.tuplets[0].bracketed);
        assert!(m.glyphs.iter().any(|g| g.text == digit(5)));
        assert!(m.beams.iter().any(|b| b.elements == ids));
    }

    #[test]
    fn unbeamed_and_incomplete_tuplets_keep_their_bracket() {
        let quarter = Duration::Quarter.with_tuplet(Tuplet::usual(5, NoteValue::Quarter));
        let (score, ids) = bar(&[quarter; 5]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        assert_eq!(m.tuplets.len(), 1);
        assert!(m.tuplets[0].bracketed);
        let eighth = Duration::Eighth.with_tuplet(Tuplet::usual(6, NoteValue::Eighth));
        let (score, ids) = bar(&[eighth; 4]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        assert!(m.tuplets[0].bracketed);
        assert!(m.glyphs.iter().any(|g| g.text == digit(6)));
    }

    #[test]
    fn mixed_values_share_one_bracket() {
        let in_eighths = |d: Duration| d.with_tuplet(Some(Tuplet::triplet(NoteValue::Eighth)));
        let (score, ids) = bar(&[in_eighths(Duration::Quarter), in_eighths(Duration::Eighth)]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        assert_eq!(m.tuplets.len(), 1);
        assert_eq!(m.tuplets[0].elements, ids);
    }

    #[test]
    fn two_dots_sit_side_by_side_in_one_space() {
        let (score, ids) = bar(&[Duration::Quarter.with_dots(2), Duration::Sixteenth]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        let mut dots: Vec<_> = m
            .glyphs
            .iter()
            .filter(|g| g.element == Some(ids[0]) && g.text == "\u{e1e7}")
            .map(|g| g.position)
            .collect();
        dots.sort_by(|a, b| a.x.total_cmp(&b.x));
        assert_eq!(dots.len(), 2);
        assert_eq!(dots[0].y, dots[1].y);
        assert_eq!(dots[1].x - dots[0].x, DOT_STEP);
        // The next note starts clear of the second dot.
        let next = m.events.iter().find(|e| e.id == ids[1]).unwrap();
        assert!(next.center.x - metrics_head() / 2. > dots[1].x + 4.);
    }

    fn metrics_head() -> f32 {
        Metrics::bravura().head
    }

    #[test]
    fn sixty_fourths_get_four_beams_and_a_lone_one_its_flag() {
        let (score, ids) = bar(&[Duration::SixtyFourth; 4]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        let beam = m.beams.iter().find(|b| b.elements == ids).unwrap();
        let levels: std::collections::HashSet<_> = beam.segments.iter().map(|s| s.level).collect();
        assert_eq!(levels.len(), 4);
        // A lone note is one Bravura glyph with its stem and flags.
        let (score, ids) = bar(&[Duration::SixtyFourth, Duration::Quarter]);
        let layout = LayoutEngine::default().layout_compact(&score, 960.);
        let m = &layout.measure(ids[0].part, 0).unwrap().content;
        let lone = ["\u{e1dd}", "\u{e1de}"];
        assert!(
            m.glyphs
                .iter()
                .any(|g| g.element == Some(ids[0]) && lone.contains(&g.text.as_str()))
        );
        let metrics = Metrics::bravura();
        for glyph in ['\u{e1dd}', '\u{e1de}', '\u{e246}', '\u{e247}', '\u{e4e9}'] {
            assert!(metrics.glyph_bounds.contains_key(&glyph), "{glyph:?}");
        }
    }
}

#[cfg(test)]
mod dense_tests {
    use super::*;
    use score_core::{Duration, Event, Pitches};
    #[test]
    fn consecutive_brackets_use_note_ink_and_keep_a_flat_lane_without_touching() {
        let mut s = Score::default();
        for i in 0..24 {
            s.insert(
                0,
                i,
                Event {
                    pitches: Pitches::default(),
                    duration: Duration::Sixteenth.with_triplet(true),
                    notation: Default::default(),
                },
            )
            .unwrap();
        }
        for width in [600., 960., 1600.] {
            let layout = LayoutEngine::default().layout_compact(&s, width);
            let groups = &layout
                .measure(s.parts()[0].id(), 0)
                .unwrap()
                .content
                .tuplets;
            assert_eq!(groups.len(), 8);
            assert!(groups.iter().all(|g| g.top == groups[0].top));
            assert!(groups.windows(2).all(|p| p[0].right < p[1].left));
        }
    }
}
