use super::*;
use score_core::{Instrument, LayoutBreak};

#[test]
fn layout_breaks_round_trip_and_legacy_files_have_none() {
    let mut score = Score::new("Saltos".into(), &[Instrument::Violin], 6).unwrap();
    score.toggle_layout_break(1, LayoutBreak::System).unwrap();
    score.toggle_layout_break(3, LayoutBreak::Page).unwrap();
    let bytes = encode(&score).unwrap();
    let back = decode(&bytes).unwrap();
    assert_eq!(back, score);
    assert_eq!(back.layout_break(1), Some(LayoutBreak::System));
    assert_eq!(back.layout_break(3), Some(LayoutBreak::Page));
    let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data["version"], SCHEMA_VERSION);
    data["score"].as_object_mut().unwrap().remove("breaks");
    data["version"] = 27.into();
    let old = decode(&serde_json::to_vec(&data).unwrap()).unwrap();
    assert_eq!(old.layout_breaks().count(), 0);
}
