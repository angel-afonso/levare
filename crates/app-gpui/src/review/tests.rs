use super::{ReviewState, select_target};
use score_core::{
    BarForm, BarMark, BarMarkRef, Duration, ElementId, Event, Instrument, Jump, Notation, PartId,
    Pitch, Pitches, ReviewKind, ReviewTarget, Score, StaffId, Step, Voice,
};

fn score_with_notes(count: usize) -> (Score, Vec<ElementId>) {
    let mut score = Score::new("Avisos".into(), &[Instrument::Flute], count).unwrap();
    let mut ids = Vec::new();
    for measure in 0..count {
        ids.push(
            score
                .insert_on(
                    score.parts()[0].id(),
                    StaffId::Upper,
                    measure,
                    0,
                    Event {
                        pitches: Pitches::single(Pitch::new(Step::C, 3)),
                        duration: Duration::Quarter,
                        notation: Notation::default(),
                    },
                )
                .unwrap(),
        );
    }
    (score, ids)
}

#[test]
fn navigation_wraps_and_reveals_the_current_page_without_truncating_reports() {
    let (score, _) = score_with_notes(43);
    let mut review = ReviewState::default();
    assert_eq!(review.next(true), None);
    review.check(&score, None);
    assert_eq!(review.next(false), Some(42));
    review.current = Some(42);
    review.reveal_current();
    assert_eq!(review.page, 2);
    assert_eq!(review.next(true), Some(0));
    review.current = Some(0);
    assert_eq!(review.next(false), Some(42));
    let other = PartId(999);
    review.set_filter(Some(other));
    assert!(review.filtered().is_empty());
    assert_eq!(review.next(true), None);
}

#[test]
fn full_reports_update_selection_reports_expire_and_filters_keep_global_issues() {
    let (mut score, ids) = score_with_notes(2);
    let mut review = ReviewState::default();
    review.check(&score, None);
    let first_snapshot = review.checked_score.clone();
    review.refresh(&score);
    assert_eq!(review.checked_score, first_snapshot);
    score
        .replace_pitch(ids[0], 0, Pitch::new(Step::C, 5).unwrap())
        .unwrap();
    review.refresh(&score);
    assert_eq!(review.issues.len(), 1);
    review.check(&score, Some(&[ids[1]]));
    score
        .replace_pitch(ids[1], 0, Pitch::new(Step::C, 5).unwrap())
        .unwrap();
    review.refresh(&score);
    assert!(review.stale);
    assert!(!review.checked());
    assert!(review.issues.is_empty());
    score
        .set_bar_form(
            1,
            BarForm {
                jump: Some(Jump::DalSegno),
                ..Default::default()
            },
        )
        .unwrap();
    review.check(&score, None);
    review.set_filter(Some(ids[0].part));
    assert_eq!(review.filtered().len(), 1);
    assert_eq!(review.issues[0].kind, ReviewKind::MissingSegno);
    review.show_ranges = false;
    review.reset_document();
    assert!(!review.show_ranges);
    assert!(!review.checked());
    assert_eq!(review.filter, None);
}

#[test]
fn targets_select_the_exact_chord_tone_and_global_mark_in_the_correct_voice() {
    let mut score = Score::new("Navegar".into(), &[Instrument::Piano], 3).unwrap();
    let part = score.parts()[0].id();
    let low = Pitch::new(Step::C, 0).unwrap();
    let valid = Pitch::new(Step::C, 4).unwrap();
    let id = score
        .insert_in_voice(
            part,
            StaffId::Lower,
            2,
            Voice::Two,
            0,
            Event {
                pitches: Pitches::new(&[low, valid]).unwrap(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    let mut editor = score_edit::Editor::new(score).unwrap();
    select_target(&mut editor, ReviewTarget::Note(id, low));
    assert_eq!(editor.selection(), Some(id));
    assert_eq!(editor.selected_pitch(), Some(low));
    assert_eq!(editor.cursor().voice, Voice::Two);
    assert_eq!(editor.cursor().staff, StaffId::Lower);
    let mark = select_target(&mut editor, ReviewTarget::Bar(0, Some(BarMark::Jump)));
    assert_eq!(
        mark,
        Some(BarMarkRef {
            measure: 0,
            mark: BarMark::Jump
        })
    );
    assert_eq!(editor.cursor().measure, 0);
    assert!(editor.selection().is_none());
}

#[test]
fn a_full_report_follows_the_editor_through_edits_and_undo() {
    let (score, ids) = score_with_notes(2);
    let mut editor = score_edit::Editor::new(score).unwrap();
    let mut review = ReviewState::default();
    review.check(editor.score(), None);
    assert_eq!(review.issues.len(), 2);
    review.follow(&editor);
    assert_eq!(review.issues.len(), 2);
    editor.select(ids[0]);
    editor.execute(score_edit::Command::Transpose(14)).unwrap();
    review.follow(&editor);
    assert_eq!(review.issues.len(), 1);
    assert!(editor.undo());
    review.follow(&editor);
    assert_eq!(review.issues.len(), 2);
}
