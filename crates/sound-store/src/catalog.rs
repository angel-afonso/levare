//! The packs Levare offers, compiled into the app and updated with it.
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Catalog {
    pub packs: Vec<Entry>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: u32,
    pub author: String,
    pub license: License,
    /// Bytes on disk once installed.
    pub installed_size: u64,
    pub archives: Vec<Archive>,
    /// Written by Levare when the archives come from their author without
    /// one. It never names an engine.
    #[serde(default)]
    pub manifest: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct License {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Archive {
    pub url: String,
    pub sha256: String,
    pub size: u64,
    /// Folder inside the zip whose contents are installed.
    #[serde(default)]
    pub strip: String,
    /// Folder of the pack that receives them.
    #[serde(default)]
    pub into: String,
}

impl Catalog {
    pub fn builtin() -> Self {
        serde_json::from_str(include_str!("catalog.json")).expect("valid built-in catalog")
    }

    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.packs.iter().find(|entry| entry.id == id)
    }
}

impl Entry {
    pub fn download_size(&self) -> u64 {
        self.archives.iter().map(|archive| archive.size).sum()
    }
}
