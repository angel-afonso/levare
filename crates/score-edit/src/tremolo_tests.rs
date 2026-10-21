use super::*;
use score_core::{MarkKind, Tremolo};
#[test]
fn passage_tremolo_replaces_toggles_and_preserves_selection_and_undo() {
    let mut editor = Editor::default();
    let note = editor.note(score_core::Step::C);
    editor.execute(Command::Insert(note)).unwrap();
    let a = editor.selection().unwrap();
    editor.execute(Command::Insert(note)).unwrap();
    let b = editor.selection().unwrap();
    editor.select(b);
    editor.extend_selection(a);
    for mark in [Tremolo::One, Tremolo::Two, Tremolo::Three] {
        editor
            .execute(Command::ToggleMark(MarkKind::Tremolo(mark)))
            .unwrap();
        assert_eq!(editor.selected_ids(), [a, b]);
        for id in [a, b] {
            assert_eq!(
                editor.score().event(id).unwrap().notation.tremolo,
                Some(mark)
            );
        }
    }
    editor
        .execute(Command::ToggleMark(MarkKind::Tremolo(Tremolo::Three)))
        .unwrap();
    assert!(editor.score().event(a).unwrap().notation.tremolo.is_none());
    assert!(editor.undo());
    assert_eq!(
        editor.score().event(a).unwrap().notation.tremolo,
        Some(Tremolo::Three)
    );
    assert_eq!(editor.selected_ids(), [a, b]);
}
#[test]
fn clipboard_split_keeps_tremolo_on_all_tied_fragments_and_rest_failure_is_atomic() {
    let mut source = Editor::default();
    source
        .execute(Command::Insert(Event {
            duration: Duration::Whole,
            notation: score_core::Notation {
                tremolo: Some(Tremolo::Two),
                ..Default::default()
            },
            ..source.note(score_core::Step::C)
        }))
        .unwrap();
    let clip = source.copy_selection().unwrap();
    let mut target = Editor::default();
    target
        .execute(Command::Insert(target.note(score_core::Step::D)))
        .unwrap();
    target.paste(&clip).unwrap();
    let marked: Vec<_> = target.score().parts()[0]
        .staff(StaffId::Upper)
        .unwrap()
        .measures()
        .iter()
        .flat_map(|bar| bar.iter())
        .filter(|(_, event)| event.notation.tremolo.is_some())
        .collect();
    assert!(marked.len() > 1);
    assert!(
        marked
            .iter()
            .all(|(_, event)| event.notation.tremolo == Some(Tremolo::Two))
    );
    let mut editor = Editor::default();
    editor.execute(Command::Insert(editor.rest())).unwrap();
    let before = editor.score().clone();
    assert!(
        editor
            .execute(Command::ToggleMark(MarkKind::Tremolo(Tremolo::One)))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
}
