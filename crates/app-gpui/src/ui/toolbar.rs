//! GPUI adapter of the toolbar model: paints `ToolbarView` and the File menu
//! and turns every click into a command. No decisions are taken here: what
//! exists, its state and what folds all come from `score_toolbar`.

use super::*;
use score_toolbar::{
    BAR_MARK_CONTEXT_MENU, ControlView, FILE_MENU, GroupMode, GroupView, Item, MARK_CONTEXT_MENU,
    MenuItemView, MenuRow, Metrics, NOTE_CONTEXT_MENU, RecentItem, Role, TABS, TabId, ToolbarView,
    Visual, project, project_menu, tile_width,
};

const CONTROL_HEIGHT: f32 = 70.;
const ROW_HEIGHT: f32 = 18.;
const GROUP_HEIGHT: f32 = 88.;

fn visual(view: &AppView, visual: Visual, size: f32, color: u32) -> Option<AnyElement> {
    match visual {
        Visual::Icon(name) => {
            Icon::named(name).map(|glyph| icons::icon_sized(glyph, color, size).into_any_element())
        }
        Visual::Music(text) => {
            // Same ink box for every glyph, but a ceiling so small marks
            // (staccato, tenuto) are not blown up out of proportion.
            let frame = (size * 0.9, size);
            let font_size = fitted_music_font_size(&view.music_font, text, frame).min(size * 1.6);
            Some(music_symbol(view, text, font_size, frame, color).into_any_element())
        }
        Visual::Clef(clef) => {
            Some(clef_symbol(view, clef, (size * 0.7, size), color).into_any_element())
        }
        Visual::Harmonic(harmonic) => {
            let text = harmonic.glyph();
            let height = f32::from(view.music_font.bounds(text, size).size.height).max(1.);
            let font_size = size * (size * 0.6 / height);
            Some(
                music_symbol(view, text, font_size, (size * 0.7, size * 0.8), color)
                    .into_any_element(),
            )
        }
        Visual::Chord => Some(music_chord(view, size, color).into_any_element()),
        Visual::Tie | Visual::Slur => {
            Some(music_connection(view, size, color, visual == Visual::Slur).into_any_element())
        }
        Visual::Staff | Visual::Bar => {
            Some(music_staff(view, size, color, visual == Visual::Bar).into_any_element())
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
        Visual::Label(text) => Some(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_size(px(size * 0.55))
                .text_color(rgb(color))
                .child(text)
                .into_any_element(),
        ),
        Visual::None => None,
    }
}

fn ink(control: &ControlView) -> u32 {
    if control.checked && control.role == Role::Choice {
        ON_ACCENT
    } else if control.checked {
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
    action_with_hover(
        element,
        content,
        control.checked,
        |s| s.bg(rgb(HOVER)),
        cx,
        move |v, window, cx| {
            v.ui.ribbon_peek = false;
            if close {
                v.ui.open_group = None;
                v.ui.file_menu = false;
            }
            v.run(command, window, cx)
        },
    )
    .rounded(px(6.))
    .when(!control.checked, |frame| frame.bg(transparent_black()))
    .when(control.checked && control.role == Role::Choice, |frame| {
        frame.bg(rgb(CHOSEN))
    })
    .when(!control.checked && control.role != Role::Action, |frame| {
        frame.border_color(rgb(OUTLINE))
    })
    .when(control.pending, |frame| {
        frame.border_dashed().border_color(rgb(PENDING_OUTLINE))
    })
    .text_color(rgb(ink(control)))
    .when(!control.enabled, |frame| frame.opacity(0.4))
    .tooltip(hover_hint(
        if control.pending {
            format!("{} · Pendiente", control.description)
        } else {
            control.description.to_string()
        },
        control.keys.clone(),
    ))
}

/// A glyph over its name (when the group has room) and, under it, its key.
fn tile(
    view: &AppView,
    control: &ControlView,
    cx: &Context<AppView>,
    named: bool,
) -> Stateful<Div> {
    frame(
        format!("tool-{}", control.command),
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .line_height(px(12.))
            .whitespace_nowrap()
            .children(visual(view, control.visual, 26., ink(control)))
            .when(named, |tile| {
                tile.child(div().text_size(px(10.5)).child(control.label.clone()))
            })
            .children(
                control
                    .keys
                    .clone()
                    .map(|keys| div().text_size(px(10.)).text_color(rgb(MUTED)).child(keys)),
            ),
        control,
        cx,
        false,
    )
    .w(px(tile_width(control, named, &Metrics::default())))
    .h(px(CONTROL_HEIGHT))
}

/// Consecutive choice tiles as one sunken bar; the chosen one is filled.
fn segment(view: &AppView, controls: &[ControlView], named: bool, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(8.))
        .bg(rgb(SUNKEN))
        .children(controls.iter().map(|control| {
            tile(view, control, cx, named && !control.symbol_only)
                .h(px(CONTROL_HEIGHT - 4.))
                .when(!control.checked && !control.pending, |tile| {
                    tile.border_color(transparent_black())
                })
        }))
}

fn row(
    view: &AppView,
    control: &ControlView,
    cx: &Context<AppView>,
    prefix: &str,
    close: bool,
) -> Stateful<Div> {
    let named = close || !control.symbol_only;
    frame(
        format!("{prefix}-{}", control.command),
        div()
            .flex()
            .items_center()
            .gap_2()
            .w_full()
            .child(
                div()
                    .when(named, |slot| slot.w(px(if close { 40. } else { 16. })))
                    .when(!named, |slot| slot.w_full())
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(if close && let Visual::Music(text) = control.visual {
                        Some(
                            fitted_music_symbol(view, text, (36., 22.), ink(control))
                                .into_any_element(),
                        )
                    } else {
                        visual(
                            view,
                            control.visual,
                            if !named && matches!(control.visual, Visual::Text(_)) {
                                22.
                            } else {
                                16.
                            },
                            ink(control),
                        )
                    }),
            )
            .when(named, |row| {
                row.child(div().flex_1().truncate().child(control.label.clone()))
            })
            .when(named && control.pending, |row| row.child(pending_mark()))
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
    .text_size(px(11.))
}

fn items(view: &AppView, group: &GroupView, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(Metrics::default().gap))
        .h(px(CONTROL_HEIGHT))
        .children(group.items.iter().map(|item| {
            match item {
                Item::Tile(control) => tile(view, control, cx, false).into_any_element(),
                Item::LabeledTile(control) => tile(view, control, cx, true).into_any_element(),
                Item::Segment { controls, named } => segment(view, controls, *named, cx)
                    .w(px(item.width(&Metrics::default())))
                    .into_any_element(),
            }
        }))
}

pub(super) fn popover(content: impl IntoElement) -> Div {
    div()
        .p_1()
        .flex()
        .flex_col()
        .gap(px(1.))
        .bg(rgb(POPOVER))
        .border_1()
        .border_color(rgb(POPOVER_BORDER))
        .rounded_lg()
        .shadow_lg()
        .child(content)
}

fn folded(view: &AppView, group: &GroupView, index: usize, cx: &Context<AppView>) -> Div {
    let open = view.ui.open_group == Some(index);
    let title = group.title;
    let active = open || group.controls.iter().any(|control| control.checked);
    let color = if active { BLUE } else { TEXT };
    let glyph = |visual_of: Visual| {
        div()
            .h(px(30.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .children(match visual_of {
                Visual::Music(text) => {
                    Some(fitted_music_symbol(view, text, (40., 30.), color).into_any_element())
                }
                _ => visual(view, visual_of, 26., color),
            })
    };
    let toggle_list = move |v: &mut AppView, _: &mut Window, cx: &mut Context<AppView>| {
        v.ui.open_group = if open { None } else { Some(index) };
        v.ui.file_menu = false;
        cx.notify();
    };
    // A split button repeats the last option used; the arrow opens the list.
    let target = group
        .dropdown
        .then(|| {
            view.ui
                .last_choice
                .get(title)
                .and_then(|last| group.controls.iter().find(|c| c.command == *last))
                .or_else(|| group.controls.first())
        })
        .flatten();
    let button = if let Some(target) = target {
        let command = target.command;
        div()
            .flex()
            .w_full()
            .h(px(GROUP_HEIGHT))
            .child(
                action_with_hover(
                    format!("fold-main-{title}"),
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(glyph(target.visual))
                        .child(div().text_size(px(11.5)).child(title)),
                    active,
                    |s| s.bg(rgb(HOVER)),
                    cx,
                    move |v, window, cx| {
                        v.ui.last_choice.insert(title, command);
                        v.ui.ribbon_peek = false;
                        v.ui.open_group = None;
                        v.ui.file_menu = false;
                        v.run(command, window, cx);
                    },
                )
                .when(!active, |zone| zone.bg(transparent_black()))
                .rounded_r_none()
                .flex_1()
                .h_full()
                .px_1()
                .tooltip(hover_hint(target.description, target.keys.clone())),
            )
            .child(
                action_with_hover(
                    format!("fold-arrow-{title}"),
                    "▾",
                    active,
                    |s| s.bg(rgb(HOVER)),
                    cx,
                    toggle_list,
                )
                .when(!active, |zone| zone.bg(transparent_black()))
                .rounded_l_none()
                .border_l_1()
                .border_color(rgb(BORDER))
                .w(px(16.))
                .h_full(),
            )
    } else {
        div().w_full().child(
            action_with_hover(
                format!("fold-{title}"),
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(glyph(group.visual))
                    .child(div().text_size(px(11.5)).child(format!("{title} ▾"))),
                active,
                |s| s.bg(rgb(HOVER)),
                cx,
                toggle_list,
            )
            .when(!active, |button| button.bg(transparent_black()))
            .w_full()
            .h(px(GROUP_HEIGHT))
            .px_1(),
        )
    };
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(button)
        .when(open, |column| {
            column.child(deferred(
                // Absolute children start at the column's corner: drop below the button.
                anchored()
                    .offset(point(px(0.), px(GROUP_HEIGHT + 4.)))
                    .snap_to_window()
                    .child(
                        popover(div().w(px(240.)).flex().flex_col().gap(px(1.)).children(
                            group.controls.iter().map(|control| {
                                let command = control.command;
                                row(view, control, cx, "folded", true).h(px(26.)).on_click(
                                    cx.listener(move |v, _, _, _| {
                                        v.ui.last_choice.insert(title, command);
                                    }),
                                )
                            }),
                        ))
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
        .w(px(group.width_in(group.mode)))
        .h(px(GROUP_HEIGHT))
        .px(px(Metrics::default().group_padding / 2.))
        .flex()
        .flex_col()
        .justify_between()
        .flex_shrink_0()
        .bg(rgb(CARD))
        .rounded(px(10.))
        .py(px(4.))
        .child(match group.mode {
            GroupMode::Full | GroupMode::Expanded => items(view, group, cx),
            GroupMode::Folded => folded(view, group, index, cx),
        })
        .when(group.mode != GroupMode::Folded, |group_row| {
            group_row.child(
                div()
                    .text_center()
                    .text_size(px(10.))
                    .text_color(rgb(CAPTION))
                    .child(group.title),
            )
        })
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
    if !view.ui.ribbon_collapsed {
        return ribbon_row(view, cx);
    }
    // Folded: a thin strip; a tab click floats the ribbon over the score.
    let strip = div().h(px(4.)).flex_shrink_0().bg(rgb(PANEL));
    if !view.ui.ribbon_peek {
        return strip;
    }
    strip.child(
        deferred(
            anchored()
                .offset(point(px(0.), px(0.)))
                .snap_to_window()
                .child(
                    ribbon_row(view, cx)
                        .shadow_lg()
                        .w(px(view.ui.width))
                        .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                            v.ui.ribbon_peek = false;
                            cx.notify();
                        })),
                ),
        )
        .with_priority(1),
    )
}

fn ribbon_row(view: &AppView, cx: &Context<AppView>) -> Div {
    let toolbar = toolbar_view(view);
    div()
        .h(px(GROUP_HEIGHT + 16.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(Metrics::default().separator))
        .px(px(10.))
        .py(px(4.))
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
    let enabled = item.enabled && !view.busy;
    action(
        format!("file-{command}"),
        div()
            .flex()
            .items_center()
            .gap_3()
            .w_full()
            .child(div().flex_1().child(item.label.clone()))
            .when(item.pending, |row| row.child(pending_mark()))
            .children(
                item.keys
                    .clone()
                    .map(|keys| div().text_size(px(11.)).text_color(rgb(MUTED)).child(keys)),
            ),
        false,
        cx,
        move |v, window, cx| {
            v.ui.file_menu = false;
            v.ui.context_menu = None;
            if enabled {
                v.run(command, window, cx);
            } else {
                cx.notify();
            }
        },
    )
    .justify_start()
    .w_full()
    .h(px(28.))
    .px_2()
    .bg(rgb(POPOVER))
    .text_size(px(12.5))
    .text_color(rgb(if item.pending { MUTED } else { TEXT }))
    .when(!item.enabled, |row| row.opacity(0.4))
    .when(view.busy, |row| row.opacity(0.4))
}

pub(super) fn context_menu(
    view: &AppView,
    position: Point<Pixels>,
    is_mark: bool,
    cx: &Context<AppView>,
) -> impl IntoElement {
    let rows = project_menu(
        if is_mark && view.selected_bar_mark().is_some() {
            BAR_MARK_CONTEXT_MENU
        } else if is_mark {
            MARK_CONTEXT_MENU
        } else {
            NOTE_CONTEXT_MENU
        },
        &view.shortcuts.registry,
        &view.shortcuts.keymap,
        view,
        &[],
    );
    deferred(
        anchored()
            .position(position)
            .position_mode(AnchoredPositionMode::Window)
            .snap_to_window()
            .child(
                popover(
                    div()
                        .w(px(260.))
                        .flex()
                        .flex_col()
                        .children(rows.iter().map(|row| match row {
                            MenuRow::Item(item) => menu_row(view, item, cx).into_any_element(),
                            MenuRow::Separator => separator().into_any_element(),
                            MenuRow::Recent { .. } => unreachable!("context lists have no recents"),
                        })),
                )
                .occlude()
                .on_mouse_down_out(cx.listener(|v, _, _, cx| {
                    v.ui.context_menu = None;
                    cx.notify();
                })),
            ),
    )
    .with_priority(2)
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
                                    .when(!recent_open, |row| row.bg(rgb(POPOVER)))
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
                                        .bg(rgb(POPOVER))
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
        .items_end()
        .justify_between()
        .bg(rgb(BG))
        .child(
            div()
                .flex()
                .items_end()
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
                            .when(!open, |button| button.bg(transparent_black())),
                        )
                        .when(open, |column| column.child(file_menu(view, cx))),
                )
                .child(div().w(px(1.)).h(px(18.)).mx_1().mb(px(6.)).bg(rgb(BORDER)))
                .children(TABS.iter().map(|spec| {
                    let id: TabId = spec.id;
                    let selected = view.ui.tab == id;
                    div()
                        .id(SharedString::from(format!("tab-{}", id.label())))
                        .flex()
                        .items_center()
                        .justify_center()
                        .gap_1()
                        .flex_shrink_0()
                        .text_size(px(12.))
                        .h(px(30.))
                        .px_3()
                        .rounded_t(px(8.))
                        .bg(if selected {
                            rgb(PANEL).into()
                        } else {
                            transparent_black()
                        })
                        .text_color(rgb(if selected { TEXT } else { MUTED }))
                        .cursor_pointer()
                        .hover(move |s| if selected { s } else { s.text_color(rgb(TEXT)) })
                        .child(div().child(id.label()))
                        .tooltip(command_hint(view, id.command()))
                        .on_click(cx.listener(move |v, event: &ClickEvent, window, cx| {
                            v.focus.focus(window);
                            if event.click_count() == 2 {
                                v.ui.ribbon_collapsed = !v.ui.ribbon_collapsed;
                                v.ui.ribbon_peek = false;
                            } else if v.ui.ribbon_collapsed {
                                v.ui.ribbon_peek = !(v.ui.ribbon_peek && v.ui.tab == id);
                            }
                            v.ui.tab = id;
                            v.ui.open_group = None;
                            cx.notify();
                        }))
                })),
        )
        .child(entry_pill(view, cx))
}

/// What typing a note does right now: the active duration and voice, or how
/// to start writing. Pressing it toggles note input.
fn entry_pill(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let writing = view.ui.note_entry;
    let text = if writing {
        let duration = [
            id::WHOLE,
            id::HALF,
            id::QUARTER,
            id::EIGHTH,
            id::SIXTEENTH,
            id::THIRTY_SECOND,
        ]
        .into_iter()
        .find(|command| view.state(*command).checked)
        .and_then(|command| view.shortcuts.registry.get(command.as_str()))
        .map(|spec| format!(" · {}", spec.label))
        .unwrap_or_default();
        let voice = if view.state(id::VOICE_1).checked {
            1
        } else {
            2
        };
        format!("Escribiendo{duration} · Voz {voice}")
    } else {
        match view.shortcuts.label(id::TOGGLE_NOTE_INPUT) {
            Some(key) => format!("Seleccionar · {key} para escribir"),
            None => "Seleccionar".to_string(),
        }
    };
    action("entry-state", text, false, cx, move |v, window, cx| {
        v.run(id::TOGGLE_NOTE_INPUT, window, cx)
    })
    .tooltip(command_hint(view, id::TOGGLE_NOTE_INPUT))
    .mr_3()
    .mb(px(5.))
    .h(px(24.))
    .px_3()
    .rounded_full()
    .border_0()
    .bg(rgb(if writing { SELECT } else { FIELD }))
    .text_color(rgb(if writing { ACCENT_TEXT } else { MUTED }))
    .text_size(px(11.))
}

#[cfg(test)]
mod tests {
    // Not `super::*`: the GPUI glob would shadow `#[test]` with `gpui::test`.
    use score_toolbar::{TABS, Visual};

    #[test]
    fn every_ribbon_icon_has_a_drawing() {
        for tab in TABS {
            for group in tab.groups {
                let visuals = std::iter::once(group.visual)
                    .chain(group.controls.iter().map(|control| control.visual));
                for visual in visuals {
                    if let Visual::Icon(name) = visual {
                        assert!(
                            super::Icon::named(name).is_some(),
                            "{}: {name}",
                            group.title
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn every_music_glyph_of_the_toolbar_is_preloaded() {
        let font = crate::music_font::MusicFont::new();
        for tab in TABS {
            for group in tab.groups {
                let visuals = std::iter::once(group.visual)
                    .chain(group.controls.iter().map(|control| control.visual));
                for visual in visuals {
                    match visual {
                        Visual::Music(text) => {
                            assert!(font.covers(text), "{}: {text:?}", group.title)
                        }
                        Visual::Clef(clef) => assert!(font.covers(clef.glyph())),
                        Visual::Harmonic(harmonic) => {
                            assert!(font.covers(harmonic.glyph()), "{}", group.title)
                        }
                        Visual::Chord => assert!(font.covers(super::super::CHORD_NOTE)),
                        Visual::Tie | Visual::Slur => {
                            assert!(font.covers(super::super::CONNECTION_UP));
                            assert!(font.covers(super::super::CONNECTION_DOWN));
                            assert!(font.covers(super::super::CONNECTION_HEAD));
                        }
                        Visual::Staff | Visual::Bar => {
                            assert!(font.covers(super::super::STAFF_SYMBOL));
                            assert!(font.covers(super::super::BAR_SYMBOL));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    #[test]
    fn clefs_fit_picker_and_every_ribbon_icon_frame() {
        let font = crate::music_font::MusicFont::new();
        let frames = [
            (28., 36.),
            (26. * 0.7, 26.),
            (22. * 0.7, 22.),
            (16. * 0.7, 16.),
        ];
        for clef in score_core::Clef::PITCHED
            .into_iter()
            .chain([score_core::Clef::Percussion])
        {
            for frame in frames {
                let size = super::super::fitted_music_font_size(&font, clef.glyph(), frame);
                let ink = font.bounds(clef.glyph(), size);
                let width = f32::from(ink.size.width) + super::super::MUSIC_SYMBOL_PADDING;
                let height = f32::from(ink.size.height) + super::super::MUSIC_SYMBOL_PADDING;
                assert!(
                    width <= frame.0 + 0.001 && height <= frame.1 + 0.001,
                    "{clef:?}: canvas needs {width:.2}×{height:.2}, frame is {frame:?}"
                );
            }
        }
    }

    #[test]
    fn every_label_is_short_and_not_musical_text() {
        for tab in TABS {
            for group in tab.groups {
                let visuals = std::iter::once(group.visual)
                    .chain(group.controls.iter().map(|control| control.visual));
                for visual in visuals {
                    if let Visual::Label(text) = visual {
                        assert!(text.chars().count() <= 4, "{}: {text}", group.title);
                    }
                }
            }
        }
    }

    #[test]
    fn every_ribbon_music_glyph_fits_its_frame_under_the_size_ceiling() {
        let font = crate::music_font::MusicFont::new();
        for tab in TABS {
            for group in tab.groups {
                let visuals = std::iter::once(group.visual)
                    .chain(group.controls.iter().map(|control| control.visual));
                for visual in visuals {
                    let Visual::Music(text) = visual else {
                        continue;
                    };
                    for nominal in [16_f32, 22., 26.] {
                        let frame = (nominal * 0.9, nominal);
                        let size = super::super::fitted_music_font_size(&font, text, frame)
                            .min(nominal * 1.6);
                        let ink = font.bounds(text, size);
                        let width = f32::from(ink.size.width) + super::super::MUSIC_SYMBOL_PADDING;
                        let height =
                            f32::from(ink.size.height) + super::super::MUSIC_SYMBOL_PADDING;
                        assert!(
                            width <= frame.0 + 0.001 && height <= frame.1 + 0.001,
                            "{}: {text:?} needs {width:.2}×{height:.2}, frame is {frame:?}",
                            group.title
                        );
                    }
                }
            }
        }
    }
}
