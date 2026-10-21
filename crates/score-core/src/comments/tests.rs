use super::*;

fn add_note(
    score: &mut Score,
    part: PartId,
    measure: usize,
    voice: Voice,
    index: usize,
) -> ElementId {
    score
        .insert_in_voice(
            part,
            StaffId::Upper,
            measure,
            voice,
            index,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                notation: Default::default(),
            },
        )
        .unwrap()
}
fn fixture() -> (Score, Vec<ElementId>) {
    let mut score = Score::new(
        "Observaciones".into(),
        &[Instrument::Violin, Instrument::Flute],
        3,
    )
    .unwrap();
    let part = score.parts()[0].id();
    let ids = (0..3)
        .map(|bar| add_note(&mut score, part, bar, Voice::One, 0))
        .collect();
    (score, ids)
}
fn add(score: &mut Score, text: &str, anchors: &[ElementId]) -> CommentId {
    score.save_comment(None, text, anchors).unwrap();
    score.comments().back().unwrap().id
}

#[test]
fn comments_add_edit_resolve_reopen_and_delete_with_stable_ids() {
    let (mut score, ids) = fixture();
    let music = score.clone();
    let id = add(&mut score, "  Revisar el arco  ", &ids);
    assert_eq!(score.comment(id).unwrap().text.as_ref(), "Revisar el arco");
    assert!(!score.comment(id).unwrap().resolved);
    assert!(
        !score
            .save_comment(Some(id), "Revisar el arco", &ids)
            .unwrap()
    );
    score.set_comment_resolved(id, true).unwrap();
    assert!(
        score
            .save_comment(Some(id), "Cambiar el arco", &ids[..1])
            .unwrap()
    );
    assert!(score.comment(id).unwrap().resolved);
    score.set_comment_resolved(id, false).unwrap();
    assert!(music.same_content_except_comments(&score));
    score.remove_comment(id).unwrap();
    let next = add(&mut score, "Otra observación", &ids[..1]);
    assert!(next.0 > id.0);
    assert!(score.set_comment_resolved(id, true).is_err());
    score.validate().unwrap();
}

#[test]
fn invalid_changes_are_atomic_and_new_comments_require_real_music() {
    let (mut score, ids) = fixture();
    let original = score.clone();
    for text in [
        "".to_string(),
        "   ".into(),
        "a\nb".into(),
        "x".repeat(MAX_COMMENT_CHARS + 1),
    ] {
        assert!(score.save_comment(None, &text, &ids).is_err());
        assert_eq!(score, original);
    }
    assert!(score.save_comment(None, "Observación", &[]).is_err());
    assert!(
        score
            .save_comment(None, "Observación", &[ids[0], ids[0]])
            .is_err()
    );
    assert!(
        score
            .save_comment(
                None,
                "Observación",
                &[ElementId {
                    measure: 999,
                    ..ids[0]
                }]
            )
            .is_err()
    );
    let other = add_note(&mut score, original.parts()[1].id(), 0, Voice::One, 0);
    let before = score.clone();
    assert!(
        score
            .save_comment(None, "Dos partes", &[ids[0], other])
            .is_err()
    );
    assert_eq!(score, before);
}

#[test]
fn anchors_follow_bars_and_deleted_music_keeps_a_detached_observation() {
    let (mut score, ids) = fixture();
    let id = add(&mut score, "Revisar frase", &ids);
    score.insert_measure(1).unwrap();
    assert_eq!(
        score
            .comment_anchors(id)
            .iter()
            .map(|id| id.measure)
            .collect::<Vec<_>>(),
        vec![0, 2, 3]
    );
    score.remove_measures(0, 3).unwrap();
    assert_eq!(score.comment_anchors(id).len(), 1);
    assert_eq!(score.comment_anchors(id)[0].measure, 0);
    score.remove(score.comment_anchors(id)[0]).unwrap();
    assert!(score.comment_anchors(id).is_empty());
    assert_eq!(score.comment(id).unwrap().text.as_ref(), "Revisar frase");
    score
        .save_comment(Some(id), "Conservar observación", &[])
        .unwrap();
    let new = add_note(&mut score, ids[0].part, 0, Voice::Two, 0);
    score
        .save_comment(Some(id), "Vinculada de nuevo", &[new])
        .unwrap();
    assert_eq!(score.comment_anchors(id), vec![new]);
    score.remove_part(ids[0].part).unwrap();
    assert!(score.comment_anchors(id).is_empty());
    assert!(!score.comment(id).unwrap().part_name.is_empty());
    score.validate().unwrap();
}

#[test]
fn moved_swapped_and_overwritten_notes_keep_their_observations() {
    let (mut score, ids) = fixture();
    let first = add(&mut score, "Primera", &ids[..1]);
    let second = add(&mut score, "Segunda", &ids[1..2]);
    let moved = score.move_event(ids[0], 1, 0).unwrap();
    assert_eq!(score.comment_anchors(first), vec![moved]);
    assert_eq!(score.comment_anchors(second)[0].measure, 0);
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::D, 4)),
        duration: Duration::Half,
        notation: Default::default(),
    };
    let written = score
        .overwrite(moved.part, moved.staff, 1, Voice::One, 0, event)
        .unwrap();
    assert_eq!(score.comment_anchors(first), vec![written[0]]);
    assert_eq!(score.comment_anchors(second).len(), 1);
    score.validate().unwrap();
}

#[test]
fn divisi_and_part_extraction_preserve_the_right_comments() {
    let (mut score, ids) = fixture();
    let note = add_note(&mut score, ids[0].part, 0, Voice::Two, 0);
    let id = add(&mut score, "Segunda voz", &[note]);
    let other_part = score.parts()[1].id();
    let other = add_note(&mut score, other_part, 0, Voice::One, 0);
    add(&mut score, "Otra parte", &[other]);
    score.set_part_divisi(ids[0].part, true).unwrap();
    assert_eq!(score.comment_anchors(id)[0].staff, StaffId::Lower);
    score.set_part_divisi(ids[0].part, false).unwrap();
    assert_eq!(score.comment_anchors(id)[0].staff, StaffId::Upper);
    assert_eq!(
        score.voice_of(score.comment_anchors(id)[0]),
        Some(Voice::Two)
    );
    let extracted = score.extract_part(ids[0].part).unwrap();
    assert_eq!(extracted.comments().len(), 1);
    assert!(extracted.comment(id).is_some());
    score.validate().unwrap();
}

#[test]
fn aggregate_metadata_budget_is_atomic_and_can_be_freed_by_editing_or_deleting() {
    let (mut score, ids) = fixture();
    let text = "🎵".repeat(MAX_COMMENT_CHARS);
    while score.save_comment(None, &text, &ids).is_ok() {}
    assert!(score.comments().len() < MAX_COMMENTS);
    let before = score.clone();
    assert!(score.save_comment(None, &text, &ids).is_err());
    assert_eq!(score, before);
    score.validate().unwrap();
    // Imports must respect the same aggregate bound.
    let mut invalid = before.clone();
    for _ in 0..2 {
        let mut comment = invalid.comments[0].clone();
        invalid.next_comment_id += 1;
        comment.id = CommentId(invalid.next_comment_id);
        invalid.comments.push_back(comment);
    }
    assert!(invalid.validate().is_err());
    let id = score.comments()[0].id;
    score.save_comment(Some(id), "Breve", &ids[..1]).unwrap();
    assert!(score.save_comment(None, &text, &ids).is_ok());
    let id = score.comments()[1].id;
    score.remove_comment(id).unwrap();
    assert!(score.save_comment(None, &text, &ids).is_ok());
    score.validate().unwrap();
}
