//! Instrument rows and an ensemble picker whose draft never edits the score.
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

pub fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let score = view.document.editor.score();
    div()
        .w(px(LEFT_WIDTH))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(rgb(PANEL))
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .h(px(48.))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Instrumentos  {}", score.parts().len())),
                )
                .child(
                    button(
                        view,
                        "picker-add",
                        "+",
                        view.instrument_picker.is_some(),
                        cx,
                        |v, _, cx| v.open_instrument_picker(cx),
                    )
                    .tooltip(command_hint(view, id::ADD_INSTRUMENT)),
                ),
        )
        .child(
            div()
                .id("instrument-rows")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_2()
                .pb_3()
                .flex()
                .flex_col()
                .gap_1()
                .children(
                    InstrumentFamily::ALL
                        .into_iter()
                        .filter(|family| {
                            score
                                .parts()
                                .iter()
                                .any(|part| part.instrument().family() == *family)
                        })
                        .map(|family| {
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .mb_2()
                                .child(caption(family.label()).mt_3().mb_1().pl_2())
                                .children(
                                    score
                                        .parts()
                                        .iter()
                                        .filter(move |part| part.instrument().family() == family)
                                        .map(|part| part_row(view, part, cx)),
                                )
                        }),
                )
                .child(
                    button(
                        view,
                        "add-instrument-bottom",
                        "+ Añadir instrumentos",
                        false,
                        cx,
                        |v, _, cx| v.open_instrument_picker(cx),
                    )
                    .w_full()
                    .mt_2()
                    .h(px(34.))
                    .border_color(rgb(STRONG_BORDER))
                    .bg(rgb(PANEL))
                    .tooltip(command_hint(view, id::ADD_INSTRUMENT)),
                ),
        )
        .child(
            div()
                .border_t_1()
                .border_color(rgb(BORDER))
                .px_3()
                .py_2()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child("Orden automático por familias"),
        )
        .child(page_navigation(view, cx))
}

fn part_row(view: &AppView, part: &score_core::Part, cx: &Context<AppView>) -> Stateful<Div> {
    let id = part.id();
    let divided = part.bar_has_divisi(view.document.editor.cursor().measure);
    let selected = view.document.editor.cursor().part == id;
    let cursor = view.document.editor.cursor();
    let active_staff = part.displayed_staff(
        cursor.staff,
        cursor.voice,
        cursor.measure,
        view.document.editor.divisi_position().tick,
    );
    let can_remove = view.document.editor.score().parts().len() > 1;
    let name = instrument_name(view.document.editor.score(), id);
    let mut description = if divided {
        format!(
            "Divisi · dos pentagramas en clave de {}",
            clef_name(part.clef_at(StaffId::Upper, cursor.measure))
        )
    } else if part.staves().len() == 2 {
        format!(
            "Claves {} / {}",
            clef_name(part.clef_at(StaffId::Upper, cursor.measure)),
            clef_name(part.clef_at(StaffId::Lower, cursor.measure))
        )
    } else {
        format!(
            "Clave de {}",
            clef_name(part.clef_at(StaffId::Upper, cursor.measure))
        )
    };
    if !part.transposition().is_concert() {
        description.push_str(&format!(
            " · Armadura: {}",
            view.document
                .editor
                .score()
                .written_key_at(id, cursor.measure)
                .label()
        ));
    }
    if part.transposition().is_concert() && !part.instrument().standard_transposition().is_concert()
    {
        description.push_str(" · Altura real");
    }
    div()
        .id(("part-row", u64::from(id.0)))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .rounded_md()
                .border_1()
                .border_color(if selected {
                    rgb(SELECT_BORDER).into()
                } else {
                    transparent_black()
                })
                .bg(rgb(if selected { SELECT } else { PANEL }))
                .child(
                    action(
                        format!("select-part-{}", id.0),
                        div()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(inline_text(view.music_font.clone(), &name)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.))
                                    .text_color(rgb(if selected { ACCENT_TEXT } else { MUTED }))
                                    .child(inline_text(view.music_font.clone(), description)),
                            ),
                        false,
                        cx,
                        move |v, _, cx| v.select_part(id, cx),
                    )
                    .flex_1()
                    .min_w_0()
                    .px_2()
                    .py_2()
                    .border_0()
                    .bg(transparent_black()),
                )
                .child(
                    icon_button(
                        format!("remove-part-{}", id.0),
                        Icon::Trash,
                        false,
                        cx,
                        move |v, _, cx| {
                            if can_remove {
                                v.edit(Command::RemovePartById(id), cx);
                            }
                        },
                    )
                    .mr_1()
                    .bg(transparent_black())
                    .when(!can_remove, |button| button.opacity(0.25).cursor_default())
                    .tooltip(hover_hint(
                        if can_remove {
                            format!("Eliminar {name}")
                        } else {
                            "La partitura debe conservar un instrumento".into()
                        },
                        None,
                    )),
                ),
        )
        .when(
            part.display_staves(view.document.editor.cursor().measure)
                .count()
                == 2,
            |row| {
                row.child(
                    div()
                        .pl_2()
                        .flex()
                        .gap_1()
                        .child(
                            button(
                                view,
                                format!("right-{}", id.0),
                                if divided { "Divisi I" } else { "M. derecha" },
                                selected && active_staff == StaffId::Upper,
                                cx,
                                move |v, _, cx| v.select_hand(id, StaffId::Upper, cx),
                            )
                            .text_size(px(9.))
                            .px_1()
                            .h(px(23.)),
                        )
                        .child(
                            button(
                                view,
                                format!("left-{}", id.0),
                                if divided { "Divisi II" } else { "M. izquierda" },
                                selected && active_staff == StaffId::Lower,
                                cx,
                                move |v, _, cx| v.select_hand(id, StaffId::Lower, cx),
                            )
                            .text_size(px(9.))
                            .px_1()
                            .h(px(23.)),
                        ),
                )
            },
        )
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
