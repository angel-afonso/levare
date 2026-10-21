//! Comment list state stays in the session; saved observations live in Score.
use crate::{AppView, ui};
use gpui::{Context, Window};
use score_core::{CommentId, Score, ScoreComment};
use score_edit::Command;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CommentFilter {
    #[default]
    All,
    Pending,
    Resolved,
}
impl CommentFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Pending, Self::Resolved];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "Todos",
            Self::Pending => "Pendientes",
            Self::Resolved => "Resueltos",
        }
    }
    pub fn matches(self, comment: &ScoreComment) -> bool {
        match self {
            Self::All => true,
            Self::Pending => !comment.resolved,
            Self::Resolved => comment.resolved,
        }
    }
}

#[derive(Default)]
pub struct CommentState {
    pub open: bool,
    pub active: Option<CommentId>,
    pub filter: CommentFilter,
    pub page: usize,
}
impl CommentState {
    pub fn refresh(&mut self, score: &Score) {
        if self.active.is_some_and(|id| {
            score
                .comment(id)
                .is_none_or(|comment| !self.filter.matches(comment))
        }) {
            self.active = None;
        }
    }
    pub fn filtered(&self, score: &Score) -> Vec<CommentId> {
        let mut comments: Vec<_> = score
            .comments()
            .iter()
            .filter(|comment| self.filter.matches(comment))
            .collect();
        comments.sort_by_cached_key(|comment| {
            (
                comment.resolved,
                comment
                    .anchors
                    .iter()
                    .filter_map(|id| score.event_tick(*id))
                    .min()
                    .unwrap_or(u64::MAX),
                comment.id.0,
            )
        });
        comments.into_iter().map(|comment| comment.id).collect()
    }
}

impl AppView {
    pub(super) fn open_comment_editor(
        &mut self,
        id: Option<CommentId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_review();
        self.comments.open = true;
        match ui::CommentEditor::new(
            self.document.editor.score(),
            id,
            self.document.editor.selected_ids(),
            cx,
        ) {
            Ok(editor) => {
                self.range_editor = None;
                self.comment_editor = Some(editor);
                self.comments.active = id;
                self.comment_editor.as_ref().unwrap().focus(window, cx);
            }
            Err(error) => self.status = error.to_string(),
        }
        cx.notify();
    }

    pub(super) fn apply_comment(&mut self, cx: &mut Context<Self>) {
        let Some(draft) = &self.comment_editor else {
            return;
        };
        if draft.source != *self.document.editor.score() {
            self.status = "La partitura cambió. Actualiza el vínculo del borrador antes de guardarlo; el texto se conserva.".into();
            cx.notify();
            return;
        }
        let text = draft.text(cx);
        let id = draft.id;
        let anchors = draft.anchors.clone();
        if id
            .and_then(|id| self.document.editor.score().comment(id))
            .is_some_and(|comment| {
                comment.text.as_ref() == text.trim()
                    && comment.anchors.as_ref() == anchors.as_slice()
            })
        {
            self.comment_editor = None;
            cx.notify();
            return;
        }
        // Refuse before entering history if a nearly full document would no
        // longer fit its native save/recovery envelope. Keep the draft intact.
        let mut candidate = self.document.editor.score().clone();
        let checked = candidate
            .save_comment(id, &text, &anchors)
            .map_err(|error| error.to_string())
            .and_then(|_| {
                score_io::encode(&candidate)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            });
        if let Err(error) = checked {
            self.status = error;
            cx.notify();
            return;
        }
        if self.try_edit(Command::SaveComment { id, text, anchors }, cx) {
            let saved = id.or_else(|| {
                self.document
                    .editor
                    .score()
                    .comments()
                    .back()
                    .map(|comment| comment.id)
            });
            self.comments.active = saved;
            self.comments.filter = CommentFilter::All;
            self.comments.page = saved
                .and_then(|id| {
                    self.comments
                        .filtered(self.document.editor.score())
                        .iter()
                        .position(|candidate| *candidate == id)
                })
                .unwrap_or(0)
                / crate::review::PAGE_SIZE;
            self.comment_editor = None;
            self.status = "Comentario guardado. Puedes deshacer el cambio.".into();
        }
        cx.notify();
    }

    pub(super) fn update_comment_draft_target(&mut self, selection: bool, cx: &mut Context<Self>) {
        let score = self.document.editor.score();
        let ids = self.document.editor.selected_ids();
        if let Some(draft) = &mut self.comment_editor {
            let anchors = if selection {
                Some(ids).filter(|ids| !ids.is_empty())
            } else {
                draft
                    .id
                    .and_then(|id| score.comment(id))
                    .map(|comment| comment.anchors.to_vec())
            };
            if let Some(anchors) = anchors {
                if draft.id.is_some_and(|id| score.comment(id).is_none()) {
                    draft.id = None;
                }
                draft.anchors = anchors;
                draft.source = score.clone();
            } else {
                self.status = "Selecciona música para vincular la observación.".into();
            }
        }
        cx.notify();
    }

    pub(super) fn goto_comment(&mut self, id: CommentId, cx: &mut Context<Self>) {
        if self.document.editor.score().comment(id).is_none() {
            return;
        }
        self.show_review();
        self.comments.open = true;
        self.comments.active = Some(id);
        if self.document.editor.select_comment(id) {
            self.playback.stop();
            self.note_drag = None;
            self.mark_drag = None;
            self.selection_drag = None;
            self.pending_slur = None;
            self.pending_hairpin = None;
            self.clicked_mark = None;
            self.clicked_bar_mark = None;
            self.review.reveal_bar = None;
            self.pending_page = None;
            self.ui.note_entry = false;
            self.reveal_selection = true;
            self.status = "Pasaje del comentario seleccionado".into();
        } else {
            self.status = "La música vinculada fue eliminada. Puedes editar el comentario y vincularlo a otra selección.".into();
        }
        cx.notify();
    }

    pub(super) fn set_comment_status(
        &mut self,
        id: CommentId,
        resolved: bool,
        cx: &mut Context<Self>,
    ) {
        if self.try_edit(Command::SetCommentResolved(id, resolved), cx) {
            self.status = if resolved {
                "Comentario resuelto. Puedes reabrirlo o deshacer."
            } else {
                "Comentario pendiente de nuevo."
            }
            .into();
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
