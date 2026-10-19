//! Native editor chrome. Pending controls are visibly marked TODO; score data
//! and transport readouts always come from the current document/session.
mod command_state;
mod icons;
mod instrument_panel;
mod mixer;
pub use mixer::MixerDrag;
mod preferences;
mod toolbar;
use super::*;
use icons::{Icon, icon};
pub use instrument_panel::InstrumentPicker;
pub use preferences::{Preferences, PreferencesConflict};
use score_core::Duration;
use score_keymap::{CommandId, id};

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
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    cx: &Context<AppView>,
    click: impl Fn(&mut AppView, &mut Window, &mut Context<AppView>) + 'static,
) -> Stateful<Div> {
    action(id, div().child(label.into()), selected, cx, click)
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
    button(element, label, selected, cx, move |v, window, cx| {
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
                "export-todo",
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(icon(Icon::Export, 0x222329))
                    .child("Exportar")
                    .child(todo_badge()),
                false,
                cx,
                |v, _, cx| {
                    v.status = "TODO · Exportar MusicXML, MIDI y PDF".into();
                    cx.notify();
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
    let font = view.music_font.clone();
    let text = text.into();
    let ink = font.bounds(&text, font_size);
    let width = px(frame.0).max(ink.size.width + px(4.));
    let height = px(frame.1).max(ink.size.height + px(4.));
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            font.paint_sized(
                &text,
                bounds.origin
                    + point(
                        (bounds.size.width - ink.size.width) / 2. - ink.origin.x,
                        (bounds.size.height - ink.size.height) / 2. - ink.origin.y,
                    ),
                color,
                font_size,
                window,
            )
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
        move |v, _, cx| v.edit(Command::SetDynamic(Some(mark)), cx),
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
    let Some(part) = score.part(id) else {
        return String::new();
    };
    let matches: Vec<_> = score
        .parts()
        .iter()
        .filter(|p| p.instrument() == part.instrument())
        .map(|p| p.id())
        .collect();
    if matches.len() > 1 {
        let index = matches.iter().position(|part| *part == id).unwrap_or(0);
        format!(
            "{} {}",
            part.instrument().name(),
            [
                "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII",
                "XIV"
            ][index]
        )
    } else {
        part.instrument().name().to_owned()
    }
}
fn clef_name(clef: score_core::Clef) -> &'static str {
    match clef {
        score_core::Clef::Treble => "sol",
        score_core::Clef::Alto => "do",
        score_core::Clef::Bass => "fa",
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
        .child(
            div().flex().gap_1().children(
                [(KeyMode::Major, "Mayores"), (KeyMode::Minor, "Menores")]
                    .into_iter()
                    .map(|(mode, label)| {
                        button(
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
                .child("Para toda la partitura. Los signos individuales tienen prioridad."),
        )
        .children(
            [0, 1, 2, 3, 4, 5, 6, 7, -1, -2, -3, -4, -5, -6, -7]
                .into_iter()
                .map(|fifths| {
                    let key = KeySignature::new(fifths, view.key_mode).unwrap();
                    button(
                        format!("key-{fifths}"),
                        key.label(),
                        view.document.editor.score().key_signature() == key,
                        cx,
                        move |v, _, cx| v.choose_key(key, cx),
                    )
                    .w_full()
                }),
        )
}
fn page_thumbnail(parts: usize) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            window.paint_quad(fill(bounds, rgb(0xf4f0e6)));
            for staff in 0..parts.clamp(1, 5) {
                for line in 0..5 {
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin
                                + point(px(7.), px(24. + staff as f32 * 11. + line as f32 * 1.4)),
                            size(px(43.), px(0.5)),
                        ),
                        rgb(0x8c8c88),
                    ));
                }
            }
        },
    )
    .w(px(57.))
    .h(px(79.))
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
        .and_then(|id| measure.index_of(id.event))
        .unwrap_or(cursor.index);
    let tick: u16 = measure
        .iter()
        .take(count)
        .map(|(_, e)| e.duration.ticks())
        .sum();
    1. + f32::from(tick)
        / (f32::from(score_core::TICKS_PER_WHOLE) / f32::from(editor.score().time().denominator))
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
            "Arrastra sobre las notas para seleccionar · Shift + clic amplía".to_owned()
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
    let time = score.time();
    let position = view.playback.position(score, cursor);
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
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(music_symbol(view, "\u{e1d5}", 24., (12., 20.), TEXT))
                                .child(view.playback.tempo.bpm().to_string()),
                        ))
                        .child(readout(
                            "MÉTRICA",
                            format!("{}/{}", time.numerator, time.denominator),
                        ))
                        .child(
                            button(
                                "key-readout",
                                score.key_signature().label(),
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
                            format!("{} · Voz 1", instrument_name(score, cursor.part)),
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
                        .child(
                            div()
                                .w(px(view.layout.width * view.layout.scale))
                                .min_h(px(1050.))
                                .bg(rgb(0xfbf9f3))
                                .shadow_lg()
                                .border_1()
                                .border_color(rgb(0xe1ded6))
                                .child(
                                    div()
                                        .pt(px(32.))
                                        .pb_2()
                                        .px_10()
                                        .text_center()
                                        .text_color(rgb(0x242323))
                                        .font_family("Times New Roman")
                                        .child(
                                            div()
                                                .text_size(px(29.))
                                                .child(score.title().to_owned()),
                                        )
                                        .child(
                                            div()
                                                .mt_2()
                                                .text_size(px(13.))
                                                .text_color(rgb(0x67635c))
                                                .child(format!(
                                                    "{} · {}",
                                                    score.key_signature().label(),
                                                    if score.parts().len() == 1 {
                                                        "para un instrumento".to_owned()
                                                    } else {
                                                        format!(
                                                            "para {} instrumentos",
                                                            score.parts().len()
                                                        )
                                                    }
                                                )),
                                        )
                                        .child(
                                            div()
                                                .mt_3()
                                                .text_right()
                                                .text_size(px(10.))
                                                .text_color(rgb(0x878277))
                                                .child("Compositor · TODO"),
                                        ),
                                )
                                .child(
                                    div()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(AppView::begin_passage_selection),
                                        )
                                        .child(paint::score_canvas(
                                            view.layout.clone(),
                                            paint::CanvasState {
                                                selection: editor.selection(),
                                                selection_range: editor.selected_ids(),
                                                cursor,
                                                selected_pitch: editor.selected_pitch(),
                                                selected_mark: view.selected_mark(),
                                                playheads: view.playback.playheads.clone(),
                                                position,
                                                capacity: time.capacity(),
                                            },
                                            cx.weak_entity(),
                                            view.music_font.clone(),
                                        )),
                                ),
                        ),
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
                        .child(pending("page-mode-todo", "Página", cx))
                        .child(
                            div()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .bg(rgb(0x373a43))
                                .text_size(px(11.))
                                .child("Continua"),
                        )
                        .child(pending("single-staff-todo", "Pentagrama", cx))
                        .child(
                            div()
                                .text_size(px(10.))
                                .text_color(rgb(MUTED))
                                .px_2()
                                .child(format!("{} %", (view.ui.zoom * 100.).round() as u16)),
                        )
                        .child(button("zoom-out", "−", false, cx, |v, window, cx| {
                            v.run(id::ZOOM_OUT, window, cx)
                        }))
                        .child(button("zoom-in", "+", false, cx, |v, window, cx| {
                            v.run(id::ZOOM_IN, window, cx)
                        }))
                        .child(div()),
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
    let key = score.key_signature();
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
                                    .child(
                                        pitch
                                            .map(|p| note_name(p.in_key(key)))
                                            .unwrap_or_else(|| format!("Octava {}", editor.octave)),
                                    ),
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
                                    .child(event.map_or(editor.duration, |e| e.duration).label()),
                            )
                            .when(event.is_some(), |column| column.child(todo_badge())),
                    ),
            )
            .when(event.is_some_and(|e| e.pitches.len() > 1), |body| {
                body.child(div().flex().flex_wrap().gap_1().children(
                    event.unwrap().pitches.iter().enumerate().map(|(index, p)| {
                        button(
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
            .child(caption("En entrada se conserva el modo. Los tresillos se agrupan de tres en tres con la misma figura."))
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
            .child(caption("Voz"))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        div()
                            .flex_1()
                            .text_center()
                            .rounded_md()
                            .p_2()
                            .text_size(px(11.))
                            .text_color(rgb(BLUE))
                            .bg(rgb(SELECT))
                            .child("●  1"),
                    )
                    .children(["2", "3", "4"].into_iter().map(|voice| {
                        div()
                            .flex_1()
                            .text_center()
                            .rounded_md()
                            .p_2()
                            .text_size(px(10.))
                            .text_color(rgb(MUTED))
                            .bg(rgb(FIELD))
                            .child(format!("● {voice}"))
                            .child(todo_badge())
                    })),
            )
            .child(caption("Plica · TODO"))
            .child(div().flex().gap_1().children(
                ["Auto", "Arriba", "Abajo"].into_iter().map(|text| {
                    div()
                        .flex_1()
                        .rounded_md()
                        .py_1()
                        .text_center()
                        .text_size(px(11.))
                        .text_color(rgb(MUTED))
                        .bg(rgb(FIELD))
                        .child(text)
                }),
            )),
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
                        .child(button(
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
                        .child(button("octave-up", "+ Octava", false, cx, |v, _, cx| {
                            if !v.busy && !v.confirming {
                                v.document.editor.octave = (v.document.editor.octave + 1).min(8);
                                cx.notify();
                            }
                        })),
                )
                .child(
                    button(
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
                .child(button(
                    "property-chord",
                    "Añadir notas al acorde",
                    view.chord_entry,
                    cx,
                    |v, _, cx| v.toggle_chord(cx),
                ))
                .child(button(
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
                .child(button(
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
    div().flex().flex_wrap().gap_1().children(
        [
            (ConnectionKind::Crescendo, id::CRESCENDO, "\u{e53e}"),
            (ConnectionKind::Diminuendo, id::DIMINUENDO, "\u{e53f}"),
        ]
        .into_iter()
        .map(|(kind, command, glyph)| {
            let label = if view.pending_hairpin.is_some_and(|(_, k)| k == kind) {
                format!("Terminar {}", kind.label().to_lowercase())
            } else {
                kind.label().to_owned()
            };
            symbol_command_button(
                format!("{prefix}-{}", kind.label()),
                (glyph, &label),
                view.pending_hairpin.is_some_and(|(_, k)| k == kind)
                    || selected_connection(view, kind),
                view,
                cx,
                command,
            )
        }),
    )
}
fn technique_controls(view: &AppView, cx: &Context<AppView>, prefix: &'static str) -> Div {
    let editor = &view.document.editor;
    let mark = editor
        .selection()
        .and_then(|id| editor.score().event(id))
        .and_then(|event| event.notation.technique);
    div().flex().flex_col().gap_1().children(
        [
            (PlayingTechnique::Pizzicato, id::PIZZICATO),
            (PlayingTechnique::Arco, id::ARCO),
        ]
        .into_iter()
        .map(|(technique, command)| {
            action(
                format!(
                    "{prefix}-{}",
                    if technique == PlayingTechnique::Pizzicato {
                        "pizzicato"
                    } else {
                        "arco"
                    }
                ),
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
            .tooltip(command_hint(view, command))
        }),
    )
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
        .child(caption("Articulación del acorde o nota"))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(Articulation::ALL.into_iter().map(|mark| {
                    symbol_command_button(
                        format!("property-articulation-{}", mark.label()),
                        (mark.glyph(), mark.label()),
                        selected_articulation(view) == Some(mark),
                        view,
                        cx,
                        articulation_command(mark),
                    )
                })),
        )
        .child(button(
            "clear-articulation",
            "Sin articulación",
            selected_articulation(view).is_none(),
            cx,
            |v, _, cx| v.edit(Command::SetArticulation(None), cx),
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
        .child(button("sound-generaluser", "GeneralUser GS", view.playback.selected_bank.is_none(), cx, |v, _, cx| v.choose_sound(None, cx)).w_full())
        .children(view.playback.banks.iter().enumerate().map(|(index, bank)| {
            button(format!("sound-{}", bank.id()), bank.name(), view.playback.selected_bank == Some(index), cx,
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
                .child(format!(
                    "General MIDI {}\n{}",
                    editor
                        .selection()
                        .and_then(|id| editor.score().technique_at(id))
                        .unwrap_or_default()
                        .program(instrument)
                        + 1,
                    view.playback.sound_name(),
                )),
        )
        .when_some(view.playback.sound_description(), |body, description| body.child(div().text_size(px(10.)).text_color(rgb(MUTED)).child(description)))
        .child(button(
            "add-from-sounds",
            "+ Añadir instrumento",
            false,
            cx,
            |v, _, cx| {
                v.ui.show_instruments = true;
                v.open_instrument_picker(cx);
                v.key_picker = false;
                cx.notify();
            },
        ))
        .when(editor.score().parts().len() > 1, |body| {
            body.child(button(
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
            .child(button(
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
fn articulation_command(mark: Articulation) -> CommandId {
    match mark {
        Articulation::Staccato => id::STACCATO,
        Articulation::Tenuto => id::TENUTO,
        Articulation::Accent => id::ACCENT,
        Articulation::Marcato => id::MARCATO,
    }
}

fn transport(view: &AppView, cx: &Context<AppView>) -> Div {
    let score = view.document.editor.score();
    let position = view.playback.position(score, view.document.editor.cursor());
    let seconds = position.seconds;
    let bar = position.measure;
    let beat = position.beat;
    let bars = score.measures().len();
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
            .w(px(56.))
            .h(px(28.))
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
                            button("tempo-minus", "−", false, cx, |v, _, cx| {
                                v.playback.change_tempo(-5);
                                cx.notify();
                            })
                            .h(px(22.))
                            .px_1(),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .child(view.playback.tempo.bpm().to_string()),
                        )
                        .child(
                            button("tempo-plus", "+", false, cx, |v, _, cx| {
                                v.playback.change_tempo(5);
                                cx.notify();
                            })
                            .h(px(22.))
                            .px_1(),
                        ),
                ),
        )
        .child(timeline(bars, score.time().numerator, progress, cx))
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
fn timeline(bars: usize, beats: u8, progress: f32, cx: &Context<AppView>) -> impl IntoElement {
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
                    view.seek_playback(tick, cx);
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
                    let bar_width = usable / bars.max(1) as f32;
                    for index in (0..bars).step_by(step) {
                        let x = 8. + bar_width * index as f32;
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
                    if bar_width / f32::from(beats) >= 7. {
                        for index in 0..bars {
                            for beat in 1..beats {
                                let x = 8.
                                    + bar_width
                                        * (index as f32 + f32::from(beat) / f32::from(beats));
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
    let time = score.time();
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
                            "Entrada de notas · Voz 1"
                        } else {
                            "Selección · Voz 1"
                        }),
                )
                .child(div().text_color(rgb(MUTED)).child(format!(
                    "{} · {}/{} · {} instrumentos",
                    score.key_signature().label(),
                    time.numerator,
                    time.denominator,
                    score.parts().len()
                )))
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_color(rgb(MUTED))
                        .child(status),
                ),
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
