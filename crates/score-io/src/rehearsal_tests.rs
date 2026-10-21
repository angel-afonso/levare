use super::*;
use score_core::RehearsalStyle;
#[test]
fn native_rehearsals_round_trip_and_schema_twenty_one_defaults() {
    let mut score = Score::default();
    score.set_rehearsal_custom(0, Some("Trío".into())).unwrap();
    score.toggle_rehearsal_mark(3).unwrap();
    score.set_rehearsal_style(RehearsalStyle::BarNumbers);
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let plain = Score::default();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&plain).unwrap()).unwrap();
    old["version"] = 21.into();
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), plain);
    for invalid in [
        serde_json::json!({"4":{}}),
        serde_json::json!({"0":{"custom":""}}),
        serde_json::json!({"0":{"custom":"abcdefghijklmnopq"}}),
    ] {
        old["score"]["rehearsals"] = invalid;
        assert!(decode(&serde_json::to_vec(&old).unwrap()).is_err());
    }
}
#[test]
fn native_rehearsal_validation_retains_structural_guards() {
    let mut file: serde_json::Value =
        serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
    file["score"]["parts"] = serde_json::json!([]);
    file["score"]["rehearsals"] = serde_json::json!({"0":{}});
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
}
