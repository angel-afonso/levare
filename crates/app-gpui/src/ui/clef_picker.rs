//! Clefs belong to the selected instrument and persist from a bar boundary.
use super::*;

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
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
        .flex().flex_col().gap_2().pb_3().border_b_1().border_color(rgb(BORDER))
        .child(div().text_size(px(12.)).font_weight(FontWeight::SEMIBOLD).child(score.part_name(cursor.part)))
        .when(part.instrument().is_grand_staff(), |panel| panel.child(
            div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                if cursor.staff == StaffId::Upper { "Pentagrama superior" } else { "Pentagrama inferior" }
            )
        ))
        .child(signature_scope(view, cx))
        .children(choices.iter().copied().map(|clef| {
            action(
                format!("choose-clef-{clef:?}"),
                div().flex().items_center().gap_2()
                    .child(div().w(px(36.)).h(px(40.)).flex_shrink_0().flex().items_center().justify_center()
                        .child(clef_symbol(view, clef, (28., 36.), TEXT)))
                    .child(div().text_size(px(12.)).child(clef.label())),
                active == clef, cx, move |v, _, cx| v.choose_clef(Some(clef), cx),
            ).w_full().justify_start().px_2()
        }))
        .when(part.clef_change_at(cursor.staff, bar).is_some(), |panel| panel.child(
            button(view, "remove-clef-change", "Restablecer clave anterior", false, cx,
                |v, _, cx| v.choose_clef(None, cx)).w_full()
        ))
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(
            "Se aplica a este instrumento hasta el siguiente cambio de clave. Las notas conservan su altura y su sonido."
        ))
}
