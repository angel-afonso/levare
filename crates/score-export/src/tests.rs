use super::*;

fn menuetto() -> Score {
    score_io::decode(include_bytes!(
        "../../../examples/menuetto-y-trio-cuerdas.partitura"
    ))
    .unwrap()
}

#[test]
fn every_example_extracts_complete_parts_without_changing_the_score() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    for entry in std::fs::read_dir(examples).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("partitura") {
            continue;
        }
        let score = score_io::load(&path).unwrap();
        let before = score.clone();
        for part in score.parts() {
            let extracted = score.extract_part(part.id()).unwrap();
            assert_eq!(extracted.parts().len(), 1);
            assert_eq!(extracted.part_name(part.id()), score.part_name(part.id()));
            let mut expected = serde_json::to_value(part).unwrap();
            let mut actual = serde_json::to_value(&extracted.parts()[0]).unwrap();
            expected.as_object_mut().unwrap().remove("name");
            actual.as_object_mut().unwrap().remove("name");
            assert_eq!(
                actual,
                expected,
                "all notes, voices, staves and divisi: {}",
                path.display()
            );
            assert_eq!(
                extracted.connections().iter().copied().collect::<Vec<_>>(),
                score
                    .connections()
                    .iter()
                    .copied()
                    .filter(|c| c.start.part == part.id())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                extracted
                    .piano_arpeggios()
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                score
                    .piano_arpeggios()
                    .iter()
                    .copied()
                    .filter(|c| c.upper.part == part.id())
                    .collect::<Vec<_>>()
            );
            for bar in 0..score.measures().len() {
                assert_eq!(extracted.time_at(bar), score.time_at(bar));
                assert_eq!(extracted.key_signature_at(bar), score.key_signature_at(bar));
                assert_eq!(extracted.tempo_at(bar), score.tempo_at(bar));
            }
            assert_eq!(
                score_io::decode(&score_io::encode(&extracted).unwrap()).unwrap(),
                extracted
            );
            let xml = score_io::musicxml::encode(&extracted).unwrap();
            let text = std::str::from_utf8(&xml).unwrap();
            let doc = roxmltree::Document::parse(text).unwrap();
            let names: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("part-name"))
                .map(|n| n.text().unwrap())
                .collect();
            assert_eq!(names, [score.part_name(part.id())]);
        }
        assert_eq!(score, before);
        assert!(score.extract_part(PartId(u32::MAX)).is_err());
    }
}

#[test]
fn exports_only_selected_parts_in_score_order_with_distinct_section_names() {
    let score = menuetto();
    let before = score.clone();
    let parts: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("subset");
    let selection = Selection {
        full_score: false,
        parts: vec![parts[2], parts[1]],
        pdf: false,
        musicxml: true,
        concert_pitch: false,
    };
    let files = export(&score, &selection, &destination).unwrap();
    assert_eq!(files.len(), 2);
    assert!(
        files[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Violín II")
    );
    assert!(
        files[1]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Viola")
    );
    for (file, id) in files.iter().zip([parts[1], parts[2]]) {
        let xml = std::fs::read_to_string(file).unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        assert_eq!(
            doc.descendants()
                .filter(|n| n.has_tag_name("score-part"))
                .count(),
            1
        );
        assert_eq!(
            doc.descendants()
                .find(|n| n.has_tag_name("part-name"))
                .unwrap()
                .text()
                .unwrap(),
            score.part_name(id)
        );
        assert_eq!(
            score_io::musicxml::decode(xml.as_bytes())
                .unwrap()
                .score
                .measures()
                .len(),
            score.measures().len()
        );
    }
    assert_eq!(score, before);
}

#[test]
fn validates_selections_and_protects_existing_destinations() {
    let score = menuetto();
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join("delivery");
    let all = Selection::all(&score, true);
    let mut invalid = all.clone();
    invalid.full_score = false;
    invalid.parts.clear();
    assert!(export(&score, &invalid, &destination).is_err());
    invalid = all.clone();
    invalid.musicxml = false;
    assert!(export(&score, &invalid, &destination).is_err());
    invalid = all.clone();
    invalid.parts.push(PartId(u32::MAX));
    assert!(export(&score, &invalid, &destination).is_err());
    invalid = all.clone();
    invalid.parts.push(invalid.parts[0]);
    assert!(export(&score, &invalid, &destination).is_err());
    assert!(!destination.exists());
    let files = export(&score, &all, &destination).unwrap();
    let before: Vec<_> = files.iter().map(|p| std::fs::read(p).unwrap()).collect();
    assert!(export(&score, &all, &destination).is_err());
    for (path, bytes) in files.iter().zip(before) {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    let source = dir.path().join("source.partitura");
    score_io::save(&source, &score).unwrap();
    assert!(export(&score, &all, &source).is_err());
    assert_eq!(score_io::load(&source).unwrap(), score);
    let missing = export(&score, &all, &dir.path().join("missing").join("delivery")).unwrap_err();
    assert_eq!(missing, "el archivo o su carpeta ya no existe");
}

#[test]
fn exports_general_score_and_every_part_in_both_formats() {
    let score = menuetto();
    let mut selection = Selection::all(&score, true);
    selection.pdf = true;
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join(folder_name(score.title()));
    let files = export(&score, &selection, &destination).unwrap();
    assert_eq!(files.len(), (score.parts().len() + 1) * 2);
    for path in files {
        if path.extension().unwrap() == "pdf" {
            let bytes = std::fs::read(path).unwrap();
            assert!(bytes.starts_with(b"%PDF-"));
            assert!(bytes.len() > 1000);
        } else {
            score_io::musicxml::load(&path).unwrap();
        }
    }
}

#[test]
fn generated_folder_names_are_safe_and_bounded() {
    assert_eq!(
        folder_name("../Sonata: violín/cello\\piano\n"),
        "_Sonata_ violín_cello_piano_ - Exportación"
    );
    assert_eq!(folder_name(" .. "), "Partitura - Exportación");
    assert_eq!(folder_name("¿Quién?"), "¿Quién_ - Exportación");
    assert_eq!(folder_name("NUL"), "NUL_ - Exportación");
    assert_eq!(folder_name("Fin. "), "Fin - Exportación");
    let long = folder_name(&"🎻".repeat(200));
    assert!(long.len() < 150);
    assert!(!Path::new(&long).is_absolute());
    assert_eq!(Path::new(&long).components().count(), 1);
}

#[test]
fn a_title_with_characters_windows_forbids_still_exports() {
    let mut data = serde_json::to_value(menuetto()).unwrap();
    data["title"] = "¿Quién? <Dúo> \"1|2\" *".into();
    let score: Score = serde_json::from_value(data).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let destination = dir.path().join(folder_name(score.title()));
    assert_eq!(
        destination.file_name().unwrap(),
        "¿Quién_ _Dúo_ _1_2_ _ - Exportación"
    );
    let files = export(&score, &Selection::all(&score, true), &destination).unwrap();
    assert_eq!(files.len(), score.parts().len() + 1);
}

#[test]
fn the_full_score_follows_the_concert_choice_and_parts_stay_transposed() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/menuetto-y-trio-orquesta.partitura");
    let score = score_io::load(&path).unwrap();
    let before = score.clone();
    let (index, clarinet) = score
        .parts()
        .iter()
        .enumerate()
        .find(|(_, part)| part.instrument() == score_core::Instrument::ClarinetBb)
        .map(|(index, part)| (index, part.id()))
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    // The fifths of a part's first key in an exported file.
    let fifths = |file: &Path, part: usize| -> String {
        let text = std::fs::read_to_string(file).unwrap();
        let part = text.split("<part id=").nth(part + 1).unwrap();
        let start = part.find("<fifths>").unwrap() + "<fifths>".len();
        part[start..start + part[start..].find('<').unwrap()].to_owned()
    };
    for (concert, full) in [(false, "4"), (true, "2")] {
        let destination = dir.path().join(format!("concierto-{concert}"));
        let selection = Selection {
            full_score: true,
            parts: vec![clarinet],
            pdf: false,
            musicxml: true,
            concert_pitch: concert,
        };
        let files = export(&score, &selection, &destination).unwrap();
        assert_eq!(files.len(), 2);
        // Concert D major: the B♭ clarinet's E major, or D in concert pitch.
        assert_eq!(fifths(&files[0], index), full, "concert {concert}");
        assert_eq!(fifths(&files[1], 0), "4", "concert {concert}");
    }
    assert_eq!(score, before);
}

#[test]
fn parts_gather_empty_bars_into_multirests_in_pdf_and_musicxml() {
    use score_core::{Duration, Event, Instrument, Notation, Pitch, Pitches, Step};
    let note = |step, octave| Event {
        pitches: Pitches::single(Pitch::new(step, octave)),
        duration: Duration::Quarter,
        notation: Notation::default(),
    };
    let mut score = Score::new(
        "Tacet".into(),
        &[Instrument::Violin, Instrument::Cello],
        300,
    )
    .unwrap();
    let ids: Vec<_> = score.parts().iter().map(|p| p.id()).collect();
    for bar in 0..300 {
        score.insert_in(ids[0], bar, 0, note(Step::A, 4)).unwrap();
    }
    for bar in [0, 299] {
        score.insert_in(ids[1], bar, 0, note(Step::C, 3)).unwrap();
    }
    // The player must see the rehearsal mark: two runs of 149 bars.
    score.toggle_rehearsal_mark(150).unwrap();
    let mut selection = Selection::all(&score, true);
    selection.pdf = true;
    let dir = tempfile::tempdir().unwrap();
    let files = export(&score, &selection, &dir.path().join("tacet")).unwrap();
    let rests = |name: &str| -> Vec<String> {
        let path = files
            .iter()
            .find(|p| {
                let file = p.file_name().unwrap().to_str().unwrap();
                file.starts_with(name) && file.ends_with(".musicxml")
            })
            .unwrap();
        let xml = std::fs::read_to_string(path).unwrap();
        let doc = roxmltree::Document::parse(&xml).unwrap();
        let measures = doc
            .descendants()
            .find(|n| n.has_tag_name("part"))
            .unwrap()
            .children()
            .filter(|n| n.has_tag_name("measure"))
            .count();
        assert_eq!(measures, 300, "{name}: every bar is still written");
        doc.descendants()
            .filter(|n| n.has_tag_name("multiple-rest"))
            .map(|n| {
                let measure = n.ancestors().find(|a| a.has_tag_name("measure")).unwrap();
                format!(
                    "{}:{}",
                    measure.attribute("number").unwrap(),
                    n.text().unwrap()
                )
            })
            .collect()
    };
    assert_eq!(rests("02"), ["2:149", "151:149"]);
    assert!(rests("01").is_empty(), "the violin plays in every bar");
    assert!(rests("00").is_empty(), "the full score never gathers bars");
    // The printed part holds both runs on one page; bar by bar it takes more.
    let cello = score.extract_part(ids[1]).unwrap();
    let mut separate = cello.clone();
    separate
        .set_page_setup(score_core::PageSetup {
            separate_empty_bars: true,
            ..Default::default()
        })
        .unwrap();
    let gathered = score_pdf::export_part(&cello, &dir.path().join("a.pdf")).unwrap();
    let every = score_pdf::export_part(&separate, &dir.path().join("b.pdf")).unwrap();
    assert_eq!(gathered, 1);
    assert!(every > gathered, "{every} pages bar by bar");
    let xml = String::from_utf8(score_io::musicxml::encode_part(&separate).unwrap()).unwrap();
    assert!(!xml.contains("multiple-rest"));
    // Other programs read the part back with all its bars.
    let back = score_io::musicxml::decode(&score_io::musicxml::encode_part(&cello).unwrap())
        .unwrap()
        .score;
    assert_eq!(back.measures().len(), 300);
}
