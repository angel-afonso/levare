use super::*;
use score_core::*;
fn note(step: Step) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, 4)),
        duration: Duration::Whole,
        notation: Notation::default(),
    }
}
fn semitones(p: Pitch) -> i16 {
    i16::from(p.octave()) * 12
        + [0, 2, 4, 5, 7, 9, 11][p.step.index() as usize]
        + i16::from(p.alteration())
}
fn sounds(score: &Score) -> Vec<Vec<i16>> {
    score
        .parts()
        .iter()
        .flat_map(|part| {
            part.staves().iter().flat_map(move |staff| {
                staff
                    .measures()
                    .iter()
                    .enumerate()
                    .flat_map(move |(measure, m)| {
                        m.iter()
                            .filter(|(_, e)| !e.pitches.is_empty())
                            .map(move |(event, _)| {
                                score
                                    .sounding_pitches(ElementId {
                                        part: part.id(),
                                        staff: staff.id(),
                                        measure,
                                        event,
                                    })
                                    .unwrap()
                                    .iter()
                                    .map(semitones)
                                    .collect()
                            })
                    })
            })
        })
        .collect()
}
#[test]
fn catalog_survives_native_and_transposed_musicxml_with_sound_and_keys_intact() {
    for instruments in Instrument::ALL.chunks(MAX_PARTS) {
        let mut score = Score::new("Orquesta".into(), instruments, 2).unwrap();
        score
            .set_key_signature_at(0, KeySignature::new(2, KeyMode::Major).unwrap())
            .unwrap();
        score
            .set_key_signature_at(1, KeySignature::new(-1, KeyMode::Minor).unwrap())
            .unwrap();
        let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
        for part in parts {
            for bar in 0..2 {
                score
                    .insert_on(
                        part,
                        StaffId::Upper,
                        bar,
                        0,
                        note(if bar == 0 { Step::F } else { Step::B }),
                    )
                    .unwrap();
            }
        }
        assert_eq!(decode(&encode(&score).unwrap()).unwrap(), score);
        let bytes = musicxml::encode(&score).unwrap();
        let xml = std::str::from_utf8(&bytes).unwrap();
        assert!(!xml.contains("<concert-score"));
        let parsed = roxmltree::Document::parse(xml).unwrap();
        assert!(
            parsed
                .descendants()
                .filter(|n| n.has_tag_name("midi-channel"))
                .all(|n| (1..=16).contains(&n.text().unwrap().parse::<u8>().unwrap()))
        );
        let result = musicxml::decode(&bytes).unwrap();
        assert!(result.warnings.is_empty(), "{:?}", result.warnings);
        assert_eq!(sounds(&result.score), sounds(&score));
        for (before, after) in score.parts().iter().zip(result.score.parts()) {
            assert_eq!(before.instrument(), after.instrument());
            assert_eq!(before.transposition(), after.transposition());
            for bar in 0..2 {
                assert_eq!(
                    score.written_key_at(before.id(), bar),
                    result.score.written_key_at(after.id(), bar)
                );
            }
        }
    }
}
#[test]
fn version_18_preserves_legacy_concert_pitches_and_can_roundtrip_through_musicxml() {
    let mut score = Score::new(
        "Anterior".into(),
        &[Instrument::DoubleBass, Instrument::Clarinet],
        1,
    )
    .unwrap();
    for part in score.parts().iter().map(|p| p.id()).collect::<Vec<_>>() {
        score
            .insert_on(part, StaffId::Upper, 0, 0, note(Step::C))
            .unwrap();
    }
    let mut json: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
    json["version"] = 18.into();
    for part in json["score"]["parts"].as_array_mut().unwrap() {
        part.as_object_mut().unwrap().remove("transposition");
    }
    let old = decode(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(old.parts().iter().all(|p| p.transposition().is_concert()));
    assert_eq!(sounds(&old), vec![vec![48], vec![48]]);
    let imported = musicxml::decode(&musicxml::encode(&old).unwrap()).unwrap();
    assert_eq!(sounds(&old), sounds(&imported.score));
    assert!(
        imported
            .score
            .parts()
            .iter()
            .all(|p| p.transposition().is_concert())
    );
}
#[test]
fn foreign_b_flat_clarinet_keeps_written_c_and_sounds_b_flat() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Clarinet</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><key><fifths>2</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><transpose><diatonic>-1</diatonic><chromatic>-2</chromatic></transpose></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#;
    let score = musicxml::decode(xml.as_bytes()).unwrap().score;
    let part = &score.parts()[0];
    assert_eq!(part.instrument(), Instrument::ClarinetBb);
    assert_eq!(score.key_signature_at(0).fifths(), 0);
    assert_eq!(score.written_key_at(part.id(), 0).fifths(), 2);
    assert_eq!(
        part.staves()[0].measures()[0]
            .iter()
            .next()
            .unwrap()
            .1
            .pitches
            .first()
            .unwrap()
            .step,
        Step::C
    );
    assert_eq!(sounds(&score), vec![vec![46]]);
}
