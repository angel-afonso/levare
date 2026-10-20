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
    let voice = layout
        .measures
        .iter()
        .flat_map(|m| &m.content.events)
        .find(|e| e.id == anchor)?
        .voice;
    let point = ScorePoint {
        x: point.x / layout.scale,
        y: point.y / layout.scale,
    };
    let measures: Vec<_> = layout
        .measures
        .iter()
        .filter(|m| {
            m.part == anchor.part
                && m.source_staff == anchor.staff
                && m.source_voice.is_none_or(|v| v == voice)
                && (!same_measure || m.index == anchor.measure)
        })
        .collect();
    if let Some((id, _)) = measures
        .iter()
        .flat_map(|m| {
            m.content
                .events
                .iter()
                .filter(|e| e.voice == voice)
                .filter_map(move |event| {
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
            m.content
                .events
                .iter()
                .filter(|e| e.voice == voice)
                .map(move |event| {
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
        self.note_drag = None;
        if self.busy || self.confirming {
            return;
        }
        self.reveal_selection = false;
        let point = canvas_point(event.position, self.canvas_origin);
        self.clicked_mark = None;
        if let Some(measure) = self.layout.hit_tempo(point) {
            self.document.editor.set_cursor(score_edit::Cursor {
                measure,
                index: 0,
                ..self.document.editor.cursor()
            });
            self.toggle_tempo_picker(cx);
            return;
        }
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
        let note = self
            .layout
            .hit_pitch_with_position(point, self.document.editor.cursor().voice);
        let seek_tick = (!event.modifiers.shift)
            .then(|| {
                crate::playhead::score_tick(
                    self.document.editor.score(),
                    &self.layout,
                    point,
                    self.document.editor.cursor().voice,
                )
            })
            .flatten();
        if let Some((id, pitch, tick)) = note {
            self.ui.note_entry = false;
            if event.modifiers.shift {
                self.document.editor.extend_selection(id);
            } else {
                self.document.editor.select_pitch(id, pitch);
                self.document
                    .editor
                    .select_divisi_position(score_core::DivisiPoint::new(id.measure, tick));
            }
            self.playback.follow_staff(id.part, id.staff);
            if let Some(tick) = seek_tick {
                self.seek_playback(tick, false, cx);
            }
            if !event.modifiers.shift
                && let Some(pitch) = pitch
            {
                self.note_drag = crate::note_drag::NoteDrag::new(
                    self.document.editor.score(),
                    id,
                    pitch,
                    event.position,
                    self.canvas_origin,
                    &self.layout,
                );
            }
            self.selection_drag = Some(PassageDrag {
                anchor: id,
                start: event.position,
                active: false,
                select_anchor: false,
            });
        } else if let Some((part, staff, measure)) = self.layout.measure_at(point) {
            let placed = self.layout.measure_on(part, staff, measure).unwrap();
            let (voice, index) = placed.insertion_near(
                self.document.editor.cursor().voice,
                point.x / self.layout.scale - placed.origin.x,
            );
            let staff = placed.source_staff;
            let bar = &self
                .document
                .editor
                .score()
                .part(part)
                .unwrap()
                .staff(staff)
                .unwrap()
                .measures()[measure];
            let anchor = bar
                .voice_id_at(voice, 0)
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
                    voice,
                    part,
                    staff,
                    measure,
                    index,
                });
            }
            self.playback.follow_staff(part, staff);
            if let Some(tick) = seek_tick {
                self.seek_playback(tick, false, cx);
            }
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
            "Clic sitúa la reproducción · Arrastra arriba/abajo para cambiar altura; horizontal para mover; Shift para seleccionar"
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
            self.move_score_pointer(event.position, cx);
        } else {
            self.selection_drag = None;
            self.cancel_note_drag(cx);
        }
    }

    pub(super) fn end_passage_selection(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_score_pointer(event.position, cx);
        if let Some(drag) = self.note_drag.take() {
            if drag.active {
                if drag.is_time() {
                    if let Some(target) = drag.time_target {
                        self.document
                            .editor
                            .select_pitch(drag.id, Some(drag.original));
                        self.edit(
                            Command::MoveSelectedEvent {
                                measure: target.measure,
                                tick: target.tick,
                            },
                            cx,
                        );
                        if let Some(id) = self.document.editor.selection()
                            && self.document.editor.score().event_tick(id)
                                == Some(
                                    self.document
                                        .editor
                                        .score()
                                        .measure_start_tick(target.measure)
                                        + u64::from(target.tick),
                                )
                        {
                            let tick = self.document.editor.score().event_tick(id).unwrap();
                            self.seek_playback(tick, false, cx);
                            self.status = format!(
                                "{} al compás {} · ⌘Z para deshacer",
                                if target.swap {
                                    "Figuras intercambiadas"
                                } else {
                                    "Figura movida"
                                },
                                target.measure + 1
                            );
                            if let Err(error) =
                                self.playback.preview(self.document.editor.score(), id)
                            {
                                self.status = format!("Figura movida · Audio: {error}");
                            }
                        }
                    } else if let Some(error) = drag.error {
                        self.status = error;
                    }
                } else if let Some(pitch) = drag.target {
                    let tick = self.document.editor.score().event_tick(drag.id);
                    self.document
                        .editor
                        .select_pitch(drag.id, Some(drag.original));
                    self.edit(Command::ReplacePitch(pitch), cx);
                    if self.document.editor.selected_pitch() == Some(pitch) {
                        if let Some(tick) = tick {
                            self.seek_playback(tick, false, cx);
                        }
                        self.status = format!("Nota cambiada a {pitch} · ⌘Z para deshacer");
                        if let Err(error) =
                            self.playback.preview(self.document.editor.score(), drag.id)
                        {
                            self.status = format!("Nota cambiada a {pitch} · Audio: {error}");
                        }
                    }
                } else if let Some(error) = drag.error {
                    self.status = error;
                }
            }
            cx.notify();
        }
        self.selection_drag = None;
    }

    fn move_score_pointer(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.busy || self.confirming {
            self.cancel_note_drag(cx);
            self.selection_drag = None;
            return;
        }
        let motion = self.note_drag.as_ref().map(|drag| drag.motion(position));
        match motion {
            Some(crate::note_drag::Motion::Pending) => return,
            Some(mode @ (crate::note_drag::Motion::Time | crate::note_drag::Motion::Pitch)) => {
                let drag = self.note_drag.as_mut().unwrap();
                let starting = !drag.active;
                let changed = if mode == crate::note_drag::Motion::Time {
                    drag.update_time(position, self.canvas_origin)
                } else {
                    drag.update(position, self.canvas_origin)
                };
                let id = drag.id;
                let error = drag.error.clone();
                let pitch = drag.target.unwrap_or(drag.original);
                let time = drag.time_target;
                self.selection_drag = None;
                if starting {
                    let tick = self.document.editor.score().event_tick(id);
                    self.playback.stop();
                    if let Some(tick) = tick {
                        let _ = self.playback.seek(self.document.editor.score(), tick);
                    }
                }
                if changed {
                    self.status = error.unwrap_or_else(|| {
                        if mode == crate::note_drag::Motion::Time {
                            time.map_or_else(|| "Posición original · Esc cancela".into(), |t| {
                                let time = self.document.editor.score().time_at(t.measure);
                                let beat = 1. + f64::from(t.tick) / f64::from(score_core::TICKS_PER_WHOLE / u16::from(time.denominator));
                                format!("{} · Compás {} · Tiempo {beat:.2} · Suelta para aplicar; Esc cancela", if t.swap { "Intercambiar figuras" } else { "Mover figura" }, t.measure + 1)
                            })
                        } else { format!("{pitch} · Suelta para aplicar; Esc cancela") }
                    });
                    cx.notify();
                }
                return;
            }
            None => {}
        }
        self.extend_passage_drag(position, cx);
    }

    pub(super) fn cancel_note_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.note_drag.take() {
            if drag.active {
                self.status = "Cambio de nota cancelado".into();
            }
            self.selection_drag = None;
            cx.notify();
        }
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
    let Some(voice) = layout
        .event_measure(*first)
        .and_then(|m| m.content.events.iter().find(|e| e.id == *first))
        .map(|e| e.voice)
    else {
        return Vec::new();
    };
    let start = layout
        .measures
        .iter()
        .flat_map(|m| {
            m.content
                .events
                .iter()
                .filter(move |e| e.id == *first)
                .map(move |e| m.start_tick + u64::from(e.tick))
        })
        .min()
        .unwrap();
    let end = layout
        .measures
        .iter()
        .flat_map(|m| {
            m.content
                .events
                .iter()
                .filter(move |e| e.id == *last)
                .map(move |e| m.start_tick + u64::from(e.end_tick))
        })
        .max()
        .unwrap();
    let mut measures: Vec<_> = layout
        .measures
        .iter()
        .filter(|m| {
            m.part == first.part
                && m.source_staff == first.staff
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
        let capacity = m.content.time_positions.last().unwrap().0;
        for r in m.regions.iter().filter(|r| r.voice == voice) {
            let a = (m.start_tick + u64::from(r.start)).max(start);
            let b = (m.start_tick + u64::from(r.end)).min(end);
            if b <= a {
                continue;
            }
            let a = (a - m.start_tick) as u16;
            let b = (b - m.start_tick) as u16;
            let left = if a == 0 {
                0.
            } else {
                m.content.x_at_tick(f64::from(a)) - 12.
            };
            let right = if b == capacity {
                m.width
            } else {
                m.content.x_at_tick(f64::from(b)) + 8.
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
            if let Some(previous) = bands.last_mut().filter(|old| {
                row == Some(m.origin.y)
                    && (old.right() - bounds.left()).abs() < px(1.)
                    && old.top() <= bounds.bottom()
                    && old.bottom() >= bounds.top()
            }) {
                *previous = previous.union(&bounds);
            } else {
                bands.push(bounds);
            }
            row = Some(m.origin.y);
        }
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
    #[::core::prelude::v1::test]
    fn projected_second_voice_selection_highlights_and_drags_on_the_lower_staff() {
        let mut score = Score::default();
        let part = score.add_part(score_core::Instrument::Violin).unwrap();
        score.remove_part(score.parts()[0].id()).unwrap();
        let mut ids = Vec::new();
        for bar in 0..2 {
            ids.push(
                score
                    .insert_in_voice(
                        part,
                        StaffId::Upper,
                        bar,
                        score_core::Voice::Two,
                        0,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Whole,
                            notation: Default::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        score.set_passage_divisi(part, 0, Some(1), true).unwrap();
        let layout = score_layout::LayoutEngine::default().layout_view(
            &score,
            1000.,
            score_layout::ViewMode::Page,
        );
        let bands = passage_bounds(&layout, &ids);
        for &id in &ids {
            let m = layout.event_measure(id).unwrap();
            assert_eq!(m.staff, StaffId::Lower);
            let p = ScorePoint {
                x: m.origin.x + m.content.events[0].center.x,
                y: m.origin.y + m.content.events[0].center.y,
            };
            assert_eq!(nearest_event(&layout, ids[0], p, false), Some(id));
            assert!(
                bands
                    .iter()
                    .any(|b| b.top() <= px(m.origin.y + 20.) && b.bottom() >= px(m.origin.y + 20.))
            );
            let upper = layout.measure_on(part, StaffId::Upper, id.measure).unwrap();
            assert!(
                bands
                    .iter()
                    .all(|b| b.top() > px(upper.origin.y + 40.) || b.bottom() < px(upper.origin.y))
            );
        }
    }
    #[::core::prelude::v1::test]
    fn midbar_passage_highlights_one_continuous_voice_row_and_keeps_blank_hits() {
        let mut score = Score::default();
        let part = score.add_part(score_core::Instrument::Violin).unwrap();
        score.remove_part(score.parts()[0].id()).unwrap();
        let mut ids = Vec::new();
        for i in 0..4 {
            score
                .insert_in(
                    part,
                    0,
                    i,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::G, 5)),
                        duration: Duration::Quarter,
                        notation: Default::default(),
                    },
                )
                .unwrap();
            ids.push(
                score
                    .insert_in_voice(
                        part,
                        StaffId::Upper,
                        0,
                        score_core::Voice::Two,
                        i,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Quarter,
                            notation: Default::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        score
            .set_divisi_span(
                part,
                score_core::DivisiPoint::new(0, 48),
                Some(score_core::DivisiPoint::new(0, 144)),
                true,
            )
            .unwrap();
        let layout = score_layout::LayoutEngine::default().layout_view(
            &score,
            1000.,
            score_layout::ViewMode::Page,
        );
        let bands = passage_bounds(&layout, &ids);
        assert_eq!(bands.len(), 1);
        for &id in &ids {
            let m = layout.event_measure(id).unwrap();
            let e = m.content.events.iter().find(|e| e.id == id).unwrap();
            let p = point(px(m.origin.x + e.center.x), px(m.origin.y + e.center.y));
            assert!(bands.iter().any(|b| b.contains(&p)));
            assert_eq!(
                nearest_event(
                    &layout,
                    ids[0],
                    ScorePoint {
                        x: m.origin.x + e.center.x,
                        y: m.origin.y + e.center.y
                    },
                    false
                ),
                Some(id)
            );
        }
        let lower = layout.measure_on(part, StaffId::Lower, 0).unwrap();
        let (voice, index) =
            lower.insertion_near(score_core::Voice::Two, lower.content.x_at_tick(96.));
        assert_eq!(voice, score_core::Voice::Two);
        assert_eq!(index, 2);
    }
}
