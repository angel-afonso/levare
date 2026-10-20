use super::*;
use score_core::{Duration, Event, Harmonic, Instrument, Notation, Pitch, Pitches, StaffId, Step};
fn piece() -> Score {
    let mut score = Score::default();
    let part = score.add_part(Instrument::Viola).unwrap();
    for (index, harmonic) in [Harmonic::Natural, Harmonic::Artificial]
        .into_iter()
        .enumerate()
    {
        score
            .insert_on(
                part,
                StaffId::Upper,
                0,
                index,
                Event {
                    pitches: Pitches::single(Pitch::new(Step::G, 4)),
                    duration: Duration::Quarter,
                    notation: Notation {
                        harmonic: Some(harmonic),
                        ..Default::default()
                    },
                },
            )
            .unwrap();
    }
    score
}
#[test]
fn harmonic_documents_and_recovery_round_trip_and_earlier_scores_still_open() {
    let score = piece();
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recover.json");
    let recovery = Recovery::open(&path).unwrap();
    let snapshot = RecoveredScore {
        score: score.clone(),
        original_path: None,
    };
    recovery.update(Some(snapshot.clone()));
    recovery.flush().unwrap();
    assert_eq!(load_recovery(&path).unwrap(), Some(snapshot));
    let mut old: serde_json::Value =
        serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
    old["version"] = 17.into();
    assert_eq!(
        decode(&serde_json::to_vec(&old).unwrap()).unwrap(),
        Score::default()
    );
}
#[test]
fn native_input_rejects_harmonics_on_rests_and_incompatible_instruments() {
    let mut file: serde_json::Value = serde_json::from_slice(&encode(&piece()).unwrap()).unwrap();
    let original = file.clone();
    file["score"]["parts"][1]["instrument"] = "Flute".into();
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
    file = original;
    // SlotMap serialization has a sentinel before the live slots.
    let slots = file["score"]["parts"][1]["staves"][0]["measures"][0]["events"]
        .as_array_mut()
        .unwrap();
    for slot in slots {
        if slot["value"].is_object() {
            slot["value"]["pitches"] = serde_json::json!([]);
        }
    }
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
}
