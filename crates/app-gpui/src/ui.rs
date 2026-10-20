//! Native editor chrome. Pending controls are visibly marked TODO; score data
//! and transport readouts always come from the current document/session.
mod command_state;
mod export_picker;
pub use export_picker::ExportPicker;
mod icons;
mod instrument_panel;
mod mixer;
mod music_text;
mod tempo_editor;
pub mod text_input;
pub use mixer::MixerDrag;
use music_text::inline_text;
pub use tempo_editor::TempoEditor;
mod preferences;
mod toolbar;
use super::*;
use icons::{Icon, icon};
pub use instrument_panel::InstrumentPicker;
pub use preferences::{Preferences, PreferencesConflict};
use score_core::Duration;
use score_keymap::{CommandId, id};
use score_toolbar::CommandStates;

pub const BG: u32 = 0x131417;
pub const TEXT: u32 = 0xe8e9ee;
const PANEL: u32 = 0x18191d;
const FIELD: u32 = 0x222329;
const BORDER: u32 = 0x2b2d35;
const MUTED: u32 = 0x969ba7;
const BLUE: u32 = 0x79a4ff;
const SELECT: u32 = 0x22314d;
pub const LEFT_WIDTH: f32 = 240.;
pub const RIGHT_WIDTH: f32 = 286.;

pub struct UiState {
    pub show_instruments: bool,
    pub show_inspector: bool,
    pub note_entry: bool,
    pub compact: bool,
    pub zoom: f32,
    pub view_mode: score_layout::ViewMode,
    pub tab: score_toolbar::TabId,
    pub inspector: &'static str,
    /// Archivo dropdown and its "Abrir reciente" submenu.
    pub file_menu: bool,
    pub recent_menu: bool,
    /// The folded ribbon group whose dropdown is open, by position.
    pub open_group: Option<usize>,
    /// Window width, for the ribbon's fold decision.
    pub width: f32,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            show_instruments: true,
            show_inspector: true,
            note_entry: true,
            compact: false,
            zoom: 0.7,
            view_mode: Default::default(),
            tab: score_toolbar::TabId::NoteInput,
            inspector: "Propiedades",
            file_menu: false,
            recent_menu: false,
            open_group: None,
            width: 1440.,
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
            Hsla::from(rgb(0x3c5d94))
        } else {
            transparent_black()
        })
        .bg(rgb(if selected { SELECT } else { FIELD }))
        .text_color(rgb(if selected { 0xb8ceff } else { TEXT }))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x30343e)))
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
fn todo_badge() -> Div {
    div()
        .text_size(px(8.))
        .text_color(rgb(0xa3a7b2))
        .px_1()
        .rounded_sm()
        .border_1()
        .border_color(rgb(BORDER))
        .child("TODO")
}
fn pending(
    id: impl Into<SharedString>,
    label: &'static str,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    action(
        id,
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(label)
            .child(todo_badge()),
        false,
        cx,
        move |view, _, cx| {
            view.status = format!("TODO · {label}");
            cx.notify();
        },
    )
    .px_2()
    .h(px(28.))
    .text_color(rgb(MUTED))
    .bg(rgb(PANEL))
}
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
        .border_color(rgb(0x3b3d46))
        .text_size(px(9.))
        .text_color(rgb(MUTED))
        .child(text.into())
}
struct HoverHint {
    label: SharedString,
    keys: Option<String>,
}
impl Render for HoverHint {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_1()
            .max_w(px(360.))
            .rounded_md()
            .border_1()
            .border_color(rgb(0x3b3d46))
            .bg(rgb(FIELD))
            .shadow_md()
            .text_size(px(11.))
            .text_color(rgb(TEXT))
            .child(self.label.clone())
            .children(self.keys.clone().map(shortcut))
    }
}
fn hover_hint(
    label: impl Into<SharedString>,
    keys: Option<String>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    move |_, cx| {
        cx.new(|_| HoverHint {
            label: label.clone(),
            keys: keys.clone(),
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
                .when(view.ui.show_inspector, |row| row.child(inspector(view, cx))),
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
        .child(music_symbol(view, "\u{e1d7}", 30., (20., 30.), BLUE))
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_size(px(14.))
                .mr_2()
                .child("Partitura"),
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
        .child(pending("document-tabs-todo", "+", cx))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
        )
        .child(
            div()
                .w(px(if view.ui.compact { 130. } else { 270. }))
                .h(px(29.))
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .child(icon(Icon::Search, MUTED))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .truncate()
                        .child(if view.ui.compact {
                            "Buscar…"
                        } else {
                            "Buscar comandos, instrumentos…"
                        }),
                )
                .child(todo_badge()),
        )
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
        .child(icon_button(
            "toggle-instruments",
            Icon::LeftPanel,
            view.ui.show_instruments,
            cx,
            |v, _, cx| {
                v.ui.show_instruments = !v.ui.show_instruments;
                v.reveal_selection = true;
                cx.notify();
            },
        ))
        .child(icon_button(
            "toggle-inspector",
            Icon::RightPanel,
            view.ui.show_inspector,
            cx,
            |v, _, cx| {
                v.ui.show_inspector = !v.ui.show_inspector;
                v.reveal_selection = true;
                cx.notify();
            },
        ))
        .child(
            action(
                "export-pdf",
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon(Icon::Export, 0x222329))
                    .child("Exportar…"),
                false,
                cx,
                |v, _, cx| {
                    v.export_pdf(cx);
                },
            )
            .px_3()
            .h(px(30.))
            .bg(rgb(0xe8e9ed))
            .text_color(rgb(0x222329)),
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
    let width = px(frame.0).max(ink.size.width + px(4.));
    let height = px(frame.1).max(ink.size.height + px(4.));
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

fn symbol_command_button(
    element: impl Into<SharedString>,
    symbol: (&'static str, &str),
    selected: bool,
    view: &AppView,
    cx: &Context<AppView>,
    command: CommandId,
) -> Stateful<Div> {
    action(
        element,
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(music_symbol(
                view,
                symbol.0,
                30.,
                (18., 22.),
                if selected { BLUE } else { TEXT },
            ))
            .when(!symbol.1.is_empty(), |content| {
                content.child(symbol.1.to_owned())
            }),
        selected,
        cx,
        move |v, window, cx| v.run(command, window, cx),
    )
    .px_2()
    .h(px(29.))
    .tooltip(command_hint(view, command))
}

fn dynamic_button(
    prefix: &str,
    mark: Dynamic,
    view: &AppView,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    let selected = selected_dynamic(view) == Some(mark);
    action(
        format!("{prefix}-{}", mark.label()),
        music_symbol(
            view,
            mark.glyph(),
            30.,
            (26., 23.),
            if selected { BLUE } else { TEXT },
        ),
        selected,
        cx,
        move |v, _, cx| v.edit(Command::ToggleMark(MarkKind::Dynamic(mark)), cx),
    )
    .px_1()
    .h(px(29.))
    .tooltip(hover_hint(format!("Dinámica: {}", mark.label()), None))
}

fn accidental_symbol(view: &AppView, sign: Accidental, selected: bool) -> impl IntoElement {
    let glyph = match sign {
        Accidental::Flat => "\u{e260}",
        Accidental::Natural => "\u{e261}",
        Accidental::Sharp => "\u{e262}",
    };
    music_symbol(
        view,
        glyph,
        30.,
        (18., 25.),
        if selected { BLUE } else { TEXT },
    )
}

fn accidental_label(view: &AppView, sign: Option<Accidental>, selected: bool) -> Div {
    match sign {
        Some(sign) => div().child(accidental_symbol(view, sign, selected)),
        None => div().child("Armadura"),
    }
}
pub(super) fn instrument_name(score: &score_core::Score, id: PartId) -> String {
    score.part_name(id)
}

fn clef_name(clef: score_core::Clef) -> &'static str {
    match clef {
        score_core::Clef::Treble => "sol",
        score_core::Clef::Alto => "do",
        score_core::Clef::Bass => "fa",
        score_core::Clef::Percussion => "percusión",
    }
}
fn key_picker(view: &AppView, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .pb_3()
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(signature_scope(view, cx))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child("Tonalidad de la obra en altura real. Cada parte adapta su armadura."))
        .child(
            div().flex().gap_1().children(
                [(KeyMode::Major, "Mayores"), (KeyMode::Minor, "Menores")]
                    .into_iter()
                    .map(|(mode, label)| {
                        button(
                            view,
                            format!("keys-{label}"),
                            label,
                            view.key_mode == mode,
                            cx,
                            move |v, _, cx| {
                                v.key_mode = mode;
                                cx.notify();
                            },
                        )
                        .px_2()
                    }),
            ),
        )
        .child(
            div()
                .text_size(px(10.))
                .text_color(rgb(MUTED))
                .py_2()
                .child("Hasta el siguiente cambio, en todos los instrumentos. Los signos individuales tienen prioridad."),
        )
        .children(
            [0, 1, 2, 3, 4, 5, 6, 7, -1, -2, -3, -4, -5, -6, -7]
                .into_iter()
                .map(|fifths| {
                    let key = KeySignature::new(fifths, view.key_mode).unwrap();
                    button(
                        view,
                        format!("key-{fifths}"),
                        key.label(),
                        view.document.editor.score().key_signature_at(view.signature_measure()) == key,
                        cx,
                        move |v, _, cx| v.choose_key(key, cx),
                    )
                    .w_full()
                }),
        )
        .when(view.signature_measure() > 0 && view.document.editor.score().key_change_at(view.signature_measure()).is_some(), |picker| {
            picker.child(button(view, "remove-key-change", "Quitar cambio de tonalidad", false, cx, |v, _, cx| {
                v.edit(Command::RemoveKeyChange(v.signature_measure()), cx);
            }).w_full())
        })
}

fn signature_scope(view: &AppView, cx: &Context<AppView>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(caption("APLICAR DESDE"))
        .children(
            [
                (true, "Inicio de la pieza".to_owned()),
                (
                    false,
                    format!(
                        "Compás {}",
                        view.tempo_picker
                            .as_ref()
                            .map_or(view.document.editor.cursor().measure, |editor| editor
                                .measure)
                            + 1
                    ),
                ),
            ]
            .into_iter()
            .map(|(from_start, label)| {
                button(
                    view,
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
                .w_full()
            }),
        )
        .child(
            button(
                view,
                "close-signature-picker",
                "Volver a instrumentos",
                false,
                cx,
                |v, _, cx| {
                    v.time_picker = false;
                    v.tempo_picker = None;
                    v.key_picker = false;
                    cx.notify();
                },
            )
            .w_full(),
        )
}

fn time_picker(view: &AppView, cx: &Context<AppView>) -> Div {
    let time = view.pending_time;
    div().flex().flex_col().gap_2().pb_3().border_b_1().border_color(rgb(BORDER))
        .child(signature_scope(view, cx))
        .child(caption("MÉTRICAS HABITUALES").mt_2())
        .child(div().flex().flex_wrap().gap_1().children([(2,4), (3,4), (4,4), (5,4), (6,8), (7,8), (9,8), (12,8)].into_iter().map(|(numerator, denominator)| {
            let value = score_core::TimeSignature { numerator, denominator };
            button(view, format!("meter-{numerator}-{denominator}"), format!("{numerator}/{denominator}"), view.document.editor.score().time_at(view.signature_measure()) == value, cx, move |v, _, cx| v.choose_time(value, cx))
        })))
        .child(caption("PERSONALIZADA").mt_2())
        .child(div().flex().items_center().gap_2()
            .child(button(view, "meter-numerator-minus", "−", false, cx, |v, _, cx| { v.pending_time.numerator = v.pending_time.numerator.saturating_sub(1).max(1); cx.notify(); }))
            .child(div().flex_1().text_center().child(format!("Numerador: {}", time.numerator)))
            .child(button(view, "meter-numerator-plus", "+", false, cx, |v, _, cx| { v.pending_time.numerator = (v.pending_time.numerator + 1).min(16); cx.notify(); })))
        .child(caption("DENOMINADOR"))
        .child(div().flex().flex_wrap().gap_1().children([1,2,4,8,16].into_iter().map(|denominator| button(view, format!("meter-denominator-{denominator}"), denominator.to_string(), time.denominator == denominator, cx, move |v, _, cx| { v.pending_time.denominator = denominator; cx.notify(); }))))
        .child(button(view, "apply-meter", format!("Aplicar {}/{}", time.numerator, time.denominator), false, cx, |v, _, cx| v.choose_time(v.pending_time, cx)).w_full())
        .child(div().text_size(px(10.)).text_color(rgb(MUTED)).child("Hasta el siguiente cambio, en todos los instrumentos. Se conservan las notas y los compases existentes."))
        .when(view.signature_measure() > 0 && view.document.editor.score().time_change_at(view.signature_measure()).is_some(), |picker| {
            picker.child(button(view, "remove-meter-change", "Quitar cambio de métrica", false, cx, |v, _, cx| v.edit(Command::RemoveTimeChange(v.signature_measure()), cx)).w_full())
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

fn selected_beat(view: &AppView) -> f32 {
    let editor = &view.document.editor;
    let cursor = editor.cursor();
    let Some(measure) = editor
        .score()
        .part(cursor.part)
        .and_then(|p| p.staff(cursor.staff))
        .and_then(|s| s.measures().get(cursor.measure))
    else {
        return 1.;
    };
    let count = editor
        .selection()
        .and_then(|id| measure.voice_index_of(cursor.voice, id.event))
        .unwrap_or(cursor.index);
    let tick: u16 = measure
        .voice_iter(cursor.voice)
        .take(count)
        .map(|(_, e)| e.duration.ticks())
        .sum();
    1. + f32::from(tick)
        / (f32::from(score_core::TICKS_PER_WHOLE)
            / f32::from(editor.score().time_at(cursor.measure).denominator))
}
fn beat_label(beat: f32) -> String {
    let text = format!("{beat:.2}");
    text.trim_end_matches('0')
        .trim_end_matches('.')
        .replace('.', ",")
}
fn readout(label: &'static str, value: impl Into<SharedString>) -> Div {
    readout_content(label, div().child(value.into()))
}
fn readout_content(label: &'static str, value: impl IntoElement) -> Div {
    div()
        .px_3()
        .flex_shrink_0()
        .border_r_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .gap_1()
        .child(caption(label))
        .child(
            div()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(value),
        )
}
fn passage_bar(view: &AppView, cx: &Context<AppView>) -> Div {
    let count = view.document.editor.selected_ids().len();
    div()
        .h(px(33.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .text_size(px(11.))
        .text_color(rgb(MUTED))
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(if count > 1 {
            format!("{count} elementos seleccionados")
        } else {
            "Clic sitúa la reproducción · Arrastra arriba/abajo para cambiar altura; horizontal para mover; Shift para seleccionar".to_owned()
        })
        .when(count > 1, |bar| {
            bar.child(
                command_button("copy-passage", "Copiar", false, view, cx, id::COPY).h(px(25.)),
            )
            .child(command_button("cut-passage", "Cortar", false, view, cx, id::CUT).h(px(25.)))
        })
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
        .bg(rgb(0x24252a))
        .child(
            div()
                .id("score-readouts")
                .h(px(57.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .overflow_x_scroll()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(0x383b43))
                        .bg(rgb(FIELD))
                        .text_size(px(11.))
                        .child("Partitura completa"),
                )
                .child(
                    div()
                        .rounded_lg()
                        .bg(rgb(0x0e0f12))
                        .py_2()
                        .flex()
                        .items_center()
                        .child(readout(
                            "COMPÁS",
                            format!(
                                "{} de {}",
                                (position.measure + 1).min(score.measures().len()),
                                score.measures().len()
                            ),
                        ))
                        .child(readout(
                            "TIEMPO",
                            if position.at_end {
                                "Fin".to_owned()
                            } else {
                                beat_label(position.beat as f32)
                            },
                        ))
                        .child(readout_content(
                            "TEMPO",
                            button(
                                view,
                                "tempo-readout",
                                score.tempo_at(position.measure).label(),
                                view.tempo_picker.is_some(),
                                cx,
                                move |v, _, cx| v.toggle_tempo_at(position.measure, cx),
                            )
                            .tooltip(command_hint(view, id::TEMPO_TEXT)),
                        ))
                        .child(
                            button(
                                view,
                                "meter-readout",
                                format!("{}/{}", time.numerator, time.denominator),
                                view.time_picker,
                                cx,
                                |v, _, cx| v.toggle_time_picker(cx),
                            )
                            .tooltip(command_hint(view, id::TIME_SIGNATURE))
                            .mx_2(),
                        )
                        .child(
                            button(
                                view,
                                "key-readout",
                                score.key_signature_at(position.measure).label(),
                                view.key_picker,
                                cx,
                                |v, _, cx| {
                                    v.ui.show_instruments = true;
                                    v.toggle_key_picker(cx);
                                },
                            )
                            .mx_2(),
                        )
                        .child(readout(
                            "PENTAGRAMA",
                            format!(
                                "{} · Voz {}",
                                instrument_name(score, cursor.part),
                                cursor.voice.number()
                            ),
                        )),
                ),
        )
        .child(passage_bar(view, cx))
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("score-scroll")
                        .size_full()
                        .overflow_scroll()
                        .track_scroll(&view.viewport.scroll)
                        .py_6()
                        .px(px(64.))
                        .child(score_surface(view, cx)),
                )
                .child(
                    div()
                        .absolute()
                        .bottom_3()
                        .right_3()
                        .flex()
                        .items_center()
                        .gap_1()
                        .p_1()
                        .rounded_lg()
                        .shadow_lg()
                        .bg(rgb(PANEL))
                        .border_1()
                        .border_color(rgb(0x3a3d46))
                        .children(
                            [
                                score_layout::ViewMode::Page,
                                score_layout::ViewMode::Continuous,
                                score_layout::ViewMode::SingleSystem,
                            ]
                            .into_iter()
                            .map(|mode| {
                                button(
                                    view,
                                    format!("score-view-{mode:?}"),
                                    mode.label(),
                                    view.ui.view_mode == mode,
                                    cx,
                                    move |v, _, cx| v.set_view_mode(mode, cx),
                                )
                            }),
                        )
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(MUTED))
                                .px_2()
                                .child(format!("{} %", (view.ui.zoom * 100.).round() as u16)),
                        )
                        .child(button(
                            view,
                            "zoom-out",
                            "−",
                            false,
                            cx,
                            |v, window, cx| v.run(id::ZOOM_OUT, window, cx),
                        ))
                        .child(button(view, "zoom-in", "+", false, cx, |v, window, cx| {
                            v.run(id::ZOOM_IN, window, cx)
                        }))
                        .child(div()),
                ),
        )
}

fn score_surface(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let score = editor.score();
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
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(AppView::begin_passage_selection),
        )
        .child(paint::score_canvas(
            view.layout.clone(),
            paint::CanvasState {
                notation_point: view.document.editor.divisi_position(),
                selection: editor.selection(),
                selection_range: editor.selected_ids(),
                cursor: editor.cursor(),
                selected_pitch: view
                    .note_drag
                    .as_ref()
                    .and_then(|d| d.target)
                    .or_else(|| editor.selected_pitch()),
                selected_mark: view.selected_mark(),
                playheads: view.playback.playheads.clone(),
                position: view.position,
                time_target: view.note_drag.as_ref().and_then(|drag| drag.time_target),
            },
            cx.weak_entity(),
            view.music_font.clone(),
        ))
        .child(
            div()
                .absolute()
                .top(px(35. * scale))
                .left_0()
                .w(px(header_width * scale))
                .px(px(48. * scale))
                .text_center()
                .text_color(rgb(0x242323))
                .font_family("Times New Roman")
                .child(
                    div()
                        .text_size(px(40. * scale))
                        .line_height(px(46. * scale))
                        .h(px(46. * scale))
                        .truncate()
                        .child(
                            inline_text(view.music_font.clone(), score.title()).justify_center(),
                        ),
                )
                .child(
                    div()
                        .mt(px(12. * scale))
                        .text_size(px(18. * scale))
                        .line_height(px(24. * scale))
                        .h(px(24. * scale))
                        .truncate()
                        .text_color(rgb(0x67635c))
                        .child(
                            inline_text(
                                view.music_font.clone(),
                                format!(
                                    "{} · {}",
                                    score.key_signature().label(),
                                    if score.parts().len() == 1 {
                                        "para un instrumento".to_owned()
                                    } else {
                                        format!("para {} instrumentos", score.parts().len())
                                    }
                                ),
                            )
                            .justify_center(),
                        ),
                )
                .child(
                    div()
                        .mt(px(18. * scale))
                        .text_right()
                        .text_size(px(14. * scale))
                        .line_height(px(18. * scale))
                        .text_color(rgb(0x878277))
                        .child("Compositor · TODO"),
                ),
        )
}

fn note_name(pitch: score_core::Pitch) -> String {
    let name = match pitch.step {
        score_core::Step::C => "Do",
        score_core::Step::D => "Re",
        score_core::Step::E => "Mi",
        score_core::Step::F => "Fa",
        score_core::Step::G => "Sol",
        score_core::Step::A => "La",
        score_core::Step::B => "Si",
    };
    format!(
        "{name}{} {}",
        pitch.accidental().map_or("", Accidental::symbol),
        pitch.octave()
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
                .h(px(45.))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .border_b_1()
                .border_color(rgb(BORDER))
                .children(
                    ["Propiedades", "Mezclador", "Sonidos"]
                        .into_iter()
                        .map(|tab| {
                            button(
                                view,
                                format!("inspector-{tab}"),
                                tab,
                                view.ui.inspector == tab,
                                cx,
                                move |v, _, cx| {
                                    v.ui.inspector = tab;
                                    cx.notify();
                                },
                            )
                            .px_2()
                            .text_size(px(11.))
                            .bg(rgb(PANEL))
                        }),
                ),
        )
        .child(
            div()
                .id("inspector-content")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .when(view.ui.inspector == "Propiedades", |body| {
                    body.child(properties(view, cx))
                })
                .when(view.ui.inspector == "Mezclador", |body| {
                    body.child(mixer(view, cx))
                })
                .when(view.ui.inspector == "Sonidos", |body| {
                    body.child(sounds(view, cx))
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
fn properties(view: &AppView, cx: &Context<AppView>) -> Div {
    let editor = &view.document.editor;
    let score = editor.score();
    let cursor = editor.cursor();
    let key = score.written_key_at(cursor.part, cursor.measure);
    let event = editor.selection().and_then(|id| score.event(id));
    let pitch = editor.selected_pitch();
    let selected_count = editor.selected_ids().len();
    div()
        .flex()
        .flex_col()
        .child(
            section(if selected_count > 1 {
                "PASAJE SELECCIONADO"
            } else if event.is_some() {
                "NOTA SELECCIONADA"
            } else {
                "ENTRADA DE NOTAS"
            })
            .when(selected_count > 1, |body| {
                body.child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(BLUE))
                        .child(format!(
                            "{selected_count} elementos · propiedades de la nota final"
                        )),
                )
            })
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0xbfc3ce))
                    .child(format!(
                        "{} · Compás {} · Tiempo {}",
                        instrument_name(score, cursor.part),
                        cursor.measure + 1,
                        beat_label(selected_beat(view))
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(caption("Altura"))
                            .child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .bg(rgb(FIELD))
                                    .text_size(px(12.))
                                    .child(inline_text(view.music_font.clone(),
                                        pitch
                                            .map(|p| note_name(p.in_key(key)))
                                            .unwrap_or_else(|| format!("Octava {}", editor.octave)),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(caption("Duración"))
                            .child(
                                div()
                                    .px_2()
                                    .py_2()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .bg(rgb(FIELD))
                                    .text_size(px(12.))
                                    .child(active_duration(view).label()),
                            )
                            .child(caption("Elige la figura en la cinta de Entrada")),
                    ),
            )
            .when(event.is_some_and(|e| e.pitches.len() > 1), |body| {
                body.child(div().flex().flex_wrap().gap_1().children(
                    event.unwrap().pitches.iter().enumerate().map(|(index, p)| {
                        button(view,
                            format!("tone-{index}"),
                            note_name(p.in_key(key)),
                            Some(p) == pitch,
                            cx,
                            move |v, _, cx| {
                                if !v.busy && !v.confirming {
                                    v.document.editor.select_tone(index);
                                    cx.notify();
                                }
                            },
                        )
                        .text_size(px(10.))
                    }),
                ))
            })
            .child(div().flex().flex_wrap().gap_1()
                .child(command_button("property-dotted", "Puntillo", active_duration(view).is_dotted(), view, cx, id::DOT))
                .child(command_button("property-triplet", "Tresillo 3:2", active_duration(view).is_triplet(), view, cx, id::TRIPLET)))
            .child(caption(if view.ui.note_entry { "N desactiva la entrada para corregir notas existentes." } else { "A–G cambia la nota seleccionada. Backspace la convierte en silencio. N activa la entrada." }))
            .child(caption("Los tresillos se agrupan de tres en tres con la misma figura."))
            .child(caption("Alteración de la nota"))
            .child(
                div().flex().gap_1().children(
                    [
                        Some(Accidental::Flat),
                        Some(Accidental::Natural),
                        Some(Accidental::Sharp),
                        None,
                    ]
                    .into_iter()
                    .map(|sign| {
                        action(
                            format!("property-sign-{}", sign.map_or("key", Accidental::symbol)),
                            accidental_label(view, sign, pitch.map_or(view.entry_accidental == sign, |p| p.accidental() == sign)),
                            pitch.map_or(view.entry_accidental == sign, |p| p.accidental() == sign),
                            cx,
                            move |v, _, cx| v.apply_accidental(sign, cx),
                        )
                        .px_2()
                        .h(px(29.))
                    }),
                ),
            )
            .child(caption("Voz activa"))
            .child(div().flex().gap_2()
                .child(command_button("property-voice-one", "Voz 1", cursor.voice == score_core::Voice::One, view, cx, id::VOICE_1))
                .child(command_button("property-voice-two", "Voz 2", cursor.voice == score_core::Voice::Two, view, cx, id::VOICE_2)))
            .child(caption("Dos voces: plicas arriba para la voz 1 y abajo para la voz 2. Cada una conserva su propio ritmo."))
            .child(div().flex().flex_wrap().gap_2()
                .child(command_button("property-divisi", "div.", view.state(id::DIVISI).checked, view, cx, id::DIVISI))
                .child(command_button("property-unison", "unis.", view.state(id::UNISON).checked, view, cx, id::UNISON)))
            .child(caption("div. abre dos pentagramas desde la posición elegida; unis. vuelve a uno. También dentro de un compás."))
        )
        .child(expression_properties(view, cx))
        .child(
            section("ESCRITURA")
                .child(caption("Alteración para próximas notas"))
                .child(
                    div().flex().flex_wrap().gap_1().children(
                        [
                            None,
                            Some(Accidental::Flat),
                            Some(Accidental::Natural),
                            Some(Accidental::Sharp),
                        ]
                        .into_iter()
                        .map(|sign| {
                            action(
                                format!("entry-sign-{}", sign.map_or("key", Accidental::symbol)),
                                accidental_label(view, sign, view.entry_accidental == sign),
                                view.entry_accidental == sign,
                                cx,
                                move |v, _, cx| {
                                    if !v.busy && !v.confirming {
                                        v.entry_accidental = sign;
                                        cx.notify();
                                    }
                                },
                            )
                            .px_2()
                            .h(px(29.))
                        }),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(button(view,
                            "octave-down",
                            "− Octava",
                            false,
                            cx,
                            |v, _, cx| {
                                if !v.busy && !v.confirming {
                                    v.document.editor.octave =
                                        v.document.editor.octave.saturating_sub(1);
                                    cx.notify();
                                }
                            },
                        ))
                        .child(button(view, "octave-up", "+ Octava", false, cx, |v, _, cx| {
                            if !v.busy && !v.confirming {
                                v.document.editor.octave = (v.document.editor.octave + 1).min(8);
                                cx.notify();
                            }
                        })),
                )
                .child(
                    button(view,
                        "insert-rest",
                        "Introducir silencio",
                        false,
                        cx,
                        |v, window, cx| {
                            if !v.busy && !v.confirming {
                                v.ui.note_entry = true;
                                v.run(id::REST, window, cx);
                            }
                        },
                    )
                    .tooltip(command_hint(view, id::REST)),
                )
                .child(button(view,
                    "property-chord",
                    "Añadir notas al acorde",
                    view.chord_entry,
                    cx,
                    |v, _, cx| v.toggle_chord(cx),
                ))
                .child(button(view,
                    "property-key",
                    format!("Tonalidad: {}", key.label()),
                    view.key_picker,
                    cx,
                    |v, _, cx| {
                        v.ui.show_instruments = true;
                        v.toggle_key_picker(cx);
                    },
                )),
        )
        .child(
            section("REPRODUCCIÓN")
                .child(slider_placeholder("Velocidad personalizada", "TODO", 0.69))
                .child(slider_placeholder("Duración personalizada", "TODO", 1.))
                .child(button(view,
                    "audition-property",
                    if view.playback.audition {
                        "● Audición al escribir"
                    } else {
                        "Audición desactivada"
                    },
                    view.playback.audition,
                    cx,
                    |v, _, cx| {
                        v.playback.audition = !v.playback.audition;
                        cx.notify();
                    },
                )),
        )
}
fn selected_articulation(view: &AppView) -> Option<Articulation> {
    view.document
        .editor
        .selection()
        .and_then(|id| view.document.editor.score().event(id))
        .and_then(|e| e.notation.articulation)
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
fn selected_dynamic(view: &AppView) -> Option<Dynamic> {
    view.document
        .editor
        .selection()
        .and_then(|id| view.document.editor.score().event(id))
        .and_then(|e| e.notation.dynamic)
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
fn hairpin_controls(view: &AppView, cx: &Context<AppView>, prefix: &'static str) -> Div {
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .children(MarkFamily::Hairpin.kinds().filter_map(|mark| {
            let MarkKind::Connection(kind) = mark else {
                return None;
            };
            let (command, glyph) = (
                score_toolbar::marks::command(mark),
                mark.symbol().unwrap_or_default(),
            );
            let label = if view.pending_hairpin.is_some_and(|(_, k)| k == kind) {
                format!("Terminar {}", kind.label().to_lowercase())
            } else {
                kind.label().to_owned()
            };
            Some(symbol_command_button(
                format!("{prefix}-{}", kind.label()),
                (glyph, &label),
                view.pending_hairpin.is_some_and(|(_, k)| k == kind)
                    || selected_connection(view, kind),
                view,
                cx,
                command,
            ))
        }))
}
fn technique_controls(view: &AppView, cx: &Context<AppView>, prefix: &'static str) -> Div {
    let editor = &view.document.editor;
    let mark = editor
        .selection()
        .and_then(|id| editor.score().event(id))
        .and_then(|event| event.notation.technique);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .children(MarkFamily::Technique.kinds().filter_map(|kind| {
            let MarkKind::Technique(technique) = kind else {
                return None;
            };
            let command = score_toolbar::marks::command(kind);
            Some(
                action(
                    format!("{prefix}-{}", kind.name().to_lowercase()),
                    div()
                        .font_family("Times New Roman")
                        .italic()
                        .text_size(px(15.))
                        .child(technique.label()),
                    mark == Some(technique),
                    cx,
                    move |v, window, cx| v.run(command, window, cx),
                )
                .px_2()
                .h(px(29.))
                .tooltip(command_hint(view, command)),
            )
        }))
}
fn expression_properties(view: &AppView, cx: &Context<AppView>) -> Div {
    section("EXPRESIÓN")
        .when(
            view.document
                .editor
                .score()
                .part(view.document.editor.cursor().part)
                .unwrap()
                .instrument()
                .family()
                == score_core::InstrumentFamily::Strings,
            |body| {
                let technique = view
                    .document
                    .editor
                    .selection()
                    .and_then(|id| view.document.editor.score().technique_at(id))
                    .unwrap_or_default();
                body.child(caption("Técnica desde esta posición"))
                    .child(technique_controls(view, cx, "property"))
                    .child(command_button(
                        "clear-technique",
                        "Quitar indicación de técnica",
                        false,
                        view,
                        cx,
                        id::CLEAR_TECHNIQUE,
                    ))
                    .child(caption(format!(
                        "Sonido activo: {} · se mantiene hasta la siguiente indicación",
                        technique.label()
                    )))
            },
        )
        .child(caption("Armónicos: natural = altura sonora; artificial = nota pisada + rombo automático a la cuarta. Vuelve a pulsar para quitarlo."))
        .child(caption("Articulación, calderón, arpegio y armónico"))
        .child(
            div().flex().flex_wrap().gap_1().children(
                MarkFamily::Articulation
                    .kinds()
                    .chain(MarkFamily::Fermata.kinds())
                    .chain(MarkFamily::Arpeggio.kinds())
                    .chain(MarkFamily::Harmonic.kinds())
                    .map(|kind| {
                        let written = view.document.editor.selection().and_then(|id| {
                            view.document.editor.score().point_mark(id, kind.family())
                        });
                        symbol_command_button(
                            format!("property-mark-{}", kind.label()),
                            (kind.symbol().unwrap_or_default(), kind.label()),
                            written == Some(kind),
                            view,
                            cx,
                            score_toolbar::marks::command(kind),
                        )
                    }),
            ),
        )
        .child(button(
            view,
            "clear-articulation",
            "Sin articulación",
            selected_articulation(view).is_none(),
            cx,
            |v, _, cx| v.edit(Command::SetArticulation(None), cx),
        ))
        .child(command_button(
            "arpeggio-both-hands",
            "Arpegio en ambas manos",
            view.document
                .editor
                .selection()
                .and_then(|id| view.document.editor.score().arpeggio_at(id))
                .is_some_and(|mark| mark.other_hand.is_some()),
            view,
            cx,
            id::ARPEGGIO_BOTH_HANDS,
        ))
        .child(caption("Reguladores"))
        .child(hairpin_controls(view, cx, "property-hairpin"))
        .child(caption(
            "Selecciona un pasaje, o marca inicio y final con el mismo botón. Esc cancela.",
        ))
        .children(
            [
                (ConnectionKind::Crescendo, id::REMOVE_CRESCENDO),
                (ConnectionKind::Diminuendo, id::REMOVE_DIMINUENDO),
            ]
            .into_iter()
            .filter(|(kind, _)| selected_connection(view, *kind))
            .map(|(kind, command)| {
                command_button(
                    format!("remove-hairpin-{}", kind.label()),
                    format!("Quitar {}", kind.label().to_lowercase()),
                    false,
                    view,
                    cx,
                    command,
                )
            }),
        )
        .child(caption("Dinámica desde esta posición"))
        .child(
            div().flex().flex_wrap().gap_1().children(
                Dynamic::ALL
                    .into_iter()
                    .map(|mark| dynamic_button("property-dynamic", mark, view, cx)),
            ),
        )
        .child(button(
            view,
            "clear-dynamic",
            "Quitar indicación",
            false,
            cx,
            |v, _, cx| v.edit(Command::SetDynamic(None), cx),
        ))
        .child(caption("Ligaduras"))
        .child(command_button(
            "property-tie",
            "Unir con la siguiente",
            selected_connection(view, ConnectionKind::Tie),
            view,
            cx,
            id::TIE,
        ))
        .child(command_button(
            "property-slur",
            if view.pending_slur.is_some() {
                "Terminar expresión"
            } else {
                "Comenzar expresión"
            },
            view.pending_slur.is_some(),
            view,
            cx,
            id::SLUR,
        ))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(
            if view.pending_slur.is_some() {
                match view.shortcuts.label(id::SLUR) {
                    Some(keys) => format!("Selecciona la nota final y pulsa {keys}. Esc cancela."),
                    None => "Selecciona la nota final y pulsa Terminar expresión.".to_owned(),
                }
            } else {
                "Unión: notas o acordes iguales y consecutivos. Expresión: elige inicio y final."
                    .to_owned()
            },
        ))
        .children(
            [ConnectionKind::Tie, ConnectionKind::Slur]
                .into_iter()
                .filter(|kind| selected_connection(view, *kind))
                .map(|kind| {
                    button(
                        view,
                        format!("remove-connection-{}", kind.label()),
                        format!("Quitar {}", kind.label().to_lowercase()),
                        false,
                        cx,
                        move |v, _, cx| v.edit(Command::RemoveConnections(kind), cx),
                    )
                }),
        )
}
fn slider_placeholder(label: &'static str, value: &'static str, fraction: f32) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .text_size(px(10.))
        .text_color(rgb(MUTED))
        .child(div().flex().justify_between().child(label).child(value))
        .child(
            div()
                .relative()
                .h(px(12.))
                .flex()
                .items_center()
                .child(div().h(px(3.)).w_full().bg(rgb(BORDER)))
                .child(
                    div()
                        .absolute()
                        .left(px(fraction * 210.))
                        .w(px(10.))
                        .h(px(10.))
                        .rounded_full()
                        .bg(rgb(0x6c7280)),
                ),
        )
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
        .child(caption("FUENTE DE SONIDO"))
        .child(button(view, "sound-generaluser", "GeneralUser GS", view.playback.selected_bank.is_none(), cx, |v, _, cx| v.choose_sound(None, cx)).w_full())
        .children(view.playback.banks.iter().enumerate().map(|(index, bank)| {
            button(view, format!("sound-{}", bank.id()), bank.name(), view.playback.selected_bank == Some(index), cx,
                move |v, _, cx| v.choose_sound(Some(index), cx)).w_full()
        }))
        .when(!view.playback.banks.is_empty(), |body| body.child(div().text_size(px(10.)).text_color(rgb(MUTED)).child("La primera carga puede tardar unos segundos. Puedes volver a GeneralUser cuando quieras.")))
        .when_some(view.playback.sound_error.clone(), |body, error| body.child(div().text_size(px(11.)).text_color(rgb(0xf2b282)).child(error)))
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
                    .text_color(rgb(0xf2b282))
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
            .bg(rgb(0xe8e9ee)),
        )
        .child(
            icon_button("stop-playback", Icon::Stop, false, cx, |v, _, cx| {
                v.stop_playback(cx)
            })
            .tooltip(command_hint(view, id::STOP)),
        )
        .child(
            action(
                "loop-todo",
                icon(Icon::Loop, MUTED),
                false,
                cx,
                |v, _, cx| {
                    v.status = "TODO · Reproducción en bucle".into();
                    cx.notify();
                },
            )
            .min_w(px(76.))
            .px_2()
            .gap_2()
            .h(px(28.))
            .tooltip(hover_hint("Reproducción en bucle · pendiente", None))
            .child(todo_badge()),
        )
        .child(
            div()
                .w(px(160.))
                .ml_2()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption("COMPÁS · TIEMPO").text_size(px(8.)))
                .child(
                    div()
                        .font_family("Menlo")
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(21.))
                        .child(if position.at_end {
                            format!("{:03} : FIN", bar + 1)
                        } else {
                            format!("{:03} : {:02}", bar + 1, beat.floor() as u16)
                        }),
                )
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
                    view.ui.inspector = "Sonidos";
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(caption("MIDI"))
                .child(todo_badge()),
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
                        rgb(0x1b1d22),
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
                                rgb(0x606572),
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
                                    rgb(0x393d47),
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
        format!("Audio: {error}")
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
                        .text_color(rgb(0xb8ceff))
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
        .child(
            div()
                .flex_shrink_0()
                .text_color(rgb(if recovery_ready { MUTED } else { 0xe69292 }))
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
                    "● Cambios sin guardar"
                } else {
                    "✓ Guardado"
                }),
        )
}
