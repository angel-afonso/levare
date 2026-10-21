use super::*;
use score_core::{Clef, Instrument, PartId, StaffId};

fn score() -> Score {
    let mut score =
        Score::new("Claves".into(), &[Instrument::Viola, Instrument::Piano], 6).unwrap();
    score
        .set_clef_at(PartId(0), StaffId::Upper, 2, Clef::Treble)
        .unwrap();
    score
        .set_clef_at(PartId(0), StaffId::Upper, 5, Clef::Alto)
        .unwrap();
    score
        .set_clef_at(PartId(1), StaffId::Upper, 0, Clef::Bass)
        .unwrap();
    score
        .set_clef_at(PartId(1), StaffId::Lower, 3, Clef::Tenor)
        .unwrap();
    score
}
#[test]
fn native_round_trip_legacy_defaults_and_invalid_maps() {
    let score = score();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let plain = Score::new("Antigua".into(), &[Instrument::Viola], 3).unwrap();
    let mut file: serde_json::Value = serde_json::from_slice(&encode(&plain).unwrap()).unwrap();
    file["version"] = 22.into();
    assert_eq!(decode(&serde_json::to_vec(&file).unwrap()).unwrap(), plain);
    for map in [
        serde_json::json!({"Upper":{"changes":{"3":"Treble"}}}),
        serde_json::json!({"Upper":{"changes":{"0":"Treble"}}}),
        serde_json::json!({"Lower":{"initial":"Bass"}}),
        serde_json::json!({"Upper":{"initial":"Percussion"}}),
    ] {
        file["score"]["parts"][0]["clefs"] = map;
        assert!(decode(&serde_json::to_vec(&file).unwrap()).is_err());
    }
}
#[test]
fn musicxml_preserves_initial_and_later_staff_clefs() {
    let score = score();
    let xml = musicxml::encode(&score).unwrap();
    let imported = musicxml::decode(&xml).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    for (old, new) in score.parts().iter().zip(imported.score.parts()) {
        for staff in old.staves() {
            for bar in 0..6 {
                assert_eq!(old.clef_at(staff.id(), bar), new.clef_at(staff.id(), bar));
            }
        }
    }
    let extracted = score.extract_part(PartId(1)).unwrap();
    assert_eq!(extracted.parts()[0].clef_at(StaffId::Lower, 4), Clef::Tenor);
}
#[test]
fn musicxml_rejects_mid_measure_clef_changes_instead_of_moving_them() {
    let xml = br#"<score-partwise><part-list><score-part id="P1"><part-name>Viola</part-name></score-part></part-list><part id="P1"><measure><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note><attributes><clef><sign>G</sign><line>2</line></clef></attributes></measure></part></score-partwise>"#;
    assert!(
        musicxml::decode(xml)
            .unwrap_err()
            .to_string()
            .contains("dentro del compás")
    );
}
