//! The system's Open, Save As and Export dialogs: where they start.
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
}
