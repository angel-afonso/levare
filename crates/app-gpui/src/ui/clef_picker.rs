//! Clefs belong to the selected instrument and persist from a bar boundary.
use super::*;

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    const SCOPE: &str = "Se aplica a este instrumento hasta el siguiente cambio de clave. Las notas conservan su altura y su sonido.";
    let editor = &view.document.editor;
    let cursor = editor.cursor();
    let score = editor.score();
    let part = score.part(cursor.part).unwrap();
    let bar = view.signature_measure();
    let active = part.clef_at(cursor.staff, bar);
    let choices: &[score_core::Clef] = if active == score_core::Clef::Percussion {
        &[score_core::Clef::Percussion]
    } else {
        &score_core::Clef::PITCHED
    };
    div()
        .flex()
        .flex_col()
        .gap_2()
        .when(part.instrument().is_grand_staff(), |panel| {
            panel.child(div().text_size(px(12.)).text_color(rgb(SECONDARY)).child(
                if cursor.staff == StaffId::Upper {
                    "Pentagrama superior"
                } else {
                    "Pentagrama inferior"
                },
            ))
        })
        .child(signature_scope(view, cx))
        .children(choices.iter().copied().map(|clef| {
            toolbar::control(
                format!("choose-clef-{clef:?}"),
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .w(px(36.))
                            .h(px(40.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(clef_symbol(view, clef, (28., 36.), TEXT)),
                    )
                    .child(div().text_size(px(12.)).child(clef.label())),
                active == clef,
                cx,
                move |v, _, cx| v.choose_clef(Some(clef), cx),
            )
            .w_full()
            .justify_start()
            .px_2()
            .tooltip(described_hint(clef.label(), None, SCOPE))
        }))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .when(part.clef_change_at(cursor.staff, bar).is_some(), |row| {
                    row.child(act(
                        view,
                        "remove-clef-change",
                        "Restablecer clave anterior",
                        false,
                        cx,
                        |v, _, cx| v.choose_clef(None, cx),
                    ))
                })
                .child(close_editor("Cancelar", cx)),
        )
}
