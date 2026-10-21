use super::*;
use score_core::{Pitches, RehearsalStyle};
#[test]
fn rehearsal_toggle_and_style_preserve_passage_and_each_undo_step() {
    for reverse in [false, true] {
        let mut score = Score::default();
        let note = Event {
            duration: Duration::Quarter,
            pitches: Pitches::single(Pitch::new(score_core::Step::C, 4)),
            notation: Default::default(),
        };
        let a = score.insert(1, 0, note).unwrap();
        let b = score.insert(3, 0, note).unwrap();
        let mut editor = Editor::new(score.clone()).unwrap();
        editor.select(if reverse { b } else { a });
        editor.extend_selection(if reverse { a } else { b });
        let selection = editor.selected_ids();
        editor.execute(Command::ToggleRehearsalMark).unwrap();
        assert!(editor.score().has_rehearsal_mark(1));
        editor
            .execute(Command::SetRehearsalStyle(RehearsalStyle::Numbers))
            .unwrap();
        assert!(
            !editor
                .execute(Command::SetRehearsalStyle(RehearsalStyle::Numbers))
                .unwrap()
        );
        editor.execute(Command::ToggleRehearsalMark).unwrap();
        assert!(!editor.score().has_rehearsal_mark(1));
        assert_eq!(editor.selected_ids(), selection);
        assert!(editor.undo());
        assert!(editor.score().has_rehearsal_mark(1));
        assert!(editor.undo());
        assert_eq!(editor.score().rehearsal_style(), RehearsalStyle::Letters);
        assert!(editor.undo());
        assert_eq!(editor.score(), &score);
    }
}
