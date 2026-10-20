use super::*;

#[test]
fn tempo_marks_round_trip_validate_and_legacy_eleven_keeps_quarter_120() {
    use score_core::{TempoBeat, TempoMark};
    let mut score = Score::default();
    let mark = TempoMark {
        bpm: 72,
        text: "Adagio espressivo".into(),
        beat: TempoBeat::Quarter,
    };
    score.set_tempo_at(0, mark).unwrap();
    score
        .set_tempo_at(
            2,
            TempoMark {
                bpm: 108,
                beat: TempoBeat::DottedQuarter,
                text: "Andante".into(),
            },
        )
        .unwrap();
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let mut legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    legacy["version"] = 11.into();
    legacy["score"].as_object_mut().unwrap().remove("tempo");
    legacy["score"]
        .as_object_mut()
        .unwrap()
        .remove("tempo_changes");
    let old = decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert_eq!(old.tempo_at(3), &TempoMark::default());
    for invalid in [
        serde_json::json!({"0":{"bpm":80}}),
        serde_json::json!({"4":{"bpm":80}}),
        serde_json::json!({"2":{"bpm":401}}),
        serde_json::json!({"2":{"bpm":80,"text":"A\nB"}}),
    ] {
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["score"]["tempo_changes"] = invalid;
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}
use score_core::{
    Articulation, Connection, ConnectionKind, Duration, Dynamic, Event, Notation, Pitch, Pitches,
    Step,
};
#[test]
fn expressive_scores_round_trip_and_invalid_endpoints_and_marks_are_rejected() {
    let example = decode(include_bytes!(
        "../../../examples/piano-expresion.partitura"
    ))
    .unwrap();
    assert_eq!(decode(&encode(&example).unwrap()).unwrap(), example);
    let mut s = Score::default();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Half,
        notation: Notation::default(),
    };
    let a = s.insert(0, 0, event).unwrap();
    let b = s.insert(0, 1, event).unwrap();
    s.set_dynamic(a, Some(Dynamic::Pp)).unwrap();
    s.set_articulation(a, Some(Articulation::Staccato)).unwrap();
    s.add_connection(Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Tie,
    })
    .unwrap();
    let data = encode(&s).unwrap();
    assert_eq!(decode(&data).unwrap(), s);
    let original: serde_json::Value = serde_json::from_slice(&data).unwrap();
    for bad in [
        serde_json::json!({"part":99,"staff":"Upper","measure":0,"event":{"idx":1,"version":1}}),
        serde_json::to_value(a).unwrap(),
    ] {
        let mut invalid = original.clone();
        invalid["score"]["connections"][0]["end"] = bad;
        assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let mut duplicate = original.clone();
    duplicate["score"]["connections"]
        .as_array_mut()
        .unwrap()
        .push(original["score"]["connections"][0].clone());
    assert!(decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
    let mut invalid = original;
    invalid["score"]["parts"][0]["staves"][0]["measures"][0]["events"][1]["value"]["notation"]["dynamic"] =
        "fffff".into();
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
}
#[test]
fn schema_six_defaults_to_unmarked_notes_without_changing_identity() {
    let score = decode(include_bytes!(
        "../../../examples/cuarteto-en-sol.partitura"
    ))
    .unwrap();
    assert!(score.connections().is_empty());
    assert!(
        score
            .parts()
            .iter()
            .flat_map(|p| p.staves())
            .flat_map(|s| s.measures())
            .flat_map(|m| m.iter())
            .all(|(_, e)| e.notation == Notation::default())
    );
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
}

#[test]
fn schema_eight_preserves_techniques_and_seven_keeps_original_sound() {
    use score_core::{Instrument, PlayingTechnique as T};
    let mut s = Score::default();
    let violin = s.add_part(Instrument::Violin).unwrap();
    let a = s
        .insert_in(
            violin,
            0,
            0,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Quarter,
                notation: Notation::default(),
            },
        )
        .unwrap();
    s.set_technique(a, Some(T::Pizzicato)).unwrap();
    let data = encode(&s).unwrap();
    assert_eq!(decode(&data).unwrap(), s);
    let mut old: serde_json::Value = serde_json::from_slice(&data).unwrap();
    old["version"] = 7.into();
    let mark =
        &mut old["score"]["parts"][1]["staves"][0]["measures"][0]["events"][1]["value"]["notation"];
    mark.as_object_mut().unwrap().remove("technique");
    assert_eq!(
        decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .technique_at(a),
        Some(T::Arco)
    );
    old["score"]["parts"][1]["instrument"] = "Flute".into();
    old["score"]["parts"][1]["staves"][0]["measures"][0]["events"][1]["value"]["notation"]["technique"] =
        "Pizzicato".into();
    assert!(decode(&serde_json::to_vec(&old).unwrap()).is_err());
    let example = decode(include_bytes!(
        "../../../examples/cuerdas-pizzicato.partitura"
    ))
    .unwrap();
    assert_eq!(decode(&encode(&example).unwrap()).unwrap(), example);
}

#[test]
fn schema_nine_round_trips_regulators_rejects_overlaps_and_reads_eight() {
    let mut s = Score::default();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    };
    let a = s.insert(0, 0, event).unwrap();
    let b = s.insert(0, 1, event).unwrap();
    let c = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    };
    s.add_connection(c).unwrap();
    let data = encode(&s).unwrap();
    assert_eq!(decode(&data).unwrap(), s);
    let mut invalid: serde_json::Value = serde_json::from_slice(&data).unwrap();
    invalid["score"]["connections"]
        .as_array_mut()
        .unwrap()
        .push(
            serde_json::to_value(Connection {
                kind: ConnectionKind::Diminuendo,
                ..c
            })
            .unwrap(),
        );
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
    let old = decode(include_bytes!(
        "../../../examples/cuerdas-pizzicato.partitura"
    ))
    .unwrap();
    assert!(old.connections().iter().all(|c| !c.kind.is_hairpin()));
    let example = decode(include_bytes!("../../../examples/reguladores.partitura")).unwrap();
    assert!(example.connections().iter().any(|c| c.kind.is_hairpin()));
    assert_eq!(decode(&encode(&example).unwrap()).unwrap(), example);
}

#[test]
fn schema_ten_round_trips_rhythms_and_legacy_values_keep_their_musical_meaning() {
    let mut s = Score::default();
    let a = s
        .insert(
            0,
            0,
            Event {
                pitches: Pitches::single(Pitch::new(Step::C, 4)),
                duration: Duration::Quarter.with_dotted(true),
                notation: Notation::default(),
            },
        )
        .unwrap();
    s.insert(
        0,
        1,
        Event {
            pitches: Pitches::default(),
            duration: Duration::Eighth.with_triplet(true),
            notation: Notation::default(),
        },
    )
    .unwrap();
    let bytes = encode(&s).unwrap();
    assert_eq!(decode(&bytes).unwrap(), s);
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["version"], SCHEMA_VERSION);
    for duration in [
        serde_json::json!({"value":"Quarter","dotted":2}),
        serde_json::json!({"value":"Quarter","triplet":"yes"}),
        serde_json::json!({"value":"Quarter","dots":2}),
    ] {
        let mut bad = value.clone();
        bad["score"]["parts"][0]["staves"][0]["measures"][0]["events"][1]["value"]["duration"] =
            duration;
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let old = decode(include_bytes!("../../../examples/reguladores.partitura")).unwrap();
    assert!(
        old.parts()
            .iter()
            .flat_map(|p| p.staves())
            .flat_map(|s| s.measures())
            .flat_map(|m| m.iter())
            .all(|(_, e)| !e.duration.is_dotted() && !e.duration.is_triplet())
    );
    assert_eq!(s.event_tick(a), Some(0));
    let example = decode(include_bytes!("../../../examples/ritmos.partitura")).unwrap();
    assert_eq!(decode(&encode(&example).unwrap()).unwrap(), example);
}

#[test]
fn the_largest_supported_bar_can_save_and_restore_all_384_triplet_events() {
    let mut file: serde_json::Value =
        serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
    file["score"]["time"] = serde_json::json!({"numerator":16,"denominator":1});
    let mut s = decode(&serde_json::to_vec(&file).unwrap()).unwrap();
    for i in 0..score_core::MAX_EVENTS_PER_MEASURE {
        s.insert(
            0,
            i,
            Event {
                pitches: Pitches::default(),
                duration: Duration::Sixteenth.with_triplet(true),
                notation: Default::default(),
            },
        )
        .unwrap();
    }
    assert_eq!(s.measures()[0].len(), 384);
    assert_eq!(s.measures()[0].used_ticks(), s.time().capacity());
    assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
}

#[test]
fn signature_changes_round_trip_and_legacy_ten_keeps_its_global_signatures() {
    use score_core::{KeyMode, KeySignature, TimeSignature};
    let mut score = Score::default();
    score
        .set_time_signature_at(
            1,
            TimeSignature {
                numerator: 3,
                denominator: 4,
            },
        )
        .unwrap();
    score
        .set_time_signature_at(
            3,
            TimeSignature {
                numerator: 6,
                denominator: 8,
            },
        )
        .unwrap();
    score
        .set_key_signature_at(2, KeySignature::new(-3, KeyMode::Major).unwrap())
        .unwrap();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
    let mut value: serde_json::Value =
        serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
    value["version"] = 10.into();
    assert_eq!(
        decode(&serde_json::to_vec(&value).unwrap()).unwrap(),
        Score::default()
    );
    for changes in [
        serde_json::json!({"0":{"numerator":3,"denominator":4}}),
        serde_json::json!({"4":{"numerator":3,"denominator":4}}),
        serde_json::json!({"1":{"numerator":3,"denominator":0}}),
    ] {
        let mut bad = value.clone();
        bad["version"] = SCHEMA_VERSION.into();
        bad["score"]["time_changes"] = changes;
        assert!(decode(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
}

#[test]
fn arpeggios_round_trip_and_schema_twelve_defaults_to_simultaneous_chords() {
    use score_core::Arpeggio;
    let mut score = Score::default();
    let mut event = Event {
        pitches: Pitches::new(&[
            Pitch::new(Step::C, 4).unwrap(),
            Pitch::new(Step::G, 4).unwrap(),
        ])
        .unwrap(),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    for (index, mark) in Arpeggio::ALL.into_iter().enumerate() {
        event.notation.arpeggio = Some(mark);
        score.insert(0, index, event).unwrap();
    }
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let mut legacy: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    legacy["version"] = 12.into();
    let events = legacy["score"]["parts"][0]["staves"][0]["measures"][0]["events"]
        .as_array_mut()
        .unwrap();
    for entry in events.iter_mut() {
        if let Some(notation) = entry
            .get_mut("value")
            .and_then(|event| event.get_mut("notation"))
            .and_then(|notation| notation.as_object_mut())
        {
            notation.remove("arpeggio");
        }
    }
    let old = decode(&serde_json::to_vec(&legacy).unwrap()).unwrap();
    assert!(
        old.measures()[0]
            .iter()
            .all(|(_, event)| event.notation.arpeggio.is_none())
    );
    let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    invalid["score"]["parts"][0]["staves"][0]["measures"][0]["events"][1]["value"]["pitches"] =
        serde_json::to_value(Pitches::single(Pitch::new(Step::C, 4))).unwrap();
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
}

#[test]
fn piano_arpeggios_round_trip_and_reject_dangling_duplicated_or_wrong_staff_members() {
    let mut score = Score::default();
    let piano = score.parts()[0].id();
    let event = Event {
        pitches: Pitches::single(Pitch::new(Step::C, 4)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    let upper = score.insert(0, 0, event).unwrap();
    let lower = score
        .insert_on(piano, score_core::StaffId::Lower, 0, 0, event)
        .unwrap();
    score.set_arpeggio_both_hands(lower, true).unwrap();
    let bytes = encode(&score).unwrap();
    assert_eq!(decode(&bytes).unwrap(), score);
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut old = original.clone();
    old["version"] = 13.into();
    old["score"]
        .as_object_mut()
        .unwrap()
        .remove("piano_arpeggios");
    assert!(
        decode(&serde_json::to_vec(&old).unwrap())
            .unwrap()
            .piano_arpeggios()
            .is_empty()
    );
    for invalid in [
        serde_json::to_value(upper).unwrap(),
        serde_json::json!({"part":99,"staff":"Lower","measure":0,"event":{"idx":1,"version":1}}),
    ] {
        let mut value = original.clone();
        value["score"]["piano_arpeggios"][0]["lower"] = invalid;
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    let mut value = original.clone();
    value["score"]["piano_arpeggios"]
        .as_array_mut()
        .unwrap()
        .push(original["score"]["piano_arpeggios"][0].clone());
    assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
}
