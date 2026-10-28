//! Recently opened or saved scores (`recientes.json`), newest first.

use crate::{FileError, error};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentFiles {
    version: u32,
    files: Vec<PathBuf>,
}

impl RecentFiles {
    pub const LIMIT: usize = 10;

    pub fn new() -> Self {
        Self {
            version: FORMAT_VERSION,
            files: Vec::new(),
        }
    }

    /// A missing file means an empty list. Paths that no longer exist are
    /// dropped on load, so the menu never offers a dead entry.
    pub fn load(path: &Path) -> Result<Self, FileError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(failure) if failure.kind() == ErrorKind::NotFound => return Ok(Self::new()),
            Err(failure) => return Err(error(failure)),
        };
        let mut recent: Self = serde_json::from_str(&text).map_err(error)?;
        if recent.version != FORMAT_VERSION {
            return Err(error(format!(
                "la lista de recientes usa la versión {}",
                recent.version
            )));
        }
        recent.files.retain(|file| file.exists());
        recent.files.truncate(Self::LIMIT);
        Ok(recent)
    }

    /// Writes a temporary file next to the target and renames it over.
    pub fn save(&self, path: &Path) -> Result<(), FileError> {
        let parent = path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(error)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(error)?;
        temporary
            .write_all(&serde_json::to_vec_pretty(self).map_err(error)?)
            .map_err(error)?;
        crate::persist(temporary, path).map_err(error)?;
        Ok(())
    }

    /// Moves `file` to the front, without duplicates, keeping at most `LIMIT`.
    pub fn touch(&mut self, file: &Path) {
        // Plain `canonicalize` gives `\\?\C:\…` on Windows; dunce keeps `C:\…`.
        let file = dunce::canonicalize(file).unwrap_or_else(|_| file.to_owned());
        self.files.retain(|existing| *existing != file);
        self.files.insert(0, file);
        self.files.truncate(Self::LIMIT);
    }

    pub fn remove(&mut self, file: &Path) {
        self.files.retain(|existing| existing != file);
    }

    pub fn clear(&mut self) {
        self.files.clear();
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_duplicates_and_limited() {
        let directory = tempfile::tempdir().unwrap();
        let file = |name: &str| {
            let path = directory.path().join(name);
            fs::write(&path, "{}").unwrap();
            dunce::canonicalize(path).unwrap()
        };
        let mut recent = RecentFiles::new();
        let first = file("a.partitura");
        let second = file("b.partitura");
        recent.touch(&first);
        recent.touch(&second);
        recent.touch(&first);
        assert_eq!(recent.files(), [first.clone(), second.clone()]);
        for index in 0..20 {
            recent.touch(&file(&format!("{index}.partitura")));
        }
        assert_eq!(recent.files().len(), RecentFiles::LIMIT);
        assert!(!recent.files().contains(&first));
        recent.remove(&recent.files()[0].clone());
        assert_eq!(recent.files().len(), RecentFiles::LIMIT - 1);
        recent.clear();
        assert!(recent.files().is_empty());
    }

    #[test]
    fn persists_and_forgets_missing_files() {
        let directory = tempfile::tempdir().unwrap();
        let list = directory.path().join("Levare").join("recientes.json");
        assert_eq!(RecentFiles::load(&list).unwrap(), RecentFiles::new());
        let kept = directory.path().join("kept.partitura");
        let gone = directory.path().join("gone.partitura");
        fs::write(&kept, "{}").unwrap();
        fs::write(&gone, "{}").unwrap();
        let mut recent = RecentFiles::new();
        recent.touch(&gone);
        recent.touch(&kept);
        recent.save(&list).unwrap();
        fs::remove_file(&gone).unwrap();
        let loaded = RecentFiles::load(&list).unwrap();
        assert_eq!(loaded.files(), [dunce::canonicalize(&kept).unwrap()]);
        fs::write(&list, r#"{"version": 7, "files": []}"#).unwrap();
        assert!(RecentFiles::load(&list).is_err());
    }
}
