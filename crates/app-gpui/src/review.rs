//! Review reports are session state; custom instrument bounds live in the score.
use crate::{AppView, ui};
use gpui::Context;
use score_core::{BarMark, BarMarkRef, ElementId, PartId, ReviewIssue, ReviewTarget, Score};
use score_edit::{Command, Cursor};

pub const PAGE_SIZE: usize = 20;

pub struct ReviewState {
    pub show_ranges: bool,
    pub issues: Vec<ReviewIssue>,
    checked_score: Option<Score>,
    pub selection_scope: bool,
    pub stale: bool,
    pub filter: Option<PartId>,
    pub filter_open: bool,
    pub current: Option<usize>,
    pub page: usize,
    pub reveal_bar: Option<(usize, Option<BarMark>)>,
}
impl Default for ReviewState {
    fn default() -> Self {
        Self {
            show_ranges: true,
            issues: Vec::new(),
            checked_score: None,
            selection_scope: false,
            stale: false,
            filter: None,
            filter_open: false,
            current: None,
            page: 0,
            reveal_bar: None,
        }
    }
}
impl ReviewState {
    pub fn checked(&self) -> bool {
        self.checked_score.is_some()
    }

    pub fn check(&mut self, score: &Score, selection: Option<&[ElementId]>) {
        self.issues = score.review(selection);
        self.checked_score = Some(score.clone());
        self.selection_scope = selection.is_some();
        self.stale = false;
        self.current = None;
        self.page = 0;
        self.reveal_bar = None;
    }

    /// Whole-score reports update after edits. Selection reports expire because
    /// structural edits may shift the identities captured by the original scan.
    pub fn refresh(&mut self, score: &Score) {
        if self.filter.is_some_and(|id| score.part(id).is_none()) {
            self.set_filter(None);
        }
        if self
            .checked_score
            .as_ref()
            .is_none_or(|checked| checked == score)
        {
            return;
        }
        if self
            .checked_score
            .as_ref()
            .is_some_and(|checked| checked.same_content_except_comments(score))
        {
            self.checked_score = Some(score.clone());
            return;
        }
        if self.selection_scope {
            self.checked_score = None;
            self.issues.clear();
            self.current = None;
            self.page = 0;
            self.stale = true;
        } else {
            let current = self
                .current
                .and_then(|index| self.issues.get(index))
                .copied();
            self.check(score, None);
            self.current = current
                .and_then(|issue| self.issues.iter().position(|candidate| *candidate == issue));
            self.reveal_current();
        }
    }

    pub fn reset_document(&mut self) {
        *self = Self {
            show_ranges: self.show_ranges,
            ..Default::default()
        };
    }

    pub fn set_filter(&mut self, part: Option<PartId>) {
        self.filter = part;
        self.filter_open = false;
        self.current = None;
        self.page = 0;
    }

    pub fn filtered(&self) -> Vec<usize> {
        self.issues
            .iter()
            .enumerate()
            .filter_map(|(index, issue)| {
                let visible = match issue.target {
                    ReviewTarget::Note(id, _) => self.filter.is_none_or(|part| part == id.part),
                    ReviewTarget::Bar(_, _) => true,
                };
                visible.then_some(index)
            })
            .collect()
    }

    pub fn next(&self, forward: bool) -> Option<usize> {
        let filtered = self.filtered();
        let position = self
            .current
            .and_then(|current| filtered.iter().position(|index| *index == current));
        let next = match position {
            Some(i) if forward => (i + 1) % filtered.len(),
            Some(i) => (i + filtered.len() - 1) % filtered.len(),
            None if forward => 0,
            None => filtered.len().saturating_sub(1),
        };
        filtered.get(next).copied()
    }

    pub fn reveal_current(&mut self) {
        if let Some(position) = self
            .current
            .and_then(|current| self.filtered().iter().position(|index| *index == current))
        {
            self.page = position / PAGE_SIZE;
        }
    }
}

impl AppView {
    pub(super) fn show_review(&mut self) {
        self.ui.tab = score_toolbar::TabId::Review;
        self.ui.show_inspector = true;
        self.ui.inspector = "Revisión";
    }

    pub(super) fn check_review(&mut self, selection: bool, cx: &mut Context<Self>) {
        let ids = self.document.editor.selected_ids();
        self.show_review();
        if selection && ids.is_empty() {
            self.status = "Selecciona notas o un pasaje para comprobarlo".into();
            cx.notify();
            return;
        }
        self.review.check(
            self.document.editor.score(),
            selection.then_some(ids.as_slice()),
        );
        self.status = format!("Revisión: {} avisos", self.review.issues.len());
        cx.notify();
    }

    pub(super) fn goto_review_issue(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(issue) = self.review.issues.get(index).copied() else {
            return;
        };
        self.review.refresh(self.document.editor.score());
        let Some(index) = self
            .review
            .issues
            .iter()
            .position(|candidate| *candidate == issue)
        else {
            return;
        };
        self.playback.stop();
        self.note_drag = None;
        self.mark_drag = None;
        self.selection_drag = None;
        self.pending_slur = None;
        self.pending_hairpin = None;
        self.clicked_mark = None;
        self.clicked_bar_mark = None;
        self.ui.note_entry = false;
        self.clicked_bar_mark = select_target(&mut self.document.editor, issue.target);
        self.review.reveal_bar = match issue.target {
            ReviewTarget::Bar(bar, mark) => Some((bar, mark)),
            _ => None,
        };
        self.pending_page = None;
        self.review.current = Some(index);
        self.review.reveal_current();
        self.show_review();
        self.reveal_selection = true;
        self.status = issue.kind.message();
        cx.notify();
    }

    pub(super) fn navigate_review(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.review.refresh(self.document.editor.score());
        self.show_review();
        if let Some(index) = self.review.next(forward) {
            self.goto_review_issue(index, cx);
        }
        cx.notify();
    }

    pub(super) fn open_range_editor(&mut self, cx: &mut Context<Self>) {
        self.show_review();
        let part = self
            .document
            .editor
            .score()
            .part(self.document.editor.cursor().part)
            .unwrap();
        if let Some(range) = part.sounding_range() {
            self.range_editor = Some(ui::RangeEditor::new(part.id(), range, cx));
        } else {
            self.status = "La percusión sin altura no necesita límites de registro".into();
        }
        cx.notify();
    }

    pub(super) fn apply_range(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.range_editor else {
            return;
        };
        let part = editor.part;
        match editor.range(cx) {
            Ok(range) => {
                if self.try_edit(Command::SetInstrumentRange(part, Some(range)), cx) {
                    self.range_editor = None;
                }
            }
            Err(error) => {
                self.status = error.to_string();
            }
        }
        cx.notify();
    }
}

fn select_target(editor: &mut score_edit::Editor, target: ReviewTarget) -> Option<BarMarkRef> {
    match target {
        ReviewTarget::Note(id, pitch) => {
            editor.select_pitch(id, Some(pitch));
            None
        }
        ReviewTarget::Bar(measure, mark) => {
            editor.set_cursor(Cursor {
                measure,
                index: 0,
                ..editor.cursor()
            });
            mark.map(|mark| BarMarkRef { measure, mark })
        }
    }
}

#[cfg(test)]
mod tests;
