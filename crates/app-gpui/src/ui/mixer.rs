use super::*;

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

fn slider(
    id: String,
    label: &'static str,
    target: Target,
    view: &AppView,
    cx: &Context<AppView>,
) -> Div {
    let current = value(view, target);
    let display = if let Target::Pan(part) = target {
        let pan = view
            .playback
            .part_mix(view.document.editor.score(), part)
            .pan;
        match pan.cmp(&0) {
            std::cmp::Ordering::Less => format!("{} % izq.", pan.unsigned_abs()),
            std::cmp::Ordering::Greater => format!("{pan} % der."),
            _ => "Centro".into(),
        }
    } else {
        format!("{current} %")
    };
    let hit = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let measured = hit.clone();
    let fraction = current as f32 / 100.;
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
                .child(div().text_color(rgb(TEXT)).child(display)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    button(format!("{id}-down"), "−", false, cx, move |v, _, cx| {
                        set(v, target, value(v, target) - 5, cx);
                    })
                    .w(px(25.))
                    .h(px(25.)),
                )
                .child(
                    div()
                        .id(SharedString::from(id.clone()))
                        .flex_1()
                        .min_w(px(40.))
                        .h(px(26.))
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
                                    let start = bounds.origin + point(px(5.), px(11.));
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
                                            bounds.origin + point(px(progress), px(8.)),
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
                        ),
                )
                .child(
                    button(format!("{id}-up"), "+", false, cx, move |v, _, cx| {
                        set(v, target, value(v, target) + 5, cx);
                    })
                    .w(px(25.))
                    .h(px(25.)),
                ),
        )
}

pub fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    section("SALIDA GENERAL")
        .child(slider("master-volume".into(), "Volumen general", Target::Master, view, cx))
        .child(div().mt_2().pt_3().border_t_1().border_color(rgb(BORDER)).flex().flex_col().gap_2()
            .child(div().flex().items_center().justify_between()
                .child(caption("REVERBERACIÓN SUAVE"))
                .child(button("reverb-enabled", if view.playback.reverb.enabled { "Activa" } else { "Apagada" }, view.playback.reverb.enabled, cx, |v, _, cx| {
                    v.playback.reverb.enabled = !v.playback.reverb.enabled;
                    v.playback.apply_mixer(v.document.editor.score(), v.document.editor.cursor().part);
                    cx.notify();
                }).text_size(px(10.))))
            .child(slider("reverb-amount".into(), "Cantidad", Target::ReverbAmount, view, cx))
            .child(slider("reverb-room".into(), "Tamaño de sala", Target::Room, view, cx)))
        .child(caption("INSTRUMENTOS"))
        .children(view.document.editor.score().parts().iter().map(|part| {
            let id = part.id();
            div().flex().flex_col().gap_2().pt_3().mt_1().border_t_1().border_color(rgb(BORDER))
                .child(div().flex().items_center().justify_between()
                    .child(div().text_size(px(12.)).child(instrument_name(view.document.editor.score(), id)))
                    .child(button(format!("mix-reset-{}", id.0), "Restablecer", false, cx, move |v, _, cx| {
                        v.playback.part_mix.remove(&id);
                        v.playback.apply_mixer(v.document.editor.score(), v.document.editor.cursor().part);
                        cx.notify();
                    }).text_size(px(9.))))
                .child(slider(format!("part-volume-{}", id.0), "Volumen", Target::Volume(id), view, cx))
                .child(slider(format!("part-pan-{}", id.0), "Posición estéreo", Target::Pan(id), view, cx))
        }))
        .child(div().mt_3().text_size(px(10.)).text_color(rgb(MUTED))
            .child("Ajustes de esta sesión. Puedes cambiarlos mientras escuchas; no modifican la partitura."))
}
