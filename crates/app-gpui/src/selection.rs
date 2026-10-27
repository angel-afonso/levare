//! Pointer selection uses the existing continuous, single-staff passage model.
use super::*;
use score_keymap::Modifier;
use score_layout::{Layout, Point as ScorePoint};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Hover {
    pub hit: score_layout::Hit,
    pub bounds: Option<Bounds<Pixels>>,
}

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

pub(super) fn anchored_offset(
    offset: Point<Pixels>,
    pointer_in_canvas: Point<Pixels>,
    ratio: f32,
) -> Point<Pixels> {
    offset
        + point(
            px(f32::from(pointer_in_canvas.x) * (1. - ratio)),
            px(f32::from(pointer_in_canvas.y) * (1. - ratio)),
        )
}

fn wheel_steps(acc: &mut f32, delta: f32) -> i32 {
    *acc += delta;
    let steps = (*acc / 40.).trunc() as i32;
    *acc -= steps as f32 * 40.;
    steps
}

fn select_whole_bar(
    editor: &mut score_edit::Editor,
    part: score_core::PartId,
    staff: score_core::StaffId,
    measure: usize,
    voice: score_core::Voice,
) -> bool {
    let Some(bar) = editor
        .score()
        .part(part)
        .and_then(|part| part.staff(staff))
        .and_then(|staff| staff.measures().get(measure))
    else {
        return false;
    };
    let Some(first) = bar.voice_id_at(voice, 0) else {
        return false;
    };
    let Some(last) = bar.voice_id_at(voice, bar.voice_len(voice) - 1) else {
        return false;
    };
    let id = |event| ElementId {
        part,
        staff,
        measure,
        event,
    };
    editor.select(id(first));
    editor.extend_selection(id(last));
    true
}

/// Prefer the nearest row of the originating staff, then the nearest time on
/// that row. Pitch height and other instruments cannot divert the passage.
pub(super) fn nearest_event(
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
    pub(super) fn open_score_context_menu(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.confirming || self.agent_blocks(cx) {
            return;
        }
        self.focus.focus(window);
        self.selection_drag = None;
        self.note_drag = None;
        self.mark_drag = None;
        self.ghost = None;
        self.hover = None;
        self.ui.context_menu = None;
        self.ui.file_menu = false;
        self.ui.recent_menu = false;
        self.ui.open_group = None;
        self.reveal_selection = false;
        let point = canvas_point(event.position, self.canvas_origin);
        let is_mark = if let Some(mark) = self.layout.hit_bar_mark(point) {
            self.clicked_mark = None;
            self.document.editor.set_cursor(Cursor {
                measure: mark.measure,
                index: 0,
                ..self.document.editor.cursor()
            });
            self.clicked_bar_mark = Some(mark);
            true
        } else {
            match self.layout.hit(point) {
                Some(score_layout::Hit::Mark(mark)) => {
                    self.clicked_bar_mark = None;
                    self.document.editor.select(mark.anchor());
                    self.clicked_mark = Some(mark);
                    true
                }
                Some(score_layout::Hit::Note(id, pitch)) => {
                    self.clicked_mark = None;
                    self.clicked_bar_mark = None;
                    if !self.document.editor.selected_ids().contains(&id) {
                        self.document.editor.select_pitch(id, pitch);
                    }
                    self.ui.note_entry = false;
                    false
                }
                None => {
                    cx.notify();
                    return;
                }
            }
        };
        self.ui.context_menu = Some((event.position, is_mark));
        cx.notify();
    }

    pub(super) fn score_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ghost_cleared = self.ghost.take().is_some();
        let hover_cleared = self.hover.take().is_some();
        let cleared = ghost_cleared || hover_cleared;
        if !event.modifiers.secondary() && !event.modifiers.control {
            if cleared {
                cx.notify();
            }
            return;
        }
        let steps = wheel_steps(
            &mut self.zoom_wheel,
            f32::from(event.delta.pixel_delta(px(20.)).y),
        );
        if steps != 0 {
            self.zoom_at(self.ui.zoom + steps as f32 * 0.1, event.position);
        }
        cx.stop_propagation();
        cx.notify();
    }
    fn ghost_at_pointer(&self, position: Point<Pixels>) -> Option<crate::ghost::Ghost> {
        let visible = self.viewport.scroll.bounds();
        if !self.ui.note_entry
            || self.busy
            || self.confirming
            || self.agent_locked()
            || self.ui.context_menu.is_some()
            || self.note_drag.is_some()
            || self.mark_drag.is_some()
            || self.selection_drag.is_some()
            || self.playback.is_playing()
            || self.playback.is_paused()
            || position.x < visible.origin.x
            || position.x > visible.origin.x + visible.size.width
            || position.y < visible.origin.y
            || position.y > visible.origin.y + visible.size.height
        {
            return None;
        }
        crate::ghost::ghost_at(
            self.document.editor.score(),
            &self.layout,
            canvas_point(position, self.canvas_origin),
            self.document.editor.cursor().voice,
            self.document.editor.duration,
        )
    }

    pub(super) fn update_ghost(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let next = self.ghost_at_pointer(position);
        if next != self.ghost {
            self.ghost = next;
            cx.notify();
        }
    }

    pub(super) fn begin_passage_selection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus.focus(window);
        self.ui.context_menu = None;
        let interrupted_drag = self.selection_drag.is_some()
            || self.note_drag.is_some()
            || self.mark_drag.is_some()
            || self.connection_drag.is_some();
        // Read before the drags are dropped: the handles belong to the picked line.
        let handles = self.line_handles();
        self.selection_drag = None;
        self.note_drag = None;
        self.mark_drag = None;
        self.connection_drag = None;
        self.ghost = None;
        self.hover = None;
        if self.busy || self.confirming || self.agent_blocks(cx) {
            return;
        }
        self.reveal_selection = false;
        let point = canvas_point(event.position, self.canvas_origin);
        if self.begin_line_drag(event, point, handles, cx) {
            return;
        }
        self.select_at(event, point, interrupted_drag, cx);
        if self.close_moved_editors() {
            cx.notify();
        }
    }

    /// What a press on the score selects, writes or starts dragging.
    fn select_at(
        &mut self,
        event: &MouseDownEvent,
        point: ScorePoint,
        interrupted_drag: bool,
        cx: &mut Context<Self>,
    ) {
        self.clicked_mark = None;
        self.clicked_bar_mark = None;
        if event.click_count == 2
            && !event.modifiers.shift
            && self.layout.hit_bar_mark(point).is_none()
            && !matches!(self.layout.hit(point), Some(score_layout::Hit::Mark(_)))
            && let Some((part, staff, measure)) = self.layout.measure_at(point)
        {
            let placed = self.layout.measure_on(part, staff, measure).unwrap();
            let (voice, _) = placed.insertion_near(
                self.document.editor.cursor().voice,
                point.x / self.layout.scale - placed.origin.x,
            );
            let staff = placed.source_staff;
            if select_whole_bar(&mut self.document.editor, part, staff, measure, voice) {
                self.ui.note_entry = false;
                self.playback.follow_staff(part, staff);
                self.selection_status();
                cx.notify();
            }
            return;
        }
        if !interrupted_drag
            && !event.modifiers.shift
            && let Some(ghost) = self.ghost_at_pointer(event.position)
            && self.layout.hit_bar_mark(point).is_none()
            && !matches!(self.layout.hit(point), Some(score_layout::Hit::Mark(_)))
            && !matches!(
                self.layout.hit_pitch_with_position(point, ghost.voice),
                Some((_, Some(_), _))
            )
        {
            self.document.editor.set_cursor(Cursor {
                part: ghost.part,
                staff: ghost.staff,
                voice: ghost.voice,
                measure: ghost.measure,
                index: 0,
            });
            let event = score_core::Event {
                pitches: score_core::Pitches::single(Some(
                    ghost.pitch.with_accidental(self.entry_accidental),
                )),
                duration: ghost.duration,
                notation: Default::default(),
            };
            self.edit(
                score_edit::Command::WriteAt {
                    measure: ghost.measure,
                    tick: ghost.tick,
                    event,
                },
                cx,
            );
            self.playback.follow_staff(ghost.part, ghost.staff);
            return;
        }
        if let Some(mark) = self
            .layout
            .hit_bar_mark(point)
            .filter(|_| !event.modifiers.shift)
        {
            self.document.editor.set_cursor(score_edit::Cursor {
                measure: mark.measure,
                index: 0,
                ..self.document.editor.cursor()
            });
            self.clicked_bar_mark = Some(mark);
            self.mark_drag = crate::mark_drag::MarkDrag::new_bar(
                self.document.editor.score(),
                mark,
                event.position,
                self.canvas_origin,
                self.layout.scale,
                Rc::clone(&self.layout),
            );
            self.status = format!(
                "{} seleccionada · Arrastra para mover; {} restablece",
                crate::mark_drag::DragTarget::Bar(mark).label(self.document.editor.score()),
                self.shortcuts.hint(id::RESET_POSITION)
            );
            cx.notify();
            return;
        }
        let hit = self.layout.hit(point);
        if let Some(score_layout::Hit::Mark(mark)) = hit.filter(|_| !event.modifiers.shift) {
            let anchor = mark.anchor();
            self.document.editor.select(anchor);
            self.clicked_mark = Some(mark);
            self.mark_drag = crate::mark_drag::MarkDrag::new(
                self.document.editor.score(),
                mark,
                event.position,
                self.canvas_origin,
                self.layout.scale,
                Rc::clone(&self.layout),
            );
            self.playback.follow_staff(anchor.part, anchor.staff);
            let label = self
                .document
                .editor
                .score()
                .mark_label(mark)
                .unwrap_or_default();
            let step = if mark.follows_time() {
                // ⌥← on macOS, Alt+← elsewhere: `mark_drag::step`.
                let left = Key::parse("alt-left")
                    .map(|key| key.label())
                    .unwrap_or_default();
                format!("; {left}/→ lo lleva a otra nota")
            } else {
                String::new()
            };
            let delete = self.shortcuts.hint(id::DELETE);
            self.status = if matches!(mark, MarkRef::Grace(..)) {
                format!("Nota de adorno seleccionada · ↑/↓ cambian su altura; {delete} la borra")
            } else {
                format!(
                    "{label} seleccionado · {delete} lo borra{step}; pulsa otra marca de la cinta para cambiarlo"
                )
            };
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
            self.ghost = None;
            if event.modifiers.shift {
                self.document.editor.extend_selection(id);
            } else {
                self.document.editor.select_pitch(id, pitch);
                self.document
                    .editor
                    .select_divisi_position(score_core::DivisiPoint::new(id.measure, tick));
                self.audition_selection();
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

    /// A press on a handle of the picked line stretches it; Alt on a note draws
    /// a slur from it, Alt with the secondary key (⌘ on macOS, Ctrl elsewhere)
    /// a crescendo, and with Shift too a diminuendo.
    fn begin_line_drag(
        &mut self,
        event: &MouseDownEvent,
        point: ScorePoint,
        handles: Option<Connection>,
        cx: &mut Context<Self>,
    ) -> bool {
        use crate::connection_input::{ConnectionDrag, handle_at};
        if let Some(line) = handles
            && let Some(end) = handle_at(&self.layout, line, (point.x, point.y))
        {
            self.connection_drag = Some(ConnectionDrag::stretch(
                self.document.editor.score(),
                line,
                end,
                event.position,
                &self.layout,
            ));
            self.clicked_mark = Some(MarkRef::Connection(line));
            self.status = "Arrastra el extremo hasta otra nota; Esc cancela".into();
            cx.notify();
            return true;
        }
        if !event.modifiers.alt {
            return false;
        }
        let Some((id, pitch, _)) = self
            .layout
            .hit_pitch_with_position(point, self.document.editor.cursor().voice)
        else {
            return false;
        };
        let kind = match (event.modifiers.secondary(), event.modifiers.shift) {
            (false, _) => ConnectionKind::Slur,
            (true, false) => ConnectionKind::Crescendo,
            (true, true) => ConnectionKind::Diminuendo,
        };
        self.clicked_mark = None;
        self.ui.note_entry = false;
        self.document.editor.select(id);
        self.playback.follow_staff(id.part, id.staff);
        if kind == ConnectionKind::Slur && pitch.is_none() {
            self.status = "La ligadura de expresión empieza en una nota o acorde".into();
        } else {
            self.connection_drag = Some(ConnectionDrag::draw(
                self.document.editor.score(),
                kind,
                id,
                event.position,
                &self.layout,
            ));
            self.status = format!("{}: arrastra hasta la nota final y suelta", kind.label());
        }
        cx.notify();
        true
    }

    /// Point the dragged end of a line at the nearest event of its voice.
    fn aim_line_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = self.connection_drag.as_mut() else {
            return;
        };
        if drag.pending(position) {
            return;
        }
        let point = canvas_point(position, self.canvas_origin);
        let Some(target) = nearest_event(&drag.geometry, drag.fixed, point, false) else {
            return;
        };
        if !drag.aim(target) {
            return;
        }
        self.playback.stop();
        self.status = match (drag.candidate, drag.error) {
            (Some(line), _) => format!(
                "{} hasta el compás {} · Suelta para aplicar; Esc cancela",
                line.kind.label(),
                line.end.measure + 1
            ),
            (None, Some(error)) => error.into(),
            (None, None) => String::new(),
        };
        cx.notify();
    }

    fn finish_line_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.connection_drag.take() else {
            return false;
        };
        self.selection_drag = None;
        if let Some(command) = drag.command()
            && let Some(line) = drag.candidate
        {
            if self.try_edit(command, cx) {
                let mark = MarkRef::Connection(line);
                self.document.editor.select(mark.anchor());
                self.clicked_mark = Some(mark);
                self.status = format!(
                    "{} hasta el compás {} · {} para deshacer",
                    line.kind.label(),
                    line.end.measure + 1,
                    self.shortcuts.hint(id::UNDO)
                );
            }
        } else if drag.active {
            self.status = drag.error.map_or("Sin cambios", |error| error).into();
            if let Some(line) = drag.original {
                self.clicked_mark = Some(MarkRef::Connection(line));
            }
        }
        cx.notify();
        true
    }

    fn selection_status(&mut self) {
        let count = self.document.editor.selected_ids().len();
        self.status = if count > 1 {
            format!(
                "{count} elementos seleccionados · Copiar o cortar para llevar el pasaje a otro lugar"
            )
        } else {
            String::new()
        };
    }

    fn extend_passage_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let locked = self.agent_locked();
        let Some(drag) = self.selection_drag.as_mut() else {
            return;
        };
        if self.busy
            || self.confirming
            || locked
            || self.document.editor.score().event(drag.anchor).is_none()
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
            if self.ghost.take().is_some() {
                cx.notify();
            }
            if self.hover.take().is_some() {
                cx.notify();
            }
            self.move_score_pointer(event.position, event.modifiers, cx);
        } else {
            self.selection_drag = None;
            self.cancel_note_drag(cx);
            self.update_ghost(event.position, cx);
            let visible = self.viewport.scroll.bounds();
            let inside = event.position.x >= visible.origin.x
                && event.position.x <= visible.origin.x + visible.size.width
                && event.position.y >= visible.origin.y
                && event.position.y <= visible.origin.y + visible.size.height;
            let next = if self.ghost.is_none()
                && inside
                && self.ui.context_menu.is_none()
                && self.mark_drag.is_none()
                && !self.busy
                && !self.confirming
                && !self.agent_locked()
            {
                let point = canvas_point(event.position, self.canvas_origin);
                self.layout.hit(point).map(|hit| Hover {
                    hit,
                    bounds: crate::paint::hover_bounds(&self.layout, hit, point),
                })
            } else {
                None
            };
            if self.hover != next {
                self.hover = next;
                cx.notify();
            }
        }
    }

    pub(super) fn end_passage_selection(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_score_pointer(event.position, event.modifiers, cx);
        if self.finish_line_drag(cx) {
            return;
        }
        if let Some(drag) = self.mark_drag.take() {
            let mark = drag.mark;
            if let crate::mark_drag::DragTarget::Mark(from) = mark
                && let Some((to, offset)) = drag.destination()
            {
                self.move_mark(from, to, offset, cx);
            } else if let Some(error) = drag.error.filter(|_| drag.active) {
                self.status = error.into();
                cx.notify();
            } else if let Some(offset) = drag.target() {
                let label = mark.label(self.document.editor.score());
                match mark {
                    crate::mark_drag::DragTarget::Mark(mark) => {
                        self.edit(Command::SetMarkOffset(mark, offset), cx);
                        self.clicked_mark = Some(mark);
                    }
                    crate::mark_drag::DragTarget::Bar(mark) => {
                        self.edit(Command::SetBarMarkOffset(mark, offset), cx);
                        self.clicked_bar_mark = Some(mark);
                        self.reveal_selection = false;
                    }
                }
                self.status = format!(
                    "{label} · Posición ajustada; {} la devuelve a su sitio · {} para deshacer",
                    self.shortcuts.hint(id::RESET_POSITION),
                    self.shortcuts.hint(id::UNDO)
                );
                cx.notify();
            } else if !drag.active
                && let crate::mark_drag::DragTarget::Bar(mark) = mark
                && mark.mark == score_core::BarMark::Tempo
            {
                self.document.editor.set_cursor(score_edit::Cursor {
                    measure: mark.measure,
                    index: 0,
                    ..self.document.editor.cursor()
                });
                self.toggle_tempo_picker(cx);
            }
            self.selection_drag = None;
            return;
        }
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
                                "{} al compás {} · {} para deshacer",
                                if target.swap {
                                    "Figuras intercambiadas"
                                } else {
                                    "Figura movida"
                                },
                                target.measure + 1,
                                self.shortcuts.hint(id::UNDO)
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
                        self.status = format!(
                            "Nota cambiada a {pitch} · {} para deshacer",
                            self.shortcuts.hint(id::UNDO)
                        );
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

    fn move_score_pointer(
        &mut self,
        position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.confirming || self.agent_locked() {
            self.cancel_note_drag(cx);
            self.selection_drag = None;
            self.connection_drag = None;
            return;
        }
        if self.connection_drag.is_some() {
            self.aim_line_drag(position, cx);
            return;
        }
        if let Some(drag) = self.mark_drag.as_mut() {
            if drag.pending(position) {
                return;
            }
            let starting = !drag.active;
            let free = modifiers.secondary();
            let changed = drag.update(position, self.canvas_origin, modifiers.shift, free);
            if starting {
                self.playback.stop();
            }
            if changed {
                let score = self.document.editor.score();
                let label = drag.mark.label(score);
                self.status = if let Some(error) = drag.error {
                    error.into()
                } else if let Some((moved, _)) = drag.destination() {
                    format!(
                        "{label} → {} · Suelta para aplicar; {} mueve sin cambiar de nota; Esc cancela",
                        crate::mark_drag::place_label(score, moved.anchor()),
                        Modifier::Secondary.label()
                    )
                } else if drag.follows_time() && !free {
                    format!(
                        "{label} · Arrastra a los lados para cambiar de nota; {} mueve sin cambiar de nota; Esc cancela",
                        Modifier::Secondary.label()
                    )
                } else {
                    format!("{label} · Suelta para colocarla; Mayús fija la dirección; Esc cancela")
                };
                cx.notify();
            }
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
        if let Some(drag) = self.mark_drag.take() {
            if drag.active {
                self.status = "Movimiento de la marca cancelado".into();
            }
            self.selection_drag = None;
            cx.notify();
        }
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

    #[::core::prelude::v1::test]
    fn anchored_offset_keeps_the_pointer_still_and_reverses_cleanly() {
        let offset = point(px(-30.), px(75.));
        let pointer = point(px(120.), px(45.));
        assert_eq!(anchored_offset(offset, pointer, 1.), offset);
        let doubled = anchored_offset(offset, pointer, 2.);
        let old_point = offset + pointer;
        let new_point = doubled + point(px(240.), px(90.));
        assert!((f32::from(old_point.x - new_point.x)).abs() < 0.01);
        assert!((f32::from(old_point.y - new_point.y)).abs() < 0.01);
        let restored = anchored_offset(doubled, point(px(240.), px(90.)), 0.5);
        assert!((f32::from(restored.x - offset.x)).abs() < 0.01);
        assert!((f32::from(restored.y - offset.y)).abs() < 0.01);
    }

    #[::core::prelude::v1::test]
    fn wheel_steps_preserve_partial_scroll_motion() {
        let mut acc = 0.;
        assert_eq!(wheel_steps(&mut acc, 39.), 0);
        assert_eq!(acc, 39.);
        assert_eq!(wheel_steps(&mut acc, 1.), 1);
        assert_eq!(acc, 0.);
        assert_eq!(wheel_steps(&mut acc, -85.), -2);
        assert_eq!(acc, -5.);
    }

    #[::core::prelude::v1::test]
    fn hover_bounds_wrap_what_hit_returns_at_every_zoom() {
        let (score, _) = fixture();
        for scale in [0.5, 0.7, 1., 1.5] {
            let mut layout = score_layout::LayoutEngine::default().layout(&score, 760.);
            layout.scale = scale;
            for measure in &layout.measures {
                for event in &measure.content.events {
                    if event.pitch.is_none() {
                        continue;
                    }
                    let point = ScorePoint {
                        x: (measure.origin.x + event.center.x) * scale,
                        y: (measure.origin.y + event.center.y) * scale,
                    };
                    let hit = layout.hit(point).expect("head has a hit");
                    let bounds = crate::paint::hover_bounds(&layout, hit, point)
                        .expect("head has a hover box");
                    assert!(bounds.contains(&gpui::point(px(point.x), px(point.y))));
                }
            }
        }
    }

    #[::core::prelude::v1::test]
    fn hover_chooses_the_projected_mark_box_under_the_pointer() {
        let (mut score, ids) = fixture();
        score
            .set_dynamic(ids[0], Some(score_core::Dynamic::P))
            .unwrap();
        let mut layout = score_layout::LayoutEngine::default().layout(&score, 760.);
        let original = layout
            .measures
            .iter()
            .find(|measure| {
                measure
                    .content
                    .marks
                    .iter()
                    .any(|mark| mark.mark == MarkRef::Dynamic(ids[0]))
            })
            .unwrap()
            .clone();
        let mut projection = original.clone();
        projection.origin.y += 200.;
        layout.measures.push(projection.clone());
        let mark = projection
            .content
            .marks
            .iter()
            .find(|mark| mark.mark == MarkRef::Dynamic(ids[0]))
            .unwrap();
        let point = ScorePoint {
            x: projection.origin.x + mark.center.x,
            y: projection.origin.y + mark.center.y,
        };
        let hit = layout.hit(point).unwrap();
        assert_eq!(hit, score_layout::Hit::Mark(MarkRef::Dynamic(ids[0])));
        let bounds = crate::paint::hover_bounds(&layout, hit, point).unwrap();
        assert!(bounds.contains(&gpui::point(px(point.x), px(point.y))));
        assert!(f32::from(bounds.top()) > original.origin.y + 100.);
    }

    #[::core::prelude::v1::test]
    fn double_click_selects_all_events_of_the_resolved_bar_and_voice() {
        let (mut score, ids) = fixture();
        let mut second = Vec::new();
        for (index, step) in [Step::D, Step::F].into_iter().enumerate() {
            second.push(
                score
                    .insert_in_voice(
                        ids[0].part,
                        ids[0].staff,
                        0,
                        score_core::Voice::Two,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(step, 3)),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
        let mut editor = score_edit::Editor::new(score).unwrap();
        assert!(select_whole_bar(
            &mut editor,
            ids[0].part,
            ids[0].staff,
            0,
            score_core::Voice::One,
        ));
        assert_eq!(editor.selected_ids(), ids[..3]);
        assert!(select_whole_bar(
            &mut editor,
            ids[0].part,
            ids[0].staff,
            0,
            score_core::Voice::Two,
        ));
        assert_eq!(editor.selected_ids(), second);
        assert!(!select_whole_bar(
            &mut editor,
            ids[0].part,
            ids[0].staff,
            2,
            score_core::Voice::One,
        ));
        assert_eq!(editor.selected_ids(), second);
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
