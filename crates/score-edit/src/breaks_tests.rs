use super::*;
use score_core::{LayoutBreak, Step};

fn note() -> Event {
    Event {
        pitches: score_core::Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

#[test]
fn break_goes_after_the_last_selected_bar_and_undo_removes_it() {
    let mut score = Score::new("Saltos".into(), &[Instrument::Flute], 6).unwrap();
    let part = score.parts()[0].id();
    let first = score.insert_on(part, StaffId::Upper, 1, 0, note()).unwrap();
    let last = score.insert_on(part, StaffId::Upper, 3, 0, note()).unwrap();
    let mut editor = Editor::new(score.clone()).unwrap();
    editor.select(first);
    editor.extend_selection(last);
    let selected = editor.selected_ids();
    assert!(
        editor
            .execute(Command::ToggleLayoutBreak(LayoutBreak::System))
            .unwrap()
    );
    assert_eq!(editor.score().layout_break(3), Some(LayoutBreak::System));
    assert_eq!(editor.score().layout_breaks().count(), 1);
    assert_eq!(editor.selected_ids(), selected);
    assert!(editor.undo());
    assert_eq!(editor.score().layout_breaks().count(), 0);
    assert_eq!(editor.score(), &score);
}
