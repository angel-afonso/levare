use super::*;
use score_core::{Duration, Event, Notation, Pitch, Pitches, Step, Tremolo};
#[test]
fn native_tremolo_round_trips_and_schema_twenty_defaults_to_absent() {
    let mut score = Score::default();
    for (index, mark) in Tremolo::ALL.into_iter().enumerate() {
        score
            .insert(
                0,
                index,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::C, 4)),
                    duration: Duration::Quarter,
                    notation: Notation {
                        tremolo: Some(mark),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
    }
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let plain = Score::default();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&plain).unwrap()).unwrap();
    old["version"] = 20.into();
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), plain);
}
#[test]
fn native_deserialization_rejects_tremolo_on_a_rest() {
    let mut score = Score::default();
    score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter,
                notation: Notation {
                    tremolo: Some(Tremolo::Two),
                    ..Default::default()
                },
            },
        )
        .unwrap();
    let mut file: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    for slot in file["score"]["parts"][0]["staves"][0]["measures"][0]["events"]
        .as_array_mut()
        .unwrap()
    {
        if slot["value"].is_object() {
            slot["value"]["pitches"] = serde_json::json!([]);
        }
    }
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
}
#[test]
fn native_grace_notes_round_trip() {
    let mut score = Score::default();
    let id = score
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::D, 5)),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    score.add_grace_note(id).unwrap();
    score.add_grace_note(id).unwrap();
    score.transpose_grace_note(id, 1, 2).unwrap();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
}
