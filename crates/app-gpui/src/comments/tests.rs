use super::{CommentFilter, CommentState};
use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, Score, StaffId, Step};

#[test]
fn filters_keep_resolved_observations_available_and_clear_removed_active_items() {
    let mut score = Score::new("Comentarios".into(), &[Instrument::Flute], 2).unwrap();
    let part = score.parts()[0].id();
    let mut ids = Vec::new();
    for bar in 0..2 {
        let note = score
            .insert_on(
                part,
                StaffId::Upper,
                bar,
                0,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                    notation: Notation::default(),
                },
            )
            .unwrap();
        score.save_comment(None, "Revisar", &[note]).unwrap();
        ids.push(score.comments().back().unwrap().id);
    }
    let mut state = CommentState::default();
    assert_eq!(state.filtered(&score), ids);
    state.filter = CommentFilter::Pending;
    state.active = Some(ids[0]);
    score.set_comment_resolved(ids[0], true).unwrap();
    state.refresh(&score);
    assert!(state.active.is_none());
    assert_eq!(state.filtered(&score), vec![ids[1]]);
    state.filter = CommentFilter::Resolved;
    assert_eq!(state.filtered(&score), vec![ids[0]]);
    state.active = Some(ids[0]);
    score.remove_comment(ids[0]).unwrap();
    state.refresh(&score);
    assert!(state.active.is_none());
    assert!(state.filtered(&score).is_empty());
}

#[test]
fn adding_and_resolving_comments_does_not_expire_a_selection_review() {
    let mut score = Score::new("Revisión".into(), &[Instrument::Flute], 1).unwrap();
    let part = score.parts()[0].id();
    let note = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 3)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let mut report = crate::review::ReviewState::default();
    report.check(&score, Some(&[note]));
    let original = report.issues.clone();
    score
        .save_comment(None, "Revisar registro", &[note])
        .unwrap();
    report.refresh(&score);
    assert!(report.checked());
    assert!(!report.stale);
    let id = score.comments().back().unwrap().id;
    score.set_comment_resolved(id, true).unwrap();
    report.refresh(&score);
    assert_eq!(report.issues, original);
    assert!(!report.stale);
    score
        .replace_pitch(note, 0, Pitch::new(Step::C, 5).unwrap())
        .unwrap();
    report.refresh(&score);
    assert!(report.stale);
}
