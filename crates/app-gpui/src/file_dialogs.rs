//! The system's Open, Save As and Export dialogs: where they start, and what
//! the musician reads when one cannot open.
use std::path::{Path, PathBuf};

/// The score's own folder, else Documents, else the home folder. Never the
/// working directory: started from the Windows Start menu, that is the install
/// folder or System32.
pub fn start_folder(score: Option<&Path>) -> PathBuf {
    start_folder_from(score, dirs::document_dir(), dirs::home_dir())
}

fn start_folder_from(
    score: Option<&Path>,
    documents: Option<PathBuf>,
    home: Option<PathBuf>,
) -> PathBuf {
    score
        .and_then(Path::parent)
        .filter(|folder| !folder.as_os_str().is_empty())
        .map(Path::to_owned)
        .or(documents)
        .or(home)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Where Save writes without asking: only a document already saved as
/// `.levare`. An older `.partitura` file or an imported MusicXML is left
/// untouched, and Save asks for a new `.levare` beside it.
pub fn save_in_place(path: Option<&Path>) -> Option<&Path> {
    path.filter(|path| is_levare(path))
}

/// The folder and file name the Save dialog proposes.
pub fn suggested_save(path: Option<&Path>, title: &str) -> (PathBuf, String) {
    let stem = path
        .filter(|path| !is_levare(path))
        .and_then(Path::file_stem)
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| score_io::file_name(title, 80));
    (
        start_folder(path),
        format!("{stem}.{}", score_io::EXTENSION),
    )
}

/// `name.levare` when the chosen name has no extension of its own, as when
/// the musician deletes the suggested one. A dot inside a title («Op. 2»)
/// does not count as an extension.
pub fn with_levare_extension(path: &Path) -> Option<PathBuf> {
    let has_extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            (1..=10).contains(&extension.len())
                && extension.chars().all(|c| c.is_ascii_alphanumeric())
        });
    if has_extension {
        return None;
    }
    let name = path.file_name()?.to_string_lossy();
    let name = name.trim_end_matches('.');
    if name.is_empty() {
        return None;
    }
    Some(path.with_file_name(format!("{name}.{}", score_io::EXTENSION)))
}

fn is_levare(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(score_io::EXTENSION))
}

/// Why a dialog did not open, to follow «No se pudo abrir: » or «No se pudo
/// guardar: ».
///
/// On Linux, GPUI asks the xdg-desktop-portal FileChooser for every dialog. A
/// minimal window manager may run without one, and GPUI then fails with an
/// English message naming the portal.
pub fn dialog_error(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.contains("xdg-desktop-portal") {
        "el selector de archivos no está disponible; instala xdg-desktop-portal-gtk o xdg-desktop-portal-kde".into()
    } else {
        format!("no se pudo abrir el selector de archivos ({message})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogs_start_beside_the_score_then_in_documents_then_home() {
        let documents = Some(PathBuf::from("documentos"));
        let home = Some(PathBuf::from("casa"));
        let score = Path::new("obras").join("Sonata.partitura");
        assert_eq!(
            start_folder_from(Some(&score), documents.clone(), home.clone()),
            Path::new("obras")
        );
        assert_eq!(
            start_folder_from(Some(Path::new("Sonata.partitura")), documents.clone(), None),
            Path::new("documentos")
        );
        assert_eq!(
            start_folder_from(None, documents, home.clone()),
            Path::new("documentos")
        );
        assert_eq!(start_folder_from(None, None, home), Path::new("casa"));
        assert_eq!(start_folder_from(None, None, None), Path::new("."));
    }

    #[test]
    fn save_writes_in_place_only_over_a_levare_document() {
        let levare = Path::new("obras").join("Sonata.levare");
        assert_eq!(save_in_place(Some(&levare)), Some(levare.as_path()));
        assert_eq!(
            suggested_save(Some(&levare), "Sonata en Do"),
            (PathBuf::from("obras"), "Sonata en Do.levare".into())
        );
        let upper = Path::new("obras").join("Sonata.LEVARE");
        assert_eq!(save_in_place(Some(&upper)), Some(upper.as_path()));
        // Older files and imports keep their name, beside the original.
        for original in ["Sonata.partitura", "Sonata.musicxml", "Sonata.mxl"] {
            let path = Path::new("obras").join(original);
            assert_eq!(save_in_place(Some(&path)), None);
            assert_eq!(
                suggested_save(Some(&path), "Otra"),
                (PathBuf::from("obras"), "Sonata.levare".into())
            );
        }
        assert_eq!(save_in_place(None), None);
        assert_eq!(suggested_save(None, "Sin título").1, "Sin título.levare");
    }

    #[test]
    fn a_name_without_extension_becomes_a_levare_file() {
        let folder = Path::new("obras");
        assert_eq!(
            with_levare_extension(&folder.join("Sonata")),
            Some(folder.join("Sonata.levare"))
        );
        assert_eq!(
            with_levare_extension(&folder.join("Sonata Op. 2")),
            Some(folder.join("Sonata Op. 2.levare"))
        );
        assert_eq!(
            with_levare_extension(&folder.join("Sonata.")),
            Some(folder.join("Sonata.levare"))
        );
        for named in ["Sonata.levare", "Sonata.partitura", "Sonata.musicxml"] {
            assert_eq!(with_levare_extension(&folder.join(named)), None);
        }
    }

    #[test]
    fn a_missing_file_chooser_portal_is_explained_in_spanish() {
        // GPUI 0.2.2's FILE_PICKER_PORTAL_MISSING.
        let gpui = "Couldn't open file picker due to missing xdg-desktop-portal implementation.";
        assert_eq!(
            dialog_error(gpui),
            "el selector de archivos no está disponible; instala xdg-desktop-portal-gtk o xdg-desktop-portal-kde"
        );
        assert_eq!(
            dialog_error("D-Bus no responde"),
            "no se pudo abrir el selector de archivos (D-Bus no responde)"
        );
    }
}
