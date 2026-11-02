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

#[test]
fn numbering_round_trips_and_rejects_numbering_every_bar_by_step() {
    use score_core::{BarNumbers, Numbering, PageNumbers};
    let mut score = Score::default();
    let plain = encode(&score).unwrap();
    assert!(!String::from_utf8_lossy(&plain).contains("numbering"));
    score
        .set_numbering(Numbering {
            bars: BarNumbers::Every(5),
            pages: PageNumbers::Corner,
        })
        .unwrap();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let mut file: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    file["score"]["numbering"]["bars"] = serde_json::json!({"Every": 1});
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
}

#[test]
fn page_setup_round_trips_and_rejects_impossible_staff_sizes() {
    use score_core::{PageSetup, Paper};
    let mut score = Score::default();
    assert!(!String::from_utf8_lossy(&encode(&score).unwrap()).contains("page_setup"));
    score
        .set_page_setup(PageSetup {
            paper: Paper::Tabloid,
            landscape: true,
            staff: Some(45),
        })
        .unwrap();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let mut file: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    file["score"]["page_setup"]["staff"] = 250.into();
    assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
}
