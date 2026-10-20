use super::*;
use score_core::{
    Connection, ConnectionKind, Harmonic, MarkKind, MarkRef, Notation, Pitches, Step,
};
fn strings() -> Editor {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    score.add_part(Instrument::Violin).unwrap();
    score.remove_part(piano).unwrap();
    Editor::new(score).unwrap()
}
#[test]
fn passage_harmonics_keep_ties_and_undo_in_one_step() {
    let mut editor = strings();
    let event = editor.note(Step::G);
    editor.execute(Command::Insert(event)).unwrap();
    let a = editor.selection().unwrap();
    editor.execute(Command::Insert(event)).unwrap();
    let b = editor.selection().unwrap();
    editor
        .execute(Command::AddConnection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Tie,
        }))
        .unwrap();
    editor.select(a);
    editor.extend_selection(b);
    let before = editor.score().clone();
    editor
        .execute(Command::ToggleMark(MarkKind::Harmonic(
            Harmonic::Artificial,
        )))
        .unwrap();
    assert_eq!(editor.score().connections().len(), 1);
    for id in [a, b] {
        assert_eq!(
            editor.score().event(id).unwrap().notation.harmonic,
            Some(Harmonic::Artificial)
        );
    }
    editor.undo();
    assert_eq!(editor.score(), &before);
    editor.redo();
    editor
        .execute(Command::ToggleMark(MarkKind::Harmonic(
            Harmonic::Artificial,
        )))
        .unwrap();
    assert_eq!(editor.score(), &before);
    editor.select(b);
    editor
        .execute(Command::ToggleMark(MarkKind::Harmonic(Harmonic::Natural)))
        .unwrap();
    editor
        .execute(Command::RemoveMark(MarkRef::Harmonic(b)))
        .unwrap();
    assert!(editor.score().event(b).unwrap().notation.harmonic.is_none());
}
#[test]
fn harmonic_clipboard_keeps_marks_on_tied_fragments_and_rejects_piano() {
    let mut source = strings();
    source
        .execute(Command::Insert(Event {
            pitches: Pitches::single(Pitch::new(Step::G, 4)),
            duration: Duration::Whole,
            notation: Notation {
                harmonic: Some(Harmonic::Artificial),
                ..Default::default()
            },
        }))
        .unwrap();
    let clip = source.copy_selection().unwrap();
    let wire = serde_json::to_vec(&clip).unwrap();
    let clip: clipboard::MusicalClipboard = serde_json::from_slice(&wire).unwrap();
    assert_eq!(clip.version, 6);
    let mut piano = Editor::default();
    let before = piano.score().clone();
    assert!(piano.paste(&clip).is_err());
    assert_eq!(piano.score(), &before);
    let mut target = strings();
    target
        .execute(Command::Insert(target.note(Step::D)))
        .unwrap();
    let before = target.score().clone();
    target.paste(&clip).unwrap();
    let ids = target.selected_ids();
    assert!(ids.len() >= 2);
    assert_eq!(target.score().connections().len(), ids.len() - 1);
    assert_eq!(
        ids.iter()
            .map(|id| target.score().event(*id).unwrap().duration.ticks())
            .sum::<u16>(),
        192
    );
    for id in ids {
        assert_eq!(
            target.score().event(id).unwrap().notation.harmonic,
            Some(Harmonic::Artificial)
        );
    }
    target.score().validate().unwrap();
    target.undo();
    assert_eq!(target.score(), &before);
}
#[test]
fn mixed_note_rest_passage_is_rejected_without_partial_marks() {
    let mut editor = strings();
    editor
        .execute(Command::Insert(editor.note(Step::G)))
        .unwrap();
    let a = editor.selection().unwrap();
    editor
        .execute(Command::Insert(Event {
            pitches: Pitches::default(),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }))
        .unwrap();
    let b = editor.selection().unwrap();
    editor.select(a);
    editor.extend_selection(b);
    let before = editor.score().clone();
    assert!(
        editor
            .execute(Command::ToggleMark(MarkKind::Harmonic(Harmonic::Natural)))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
}
