use super::*;
use score_core::{InstrumentFamily, Players};
use score_platform::mixer::ChannelMix;

#[derive(Clone, Copy)]
enum Target {
    Master,
    ReverbAmount,
    Room,
    Volume(PartId),
    Pan(PartId),
}
pub struct MixerDrag {
    target: Target,
    bounds: Bounds<Pixels>,
}

fn value(view: &AppView, target: Target) -> i32 {
    match target {
        Target::Master => view.playback.volume as i32,
        Target::ReverbAmount => i32::from(view.playback.reverb.amount),
        Target::Room => i32::from(view.playback.reverb.room),
        Target::Volume(part) => i32::from(
            view.playback
                .part_mix(view.document.editor.score(), part)
                .volume,
        ),
        Target::Pan(part) => {
            (i32::from(
                view.playback
                    .part_mix(view.document.editor.score(), part)
                    .pan,
            ) + 100)
                / 2
        }
    }
}
fn set(view: &mut AppView, target: Target, value: i32, cx: &mut Context<AppView>) {
    if view.busy || view.confirming {
        return;
    }
    let value = value.clamp(0, 100);
    match target {
        Target::Master => view
            .playback
            .change_volume(value - view.playback.volume as i32),
        Target::ReverbAmount => view.playback.reverb.amount = value as u8,
        Target::Room => view.playback.reverb.room = value as u8,
        Target::Volume(part) => {
            view.playback
                .part_mix_mut(view.document.editor.score(), part)
                .volume = value as u8
        }
        Target::Pan(part) => {
            view.playback
                .part_mix_mut(view.document.editor.score(), part)
                .pan = (value * 2 - 100) as i8
        }
    }
    view.playback.apply_mixer(
        view.document.editor.score(),
        view.document.editor.cursor().part,
    );
    cx.notify();
}
fn position(bounds: Bounds<Pixels>, x: Pixels) -> i32 {
    ((f32::from(x - bounds.left() - px(5.)) / (f32::from(bounds.size.width) - 10.).max(1.))
        .clamp(0., 1.)
        * 100.)
        .round() as i32
}
pub fn drag(view: &mut AppView, event: &MouseMoveEvent, cx: &mut Context<AppView>) -> bool {
    let Some(drag) = &view.mixer_drag else {
        return false;
    };
    if !event.dragging() {
        view.mixer_drag = None;
        return false;
    }
    let target = drag.target;
    let value = position(drag.bounds, event.position.x);
    set(view, target, value, cx);
    true
}
pub fn end(view: &mut AppView, event: &MouseUpEvent, cx: &mut Context<AppView>) {
    if let Some(drag) = view.mixer_drag.take() {
        set(
            view,
            drag.target,
            position(drag.bounds, event.position.x),
            cx,
        );
    }
}

fn pan_text(pan: i8) -> String {
    match pan.cmp(&0) {
        std::cmp::Ordering::Less => format!("{} % izq.", pan.unsigned_abs()),
        std::cmp::Ordering::Greater => format!("{pan} % der."),
        _ => "Centro".into(),
    }
}

/// The bar a slider is dragged on, `height` tall with its track centred.
fn rail(
    id: String,
    target: Target,
    fraction: f32,
    height: f32,
    cx: &Context<AppView>,
) -> Stateful<Div> {
    let hit = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let measured = hit.clone();
    let middle = height / 2.;
    div()
        .id(SharedString::from(id))
        .flex_1()
        .min_w(px(40.))
        .h(px(height))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |v, event: &MouseDownEvent, window, cx| {
                if let Some(bounds) = *hit.borrow() {
                    v.selection_drag = None;
                    v.mixer_drag = Some(MixerDrag { target, bounds });
                    v.focus.focus(window);
                    set(v, target, position(bounds, event.position.x), cx);
                    cx.stop_propagation();
                }
            }),
        )
        .child(
            canvas(
                move |bounds, _, _| {
                    *measured.borrow_mut() = Some(bounds);
                },
                move |bounds, _, window, _| {
                    let width = f32::from(bounds.size.width);
                    let start = bounds.origin + point(px(5.), px(middle - 2.));
                    window.paint_quad(quad(
                        Bounds::new(start, size(px(width - 10.), px(4.))),
                        px(2.),
                        rgb(BORDER),
                        px(0.),
                        rgb(BORDER),
                        Default::default(),
                    ));
                    let progress = (width - 10.) * fraction;
                    window.paint_quad(quad(
                        Bounds::new(start, size(px(progress), px(4.))),
                        px(2.),
                        rgb(BLUE),
                        px(0.),
                        rgb(BLUE),
                        Default::default(),
                    ));
                    window.paint_quad(quad(
                        Bounds::new(
                            bounds.origin + point(px(progress), px(middle - 5.)),
                            size(px(10.), px(10.)),
                        ),
                        px(5.),
                        rgb(TEXT),
                        px(0.),
                        rgb(TEXT),
                        Default::default(),
                    ));
                },
            )
            .size_full(),
        )
}

fn slider(
    id: String,
    label: &'static str,
    target: Target,
    view: &AppView,
    cx: &Context<AppView>,
) -> Div {
    let current = value(view, target);
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .child(label)
                .child(div().text_color(rgb(TEXT)).child(format!("{current} %"))),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    button(
                        view,
                        format!("{id}-down"),
                        "−",
                        false,
                        cx,
                        move |v, _, cx| {
                            set(v, target, value(v, target) - 5, cx);
                        },
                    )
                    .w(px(25.))
                    .h(px(25.)),
                )
                .child(rail(id.clone(), target, current as f32 / 100., 26., cx))
                .child(
                    button(view, format!("{id}-up"), "+", false, cx, move |v, _, cx| {
                        set(v, target, value(v, target) + 5, cx);
                    })
                    .w(px(25.))
                    .h(px(25.)),
                ),
        )
}

/// Puts a part's volume or position back where the score seats it.
fn reset(view: &mut AppView, target: Target, cx: &mut Context<AppView>) {
    if view.busy || view.confirming {
        return;
    }
    let score = view.document.editor.score();
    let part = match target {
        Target::Volume(part) => {
            let seated = ChannelMix::seated(score, part).volume;
            view.playback.part_mix_mut(score, part).volume = seated;
            part
        }
        Target::Pan(part) => {
            let seated = ChannelMix::seated(score, part).pan;
            view.playback.part_mix_mut(score, part).pan = seated;
            part
        }
        _ => return,
    };
    view.playback.settle_part_mix(score, part);
    view.playback.apply_mixer(
        view.document.editor.score(),
        view.document.editor.cursor().part,
    );
    cx.notify();
}

/// Keeps a press on a control inside a Mezcla row from also selecting the row.
fn stop_press(_: &MouseDownEvent, _: &mut Window, cx: &mut App) {
    cx.stop_propagation();
}

/// A compact slider for a Mezcla row: the bar, named on hover, then its
/// value, which puts the default back when clicked.
fn mix_slider(
    id: String,
    name: String,
    target: Target,
    fraction: f32,
    shown: String,
    cx: &Context<AppView>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(rail(id.clone(), target, fraction, 16., cx).tooltip(hover_hint(name, None)))
        .child(
            div().on_mouse_down(MouseButton::Left, stop_press).child(
                toolbar::control(format!("{id}-reset"), shown, false, cx, move |v, _, cx| {
                    reset(v, target, cx)
                })
                .w(px(64.))
                .h(px(20.))
                .justify_end()
                .px_1()
                .text_size(px(11.))
                .text_color(rgb(MUTED))
                .tooltip(described_hint(
                    "Restablecer",
                    None,
                    "Ajuste de esta sesión: no modifica la partitura.",
                )),
            ),
        )
}

/// Volume, stereo position and, where a pack may have both, the players of
/// one part, for its row in Mezcla.
pub fn part_controls(
    view: &AppView,
    part: PartId,
    name: &str,
    family: InstrumentFamily,
    cx: &Context<AppView>,
) -> Div {
    let mix = view.playback.part_mix(view.document.editor.score(), part);
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(mix_slider(
            format!("part-volume-{}", part.0),
            format!("Volumen de {name}"),
            Target::Volume(part),
            f32::from(mix.volume) / 100.,
            format!("{} %", mix.volume),
            cx,
        ))
        .child(mix_slider(
            format!("part-pan-{}", part.0),
            format!("Posición estéreo de {name}"),
            Target::Pan(part),
            (f32::from(mix.pan) + 100.) / 200.,
            pan_text(mix.pan),
            cx,
        ))
        .when(
            matches!(
                family,
                InstrumentFamily::Strings | InstrumentFamily::Woodwinds | InstrumentFamily::Brass
            ),
            |controls| controls.child(players(view, part, cx)),
        )
}

/// Solo or section recordings, for instruments a pack may have both of.
/// Choosing the score's own default stores no choice in the document.
fn players(view: &AppView, part: PartId, cx: &Context<AppView>) -> Div {
    let current = view.document.editor.score().players(part);
    div()
        .w(px(150.))
        .on_mouse_down(MouseButton::Left, stop_press)
        .child(toolbar::track(
        [Players::Section, Players::Solo].map(|players| {
            toolbar::control(
                format!("players-{}-{}", part.0, players.label()),
                players.label(),
                current == players,
                cx,
                move |v, _, cx| {
                    let automatic = v.document.editor.score().default_players(part);
                    let choice = (players != automatic).then_some(players);
                    v.edit(Command::SetPlayers(part, choice), cx);
                },
            )
            .h(px(20.))
            .text_size(px(11.))
            .tooltip(described_hint(
                players.label(),
                None,
                "Grabaciones de un intérprete o de la sección. Se guarda con la partitura y se puede deshacer.",
            ))
        }),
    ))
}

pub fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    section("Salida")
        .child(slider(
            "master-volume".into(),
            "Volumen general",
            Target::Master,
            view,
            cx,
        ))
        .child(
            div()
                .mt_2()
                .pt_3()
                .border_t_1()
                .border_color(rgb(BORDER))
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(group_title("Reverberación suave", None))
                        .child(
                            button(
                                view,
                                "reverb-enabled",
                                if view.playback.reverb.enabled {
                                    "Activa"
                                } else {
                                    "Apagada"
                                },
                                view.playback.reverb.enabled,
                                cx,
                                |v, _, cx| {
                                    v.playback.reverb.enabled = !v.playback.reverb.enabled;
                                    v.playback.apply_mixer(
                                        v.document.editor.score(),
                                        v.document.editor.cursor().part,
                                    );
                                    cx.notify();
                                },
                            )
                            .text_size(px(10.)),
                        ),
                )
                .child(slider(
                    "reverb-amount".into(),
                    "Cantidad",
                    Target::ReverbAmount,
                    view,
                    cx,
                ))
                .child(slider(
                    "reverb-room".into(),
                    "Tamaño de sala",
                    Target::Room,
                    view,
                    cx,
                )),
        )
}
