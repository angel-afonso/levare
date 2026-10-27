//! Title-bar command search: a field plus a popover of ranked results.
use super::*;
use text_input::TextInput;

pub struct CommandSearch {
    pub field: Entity<TextInput>,
    pub selected: usize,
}
impl CommandSearch {
    pub fn new(cx: &mut Context<AppView>) -> Self {
        Self {
            field: cx.new(|cx| TextInput::new("", cx)),
            selected: 0,
        }
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }
}

/// The title-bar box: a button while closed, the field with its results open.
pub(super) fn search_box(view: &AppView, cx: &Context<AppView>) -> Div {
    let width = if view.ui.compact { 130. } else { 270. };
    let frame = div().relative().w(px(width)).h(px(29.));
    let Some(search) = &view.command_search else {
        return frame.child(
            div()
                .id("command-search")
                .size_full()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(HOVER)))
                .tooltip(command_hint(view, id::COMMAND_SEARCH))
                .on_click(cx.listener(|v, _, window, cx| v.run(id::COMMAND_SEARCH, window, cx)))
                .child(icon(Icon::Search, MUTED))
                .child(
                    div()
                        .flex_1()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .truncate()
                        .child(if view.ui.compact {
                            "Buscar…"
                        } else {
                            "Buscar comandos…"
                        }),
                )
                .children(view.shortcuts.label(id::COMMAND_SEARCH).map(shortcut)),
        );
    };
    let results = view.command_results(cx);
    let typed = !search.field.read(cx).value().trim().is_empty();
    let selected = search.selected.min(results.len().saturating_sub(1));
    frame.child(search.field.clone()).child(deferred(
        anchored()
            .offset(point(px(0.), px(34.)))
            .snap_to_window()
            .child(
                toolbar::popover(
                    div()
                        .w(px(440.))
                        .flex()
                        .flex_col()
                        .children(results.iter().enumerate().map(|(index, result)| {
                            let command = result.item.command;
                            div()
                                .id(SharedString::from(format!("search-result-{index}")))
                                .h(px(40.))
                                .px_2()
                                .flex()
                                .items_center()
                                .gap_2()
                                .rounded_md()
                                .cursor_pointer()
                                .bg(rgb(if index == selected { SELECT } else { POPOVER }))
                                .hover(|style| style.bg(rgb(HOVER)))
                                .on_click(cx.listener(move |v, _, window, cx| {
                                    v.run_search_result(command, window, cx)
                                }))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .text_color(rgb(TEXT))
                                                .truncate()
                                                .child(result.item.label.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(10.5))
                                                .text_color(rgb(MUTED))
                                                .truncate()
                                                .child(result.place.clone()),
                                        ),
                                )
                                .when(result.item.pending, |row| row.child(pending_mark()))
                                .children(result.item.keys.clone().map(shortcut))
                        }))
                        .when(results.is_empty() && typed, |list| {
                            list.child(
                                div()
                                    .h(px(40.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .text_size(px(12.5))
                                    .text_color(rgb(MUTED))
                                    .child("Ningún comando coincide"),
                            )
                        }),
                )
                .occlude()
                .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                    v.command_search = None;
                    cx.notify();
                })),
            ),
    ))
}
