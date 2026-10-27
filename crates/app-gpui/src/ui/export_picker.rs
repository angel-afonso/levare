use super::*;

pub struct ExportPicker {
    pub selection: score_export::Selection,
    pub error: Option<String>,
}

pub fn dialog(view: &AppView, cx: &Context<AppView>) -> impl IntoElement {
    let picker = view.export_picker.as_ref().expect("export dialog open");
    let selection = &picker.selection;
    let score = view.document.editor.score();
    let count = selection.file_count();
    div().id("export-dialog").absolute().top_0().left_0().size_full()
        .flex().items_center().justify_center().bg(rgba(SCRIM)).occlude()
        .child(div().w(px(580.)).max_w(relative(0.94)).max_h(relative(0.92))
            .flex().flex_col().bg(rgb(PANEL)).border_1().border_color(rgb(POPOVER_BORDER))
            .rounded_lg().shadow_lg().overflow_hidden()
            .child(div().p_4().flex().flex_col().gap_2().border_b_1().border_color(rgb(BORDER))
                .child(div().text_size(px(20.)).font_weight(FontWeight::SEMIBOLD).child("Exportar partitura y partes"))
                .child(div().text_size(px(12.)).text_color(rgb(MUTED)).child("Elige los documentos y formatos que quieres entregar.")))
            .child(div().p_4().flex().flex_col().gap_2()
                .child(caption("Formatos"))
                .child(div().flex().gap_2()
                    .child(button(view, "export-pdf", check(selection.pdf, "PDF"), selection.pdf, cx, |v, _, cx| {
                        if let Some(p) = &mut v.export_picker { p.selection.pdf = !p.selection.pdf; p.error = None; } cx.notify();
                    }).px_3())
                    .child(button(view, "export-musicxml", check(selection.musicxml, "MusicXML"), selection.musicxml, cx, |v, _, cx| {
                        if let Some(p) = &mut v.export_picker { p.selection.musicxml = !p.selection.musicxml; p.error = None; } cx.notify();
                    }).px_3()))
                .child(div().mt_3().flex().items_center().justify_between()
                    .child(caption("Documentos"))
                    .child(div().flex().gap_2()
                        .child(button(view, "export-all", "Todo", false, cx, |v, _, cx| {
                            let parts = v.document.editor.score().parts().iter().map(|p| p.id()).collect();
                            if let Some(p) = &mut v.export_picker { p.selection.full_score = true; p.selection.parts = parts; p.error = None; } cx.notify();
                        }))
                        .child(button(view, "export-none", "Ninguno", false, cx, |v, _, cx| {
                            if let Some(p) = &mut v.export_picker { p.selection.full_score = false; p.selection.parts.clear(); p.error = None; } cx.notify();
                        }))))
                .child(button(view, "export-full-score", check(selection.full_score, "Partitura general"), selection.full_score, cx, |v, _, cx| {
                    if let Some(p) = &mut v.export_picker { p.selection.full_score = !p.selection.full_score; p.error = None; } cx.notify();
                }).justify_start().px_3().w_full()))
            .child(div().id("export-parts-list").min_h_0().max_h(px(330.)).overflow_y_scroll().px_4().pb_3().flex().flex_col().gap_2()
                .children(score.parts().iter().map(|part| {
                    let id = part.id();
                    let selected = selection.parts.contains(&id);
                    button(view, format!("export-part-{}", id.0), check(selected, &score.part_name(id)), selected, cx, move |v, _, cx| {
                        if let Some(p) = &mut v.export_picker {
                            if p.selection.parts.contains(&id) { p.selection.parts.retain(|p| *p != id); }
                            else { p.selection.parts.push(id); }
                            p.error = None;
                        } cx.notify();
                    }).justify_start().px_3().w_full()
                })))
            .child(div().p_4().flex().flex_col().gap_3().border_t_1().border_color(rgb(BORDER))
                .child(div().text_size(px(12.)).child(format!("{count} archivos · Un archivo por documento y formato")))
                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child("Se creará una carpeta nueva en el destino que elijas. Las partes conservan sus voces, pentagramas y divisis. Alturas de concierto."))
                .when_some(picker.error.clone(), |footer, error| footer.child(div().text_size(px(12.)).text_color(rgb(ERROR)).child(error)))
                .child(div().flex().justify_end().gap_2()
                    .child(button(view, "export-cancel", "Cancelar", false, cx, |v, _, cx| v.close_export_picker(cx)).px_3())
                    .child(button(view, "export-confirm", "Elegir destino…", true, cx, |v, _, cx| v.confirm_export(cx)).px_3()
                        .when(count == 0, |b| b.opacity(0.4).cursor_default())))))
}

fn check(selected: bool, label: &str) -> String {
    format!("{}  {label}", if selected { "☑" } else { "☐" })
}
