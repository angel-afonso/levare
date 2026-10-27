//! Writing words at the selection: expression and technique texts, lyrics
//! and chord symbols.
use super::*;
use score_core::{ChordSymbol, Join, Score, TextKind};

impl AppView {
    /// Open the words of `kind` at the first selected note; the same command
    /// again closes them.
    pub(super) fn open_text_editor(
        &mut self,
        kind: TextKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self
            .text_editor
            .as_ref()
            .is_some_and(|editor| editor.kind == kind)
        {
            self.text_editor = None;
            cx.notify();
            return;
        }
        let editor = &self.document.editor;
        let Some(&target) = editor.selected_ids().first() else {
            self.status = format!("{}: selecciona primero una nota o silencio", kind.label());
            cx.notify();
            return;
        };
        let text = ui::StaffTextEditor::new(editor.score(), kind, target, cx);
        text.focus(window, cx);
        self.text_editor = Some(text);
        self.close_other_word_editors(Words::Text);
        cx.notify();
    }

    /// Write the field's words; an empty field removes them. Words that do
    /// not fit keep the editor open with the reason.
    pub(super) fn apply_text(&mut self, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(editor) = &self.text_editor else {
            return;
        };
        let (kind, target) = (editor.kind, editor.target);
        let text = editor.text(cx).trim().to_owned();
        let text = (!text.is_empty()).then_some(text);
        if let Some(Err(error)) = text.as_deref().map(score_core::Words::new) {
            self.status = error.to_string();
            cx.notify();
            return;
        }
        let changed = self.try_edit(Command::SetText(target, kind, text.clone()), cx);
        self.text_editor = None;
        if changed {
            self.status = match text {
                Some(text) => format!(
                    "{} «{text}» en el compás {}",
                    kind.label(),
                    target.measure + 1
                ),
                None => format!("{} quitado", kind.label()),
            };
        }
        cx.notify();
    }
}

impl AppView {
    /// Sing at the selected note, from the verse of a clicked syllable.
    pub(super) fn open_lyric_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.lyric_editor.take().is_some() {
            cx.notify();
            return;
        }
        let verse = match self.selected_mark() {
            Some(MarkRef::Lyric(_, verse)) => verse,
            _ => 0,
        };
        let editor = &self.document.editor;
        let Some(target) = editor
            .selected_ids()
            .first()
            .copied()
            .and_then(|id| sung_from(editor.score(), id))
        else {
            self.status = "Letra: selecciona primero una nota".into();
            cx.notify();
            return;
        };
        self.document.editor.select(target);
        let lyrics = ui::LyricEditor::new(self.document.editor.score(), target, verse, cx);
        lyrics.focus(window, cx);
        self.lyric_editor = Some(lyrics);
        self.close_other_word_editors(Words::Lyrics);
        cx.notify();
    }

    pub(super) fn choose_verse(&mut self, verse: u8, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.lyric_editor {
            let target = editor.target;
            editor.retarget(self.document.editor.score(), target, verse, cx);
        }
        cx.notify();
    }

    /// Write the field as the note's syllable joined by `join`; with
    /// `advance`, go on to the next note that can take one.
    pub(super) fn apply_syllable(&mut self, join: Join, advance: bool, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(editor) = &self.lyric_editor else {
            return;
        };
        let (target, verse, text) = (editor.target, editor.verse, editor.text(cx));
        // An empty field only moves on; Enter on it clears the syllable.
        if !text.is_empty() || !advance {
            let syllable = (!text.is_empty()).then(|| (text.clone(), join));
            if let Some((text, join)) = &syllable
                && let Err(error) = score_core::Syllable::new(text, *join)
            {
                self.status = error.to_string();
                cx.notify();
                return;
            }
            self.try_edit(Command::SetSyllable(target, verse, syllable), cx);
        }
        if !advance {
            self.lyric_editor = None;
            cx.notify();
            return;
        }
        let score = self.document.editor.score();
        match score.next_event(target).and_then(|id| sung_from(score, id)) {
            Some(next) => {
                self.document.editor.select(next);
                self.reveal_selection = true;
                let score = self.document.editor.score();
                if let Some(editor) = &mut self.lyric_editor {
                    editor.retarget(score, next, verse, cx);
                }
            }
            None => {
                self.lyric_editor = None;
                self.status = "Letra escrita hasta la última nota".into();
            }
        }
        cx.notify();
    }

    pub(super) fn open_chord_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.confirming || self.agent_locked() {
            return;
        }
        if self.chord_editor.take().is_some() {
            cx.notify();
            return;
        }
        let editor = &self.document.editor;
        let Some(&target) = editor.selected_ids().first() else {
            self.status = "Cifrado: selecciona primero una nota o silencio".into();
            cx.notify();
            return;
        };
        let chords = ui::ChordEditor::new(editor.score(), target, cx);
        chords.focus(window, cx);
        self.chord_editor = Some(chords);
        self.close_other_word_editors(Words::Chords);
        cx.notify();
    }

    /// Write the typed symbol; an empty field removes it. With `advance`,
    /// stay open on the next note or rest.
    pub(super) fn apply_chord(&mut self, advance: bool, cx: &mut Context<Self>) {
        if self.agent_blocks(cx) {
            return;
        }
        let Some(editor) = &self.chord_editor else {
            return;
        };
        let (target, text) = (editor.target, editor.text(cx));
        let chord = if text.is_empty() {
            None
        } else {
            match ChordSymbol::parse(&text) {
                Ok(chord) => Some(chord),
                Err(error) => {
                    self.status = error.to_string();
                    cx.notify();
                    return;
                }
            }
        };
        if self.try_edit(Command::SetChord(target, chord), cx)
            && let Some(chord) = chord
        {
            self.status = format!(
                "Cifrado {} en el compás {}",
                chord.label(),
                target.measure + 1
            );
        }
        let next = advance
            .then(|| self.document.editor.score().next_event(target))
            .flatten();
        match next {
            Some(next) => {
                self.document.editor.select(next);
                self.reveal_selection = true;
                let score = self.document.editor.score();
                if let Some(editor) = &mut self.chord_editor {
                    editor.retarget(score, next, cx);
                }
            }
            None => self.chord_editor = None,
        }
        cx.notify();
    }

    fn close_other_word_editors(&mut self, keep: Words) {
        if keep != Words::Text {
            self.text_editor = None;
        }
        if keep != Words::Lyrics {
            self.lyric_editor = None;
        }
        if keep != Words::Chords {
            self.chord_editor = None;
        }
        self.tempo_picker = None;
        self.header_editor = None;
        self.key_picker = false;
        self.time_picker = false;
        self.clef_picker = false;
        self.ui.show_inspector = true;
        self.ui.right = crate::panels::RightTab::Selection;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Words {
    Text,
    Lyrics,
    Chords,
}

/// The first note from `id` on, in its voice, that starts a sound: rests
/// and the far end of a tie take no syllable.
fn sung_from(score: &Score, mut id: ElementId) -> Option<ElementId> {
    loop {
        let event = score.event(id)?;
        let tied = score
            .connections()
            .iter()
            .any(|c| c.kind == ConnectionKind::Tie && c.end == id);
        if !event.pitches.is_empty() && !tied {
            return Some(id);
        }
        id = score.next_event(id)?;
    }
}
