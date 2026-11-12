//! The start view (`crate::start`): templates on the left, recent scores and
//! examples on the right, opening and the empty score at the bottom. Drawn
//! as the other dialogs are, over the empty score it leaves when closed.

use super::*;
use crate::start::{EXAMPLES, Entry};
use score_core::TEMPLATES;
use score_keymap::KeySequence;

const ROW_HEIGHT: f32 = 44.;

fn key(name: &str) -> String {
    KeySequence::parse(name).map_or_else(|_| name.to_owned(), |keys| keys.label())
}

/// A row with a name and, under it, what it holds; `aside` sits on the right.
fn entry_row(
    view: &AppView,
    entry: Entry,
    name: &str,
    detail: &str,
    aside: Option<String>,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    let selected = view
        .start
        .as_ref()
        .is_some_and(|start| start.selected == entry);
    let element = match entry {
        Entry::Template(index) => format!("start-template-{index}"),
        Entry::Recent(index) => format!("start-recent-{index}"),
        Entry::Example(index) => format!("start-example-{index}"),
        Entry::Open => "start-open".into(),
        Entry::Empty => "start-empty".into(),
    };
    action(
        element,
        div()
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        inline_text(view.music_font.clone(), name)
                            .text_size(px(12.5))
                            .whitespace_nowrap(),
                    )
                    .child(
                        inline_text(view.music_font.clone(), detail)
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .whitespace_nowrap(),
                    ),
            )
            .children(aside.map(|aside| {
                div()
                    .flex_shrink_0()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(aside)
            })),
        selected,
        cx,
        move |v, window, cx| v.choose_start(entry, window, cx),
    )
    .flex_shrink_0()
    .justify_start()
    .w_full()
    .h(px(ROW_HEIGHT))
    .px_2()
}

fn templates(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let start = view.start.as_ref().expect("start view open");
    div()
        .id("start-templates")
        .flex_1()
        .min_w_0()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&start.templates)
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(caption("Partitura nueva").mb_1())
        .children(TEMPLATES.iter().enumerate().map(|(index, template)| {
            let parts = template.instruments.len();
            entry_row(
                view,
                Entry::Template(index),
                template.name,
                template.detail,
                Some(if parts == 1 {
                    "1 parte".into()
                } else {
                    format!("{parts} partes")
                }),
                cx,
            )
        }))
}

fn sources(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let start = view.start.as_ref().expect("start view open");
    let recent = view.recent.files();
    div()
        .id("start-sources")
        .w(px(330.))
        .flex_shrink_0()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&start.sources)
        .p_3()
        .flex()
        .flex_col()
        .gap_1()
        .child(caption("Recientes").mb_1())
        .when(recent.is_empty(), |column| {
            column.child(
                div()
                    .flex_shrink_0()
                    .py_2()
                    .text_size(px(11.5))
                    .text_color(rgb(MUTED))
                    .child("Todavía no has abierto ni guardado partituras"),
            )
        })
        .children(recent.iter().enumerate().map(|(index, path)| {
            entry_row(
                view,
                Entry::Recent(index),
                &crate::shortcuts::recent_title(path),
                &crate::shortcuts::recent_folder(path),
                None,
                cx,
            )
        }))
        .child(caption("Ejemplos").mt_3().mb_1())
        .children(EXAMPLES.iter().enumerate().map(|(index, example)| {
            entry_row(
                view,
                Entry::Example(index),
                example.name,
                example.detail,
                None,
                cx,
            )
            .tooltip(hover_hint(
                "Se abre como copia sin título; el ejemplo no cambia",
                None,
            ))
        }))
}

fn footer(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let start = view.start.as_ref().expect("start view open");
    let selected = start.selected;
    let preferences = view
        .shortcuts
        .spec(id::PREFERENCES)
        .map_or("Preferencias de atajos…", |spec| spec.label);
    let open_keys = view.shortcuts.label(id::OPEN);
    div()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .border_t_1()
        .border_color(rgb(BORDER))
        .when_some(start.message.clone(), |footer, message| {
            footer.child(
                div()
                    .text_size(px(11.5))
                    .text_color(rgb(WARNING))
                    .child(message),
            )
        })
        .child(
            div()
                .flex()
                .items_end()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(rgb(SECONDARY))
                                .child(format!(
                                    "{} busca cualquier orden por su nombre. Los atajos están en \
                                     Archivo › {preferences}",
                                    view.shortcuts.hint(id::COMMAND_SEARCH)
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED))
                                .child(format!(
                                    "Flechas y {} para elegir · {} abre · {} vuelve a la partitura",
                                    key("tab"),
                                    key("enter"),
                                    key("escape")
                                )),
                        )
                        .child(
                            div()
                                .id("start-at-launch")
                                .mt_1()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(11.5))
                                .cursor_pointer()
                                .child(
                                    div()
                                        .w(px(14.))
                                        .text_color(rgb(if start.show_at_launch {
                                            BLUE
                                        } else {
                                            MUTED
                                        }))
                                        .child(if start.show_at_launch { "☑" } else { "☐" }),
                                )
                                .child("Mostrar al abrir Levare")
                                .on_click(cx.listener(|v, _, window, cx| {
                                    v.focus.focus(window);
                                    v.toggle_start_at_launch(cx);
                                })),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .gap_2()
                        .child(
                            button(
                                view,
                                "start-open",
                                "Abrir…",
                                selected == Entry::Open,
                                cx,
                                |v, window, cx| v.choose_start(Entry::Open, window, cx),
                            )
                            .px_3()
                            .tooltip(hover_hint("Abrir una partitura o un MusicXML", open_keys)),
                        )
                        .child(
                            button(
                                view,
                                "start-empty",
                                "Partitura vacía",
                                selected == Entry::Empty,
                                cx,
                                |v, window, cx| v.choose_start(Entry::Empty, window, cx),
                            )
                            .px_3()
                            .tooltip(hover_hint("Piano a dos manos, sin título", None)),
                        ),
                ),
        )
}

pub fn dialog(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    div()
        .id("start-dialog")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(SCRIM))
        .occlude()
        .child(
            div()
                .w(px(880.))
                .max_w(relative(0.94))
                .h(px(640.))
                .max_h(relative(0.92))
                .flex()
                .flex_col()
                .bg(rgb(PANEL))
                .border_1()
                .border_color(rgb(POPOVER_BORDER))
                .rounded_lg()
                .shadow_lg()
                .overflow_hidden()
                .child(
                    div()
                        .p_4()
                        .flex()
                        .items_start()
                        .gap_3()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(
                                    div()
                                        .text_size(px(18.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child("Empezar"),
                                )
                                .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child(
                                    "Elige un conjunto para una partitura nueva, abre una \
                                         reciente o mira un ejemplo.",
                                )),
                        )
                        .child(
                            button(view, "start-close", "×", false, cx, |v, _, cx| {
                                v.close_start(cx)
                            })
                            .tooltip(hover_hint("Cerrar", Some(key("escape")))),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(templates(view, cx))
                        .child(sources(view, cx)),
                )
                .child(footer(view, cx)),
        )
}
