//! Native editor chrome. Pending controls are visibly marked «Pendiente»; score
//! data and transport readouts always come from the current document/session.
pub mod agent_panel;
mod clef_picker;
mod comments;
pub use comments::CommentEditor;
mod command_state;
mod export_picker;
pub use export_picker::ExportPicker;
mod command_search;
mod header_editor;
mod icons;
mod instrument_panel;
mod mixer;
mod music_text;
mod staff_text;
pub use staff_text::{ChordEditor, LyricEditor, StaffTextEditor};
mod tempo_editor;
pub mod text_input;
pub use command_search::CommandSearch;
pub use header_editor::HeaderEditor;
pub use mixer::MixerDrag;
use music_text::inline_text;
use selection_view::beat_label;
pub use tempo_editor::TempoEditor;
mod preferences;
mod review;
mod selection_panel;
mod selection_view;
pub use selection_view::EditorKind;
#[cfg(test)]
mod selection_view_tests;
#[cfg(feature = "vsco")]
mod sound_store;
pub use review::RangeEditor;
mod theme;
mod toolbar;
use super::*;
use icons::{Icon, icon};
pub use instrument_panel::InstrumentPicker;
pub use preferences::{Preferences, PreferencesConflict};
use score_core::Duration;
use score_keymap::{CommandId, id};
use score_toolbar::CommandStates;

use crate::panels::{PanelMemory, PartsMode, RightTab};
use theme::*;
pub use theme::{BG, TEXT};
pub const LEFT_WIDTH: f32 = 240.;
pub const RIGHT_WIDTH: f32 = 286.;
pub const AGENT_WIDTH: f32 = 320.;

pub struct UiState {
    pub show_instruments: bool,
    pub show_inspector: bool,
    pub show_agent: bool,
    /// The agent list above the agent panel's field.
    pub agent_menu: bool,
    pub note_entry: bool,
    pub compact: bool,
    pub zoom: f32,
    pub view_mode: score_layout::ViewMode,
    pub tab: score_toolbar::TabId,
    pub right: RightTab,
    pub parts_mode: PartsMode,
    /// Instrument families folded in the parts list, by name.
    pub folded_families: std::collections::BTreeSet<String>,
    /// The parts list filter; not remembered between sessions.
    #[allow(dead_code)] // read by the parts panel once it is redrawn
    pub parts_query: String,
    /// Archivo dropdown and its "Abrir reciente" submenu.
    pub file_menu: bool,
    pub recent_menu: bool,
    /// Window position and whether the clicked target is a mark.
    pub context_menu: Option<(Point<Pixels>, bool)>,
    /// The folded ribbon group whose dropdown is open, by position.
    pub open_group: Option<usize>,
    /// Window width, for the ribbon's fold decision.
    pub width: f32,
    /// The ribbon is folded away; a tab click shows it floating (`ribbon_peek`).
    pub ribbon_collapsed: bool,
    pub ribbon_peek: bool,
    /// The last option used from each dropdown group, by group title.
    pub last_choice: std::collections::HashMap<&'static str, score_keymap::CommandId>,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            show_instruments: true,
            show_inspector: true,
            show_agent: false,
            agent_menu: false,
            note_entry: true,
            compact: false,
            zoom: 0.7,
            view_mode: Default::default(),
            tab: score_toolbar::TabId::Write,
            right: RightTab::default(),
            parts_mode: PartsMode::default(),
            folded_families: Default::default(),
            parts_query: String::new(),
            file_menu: false,
            recent_menu: false,
            context_menu: None,
            open_group: None,
            width: 1440.,
            ribbon_collapsed: false,
            ribbon_peek: false,
            last_choice: Default::default(),
        }
    }
}

impl UiState {
    /// The remembered part of the state, as read from and written to `paneles.json`.
    pub fn panel_memory(&self) -> PanelMemory {
        PanelMemory {
            show_parts: self.show_instruments,
            show_right: self.show_inspector,
            right: self.right,
            parts_mode: self.parts_mode,
            folded: self.folded_families.iter().cloned().collect(),
        }
    }
    pub fn restore(memory: PanelMemory) -> Self {
        Self {
            show_instruments: memory.show_parts,
            show_inspector: memory.show_right,
            right: memory.right,
            parts_mode: memory.parts_mode,
            folded_families: memory.folded.into_iter().collect(),
            ..Self::default()
        }
    }
}

fn button(
    view: &AppView,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    let font = view.music_font.clone();
    let label: SharedString = label.into();
    action(id, inline_text(font, label.as_ref()), selected, cx, click)
        .px_2()
        .h(px(29.))
}
fn action(
    id: impl Into<SharedString>,
    content: impl IntoElement,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    action_with_hover(id, content, selected, |s| s.bg(rgb(FIELD_HOVER)), cx, click)
}
/// Like `action`, but the caller picks the hover style: GPUI allows setting
/// it only once, so it cannot be chained after `action(...)`.
fn action_with_hover(
    id: impl Into<SharedString>,
    content: impl IntoElement,
    selected: bool,
    hover: impl Fn(StyleRefinement) -> StyleRefinement + 'static,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    div()
        .id(id.into())
        .flex()
        .items_center()
        .justify_center()
        .gap_1()
        .flex_shrink_0()
        .text_size(px(12.))
        .rounded_md()
        .border_1()
        .border_color(if selected {
            Hsla::from(rgb(SELECT_BORDER))
        } else {
            transparent_black()
        })
        .bg(rgb(if selected { SELECT } else { FIELD }))
        .text_color(rgb(if selected { ACCENT_TEXT } else { TEXT }))
        .cursor_pointer()
        .hover(hover)
        .child(content)
        .on_click(cx.listener(move |view, _, window, cx| {
            view.focus.focus(window);
            click(view, window, cx);
        }))
}
fn icon_button(
    id: impl Into<SharedString>,
    glyph: Icon,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    action(
        id,
        icon(glyph, if selected { BLUE } else { MUTED }),
        selected,
        cx,
        click,
    )
    .w(px(28.))
    .h(px(28.))
}
fn pending_mark() -> Div {
    div()
        .flex_shrink_0()
        .text_size(px(9.))
        .text_color(rgb(MUTED))
        .child("Pendiente")
}
fn pending_badge() -> Div {
    div()
        .text_size(px(9.))
        .text_color(rgb(MUTED))
        .px_1()
        .rounded_sm()
        .border_1()
        .border_dashed()
        .border_color(rgb(PENDING_OUTLINE))
        .child("Pendiente")
}
/// A small heading: semibold so that at 10 px it reads as a heading, not as
/// one more muted label. Written in sentence case.
fn caption(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(text.into())
}
fn shortcut(text: impl Into<SharedString>) -> Div {
    div()
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(POPOVER_BORDER))
        .text_size(px(9.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
struct HoverHint {
    label: SharedString,
    keys: Option<String>,
    /// What the control does, on a second line.
    detail: Option<SharedString>,
}
impl Render for HoverHint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(2.))
            .px_2()
            .py_1()
            .max_w(px(360.))
            .rounded_md()
            .border_1()
            .border_color(rgb(POPOVER_BORDER))
            .bg(rgb(FIELD))
            .shadow_md()
            .text_size(px(11.))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(self.label.clone())
                    .children(self.keys.clone().map(shortcut)),
            )
            .children(
                self.detail
                    .clone()
                    .map(|detail| div().text_color(rgb(SECONDARY)).child(detail)),
            )
    }
}
fn hover_hint(
    label: impl Into<SharedString>,
    keys: Option<String>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    described_hint(label, keys, "")
}
/// A hover hint with a sentence on what the control does under its name;
/// an empty `detail` leaves the second line out.
fn described_hint(
    label: impl Into<SharedString>,
    keys: Option<String>,
    detail: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    let detail: SharedString = detail.into();
    let detail = (!detail.is_empty()).then_some(detail);
    move |_, cx| {
        cx.new(|_| HoverHint {
            label: label.clone(),
            keys: keys.clone(),
            detail: detail.clone(),
        })
        .into()
    }
}
fn command_hint(
    view: &AppView,
    command: CommandId,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    hover_hint(
        view.shortcuts
            .registry
            .get(command.as_str())
            .map_or(command.as_str(), |spec| spec.label),
        view.shortcuts.label(command),
    )
}
/// A command's shortcut is available on hover without taking space in the button.
fn command_button(
    element: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    view: &AppView,
    cx: &Context<AppView>,
    command: CommandId,
) -> Stateful<Div> {
    button(view, element, label, selected, cx, move |v, window, cx| {
        v.run(command, window, cx)
    })
    .tooltip(command_hint(view, command))
}
pub fn workspace(view: &AppView, title: String, cx: &Context<AppView>) -> impl IntoElement {
    div()
        .relative()
        .size_full()
        .flex()
        .flex_col()
        .on_mouse_move(cx.listener(|v, event: &MouseMoveEvent, window, cx| {
            if !mixer::drag(v, event, cx) {
                v.move_passage_selection(event, window, cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(|v, event, window, cx| {
                mixer::end(v, event, cx);
                v.end_passage_selection(event, window, cx);
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(|v, event, window, cx| {
                mixer::end(v, event, cx);
                v.cancel_note_drag(cx);
                v.end_passage_selection(event, window, cx);
            }),
        )
        .child(titlebar(view, title, cx))
        .child(toolbar::tabs(view, cx))
        .child(toolbar::ribbon(view, cx))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .when(view.ui.show_instruments, |row| {
                    row.child(instrument_panel::panel(view, cx))
                })
                .child(score_area(view, cx))
                .when(view.ui.show_inspector, |row| row.child(inspector(view, cx)))
                .when(view.ui.show_agent, |row| {
                    row.child(agent_panel::panel(view, cx))
                }),
        )
        .child(transport(view, cx))
        .child(statusbar(view))
        .when(view.preferences.is_some(), |root| {
            root.child(preferences::overlay(view, cx))
        })
        .when(view.instrument_picker.is_some(), |root| {
            root.child(instrument_panel::dialog(view, cx))
        })
        .when(view.export_picker.is_some(), |root| {
            root.child(export_picker::dialog(view, cx))
        })
        .when_some(view.ui.context_menu, |root, (position, is_mark)| {
            root.child(toolbar::context_menu(view, position, is_mark, cx))
        })
}

fn titlebar(view: &AppView, title: String, cx: &Context<AppView>) -> Div {
    div()
        .h(px(44.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_2()
        .pl(px(if cfg!(target_os = "macos") { 90. } else { 16. }))
        .pr_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(
            img(crate::app_icon::image())
                .size(px(30.))
                .object_fit(ObjectFit::Contain)
                .flex_shrink_0(),
        )
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.))
                .mr_2()
                .child("Levare"),
        )
        .child(div().h(px(25.)).border_r_1().border_color(rgb(BORDER)))
        .child(
            icon_button("save-document", Icon::Save, false, cx, |v, _, cx| {
                v.save(false, cx)
            })
            .tooltip(command_hint(view, id::SAVE)),
        )
        .child(
            icon_button("undo-edit", Icon::Undo, false, cx, |v, _, cx| {
                v.history(false, cx)
            })
            .tooltip(command_hint(view, id::UNDO))
            .opacity(if view.document.editor.can_undo() {
                1.
            } else {
                0.35
            }),
        )
        .child(
            icon_button("redo-edit", Icon::Redo, false, cx, |v, _, cx| {
                v.history(true, cx)
            })
            .tooltip(command_hint(view, id::REDO))
            .opacity(if view.document.editor.can_redo() {
                1.
            } else {
                0.35
            }),
        )
        .child(
            div()
                .ml_3()
                .max_w(px(if view.ui.compact { 140. } else { 240. }))
                .px_3()
                .h(px(30.))
                .flex()
                .items_center()
                .gap_2()
                .rounded_md()
                .bg(rgb(FIELD))
                .child(div().text_size(px(12.)).truncate().child(title))
                .when(view.document.is_dirty(), |tab| {
                    tab.child(div().text_color(rgb(BLUE)).child("●"))
                }),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
        )
        .child(command_search::search_box(view, cx))
        .child(
            command_button(
                "open-shortcuts",
                "Atajos",
                view.preferences.is_some(),
                view,
                cx,
                id::PREFERENCES,
            )
            .h(px(28.))
            .text_size(px(11.)),
        )
        .child(
            icon_button(
                "toggle-instruments",
                Icon::LeftPanel,
                view.ui.show_instruments,
                cx,
                |v, _, cx| {
                    v.ui.show_instruments = !v.ui.show_instruments;
                    v.reveal_selection = true;
                    cx.notify();
                },
            )
            .tooltip(command_hint(view, id::TOGGLE_INSTRUMENTS_PANEL)),
        )
        .child(
            icon_button(
                "toggle-inspector",
                Icon::RightPanel,
                view.ui.show_inspector,
                cx,
                |v, _, cx| {
                    v.ui.show_inspector = !v.ui.show_inspector;
                    v.reveal_selection = true;
                    cx.notify();
                },
            )
            .tooltip(command_hint(view, id::TOGGLE_INSPECTOR)),
        )
        .child(
            icon_button(
                "toggle-agent",
                Icon::Agent,
                view.ui.show_agent,
                cx,
                |v, window, cx| v.run(id::TOGGLE_AGENT_PANEL, window, cx),
            )
            .tooltip(command_hint(view, id::TOGGLE_AGENT_PANEL)),
        )
        .child(
            action(
                "export-pdf",
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon(Icon::Export, FIELD))
                    .child("Exportar…"),
                false,
                cx,
                |v, _, cx| {
                    v.export_pdf(cx);
                },
            )
            .px_3()
            .h(px(30.))
            .bg(rgb(TEXT))
            .text_color(rgb(FIELD)),
        )
}
const MUSIC_SYMBOL_PADDING: f32 = 4.;

fn fitted_music_font_size(
    font: &crate::music_font::MusicFont,
    text: &str,
    frame: (f32, f32),
) -> f32 {
    // SMuFL font size measures staff spaces, not a symbol's visible height.
    // Include the canvas padding so fitting the ink also fits its layout box.
    let reference = 30.;
    let ink = font.bounds(text, reference);
    let width = (frame.0 - MUSIC_SYMBOL_PADDING).max(0.);
    let height = (frame.1 - MUSIC_SYMBOL_PADDING).max(0.);
    reference
        * (width / f32::from(ink.size.width).max(1.))
            .min(height / f32::from(ink.size.height).max(1.))
}

fn clef_symbol(
    view: &AppView,
    clef: score_core::Clef,
    frame: (f32, f32),
    color: u32,
) -> impl IntoElement {
    fitted_music_symbol(view, clef.glyph(), frame, color)
}

fn fitted_music_symbol(
    view: &AppView,
    text: &'static str,
    frame: (f32, f32),
    color: u32,
) -> impl IntoElement {
    music_symbol(
        view,
        text,
        fitted_music_font_size(&view.music_font, text, frame),
        frame,
        color,
    )
}

fn music_symbol(
    view: &AppView,
    text: impl Into<SharedString>,
    font_size: f32,
    frame: (f32, f32),
    color: u32,
) -> impl IntoElement {
    music_glyphs(
        view,
        [(text.into(), Point::default(), font_size)],
        frame,
        color,
    )
}

const CHORD_NOTE: &str = "\u{e1d5}";
const STAFF_SYMBOL: &str = "\u{e014}";
const BAR_SYMBOL: &str = "\u{e030}";

fn music_staff(view: &AppView, size: f32, color: u32, bar: bool) -> impl IntoElement {
    let font_size = size * 0.8;
    let staff = view.music_font.bounds(STAFF_SYMBOL, font_size);
    div().flex().items_center().child(if bar {
        music_glyphs(
            view,
            [
                (STAFF_SYMBOL.into(), Point::default(), font_size),
                (
                    BAR_SYMBOL.into(),
                    point(staff.size.width * 0.6, px(0.)),
                    font_size,
                ),
            ],
            (size * 0.7, size * 0.8),
            color,
        )
        .into_any_element()
    } else {
        music_symbol(
            view,
            STAFF_SYMBOL,
            font_size,
            (size * 0.7, size * 0.8),
            color,
        )
        .into_any_element()
    })
}

fn music_chord(view: &AppView, size: f32, color: u32) -> impl IntoElement {
    let font_size = size * 0.8;
    // Thirds are one staff space apart. Identical x positions make the original
    // Bravura stems overlap into one stem without altering any glyph outlines.
    let space = font_size / 4.;
    music_glyphs(
        view,
        [0., space, 2. * space].map(|y| (CHORD_NOTE.into(), point(px(0.), px(y)), font_size)),
        (size * 0.7, size * 0.8),
        color,
    )
}

pub(crate) const CONNECTION_HEAD: &str = "\u{e0a4}"; // Bravura noteheadBlack.
pub(crate) const CONNECTION_UP: &str = "\u{e1d5}"; // Bravura noteQuarterUp.
pub(crate) const CONNECTION_DOWN: &str = "\u{e1d6}"; // Bravura noteQuarterDown.

/// An engraved curve: thin at both ends, `thickness` thick in the middle,
/// bowing by `bend` (negative bends upward).
fn engraved_curve(
    from: Point<Pixels>,
    to: Point<Pixels>,
    bend: f32,
    thickness: f32,
) -> Option<Path<Pixels>> {
    let span = to.x - from.x;
    let control = |t: f32, bend: f32| point(from.x + span * t, from.y + px(bend));
    let inner = bend - thickness.copysign(bend);
    let mut path = PathBuilder::fill();
    path.move_to(from);
    path.cubic_bezier_to(to, control(0.25, bend * 1.33), control(0.75, bend * 1.33));
    path.cubic_bezier_to(
        from,
        control(0.75, inner * 1.33),
        control(0.25, inner * 1.33),
    );
    path.close();
    path.build().ok()
}

/// Ribbon icons for the two connections, engraved as notation programs show
/// them: a tie joins two quarter notes on the same pitch under their heads; a
/// slur arches over two stem-down quarters a step apart. Stems keep both
/// readable as notes at toolbar size.
fn music_connection(view: &AppView, size: f32, color: u32, slur: bool) -> impl IntoElement {
    let font = view.music_font.clone();
    let note_size = size * 0.72;
    let space = note_size / 4.;
    let head = f32::from(font.bounds(CONNECTION_HEAD, note_size).size.width);
    let width = size * 1.05;
    let ink = if slur { 6.1 * space } else { 5.1 * space };
    let height = (size * 0.8).max(ink + 2.);
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let top = f32::from(bounds.size.height) / 2. - ink / 2.;
            let at = |x: f32, y: f32| bounds.origin + point(px(x), px(top + y));
            let xs = [width * 0.24, width * 0.76];
            // Bravura notes sit with their head centred on the baseline.
            if slur {
                let ys = [2.6 * space, 1.6 * space];
                for (x, y) in xs.into_iter().zip(ys) {
                    font.paint_sized(
                        CONNECTION_DOWN,
                        at(x - head / 2., y),
                        color,
                        note_size,
                        window,
                    );
                }
                if let Some(path) = engraved_curve(
                    at(xs[0] - head * 0.3, ys[0] - 0.85 * space),
                    at(xs[1] + head * 0.3, ys[1] - 0.85 * space),
                    -0.9 * space,
                    0.3 * space,
                ) {
                    window.paint_path(path, rgb(color));
                }
            } else {
                let y = 3.6 * space;
                for x in xs {
                    font.paint_sized(
                        CONNECTION_UP,
                        at(x - head / 2., y),
                        color,
                        note_size,
                        window,
                    );
                }
                if let Some(path) = engraved_curve(
                    at(xs[0] + head * 0.25, y + 0.75 * space),
                    at(xs[1] - head * 0.25, y + 0.75 * space),
                    0.8 * space,
                    0.3 * space,
                ) {
                    window.paint_path(path, rgb(color));
                }
            }
        },
    )
    .w(px(width))
    .h(px(height))
    .flex_shrink_0()
}

/// Center the combined ink of original Bravura glyphs at their own baselines.
fn music_glyphs<const N: usize>(
    view: &AppView,
    glyphs: [(SharedString, Point<Pixels>, f32); N],
    frame: (f32, f32),
    color: u32,
) -> impl IntoElement {
    let font = view.music_font.clone();
    let ink = glyphs
        .iter()
        .map(|(text, offset, font_size)| {
            let mut bounds = font.bounds(text, *font_size);
            bounds.origin += *offset;
            bounds
        })
        .reduce(|bounds, next| bounds.union(&next))
        .unwrap_or_default();
    let width = px(frame.0).max(ink.size.width + px(MUSIC_SYMBOL_PADDING));
    let height = px(frame.1).max(ink.size.height + px(MUSIC_SYMBOL_PADDING));
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let origin = bounds.origin
                + point(
                    (bounds.size.width - ink.size.width) / 2. - ink.origin.x,
                    (bounds.size.height - ink.size.height) / 2. - ink.origin.y,
                );
            for (text, offset, font_size) in &glyphs {
                font.paint_sized(text, origin + *offset, color, *font_size, window);
            }
        },
    )
    .w(width)
    .h(height)
    .flex_shrink_0()
}

pub(super) fn instrument_name(score: &score_core::Score, id: PartId) -> String {
    score.part_name(id)
}

fn clef_name(clef: score_core::Clef) -> &'static str {
    match clef {
        score_core::Clef::Treble => "sol",
        score_core::Clef::Alto => "do en tercera",
        score_core::Clef::Tenor => "do en cuarta",
        score_core::Clef::Bass => "fa",
        score_core::Clef::Percussion => "percusión",
    }
}
/// `button` in the ribbon's grammar, for the editors in the right panel.
fn choice(
    view: &AppView,
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    let label: SharedString = label.into();
    toolbar::control(
        id,
        inline_text(view.music_font.clone(), label.as_ref()),
        selected,
        cx,
        click,
    )
    .px_2()
    .h(px(29.))
}
/// Closes the open editor without writing anything more, as Esc does.
fn close_editor(label: &'static str, cx: &Context<AppView>) -> Stateful<Div> {
    toolbar::control(
        "close-editor",
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(label)
            .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child("Esc")),
        false,
        cx,
        |v, _, cx| {
            v.time_picker = false;
            v.clef_picker = false;
            v.tempo_picker = None;
            v.key_picker = false;
            v.header_editor = None;
            v.text_editor = None;
            v.lyric_editor = None;
            v.chord_editor = None;
            cx.notify();
        },
    )
    .px(px(10.))
    .h(px(30.))
}
fn key_picker(view: &AppView, cx: &Context<AppView>) -> Div {
    const SCOPE: &str = "Hasta el siguiente cambio, en todos los instrumentos. Los signos individuales tienen prioridad.";
    let bar = view.signature_measure();
    let score = view.document.editor.score();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(signature_scope(view, cx))
        .child(toolbar::track(
            [(KeyMode::Major, "Mayores"), (KeyMode::Minor, "Menores")].map(|(mode, label)| {
                toolbar::control(
                    format!("keys-{label}"),
                    label,
                    view.key_mode == mode,
                    cx,
                    move |v, _, cx| {
                        v.key_mode = mode;
                        cx.notify();
                    },
                )
                .h(px(26.))
                .tooltip(described_hint(
                    label,
                    None,
                    "Tonalidad de la obra en altura real. Cada parte adapta su armadura.",
                ))
            }),
        ))
        .child(
            div().flex().flex_wrap().gap_1().children(
                [0, 1, 2, 3, 4, 5, 6, 7, -1, -2, -3, -4, -5, -6, -7]
                    .into_iter()
                    .map(|fifths| {
                        let key = KeySignature::new(fifths, view.key_mode).unwrap();
                        choice(
                            view,
                            format!("key-{fifths}"),
                            key.label(),
                            score.key_signature_at(bar) == key,
                            cx,
                            move |v, _, cx| v.choose_key(key, cx),
                        )
                        .w(px(82.))
                        .tooltip(described_hint(key.label(), None, SCOPE))
                    }),
            ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .when(bar > 0 && score.key_change_at(bar).is_some(), |row| {
                    row.child(choice(
                        view,
                        "remove-key-change",
                        "Quitar cambio de tonalidad",
                        false,
                        cx,
                        |v, _, cx| v.edit(Command::RemoveKeyChange(v.signature_measure()), cx),
                    ))
                })
                .child(close_editor("Cancelar", cx)),
        )
}

fn signature_scope(view: &AppView, cx: &Context<AppView>) -> Div {
    let bar = view
        .tempo_picker
        .as_ref()
        .map_or(view.document.editor.cursor().measure, |editor| {
            editor.measure
        })
        + 1;
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(caption("Aplicar desde"))
        .child(toolbar::track(
            [
                (true, "Inicio de la pieza".to_owned()),
                (false, format!("Compás {bar}")),
            ]
            .map(|(from_start, label)| {
                toolbar::control(
                    format!("signature-scope-{from_start}"),
                    label,
                    view.signature_from_start == from_start,
                    cx,
                    move |v, _, cx| {
                        v.signature_from_start = from_start;
                        v.sync_signature_controls(cx);
                        cx.notify();
                    },
                )
                .h(px(26.))
            }),
        ))
}

fn time_picker(view: &AppView, cx: &Context<AppView>) -> Div {
    const SCOPE: &str = "Hasta el siguiente cambio, en todos los instrumentos. Se conservan las notas y los compases existentes.";
    let time = view.pending_time;
    let bar = view.signature_measure();
    let score = view.document.editor.score();
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(signature_scope(view, cx))
        .child(caption("Métricas habituales").mt_2())
        .child(
            div().flex().flex_wrap().gap_1().children(
                [
                    (2, 4),
                    (3, 4),
                    (4, 4),
                    (5, 4),
                    (6, 8),
                    (7, 8),
                    (9, 8),
                    (12, 8),
                ]
                .into_iter()
                .map(|(numerator, denominator)| {
                    let value = score_core::TimeSignature {
                        numerator,
                        denominator,
                    };
                    choice(
                        view,
                        format!("meter-{numerator}-{denominator}"),
                        format!("{numerator}/{denominator}"),
                        score.time_at(bar) == value,
                        cx,
                        move |v, _, cx| v.choose_time(value, cx),
                    )
                    .w(px(60.))
                    .tooltip(described_hint(
                        format!("{numerator}/{denominator}"),
                        None,
                        SCOPE,
                    ))
                }),
            ),
        )
        .child(caption("Personalizada").mt_2())
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(choice(
                    view,
                    "meter-numerator-minus",
                    "−",
                    false,
                    cx,
                    |v, _, cx| {
                        v.pending_time.numerator =
                            v.pending_time.numerator.saturating_sub(1).max(1);
                        cx.notify();
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .text_center()
                        .text_size(px(12.))
                        .child(format!("Numerador: {}", time.numerator)),
                )
                .child(choice(
                    view,
                    "meter-numerator-plus",
                    "+",
                    false,
                    cx,
                    |v, _, cx| {
                        v.pending_time.numerator = (v.pending_time.numerator + 1).min(16);
                        cx.notify();
                    },
                )),
        )
        .child(caption("Denominador"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children([1, 2, 4, 8, 16].into_iter().map(|denominator| {
                    choice(
                        view,
                        format!("meter-denominator-{denominator}"),
                        denominator.to_string(),
                        time.denominator == denominator,
                        cx,
                        move |v, _, cx| {
                            v.pending_time.denominator = denominator;
                            cx.notify();
                        },
                    )
                })),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .mt_2()
                .child(
                    choice(
                        view,
                        "apply-meter",
                        format!("Aplicar {}/{}", time.numerator, time.denominator),
                        true,
                        cx,
                        |v, _, cx| v.choose_time(v.pending_time, cx),
                    )
                    .tooltip(described_hint(
                        format!("Aplicar {}/{}", time.numerator, time.denominator),
                        None,
                        SCOPE,
                    )),
                )
                .child(close_editor("Cancelar", cx)),
        )
        .when(bar > 0 && score.time_change_at(bar).is_some(), |picker| {
            picker.child(choice(
                view,
                "remove-meter-change",
                "Quitar cambio de métrica",
                false,
                cx,
                |v, _, cx| v.edit(Command::RemoveTimeChange(v.signature_measure()), cx),
            ))
        })
}
fn page_navigation(view: &AppView, cx: &Context<AppView>) -> Div {
    let paged = view.ui.view_mode == score_layout::ViewMode::Page;
    let index = view
        .viewport
        .visible_page(&view.layout)
        .min(view.layout.pages.len().saturating_sub(1));
    div()
        .border_t_1()
        .border_color(rgb(BORDER))
        .px_3()
        .py_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(caption(if paged { "PÁGINAS" } else { "VISTA" }))
        .when(paged, |panel| {
            panel
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            button(view, "previous-page", "‹", false, cx, move |v, _, cx| {
                                v.pending_page = Some(index.saturating_sub(1));
                                cx.notify();
                            })
                            .opacity(if index > 0 {
                                1.
                            } else {
                                0.35
                            }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_center()
                                .text_size(px(12.))
                                .child(format!(
                                    "Página {} de {}",
                                    index + 1,
                                    view.layout.pages.len()
                                )),
                        )
                        .child(
                            button(view, "next-page", "›", false, cx, move |v, _, cx| {
                                v.pending_page =
                                    Some((index + 1).min(v.layout.pages.len().saturating_sub(1)));
                                cx.notify();
                            })
                            .opacity(
                                if index + 1 < view.layout.pages.len() {
                                    1.
                                } else {
                                    0.35
                                },
                            ),
                        ),
                )
                .when_some(view.layout.pages.get(index), |panel, page| {
                    panel.child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "Compases {}–{}",
                                page.first_measure + 1,
                                page.last_measure + 1
                            )),
                    )
                })
        })
        .when(!paged, |panel| {
            panel
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                    if view.ui.view_mode == score_layout::ViewMode::Continuous {
                        "Continua · Desplazamiento vertical"
                    } else {
                        "Pentagrama · Desplazamiento horizontal"
                    },
                ))
                .child(
                    button(
                        view,
                        "show-pages",
                        "Mostrar páginas",
                        false,
                        cx,
                        |v, _, cx| v.set_view_mode(score_layout::ViewMode::Page, cx),
                    )
                    .w_full(),
                )
        })
}

fn readout_value(value: impl IntoElement) -> Div {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(value)
}
fn readout_label(label: &'static str) -> Div {
    div().text_size(px(11.)).text_color(rgb(MUTED)).child(label)
}
fn readout(label: &'static str, value: impl Into<SharedString>) -> Div {
    div()
        .px_2()
        .py_1()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(1.))
        .child(readout_label(label))
        .child(readout_value(div().child(value.into())))
}
/// A readout that opens its editor in the right panel, in the ribbon's grammar.
fn readout_button(
    view: &AppView,
    id: &'static str,
    label: &'static str,
    value: impl Into<SharedString>,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    let value: SharedString = value.into();
    toolbar::control(
        id,
        div()
            .flex()
            .flex_col()
            .gap(px(1.))
            .child(readout_label(label))
            .child(readout_value(inline_text(
                view.music_font.clone(),
                value.as_ref(),
            ))),
        selected,
        cx,
        click,
    )
    .justify_start()
    .px_2()
    .py_1()
}
fn readout_divider() -> Div {
    div().w(px(1.)).h(px(24.)).flex_shrink_0().bg(rgb(BORDER))
}
/// What is selected, in a few words; the right panel has the rest.
fn selection_summary(view: &AppView) -> String {
    let editor = &view.document.editor;
    match editor.selected_ids().as_slice() {
        [] => "Nada seleccionado".into(),
        [id] => match editor.score().event(*id).map(|event| event.pitches.len()) {
            Some(0) => "1 silencio".into(),
            Some(1) => "1 nota".into(),
            Some(_) => "1 acorde".into(),
            None => "Nada seleccionado".into(),
        },
        ids => match view.shortcuts.label(id::COPY) {
            Some(keys) => format!("{} elementos · Copiar {keys}", ids.len()),
            None => format!("{} elementos", ids.len()),
        },
    }
}

fn score_area(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let score = editor.score();
    let cursor = editor.cursor();
    let position = view.position;
    let time = score.time_at(position.measure);
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .bg(rgb(CANVAS))
        .child(
            div()
                .id("score-readouts")
                .h(px(46.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1()
                .px_3()
                .overflow_x_scroll()
                .bg(rgb(PANEL))
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(readout(
                    "Compás",
                    format!(
                        "{} de {}",
                        (position.measure + 1).min(score.measures().len()),
                        score.measures().len()
                    ),
                ))
                .child(readout(
                    "Tiempo",
                    if position.at_end {
                        "Fin".to_owned()
                    } else {
                        beat_label(position.beat as f32)
                    },
                ))
                .child(readout_divider())
                .child(
                    readout_button(
                        view,
                        "tempo-readout",
                        "Tempo",
                        score.tempo_at(position.measure).label(),
                        view.tempo_picker.is_some(),
                        cx,
                        move |v, _, cx| v.toggle_tempo_at(position.measure, cx),
                    )
                    .tooltip(command_hint(view, id::TEMPO_TEXT)),
                )
                .child(
                    readout_button(
                        view,
                        "meter-readout",
                        "Métrica",
                        format!("{}/{}", time.numerator, time.denominator),
                        view.time_picker,
                        cx,
                        |v, _, cx| v.toggle_time_picker(cx),
                    )
                    .tooltip(command_hint(view, id::TIME_SIGNATURE)),
                )
                .child(
                    readout_button(
                        view,
                        "key-readout",
                        "Tonalidad",
                        score.key_signature_at(position.measure).label(),
                        view.key_picker,
                        cx,
                        |v, _, cx| v.toggle_key_picker(cx),
                    )
                    .tooltip(command_hint(view, id::KEY_SIGNATURE)),
                )
                .child(readout_divider())
                .child(readout(
                    "Parte",
                    format!(
                        "{} · Voz {}",
                        instrument_name(score, cursor.part),
                        cursor.voice.number()
                    ),
                ))
                .child(
                    div()
                        .ml_auto()
                        .pl_3()
                        .flex_shrink_0()
                        .text_size(px(12.5))
                        .text_color(rgb(SECONDARY))
                        .child(selection_summary(view)),
                ),
        )
        .child(
            div().flex_1().min_h_0().child(
                div()
                    .id("score-scroll")
                    .size_full()
                    .overflow_scroll()
                    .track_scroll(&view.viewport.scroll)
                    .py_6()
                    .px(px(64.))
                    .child(score_surface(view, cx)),
            ),
        )
}

fn score_surface(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let scale = view.layout.scale;
    let header_width = if view.ui.view_mode == score_layout::ViewMode::SingleSystem {
        view.layout.width.min(1000.)
    } else {
        view.layout.width
    };
    div()
        .relative()
        .flex_shrink_0()
        .w(px(view.layout.width * scale))
        .h(px(view.layout.height * scale))
        .when(view.ui.note_entry, |surface| {
            surface.cursor(CursorStyle::Crosshair)
        })
        .when(view.hover.is_some(), |surface| surface.cursor_pointer())
        .on_scroll_wheel(cx.listener(AppView::score_wheel))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(AppView::open_score_context_menu),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(AppView::begin_passage_selection),
        )
        .child(paint::score_canvas(
            view.layout.clone(),
            paint::CanvasState {
                show_ranges: view.review.show_ranges,
                score: view
                    .note_drag
                    .as_ref()
                    .and_then(|drag| drag.preview.as_ref())
                    .or_else(|| {
                        view.connection_drag
                            .as_ref()
                            .and_then(|drag| drag.preview.as_ref())
                    })
                    .or_else(|| {
                        view.mark_drag
                            .as_ref()
                            .and_then(|drag| drag.preview_score())
                    })
                    .unwrap_or_else(|| editor.score())
                    .clone(),
                notation_point: view.document.editor.divisi_position(),
                selection: editor.selection(),
                selection_range: editor.selected_ids(),
                cursor: editor.cursor(),
                note_entry: view.ui.note_entry,
                playing: view.playback.is_playing() || view.playback.is_paused(),
                selected_pitch: view
                    .note_drag
                    .as_ref()
                    .and_then(|d| d.target)
                    .or_else(|| editor.selected_pitch()),
                selected_mark: view
                    .mark_drag
                    .as_ref()
                    .filter(|drag| drag.active)
                    .and_then(|drag| drag.shown_mark())
                    .or_else(|| view.selected_mark()),
                selected_bar_mark: view.selected_bar_mark(),
                line_handles: view.line_handles(),
                anchor_lines: view
                    .mark_drag
                    .as_ref()
                    .map(|drag| drag.anchor_lines())
                    .unwrap_or_default(),
                hover: view.hover,
                playheads: view.playback.playheads.clone(),
                position: view.position,
                time_target: view.note_drag.as_ref().and_then(|drag| drag.time_target),
                ghost: view.ghost.filter(|_| {
                    view.ui.note_entry && !view.playback.is_playing() && !view.playback.is_paused()
                }),
            },
            cx.weak_entity(),
            view.music_font.clone(),
        ))
        .child(score_header(view, header_width, cx))
        .children(copyright(view))
}

/// Title block of the first page. Clicking it opens the header editor.
fn score_header(view: &AppView, width: f32, cx: &Context<AppView>) -> Stateful<Div> {
    let score = view.document.editor.score();
    let credits = score.credits();
    let scale = view.layout.scale;
    let font = view.music_font.clone();
    let line = |text: String, size: f32, color: u32| {
        div()
            .text_size(px(size * scale))
            .line_height(px(size * 1.3 * scale))
            .text_color(rgb(color))
            .truncate()
            .child(inline_text(font.clone(), &text))
    };
    let empty = credits == &score_core::Credits::default();
    div()
        .id("score-header")
        .absolute()
        .top(px(35. * scale))
        .left_0()
        .w(px(width * scale))
        .px(px(48. * scale))
        .py(px(4. * scale))
        .rounded_md()
        .cursor_pointer()
        .hover(|header| header.bg(gpui::rgba(HEADER_HOVER)))
        .text_center()
        .text_color(rgb(PAPER_INK))
        .font_family("Times New Roman")
        .tooltip(command_hint(view, id::SCORE_INFO))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|v, _, window, cx| {
                cx.stop_propagation();
                if v.header_editor.is_none() {
                    v.toggle_header_editor(window, cx);
                }
            }),
        )
        .child(
            div()
                .text_size(px(40. * scale))
                .line_height(px(46. * scale))
                .h(px(46. * scale))
                .truncate()
                .child(inline_text(font.clone(), score.title()).justify_center()),
        )
        .child(
            div()
                .mt(px(6. * scale))
                .h(px(24. * scale))
                .flex()
                .justify_center()
                .child(line(credits.subtitle.clone(), 18., PAPER_SECONDARY)),
        )
        .child(
            div()
                .mt(px(8. * scale))
                .flex()
                .justify_between()
                .items_start()
                .gap(px(24. * scale))
                .child(div().flex_1().min_w_0().text_left().child(line(
                    credits.lyricist_line(),
                    14.,
                    PAPER_SECONDARY,
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .items_end()
                        .when(empty, |column| {
                            column.child(line(
                                "Compositor, arreglista, año…".into(),
                                14.,
                                PAPER_PLACEHOLDER,
                            ))
                        })
                        .child(line(credits.composer_line(), 14., PAPER_SECONDARY))
                        .child(line(credits.arranger_line(), 14., PAPER_SECONDARY)),
                ),
        )
}
/// Copyright at the foot of the first page, as the PDF prints it.
fn copyright(view: &AppView) -> Option<Div> {
    let text = &view.document.editor.score().credits().copyright;
    let page = view.layout.pages.first()?;
    if text.is_empty() || view.ui.view_mode != score_layout::ViewMode::Page {
        return None;
    }
    let scale = view.layout.scale;
    Some(
        div()
            .absolute()
            .top(px((page.top + page.height - 52.) * scale))
            .left_0()
            .w(px(page.width * scale))
            .px(px(56. * scale))
            .text_center()
            .truncate()
            .font_family("Times New Roman")
            .text_size(px(10. * scale))
            .text_color(rgb(PAPER_FINE))
            .child(text.clone()),
    )
}

fn inspector(view: &AppView, cx: &Context<AppView>) -> Div {
    div()
        .w(px(RIGHT_WIDTH))
        .flex_shrink_0()
        .bg(rgb(PANEL))
        .border_l_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .child(
            div()
                .flex_shrink_0()
                .px(px(10.))
                .py_2()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(toolbar::track(RightTab::ALL.into_iter().map(|tab| {
                    toolbar::control(
                        format!("inspector-{}", tab.label()),
                        tab.label(),
                        view.ui.right == tab,
                        cx,
                        move |v, _, cx| {
                            v.ui.right = tab;
                            cx.notify();
                        },
                    )
                    .h(px(26.))
                    .text_size(px(12.))
                }))),
        )
        .child(
            div()
                .id("inspector-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .when(view.ui.right == RightTab::Selection, |body| {
                    body.child(selection_panel::panel(view, cx))
                })
                // The mixer stays here, above the sounds, until it moves to the left panel.
                .when(view.ui.right == RightTab::Sound, |body| {
                    body.child(mixer(view, cx)).child(sounds(view, cx))
                })
                .when(view.ui.right == RightTab::Review, |body| {
                    body.child(review::panel(view, cx))
                }),
        )
}
fn section(label: &'static str) -> Div {
    div()
        .px_4()
        .py_4()
        .flex()
        .flex_col()
        .gap_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(caption(label))
}
fn active_duration(view: &AppView) -> Duration {
    let editor = &view.document.editor;
    if view.ui.note_entry {
        return editor.duration;
    }
    editor
        .selection()
        .and_then(|id| editor.score().event(id))
        .map_or(editor.duration, |e| e.duration)
}
fn selected_connection(view: &AppView, kind: ConnectionKind) -> bool {
    view.document.editor.selection().is_some_and(|id| {
        view.document
            .editor
            .score()
            .connections()
            .iter()
            .any(|c| c.kind == kind && view.document.editor.score().connection_contains(*c, id))
    })
}
fn mixer(view: &AppView, cx: &Context<AppView>) -> Div {
    mixer::panel(view, cx)
}

fn sounds(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let instrument = editor
        .score()
        .part(editor.cursor().part)
        .unwrap()
        .instrument();
    section("SONIDO DEL INSTRUMENTO")
        .child(caption("INTERPRETACIÓN"))
        .child(button(view, "interpret-natural", "Natural", view.playback.style == score_playback::Style::Natural, cx, |v, _, cx| {
            if v.playback.style != score_playback::Style::Natural {
                v.playback.stop();
                v.playback.style = score_playback::Style::Natural;
                v.status = "Interpretación: natural".into();
                cx.notify();
            }
        }).w_full())
        .child(button(view, "interpret-literal", "Literal (exacta)", view.playback.style == score_playback::Style::Literal, cx, |v, _, cx| {
            if v.playback.style != score_playback::Style::Literal {
                v.playback.stop();
                v.playback.style = score_playback::Style::Literal;
                v.status = "Interpretación: literal".into();
                cx.notify();
            }
        }).w_full())
        .child(caption("FUENTE DE SONIDO"))
        .child(button(view, "sound-generaluser", "GeneralUser GS", view.playback.selected_bank.is_none(), cx, |v, _, cx| v.choose_sound(None, cx)).w_full())
        .children(view.playback.banks.iter().enumerate().map(|(index, bank)| {
            button(view, format!("sound-{}", bank.id()), bank.name().to_owned(), view.playback.selected_bank == Some(index), cx,
                move |v, _, cx| v.choose_sound(Some(index), cx)).w_full()
        }))
        .when(view.playback.selected_bank.and_then(|index| view.playback.banks.get(index))
            .is_some_and(|bank| bank.id() == "sfz-combinadas"), |body| {
            body.child(caption("PRIORIDAD"))
                .children(view.playback.sound_priority.iter().enumerate().map(|(index, id)| {
                    div().flex().items_center().gap_1()
                        .child(div().flex_1().text_size(px(11.)).child(view.playback.sound_priority_name(id)))
                        .when(index > 0, |row| row.child(button(view, format!("priority-up-{index}"), "↑", false, cx,
                            move |v, _, cx| match v.playback.move_sound_priority(index, true) {
                                Ok(bank) => v.choose_sound(Some(bank), cx),
                                Err(error) => { v.playback.sound_error = Some(error.clone()); v.status = error; cx.notify(); }
                            })))
                        .when(index + 1 < view.playback.sound_priority.len(), |row| row.child(button(view, format!("priority-down-{index}"), "↓", false, cx,
                            move |v, _, cx| match v.playback.move_sound_priority(index, false) {
                                Ok(bank) => v.choose_sound(Some(bank), cx),
                                Err(error) => { v.playback.sound_error = Some(error.clone()); v.status = error; cx.notify(); }
                            })))
                }))
        })
        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(format!(
            "Instala bibliotecas SFZ en {}. Ver docs/sfz.md.",
            crate::data_path("soundpacks").map_or_else(|| "la carpeta de datos".into(), |path| path.display().to_string())
        )))
        .children(view.playback.sound_discovery_errors.iter().map(|error| div().text_size(px(10.)).text_color(rgb(WARNING)).child(error.clone())))
        .children(more_sounds(view, cx))
        .when(!view.playback.banks.is_empty(), |body| body.child(div().text_size(px(10.)).text_color(rgb(MUTED)).child("La primera carga puede tardar unos segundos. Puedes volver a GeneralUser cuando quieras.")))
        .when_some(view.playback.sound_error.clone(), |body, error| body.child(div().text_size(px(11.)).text_color(rgb(WARNING)).child(error)))
        .child(caption("INSTRUMENTO ACTUAL"))
        .child(div().text_size(px(14.)).child(instrument.name()))
        .child(
            div()
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(if let Some(harmonic) = editor.selection().and_then(|id| editor.score().event(id)).and_then(|event| event.notation.harmonic) {
                    format!("{} · {}\nSíntesis de armónicos", harmonic.label(), editor.selection().and_then(|id| editor.score().technique_at(id)).unwrap_or_default().label())
                } else { format!(
                    "General MIDI {}\n{}",
                    editor
                        .selection()
                        .and_then(|id| editor.score().technique_at(id))
                        .unwrap_or_default()
                        .program(instrument)
                        + 1,
                    view.playback.sound_name(),
                ) }),
        )
        .when(instrument.family() == score_core::InstrumentFamily::Strings, |body| body.child(caption("Armónicos con timbre sintetizado: sostenidos al arco y resonantes en pizzicato.")))
        .when_some(view.playback.sound_description(), |body, description| body.child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(description)))
        .child(button(view,
            "add-from-sounds",
            "+ Añadir instrumento",
            false,
            cx,
            |v, _, cx| {
                v.ui.show_instruments = true;
                v.open_instrument_picker(cx);
                v.key_picker = false;
                v.time_picker = false;
                    v.clef_picker = false;
                    v.tempo_picker = None;
                cx.notify();
            },
        ))
        .when(editor.score().parts().len() > 1, |body| {
            body.child(button(view,
                "remove-part",
                "Quitar instrumento",
                false,
                cx,
                |v, _, cx| v.edit(Command::RemovePart, cx),
            ))
        })
        .when(view.playback.error.is_some(), |body| {
            body.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(WARNING))
                    .child(view.playback.error.clone().unwrap()),
            )
            .child(button(view,
                "reconnect-audio",
                "Reintentar audio",
                false,
                cx,
                |v, _, cx| {
                    v.playback.connect();
                    cx.notify();
                },
            ))
        })
}
/// The packs available on demand, when Levare is built with SFZ support.
fn more_sounds(view: &AppView, cx: &Context<AppView>) -> Option<Div> {
    #[cfg(feature = "vsco")]
    {
        Some(sound_store::panel(view, cx))
    }
    #[cfg(not(feature = "vsco"))]
    {
        let _ = (view, cx);
        None
    }
}
fn transport(view: &AppView, cx: &Context<AppView>) -> Div {
    let score = view.document.editor.score();
    let position = view.position;
    let seconds = position.seconds;
    let bar = position.measure;
    let beat = position.beat;
    let progress = position.progress;
    div()
        .h(px(67.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .bg(rgb(BG))
        .border_t_1()
        .border_color(rgb(BORDER))
        .child(
            icon_button("play-from-start", Icon::First, false, cx, |v, _, cx| {
                v.playback_from_start(cx)
            })
            .tooltip(command_hint(view, id::PLAY_FROM_START)),
        )
        .child(
            action(
                "toggle-playback",
                icon(
                    if view.playback.is_playing() {
                        Icon::Pause
                    } else {
                        Icon::Play
                    },
                    BG,
                ),
                false,
                cx,
                |v, _, cx| v.toggle_playback(cx),
            )
            .w(px(38.))
            .h(px(38.))
            .rounded_full()
            .tooltip(command_hint(view, id::PLAY))
            .bg(rgb(TEXT)),
        )
        .child(
            icon_button("stop-playback", Icon::Stop, false, cx, |v, _, cx| {
                v.stop_playback(cx)
            })
            .tooltip(command_hint(view, id::STOP)),
        )
        .child(
            action(
                "loop",
                icon(Icon::Loop, if view.playback.looping { BLUE } else { MUTED }),
                view.playback.looping,
                cx,
                |v, _, cx| v.toggle_loop(cx),
            )
            .w(px(34.))
            .h(px(28.))
            .tooltip(command_hint(view, id::LOOP)),
        )
        .child(
            action(
                "half-speed",
                div()
                    .text_size(px(11.))
                    .text_color(rgb(if view.playback.half_speed {
                        BLUE
                    } else {
                        MUTED
                    }))
                    .child("½×"),
                view.playback.half_speed,
                cx,
                |v, _, cx| v.toggle_half_speed(cx),
            )
            .w(px(34.))
            .h(px(28.))
            .tooltip(command_hint(view, id::HALF_SPEED)),
        )
        .child(
            div()
                .w(px(160.))
                .ml_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption("COMPÁS · TIEMPO").text_size(px(8.)))
                .child(match &view.go_to_bar {
                    Some(field) => div().h(px(28.)).child(field.clone()),
                    None => div().child(
                        div()
                            .id("go-to-bar")
                            .cursor_pointer()
                            .rounded_md()
                            .hover(|style| style.bg(rgb(HOVER)))
                            .font_family("Menlo")
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(21.))
                            .tooltip(command_hint(view, id::GO_TO_BAR))
                            .on_click(cx.listener(|v, _, window, cx| v.open_go_to_bar(window, cx)))
                            .child(if position.at_end {
                                format!("{:03} : FIN", bar + 1)
                            } else {
                                format!("{:03} : {:02}", bar + 1, beat.floor() as u16)
                            }),
                    ),
                })
                .child(
                    div()
                        .text_size(px(9.))
                        .text_color(rgb(MUTED))
                        .child(format!(
                            "{:02}:{:02},{:02} · {}",
                            (seconds / 60.) as u64,
                            seconds as u64 % 60,
                            (seconds.fract() * 100.) as u64,
                            if view.playback.is_playing() {
                                "Reproduciendo"
                            } else if view.playback.is_paused() {
                                "Pausa"
                            } else {
                                "Posición"
                            }
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption("TEMPO"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            button(view, "tempo-minus", "−", false, cx, |v, _, cx| {
                                v.change_written_tempo(-5, cx);
                                cx.notify();
                            })
                            .h(px(22.))
                            .px_1(),
                        )
                        .child(
                            button(
                                view,
                                "tempo-value",
                                score.tempo_at(position.measure).bpm.to_string(),
                                view.tempo_picker.is_some(),
                                cx,
                                move |v, _, cx| v.toggle_tempo_at(position.measure, cx),
                            )
                            .h(px(22.))
                            .px_1(),
                        )
                        .child(
                            button(view, "tempo-plus", "+", false, cx, |v, _, cx| {
                                v.change_written_tempo(5, cx);
                                cx.notify();
                            })
                            .h(px(22.))
                            .px_1(),
                        ),
                ),
        )
        .child(timeline(score, progress, cx))
        .child(
            div()
                .id("sound-source")
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(FIELD))
                .child(icon(Icon::Speaker, MUTED))
                .child(
                    div()
                        .text_size(px(10.))
                        .text_color(rgb(MUTED))
                        .child(view.playback.sound_name()),
                )
                .cursor_pointer()
                .tooltip(hover_hint("Elegir fuente de sonido", None))
                .on_click(cx.listener(|view, _, _, cx| {
                    view.ui.show_inspector = true;
                    view.ui.right = RightTab::Sound;
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption("MIDI"))
                .child(pending_badge()),
        )
}
fn timeline(score: &score_core::Score, progress: f32, cx: &Context<AppView>) -> impl IntoElement {
    let bars = score.measures().len();
    let total = score.total_ticks() as f32;
    let boundaries: Vec<_> = (0..bars)
        .map(|bar| {
            (
                score.measure_start_tick(bar) as f32 / total,
                score.measure_start_tick(bar + 1) as f32 / total,
                score.time_at(bar).numerator,
            )
        })
        .collect();
    let hit_bounds = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let painted_bounds = hit_bounds.clone();
    div()
        .id("playback-timeline")
        .cursor_pointer()
        .flex_1()
        .min_w(px(80.))
        .h(px(36.))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, event: &MouseDownEvent, window, cx| {
                if let Some(bounds) = *hit_bounds.borrow() {
                    let tick = crate::playhead::ruler_tick(
                        view.document.editor.score(),
                        f32::from(event.position.x - bounds.left()),
                        f32::from(bounds.size.width),
                    );
                    view.focus.focus(window);
                    view.seek_playback(tick, true, cx);
                    cx.stop_propagation();
                }
            }),
        )
        .child(
            canvas(
                move |bounds, _, _| {
                    *painted_bounds.borrow_mut() = Some(bounds);
                },
                move |bounds, _, window, cx| {
                    window.paint_quad(quad(
                        bounds,
                        px(4.),
                        rgb(TRACK),
                        px(1.),
                        rgb(BORDER),
                        Default::default(),
                    ));
                    let width = f32::from(bounds.size.width);
                    let usable = (width - 16.).max(1.);
                    let step = (bars as f32 * 48. / usable).ceil().max(1.) as usize;
                    for (index, (start, end, beats)) in boundaries.iter().enumerate() {
                        let x = 8. + usable * start;
                        let bar_width = usable * (end - start);
                        if index % step == 0 {
                            paint::text_at(
                                &(index + 1).to_string(),
                                font(".SystemUIFont"),
                                9.,
                                bounds.origin + point(px(x + 3.), px(12.)),
                                MUTED,
                                window,
                                cx,
                            );
                            window.paint_quad(fill(
                                Bounds::new(
                                    bounds.origin + point(px(x), px(17.)),
                                    size(px(1.), px(17.)),
                                ),
                                rgb(TICK),
                            ));
                        }
                        if bar_width / f32::from(*beats) >= 7. {
                            for beat in 1..*beats {
                                let x = x + bar_width * f32::from(beat) / f32::from(*beats);
                                window.paint_quad(fill(
                                    Bounds::new(
                                        bounds.origin + point(px(x), px(27.)),
                                        size(px(1.), px(7.)),
                                    ),
                                    rgb(MINOR_TICK),
                                ));
                            }
                        }
                    }
                    let x = bounds.origin.x + px(8. + progress.clamp(0., 1.) * usable);
                    crate::playhead::paint_marker(
                        x,
                        bounds.origin.y + px(15.),
                        bounds.bottom() - px(1.),
                        window,
                    );
                },
            )
            .size_full(),
        )
}

fn statusbar(view: &AppView) -> Div {
    let score = view.document.editor.score();
    let measure = view.position.measure;
    let time = score.time_at(measure);
    let recovery_ready = view
        .quit_snapshot
        .as_ref()
        .is_some_and(|recovery| recovery.last_error().is_none());
    let status = if view.busy {
        "Procesando archivo…".to_owned()
    } else if let Some(error) = &view.playback.error {
        error.clone()
    } else {
        view.status.clone()
    };
    div()
        .h(px(25.))
        .flex_shrink_0()
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .border_t_1()
        .border_color(rgb(BORDER))
        .text_size(px(10.))
        .child(
            div()
                .flex()
                .gap_4()
                .min_w_0()
                .child(
                    div()
                        .text_color(rgb(ACCENT_TEXT))
                        .child(if view.ui.note_entry {
                            format!(
                                "Entrada de notas · Voz {}",
                                view.document.editor.cursor().voice.number()
                            )
                        } else {
                            format!(
                                "Selección · Voz {}",
                                view.document.editor.cursor().voice.number()
                            )
                        }),
                )
                .child(div().text_color(rgb(MUTED)).child(inline_text(
                    view.music_font.clone(),
                    format!(
                        "{} · {}/{} · {} instrumentos",
                        score.key_signature_at(measure).label(),
                        time.numerator,
                        time.denominator,
                        score.parts().len()
                    ),
                )))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(MUTED))
                        .child(inline_text(view.music_font.clone(), status)),
                ),
        )
        .child(div().flex_shrink_0().text_color(rgb(MUTED)).child(format!(
            "{} · {} %",
            view.ui.view_mode.label(),
            (view.ui.zoom * 100.).round() as u16
        )))
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgb(if recovery_ready { MUTED } else { ERROR }))
                .child(if recovery_ready {
                    "Autoguardado activo"
                } else {
                    "Autoguardado no disponible"
                }),
        )
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgb(if view.document.is_dirty() {
                    BLUE
                } else {
                    MUTED
                }))
                .child(if view.document.is_dirty() {
                    "Cambios sin guardar"
                } else {
                    "Guardado"
                }),
        )
}
