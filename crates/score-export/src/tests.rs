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
    assert!(export(&score, &all, &dir.path().join("missing/delivery")).is_err());
}

#[cfg(target_os = "macos")]
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
    let long = folder_name(&"🎻".repeat(200));
    assert!(long.len() < 150);
    assert!(!Path::new(&long).is_absolute());
    assert_eq!(Path::new(&long).components().count(), 1);
}
