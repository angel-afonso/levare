use super::*;
use crate::{BarForm, Duration, Event, Instrument, Jump, Notation, Pitches, StaffId, Step, Voice};

fn note(
    score: &mut Score,
    part: crate::PartId,
    staff: StaffId,
    voice: Voice,
    measure: usize,
    pitches: &[Pitch],
) -> ElementId {
    score
        .insert_in_voice(
            part,
            staff,
            measure,
            voice,
            0,
            Event {
                pitches: Pitches::new(pitches).unwrap(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap()
}

#[test]
fn checks_each_chord_tone_and_both_voices_and_hands_in_musical_bar_order() {
    let mut score = Score::new(
        "Revisión".into(),
        &[Instrument::Flute, Instrument::Piano],
        3,
    )
    .unwrap();
    let flute = score.parts()[0].id();
    let piano = score.parts()[1].id();
    let low = Pitch::new(Step::C, 2).unwrap();
    let high = Pitch::new(Step::G, 8).unwrap();
    let valid = Pitch::new(Step::C, 5).unwrap();
    let a = note(
        &mut score,
        flute,
        StaffId::Upper,
        Voice::Two,
        2,
        &[low, valid, high],
    );
    let b = note(
        &mut score,
        piano,
        StaffId::Lower,
        Voice::One,
        0,
        &[Pitch::new(Step::C, 0).unwrap()],
    );
    let issues = score.review(None);
    assert_eq!(issues.len(), 3);
    assert!(matches!(issues[0].target, ReviewTarget::Note(id, _) if id == b));
    assert_eq!(issues[1].target, ReviewTarget::Note(a, low));
    assert_eq!(issues[2].target, ReviewTarget::Note(a, high));
    assert_eq!(score.review(Some(&[a])).len(), 2);
    assert!(score.review(Some(&[])).is_empty());
}

#[test]
fn missing_form_destinations_are_global_once_and_selection_is_local_to_its_bars() {
    let mut score = Score::new("Forma".into(), &[Instrument::Piano, Instrument::Flute], 4).unwrap();
    score
        .set_bar_form(
            3,
            BarForm {
                jump: Some(Jump::DalSegnoAlFine),
                ..Default::default()
            },
        )
        .unwrap();
    let issues = score.review(None);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ReviewKind::MissingSegno);
    score
        .set_bar_form(
            0,
            BarForm {
                segno: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(score.review(None)[0].kind, ReviewKind::MissingFine);
    score
        .set_bar_form(
            2,
            BarForm {
                fine: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(None).is_empty());
    score
        .set_bar_form(
            3,
            BarForm {
                jump: Some(Jump::DaCapoAlCoda),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(score.review(None)[0].kind, ReviewKind::MissingToCoda);
    score
        .set_bar_form(
            1,
            BarForm {
                to_coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(score.review(None)[0].kind, ReviewKind::MissingCoda);
    let part = score.parts()[0].id();
    let note = note(
        &mut score,
        part,
        StaffId::Upper,
        Voice::One,
        0,
        &[Pitch::new(Step::C, 4).unwrap()],
    );
    assert!(score.review(Some(&[note])).is_empty());
    score
        .set_bar_form(
            2,
            BarForm {
                coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(None).is_empty());
    score.set_bar_form(3, BarForm::default()).unwrap();
    assert_eq!(score.review(None)[0].kind, ReviewKind::ToCodaWithoutJump);
}

#[test]
fn closing_repeat_can_return_to_start_but_opening_repeat_needs_an_end() {
    let mut score = Score::new("Repeticiones".into(), &[Instrument::Violin], 4).unwrap();
    assert!(score.review(None).is_empty()); // Empty measures are intentional, not errors.
    score
        .set_bar_form(
            1,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(None).is_empty());
    score
        .set_bar_form(
            2,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(score.review(None)[0].target, ReviewTarget::Bar(2, None));
    score
        .set_bar_form(
            3,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(None).is_empty());
}

#[test]
fn passage_checks_form_marks_in_empty_interior_bars_and_empty_scope_checks_nothing() {
    let mut score = Score::new("Pasaje".into(), &[Instrument::Violin], 4).unwrap();
    let part = score.parts()[0].id();
    let pitch = Pitch::new(Step::C, 4).unwrap();
    let first = note(&mut score, part, StaffId::Upper, Voice::One, 0, &[pitch]);
    let last = note(&mut score, part, StaffId::Upper, Voice::One, 3, &[pitch]);
    score
        .set_bar_form(
            2,
            BarForm {
                jump: Some(Jump::DalSegno),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(Some(&[])).is_empty());
    let issues = score.review(Some(&[first, last]));
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].target, ReviewTarget::Bar(2, Some(BarMark::Jump)));
}

#[test]
fn destinations_inside_a_skipped_first_ending_do_not_satisfy_a_jump() {
    use crate::Ending;
    let mut score = Score::new("Casillas".into(), &[Instrument::Violin], 6).unwrap();
    score
        .set_ending(Ending {
            start: 2,
            end: 2,
            numbers: vec![1],
        })
        .unwrap();
    score
        .set_ending(Ending {
            start: 3,
            end: 3,
            numbers: vec![2],
        })
        .unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                fine: true,
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            4,
            BarForm {
                jump: Some(Jump::DaCapoAlFine),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        score
            .review(None)
            .iter()
            .any(|issue| issue.kind == ReviewKind::MissingFine)
    );
    assert_eq!(score.play_order(true).last().unwrap().bar, 5);
    score
        .set_bar_form(
            3,
            BarForm {
                fine: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(score.review(None).is_empty());
    score
        .set_bar_form(
            3,
            BarForm {
                coda: true,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            2,
            BarForm {
                to_coda: true,
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            4,
            BarForm {
                jump: Some(Jump::DaCapoAlCoda),
                ..Default::default()
            },
        )
        .unwrap();
    let issues = score.review(None);
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == ReviewKind::MissingToCoda)
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.kind == ReviewKind::ToCodaWithoutJump)
    );
}
