//! What the side panels remember between sessions (`paneles.json`).

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

/// The tabs of the right panel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RightTab {
    #[default]
    Selection,
    Sound,
    Review,
}

impl RightTab {
    pub const ALL: [Self; 3] = [Self::Selection, Self::Sound, Self::Review];

    pub fn label(self) -> &'static str {
        match self {
            Self::Selection => "Selección",
            Self::Sound => "Sonido",
            Self::Review => "Revisión",
        }
    }
}

/// How the left panel shows the parts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartsMode {
    #[default]
    List,
    Mix,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelMemory {
    pub show_parts: bool,
    pub show_right: bool,
    pub right: RightTab,
    pub parts_mode: PartsMode,
    /// Names of the folded instrument families.
    pub folded: Vec<String>,
}

impl Default for PanelMemory {
    fn default() -> Self {
        Self {
            show_parts: true,
            show_right: true,
            right: RightTab::default(),
            parts_mode: PartsMode::default(),
            folded: Vec::new(),
        }
    }
}

/// A missing or damaged file gives the defaults: panels are never worth an error.
pub fn load(path: &Path) -> PanelMemory {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes a temporary file next to the target and renames it over.
pub fn save(path: &Path, memory: &PanelMemory) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(&serde_json::to_vec_pretty(memory)?)?;
    temporary.persist(path).map_err(|failure| failure.error)?;
    Ok(())
}
