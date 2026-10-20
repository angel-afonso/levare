use super::*;
use roxmltree::{Document, Node};

fn child<'a, 'b>(node: Node<'a, 'b>, tag: &str) -> Option<Node<'a, 'b>> {
    node.children().find(|n| n.has_tag_name(tag))
}
fn text<'a>(node: Node<'a, '_>, tag: &str) -> &'a str {
    child(node, tag).and_then(|n| n.text()).unwrap()
}
fn xml(score: &Score) -> String {
    String::from_utf8(encode(score).unwrap()).unwrap()
}
fn note(step: Step, octave: u8, duration: Duration) -> Event {
    Event {
        pitches: Pitches::single(Pitch::new(step, octave)),
        duration,
        notation: Notation::default(),
    }
}
fn check_timing(doc: &Document<'_>, score: &Score) {
    for part in doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("part"))
    {
        for (bar, measure) in part
            .children()
            .filter(|n| n.has_tag_name("measure"))
            .enumerate()
        {
            let cap = i32::from(score.time_at(bar).capacity());
            let mut cursor = 0;
            let mut last = 0;
            for element in measure.children().filter(Node::is_element) {
                match element.tag_name().name() {
                    "backup" => {
                        cursor -= text(element, "duration").parse::<i32>().unwrap();
                        assert!(cursor >= 0);
                    }
                    "note" => {
                        let duration: i32 = text(element, "duration").parse().unwrap();
                        assert!(duration > 0);
                        if child(element, "chord").is_none() {
                            last = cursor;
                            cursor += duration;
                        }
                        assert!(last + duration <= cap, "note overruns bar {}", bar + 1);
                    }
                    _ => {}
                }
            }
            assert_eq!(cursor, cap, "bar {}", bar + 1);
        }
    }
}

#[test]
fn all_example_scores_produce_parseable_metered_musicxml_without_mutating_documents() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut count = 0;
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("partitura") {
            continue;
        }
        let score = crate::load(&path).unwrap();
        let before = score.clone();
        let data = xml(&score);
        let doc = Document::parse(&data).unwrap();
        assert_eq!(doc.root_element().attribute("version"), Some("4.0"));
        check_timing(&doc, &score);
        assert_eq!(before, score);
        count += 1;
    }
    assert!(count >= 10);
}

#[test]
fn midbar_divisi_splits_a_sustained_second_voice_and_returns_to_the_first_staff() {
    let mut score = Score::default();
    let part = score.add_part(Instrument::Violin).unwrap();
    score
        .insert_in_voice(
            part,
            StaffId::Upper,
            0,
            Voice::Two,
            0,
            note(Step::C, 4, Duration::Whole),
        )
        .unwrap();
    score
        .set_divisi_span(
            part,
            DivisiPoint::new(0, 48),
            Some(DivisiPoint::new(0, 144)),
            true,
        )
        .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    check_timing(&doc, &score);
    let p = doc
        .descendants()
        .find(|n| n.has_tag_name("part") && n.attribute("id") == Some("P2"))
        .unwrap();
    let first = p.children().find(|n| n.has_tag_name("measure")).unwrap();
    let notes: Vec<_> = first
        .children()
        .filter(|n| n.has_tag_name("note") && text(*n, "voice") == "2")
        .collect();
    assert_eq!(
        notes
            .iter()
            .map(|n| (text(*n, "duration"), text(*n, "staff")))
            .collect::<Vec<_>>(),
        vec![("48", "1"), ("96", "2"), ("48", "1")]
    );
    assert_eq!(
        notes[0]
            .descendants()
            .filter(|n| n.has_tag_name("tied") && n.attribute("type") == Some("start"))
            .count(),
        1
    );
    assert_eq!(
        notes[1]
            .descendants()
            .filter(|n| n.has_tag_name("tied"))
            .count(),
        2
    );
    let words: Vec<_> = first
        .descendants()
        .filter(|n| n.has_tag_name("words"))
        .map(|n| {
            (
                n.text().unwrap(),
                text(n.parent().unwrap().parent().unwrap(), "offset"),
            )
        })
        .collect();
    assert!(words.contains(&("div.", "48")));
    assert!(words.contains(&("unis.", "144")));
}

#[test]
fn concert_pitches_explicit_accidentals_triplets_and_empty_bars_are_preserved() {
    let mut score = Score::default();
    let part = score.parts()[0].id();
    score.set_key_signature(KeySignature::new(2, KeyMode::Major).unwrap());
    score
        .insert_in(
            part,
            0,
            0,
            note(Step::F, 4, Duration::Eighth.with_triplet(true)),
        )
        .unwrap();
    let mut natural = note(Step::F, 4, Duration::Quarter.with_dotted(true));
    natural.pitches = natural
        .pitches
        .with_accidental(0, Some(Accidental::Natural))
        .unwrap();
    score.insert_in(part, 0, 1, natural).unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    check_timing(&doc, &score);
    let pitches: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("pitch"))
        .collect();
    assert_eq!(text(pitches[0], "alter"), "1");
    assert_eq!(text(pitches[1], "alter"), "0");
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("time-modification"))
    );
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("rest") && n.attribute("measure") == Some("yes"))
    );
}

#[test]
fn harmonic_roles_and_shared_piano_arpeggios_are_standard_notations() {
    let score = crate::decode(include_bytes!(
        "../../../../examples/armonicos-cuerdas.partitura"
    ))
    .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    for tag in [
        "natural",
        "artificial",
        "base-pitch",
        "touching-pitch",
        "sounding-pitch",
    ] {
        assert!(doc.descendants().any(|n| n.has_tag_name(tag)), "{tag}");
    }
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("notehead") && n.text() == Some("diamond"))
    );
    let score = crate::decode(include_bytes!(
        "../../../../examples/piano-arpegios.partitura"
    ))
    .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    assert!(!score.piano_arpeggios().is_empty());
    assert!(
        doc.descendants()
            .filter(|n| n.has_tag_name("arpeggiate"))
            .count()
            >= score.piano_arpeggios().len() * 2
    );
}

#[test]
fn midbar_dynamics_apply_their_offsets_to_playback_and_hairpins_keep_endpoints() {
    let mut score = Score::default();
    let part = score.parts()[0].id();
    let a = score
        .insert_in(part, 0, 0, note(Step::C, 4, Duration::Half))
        .unwrap();
    let b = score
        .insert_in(part, 0, 1, note(Step::D, 4, Duration::Half))
        .unwrap();
    score.set_dynamic(a, Some(Dynamic::P)).unwrap();
    score.set_dynamic(b, Some(Dynamic::F)).unwrap();
    score
        .add_connection(Connection {
            start: a,
            end: b,
            kind: ConnectionKind::Crescendo,
        })
        .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let later = doc
        .descendants()
        .find(|n| n.has_tag_name("direction") && n.descendants().any(|d| d.has_tag_name("f")))
        .unwrap();
    let offset = child(later, "offset").unwrap();
    assert_eq!(offset.text(), Some("96"));
    assert_eq!(offset.attribute("sound"), Some("yes"));
    assert!(
        child(later, "sound")
            .unwrap()
            .attribute("dynamics")
            .unwrap()
            .parse::<f64>()
            .unwrap()
            > 100.0
    );
    let wedges: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("wedge"))
        .collect();
    assert_eq!(wedges.len(), 2);
    assert_eq!(wedges[0].attribute("type"), Some("crescendo"));
    assert_eq!(wedges[1].attribute("type"), Some("stop"));
    assert_eq!(
        text(wedges[1].parent().unwrap().parent().unwrap(), "offset"),
        "96"
    );
}

#[test]
fn independent_hand_arpeggios_are_not_linked_and_shared_rolls_have_matching_numbers() {
    let mut score = Score::default();
    let part = score.parts()[0].id();
    let a = score
        .insert_on(
            part,
            StaffId::Upper,
            0,
            0,
            note(Step::C, 4, Duration::Whole),
        )
        .unwrap();
    let b = score
        .insert_on(
            part,
            StaffId::Lower,
            0,
            0,
            note(Step::C, 3, Duration::Whole),
        )
        .unwrap();
    score.add_pitch(a, Pitch::new(Step::E, 4).unwrap()).unwrap();
    score.add_pitch(b, Pitch::new(Step::G, 3).unwrap()).unwrap();
    score.set_arpeggio(a, Some(Arpeggio::Up)).unwrap();
    score.set_arpeggio(b, Some(Arpeggio::Down)).unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let rolls: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("arpeggiate"))
        .collect();
    assert_eq!(rolls.len(), 4);
    assert_ne!(rolls[0].attribute("number"), rolls[2].attribute("number"));
    assert_eq!(rolls[0].attribute("direction"), Some("up"));
    assert_eq!(rolls[2].attribute("direction"), Some("down"));
    score.set_arpeggio_both_hands(a, true).unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let rolls: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("arpeggiate"))
        .collect();
    assert_eq!(rolls.len(), 4);
    assert!(
        rolls
            .iter()
            .all(|n| n.attribute("number") == rolls[0].attribute("number"))
    );
}

#[test]
fn menuetto_preserves_conductor_changes_and_expressive_marks() {
    let score = crate::decode(include_bytes!(
        "../../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let part = doc.descendants().find(|n| n.has_tag_name("part")).unwrap();
    let measures: Vec<_> = part
        .children()
        .filter(|n| n.has_tag_name("measure"))
        .collect();
    assert_eq!(measures.len(), 84);
    assert_eq!(
        measures[48]
            .descendants()
            .find(|n| n.has_tag_name("beats"))
            .unwrap()
            .text(),
        Some("6")
    );
    let tempo = measures[48]
        .descendants()
        .find(|n| n.has_tag_name("sound") && n.attribute("tempo").is_some())
        .unwrap();
    assert_eq!(tempo.attribute("tempo"), Some("87")); // Dotted quarter = 58.
    assert!(doc.descendants().any(|n| n.has_tag_name("fermata")));
    assert!(doc.descendants().any(|n| n.has_tag_name("slur")));
    assert!(
        doc.descendants()
            .any(|n| n.has_tag_name("note") && n.attribute("pizzicato") == Some("yes"))
    );
    assert_eq!(
        measures[32]
            .descendants()
            .find(|n| n.has_tag_name("fifths"))
            .unwrap()
            .text(),
        Some("1")
    );
    assert_eq!(
        measures[64]
            .descendants()
            .find(|n| n.has_tag_name("fifths"))
            .unwrap()
            .text(),
        Some("2")
    );
}

#[test]
fn escaped_text_and_atomic_file_writes_are_valid_xml() {
    let mut value: serde_json::Value =
        serde_json::from_slice(&crate::encode(&Score::default()).unwrap()).unwrap();
    value["score"]["title"] = "Trío <A&B> \"cuerdas\"\u{1}".into();
    let score = crate::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    assert_eq!(
        doc.descendants()
            .find(|n| n.has_tag_name("work-title"))
            .unwrap()
            .text(),
        Some("Trío <A&B> \"cuerdas\"�")
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("score.musicxml");
    std::fs::write(&path, "previous").unwrap();
    export(&path, &score).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), data.as_bytes());
    assert!(export(&dir.path().join("missing/score.musicxml"), &score).is_err());
}
