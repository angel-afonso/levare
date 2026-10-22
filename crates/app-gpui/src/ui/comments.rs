use super::*;
use crate::comments::CommentFilter;
use score_core::{CommentId, ElementId, Score, ScoreComment, ScoreError};
use text_input::TextInput;

pub struct CommentEditor {
    pub id: Option<CommentId>,
    pub anchors: Vec<ElementId>,
    pub source: Score,
    field: Entity<TextInput>,
}
impl CommentEditor {
    pub fn new(
        score: &Score,
        id: Option<CommentId>,
        selection: Vec<ElementId>,
        cx: &mut Context<AppView>,
    ) -> Result<Self, ScoreError> {
        let (anchors, text) = if let Some(id) = id {
            let comment = score
                .comment(id)
                .ok_or(ScoreError("El comentario ya no existe"))?;
            (comment.anchors.to_vec(), comment.text.to_string())
        } else {
            if selection.is_empty() {
                return Err(ScoreError("Selecciona una nota o un pasaje para comentar"));
            }
            (selection, String::new())
        };
        let field =
            cx.new(|cx| TextInput::new(&text, cx).with_max_chars(score_core::MAX_COMMENT_CHARS));
        cx.observe(&field, |_, _, cx| cx.notify()).detach();
        Ok(Self {
            id,
            anchors,
            source: score.clone(),
            field,
        })
    }
    pub fn text(&self, cx: &App) -> String {
        self.field.read(cx).value().to_owned()
    }
    pub fn focused(&self, window: &Window, cx: &App) -> bool {
        self.field.focus_handle(cx).is_focused(window)
    }
    pub fn focus(&self, window: &mut Window, cx: &App) {
        self.field.focus_handle(cx).focus(window);
    }
}

fn anchor_label(score: &Score, part_name: &str, anchors: &[ElementId]) -> String {
    let first = anchors
        .iter()
        .filter(|id| score.event(**id).is_some())
        .min_by_key(|id| score.event_tick(**id));
    let last = anchors
        .iter()
        .filter(|id| score.event(**id).is_some())
        .max_by_key(|id| score.event_tick(**id));
    let (Some(first), Some(last)) = (first, last) else {
        return format!("{part_name} · sin vínculo musical");
    };
    if anchors.len() == 1 {
        format!("{part_name} · compás {} · nota o figura", first.measure + 1)
    } else if first.measure == last.measure {
        format!(
            "{part_name} · compás {} · {} elementos",
            first.measure + 1,
            anchors.len()
        )
    } else {
        format!(
            "{part_name} · compases {}–{} · {} elementos",
            first.measure + 1,
            last.measure + 1,
            anchors.len()
        )
    }
}

fn comment_label(score: &Score, comment: &ScoreComment) -> String {
    let name = if score.part(comment.part).is_some() {
        score.part_name(comment.part)
    } else {
        comment.part_name.to_string()
    };
    anchor_label(score, &name, &comment.anchors)
}

fn draft_form(view: &AppView, cx: &Context<AppView>) -> Div {
    let draft = view.comment_editor.as_ref().unwrap();
    let score = view.document.editor.score();
    let text = draft.text(cx);
    let name = draft
        .anchors
        .first()
        .map(|id| draft.source.part_name(id.part))
        .or_else(|| {
            draft
                .id
                .and_then(|id| draft.source.comment(id))
                .map(|comment| comment.part_name.to_string())
        })
        .unwrap_or_default();
    div().flex().flex_col().gap_2()
        .child(caption(if draft.id.is_some() { "EDITAR COMENTARIO" } else { "NUEVO COMENTARIO" }))
        .child(review::wrapped_text(view, anchor_label(&draft.source, &name, &draft.anchors)))
        .child(draft.field.clone())
        .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(format!("{} / {} caracteres · Enter guarda; Esc cancela", text.trim().chars().count(), score_core::MAX_COMMENT_CHARS)))
        .when(!text.is_empty(), |body| body.child(div().id("comment-draft-preview").max_h(px(128.)).overflow_y_scroll().text_size(px(12.)).child(review::wrapped_text(view, &text))))
        .when(draft.source != *score, |body| body.child(div().text_size(px(11.)).text_color(rgb(WARNING)).child("La partitura cambió. Actualiza el vínculo antes de guardar; el borrador conserva tu texto.")))
        .child(div().flex().gap_2()
            .child(button(view, "save-comment", "Guardar", true, cx, |v, _, cx| v.apply_comment(cx)).flex_1())
            .child(button(view, "cancel-comment", "Cancelar", false, cx, |v, _, cx| { v.comment_editor = None; cx.notify(); }).flex_1()))
        .child(button(view, "comment-use-selection", "Usar selección actual", false, cx, |v, _, cx| v.update_comment_draft_target(true, cx)).w_full())
        .when(draft.id.is_some_and(|id| score.comment(id).is_some()) && draft.source != *score, |body| body.child(button(view, "comment-update-link", "Actualizar vínculo existente", false, cx, |v, _, cx| v.update_comment_draft_target(false, cx)).w_full()))
}

pub(super) fn panel(view: &AppView, cx: &Context<AppView>) -> Div {
    let score = view.document.editor.score();
    let state = &view.comments;
    let ids = state.filtered(score);
    let pages = ids.len().div_ceil(crate::review::PAGE_SIZE).max(1);
    let page = state.page.min(pages - 1);
    let pending = score
        .comments()
        .iter()
        .filter(|comment| !comment.resolved)
        .count();
    section("COMENTARIOS")
        .child(div().text_size(px(12.)).child(format!("{pending} pendientes · {} resueltos", score.comments().len() - pending)))
        .child(div().flex().flex_wrap().gap_1().children(CommentFilter::ALL.into_iter().map(|filter| button(view, format!("comment-filter-{filter:?}"), filter.label(), state.filter == filter, cx, move |v, _, cx| { v.comments.filter = filter; v.comments.page = 0; v.comments.refresh(v.document.editor.score()); cx.notify(); }))))
        .child(command_button("comments-new", "Comentar selección…", false, view, cx, id::NEW_COMMENT).w_full())
        .when(view.comment_editor.is_some(), |body| body.child(draft_form(view, cx)))
        .when(ids.is_empty(), |body| body.child(caption("No hay comentarios en este filtro.")))
        .children(ids.iter().skip(page * crate::review::PAGE_SIZE).take(crate::review::PAGE_SIZE).map(|&id| {
            let comment = score.comment(id).unwrap();
            let resolved = comment.resolved;
            div().flex().flex_col().gap_1().pb_2().border_b_1().border_color(rgb(BORDER))
                .child(action(format!("comment-{id:?}"), div().w_full().min_w_0().flex().flex_col().gap_1()
                    .child(div().text_size(px(10.)).text_color(rgb(if resolved { SUCCESS } else { WARNING })).child(if resolved { "Resuelto" } else { "Pendiente" }))
                    .child(div().text_size(px(11.)).text_color(rgb(BLUE)).child(review::wrapped_text(view, comment_label(score, comment))))
                    .child(div().text_size(px(12.)).child(review::wrapped_text(view, &comment.text))), state.active == Some(id), cx, move |v, _, cx| v.goto_comment(id, cx)).w_full().justify_start().px_2().py_2())
                .child(div().flex().flex_wrap().gap_1()
                    .child(button(view, format!("comment-resolve-{id:?}"), if resolved { "Reabrir" } else { "Resolver" }, false, cx, move |v, _, cx| v.set_comment_status(id, !resolved, cx)).text_size(px(10.)))
                    .child(button(view, format!("comment-edit-{id:?}"), "Editar", false, cx, move |v, window, cx| v.open_comment_editor(Some(id), window, cx)).text_size(px(10.)))
                    .child(button(view, format!("comment-delete-{id:?}"), "Eliminar", false, cx, move |v, _, cx| v.edit(Command::RemoveComment(id), cx)).text_size(px(10.))))
        }))
        .when(pages > 1, |body| body.child(div().flex().flex_col().gap_1()
            .child(caption(format!("Página {} de {pages}", page + 1)))
            .child(div().flex().gap_2()
                .child(button(view, "comment-prev-page", "‹ Lista", false, cx, move |v, _, cx| { v.comments.page = page.saturating_sub(1); cx.notify(); }).flex_1())
                .child(button(view, "comment-next-page", "Lista ›", false, cx, move |v, _, cx| { v.comments.page = (page + 1).min(pages - 1); cx.notify(); }).flex_1()))))
        .child(caption("Pulsa una observación para seleccionar su música. Los comentarios se guardan y admiten Deshacer."))
        .child(button(view, "comments-hide", "Ocultar lista", false, cx, |v, _, cx| { v.comments.open = false; v.comment_editor = None; cx.notify(); }).w_full())
}
