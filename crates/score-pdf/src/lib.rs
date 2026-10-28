//! Vector PDF adapter. Export uses print layout, independent of screen zoom/view.
//!
//! macOS prints through Quartz with the system's Times New Roman; Windows and Linux
//! use the portable writer with the bundled Liberation Serif. Both draw through the
//! same painter, and the two fonts share their metrics, so the pages match.
mod painter;
#[cfg_attr(target_os = "macos", allow(dead_code))]
mod portable;
#[cfg(target_os = "macos")]
mod quartz;

/// Liberation Serif regular, bold, italic and bold italic: the serif for score text
/// wherever Times New Roman is missing. Same advance widths as Times New Roman.
pub const SERIF_FONTS: [&[u8]; 4] = [
    include_bytes!("../../../assets/fonts/LiberationSerif-Regular.ttf"),
    include_bytes!("../../../assets/fonts/LiberationSerif-Bold.ttf"),
    include_bytes!("../../../assets/fonts/LiberationSerif-Italic.ttf"),
    include_bytes!("../../../assets/fonts/LiberationSerif-BoldItalic.ttf"),
];

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
        portable::export(score, path, part)
    }
}

#[cfg(test)]
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
    #[test]
    fn exports_navigation_and_ending_continuations_in_full_score_and_part() {
        use score_core::{BarForm, Ending, Instrument, Jump, Score};
        let mut score =
            Score::new("Form".into(), &[Instrument::Violin, Instrument::Viola], 80).unwrap();
        for (bar, form) in [
            (
                0,
                BarForm {
                    repeat_start: true,
                    segno: true,
                    ..Default::default()
                },
            ),
            (
                3,
                BarForm {
                    repeat_end: Some(2),
                    ..Default::default()
                },
            ),
            (
                20,
                BarForm {
                    jump: Some(Jump::DalSegnoAlCoda),
                    ..Default::default()
                },
            ),
            (
                32,
                BarForm {
                    to_coda: true,
                    ..Default::default()
                },
            ),
            (
                40,
                BarForm {
                    coda: true,
                    ..Default::default()
                },
            ),
            (
                72,
                BarForm {
                    fine: true,
                    ..Default::default()
                },
            ),
        ] {
            score.set_bar_form(bar, form).unwrap();
        }
        for (start, end, number) in [(3, 3, 1), (4, 14, 2)] {
            score
                .set_ending(Ending {
                    start,
                    end,
                    numbers: vec![number],
                })
                .unwrap();
        }
        let original = score.clone();
        let dir = tempfile::tempdir().unwrap();
        let pages = export(&score, &dir.path().join("form.pdf")).unwrap();
        assert!(pages > 1);
        let part = score.extract_part(score.parts()[0].id()).unwrap();
        assert!(export_part(&part, &dir.path().join("form-part.pdf")).unwrap() > 0);
        assert_eq!(score, original);
    }
}

#[cfg(test)]
mod inspect;

#[cfg(test)]
mod portable_tests {
    use crate::{inspect, painter, portable};
    use score_core::Score;

    fn example(name: &str) -> Score {
        let path = format!(
            "{}/../../examples/{name}.partitura",
            env!("CARGO_MANIFEST_DIR")
        );
        score_io::decode(&std::fs::read(path).unwrap()).unwrap()
    }

    /// Checks pages, paper and the extractable text of one portable PDF.
    fn check(score: &Score, part: bool) -> Vec<inspect::Page> {
        let (pdf, count) = portable::render(score, part).unwrap();
        let pages = inspect::pages(&pdf);
        assert_eq!(pages.len(), count);
        assert_eq!(count, painter::layout(score, part).pages.len());
        let (width, height) = painter::paper(score, part);
        for page in &pages {
            assert!((page.size.0 - width).abs() < 0.01, "{:?}", page.size);
            assert!((page.size.1 - height).abs() < 0.01, "{:?}", page.size);
        }
        for word in score.title().split_whitespace() {
            assert!(pages[0].text.contains(word), "{word} in {}", pages[0].text);
        }
        if part {
            for run in score_layout::text_runs(&score.part_name(score.parts()[0].id())) {
                if let score_layout::TextRun::Text(text) = run {
                    assert!(pages[0].text.contains(text.trim()), "{text}");
                }
            }
        }
        for (index, page) in pages.iter().enumerate() {
            let number = format!("{} / {count}", index + 1);
            assert!(page.text.lines().any(|line| line == number), "{number}");
        }
        pages
    }

    #[test]
    fn prints_full_scores_and_parts_on_their_paper_with_their_text() {
        for name in [
            "piano-expresion",
            "trio",
            "tres-luces-orquesta",
            "voces-y-divisi",
            "orquesta-completa",
        ] {
            let score = example(name);
            check(&score, false);
            for part in score.parts().iter().take(3) {
                check(&score.extract_part(part.id()).unwrap(), true);
            }
        }
        // Twelve parts or more go on A3.
        let orchestra = example("orquesta-completa");
        assert!(orchestra.parts().len() >= 12);
        assert_eq!(painter::paper(&orchestra, false), (841.8898, 1190.5512));
    }

    #[test]
    fn prints_long_scores_on_numbered_pages() {
        let mut score = Score::default();
        for _ in 0..80 {
            score.append_measure().unwrap();
        }
        assert!(check(&score, false).len() > 1);
    }

    #[test]
    fn writes_text_in_every_serif_style() {
        let score = example("tres-luces-orquesta");
        let (pdf, _) = portable::render(&score, false).unwrap();
        // Regular title, bold tempo, italic technique: three embedded subsets.
        let fonts = pdf.windows(b"+LiberationSerif".len());
        let fonts: std::collections::HashSet<_> = fonts
            .enumerate()
            .filter(|(_, w)| *w == b"+LiberationSerif")
            .map(|(at, _)| {
                pdf[at + 16..]
                    .iter()
                    .take_while(|b| b.is_ascii_alphanumeric() || **b == b'-')
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(fonts.contains(b"".as_slice()), "{fonts:?}");
        assert!(fonts.contains(b"-Bold".as_slice()), "{fonts:?}");
        assert!(fonts.contains(b"-Italic".as_slice()), "{fonts:?}");
    }

    /// Quartz reads the portable PDF and finds the pages it wrote itself.
    #[cfg(target_os = "macos")]
    #[test]
    fn quartz_and_portable_print_the_same_pages() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["piano-expresion", "orquesta-completa"] {
            let score = example(name);
            let part = score.extract_part(score.parts()[0].id()).unwrap();
            for (document, is_part) in [(&score, false), (&part, true)] {
                let quartz = dir.path().join("quartz.pdf");
                let portable = dir.path().join("portable.pdf");
                let pages = crate::quartz::export(document, &quartz, is_part).unwrap();
                assert_eq!(
                    portable::export(document, &portable, is_part).unwrap(),
                    pages
                );
                assert_eq!(crate::quartz::page_count(&quartz), Some(pages));
                assert_eq!(crate::quartz::page_count(&portable), Some(pages));
            }
        }
    }
}
