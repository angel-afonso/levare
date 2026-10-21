use super::*;
use score_core::{
    Articulation, BarMark, BarMarkRef, Connection, ConnectionKind, Dynamic, Instrument, MarkRef,
    Notation, Offset, Pitches, PlayingTechnique, Step,
};

fn note(step: Step, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration,
        notation: Notation::default(),
    }
}

#[test]
fn setting_a_mark_offset_is_one_undoable_step() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    let id = editor.selection().unwrap();
    editor
        .execute(Command::SetDynamic(Some(Dynamic::P)))
        .unwrap();
    let before = editor.score().clone();
    let mark = MarkRef::Dynamic(id);
    let offset = Offset { dx: 20, dy: -15 };
    assert!(
        editor
            .execute(Command::SetMarkOffset(mark, offset))
            .unwrap()
    );
    assert_eq!(editor.score().mark_offset(mark), offset);
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
    assert!(editor.redo());
    assert_eq!(editor.score().mark_offset(mark), offset);
}

#[test]
fn invalid_mark_offset_keeps_score_and_history() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    let id = editor.selection().unwrap();
    editor
        .execute(Command::SetDynamic(Some(Dynamic::P)))
        .unwrap();
    let before = editor.score().clone();
    let can_undo = editor.can_undo();
    assert!(
        editor
            .execute(Command::SetMarkOffset(
                MarkRef::Dynamic(id),
                Offset { dx: 401, dy: 0 }
            ))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
    assert_eq!(editor.can_undo(), can_undo);
    assert!(editor.undo());
    assert_eq!(editor.score().event(id).unwrap().notation.dynamic, None);
}

#[test]
fn resetting_selected_marks_and_starting_spans_is_one_step() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    let a = editor.selection().unwrap();
    editor
        .execute(Command::Insert(note(Step::D, Duration::Quarter)))
        .unwrap();
    let b = editor.selection().unwrap();
    editor.select(a);
    editor
        .execute(Command::SetDynamic(Some(Dynamic::P)))
        .unwrap();
    let slur = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Slur,
    };
    editor.execute(Command::AddConnection(slur)).unwrap();
    let offset = Offset { dx: 10, dy: 5 };
    editor
        .execute(Command::SetMarkOffset(MarkRef::Dynamic(a), offset))
        .unwrap();
    editor
        .execute(Command::SetMarkOffset(MarkRef::Connection(slur), offset))
        .unwrap();
    editor.extend_selection(b);
    let before = editor.score().clone();
    assert!(editor.execute(Command::ResetMarkOffsets(None)).unwrap());
    assert_eq!(
        editor.score().mark_offset(MarkRef::Dynamic(a)),
        Offset::ZERO
    );
    assert_eq!(
        editor.score().mark_offset(MarkRef::Connection(slur)),
        Offset::ZERO
    );
    assert_eq!(editor.selected_ids(), vec![a, b]);
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
}

#[test]
fn clipboard_keeps_a_point_mark_offset() {
    let mut source = Editor::default();
    let mut event = note(Step::C, Duration::Quarter);
    event.notation.dynamic = Some(Dynamic::F);
    source.execute(Command::Insert(event)).unwrap();
    let id = source.selection().unwrap();
    let offset = Offset { dx: 13, dy: -8 };
    source
        .execute(Command::SetMarkOffset(MarkRef::Dynamic(id), offset))
        .unwrap();
    let clip = source.copy_selection().unwrap();
    let mut target = Editor::default();
    target.paste(&clip).unwrap();
    let pasted = target.selection().unwrap();
    assert_eq!(target.score().mark_offset(MarkRef::Dynamic(pasted)), offset);
    target.score().validate().unwrap();
}

#[test]
fn split_paste_keeps_offsets_only_on_fragments_with_their_marks() {
    let score = Score::new("".into(), &[Instrument::Violin], 4).unwrap();
    let mut source = Editor::new(score.clone()).unwrap();
    let mut event = note(Step::C, Duration::Whole);
    event.notation.dynamic = Some(Dynamic::F);
    event.notation.technique = Some(PlayingTechnique::Pizzicato);
    event.notation.fermata = true;
    event.notation.articulation = Some(Articulation::Staccato);
    source.execute(Command::Insert(event)).unwrap();
    let id = source.selection().unwrap();
    let offset = Offset { dx: 10, dy: -6 };
    for mark in [
        MarkRef::Dynamic(id),
        MarkRef::Technique(id),
        MarkRef::Fermata(id),
        MarkRef::Articulation(id),
    ] {
        source
            .execute(Command::SetMarkOffset(mark, offset))
            .unwrap();
    }
    let clip = source.copy_selection().unwrap();
    let mut target = Editor::new(score).unwrap();
    target
        .execute(Command::Insert(note(Step::D, Duration::Quarter)))
        .unwrap();
    let before = target.score().clone();
    target.paste(&clip).unwrap();
    let ids = target.selected_ids();
    assert!(ids.len() >= 2);
    let first = target.score().event(ids[0]).unwrap().notation;
    let last = target.score().event(*ids.last().unwrap()).unwrap().notation;
    assert_eq!(first.offsets.dynamic, Some(offset));
    assert_eq!(first.offsets.technique, Some(offset));
    assert_eq!(first.offsets.fermata, None);
    assert_eq!(first.offsets.articulation, None);
    assert_eq!(last.offsets.dynamic, None);
    assert_eq!(last.offsets.technique, None);
    assert_eq!(last.offsets.fermata, Some(offset));
    assert_eq!(last.offsets.articulation, Some(offset));
    target.score().validate().unwrap();
    assert!(target.undo());
    assert_eq!(target.score(), &before);
}

#[test]
fn bar_mark_offset_is_one_undoable_step_and_reset_is_another() {
    let mut editor = Editor::default();
    let mark = BarMarkRef {
        measure: 0,
        mark: BarMark::Tempo,
    };
    let initial = editor.score().clone();
    let offset = Offset { dx: 25, dy: -14 };
    assert!(
        editor
            .execute(Command::SetBarMarkOffset(mark, offset))
            .unwrap()
    );
    assert_eq!(editor.score().bar_mark_offset(mark), offset);
    assert!(
        !editor
            .execute(Command::SetBarMarkOffset(mark, offset))
            .unwrap()
    );
    assert!(editor.undo());
    assert_eq!(editor.score(), &initial);
    assert!(editor.redo());
    assert!(editor.execute(Command::ResetBarMarkOffset(mark)).unwrap());
    assert_eq!(editor.score().bar_mark_offset(mark), Offset::ZERO);
    assert!(editor.undo());
    assert_eq!(editor.score().bar_mark_offset(mark), offset);
}

#[test]
fn removing_rehearsal_prunes_its_offset_in_the_same_undo_step() {
    let mut editor = Editor::default();
    let mark = BarMarkRef {
        measure: 0,
        mark: BarMark::Rehearsal,
    };
    editor.execute(Command::ToggleRehearsalMark).unwrap();
    let offset = Offset { dx: 10, dy: 20 };
    editor
        .execute(Command::SetBarMarkOffset(mark, offset))
        .unwrap();
    let before = editor.score().clone();
    editor.execute(Command::ToggleRehearsalMark).unwrap();
    assert!(!editor.score().has_bar_mark(mark));
    assert_eq!(editor.score().bar_mark_offset(mark), Offset::ZERO);
    editor.score().validate().unwrap();
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
}

#[test]
fn clearing_form_prunes_navigation_offsets_and_selected_bar_reset() {
    let mut editor = Editor::default();
    editor
        .execute(Command::Insert(note(Step::C, Duration::Quarter)))
        .unwrap();
    editor
        .execute(Command::EditForm(FormEdit::ToggleSegno))
        .unwrap();
    editor
        .execute(Command::EditForm(FormEdit::ToggleCoda))
        .unwrap();
    editor
        .execute(Command::EditForm(FormEdit::ToggleFine))
        .unwrap();
    let marks = [BarMark::Segno, BarMark::Coda, BarMark::Fine];
    for mark in marks {
        editor
            .execute(Command::SetBarMarkOffset(
                BarMarkRef { measure: 0, mark },
                Offset { dx: 3, dy: 4 },
            ))
            .unwrap();
    }
    let with_offsets = editor.score().clone();
    assert!(editor.execute(Command::ResetMarkOffsets(None)).unwrap());
    for mark in marks {
        assert_eq!(
            editor
                .score()
                .bar_mark_offset(BarMarkRef { measure: 0, mark }),
            Offset::ZERO
        );
    }
    assert!(editor.undo());
    assert_eq!(editor.score(), &with_offsets);
    editor.execute(Command::EditForm(FormEdit::Clear)).unwrap();
    for mark in marks {
        let reference = BarMarkRef { measure: 0, mark };
        assert!(!editor.score().has_bar_mark(reference));
        assert_eq!(editor.score().bar_mark_offset(reference), Offset::ZERO);
    }
    assert!(editor.undo());
    assert_eq!(editor.score(), &with_offsets);
}
