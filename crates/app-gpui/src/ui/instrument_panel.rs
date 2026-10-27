//! The parts panel, by family, in Lista or Mezcla, and an ensemble picker
//! whose draft never edits the score.
use super::icons::icon_sized;
use super::parts_view::{FamilyRows, PartRow, parts_view};
use super::text_input::TextInput;
use super::*;
use score_core::{InstrumentFamily, MAX_PARTS};

pub struct InstrumentPicker {
    counts: [u8; Instrument::ALL.len()],
    pub error: Option<String>,
}

impl Default for InstrumentPicker {
    fn default() -> Self {
        Self {
            counts: [0; Instrument::ALL.len()],
            error: None,
        }
    }
}

impl InstrumentPicker {
    pub fn total(&self) -> usize {
        self.counts.iter().map(|&count| usize::from(count)).sum()
    }

    fn change(&mut self, index: usize, delta: i8, available: usize) {
        let Some(&count) = self.counts.get(index) else {
            return;
        };
        if (delta > 0 && self.total() >= available) || (delta < 0 && count == 0) {
            return;
        }
        self.counts[index] = (i16::from(count) + i16::from(delta)).clamp(0, MAX_PARTS as i16) as u8;
        self.error = None;
    }

    pub fn instruments(&self) -> Vec<Instrument> {
        Instrument::ALL
            .into_iter()
            .zip(self.counts)
            .flat_map(|(instrument, count)| std::iter::repeat_n(instrument, usize::from(count)))
            .collect()
    }
}

fn change_count(view: &mut AppView, index: usize, delta: i8, cx: &mut Context<AppView>) {
    let available = MAX_PARTS.saturating_sub(view.document.editor.score().parts().len());
    if let Some(picker) = &mut view.instrument_picker {
        picker.change(index, delta, available);
        cx.notify();
    }
}

/// The parts field: what it holds filters the list (`UiState::parts_query`).
pub fn search_field(cx: &mut Context<AppView>) -> Entity<TextInput> {
    let field = cx.new(|cx| {
        TextInput::new("", cx)
            .with_max_chars(40)
            .with_placeholder("Buscar parte")
    });
    cx.observe(&field, |view, field, cx| {
        let query = field.read(cx).value();
        if view.ui.parts_query != query {
            view.ui.parts_query = query.to_owned();
            cx.notify();
        }
    })
    .detach();
    field
}

impl AppView {
    pub(crate) fn parts_search_focused(&self, window: &Window, cx: &App) -> bool {
        self.parts_search.focus_handle(cx).is_focused(window)
    }

    /// Enter in the field: the first part found becomes the current one.
    pub(crate) fn choose_found_part(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let score = self.document.editor.score();
        let found = parts_view(
            score,
            self.document.editor.cursor().measure,
            &self.ui.parts_query,
            &[],
        )
        .into_iter()
        .find_map(|family| family.parts.into_iter().next());
        if let Some(row) = found {
            window.focus(&self.focus);
            self.select_part(row.id, cx);
        }
    }

    /// Esc in the field: the whole list again, and the keys back to the score.
    pub(crate) fn clear_parts_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.parts_search
            .update(cx, |field, cx| field.set_value("", cx));
        self.ui.parts_query.clear();
        window.focus(&self.focus);
        cx.notify();
    }
}

pub fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let folded: Vec<String> = view.ui.folded_families.iter().cloned().collect();
    let families = parts_view(
        editor.score(),
        editor.cursor().measure,
        &view.ui.parts_query,
        &folded,
    );
    div()
        .w(px(LEFT_WIDTH))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(rgb(PANEL))
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(head(view, cx))
        .child(
            div()
                .id("instrument-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(6.))
                .pt(px(2.))
                .pb_3()
                .flex()
                .flex_col()
                .when(families.is_empty(), |list| {
                    list.child(
                        div()
                            .px_2()
                            .py_3()
                            .text_size(px(12.))
                            .text_color(rgb(MUTED))
                            .child("Ninguna parte se llama así."),
                    )
                })
                .children(
                    families
                        .into_iter()
                        .map(|family| family_rows(view, family, cx)),
                ),
        )
        .child(footer(view))
}

/// «Partes N» with its «+», Lista | Mezcla and the search field.
fn head(view: &AppView, cx: &Context<AppView>) -> Div {
    div()
        .px_3()
        .pt(px(10.))
        .pb_2()
        .flex()
        .flex_col()
        .gap_2()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .items_baseline()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child("Partes"))
                        .child(
                            div()
                                .text_color(rgb(MUTED))
                                .child(view.document.editor.score().parts().len().to_string()),
                        ),
                )
                .child(
                    toolbar::control(
                        "picker-add",
                        div().text_size(px(16.)).child("+"),
                        view.instrument_picker.is_some(),
                        cx,
                        |v, _, cx| v.open_instrument_picker(cx),
                    )
                    .size(px(26.))
                    .tooltip(command_hint(view, id::ADD_INSTRUMENT)),
                ),
        )
        .child(toolbar::track(PartsMode::ALL.map(|mode| {
            toolbar::control(
                format!("parts-mode-{}", mode.label()),
                mode.label(),
                view.ui.parts_mode == mode,
                cx,
                move |v, _, cx| {
                    v.ui.parts_mode = mode;
                    cx.notify();
                },
            )
            .h(px(24.))
        })))
        .child(view.parts_search.clone())
}

/// A family's header, which folds it, and its rows.
fn family_rows(view: &AppView, family: FamilyRows, cx: &Context<AppView>) -> Div {
    let label = family.label;
    let open = !family.folded;
    let mix = view.ui.parts_mode == PartsMode::Mix;
    div()
        .flex()
        .flex_col()
        .mt(px(6.))
        .child(
            toolbar::control(
                format!("family-{label}"),
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(px(11.5))
                    .text_color(rgb(MUTED))
                    .child(icon_sized(
                        if open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        MUTED,
                        12.,
                    ))
                    .child(div().flex_1().child(label))
                    .child(family.count.to_string()),
                false,
                cx,
                move |v, _, cx| {
                    if !v.ui.folded_families.remove(label) {
                        v.ui.folded_families.insert(label.to_owned());
                    }
                    cx.notify();
                },
            )
            .w_full()
            .justify_start()
            .px(px(6.))
            .py(px(4.))
            .tooltip(hover_hint(
                if open {
                    format!("Plegar {label}")
                } else {
                    format!("Desplegar {label}")
                },
                None,
            )),
        )
        .children(family.parts.into_iter().map(|row| {
            if mix {
                mix_row(view, row, family.family, cx)
            } else {
                list_row(view, row, cx)
            }
        }))
}

/// Name on the left, clefs and transposition on the right.
fn row_line(view: &AppView, row: &PartRow, chosen: bool) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(12.))
                .text_color(rgb(if chosen { ACCENT_TEXT } else { TEXT }))
                .child(inline_text(view.music_font.clone(), &row.name)),
        )
        .when(!row.meta.is_empty(), |line| {
            line.child(
                div()
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(inline_text(view.music_font.clone(), &row.meta)),
            )
        })
}

fn list_row(view: &AppView, row: PartRow, cx: &Context<AppView>) -> Stateful<Div> {
    let id = row.id;
    if view.document.editor.cursor().part != id {
        return toolbar::control(
            format!("part-row-{}", id.0),
            row_line(view, &row, false),
            false,
            cx,
            move |v, _, cx| v.select_part(id, cx),
        )
        .w_full()
        .h(px(28.))
        .pl(px(22.))
        .pr_2()
        .justify_start();
    }
    chosen_row(view, row, cx)
}

/// The current part: the chosen look, its delete button and, with two
/// staves in this bar, which one the cursor writes on.
fn chosen_row(view: &AppView, row: PartRow, cx: &Context<AppView>) -> Stateful<Div> {
    let id = row.id;
    let editor = &view.document.editor;
    let cursor = editor.cursor();
    let can_remove = editor.score().parts().len() > 1;
    let staves = editor.score().part(id).map(|part| {
        let divided = part.bar_has_divisi(cursor.measure);
        let two = part.display_staves(cursor.measure).count() == 2;
        let active = part.displayed_staff(
            cursor.staff,
            cursor.voice,
            cursor.measure,
            editor.divisi_position().tick,
        );
        (divided, two, active)
    });
    div()
        .id(("part-row", u64::from(id.0)))
        .flex()
        .flex_col()
        .rounded(px(7.))
        .border_1()
        .border_color(rgb(SELECT_BORDER))
        .bg(rgb(SELECT))
        .child(
            div()
                .h(px(26.))
                .flex()
                .items_center()
                .child(
                    div()
                        .id(("select-part", u64::from(id.0)))
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .pl(px(21.))
                        .pr_1()
                        .flex()
                        .items_center()
                        .cursor_pointer()
                        .child(row_line(view, &row, true))
                        .on_click(cx.listener(move |v, _, window, cx| {
                            v.focus.focus(window);
                            v.select_part(id, cx);
                        })),
                )
                .child(
                    toolbar::control(
                        format!("remove-part-{}", id.0),
                        icon_sized(Icon::Trash, ACCENT_TEXT, 14.),
                        false,
                        cx,
                        move |v, _, cx| {
                            if can_remove {
                                v.edit(Command::RemovePartById(id), cx);
                            }
                        },
                    )
                    .size(px(24.))
                    .mr(px(1.))
                    .when(!can_remove, |button| button.opacity(0.4).cursor_default())
                    .tooltip(hover_hint(
                        if can_remove {
                            format!("Eliminar {}", row.name)
                        } else {
                            "La partitura debe conservar un instrumento".into()
                        },
                        None,
                    )),
                ),
        )
        .when_some(
            staves.filter(|(_, two, _)| *two),
            |chosen, (divided, _, active)| {
                chosen.child(
                    div().pl(px(21.)).pr_2().pb(px(6.)).child(toolbar::track(
                        [
                            (
                                StaffId::Upper,
                                if divided { "Divisi I" } else { "M. derecha" },
                            ),
                            (
                                StaffId::Lower,
                                if divided { "Divisi II" } else { "M. izquierda" },
                            ),
                        ]
                        .map(|(staff, label)| {
                            let element = match staff {
                                StaffId::Upper => format!("right-{}", id.0),
                                StaffId::Lower => format!("left-{}", id.0),
                            };
                            toolbar::control(
                                element,
                                label,
                                active == staff,
                                cx,
                                move |v, _, cx| v.select_hand(id, staff, cx),
                            )
                            .h(px(22.))
                            .text_size(px(11.))
                        }),
                    )),
                )
            },
        )
}

/// A row in Mezcla: the name, which makes it the current part, then its
/// volume, position and players.
fn mix_row(
    view: &AppView,
    row: PartRow,
    family: InstrumentFamily,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    let id = row.id;
    let chosen = view.document.editor.cursor().part == id;
    div()
        .id(("part-row", u64::from(id.0)))
        .flex()
        .flex_col()
        .gap(px(4.))
        .pl(px(21.))
        .pr_2()
        .py(px(6.))
        .rounded(px(7.))
        .border_1()
        .border_color(if chosen {
            Hsla::from(rgb(SELECT_BORDER))
        } else {
            transparent_black()
        })
        .when(chosen, |row| row.bg(rgb(SELECT)))
        .when(!chosen, |row| row.hover(|s| s.bg(rgb(HOVER))))
        .child(
            div()
                .id(("select-part", u64::from(id.0)))
                .cursor_pointer()
                .child(row_line(view, &row, chosen))
                .on_click(cx.listener(move |v, _, window, cx| {
                    v.focus.focus(window);
                    v.select_part(id, cx);
                })),
        )
        .child(mixer::part_controls(view, id, family, cx))
}

/// One quiet line: the current part and the keys that change it, or in
/// Mezcla the general volume.
fn footer(view: &AppView) -> Div {
    let line = div()
        .flex_shrink_0()
        .border_t_1()
        .border_color(rgb(BORDER))
        .px_3()
        .py_2()
        .flex()
        .justify_between()
        .gap_2()
        .text_size(px(11.5))
        .text_color(rgb(MUTED));
    if view.ui.parts_mode == PartsMode::Mix {
        return line
            .child("Volumen general")
            .child(format!("{} %", view.playback.volume));
    }
    let editor = &view.document.editor;
    let keys: Vec<String> = [id::UPPER_NOTE_OR_STAFF, id::LOWER_NOTE_OR_STAFF]
        .into_iter()
        .filter_map(|command| view.shortcuts.label(command))
        .collect();
    line.child(div().min_w_0().truncate().child(inline_text(
        view.music_font.clone(),
        format!(
            "{} seleccionada",
            editor.score().part_name(editor.cursor().part)
        ),
    )))
    .when(!keys.is_empty(), |line| {
        line.child(
            div()
                .flex_shrink_0()
                .child(format!("{} cambian", keys.join(" "))),
        )
    })
}

pub fn dialog(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let picker = view
        .instrument_picker
        .as_ref()
        .expect("instrument dialog open");
    let total = picker.total();
    let available = MAX_PARTS.saturating_sub(view.document.editor.score().parts().len());
    div().id("instruments-dialog").absolute().top_0().left_0().size_full()
        .flex().items_center().justify_center().bg(rgba(SCRIM)).occlude()
        .child(div().w(px(620.)).max_w(relative(0.94)).h(px(660.)).max_h(relative(0.92))
            .flex().flex_col().bg(rgb(PANEL)).border_1().border_color(rgb(POPOVER_BORDER))
            .rounded_lg().shadow_lg().overflow_hidden()
            .child(div().p_4().flex().items_start().gap_3().border_b_1().border_color(rgb(BORDER))
                .child(div().flex_1().flex().flex_col().gap_2()
                    .child(div().text_size(px(18.)).font_weight(FontWeight::SEMIBOLD).child("Añadir instrumentos"))
                    .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("Elige los instrumentos y ajusta la cantidad de cada uno.")))
                .child(button(view, "instruments-close", "×", false, cx, |v, _, cx| v.close_instrument_picker(cx))
                    .tooltip(hover_hint("Cerrar", None))))
            .child(div().id("instrument-catalog").flex_1().min_h_0().overflow_y_scroll().p_4().flex().flex_col().gap_3()
                .children(InstrumentFamily::ALL.into_iter().map(|family| {
                    div().flex().flex_col().flex_shrink_0().gap_1().child(caption(family.label()).mb_1())
                        .children(catalog(family).into_iter()
                            .map(|(index, instrument)| catalog_row(view, index, instrument, available, cx)))
                })))
            .child(div().p_4().flex().flex_col().gap_3().border_t_1().border_color(rgb(BORDER))
                .when_some(picker.error.clone(), |footer, error| footer.child(div().text_size(px(11.)).text_color(rgb(ERROR)).child(error)))
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(if available == 0 {
                    format!("La partitura ya tiene el máximo de {MAX_PARTS} instrumentos.")
                } else {
                    format!("{total} seleccionados · Puedes añadir hasta {available} instrumentos.")
                }))
                .child(div().flex().justify_end().gap_2()
                    .child(button(view, "instruments-cancel", "Cancelar", false, cx, |v, _, cx| v.close_instrument_picker(cx)).px_3())
                    .child(button(view, "instruments-confirm", match total { 0 => "Añadir instrumentos".into(), 1 => "Añadir 1 instrumento".into(), _ => format!("Añadir {total} instrumentos") },
                        true, cx, |v, _, cx| v.confirm_instruments(cx))
                        .px_3().when(total == 0, |button| button.opacity(0.4).cursor_default())))))
}

fn catalog(family: InstrumentFamily) -> Vec<(usize, Instrument)> {
    let mut result: Vec<_> = Instrument::ALL
        .into_iter()
        .enumerate()
        .filter(|(_, i)| i.family() == family)
        .collect();
    result.sort_by_key(|(_, i)| i.score_order());
    result
}

fn catalog_row(
    view: &AppView,
    index: usize,
    instrument: Instrument,
    available: usize,
    cx: &Context<AppView>,
) -> Div {
    let picker = view
        .instrument_picker
        .as_ref()
        .expect("instrument dialog open");
    let count = picker.counts[index];
    let can_add = picker.total() < available;
    div()
        .flex()
        .flex_shrink_0()
        .min_w_0()
        .items_center()
        .gap_2()
        .rounded_md()
        .bg(rgb(if count > 0 { SELECT } else { FIELD }))
        .child(
            action(
                format!("choose-instrument-{index}"),
                div()
                    .w_full()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .w(px(18.))
                            .flex_shrink_0()
                            .text_color(rgb(if count > 0 { BLUE } else { MUTED }))
                            .child(if count > 0 { "☑" } else { "☐" }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .justify_center()
                            .overflow_hidden()
                            .child(
                                inline_text(view.music_font.clone(), instrument.name()).w_full(),
                            ),
                    ),
                false,
                cx,
                move |v, _, cx| {
                    let available =
                        MAX_PARTS.saturating_sub(v.document.editor.score().parts().len());
                    if let Some(picker) = &mut v.instrument_picker {
                        if picker.counts[index] > 0 {
                            picker.counts[index] = 0;
                            picker.error = None;
                        } else {
                            picker.change(index, 1, available);
                        }
                        cx.notify();
                    }
                },
            )
            .flex_1()
            .min_w_0()
            .h(px(48.))
            .px_2()
            .bg(transparent_black()),
        )
        .child(
            button(
                view,
                format!("instrument-minus-{index}"),
                "−",
                false,
                cx,
                move |v, _, cx| change_count(v, index, -1, cx),
            )
            .w(px(28.))
            .when(count == 0, |button| button.opacity(0.3).cursor_default()),
        )
        .child(
            div()
                .w(px(20.))
                .text_center()
                .text_size(px(12.))
                .child(count.to_string()),
        )
        .child(
            button(
                view,
                format!("instrument-plus-{index}"),
                "+",
                false,
                cx,
                move |v, _, cx| change_count(v, index, 1, cx),
            )
            .w(px(28.))
            .mr_2()
            .when(!can_add, |button| button.opacity(0.3).cursor_default()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn ensemble_draft_supports_duplicates_and_enforces_the_remaining_slots() {
        let mut picker = InstrumentPicker::default();
        picker.change(0, -1, 3);
        picker.change(0, 1, 3);
        picker.change(0, 1, 3);
        picker.change(1, 1, 3);
        picker.change(2, 1, 3);
        assert_eq!(
            picker.instruments(),
            [Instrument::Piano, Instrument::Piano, Instrument::Guitar]
        );
        picker.change(0, -1, 3);
        picker.change(2, 1, 3);
        assert_eq!(
            picker.instruments(),
            [Instrument::Piano, Instrument::Guitar, Instrument::Violin]
        );
        let mut full = InstrumentPicker::default();
        full.change(0, 1, 0);
        assert!(full.instruments().is_empty());
    }
    #[test]
    fn catalog_is_sorted_in_orchestral_order() {
        assert!(
            catalog(InstrumentFamily::Woodwinds)
                .windows(2)
                .all(|w| w[0].1.score_order() <= w[1].1.score_order())
        );
    }
}
