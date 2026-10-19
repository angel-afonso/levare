//! GPUI adapter of the toolbar model: paints `ToolbarView` and the File menu
//! and turns every click into a command. No decisions are taken here — what
//! exists, its state and what folds all come from `score_toolbar`.

use super::*;
use score_toolbar::{
    ControlView, FILE_MENU, GroupMode, GroupView, Item, MenuItemView, MenuRow, Metrics, RecentItem,
    TABS, TabId, ToolbarView, Visual, project, project_menu,
};

const CONTROL_HEIGHT: f32 = 62.;
const ROW_HEIGHT: f32 = 20.;
const GROUP_HEIGHT: f32 = 92.;

fn visual(view: &AppView, visual: Visual, size: f32, color: u32) -> Option<AnyElement> {
    match visual {
        Visual::Icon(name) => Icon::named(name).map(|glyph| icon(glyph, color).into_any_element()),
        Visual::Music(text) => {
            Some(music_symbol(view, text, size, (size * 0.7, size * 0.8), color).into_any_element())
        }
        Visual::Text(text) => Some(
            div()
                .font_family("Times New Roman")
                .italic()
                .text_size(px(size * 0.5))
                .text_color(rgb(color))
                .child(text)
                .into_any_element(),
        ),
        Visual::None => None,
    }
}

fn ink(control: &ControlView) -> u32 {
    if control.checked {
        BLUE
    } else if control.pending {
        MUTED
    } else {
        TEXT
    }
}

/// Every control shares this frame: same corners, hover, checked colour and
/// click path (`AppView::run`). `close` also folds an open dropdown.
fn frame(
    element: String,
    content: impl IntoElement,
    control: &ControlView,
    cx: &Context<AppView>,
    close: bool,
) -> Stateful<Div> {
    let command = control.command;
    action(
        element,
        content,
        control.checked,
        cx,
        move |v, window, cx| {
            if close {
                v.ui.open_group = None;
                v.ui.file_menu = false;
            }
            v.run(command, window, cx)
        },
    )
    .when(!control.checked, |frame| frame.bg(rgb(PANEL)))
    .text_color(rgb(ink(control)))
    .when(!control.enabled, |frame| frame.opacity(0.4))
    .tooltip(hover_hint(control.description, control.keys.clone()))
}

fn large(view: &AppView, control: &ControlView, cx: &Context<AppView>) -> Stateful<Div> {
    frame(
        format!("tool-{}", control.command),
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .children(visual(view, control.visual, 30., ink(control)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_size(px(11.5))
                    .child(control.label.clone()),
            ),
        control,
        cx,
        false,
    )
    .min_w(px(Metrics::default().large))
    .h(px(CONTROL_HEIGHT))
    .px_1()
}

/// A musical glyph; `named` adds its name when the group has room.
fn tile(
    view: &AppView,
    control: &ControlView,
    cx: &Context<AppView>,
    named: bool,
) -> Stateful<Div> {
    let metrics = Metrics::default();
    frame(
        format!("tool-{}", control.command),
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .children(visual(
                view,
                control.visual,
                if named { 26. } else { 30. },
                ink(control),
            ))
            .when(named, |tile| {
                tile.child(
                    div()
                        .text_size(px(10.))
                        .line_height(px(11.))
                        .child(control.label.clone()),
                )
            }),
        control,
        cx,
        false,
    )
    .when(named, |tile| tile.min_w(px(metrics.labeled_tile)).px_1())
    .when(!named, |tile| tile.w(px(metrics.tile)))
    .h(px(CONTROL_HEIGHT))
}

fn row(
    view: &AppView,
    control: &ControlView,
    cx: &Context<AppView>,
    prefix: &str,
    close: bool,
) -> Stateful<Div> {
    frame(
        format!("{prefix}-{}", control.command),
        div()
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .child(div().w(px(16.)).flex().justify_center().children(visual(
                view,
                control.visual,
                18.,
                ink(control),
            )))
            .child(div().flex_1().truncate().child(control.label.clone()))
            .when(close, |row| {
                row.children(control.keys.clone().map(shortcut))
            }),
        control,
        cx,
        close,
    )
    .justify_start()
    .w_full()
    .h(px(ROW_HEIGHT))
    .px_1()
    .text_size(px(11.5))
}

fn items(view: &AppView, group: &GroupView, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Metrics::default().gap))
        .h(px(CONTROL_HEIGHT))
        .children(group.items.iter().map(|item| {
            match item {
                Item::Large(control) => large(view, control, cx).into_any_element(),
                Item::Tile(control) => tile(view, control, cx, false).into_any_element(),
                Item::LabeledTile(control) => tile(view, control, cx, true).into_any_element(),
                Item::Column(column) => div()
                    .flex()
                    .flex_col()
                    .gap(px(1.))
                    .children(
                        column
                            .iter()
                            .map(|control| row(view, control, cx, "tool", false)),
                    )
                    .into_any_element(),
            }
        }))
}

fn popover(content: impl IntoElement) -> Div {
    div()
        .p_1()
        .flex()
        .flex_col()
        .gap(px(1.))
        .bg(rgb(0x1d1e23))
        .border_1()
        .border_color(rgb(0x3a3d46))
        .rounded_lg()
        .shadow_lg()
        .child(content)
}

fn folded(view: &AppView, group: &GroupView, index: usize, cx: &Context<AppView>) -> Div {
    let open = view.ui.open_group == Some(index);
    let title = group.title;
    div()
        .flex()
        .flex_col()
        .child(
            action(
                format!("fold-{title}"),
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .children(visual(
                        view,
                        group.visual,
                        30.,
                        if open { BLUE } else { TEXT },
                    ))
                    .child(div().text_size(px(11.5)).child(format!("{title} ▾"))),
                open,
                cx,
                move |v, _, cx| {
                    v.ui.open_group = if open { None } else { Some(index) };
                    v.ui.file_menu = false;
                    cx.notify();
                },
            )
            .when(!open, |button| button.bg(rgb(PANEL)))
            .min_w(px(Metrics::default().folded))
            .h(px(CONTROL_HEIGHT))
            .px_1(),
        )
        .when(open, |column| {
            column.child(deferred(
                // Absolute children start at the column's corner: drop below the button.
                anchored()
                    .offset(point(px(0.), px(CONTROL_HEIGHT + 4.)))
                    .snap_to_window()
                    .child(
                        popover(
                            div().w(px(240.)).flex().flex_col().gap(px(1.)).children(
                                group.controls.iter().map(|control| {
                                    row(view, control, cx, "folded", true).h(px(26.))
                                }),
                            ),
                        )
                        .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                            v.ui.open_group = None;
                            cx.notify();
                        })),
                    ),
            ))
        })
}

fn group(view: &AppView, group: &GroupView, index: usize, cx: &Context<AppView>) -> Div {
    div()
        .h(px(GROUP_HEIGHT))
        .px_2()
        .flex()
        .flex_col()
        .justify_between()
        .flex_shrink_0()
        .border_r_1()
        .border_color(rgb(BORDER))
        .child(match group.mode {
            GroupMode::Full | GroupMode::Expanded => items(view, group, cx),
            GroupMode::Folded => folded(view, group, index, cx),
        })
        .child(
            div()
                .text_center()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .child(group.title),
        )
}

pub fn toolbar_view(view: &AppView) -> ToolbarView {
    project(
        score_toolbar::tab(view.ui.tab),
        &view.shortcuts.registry,
        &view.shortcuts.keymap,
        view,
        view.ui.width - 20.,
        &Metrics::default(),
    )
}

/// The ribbon row. It never scrolls: groups fold instead.
pub fn ribbon(view: &AppView, cx: &Context<AppView>) -> Div {
    let toolbar = toolbar_view(view);
    div()
        .h(px(GROUP_HEIGHT + 12.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .px(px(10.))
        .overflow_hidden()
        .bg(rgb(PANEL))
        .border_b_1()
        .border_color(rgb(BORDER))
        .when(toolbar.groups.is_empty(), |row| {
            row.child(
                div()
                    .p_4()
                    .text_size(px(11.))
                    .text_color(rgb(MUTED))
                    .child(format!("{} · herramientas pendientes", toolbar.tab.label())),
            )
        })
        .children(
            toolbar
                .groups
                .iter()
                .enumerate()
                .map(|(index, view_group)| group(view, view_group, index, cx)),
        )
}

fn menu_row(view: &AppView, item: &MenuItemView, cx: &Context<AppView>) -> Stateful<Div> {
    let command = item.command;
    action(
        format!("file-{command}"),
        div()
            .flex()
            .items_center()
            .gap_3()
            .w_full()
            .child(div().flex_1().child(item.label.clone()))
            .children(
                item.keys
                    .clone()
                    .map(|keys| div().text_size(px(11.)).text_color(rgb(MUTED)).child(keys)),
            ),
        false,
        cx,
        move |v, window, cx| {
            v.ui.file_menu = false;
            v.run(command, window, cx)
        },
    )
    .justify_start()
    .w_full()
    .h(px(28.))
    .px_2()
    .bg(rgb(0x1d1e23))
    .text_size(px(12.5))
    .text_color(rgb(if item.pending { MUTED } else { TEXT }))
    .when(!item.enabled, |row| row.opacity(0.4))
    .when(view.busy, |row| row.opacity(0.4))
}

fn recent_rows(view: &AppView) -> Vec<RecentItem> {
    view.recent
        .files()
        .iter()
        .enumerate()
        .map(|(index, path)| RecentItem {
            index,
            title: crate::shortcuts::recent_title(path),
            detail: crate::shortcuts::recent_folder(path),
        })
        .collect()
}

fn separator() -> Div {
    div().h(px(1.)).mx_2().my_1().bg(rgb(BORDER))
}

/// Archivo: a dropdown from the tab row, built from `FILE_MENU`.
fn file_menu(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let rows = project_menu(
        FILE_MENU,
        &view.shortcuts.registry,
        &view.shortcuts.keymap,
        view,
        &recent_rows(view),
    );
    let recent_open = view.ui.recent_menu;
    deferred(
        anchored()
            .offset(point(px(0.), px(30.)))
            .snap_to_window()
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_1()
                    .child(popover(
                        div()
                            .w(px(300.))
                            .flex()
                            .flex_col()
                            .gap(px(1.))
                            .children(rows.iter().map(|row| {
                                match row {
                                    MenuRow::Item(item) => {
                                        menu_row(view, item, cx).into_any_element()
                                    }
                                    MenuRow::Separator => separator().into_any_element(),
                                    MenuRow::Recent { label, .. } => action(
                                        "file-recent",
                                        div()
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .child(div().flex_1().child(*label))
                                            .child(div().text_color(rgb(MUTED)).child("›")),
                                        recent_open,
                                        cx,
                                        move |v, _, cx| {
                                            v.ui.recent_menu = !recent_open;
                                            cx.notify();
                                        },
                                    )
                                    .justify_start()
                                    .w_full()
                                    .h(px(28.))
                                    .px_2()
                                    .when(!recent_open, |row| row.bg(rgb(0x1d1e23)))
                                    .text_size(px(12.5))
                                    .into_any_element(),
                                }
                            })),
                    ))
                    .when(recent_open, |menus| {
                        let recent = rows.iter().find_map(|row| match row {
                            MenuRow::Recent { files, clear, .. } => Some((files, clear)),
                            _ => None,
                        });
                        menus.children(recent.map(|(files, clear)| {
                            popover(
                                div()
                                    .w(px(340.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(1.))
                                    .child(caption("RECIENTES").px_2().py_1())
                                    .when(files.is_empty(), |list| {
                                        list.child(
                                            div()
                                                .px_2()
                                                .py_2()
                                                .text_size(px(11.5))
                                                .text_color(rgb(MUTED))
                                                .child(
                                                    "Todavía no has abierto ni guardado partituras",
                                                ),
                                        )
                                    })
                                    .children(files.iter().map(|file| {
                                        let index = file.index;
                                        action(
                                            format!("recent-{index}"),
                                            div()
                                                .flex()
                                                .flex_col()
                                                .w_full()
                                                .child(
                                                    div()
                                                        .text_size(px(12.5))
                                                        .truncate()
                                                        .child(file.title.clone()),
                                                )
                                                .child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(rgb(MUTED))
                                                        .truncate()
                                                        .child(file.detail.clone()),
                                                ),
                                            false,
                                            cx,
                                            move |v, window, cx| {
                                                v.ui.file_menu = false;
                                                if let Some(path) =
                                                    v.recent.files().get(index).cloned()
                                                {
                                                    v.open_recent(path, window, cx);
                                                }
                                            },
                                        )
                                        .justify_start()
                                        .w_full()
                                        .h(px(40.))
                                        .px_2()
                                        .bg(rgb(0x1d1e23))
                                    }))
                                    .child(separator())
                                    .child(menu_row(view, clear, cx)),
                            )
                        }))
                    })
                    .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                        v.ui.file_menu = false;
                        cx.notify();
                    })),
            ),
    )
    .with_priority(1)
}

/// The tab row: the Archivo dropdown, then one tab per `TabId`.
pub fn tabs(view: &AppView, cx: &Context<AppView>) -> Div {
    let open = view.ui.file_menu;
    div()
        .h(px(35.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .bg(rgb(PANEL))
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            div()
                .flex()
                .items_center()
                .min_w_0()
                .px_2()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            action(
                                "file-menu",
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child("Archivo")
                                    .child("▾"),
                                open,
                                cx,
                                move |v, _, cx| {
                                    v.ui.file_menu = !open;
                                    v.ui.recent_menu = false;
                                    v.ui.open_group = None;
                                    cx.notify();
                                },
                            )
                            .h(px(26.))
                            .px_3()
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(!open, |button| button.bg(rgb(PANEL))),
                        )
                        .when(open, |column| column.child(file_menu(view, cx))),
                )
                .child(div().w(px(1.)).h(px(18.)).mx_1().bg(rgb(BORDER)))
                .children(TABS.iter().map(|spec| {
                    let id: TabId = spec.id;
                    let selected = view.ui.tab == id;
                    action(
                        format!("tab-{}", id.label()),
                        div().child(id.label()),
                        false,
                        cx,
                        move |v, _, cx| {
                            v.ui.tab = id;
                            v.ui.open_group = None;
                            cx.notify();
                        },
                    )
                    .h(px(35.))
                    .px_3()
                    .rounded_none()
                    .bg(rgb(PANEL))
                    .border_0()
                    .border_b_2()
                    .border_color(if selected {
                        Hsla::from(rgb(BLUE))
                    } else {
                        transparent_black()
                    })
                    .text_color(rgb(if selected { TEXT } else { MUTED }))
                })),
        )
        .child(
            command_button(
                "entry-state",
                if view.ui.note_entry {
                    "●  Entrada activa"
                } else {
                    "Entrada desactivada"
                },
                view.ui.note_entry,
                view,
                cx,
                id::TOGGLE_NOTE_INPUT,
            )
            .mr_3()
            .h(px(23.))
            .rounded_full()
            .text_size(px(10.)),
        )
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the GPUI glob would shadow `#[test]` with `gpui::test`.
    use score_toolbar::{TABS, Visual};

    #[test]
    fn every_music_glyph_of_the_toolbar_is_preloaded() {
        let font = crate::music_font::MusicFont::new();
        for tab in TABS {
            for group in tab.groups {
                let visuals = std::iter::once(group.visual)
                    .chain(group.controls.iter().map(|control| control.visual));
                for visual in visuals {
                    if let Visual::Music(text) = visual {
                        assert!(font.covers(text), "{}: {text:?}", group.title);
                    }
                }
            }
        }
    }
}
