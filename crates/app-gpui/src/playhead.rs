//! One musical position for the score, transport ruler and position display.
use gpui::{Bounds, Pixels, point, px, size};
use score_core::{Score, TICKS_PER_QUARTER};
use score_edit::Cursor;
use score_layout::Layout;
use score_playback::Tempo;

pub const ACTIVE: u32 = 0xd9781e;
pub const SELECTED: u32 = 0x3c70cf;
pub const INK: u32 = 0x202c27;

#[derive(Clone, Copy)]
pub struct Position {
    pub tick: f64,
    pub measure: usize,
    pub beat: f64,
    pub seconds: f64,
    pub progress: f32,
    pub at_end: bool,
}

pub fn position(
    score: &Score,
    cursor: Cursor,
    clock: Option<(u64, u32)>,
    tempo: Tempo,
) -> Position {
    let tick = if let Some((frame, rate)) = clock.filter(|(_, rate)| *rate > 0) {
        frame as f64 * f64::from(tempo.bpm()) * f64::from(TICKS_PER_QUARTER)
            / (f64::from(rate) * 60.)
    } else {
        let local: u16 = score
            .part(cursor.part)
            .and_then(|p| p.staff(cursor.staff))
            .and_then(|s| s.measures().get(cursor.measure))
            .map_or(0, |bar| {
                bar.iter()
                    .take(cursor.index)
                    .map(|(_, e)| e.duration.ticks())
                    .sum()
            });
        cursor.measure as f64 * f64::from(score.time().capacity()) + f64::from(local)
    };
    position_at_tick(score, tick, tempo)
}

pub fn position_at_tick(score: &Score, tick: f64, tempo: Tempo) -> Position {
    let capacity = f64::from(score.time().capacity());
    let total = capacity * score.measures().len() as f64;
    let tick = tick.clamp(0., total);
    let measure =
        ((tick / capacity).floor() as usize).min(score.measures().len().saturating_sub(1));
    Position {
        tick,
        measure,
        beat: 1.
            + (tick - measure as f64 * capacity)
                / (f64::from(score_core::TICKS_PER_WHOLE) / f64::from(score.time().denominator)),
        seconds: tick * 60. / (f64::from(tempo.bpm()) * f64::from(TICKS_PER_QUARTER)),
        progress: (tick / total) as f32,
        at_end: tick >= total,
    }
}

/// The ruler's eight-pixel inset is shared by painting and hit testing.
pub fn ruler_tick(score: &Score, x: f32, width: f32) -> u64 {
    let beat = u64::from(score_core::TICKS_PER_WHOLE / u16::from(score.time().denominator));
    let total = u64::from(score.time().capacity()) * score.measures().len() as u64;
    let fraction = ((x - 8.) / (width - 16.).max(1.)).clamp(0., 1.);
    let ticks = (f64::from(fraction) * total as f64 / beat as f64).round() as u64 * beat;
    ticks.min(total.saturating_sub(beat))
}

/// The rhythmic grid is shared by every staff, including implicit silent gaps.
/// The returned rectangle is already scaled into canvas coordinates.
pub fn score_bounds(
    layout: &Layout,
    cursor: Cursor,
    position: Position,
    capacity: u16,
) -> Option<Bounds<Pixels>> {
    let last = layout.measures.iter().map(|m| m.index).max()?;
    let index = position.measure.min(last);
    let bar = layout
        .measure_on(cursor.part, cursor.staff, index)
        .or_else(|| layout.measures.iter().find(|m| m.index == index))?;
    let local = position.tick - index as f64 * f64::from(capacity);
    let x = bar.origin.x + bar.content.x_at_tick(local);
    let mut top = f32::INFINITY;
    let mut bottom = f32::NEG_INFINITY;
    for m in layout.measures.iter().filter(|m| m.index == index) {
        top = top.min(m.origin.y + m.content.ink_top.min(0.) - 14.);
        bottom = bottom.max(m.origin.y + m.content.ink_bottom.max(40.) + 10.);
    }
    Some(Bounds::new(
        point(px(x * layout.scale), px(top * layout.scale)),
        size(px(1.), px((bottom - top) * layout.scale)),
    ))
}

pub fn tracking_bounds(
    layout: &Layout,
    cursor: Cursor,
    position: Position,
    capacity: u16,
) -> Option<Bounds<Pixels>> {
    let last = layout.measures.iter().map(|m| m.index).max()?;
    let index = position.measure.min(last);
    let bar = layout.measure_on(cursor.part, cursor.staff, index)?;
    let x = bar.origin.x
        + bar
            .content
            .x_at_tick(position.tick - index as f64 * f64::from(capacity));
    let top = bar.origin.y + bar.content.ink_top.min(0.) - 22.;
    let bottom = bar.origin.y + bar.content.ink_bottom.max(40.) + 14.;
    Some(Bounds::new(
        point(px(x * layout.scale - 24.), px(top * layout.scale)),
        size(px(48.), px((bottom - top) * layout.scale)),
    ))
}

/// A thin line with a compact down-pointing handle, drawn behind musical ink.
pub fn paint_marker(x: Pixels, top: Pixels, bottom: Pixels, window: &mut gpui::Window) {
    use gpui::{PathBuilder, fill, rgb, rgba};
    window.paint_quad(fill(
        Bounds::new(
            point(x - px(0.5), top + px(7.)),
            size(px(1.), (bottom - top - px(7.)).max(px(0.))),
        ),
        rgba(0xe8974399),
    ));
    let mut cap = PathBuilder::fill();
    cap.move_to(point(x - px(5.), top));
    cap.line_to(point(x + px(5.), top));
    cap.line_to(point(x + px(5.), top + px(4.)));
    cap.line_to(point(x, top + px(9.)));
    cap.line_to(point(x - px(5.), top + px(4.)));
    cap.close();
    if let Ok(path) = cap.build() {
        window.paint_path(path, rgb(0xe89743));
    }
}

pub fn note_color(
    id: Option<score_core::ElementId>,
    selected: &std::collections::HashSet<score_core::ElementId>,
    active: &[score_core::ElementId],
) -> u32 {
    if id.is_some_and(|id| active.contains(&id)) {
        ACTIVE
    } else if id.is_some_and(|id| selected.contains(&id)) {
        SELECTED
    } else {
        INK
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Notation, Pitch, Pitches, StaffId, Step};

    fn note(step: Step, duration: Duration) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration,
            notation: Notation::default(),
        }
    }

    #[test]
    fn ruler_clicks_use_painted_insets_and_snap_to_beats_inside_the_score() {
        let score = Score::default(); // Four bars of 4/4.
        for width in [80., 400., 1000.] {
            let usable = width - 16.;
            for beat in 0..16 {
                let x = 8. + usable * beat as f32 / 16.;
                assert_eq!(ruler_tick(&score, x, width), beat * 48);
            }
            assert_eq!(ruler_tick(&score, -20., width), 0);
            assert_eq!(ruler_tick(&score, width + 20., width), 15 * 48);
            // The last endpoint selects the final playable beat, not FIN.
            assert_eq!(ruler_tick(&score, width - 8., width), 15 * 48);
            assert_eq!(ruler_tick(&score, 8. + usable * 1.6 / 16., width), 96);
        }
    }

    #[test]
    fn ruler_uses_denominator_beats_in_six_eight() {
        let mut value = serde_json::to_value(Score::default()).unwrap();
        value["time"] = serde_json::json!({"numerator":6,"denominator":8});
        let score: Score = serde_json::from_value(value).unwrap();
        assert_eq!(ruler_tick(&score, 8. + 384. / 24., 400.), 24);
        assert_eq!(ruler_tick(&score, 8. + 384. / 4., 400.), 144);
        assert_eq!(ruler_tick(&score, 400., 400.), 552);
    }

    #[test]
    fn audio_clock_is_absolute_and_paused_position_does_not_follow_selection() {
        let mut score = Score::default();
        score
            .insert(2, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let cursor = Cursor {
            part: score.parts()[0].id(),
            staff: StaffId::Upper,
            measure: 2,
            index: 1,
        };
        let tempo = Tempo::default();
        let clock = Some((120_000, 48_000)); // 2.5 seconds = bar 2, beat 2.
        let p = position(&score, cursor, clock, tempo);
        assert_eq!(p.tick, 240.);
        assert_eq!((p.measure, p.beat), (1, 2.));
        assert_eq!(p.seconds, 2.5);
        let paused = position(
            &score,
            Cursor {
                measure: 0,
                index: 0,
                ..cursor
            },
            clock,
            tempo,
        );
        assert_eq!(paused.tick, p.tick);
        let idle = position(&score, cursor, None, tempo);
        assert_eq!((idle.tick, idle.measure, idle.beat), (432., 2, 2.));
    }

    #[test]
    fn playhead_interpolates_shared_rhythm_and_keeps_moving_through_silent_gaps() {
        let mut score = Score::default();
        let part = score.parts()[0].id();
        score.insert(0, 0, note(Step::C, Duration::Eighth)).unwrap();
        score.insert(0, 1, note(Step::D, Duration::Eighth)).unwrap();
        score
            .insert_on(part, StaffId::Lower, 0, 0, note(Step::E, Duration::Half))
            .unwrap();
        let cursor = Cursor {
            part,
            staff: StaffId::Upper,
            measure: 0,
            index: 0,
        };
        let mut layout = score_layout::LayoutEngine::default().layout(&score, 760.);
        for scale in [0.5, 0.7, 1., 1.5] {
            layout.scale = scale;
            let upper = layout.measure_on(part, StaffId::Upper, 0).unwrap();
            let lower = layout.measure_on(part, StaffId::Lower, 0).unwrap();
            for pair in upper.content.time_positions.windows(2) {
                let [(a, x1), (b, x2)] = pair else {
                    unreachable!()
                };
                let tick = f64::from(a + b) / 2.;
                assert!((upper.content.x_at_tick(tick) - (x1 + x2) / 2.).abs() < 0.01);
                assert!(
                    (upper.content.x_at_tick(tick) - lower.content.x_at_tick(tick)).abs() < 0.01
                );
                let p = Position {
                    tick,
                    measure: 0,
                    beat: 1.,
                    seconds: 0.,
                    progress: 0.,
                    at_end: false,
                };
                let marker = score_bounds(&layout, cursor, p, score.time().capacity()).unwrap();
                assert!(
                    (f32::from(marker.left()) - (upper.origin.x + (x1 + x2) / 2.) * scale).abs()
                        < 0.01
                );
                assert!(marker.size.height > px(40. * scale));
            }
            assert!(upper.content.x_at_tick(180.) > upper.content.x_at_tick(120.));
        }
    }

    #[test]
    fn system_boundaries_and_end_of_score_use_the_correct_measure_geometry() {
        let mut score = Score::default();
        for measure in 0..4 {
            score
                .insert(measure, 0, note(Step::C, Duration::Quarter))
                .unwrap();
        }
        let cursor = Cursor {
            part: score.parts()[0].id(),
            ..Default::default()
        };
        let layout = score_layout::LayoutEngine::default().layout_compact(&score, 500.);
        for measure in 0..4 {
            let p = Position {
                tick: measure as f64 * 192.,
                measure,
                beat: 1.,
                seconds: 0.,
                progress: 0.,
                at_end: false,
            };
            let bar = layout
                .measure_on(cursor.part, cursor.staff, measure)
                .unwrap();
            let marker = score_bounds(&layout, cursor, p, 192).unwrap();
            assert!(
                (f32::from(marker.left()) - bar.origin.x - bar.content.x_at_tick(0.)).abs() < 0.01
            );
            let tracking = tracking_bounds(&layout, cursor, p, 192).unwrap();
            assert!(tracking.top() < px(bar.origin.y));
        }
        let end = Position {
            tick: 768.,
            measure: 4,
            beat: 1.,
            seconds: 8.,
            progress: 1.,
            at_end: true,
        };
        let last = layout.measure_on(cursor.part, cursor.staff, 3).unwrap();
        let marker = score_bounds(&layout, cursor, end, 192).unwrap();
        assert!(
            (f32::from(marker.left()) - last.origin.x - last.content.x_at_tick(192.)).abs() < 0.01
        );
    }

    #[test]
    fn stopped_cursor_after_the_final_whole_note_stays_in_the_last_measure() {
        let mut score = Score::default();
        let id = score.insert(3, 0, note(Step::C, Duration::Whole)).unwrap();
        let mut editor = score_edit::Editor::new(score).unwrap();
        editor.select(id);
        let p = position(editor.score(), editor.cursor(), None, Tempo::default());
        assert_eq!(
            (p.measure, p.tick, p.progress, p.at_end),
            (3, 768., 1., true)
        );
        assert_eq!(p.beat, 5.);
    }

    #[test]
    fn active_note_ink_wins_over_selection_and_only_stems_own_strokes() {
        let mut score = Score::default();
        let a = score.insert(0, 0, note(Step::C, Duration::Eighth)).unwrap();
        let b = score.insert(0, 1, note(Step::D, Duration::Eighth)).unwrap();
        let selected = std::collections::HashSet::from([a, b]);
        assert_eq!(note_color(Some(a), &selected, &[a]), ACTIVE);
        assert_eq!(note_color(Some(b), &selected, &[a]), SELECTED);
        assert_eq!(note_color(None, &selected, &[a]), INK);
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let bar = layout.measure_on(a.part, a.staff, 0).unwrap();
        assert!(bar.content.lines.iter().any(|line| line.element == Some(a)));
        assert!(bar.content.lines.iter().any(|line| line.element == Some(b)));
        for line in &bar.content.lines {
            if line.element.is_some() {
                assert_eq!(line.from.x, line.to.x);
            }
            if line.from.y == line.to.y {
                assert!(line.element.is_none());
            }
        }
    }
}
