//! Rehearsal: loop a passage, play at half speed, replay, and move by bars.
use super::*;

impl AppView {
    /// The written ticks to loop: a selected passage, the bar of a single
    /// selected note, or the whole score without a selection.
    fn loop_region(&self) -> Option<(u64, u64)> {
        let editor = &self.document.editor;
        let score = editor.score();
        let ids = editor.selected_ids();
        match ids.as_slice() {
            [] => None,
            [id] => {
                let start = score.measure_start_tick(id.measure);
                Some((
                    start,
                    start + u64::from(score.time_at(id.measure).capacity()),
                ))
            }
            _ => {
                let start = ids.iter().filter_map(|id| score.event_tick(*id)).min()?;
                let end = ids
                    .iter()
                    .filter_map(|id| {
                        Some(score.event_tick(*id)? + u64::from(score.event(*id)?.duration.ticks()))
                    })
                    .max()?;
                Some((start, end))
            }
        }
    }

    fn key(&self, command: CommandId) -> String {
        self.shortcuts
            .label(command)
            .map_or_else(String::new, |keys| format!(" ({keys})"))
    }

    pub(crate) fn toggle_loop(&mut self, cx: &mut Context<Self>) {
        let looping = !self.playback.looping;
        let region = looping.then(|| self.loop_region()).flatten();
        let score = self.document.editor.score();
        let cursor = self.document.editor.cursor();
        self.status = match self.playback.set_looping(looping, region, score, cursor) {
            Err(error) => error,
            Ok(()) if !looping => "Bucle desactivado".into(),
            Ok(()) => {
                let passage = match region {
                    None => "toda la partitura".into(),
                    Some((start, end)) => {
                        let first = score.measure_at_tick(start).0 + 1;
                        let last = score.measure_at_tick(end.saturating_sub(1)).0 + 1;
                        if first == last {
                            format!("compás {first}")
                        } else {
                            format!("compases {first}–{last}")
                        }
                    }
                };
                format!(
                    "Bucle: {passage} · vuelve a pulsar{} para quitarlo",
                    self.key(id::LOOP)
                )
            }
        };
        cx.notify();
    }

    /// Half speed starts playing when nothing plays; during playback it
    /// switches speed in place.
    pub(crate) fn toggle_half_speed(&mut self, cx: &mut Context<Self>) {
        let half = !self.playback.half_speed;
        let active = self.playback.is_playing() || self.playback.is_paused();
        let score = self.document.editor.score();
        let cursor = self.document.editor.cursor();
        if let Err(error) = self.playback.set_half_speed(half, score, cursor) {
            self.status = error;
            cx.notify();
            return;
        }
        if half && !active {
            self.toggle_playback(cx);
            if !self.playback.is_playing() {
                return;
            }
        }
        self.status = if half {
            format!(
                "Media velocidad: el tempo escrito al 50 % · vuelve a pulsar{} para la normal",
                self.key(id::HALF_SPEED)
            )
        } else {
            "Velocidad normal".into()
        };
        cx.notify();
    }

    pub(crate) fn replay(&mut self, cx: &mut Context<Self>) {
        let Some(tick) = self.playback.last_start else {
            self.status = "Todavía no has reproducido nada en esta sesión".into();
            cx.notify();
            return;
        };
        self.playback.stop();
        if let Err(error) = self.playback.seek(self.document.editor.score(), tick) {
            self.status = error;
            cx.notify();
            return;
        }
        self.toggle_playback(cx);
        if self.playback.is_playing() {
            let bar = self.document.editor.score().measure_at_tick(tick).0 + 1;
            self.status = format!("Otra vez desde el compás {bar} · Espacio para pausar");
        }
        cx.notify();
    }

    pub(crate) fn playline_to_start(&mut self, cx: &mut Context<Self>) {
        self.seek_playback(0, true, cx);
    }

    pub(crate) fn move_by_bar(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.clicked_bar_mark = None;
        if self.document.editor.navigate_bar(forward) {
            self.reveal_selection = true;
            self.audition_selection();
        }
        cx.notify();
    }

    pub(crate) fn open_go_to_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| ui::text_input::TextInput::new("", cx));
        field.focus_handle(cx).focus(window);
        self.go_to_bar = Some(field);
        self.status = format!(
            "Escribe un compás del 1 al {} · Enter va; Esc cierra",
            self.document.editor.score().measures().len()
        );
        cx.notify();
    }

    pub(crate) fn close_go_to_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.go_to_bar = None;
        window.focus(&self.focus);
        cx.notify();
    }

    /// Go to the typed bar; during playback the music jumps there too.
    pub(crate) fn confirm_go_to_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(field) = &self.go_to_bar else {
            return;
        };
        let count = self.document.editor.score().measures().len();
        let bar = field
            .read(cx)
            .value()
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|bar| (1..=count).contains(bar));
        let Some(bar) = bar else {
            self.status = format!("Escribe un compás del 1 al {count}");
            cx.notify();
            return;
        };
        self.close_go_to_bar(window, cx);
        self.clicked_bar_mark = None;
        self.document.editor.go_to_bar(bar - 1);
        self.reveal_selection = true;
        if self.playback.is_playing() || self.playback.is_paused() {
            let tick = self.document.editor.score().measure_start_tick(bar - 1);
            self.seek_playback(tick, true, cx);
        }
        self.status = format!("Compás {bar}");
        cx.notify();
    }
}
