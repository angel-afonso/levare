//! Más sonidos: the packs Levare installs on demand, each with its license.
use super::*;
use crate::store::{State, progress_label};
use score_sound_store::size_label;

pub fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let store = &view.store;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(caption("MÁS SONIDOS"))
        .children(store.catalog.packs.iter().map(|entry| {
            let id = entry.id.clone();
            let note = |text: String| div().text_size(px(10.)).text_color(rgb(MUTED)).child(text);
            let card = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .child(div().text_size(px(12.)).child(entry.name.clone()))
                .child(note(entry.description.clone()))
                .child(note(format!(
                    "{} de descarga · {} instalado · {}",
                    size_label(entry.download_size()),
                    size_label(entry.installed_size),
                    entry.license.name
                )));
            let row = || div().flex().flex_wrap().gap_1();
            match store.state(entry) {
                State::Available => card.child(button(
                    view,
                    format!("pack-install-{id}"),
                    "Instalar…",
                    false,
                    cx,
                    move |v, _, cx| {
                        v.store.accepting = Some(id.clone());
                        cx.notify();
                    },
                )),
                State::Accepting => {
                    let url = entry.license.url.clone();
                    let accept = id.clone();
                    card.child(div().text_size(px(11.)).child(format!(
                        "Licencia {} · {}. Al instalar aceptas sus condiciones.",
                        entry.license.name, entry.author
                    )))
                    .child(
                        row()
                            .child(button(
                                view,
                                format!("pack-license-{id}"),
                                "Ver licencia",
                                false,
                                cx,
                                move |_, _, cx| cx.open_url(&url),
                            ))
                            .child(button(
                                view,
                                format!("pack-accept-{id}"),
                                "Aceptar e instalar",
                                true,
                                cx,
                                move |v, _, cx| v.install_pack(&accept, cx),
                            ))
                            .child(button(
                                view,
                                format!("pack-decline-{id}"),
                                "Cancelar",
                                false,
                                cx,
                                |v, _, cx| {
                                    v.store.accepting = None;
                                    cx.notify();
                                },
                            )),
                    )
                }
                State::Installing(progress) => card
                    .child(div().text_size(px(11.)).child(progress_label(progress)))
                    .child(button(
                        view,
                        format!("pack-cancel-{id}"),
                        "Cancelar",
                        false,
                        cx,
                        |v, _, cx| {
                            v.cancel_install();
                            cx.notify();
                        },
                    )),
                State::Installed { outdated } => {
                    let remove = id.clone();
                    card.child(
                        row()
                            .when(!outdated, |row| {
                                row.child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(rgb(SUCCESS))
                                        .child("Instalado"),
                                )
                            })
                            .when(outdated, |row| {
                                row.child(button(
                                    view,
                                    format!("pack-update-{id}"),
                                    "Actualizar",
                                    true,
                                    cx,
                                    move |v, _, cx| v.install_pack(&id, cx),
                                ))
                            })
                            .child(button(
                                view,
                                format!("pack-remove-{remove}"),
                                "Quitar…",
                                false,
                                cx,
                                move |v, _, cx| {
                                    v.store.removing = Some(remove.clone());
                                    cx.notify();
                                },
                            )),
                    )
                }
                State::Removing => {
                    let remove = id.clone();
                    card.child(div().text_size(px(11.)).child(format!(
                        "Se borrarán {} del disco. Podrás volver a instalarlo.",
                        size_label(entry.installed_size)
                    )))
                    .child(
                        row()
                            .child(button(
                                view,
                                format!("pack-confirm-remove-{id}"),
                                "Quitar",
                                true,
                                cx,
                                move |v, _, cx| v.uninstall_pack(&remove, cx),
                            ))
                            .child(button(
                                view,
                                format!("pack-keep-{id}"),
                                "Conservar",
                                false,
                                cx,
                                |v, _, cx| {
                                    v.store.removing = None;
                                    cx.notify();
                                },
                            )),
                    )
                }
            }
        }))
        .when_some(store.error.clone(), |body, error| {
            body.child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(WARNING))
                    .child(error),
            )
        })
}
