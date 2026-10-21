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
        assert_eq!(
            doc.descendants()
                .find(|node| node.has_tag_name("software"))
                .and_then(|node| node.text()),
            Some("Levare")
        );
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

#[test]
fn header_credits_round_trip_through_musicxml() {
    let mut score = Score::default();
    let credits = score_core::Credits {
        subtitle: "Op. 9 & n.º 2".into(),
        composer: "Frédéric Chopin".into(),
        arranger: "Ana Ruiz".into(),
        lyricist: "Luis Cernuda".into(),
        year: "1832".into(),
        copyright: "© 2026 Ediciones <Partitura>".into(),
    };
    score.set_header("Nocturno", &credits).unwrap();
    let text = xml(&score);
    Document::parse(&text).unwrap();
    let back = decode(text.as_bytes()).unwrap().score;
    assert_eq!(back.title(), "Nocturno");
    assert_eq!(back.credits(), &credits);
}

#[test]
fn credit_texts_fill_missing_creators_on_import() {
    let mut score = Score::default();
    score.set_header("Pieza", &Default::default()).unwrap();
    let text = xml(&score).replace(
        "<part-list>",
        "<credit page=\"1\"><credit-type>composer</credit-type><credit-words>J. S.\n Bach</credit-words></credit><credit page=\"1\"><credit-type>subtitle</credit-type><credit-words>BWV 846</credit-words></credit><part-list>",
    );
    let back = decode(text.as_bytes()).unwrap().score;
    assert_eq!(back.credits().composer, "J. S. Bach");
    assert_eq!(back.credits().subtitle, "BWV 846");
}

#[test]
fn form_round_trips_through_musicxml() {
    let mut score = Score::new("Form".into(), &[Instrument::Violin, Instrument::Flute], 6).unwrap();
    for (bar, form) in [
        (
            0,
            BarForm {
                barline: Barline::Double,
                ..Default::default()
            },
        ),
        (
            1,
            BarForm {
                repeat_start: true,
                ..Default::default()
            },
        ),
        (
            2,
            BarForm {
                repeat_end: Some(3),
                ..Default::default()
            },
        ),
        (
            4,
            BarForm {
                segno: true,
                to_coda: true,
                ..Default::default()
            },
        ),
        (
            5,
            BarForm {
                coda: true,
                jump: Some(Jump::DalSegnoAlCoda),
                ..Default::default()
            },
        ),
    ] {
        score.set_bar_form(bar, form).unwrap();
    }
    for (bar, number) in [(2, 1), (3, 2)] {
        score
            .set_ending(Ending {
                start: bar,
                end: bar,
                numbers: vec![number],
            })
            .unwrap();
    }
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let imported = decode(data.as_bytes()).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    for bar in 0..6 {
        assert_eq!(
            score.bar_form(bar),
            imported.score.bar_form(bar),
            "bar {bar}"
        );
    }
    assert_eq!(
        score.endings().collect::<Vec<_>>(),
        imported.score.endings().collect::<Vec<_>>()
    );
    for (index, part) in doc
        .root_element()
        .children()
        .filter(|node| node.has_tag_name("part"))
        .enumerate()
    {
        let bars: Vec<_> = part
            .children()
            .filter(|node| node.has_tag_name("measure"))
            .collect();
        assert!(
            bars[1]
                .children()
                .find(Node::is_element)
                .unwrap()
                .has_tag_name("barline")
        );
        assert_eq!(
            bars[2]
                .descendants()
                .find(|node| node.has_tag_name("repeat"))
                .unwrap()
                .attribute("times"),
            Some("3")
        );
        assert_eq!(
            part.descendants()
                .filter(|node| node.has_tag_name("segno"))
                .count(),
            usize::from(index == 0)
        );
        assert_eq!(
            part.descendants()
                .filter(|node| node.has_tag_name("coda"))
                .count(),
            usize::from(index == 0)
        );
        let right = bars[2]
            .children()
            .find(|node| {
                node.has_tag_name("barline") && node.attribute("location") == Some("right")
            })
            .unwrap();
        assert_eq!(
            right
                .children()
                .filter(Node::is_element)
                .map(|node| node.tag_name().name())
                .collect::<Vec<_>>(),
            ["bar-style", "ending", "repeat"]
        );
    }
    for direction in doc.descendants().filter(|node| {
        node.has_tag_name("direction")
            && node.descendants().any(|child| {
                child.has_tag_name("sound")
                    && (child.attribute("tocoda").is_some()
                        || child.attribute("dalsegno").is_some())
            })
    }) {
        assert_eq!(text(direction, "offset"), "192");
    }
}

#[test]
fn last_ending_is_written_as_discontinue() {
    let mut score = Score::default();
    for (bar, number) in [(1, 1), (2, 2)] {
        score
            .set_ending(Ending {
                start: bar,
                end: bar,
                numbers: vec![number],
            })
            .unwrap();
    }
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    let endings: Vec<_> = doc
        .descendants()
        .filter(|node| node.has_tag_name("ending") && node.attribute("type") != Some("start"))
        .collect();
    assert_eq!(
        endings
            .iter()
            .filter(|node| node.attribute("number") == Some("1"))
            .map(|node| node.attribute("type").unwrap())
            .collect::<Vec<_>>(),
        ["stop"]
    );
    assert_eq!(
        endings
            .iter()
            .filter(|node| node.attribute("number") == Some("2"))
            .map(|node| node.attribute("type").unwrap())
            .collect::<Vec<_>>(),
        ["discontinue"]
    );
}

#[test]
fn final_repeat_does_not_write_two_barlines() {
    let mut score = Score::default();
    score
        .set_bar_form(
            3,
            BarForm {
                repeat_end: Some(2),
                ..Default::default()
            },
        )
        .unwrap();
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    for part in doc
        .root_element()
        .children()
        .filter(|node| node.has_tag_name("part"))
    {
        let bar = part
            .children()
            .rfind(|node| node.has_tag_name("measure"))
            .unwrap();
        assert_eq!(
            bar.children()
                .filter(|node| node.has_tag_name("barline")
                    && node.attribute("location") == Some("right"))
                .count(),
            1
        );
    }
}

#[test]
fn every_jump_variant_and_explicit_interior_style_round_trips() {
    let mut score = Score::new("Variants".into(), &[Instrument::Violin], 8).unwrap();
    for (bar, jump) in Jump::ALL.into_iter().enumerate() {
        // Independent Fine/To Coda flags must not upgrade a recognized D.C./D.S.
        score
            .set_bar_form(
                bar,
                BarForm {
                    jump: Some(jump),
                    fine: true,
                    to_coda: true,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    score
        .set_bar_form(
            6,
            BarForm {
                barline: Barline::Final,
                ..Default::default()
            },
        )
        .unwrap();
    score
        .set_bar_form(
            7,
            BarForm {
                barline: Barline::Double,
                ..Default::default()
            },
        )
        .unwrap();
    let imported = decode(&encode(&score).unwrap()).unwrap();
    assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
    for bar in 0..8 {
        assert_eq!(
            imported.score.bar_form(bar),
            score.bar_form(bar),
            "bar {bar}"
        );
    }
}

#[test]
fn single_note_tremolo_variants_round_trip_through_musicxml() {
    let mut score = Score::default();
    for (index, mark) in Tremolo::ALL.into_iter().enumerate() {
        let id = score
            .insert(0, index, note(Step::C, 4, Duration::Quarter))
            .unwrap();
        score.set_tremolo(id, Some(mark)).unwrap();
    }
    let data = xml(&score);
    let doc = Document::parse(&data).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|node| node.has_tag_name("tremolo"))
            .map(|node| (node.attribute("type"), node.text()))
            .collect::<Vec<_>>(),
        [
            (Some("single"), Some("1")),
            (Some("single"), Some("2")),
            (Some("single"), Some("3"))
        ]
    );
    let result = decode(data.as_bytes()).unwrap();
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let notes: Vec<_> = result.score.measures()[0]
        .iter()
        .filter_map(|(_, event)| event.notation.tremolo)
        .collect();
    assert_eq!(notes, Tremolo::ALL);
}

#[test]
fn rehearsal_styles_and_custom_labels_round_trip() {
    for style in RehearsalStyle::ALL {
        let mut score = Score::new(
            "Ensayo".into(),
            &[Instrument::Violin, Instrument::Cello],
            10,
        )
        .unwrap();
        for bar in [2, 4, 8] {
            score.toggle_rehearsal_mark(bar).unwrap();
        }
        score.set_rehearsal_style(style);
        let data = xml(&score);
        let doc = Document::parse(&data).unwrap();
        assert_eq!(
            doc.descendants()
                .filter(|n| n.has_tag_name("rehearsal"))
                .count(),
            3
        );
        let imported = decode(data.as_bytes()).unwrap();
        assert!(imported.warnings.is_empty(), "{:?}", imported.warnings);
        assert_eq!(imported.score.rehearsal_style(), style);
        for bar in [2, 4, 8] {
            assert_eq!(
                imported.score.rehearsal_label(bar),
                score.rehearsal_label(bar)
            );
            assert!(
                imported
                    .score
                    .rehearsal_marks()
                    .find(|(b, _)| *b == bar)
                    .unwrap()
                    .1
                    .custom
                    .is_none()
            );
        }
    }
    let mut score = Score::default();
    score
        .set_rehearsal_custom(0, Some("A & B <Trío>".into()))
        .unwrap();
    let imported = decode(&encode(&score).unwrap()).unwrap();
    assert_eq!(imported.score.rehearsal_label(0), score.rehearsal_label(0));
}
