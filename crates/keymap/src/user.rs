//! The user's changes, kept apart from the profile so updates never lose them.

use crate::{CommandId, KeySequence};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt, fs, io,
    path::{Path, PathBuf},
};

pub const FORMAT_VERSION: u32 = 1;

/// `atajos.json`: the chosen profile and, per command, the full list of keys
/// that replaces the profile's. An empty list removes every shortcut.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserKeymap {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub overrides: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub enum UserError {
    Io(PathBuf, io::Error),
    Format(PathBuf, serde_json::Error),
    Version(PathBuf, u32),
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, error) => write!(f, "{}: {error}", path.display()),
            Self::Format(path, error) => write!(f, "{} no es válido: {error}", path.display()),
            Self::Version(path, version) => write!(
                f,
                "{} usa la versión {version}; esta aplicación entiende la {FORMAT_VERSION}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for UserError {}

impl UserKeymap {
    pub fn new() -> Self {
        Self {
            version: FORMAT_VERSION,
            ..Self::default()
        }
    }

    /// A missing file means "no changes".
    pub fn load(path: &Path) -> Result<Self, UserError> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::new()),
            Err(error) => return Err(UserError::Io(path.to_owned(), error)),
        };
        let keymap: Self = serde_json::from_str(&text)
            .map_err(|error| UserError::Format(path.to_owned(), error))?;
        if keymap.version != FORMAT_VERSION {
            return Err(UserError::Version(path.to_owned(), keymap.version));
        }
        Ok(keymap)
    }

    /// Writes a temporary file next to the target and renames it over.
    pub fn save(&self, path: &Path) -> Result<(), UserError> {
        let io_error = |error| UserError::Io(path.to_owned(), error);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_error)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| UserError::Format(path.to_owned(), error))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, text + "\n").map_err(io_error)?;
        fs::rename(&temporary, path).map_err(io_error)
    }

    pub fn is_overridden(&self, command: CommandId) -> bool {
        self.overrides.contains_key(command.as_str())
    }

    pub fn set(&mut self, command: CommandId, keys: &[KeySequence]) {
        self.overrides.insert(
            command.as_str().to_owned(),
            keys.iter().map(KeySequence::text).collect(),
        );
    }

    pub fn reset(&mut self, command: CommandId) {
        self.overrides.remove(command.as_str());
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::id;

    #[test]
    fn saves_atomically_and_reads_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Partitura").join("atajos.json");
        assert_eq!(UserKeymap::load(&path).unwrap(), UserKeymap::new());
        let mut keymap = UserKeymap::new();
        keymap.set(id::STACCATO, &[KeySequence::parse("alt-s").unwrap()]);
        keymap.set(id::SLUR, &[]);
        keymap.save(&path).unwrap();
        assert!(!path.with_extension("json.tmp").exists());
        let loaded = UserKeymap::load(&path).unwrap();
        assert_eq!(loaded, keymap);
        assert_eq!(loaded.overrides["expresion.staccato"], vec!["alt-s"]);
        assert!(loaded.overrides["expresion.ligadura-expresion"].is_empty());
    }

    #[test]
    fn rejects_broken_files_and_future_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("atajos.json");
        fs::write(&path, "{ esto no es json").unwrap();
        assert!(matches!(
            UserKeymap::load(&path),
            Err(UserError::Format(..))
        ));
        fs::write(&path, r#"{"version": 9, "overrides": {}}"#).unwrap();
        assert!(matches!(
            UserKeymap::load(&path),
            Err(UserError::Version(_, 9))
        ));
    }
}
