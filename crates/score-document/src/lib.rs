//! Open-document state and the boundary between editing and engraving.
use score_core::{Score, ScoreError};
use score_edit::{Command, Editor};
use score_layout::{Layout, LayoutEngine};

pub struct Document {
    pub editor: Editor,
    layout: LayoutEngine,
    saved: Option<Score>,
}

impl Default for Document {
    fn default() -> Self {
        Self::new(Score::default()).expect("valid default score")
    }
}
impl Document {
    pub fn new(mut score: Score) -> Result<Self, ScoreError> {
        score.validate()?;
        score.arrange_parts();
        Ok(Self {
            editor: Editor::new(score.clone())?,
            layout: LayoutEngine::default(),
            saved: Some(score),
        })
    }
    pub fn execute(&mut self, command: Command) -> Result<bool, ScoreError> {
        self.editor.execute(command)
    }
    pub fn is_dirty(&self) -> bool {
        self.saved.as_ref() != Some(self.editor.score())
    }
    pub fn mark_saved(&mut self) {
        self.saved = Some(self.editor.score().clone());
    }
    pub fn recover(score: Score) -> Result<Self, ScoreError> {
        let mut document = Self::new(score)?;
        document.saved = None;
        Ok(document)
    }
    pub fn layout_compact(&mut self, width: f32) -> Layout {
        self.layout.layout_compact(self.editor.score(), width)
    }
    pub fn layout(&mut self, width: f32) -> Layout {
        self.layout.layout(self.editor.score(), width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::Step;
    #[test]
    fn dirty_state_tracks_undo_and_saved_round_trip() {
        let mut doc = Document::default();
        assert!(!doc.is_dirty());
        doc.execute(Command::Insert(doc.editor.note(Step::G)))
            .unwrap();
        assert!(doc.is_dirty());
        let bytes = score_io::encode(doc.editor.score()).unwrap();
        let loaded = score_io::decode(&bytes).unwrap();
        assert_eq!(&loaded, doc.editor.score());
        doc.mark_saved();
        assert!(!doc.is_dirty());
        doc.editor.undo();
        assert!(doc.is_dirty());
        doc.editor.redo();
        assert!(!doc.is_dirty());
        assert_eq!(doc.layout(900.).measures.len(), 8); // Four bars on each piano hand.
    }

    #[test]
    fn opening_an_unsorted_score_normalizes_parts_without_marking_it_dirty() {
        let mut score = Score::default();
        let piano = score.parts()[0].id();
        let flute = score.add_part(score_core::Instrument::Flute).unwrap();
        let document = Document::new(score).unwrap();
        assert_eq!(
            document
                .editor
                .score()
                .parts()
                .iter()
                .map(|p| p.id())
                .collect::<Vec<_>>(),
            [flute, piano]
        );
        assert!(!document.is_dirty());
        assert_eq!(
            score_io::decode(&score_io::encode(document.editor.score()).unwrap()).unwrap(),
            *document.editor.score()
        );
    }
    #[test]
    fn recovered_document_requires_explicit_save() {
        let mut document = Document::recover(Score::default()).unwrap();
        assert!(document.is_dirty());
        document.mark_saved();
        assert!(!document.is_dirty());
    }
}
