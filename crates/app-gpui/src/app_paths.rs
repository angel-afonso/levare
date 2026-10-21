//! Shared storage for shortcuts, recent files and recovery across the rename.
use std::path::{Path, PathBuf};

pub fn data_path(name: &str) -> Option<PathBuf> {
    // Keep the previous override as an alias for existing preview/QA scripts.
    ["LEVARE_DATA_DIR", "PARTITURA_DATA_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|path| path.is_absolute())
        .or_else(|| dirs::data_local_dir().map(|base| default_directory(&base)))
        .map(|directory| directory.join(name))
}

fn default_directory(base: &Path) -> PathBuf {
    let current = base.join("Levare");
    let legacy = base.join("Partitura");
    // Reuse existing data in place, including recovery locks, instead of
    // splitting an existing session or overwriting the user's preferences.
    if !current.is_dir() && legacy.is_dir() {
        legacy
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_install_uses_levare_without_creating_a_directory() {
        let base = tempfile::tempdir().unwrap();
        let directory = default_directory(base.path());
        assert_eq!(directory, base.path().join("Levare"));
        assert!(!directory.exists());
    }

    #[test]
    fn existing_shortcuts_recents_and_recovery_stay_in_the_same_directory() {
        let base = tempfile::tempdir().unwrap();
        let legacy = base.path().join("Partitura");
        std::fs::create_dir(&legacy).unwrap();
        for name in ["atajos.json", "recientes.json", "recovery.partitura"] {
            std::fs::write(legacy.join(name), name).unwrap();
            assert_eq!(
                std::fs::read_to_string(default_directory(base.path()).join(name)).unwrap(),
                name
            );
        }
        assert!(!base.path().join("Levare").exists());
    }

    #[test]
    fn existing_levare_directory_takes_precedence_over_legacy_data() {
        let base = tempfile::tempdir().unwrap();
        std::fs::create_dir(base.path().join("Partitura")).unwrap();
        let current = base.path().join("Levare");
        std::fs::create_dir(&current).unwrap();
        assert_eq!(default_directory(base.path()), current);
    }
}
