//! Vector PDF adapter. Export uses print layout, independent of screen zoom/view.
#[cfg(target_os = "macos")]
mod quartz;

pub fn export(score: &score_core::Score, path: &std::path::Path) -> Result<usize, String> {
    export_as(score, path, false)
}

/// Print an extracted part with its instrument in the page heading.
pub fn export_part(score: &score_core::Score, path: &std::path::Path) -> Result<usize, String> {
    if score.parts().len() != 1 {
        return Err("La parte individual debe contener un solo instrumento".into());
    }
    export_as(score, path, true)
}

fn export_as(
    score: &score_core::Score,
    path: &std::path::Path,
    part: bool,
) -> Result<usize, String> {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
    {
        return Err("Elige un nombre terminado en .pdf".into());
    }
    score.validate().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        quartz::export(score, path, part)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, part);
        Err("La exportación a PDF requiere macOS".into())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    #[test]
    fn exports_all_pages_and_keeps_the_document_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        for bytes in [
            include_bytes!("../../../examples/piano-arpegios.partitura").as_slice(),
            include_bytes!("../../../examples/cuerdas-pizzicato.partitura").as_slice(),
            include_bytes!("../../../examples/trio.partitura").as_slice(),
            include_bytes!("../../../examples/voces-y-divisi.partitura").as_slice(),
            include_bytes!("../../../examples/divisi-dos-pentagramas.partitura").as_slice(),
            include_bytes!("../../../examples/divisi-por-pasaje.partitura").as_slice(),
            include_bytes!("../../../examples/divisi-dentro-del-compas.partitura").as_slice(),
        ] {
            let score = score_io::decode(bytes).unwrap();
            let before = score.clone();
            let path = dir.path().join("score.pdf");
            let pages = export(&score, &path).unwrap();
            assert!(pages >= 1);
            let data = std::fs::read(&path).unwrap();
            assert!(data.starts_with(b"%PDF-"));
            assert!(data.len() > 1000);
            assert_eq!(score, before);
        }
        let mut score = score_core::Score::default();
        for _ in 0..80 {
            score.append_measure().unwrap();
        }
        assert!(export(&score, &dir.path().join("long.pdf")).unwrap() > 1);
    }
    #[test]
    fn part_export_requires_one_instrument_and_preserves_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("part.pdf");
        std::fs::write(&path, b"previous").unwrap();
        let mut score = score_core::Score::default();
        score.add_part(score_core::Instrument::Violin).unwrap();
        assert!(export_part(&score, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
        let part = score.extract_part(score.parts()[1].id()).unwrap();
        assert!(export_part(&part, &path).unwrap() > 0);
        assert!(std::fs::read(path).unwrap().starts_with(b"%PDF-"));
    }

    #[test]
    fn failed_export_does_not_replace_an_existing_destination() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("previous.pdf");
        std::fs::write(&path, b"previous").unwrap();
        let native = dir.path().join("original.partitura");
        score_io::save(&native, &score_core::Score::default()).unwrap();
        assert!(export(&score_core::Score::default(), &native).is_err());
        assert_eq!(
            score_io::load(&native).unwrap(),
            score_core::Score::default()
        );
        // A malformed native value is rejected before touching the destination.
        let mut invalid = serde_json::to_value(score_core::Score::default()).unwrap();
        invalid["title"] = "x".repeat(2048).into();
        let invalid = serde_json::from_value(invalid).unwrap();
        assert!(export(&invalid, &path).is_err());
        assert!(
            export(
                &score_core::Score::default(),
                &dir.path().join("missing/score.pdf")
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"previous");
    }
}
