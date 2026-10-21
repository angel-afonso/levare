use super::*;
use score_core::{
    BarMark, BarMarkRef, Connection, ConnectionKind, Duration, Dynamic, Ending, Event, MarkRef,
    Offset, Pitch, Pitches, Step,
};

fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Quarter,
        notation: Default::default(),
    }
}

#[test]
fn native_offsets_round_trip_for_point_and_connection_marks() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    let crescendo = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    };
    score.add_connection(crescendo).unwrap();
    score
        .set_mark_offset(MarkRef::Dynamic(a), Offset { dx: 13, dy: -8 })
        .unwrap();
    score
        .set_mark_offset(MarkRef::Connection(crescendo), Offset { dx: 20, dy: 11 })
        .unwrap();
    assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
}

#[test]
fn schema_twenty_three_defaults_offsets_to_zero() {
    let plain = Score::default();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&plain).unwrap()).unwrap();
    old["version"] = 23.into();
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), plain);
}

#[test]
fn invalid_connection_offsets_are_rejected_on_decode() {
    let mut score = Score::default();
    let a = score.insert(0, 0, note(Step::C)).unwrap();
    let b = score.insert(0, 1, note(Step::D)).unwrap();
    let connection = Connection {
        start: a,
        end: b,
        kind: ConnectionKind::Crescendo,
    };
    let mut invalid: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    invalid["score"]["connection_offsets"] = serde_json::json!([{
        "connection": connection,
        "offset": {"dx": 10, "dy": 0}
    }]);
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());

    score.add_connection(connection).unwrap();
    score
        .set_mark_offset(MarkRef::Connection(connection), Offset { dx: 1, dy: 0 })
        .unwrap();
    let mut invalid: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    invalid["score"]["connection_offsets"][0]["offset"]["dx"] = 401.into();
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
}

#[test]
fn zero_offsets_do_not_add_native_keys() {
    let bytes = encode(&Score::default()).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("offsets"));
}

#[test]
fn bar_mark_offsets_round_trip_for_tempo_rehearsal_and_ending() {
    let mut score = Score::default();
    score.toggle_rehearsal_mark(1).unwrap();
    score
        .set_ending(Ending {
            start: 1,
            end: 2,
            numbers: vec![1],
        })
        .unwrap();
    for (mark, offset) in [
        (
            BarMarkRef {
                measure: 0,
                mark: BarMark::Tempo,
            },
            Offset { dx: 9, dy: -12 },
        ),
        (
            BarMarkRef {
                measure: 1,
                mark: BarMark::Rehearsal,
            },
            Offset { dx: -7, dy: 20 },
        ),
        (
            BarMarkRef {
                measure: 1,
                mark: BarMark::Ending,
            },
            Offset { dx: 31, dy: 8 },
        ),
    ] {
        score.set_bar_mark_offset(mark, offset).unwrap();
    }
    let bytes = encode(&score).unwrap();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["version"],
        crate::SCHEMA_VERSION
    );
    assert_eq!(decode(&bytes).unwrap(), score);
}

#[test]
fn schema_twenty_four_defaults_bar_offsets_to_zero() {
    let mut score = Score::default();
    score.toggle_rehearsal_mark(1).unwrap();
    let mut old: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    old["version"] = 24.into();
    assert_eq!(decode(&serde_json::to_vec(&old).unwrap()).unwrap(), score);
}

#[test]
fn orphan_bar_offsets_are_rejected_on_decode() {
    let mut invalid: serde_json::Value =
        serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
    invalid["score"]["bar_offsets"] = serde_json::json!({"1": {"rehearsal": {"dx": 1, "dy": 0}}});
    assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
}
