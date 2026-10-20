//! Open-document state and the boundary between editing and engraving.
use score_core::{Score, ScoreError};
use score_edit::{Command, Editor};
use score_layout::{Layout, LayoutEngine};
use std::rc::Rc;

pub struct Document {
    pub editor: Editor,
    layout: LayoutEngine,
    /// The last displayed view. Most frames (playback, pointer, panels) leave
    /// the score untouched; comparing it is far cheaper than engraving again.
    view: Option<View>,
    saved: Option<Score>,
}

struct View {
    score: Score,
    width: f32,
    mode: score_layout::ViewMode,
    scale: f32,
    layout: Rc<Layout>,
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
            view: None,
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
    /// The displayed view at a presentation `scale`. An unchanged view is
    /// shared with the caller rather than copied, frame after frame.
    pub fn layout_view(
        &mut self,
        width: f32,
        mode: score_layout::ViewMode,
        scale: f32,
    ) -> Rc<Layout> {
        let score = self.editor.score();
        if let Some(view) = &self.view
            && view.width == width
            && view.mode == mode
            && view.scale == scale
            && view.score == *score
        {
            return view.layout.clone();
        }
        let mut layout = self.layout.layout_view(score, width, mode);
        layout.scale = scale;
        let layout = Rc::new(layout);
        self.view = Some(View {
            score: score.clone(),
            width,
            mode,
            scale,
            layout: layout.clone(),
        });
        layout
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
    fn view_changes_leave_the_saved_document_and_selection_intact() {
        let mut doc = Document::default();
        doc.execute(Command::Insert(doc.editor.note(Step::G)))
            .unwrap();
        doc.mark_saved();
        let score = doc.editor.score().clone();
        let selection = doc.editor.selection();
        let cursor = doc.editor.cursor();
        for mode in [
            score_layout::ViewMode::Page,
            score_layout::ViewMode::SingleSystem,
            score_layout::ViewMode::Continuous,
        ] {
            let layout = doc.layout_view(800., mode, 1.);
            assert_eq!(layout.mode, mode);
            assert_eq!(doc.editor.score(), &score);
            assert_eq!(doc.editor.selection(), selection);
            assert_eq!(doc.editor.cursor(), cursor);
            assert!(!doc.is_dirty());
        }
        assert!(doc.editor.undo());
    }

    #[test]
    fn repeated_views_follow_edits_undo_width_mode_and_zoom() {
        use score_layout::ViewMode;
        let mut doc = Document::default();
        let fresh = |doc: &Document, width, mode, scale| {
            let mut layout = LayoutEngine::default().layout_view(doc.editor.score(), width, mode);
            layout.scale = scale;
            format!("{layout:?}")
        };
        let check = |doc: &mut Document, width, mode, scale| {
            let first = doc.layout_view(width, mode, scale);
            assert_eq!(format!("{first:?}"), fresh(doc, width, mode, scale));
            // An unchanged view is shared, not copied, on every later frame.
            assert!(Rc::ptr_eq(&first, &doc.layout_view(width, mode, scale)));
        };
        check(&mut doc, 800., ViewMode::Continuous, 1.);
        doc.execute(Command::Insert(doc.editor.note(Step::G)))
            .unwrap();
        check(&mut doc, 800., ViewMode::Continuous, 1.);
        // Edits made through the editor directly must not leave a stale view.
        assert!(doc.editor.undo());
        check(&mut doc, 800., ViewMode::Continuous, 1.);
        assert!(doc.editor.redo());
        check(&mut doc, 800., ViewMode::Continuous, 1.);
        check(&mut doc, 800., ViewMode::Continuous, 0.7);
        check(&mut doc, 1100., ViewMode::Continuous, 0.7);
        check(&mut doc, 1100., ViewMode::Page, 0.7);
        check(&mut doc, 1100., ViewMode::SingleSystem, 0.7);
        check(&mut doc, 800., ViewMode::Continuous, 1.);
    }

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

    #[test]
    fn imported_musicxml_is_dirty_editable_and_saves_as_a_native_document() {
        let mut source = Score::new(
            "Importación editable".into(),
            &[score_core::Instrument::Violin],
            1,
        )
        .unwrap();
        source
            .insert(
                0,
                0,
                score_core::Event {
                    pitches: score_core::Pitches::single(score_core::Pitch::new(
                        score_core::Step::C,
                        4,
                    )),
                    duration: score_core::Duration::Quarter,
                    notation: Default::default(),
                },
            )
            .unwrap();
        let original = score_io::musicxml::encode(&source).unwrap();
        let opened = score_io::musicxml::decode(&original).unwrap();
        let mut document = Document::recover(opened.score).unwrap();
        assert!(document.is_dirty());
        let part = document.editor.score().parts()[0].id();
        let event = document.editor.score().parts()[0].measures()[0]
            .id_at(0)
            .unwrap();
        let id = score_core::ElementId {
            part,
            staff: score_core::StaffId::Upper,
            measure: 0,
            event,
        };
        document.editor.select(id);
        document.execute(Command::Transpose(1)).unwrap();
        assert_eq!(
            document
                .editor
                .score()
                .event(id)
                .unwrap()
                .pitches
                .first()
                .unwrap()
                .step,
            score_core::Step::D
        );
        let native = score_io::encode(document.editor.score()).unwrap();
        assert_eq!(&score_io::decode(&native).unwrap(), document.editor.score());
        assert!(document.editor.undo());
        assert!(document.is_dirty());
        document.mark_saved();
        assert!(!document.is_dirty());
    }
}
