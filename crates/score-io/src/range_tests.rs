use super::*;
use score_core::{Instrument, InstrumentRange};

#[test]
fn custom_ranges_round_trip_and_legacy_files_keep_the_instrument_defaults() {
    let mut score = Score::new(
        "Registro".into(),
        &[Instrument::ClarinetBb, Instrument::ClarinetBb],
        1,
    )
    .unwrap();
    let part = score.parts()[0].id();
    let range = InstrumentRange {
        lowest: 50,
        highest: 82,
    };
    score.set_instrument_range(part, Some(range)).unwrap();
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let extracted = score.extract_part(part).unwrap();
    assert_eq!(extracted.part(part).unwrap().range_override(), Some(range));
    let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data["version"], SCHEMA_VERSION);
    data["score"]["parts"][0]
        .as_object_mut()
        .unwrap()
        .remove("instrument_range");
    data["version"] = 25.into();
    let old = decode(&serde_json::to_vec(&data).unwrap()).unwrap();
    assert_eq!(old.part(part).unwrap().range_override(), None);
    assert_eq!(
        old.part(part).unwrap().sounding_range(),
        Instrument::ClarinetBb.sounding_range()
    );
    for bounds in [(90, 60), (0, 60), (12, 128)] {
        data["version"] = 26.into();
        data["score"]["parts"][0]["instrument_range"] =
            serde_json::json!({"lowest": bounds.0, "highest": bounds.1});
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
    }
}
