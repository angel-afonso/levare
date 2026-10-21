use super::*;
use score_core::{BarForm, Ending, Pitches, Step};
#[test]
fn toggle_repeat_end_twice_restores_the_bar_and_each_is_one_undo_step() {
    let mut e = Editor::default();
    let original = e.score().clone();
    e.execute(Command::EditForm(FormEdit::ToggleRepeatEnd))
        .unwrap();
    assert_eq!(e.score().bar_form(0).repeat_end, Some(2));
    e.execute(Command::EditForm(FormEdit::ToggleRepeatEnd))
        .unwrap();
    assert_eq!(e.score(), &original);
    assert!(e.undo());
    assert_eq!(e.score().bar_form(0).repeat_end, Some(2));
    assert!(e.undo());
    assert_eq!(e.score(), &original);
    assert!(e.redo());
}
#[test]
fn ending_covers_the_selected_bars() {
    let mut s = Score::default();
    let note = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    };
    let a = s.insert(2, 0, note).unwrap();
    let b = s.insert(3, 0, note).unwrap();
    let mut e = Editor::new(s).unwrap();
    e.select(a);
    e.extend_selection(b);
    e.execute(Command::SetEnding(vec![1])).unwrap();
    assert_eq!(
        e.score().endings().next(),
        Some(&Ending {
            start: 2,
            end: 3,
            numbers: vec![1]
        })
    );
    assert!(e.undo());
    assert_eq!(e.score().endings().count(), 0);
}
#[test]
fn clear_removes_marks_and_endings_in_range() {
    let mut s = Score::default();
    s.set_bar_form(
        0,
        BarForm {
            segno: true,
            ..Default::default()
        },
    )
    .unwrap();
    s.set_ending(Ending {
        start: 0,
        end: 1,
        numbers: vec![1],
    })
    .unwrap();
    let mut e = Editor::new(s.clone()).unwrap();
    e.execute(Command::EditForm(FormEdit::Clear)).unwrap();
    assert!(e.score().bar_form(0).is_plain());
    assert_eq!(e.score().endings().count(), 0);
    assert!(e.undo());
    assert_eq!(e.score(), &s);
}
#[test]
fn invalid_ending_numbers_are_rejected_without_changing_history() {
    let mut e = Editor::default();
    let s = e.score().clone();
    assert!(e.execute(Command::SetEnding(vec![])).is_err());
    assert_eq!(e.score(), &s);
    assert!(!e.can_undo());
}

#[test]
fn successive_form_edits_preserve_both_selection_directions_and_each_undo_step() {
    for reverse in [false, true] {
        let mut score = Score::default();
        let note = Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Quarter,
            notation: Default::default(),
        };
        let ids: Vec<_> = (1..=3)
            .map(|bar| score.insert(bar, 0, note).unwrap())
            .collect();
        let mut editor = Editor::new(score).unwrap();
        let (anchor, focus) = if reverse {
            (ids[2], ids[0])
        } else {
            (ids[0], ids[2])
        };
        editor.select(anchor);
        editor.extend_selection(focus);
        let mut snapshots = Vec::new();
        for command in [
            Command::EditForm(FormEdit::ToggleRepeatStart),
            Command::EditForm(FormEdit::ToggleRepeatEnd),
            Command::SetEnding(vec![1]),
            Command::RemoveEndings,
            Command::SetEnding(vec![2]),
            Command::EditForm(FormEdit::Clear),
        ] {
            snapshots.push(editor.score().clone());
            assert!(editor.execute(command).unwrap());
            assert_eq!(editor.selected_ids(), ids);
            assert_eq!(editor.cursor().measure, focus.measure);
        }
        for before in snapshots.into_iter().rev() {
            assert!(editor.undo());
            assert_eq!(editor.score(), &before);
            assert_eq!(editor.selected_ids(), ids);
        }
        assert!(!editor.can_undo());
    }
}
