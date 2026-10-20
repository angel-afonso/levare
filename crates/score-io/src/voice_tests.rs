use super::*;
use score_core::{Duration, Event, Instrument, Pitch, Pitches, StaffId, Step, Voice};
#[test]
fn both_voices_and_divided_parts_round_trip_and_invalid_voice_indices_are_rejected() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    let note = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Whole,
        notation: Default::default(),
    };
    s.insert_in(p, 0, 0, note).unwrap();
    s.insert_in_voice(p, StaffId::Upper, 0, Voice::Two, 0, note)
        .unwrap();
    assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
    let mut invalid: serde_json::Value = serde_json::from_slice(&encode(&s).unwrap()).unwrap();
    let bar = &mut invalid["score"]["parts"][1]["staves"][0]["measures"][0];
    bar["second_order"] = bar["order"].clone();
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    s.set_part_divisi(p, true).unwrap();
    assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recover.partitura");
    let recovery = Recovery::open(&path).unwrap();
    recovery.update(Some(RecoveredScore {
        score: s.clone(),
        original_path: None,
    }));
    recovery.flush().unwrap();
    assert_eq!(load_recovery(&path).unwrap().unwrap().score, s);
}

#[test]
fn passage_changes_round_trip_recover_and_follow_bar_edits() {
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    for _ in 0..5 {
        s.append_measure().unwrap();
    }
    s.set_passage_divisi(p, 1, Some(3), true).unwrap();
    assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
    let mut invalid: serde_json::Value = serde_json::from_slice(&encode(&s).unwrap()).unwrap();
    invalid["score"]["parts"][1]["divisi_changes"]["99"] = true.into();
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    let mut malformed = invalid.clone();
    malformed["score"]["parts"][0]["staves"] = serde_json::json!([]);
    malformed["score"]["parts"][0]["divisi_changes"] = serde_json::json!({"0": true});
    assert!(decode(&serde_json::to_vec(&malformed).unwrap()).is_err());
    s.insert_measure(2).unwrap();
    assert!(s.part(p).unwrap().divisi_at(4));
    assert!(!s.part(p).unwrap().divisi_at(5));
    s.remove_measures(0, 3).unwrap();
    assert!(s.part(p).unwrap().divisi_at(0));
    assert!(!s.part(p).unwrap().divisi_at(2));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recover.partitura");
    let recovery = Recovery::open(&path).unwrap();
    recovery.update(Some(RecoveredScore {
        score: s.clone(),
        original_path: None,
    }));
    recovery.flush().unwrap();
    assert_eq!(load_recovery(&path).unwrap().unwrap().score, s);
    s.remove_measures(0, s.measures().len()).unwrap();
    assert!(!s.part(p).unwrap().divisi_at(0));
    s.validate().unwrap();
}

#[test]
fn midbar_boundaries_migrate_bar_maps_round_trip_recover_and_reject_bad_orders() {
    use score_core::DivisiPoint;
    let legacy = include_bytes!("../../../examples/divisi-por-pasaje.partitura");
    let migrated = decode(legacy).unwrap();
    assert!(migrated.parts()[0].divisi_at(1));
    assert!(!migrated.parts()[0].divisi_at(3));
    let mut s = Score::default();
    let p = s.add_part(Instrument::Violin).unwrap();
    let note = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Whole,
        notation: Default::default(),
    };
    s.insert_in_voice(p, StaffId::Upper, 0, Voice::Two, 0, note)
        .unwrap();
    s.set_divisi_span(
        p,
        DivisiPoint::new(0, 48),
        Some(DivisiPoint::new(0, 144)),
        true,
    )
    .unwrap();
    assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
    let mut malformed: serde_json::Value = serde_json::from_slice(&encode(&s).unwrap()).unwrap();
    let bar = &mut malformed["score"]["parts"][1]["staves"][0]["measures"][0];
    bar["second_order"] = serde_json::Value::Array(vec![bar["second_order"][0].clone(); 400]);
    assert!(decode(&serde_json::to_vec(&malformed).unwrap()).is_err());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("recover.partitura");
    let recovery = Recovery::open(&path).unwrap();
    recovery.update(Some(RecoveredScore {
        score: s.clone(),
        original_path: None,
    }));
    recovery.flush().unwrap();
    assert_eq!(load_recovery(&path).unwrap().unwrap().score, s);
    s.insert_measure(0).unwrap();
    assert!(s.part(p).unwrap().divisi_at_tick(1, 48));
    s.remove_measures(0, 1).unwrap();
    assert!(s.part(p).unwrap().divisi_at_tick(0, 48));
    assert!(!s.part(p).unwrap().divisi_at_tick(0, 144));
}
