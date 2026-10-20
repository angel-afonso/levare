use super::*;
use score_core::{Connection, ConnectionKind, Pitches, Step, Voice};

fn event(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Default::default(),
    }
}

#[test]
fn move_has_one_history_entry_and_keeps_the_selected_chord_tone() {
    let mut s = Score::default();
    let id = s.insert(0, 0, event(Step::C, Duration::Quarter)).unwrap();
    let e = Pitch::new(Step::E, 4).unwrap();
    s.add_pitch(id, e).unwrap();
    let mut editor = Editor::new(s.clone()).unwrap();
    editor.select_pitch(id, Some(e));
    assert!(
        editor
            .execute(Command::MoveSelectedEvent {
                measure: 2,
                tick: 24
            })
            .unwrap()
    );
    let moved = editor.selection().unwrap();
    assert_eq!(moved.measure, 2);
    assert_eq!(editor.score().event_tick(moved), Some(408));
    assert_eq!(editor.selected_pitch(), Some(e));
    assert_eq!(editor.cursor().voice, Voice::One);
    let final_score = editor.score().clone();
    assert!(editor.undo());
    assert_eq!(editor.score(), &s);
    assert!(!editor.can_undo());
    assert_eq!(editor.selection(), Some(id));
    assert_eq!(editor.selected_pitch(), Some(e));
    assert!(editor.redo());
    assert_eq!(editor.score(), &final_score);
    assert_eq!(editor.selection(), Some(moved));
}

#[test]
fn failed_and_no_op_moves_preserve_score_selection_and_history() {
    let mut s = Score::default();
    let a = s.insert(0, 0, event(Step::C, Duration::Quarter)).unwrap();
    s.insert(1, 0, event(Step::D, Duration::Half)).unwrap();
    let mut editor = Editor::new(s.clone()).unwrap();
    editor.select(a);
    assert!(
        !editor
            .execute(Command::MoveSelectedEvent {
                measure: 0,
                tick: 0
            })
            .unwrap()
    );
    assert!(
        editor
            .execute(Command::MoveSelectedEvent {
                measure: 1,
                tick: 0
            })
            .is_err()
    );
    assert!(
        editor
            .execute(Command::MoveSelectedEvent {
                measure: usize::MAX,
                tick: 0
            })
            .is_err()
    );
    assert_eq!(editor.score(), &s);
    assert_eq!(editor.selection(), Some(a));
    assert!(!editor.can_undo());
}

#[test]
fn undo_restores_ties_invalidated_by_the_move() {
    let mut s = Score::default();
    let a = s.insert(0, 0, event(Step::C, Duration::Quarter)).unwrap();
    let b = s.insert(0, 1, event(Step::C, Duration::Quarter)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let mut editor = Editor::new(s.clone()).unwrap();
    editor.select(a);
    editor
        .execute(Command::MoveSelectedEvent {
            measure: 1,
            tick: 0,
        })
        .unwrap();
    assert!(editor.score().connections().is_empty());
    editor.undo();
    assert_eq!(editor.score(), &s);
}
