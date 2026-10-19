//! Playback scrolling stays on the UI thread and uses the painted canvas geometry.
use gpui::{Bounds, Pixels, Point, ScrollHandle, Size, point, px, size};
use score_core::ElementId;
use score_layout::Layout;

#[derive(Default)]
pub struct PlaybackViewport {
    pub scroll: ScrollHandle,
    canvas_origin: Point<Pixels>,
    painted_offset: Point<Pixels>,
}

impl PlaybackViewport {
    pub fn record_canvas(&mut self, origin: Point<Pixels>) {
        self.canvas_origin = origin;
        self.painted_offset = self.scroll.offset();
    }

    pub fn follow(
        &self,
        layout: &Layout,
        element: Option<ElementId>,
        tracking: Option<Bounds<Pixels>>,
        playing: bool,
    ) {
        if !playing {
            return;
        }
        let Some(mut target) = element
            .and_then(|id| follow_bounds(layout, id, self.scroll.bounds().size.height))
            .or(tracking)
        else {
            return;
        };
        if let Some(tracking) = tracking {
            target.origin.x = tracking.origin.x;
            target.size.width = tracking.size.width;
        }
        self.scroll.set_offset(self.offset_to_reveal(
            target,
            self.scroll.bounds(),
            self.scroll.max_offset(),
        ));
    }

    pub fn reveal_cursor(&self, layout: &Layout, cursor: score_edit::Cursor) {
        if let Some(position) =
            layout.cursor(cursor.part, cursor.staff, cursor.measure, cursor.index)
        {
            let target = Bounds::new(
                point(px(position.x - 20.), px(position.y - 16.)),
                size(px(40.), px(88.)),
            );
            self.scroll.set_offset(self.offset_to_reveal(
                target,
                self.scroll.bounds(),
                self.scroll.max_offset(),
            ));
        }
    }

    pub fn reveal_selection(
        &self,
        layout: &Layout,
        selection: Option<ElementId>,
        cursor: score_edit::Cursor,
    ) {
        if let Some(target) = selection.and_then(|id| target_bounds(layout, id)) {
            self.scroll.set_offset(self.offset_to_reveal(
                target,
                self.scroll.bounds(),
                self.scroll.max_offset(),
            ));
        } else {
            self.reveal_cursor(layout, cursor);
        }
    }

    fn offset_to_reveal(
        &self,
        target: Bounds<Pixels>,
        viewport: Bounds<Pixels>,
        maximum: Size<Pixels>,
    ) -> Point<Pixels> {
        let offset = self.scroll.offset();
        // Offset may change between paints (manual scroll or another clock tick).
        // Correct the last painted origin so repeated updates cannot overshoot.
        let origin = self.canvas_origin + offset - self.painted_offset;
        let target = Bounds::new(origin + target.origin, target.size);
        reveal_offset(target, viewport, offset, maximum)
    }
}

fn follow_bounds(layout: &Layout, id: ElementId, height: Pixels) -> Option<Bounds<Pixels>> {
    let note = target_bounds(layout, id)?;
    for whole_system in [true, false] {
        let mut top = note.top();
        let mut bottom = note.bottom();
        for measure in layout
            .measures
            .iter()
            .filter(|placed| placed.index == id.measure && (whole_system || placed.part == id.part))
        {
            top = top.min(px((measure.origin.y + measure.content.top) * layout.scale));
            bottom = bottom.max(px(
                (measure.origin.y + measure.content.bottom) * layout.scale
            ));
        }
        if bottom - top <= height - px(48.) {
            return Some(Bounds::new(
                point(note.left(), top),
                size(note.size.width, bottom - top),
            ));
        }
    }
    Some(note)
}

fn target_bounds(layout: &Layout, id: ElementId) -> Option<Bounds<Pixels>> {
    let measure = layout.measure_on(id.part, id.staff, id.measure)?;
    let mut heads = measure.content.events.iter().filter(|event| event.id == id);
    let first = heads.next()?;
    let (mut left, mut right, mut top, mut bottom) = (
        first.center.x - 24.,
        first.center.x + 24.,
        first.center.y - 44.,
        first.center.y + 44.,
    );
    for head in heads {
        left = left.min(head.center.x - 24.);
        right = right.max(head.center.x + 24.);
        top = top.min(head.center.y - 44.);
        bottom = bottom.max(head.center.y + 44.);
    }
    for glyph in measure
        .content
        .glyphs
        .iter()
        .filter(|glyph| glyph.element == Some(id))
    {
        left = left.min(glyph.position.x - 4.);
        right = right.max(glyph.position.x + 20.);
        top = top.min(glyph.position.y - 30.);
        bottom = bottom.max(glyph.position.y + 30.);
    }
    if let Some(stem) = measure.content.beam_stem(id) {
        top = top.min(stem.to.y - 6.);
        bottom = bottom.max(stem.to.y + 6.);
    }
    for group in measure
        .content
        .tuplets
        .iter()
        .filter(|g| g.elements.contains(&id))
    {
        top = top.min(group.top - 4.);
    }
    for hairpin in layout.hairpins.iter().filter(|h| {
        h.staff_origin == measure.origin.y
            && h.connection.start.part == id.part
            && h.connection.start.staff == id.staff
            && (h.connection.start.measure..=h.connection.end.measure).contains(&id.measure)
    }) {
        top = top.min(hairpin.upper.from.y.min(hairpin.upper.to.y) - measure.origin.y - 4.);
        bottom = bottom.max(hairpin.lower.from.y.max(hairpin.lower.to.y) - measure.origin.y + 4.);
    }
    Some(Bounds::new(
        point(
            px((measure.origin.x + left) * layout.scale),
            px((measure.origin.y + top) * layout.scale),
        ),
        size(
            px((right - left) * layout.scale),
            px((bottom - top) * layout.scale),
        ),
    ))
}

fn reveal_offset(
    target: Bounds<Pixels>,
    viewport: Bounds<Pixels>,
    offset: Point<Pixels>,
    maximum: Size<Pixels>,
) -> Point<Pixels> {
    if viewport.size.width <= px(0.) || viewport.size.height <= px(0.) {
        return offset;
    }
    point(
        (offset.x
            + axis_delta(
                target.left(),
                target.size.width,
                viewport.left(),
                viewport.size.width,
            ))
        .clamp(-maximum.width, px(0.)),
        (offset.y
            + axis_delta(
                target.top(),
                target.size.height,
                viewport.top(),
                viewport.size.height,
            ))
        .clamp(-maximum.height, px(0.)),
    )
}

fn axis_delta(
    start: Pixels,
    length: Pixels,
    visible_start: Pixels,
    visible_length: Pixels,
) -> Pixels {
    let margin = px(24.).min(visible_length / 8.);
    let low = visible_start + margin;
    let high = visible_start + visible_length - margin;
    if length > high - low {
        // Tiny viewports cannot show the whole glyph: keep its center visible.
        visible_start + visible_length / 2. - start - length / 2.
    } else if start < low {
        low - start
    } else if start + length > high {
        high - start - length
    } else {
        px(0.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Pitch, Score, Step};

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    #[test]
    fn zoom_scales_follow_bounds_with_the_painted_chord() {
        use score_core::{Duration, Event, Pitch, Score, Step};
        let mut score = Score::default();
        let id = score
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
        let mut layout = score_layout::LayoutEngine::default().layout_compact(&score, 960.);
        let original = target_bounds(&layout, id).unwrap();
        for scale in [0.5, 0.75, 1.5] {
            layout.scale = scale;
            let target = target_bounds(&layout, id).unwrap();
            assert!((f32::from(target.left()) - f32::from(original.left()) * scale).abs() < 0.001);
            assert!((f32::from(target.top()) - f32::from(original.top()) * scale).abs() < 0.001);
            assert!(
                (f32::from(target.size.width) - f32::from(original.size.width) * scale).abs()
                    < 0.001
            );
            assert!(
                (f32::from(target.size.height) - f32::from(original.size.height) * scale).abs()
                    < 0.001
            );
        }
    }

    #[test]
    fn follow_bounds_include_all_accidental_columns_in_dense_chords() {
        use score_core::{Accidental, Duration, Event, Pitch, Pitches, Score, Step};
        let pitches = Pitches::new(
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
        let id = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches,
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let layout = score_layout::LayoutEngine::default().layout(&score, 500.);
        let target = target_bounds(&layout, id).unwrap();
        let bar = layout.measure_on(id.part, id.staff, id.measure).unwrap();
        for glyph in bar.content.glyphs.iter().filter(|g| g.element == Some(id)) {
            assert!(target.left() <= px(bar.origin.x + glyph.position.x));
            assert!(target.right() >= px(bar.origin.x + glyph.position.x + 14.));
            assert!(target.top() <= px(bar.origin.y + glyph.position.y - 25.));
        }
    }

    #[test]
    fn chord_follow_includes_every_head_and_its_stem_clearance() {
        let mut score = Score::default();
        let pitches = score_core::Pitches::new(&[
            Pitch::new(Step::C, 3).unwrap(),
            Pitch::new(Step::D, 3).unwrap(),
            Pitch::new(Step::C, 6).unwrap(),
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
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let target = target_bounds(&layout, id).unwrap();
        let measure = layout.measure_on(id.part, id.staff, 0).unwrap();
        assert!(target.size.height > px(88.));
        for head in &measure.content.events {
            assert!(target.left() <= px(measure.origin.x + head.center.x - 24.));
            assert!(target.right() >= px(measure.origin.x + head.center.x + 24.));
            assert!(target.top() <= px(measure.origin.y + head.center.y - 44.));
            assert!(target.bottom() >= px(measure.origin.y + head.center.y + 44.));
        }
    }

    #[test]
    fn tall_scores_follow_both_piano_hands_when_the_full_ensemble_does_not_fit() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        score
            .insert_on(
                piano,
                score_core::StaffId::Upper,
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
                score_core::StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        score.add_part(score_core::Instrument::Cello).unwrap();
        score.add_part(score_core::Instrument::Flute).unwrap();
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let upper = layout
            .measure_on(piano, score_core::StaffId::Upper, 0)
            .unwrap();
        let lower = layout
            .measure_on(piano, score_core::StaffId::Lower, 0)
            .unwrap();
        let target = follow_bounds(&layout, left, px(430.)).unwrap();
        assert!(target.top() <= px(upper.origin.y + upper.content.top));
        assert!(target.bottom() >= px(lower.origin.y + lower.content.bottom));
        assert!(target.size.height <= px(430. - 48.));
        assert_eq!(
            target_bounds(&layout, left).unwrap().origin.y,
            px(lower.origin.y + lower.content.events[0].center.y - 44.)
        );
    }

    #[test]
    fn ensemble_follow_finds_the_correct_staff_and_shows_the_group_when_it_fits() {
        let mut score = Score::default();
        score
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
        let id = score
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
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let note = target_bounds(&layout, id).unwrap();
        let staff = layout.measure(cello, 0).unwrap();
        assert!(note.top() >= px(staff.origin.y - 44.));
        let group = follow_bounds(&layout, id, px(900.)).unwrap();
        assert!(group.top() < note.top());
        assert!(group.bottom() >= note.bottom());
        assert_eq!(follow_bounds(&layout, id, px(120.)).unwrap(), note);
    }

    #[test]
    fn follows_later_rows_and_returns_to_start_without_repeated_drift() {
        let mut score = Score::default();
        for _ in 0..12 {
            score.append_measure().unwrap();
        }
        let mut ids = Vec::new();
        for measure in [0, 8, 15] {
            ids.push(
                score
                    .insert(
                        measure,
                        0,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Quarter,
                        },
                    )
                    .unwrap(),
            );
        }
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let viewport = rect(0., 180., 800., 320.);
        // Includes the page title and padding above the canvas.
        let canvas = point(px(24.), px(290.));
        let maximum = size(px(208.), px(layout.height));
        let mut offset = Point::default();
        let mut furthest = px(0.);
        for id in [ids[0], ids[1], ids[2], ids[0]] {
            let local = target_bounds(&layout, id).unwrap();
            let before = Bounds::new(canvas + offset + local.origin, local.size);
            let next = reveal_offset(before, viewport, offset, maximum);
            let after = Bounds::new(canvas + next + local.origin, local.size);
            assert!(after.top() >= viewport.top() + px(24.));
            assert!(after.bottom() <= viewport.bottom() - px(24.));
            assert_eq!(reveal_offset(after, viewport, next, maximum), next);
            offset = next;
            furthest = furthest.min(offset.y);
        }
        assert!(offset.y > furthest);
    }

    #[test]
    fn follows_dense_horizontal_measures_extreme_pitches_and_rests() {
        let mut score = Score::default();
        let mut last = None;
        for index in 0..16 {
            last = Some(
                score
                    .insert(
                        0,
                        index,
                        Event {
                            notation: Default::default(),
                            pitches: score_core::Pitches::single(if index == 15 {
                                None
                            } else {
                                Pitch::new(Step::C, 8)
                            }),
                            duration: Duration::Sixteenth,
                        },
                    )
                    .unwrap(),
            );
        }
        let layout = score_layout::LayoutEngine::default().layout(&score, 360.);
        let viewport = rect(0., 0., 300., 160.);
        let target = target_bounds(&layout, last.unwrap()).unwrap();
        let offset = reveal_offset(
            target,
            viewport,
            Point::default(),
            size(px(layout.width), px(layout.height)),
        );
        let visible = Bounds::new(target.origin + offset, target.size);
        assert!(offset.x < px(0.));
        assert!(visible.right() <= viewport.right() - px(24.));
        assert!(visible.top() >= viewport.top());
        assert!(visible.bottom() <= viewport.bottom());
        let high = target_bounds(&layout, layout.measures[0].content.events[0].id).unwrap();
        assert!(high.top() < target.top());
        let high_offset = reveal_offset(
            Bounds::new(high.origin + offset, high.size),
            viewport,
            offset,
            size(px(layout.width), px(layout.height)),
        );
        assert!(high.top() + high_offset.y >= viewport.top());
        assert!(high.bottom() + high_offset.y <= viewport.bottom());
    }

    #[test]
    fn paused_stopped_and_silent_gaps_preserve_manual_scrolling() {
        let mut viewport = PlaybackViewport::default();
        viewport.record_canvas(point(px(24.), px(-500.)));
        let manual = point(px(-100.), px(-700.));
        viewport.scroll.set_offset(manual);
        let mut score = Score::default();
        let id = score
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
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        viewport.follow(&layout, Some(id), None, false);
        assert_eq!(viewport.scroll.offset(), manual);
        viewport.follow(&layout, None, None, true);
        assert_eq!(viewport.scroll.offset(), manual);
    }

    #[test]
    fn repeated_ticks_before_paint_and_manual_scroll_do_not_accumulate_offsets() {
        let mut view = PlaybackViewport::default();
        view.scroll.set_offset(point(px(0.), px(-200.)));
        view.record_canvas(point(px(24.), px(100.)));
        let viewport = rect(0., 180., 800., 320.);
        let target = rect(400., 900., 48., 88.);
        let maximum = size(px(208.), px(2000.));
        let first = view.offset_to_reveal(target, viewport, maximum);
        assert!(first.y < px(-200.));
        view.scroll.set_offset(first);
        // No new canvas paint has happened yet.
        assert_eq!(view.offset_to_reveal(target, viewport, maximum), first);
        // Manually scrolling away between paints must reveal the same note again.
        view.scroll.set_offset(point(px(0.), px(-50.)));
        assert_eq!(view.offset_to_reveal(target, viewport, maximum), first);
    }

    #[test]
    fn clamps_to_content_edges_and_centers_in_a_tiny_viewport() {
        let viewport = rect(10., 20., 40., 40.);
        let offset = reveal_offset(
            rect(100., 120., 48., 88.),
            viewport,
            Point::default(),
            size(px(200.), px(200.)),
        );
        assert_eq!(offset, point(px(-94.), px(-124.)));
        assert_eq!(
            reveal_offset(
                rect(-500., -500., 48., 88.),
                viewport,
                offset,
                size(px(200.), px(200.))
            ),
            Point::default()
        );
        assert_eq!(
            reveal_offset(
                rect(900., 900., 48., 88.),
                viewport,
                offset,
                size(px(200.), px(200.))
            ),
            point(px(-200.), px(-200.))
        );
        assert_eq!(
            reveal_offset(
                rect(900., 900., 48., 88.),
                rect(0., 0., 0., 0.),
                offset,
                size(px(200.), px(200.))
            ),
            offset
        );
    }
}

#[cfg(test)]
mod hairpin_tests {
    use super::*;
    use score_core::{
        Connection, ConnectionKind, Duration, Dynamic, Event, Pitch, Pitches, Score, Step,
    };
    #[test]
    fn selected_and_playing_notes_reveal_their_hairpin_at_any_zoom() {
        let mut s = Score::default();
        let event = Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Half,
            notation: Default::default(),
        };
        let a = s.insert(0, 0, event).unwrap();
        let b = s.insert(0, 1, event).unwrap();
        s.set_dynamic(a, Some(Dynamic::P)).unwrap();
        s.add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
        let mut layout = score_layout::LayoutEngine::default().layout_compact(&s, 960.);
        for scale in [0.5, 1., 2.] {
            layout.scale = scale;
            let target = target_bounds(&layout, a).unwrap();
            let h = &layout.hairpins[0];
            assert!(target.bottom() >= px(h.lower.to.y.max(h.lower.from.y) * scale));
        }
    }
}

#[cfg(test)]
mod beam_tests {
    use super::*;
    use score_core::{Duration, Event, Pitch, Pitches, Score, Step};
    #[test]
    fn following_a_beamed_note_includes_the_shared_stem_tip_at_every_zoom() {
        let mut s = Score::default();
        let a = s
            .insert(
                0,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Eighth,
                    notation: Default::default(),
                },
            )
            .unwrap();
        let b = s
            .insert(
                0,
                1,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 6)),
                    duration: Duration::Eighth,
                    notation: Default::default(),
                },
            )
            .unwrap();
        let mut layout = score_layout::LayoutEngine::default().layout_compact(&s, 960.);
        for scale in [0.5, 1., 2.] {
            layout.scale = scale;
            for id in [a, b] {
                let bounds = target_bounds(&layout, id).unwrap();
                let m = layout.measure_on(id.part, id.staff, id.measure).unwrap();
                let stem = m.content.beam_stem(id).unwrap();
                let tip = px((m.origin.y + stem.to.y) * scale);
                assert!(bounds.top() <= tip && bounds.bottom() >= tip);
            }
        }
    }
}
