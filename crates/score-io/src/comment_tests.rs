use super::*;
use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, StaffId, Step};

fn fixture() -> Score {
    let mut score = Score::new("Observaciones".into(), &[Instrument::Flute], 2).unwrap();
    let part = score.parts()[0].id();
    let id = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    score
        .save_comment(None, "Frase más cálida ♯", &[id])
        .unwrap();
    let comment = score.comments().back().unwrap().id;
    score.set_comment_resolved(comment, true).unwrap();
    score
}

#[test]
fn comments_round_trip_with_state_and_anchors_and_legacy_scores_have_none() {
    let score = fixture();
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let mut old: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    old["version"] = 26.into();
    old["score"].as_object_mut().unwrap().remove("comments");
    old["score"]
        .as_object_mut()
        .unwrap()
        .remove("next_comment_id");
    assert!(
        decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .comments()
            .is_empty()
    );
    let mut detached = score.clone();
    let note = detached.comments()[0].anchors[0];
    detached.remove(note).unwrap();
    assert_eq!(decode(&encode(&detached).unwrap()).unwrap(), detached);
}

#[test]
fn malformed_comment_metadata_is_rejected_without_silently_dropping_observations() {
    let data: serde_json::Value = serde_json::from_slice(&encode(&fixture()).unwrap()).unwrap();
    for change in [0, 1, 2, 3, 4] {
        let mut invalid = data.clone();
        match change {
            0 => invalid["score"]["comments"][0]["text"] = " ".into(),
            1 => invalid["score"]["comments"][0]["anchors"][0]["measure"] = 99.into(),
            2 => invalid["score"]["next_comment_id"] = 0.into(),
            3 => {
                let duplicate = invalid["score"]["comments"][0].clone();
                invalid["score"]["comments"]
                    .as_array_mut()
                    .unwrap()
                    .push(duplicate);
            }
            _ => invalid["score"]["comments"][0]["part"] = 99.into(),
        }
        assert!(
            decode(&serde_json::to_vec(&invalid).unwrap()).is_err(),
            "case {change}"
        );
    }
}

#[test]
fn repeated_long_passage_comments_reach_the_metadata_budget_before_the_file_limit() {
    let mut score = Score::new("Pasajes".into(), &[Instrument::Violin], 100).unwrap();
    let part = score.parts()[0].id();
    let mut anchors = Vec::new();
    for bar in 0..100 {
        for index in 0..4 {
            anchors.push(
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        index,
                        Event {
                            pitches: Pitches::single(Pitch::new(Step::C, 4)),
                            duration: Duration::Quarter,
                            notation: Notation::default(),
                        },
                    )
                    .unwrap(),
            );
        }
    }
    let mut accepted = 0;
    while score.save_comment(None, "Revisar pasaje", &anchors).is_ok() {
        accepted += 1;
    }
    assert!(accepted > 1 && accepted < score_core::MAX_COMMENTS);
    let before = score.clone();
    assert!(
        score
            .save_comment(None, "Revisar pasaje", &anchors)
            .is_err()
    );
    assert_eq!(score, before);
    let bytes = encode(&score).unwrap();
    assert!(bytes.len() < MAX_FILE_BYTES / 2);
    assert_eq!(decode(&bytes).unwrap(), score);
    let id = score.comments()[0].id;
    score
        .save_comment(Some(id), "Revisar una nota", &anchors[..1])
        .unwrap();
    score
        .save_comment(None, "Revisar pasaje", &anchors)
        .unwrap();
    assert!(encode(&score).is_ok());
}
