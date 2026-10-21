//! One musical position for the score, transport ruler and position display.
use gpui::{Bounds, Pixels, point, px, size};
use score_core::Score;
use score_edit::Cursor;
use score_layout::Layout;
use score_playback::Timeline;

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
    timeline: &Timeline,
) -> Position {
    let tick = if let Some((frame, rate)) = clock.filter(|(_, rate)| *rate > 0) {
        timeline.tick_at(frame, rate)
    } else {
        let local: u16 = score
            .part(cursor.part)
            .and_then(|p| p.staff(cursor.staff))
            .and_then(|s| s.measures().get(cursor.measure))
            .map_or(0, |bar| {
                bar.voice_iter(cursor.voice)
                    .take(cursor.index)
                    .map(|(_, e)| e.duration.ticks())
                    .sum()
            });
        score.measure_start_tick(cursor.measure) as f64 + f64::from(local)
    };
    let mut position = position_at_tick(score, tick, timeline);
    if let Some((frame, rate)) = clock.filter(|(_, rate)| *rate > 0) {
        position.seconds = frame as f64 / f64::from(rate);
        position.at_end = frame >= timeline.total_frames(rate);
    }
    position
}

pub fn position_at_tick(score: &Score, tick: f64, timeline: &Timeline) -> Position {
    let total = score.total_ticks() as f64;
    let tick = tick.clamp(0., total);
    let measure = score
        .measure_at_tick(tick.floor() as u64)
        .0
        .min(score.measures().len() - 1);
    let begin = score.measure_start_tick(measure) as f64;
    Position {
        tick,
        measure,
        beat: 1.
            + (tick - begin)
                / (f64::from(score_core::TICKS_PER_WHOLE)
                    / f64::from(score.time_at(measure).denominator)),
        seconds: timeline.seconds_at(tick),
        progress: (tick / total) as f32,
        at_end: tick >= total,
    }
}

/// The ruler's eight-pixel inset is shared by painting and hit testing.
pub fn ruler_tick(score: &Score, x: f32, width: f32) -> u64 {
    let total = score.total_ticks();
    let fraction = ((x - 8.) / (width - 16.).max(1.)).clamp(0., 1.);
    let raw = (f64::from(fraction) * total as f64).floor() as u64;
    let measure = score.measure_at_tick(raw).0.min(score.measures().len() - 1);
    let begin = score.measure_start_tick(measure);
    let time = score.time_at(measure);
    let beat = u64::from(score_core::TICKS_PER_WHOLE / u16::from(time.denominator));
    let local = (((raw - begin) as f64 / beat as f64).round() as u64 * beat)
        .min(u64::from(time.capacity()));
    if begin + local >= total {
        let last = score.time_at(score.measures().len() - 1);
        total - u64::from(score_core::TICKS_PER_WHOLE / u16::from(last.denominator))
    } else {
        begin + local
    }
}

/// Clicked notes keep their exact onset (including tuplets); whitespace snaps
/// to the local beat on the engraved rhythmic grid, across zoom and divisi.
pub fn score_tick(
    score: &Score,
    layout: &Layout,
    point: score_layout::Point,
    voice: score_core::Voice,
) -> Option<u64> {
    if let Some((id, _, tick)) = layout.hit_pitch_with_position(point, voice) {
        return Some(score.measure_start_tick(id.measure) + u64::from(tick));
    }
    let (part, staff, bar) = layout.measure_at(point)?;
    let measure = layout.measure_on(part, staff, bar)?;
    let raw = measure
        .content
        .tick_at_x(point.x / layout.scale - measure.origin.x);
    let time = score.time_at(bar);
    let beat = score_core::TICKS_PER_WHOLE / u16::from(time.denominator);
    let mut local = ((raw / f64::from(beat)).round() as u16 * beat).min(time.capacity());
    if bar + 1 == score.measures().len() {
        local = local.min(time.capacity() - beat);
    }
    Some(score.measure_start_tick(bar) + u64::from(local))
}

/// The rhythmic grid is shared by every staff, including implicit silent gaps.
/// The returned rectangle is already scaled into canvas coordinates.
pub fn score_bounds(layout: &Layout, cursor: Cursor, position: Position) -> Option<Bounds<Pixels>> {
    let last = layout.measures.iter().map(|m| m.index).max()?;
    let index = position.measure.min(last);
    let bar = layout
        .voice_measure_at_tick(
            cursor.part,
            cursor.staff,
            index,
            cursor.voice,
            position.tick
                - layout
                    .measures
                    .iter()
                    .find(|m| m.index == index)?
                    .start_tick as f64,
        )
        .or_else(|| layout.measures.iter().find(|m| m.index == index))?;
    let local = position.tick - bar.start_tick as f64;
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

/// Input caret on the cursor's displayed staff, in scaled canvas coordinates.
pub fn caret_bounds(layout: &Layout, cursor: Cursor) -> Option<Bounds<Pixels>> {
    let at = layout.voice_cursor(
        cursor.part,
        cursor.staff,
        cursor.measure,
        cursor.voice,
        cursor.index,
    )?;
    let space = score_layout::STAFF_SPACE * layout.scale;
    Some(Bounds::new(
        point(px(at.x - 1.), px(at.y - space)),
        size(px(2.), px(6. * space)),
    ))
}

pub fn voice_color(voice: score_core::Voice) -> u32 {
    match voice {
        score_core::Voice::One => 0x2f6fdf,
        score_core::Voice::Two => 0x2f9e57,
    }
}

/// The input caret and playback marker are mutually exclusive.
pub fn indicators(note_entry: bool, playing: bool) -> (bool, bool) {
    (note_entry && !playing, playing || !note_entry)
}

pub fn tracking_bounds(
    layout: &Layout,
    cursor: Cursor,
    position: Position,
) -> Option<Bounds<Pixels>> {
    let last = layout.measures.iter().map(|m| m.index).max()?;
    let index = position.measure.min(last);
    let bar = layout.voice_measure_at_tick(
        cursor.part,
        cursor.staff,
        index,
        cursor.voice,
        position.tick
            - layout
                .measures
                .iter()
                .find(|m| m.index == index)?
                .start_tick as f64,
    )?;
    let x = bar.origin.x + bar.content.x_at_tick(position.tick - bar.start_tick as f64);
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
    use score_core::{Duration, Event, Notation, Pitch, Pitches, StaffId, Step, Voice};

    fn note(step: Step, duration: Duration) -> Event {
        Event {
            pitches: Pitches::single(Pitch::new(step, 4)),
            duration,
            notation: Notation::default(),
        }
    }

    #[test]
    fn caret_stays_on_the_cursor_staff_across_views_and_zoom() {
        for bytes in [
            include_bytes!("../../../examples/menuetto-y-trio-orquesta.partitura").as_slice(),
            include_bytes!("../../../examples/piano-arpegios.partitura").as_slice(),
        ] {
            let score = score_io::decode(bytes).unwrap();
            for mode in [
                score_layout::ViewMode::Page,
                score_layout::ViewMode::Continuous,
                score_layout::ViewMode::SingleSystem,
            ] {
                let mut layout =
                    score_layout::LayoutEngine::default().layout_view(&score, 1000., mode);
                for scale in [0.5, 0.7, 1., 1.5] {
                    layout.scale = scale;
                    for m in layout
                        .measures
                        .iter()
                        .filter(|m| m.source_staff == m.staff && m.source_voice.is_none())
                    {
                        let bar = &score
                            .part(m.part)
                            .unwrap()
                            .staff(m.source_staff)
                            .unwrap()
                            .measures()[m.index];
                        for index in 0..=bar.voice_len(Voice::One) {
                            let cursor = Cursor {
                                part: m.part,
                                staff: m.source_staff,
                                voice: Voice::One,
                                measure: m.index,
                                index,
                            };
                            let caret = caret_bounds(&layout, cursor).unwrap_or_else(|| {
                                panic!(
                                    "missing caret for part {:?} bar {} index {}",
                                    m.part, m.index, index
                                )
                            });
                            let x = f32::from(caret.origin.x);
                            let y = f32::from(caret.origin.y);
                            let height = f32::from(caret.size.height);
                            assert!(
                                x >= m.origin.x * scale - 2.
                                    && x <= (m.origin.x + m.width) * scale + 2.
                            );
                            assert!((y - (m.origin.y - 10.) * scale).abs() < 0.01);
                            assert!((height - 60. * scale).abs() < 0.01);
                            if let Some(next) = layout
                                .measures
                                .iter()
                                .filter(|other| {
                                    other.system_start == m.system_start
                                        && other.index == m.index
                                        && other.origin.y > m.origin.y
                                })
                                .map(|other| other.origin.y - m.origin.y)
                                .min_by(f32::total_cmp)
                            {
                                assert!(height < next * scale);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn voice_two_and_divisi_carets_resolve_to_their_displayed_staves() {
        let mut checked_voice_two = 0;
        let mut checked_divisi = 0;
        for bytes in [
            include_bytes!("../../../examples/divisi-dentro-del-compas.partitura").as_slice(),
            include_bytes!("../../../examples/piano-arpegios.partitura").as_slice(),
        ] {
            let score = score_io::decode(bytes).unwrap();
            for mode in [
                score_layout::ViewMode::Page,
                score_layout::ViewMode::Continuous,
                score_layout::ViewMode::SingleSystem,
            ] {
                let mut layout =
                    score_layout::LayoutEngine::default().layout_view(&score, 1000., mode);
                for scale in [0.5, 1., 1.5] {
                    layout.scale = scale;
                    let mut sources = std::collections::HashSet::new();
                    for m in &layout.measures {
                        if m.source_staff != m.staff {
                            checked_divisi += 1;
                        }
                        if !sources.insert((m.part, m.source_staff, m.index)) {
                            continue;
                        }
                        let bar = &score
                            .part(m.part)
                            .unwrap()
                            .staff(m.source_staff)
                            .unwrap()
                            .measures()[m.index];
                        for index in 0..=bar.voice_len(Voice::Two) {
                            let cursor = Cursor {
                                part: m.part,
                                staff: m.source_staff,
                                voice: Voice::Two,
                                measure: m.index,
                                index,
                            };
                            let caret = caret_bounds(&layout, cursor).unwrap_or_else(|| panic!("missing voice-two caret for part {:?} staff {:?} bar {} index {}", m.part, m.source_staff, m.index, index));
                            let tick = f64::from(m.source_onsets[1][index]);
                            let cap = f64::from(m.content.time_positions.last().unwrap().0);
                            let displayed = layout
                                .voice_measure_at_tick(
                                    m.part,
                                    m.source_staff,
                                    m.index,
                                    Voice::Two,
                                    tick.min(cap - 0.001),
                                )
                                .unwrap();
                            assert!(
                                (f32::from(caret.origin.y) - (displayed.origin.y - 10.) * scale)
                                    .abs()
                                    < 0.01
                            );
                            checked_voice_two += 1;
                        }
                    }
                }
            }
        }
        assert!(checked_voice_two > 0);
        assert!(checked_divisi > 0);
    }

    #[test]
    fn each_voice_has_its_own_caret_color() {
        assert_ne!(voice_color(Voice::One), voice_color(Voice::Two));
        assert_ne!(voice_color(Voice::One), 0xe89743);
        assert_ne!(voice_color(Voice::Two), 0xe89743);
    }

    #[test]
    fn indicator_states_cover_entry_and_playback_without_overlap() {
        for (entry, playing, expected) in [
            (false, false, (false, true)),
            (false, true, (false, true)),
            (true, false, (true, false)),
            (true, true, (false, true)),
        ] {
            assert_eq!(indicators(entry, playing), expected);
        }
    }

    #[test]
    fn score_clicks_keep_exact_note_onsets_across_views_zoom_voices_and_divisi() {
        for bytes in [
            include_bytes!("../../../examples/divisi-dentro-del-compas.partitura").as_slice(),
            include_bytes!("../../../examples/menuetto-y-trio-cuerdas.partitura").as_slice(),
            include_bytes!("../../../examples/piano-arpegios.partitura").as_slice(),
        ] {
            let score = score_io::decode(bytes).unwrap();
            for mode in [
                score_layout::ViewMode::Page,
                score_layout::ViewMode::Continuous,
                score_layout::ViewMode::SingleSystem,
            ] {
                let mut layout =
                    score_layout::LayoutEngine::default().layout_view(&score, 1000., mode);
                for scale in [0.5, 0.7, 1., 1.5] {
                    layout.scale = scale;
                    for m in &layout.measures {
                        for event in &m.content.events {
                            let point = score_layout::Point {
                                x: (m.origin.x + event.center.x) * scale,
                                y: (m.origin.y + event.center.y) * scale,
                            };
                            assert_eq!(
                                score_tick(&score, &layout, point, event.voice),
                                Some(m.start_tick + u64::from(event.tick))
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn score_whitespace_clicks_snap_to_local_meter_without_requiring_notes() {
        let mut score = Score::default();
        score
            .set_time_signature_at(
                1,
                score_core::TimeSignature {
                    numerator: 6,
                    denominator: 8,
                },
            )
            .unwrap();
        let mut layout = score_layout::LayoutEngine::default().layout(&score, 1000.);
        for scale in [0.5, 1., 2.] {
            layout.scale = scale;
            for m in &layout.measures {
                let time = score.time_at(m.index);
                let beat = score_core::TICKS_PER_WHOLE / u16::from(time.denominator);
                for b in 0..time.numerator {
                    let tick = u16::from(b) * beat;
                    let point = score_layout::Point {
                        x: (m.origin.x + m.content.x_at_tick(f64::from(tick))) * scale,
                        y: (m.origin.y + 40.) * scale,
                    };
                    assert_eq!(
                        score_tick(&score, &layout, point, score_core::Voice::One),
                        Some(m.start_tick + u64::from(tick))
                    );
                }
            }
            assert_eq!(
                score_tick(
                    &score,
                    &layout,
                    score_layout::Point { x: -100., y: -100. },
                    score_core::Voice::One
                ),
                None
            );
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
    fn mixed_meters_use_local_beats_and_can_seek_to_a_short_final_bar() {
        use score_core::TimeSignature;
        let mut score = Score::default();
        score
            .set_time_signature_at(
                3,
                TimeSignature {
                    numerator: 1,
                    denominator: 16,
                },
            )
            .unwrap();
        assert_eq!(score.total_ticks(), 588);
        // A click near the end of bar 3 rounds forward to bar 4, not backward
        // by a quarter-note beat inherited from the clicked bar.
        let x = 8. + 384. * 560. / 588.;
        assert_eq!(ruler_tick(&score, x, 400.), 576);
        assert_eq!(ruler_tick(&score, 400., 400.), 576);

        score
            .set_time_signature_at(
                1,
                TimeSignature {
                    numerator: 6,
                    denominator: 8,
                },
            )
            .unwrap();
        let timeline = Timeline::new(&score, score_playback::Tempo::default());
        let position = position_at_tick(&score, 360., &timeline);
        assert_eq!((position.measure, position.beat), (2, 2.));
        assert_eq!(position.seconds, 3.75);
        assert_eq!(score.total_ticks(), 492);
        let x = 8. + 384. * 360. / 492.;
        assert_eq!(ruler_tick(&score, x, 400.), 360);
    }

    #[test]
    fn audio_clock_is_absolute_and_paused_position_does_not_follow_selection() {
        let mut score = Score::default();
        score
            .insert(2, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        let cursor = Cursor {
            voice: Default::default(),
            part: score.parts()[0].id(),
            staff: StaffId::Upper,
            measure: 2,
            index: 1,
        };
        let timeline = Timeline::new(&score, score_playback::Tempo::default());
        let tempo = &timeline;
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
        // A fermata on the first beat holds the clock: 2.5 s is now one beat earlier.
        let first = score
            .insert(0, 0, note(Step::C, Duration::Quarter))
            .unwrap();
        score.set_fermata(first, true).unwrap();
        let held = Timeline::new(&score, score_playback::Tempo::default());
        let p = position(&score, cursor, clock, &held);
        assert_eq!(p.tick, 192.);
        assert_eq!(p.seconds, 2.5);
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
            voice: Default::default(),
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
                let marker = score_bounds(&layout, cursor, p).unwrap();
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
            let marker = score_bounds(&layout, cursor, p).unwrap();
            assert!(
                (f32::from(marker.left()) - bar.origin.x - bar.content.x_at_tick(0.)).abs() < 0.01
            );
            let tracking = tracking_bounds(&layout, cursor, p).unwrap();
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
        let marker = score_bounds(&layout, cursor, end).unwrap();
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
        let p = position(
            editor.score(),
            editor.cursor(),
            None,
            &Timeline::plain(score_playback::Tempo::default()),
        );
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
    #[test]
    fn repeat_clock_returns_to_the_written_bar_and_elapsed_seconds_keep_growing() {
        let mut score = Score::new("Repeat".into(), &[score_core::Instrument::Piano], 2).unwrap();
        score
            .set_bar_form(
                1,
                score_core::BarForm {
                    repeat_end: Some(2),
                    ..Default::default()
                },
            )
            .unwrap();
        let timeline = Timeline::with_repeats(&score, score_playback::Tempo::default(), true);
        let frame = timeline.performed_frame(2, 0, 48_000);
        let current = position(&score, Cursor::default(), Some((frame, 48_000)), &timeline);
        assert_eq!((current.tick, current.measure), (0., 0));
        assert_eq!(current.seconds, frame as f64 / 48_000.);
        assert!(current.seconds > 0.);
        assert!(!current.at_end);
        let end = position(
            &score,
            Cursor::default(),
            Some((timeline.total_frames(48_000), 48_000)),
            &timeline,
        );
        assert!(end.at_end);
    }
}
