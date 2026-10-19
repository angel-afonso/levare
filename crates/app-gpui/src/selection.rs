//! Pointer selection uses the existing continuous, single-staff passage model.
use super::*;
use score_layout::{Layout, Point as ScorePoint};

pub(super) struct PassageDrag {
    anchor: ElementId,
    start: Point<Pixels>,
    active: bool,
    select_anchor: bool,
}

impl PassageDrag {
    fn target(
        &mut self,
        layout: &Layout,
        position: Point<Pixels>,
        origin: Point<Pixels>,
    ) -> Option<ElementId> {
        let delta = position - self.start;
        if !self.active && delta.x.abs().max(delta.y.abs()) < px(5.) {
            return None;
        }
        self.active = true;
        nearest_event(layout, self.anchor, canvas_point(position, origin), false)
    }
    fn update(
        &mut self,
        editor: &mut score_edit::Editor,
        layout: &Layout,
        position: Point<Pixels>,
        origin: Point<Pixels>,
    ) -> bool {
        let Some(target) = self.target(layout, position, origin) else {
            return false;
        };
        let mut changed = self.select_anchor;
        if self.select_anchor {
            editor.select(self.anchor);
            self.select_anchor = false;
        }
        if editor.selection() != Some(target) {
            editor.extend_selection(target);
            changed = true;
        }
        changed
    }
}

fn canvas_point(position: Point<Pixels>, origin: Point<Pixels>) -> ScorePoint {
    let p = position - origin;
    ScorePoint {
        x: p.x.into(),
        y: p.y.into(),
    }
}

/// Prefer the nearest row of the originating staff, then the nearest time on
/// that row. Pitch height and other instruments cannot divert the passage.
fn nearest_event(
    layout: &Layout,
    anchor: ElementId,
    point: ScorePoint,
    same_measure: bool,
) -> Option<ElementId> {
    let point = ScorePoint {
        x: point.x / layout.scale,
        y: point.y / layout.scale,
    };
    let measures: Vec<_> = layout
        .measures
        .iter()
        .filter(|m| {
            m.part == anchor.part
                && m.staff == anchor.staff
                && (!same_measure || m.index == anchor.measure)
        })
        .collect();
    if let Some((id, _)) = measures
        .iter()
        .flat_map(|m| {
            m.content.events.iter().filter_map(move |event| {
                let dx = point.x - m.origin.x - event.center.x;
                let dy = point.y - m.origin.y - event.center.y;
                (dx.abs() <= 15. && dy.abs() <= 18.).then_some((event.id, dx * dx + dy * dy))
            })
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
    {
        return Some(id);
    }
    // Ledger lines can place a note closer to another row's staff center.
    // Use the complete row's ink envelope when the pointer is between notes.
    let mut rows: HashMap<u32, (f32, f32)> = HashMap::new();
    for m in &measures {
        let top = m.origin.y + m.content.ink_top.min(0.);
        let bottom = m.origin.y + m.content.ink_bottom.max(40.);
        rows.entry(m.origin.y.to_bits())
            .and_modify(|bounds| {
                bounds.0 = bounds.0.min(top);
                bounds.1 = bounds.1.max(bottom);
            })
            .or_insert((top, bottom));
    }
    measures
        .iter()
        .flat_map(|m| {
            let (top, bottom) = rows[&m.origin.y.to_bits()];
            m.content.events.iter().map(move |event| {
                let row = (top - point.y).max(point.y - bottom).max(0.);
                let center = (point.y - m.origin.y - 20.).abs();
                let time = (point.x - m.origin.x - event.center.x).abs();
                (event.id, row, center, time)
            })
        })
        .min_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| a.2.total_cmp(&b.2))
                .then_with(|| a.3.total_cmp(&b.3))
        })
        .map(|(id, _, _, _)| id)
}

impl AppView {
    pub(super) fn begin_passage_selection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        self.selection_drag = None;
        if self.busy || self.confirming {
            return;
        }
        self.reveal_selection = false;
        let point = canvas_point(event.position, self.canvas_origin);
        self.clicked_mark = None;
        let hit = self.layout.hit(point);
        if let Some(score_layout::Hit::Mark(mark)) = hit.filter(|_| !event.modifiers.shift) {
            let anchor = mark.anchor();
            self.document.editor.select(anchor);
            self.clicked_mark = Some(mark);
            self.playback.follow_staff(anchor.part, anchor.staff);
            let label = self
                .document
                .editor
                .score()
                .mark_label(mark)
                .unwrap_or_default();
            self.status = format!(
                "{label} seleccionado · Backspace lo borra; pulsa otra marca de la cinta para cambiarlo"
            );
            cx.notify();
            return;
        }
        let note = match hit {
            Some(score_layout::Hit::Note(id, pitch)) => Some((id, pitch)),
            _ => self.layout.hit_pitch(point),
        };
        if let Some((id, pitch)) = note {
            if event.modifiers.shift {
                self.document.editor.extend_selection(id);
            } else {
                self.document.editor.select_pitch(id, pitch);
            }
            self.playback.follow_staff(id.part, id.staff);
            self.selection_drag = Some(PassageDrag {
                anchor: id,
                start: event.position,
                active: false,
                select_anchor: false,
            });
        } else if let Some((part, staff, measure)) = self.layout.measure_at(point) {
            let bar = &self
                .document
                .editor
                .score()
                .part(part)
                .unwrap()
                .staff(staff)
                .unwrap()
                .measures()[measure];
            let index = bar.len();
            let anchor = bar
                .id_at(0)
                .map(|event| ElementId {
                    part,
                    staff,
                    measure,
                    event,
                })
                .and_then(|id| nearest_event(&self.layout, id, point, true));
            if let Some(id) = anchor.filter(|_| event.modifiers.shift) {
                self.document.editor.extend_selection(id);
            } else {
                self.document.editor.set_cursor(Cursor {
                    part,
                    staff,
                    measure,
                    index,
                });
            }
            self.playback.follow_staff(part, staff);
            self.selection_drag = anchor.map(|anchor| PassageDrag {
                anchor,
                start: event.position,
                active: false,
                select_anchor: !event.modifiers.shift,
            });
        }
        self.selection_status();
        cx.notify();
    }

    fn selection_status(&mut self) {
        let count = self.document.editor.selected_ids().len();
        self.status = if count > 1 {
            format!(
                "{count} elementos seleccionados · Copiar o cortar para llevar el pasaje a otro lugar"
            )
        } else {
            "Arrastra sobre las notas para seleccionar un pasaje; Shift + clic amplía la selección"
                .into()
        };
    }

    fn extend_passage_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.selection_drag.as_mut() else {
            return;
        };
        if self.busy || self.confirming || self.document.editor.score().event(drag.anchor).is_none()
        {
            self.selection_drag = None;
            return;
        }
        if drag.update(
            &mut self.document.editor,
            &self.layout,
            position,
            self.canvas_origin,
        ) {
            self.selection_status();
            cx.notify();
        }
    }

    pub(super) fn move_passage_selection(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.dragging() {
            self.extend_passage_drag(event.position, cx);
        } else {
            self.selection_drag = None;
        }
    }

    pub(super) fn end_passage_selection(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.extend_passage_drag(event.position, cx);
        self.selection_drag = None;
    }
}

/// One continuous band per system, including empty bars between selected notes.
pub(super) fn passage_bounds(layout: &Layout, ids: &[ElementId]) -> Vec<Bounds<Pixels>> {
    let (Some(first), Some(last)) = (ids.first(), ids.last()) else {
        return Vec::new();
    };
    if ids.len() < 2 {
        return Vec::new();
    }
    let mut measures: Vec<_> = layout
        .measures
        .iter()
        .filter(|m| {
            m.part == first.part
                && m.staff == first.staff
                && m.index >= first.measure
                && m.index <= last.measure
        })
        .collect();
    measures.sort_by(|a, b| {
        a.origin
            .y
            .total_cmp(&b.origin.y)
            .then_with(|| a.origin.x.total_cmp(&b.origin.x))
    });
    let mut bands: Vec<Bounds<Pixels>> = Vec::new();
    let mut row = None;
    for m in measures {
        let left = if m.index == first.measure {
            m.content
                .events
                .iter()
                .filter(|e| e.id == *first)
                .map(|e| e.center.x - 16.)
                .min_by(f32::total_cmp)
                .unwrap_or(0.)
        } else {
            0.
        };
        let right = if m.index == last.measure {
            m.content
                .events
                .iter()
                .filter(|e| e.id == *last)
                .map(|e| e.center.x + 24.)
                .max_by(f32::total_cmp)
                .unwrap_or(m.width)
        } else {
            m.width
        };
        let top = (-12f32).min(m.content.ink_top - 6.);
        let bottom = 52f32.max(m.content.ink_bottom + 6.);
        let bounds = Bounds::new(
            point(
                px((m.origin.x + left) * layout.scale),
                px((m.origin.y + top) * layout.scale),
            ),
            size(
                px((right - left).max(1.) * layout.scale),
                px((bottom - top) * layout.scale),
            ),
        );
        if let Some(previous) = bands.last_mut().filter(|previous| {
            row == Some(m.origin.y)
                && (previous.right() - bounds.left()).abs() < px(1.)
                && previous.top() <= bounds.bottom()
                && previous.bottom() >= bounds.top()
        }) {
            *previous = previous.union(&bounds);
        } else {
            bands.push(bounds);
        }
        row = Some(m.origin.y);
    }
    bands
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Notation, Pitch, Pitches, Score, Step};

    fn fixture() -> (Score, Vec<ElementId>) {
        let mut score = Score::default();
        let mut ids = Vec::new();
        for measure in [0, 1, 3] {
            for (index, step) in [Step::C, Step::E, Step::G].into_iter().enumerate() {
                ids.push(
                    score
                        .insert(
                            measure,
                            index,
                            Event {
                                pitches: if index == 1 {
                                    Pitches::default()
                                } else {
                                    Pitches::single(Pitch::new(step, 4))
                                },
                                duration: Duration::Quarter,
                                notation: Notation::default(),
                            },
                        )
                        .unwrap(),
                );
            }
        }
        let mut chord = *score.event(ids[0]).unwrap();
        chord.pitches = chord.pitches.with(Pitch::new(Step::G, 4).unwrap()).unwrap();
        score
            .insert_on(ids[0].part, StaffId::Lower, 0, 0, chord)
            .unwrap();
        (score, ids)
    }

    fn position(layout: &Layout, id: ElementId) -> Point<Pixels> {
        let m = layout.measure_on(id.part, id.staff, id.measure).unwrap();
        let event = m
            .content
            .events
            .iter()
            .find(|event| event.id == id)
            .unwrap();
        point(
            px((m.origin.x + event.center.x) * layout.scale),
            px((m.origin.y + 20.) * layout.scale),
        )
    }

    #[::core::prelude::v1::test]
    fn drag_selects_whole_passages_in_both_directions_at_every_zoom_and_copies_atomically() {
        for scale in [0.5, 0.7, 1., 1.5] {
            let (score, ids) = fixture();
            let mut layout = score_layout::LayoutEngine::default().layout(&score, 760.);
            layout.scale = scale;
            for reverse in [false, true] {
                let mut editor = score_edit::Editor::new(score.clone()).unwrap();
                let (a, b) = if reverse {
                    (ids[8], ids[0])
                } else {
                    (ids[0], ids[8])
                };
                editor.select(a);
                let origin = point(px(-120.), px(240.));
                let start = position(&layout, a) + origin;
                let mut drag = PassageDrag {
                    anchor: a,
                    start,
                    active: false,
                    select_anchor: false,
                };
                assert!(!drag.update(&mut editor, &layout, start + point(px(2.), px(1.)), origin));
                assert_eq!(editor.selected_ids(), vec![a]);
                assert!(drag.update(&mut editor, &layout, position(&layout, b) + origin, origin));
                assert_eq!(editor.selected_ids(), ids);
                assert_eq!(editor.score(), &score);
                assert!(!editor.can_undo());
                let clip = editor.copy_selection().unwrap();
                assert_eq!(clip.events.len(), ids.len());
                assert!(clip.events.iter().any(|e| e.event.pitches.is_empty()));
                let mut destination = score_edit::Editor::default();
                assert!(destination.paste(&clip).unwrap() >= ids.len());
                assert!(destination.undo());
                assert!(
                    destination
                        .score()
                        .measures()
                        .iter()
                        .all(|bar| bar.is_empty())
                );
            }
        }
    }

    #[::core::prelude::v1::test]
    fn whitespace_drag_initializes_anchor_and_stays_on_the_original_staff() {
        let (score, ids) = fixture();
        let layout = score_layout::LayoutEngine::default().layout(&score, 960.);
        let mut editor = score_edit::Editor::new(score).unwrap();
        let start = position(&layout, ids[0]) - point(px(10.), px(10.));
        let mut drag = PassageDrag {
            anchor: ids[0],
            start,
            active: false,
            select_anchor: true,
        };
        let target = position(&layout, ids[2]) + point(px(0.), px(100.));
        assert!(drag.update(&mut editor, &layout, target, Point::default()));
        assert_eq!(editor.selected_ids(), ids[..3]);
        // Pulling back over the anchor shrinks the existing range.
        assert!(drag.update(
            &mut editor,
            &layout,
            position(&layout, ids[0]),
            Point::default()
        ));
        assert_eq!(editor.selected_ids(), vec![ids[0]]);
    }

    #[::core::prelude::v1::test]
    fn ledger_notes_keep_their_system_when_the_staff_center_is_far_away() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        score.add_part(Instrument::Violin).unwrap();
        score.remove_part(piano).unwrap();
        let mut ids = Vec::new();
        for measure in 0..4 {
            ids.push(
                score
                    .insert(
                        measure,
                        0,
                        Event {
                            pitches: Pitches::single(Pitch::new(
                                Step::C,
                                if measure == 3 { 8 } else { 4 },
                            )),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        for scale in [0.5, 0.7, 1., 1.5] {
            let mut layout = score_layout::LayoutEngine::default().layout_compact(&score, 500.);
            layout.scale = scale;
            let measure = layout.measure_on(ids[3].part, ids[3].staff, 3).unwrap();
            let head = &measure.content.events[0];
            let point = ScorePoint {
                x: (measure.origin.x + head.center.x + 6.) * scale,
                y: (measure.origin.y + head.center.y) * scale,
            };
            assert_eq!(nearest_event(&layout, ids[3], point, false), Some(ids[3]));
            // Also keep the row when the pointer has left the note's hit box.
            assert_eq!(
                nearest_event(
                    &layout,
                    ids[3],
                    ScorePoint {
                        x: point.x + 30. * scale,
                        ..point
                    },
                    false
                ),
                Some(ids[3])
            );
        }
    }

    #[::core::prelude::v1::test]
    fn passage_bands_cover_empty_bars_and_split_at_system_breaks() {
        let (score, ids) = fixture();
        for width in [600., 1800.] {
            let layout = score_layout::LayoutEngine::default().layout(&score, width);
            let bands = passage_bounds(&layout, &ids);
            assert!(!bands.is_empty());
            assert!(passage_bounds(&layout, &ids[..1]).is_empty());
            for index in 0..=3 {
                let measure = layout.measure_on(ids[0].part, ids[0].staff, index).unwrap();
                let p = if index == 2 {
                    point(
                        px((measure.origin.x + measure.width / 2.) * layout.scale),
                        px((measure.origin.y + 20.) * layout.scale),
                    )
                } else {
                    position(
                        &layout,
                        ids.iter().copied().find(|id| id.measure == index).unwrap(),
                    )
                };
                assert!(
                    bands.iter().any(|band| band.contains(&p)),
                    "measure {index} at width {width}"
                );
            }
            let rows: std::collections::HashSet<_> = layout
                .measures
                .iter()
                .filter(|m| m.part == ids[0].part && m.staff == ids[0].staff)
                .map(|m| m.origin.y.to_bits())
                .collect();
            assert_eq!(bands.len(), rows.len());
        }
    }
}
