use super::*;
use score_core::{CommentId, DivisiPoint, Step, Voice};

fn fixture() -> (Editor, Vec<ElementId>) {
    let mut score = Score::new("Comentarios".into(), &[Instrument::Piano], 3).unwrap();
    let part = score.parts()[0].id();
    let anchors = (0..3)
        .map(|measure| {
            score
                .insert_in_voice(
                    part,
                    StaffId::Lower,
                    measure,
                    Voice::Two,
                    0,
                    Event {
                        pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                        duration: Duration::Whole,
                        notation: Default::default(),
                    },
                )
                .unwrap()
        })
        .collect();
    (Editor::new(score).unwrap(), anchors)
}

#[test]
fn comment_transactions_preserve_passage_and_exact_position_and_undo_independently() {
    let (mut editor, anchors) = fixture();
    editor.select(anchors[0]);
    editor.extend_selection(anchors[2]);
    let selected = editor.selected_ids();
    let point = DivisiPoint::new(2, Duration::Quarter.ticks());
    editor.select_divisi_position(point);
    let before = editor.score().clone();
    editor
        .execute(Command::SaveComment {
            id: None,
            text: "Más ligado".into(),
            anchors: selected.clone(),
        })
        .unwrap();
    let id = editor.score().comments().back().unwrap().id;
    assert_eq!(editor.selected_ids(), selected);
    assert_eq!(editor.divisi_position(), point);
    assert!(editor.score().same_content_except_comments(&before));
    editor
        .execute(Command::SetCommentResolved(id, true))
        .unwrap();
    assert!(editor.score().comment(id).unwrap().resolved);
    assert!(editor.undo());
    assert!(!editor.score().comment(id).unwrap().resolved);
    assert!(editor.undo());
    assert_eq!(editor.score(), &before);
    assert!(editor.redo());
    assert!(editor.redo());
    editor.execute(Command::RemoveComment(id)).unwrap();
    assert!(editor.score().comment(id).is_none());
    assert!(editor.undo());
    assert!(editor.score().comment(id).unwrap().resolved);
    assert!(editor.select_comment(id));
    assert_eq!(editor.selected_ids(), selected);
    assert_eq!(editor.selection(), Some(anchors[0]));
    assert_eq!(editor.cursor().voice, Voice::Two);
    assert_eq!(editor.cursor().staff, StaffId::Lower);
}

#[test]
fn deleting_the_commented_music_keeps_text_and_undo_restores_the_link() {
    let (mut editor, anchors) = fixture();
    editor.select(anchors[0]);
    editor
        .execute(Command::SaveComment {
            id: None,
            text: "Revisar".into(),
            anchors: vec![anchors[0]],
        })
        .unwrap();
    let id = editor.score().comments().back().unwrap().id;
    editor.execute(Command::Delete).unwrap();
    assert!(editor.score().comment_anchors(id).is_empty());
    assert!(!editor.select_comment(id));
    assert!(editor.undo());
    assert!(editor.select_comment(id));
    assert_eq!(editor.selection(), Some(anchors[0]));
    let before = editor.score().clone();
    assert!(
        !editor
            .execute(Command::SaveComment {
                id: Some(id),
                text: "Revisar".into(),
                anchors: vec![anchors[0]]
            })
            .unwrap()
    );
    assert!(
        editor
            .execute(Command::SetCommentResolved(CommentId(999), true))
            .is_err()
    );
    assert_eq!(editor.score(), &before);
}
