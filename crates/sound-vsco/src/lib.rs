//! Optional sound packs. Loading and preparation happen before opening audio.
//! VSCO uses original SFZ files through sfizz; the bundled SF2 remains a fallback.
#[cfg(test)]
mod installed_tests;
mod manifest;
#[cfg(test)]
mod manifest_tests;
#[cfg(test)]
mod mixer_tests;
mod sfizz;
use sfizz::NativeBank;

use score_platform::{
    AudioError,
    sound_engine::{SoundBank, SoundEngine},
};
fn error(value: impl std::fmt::Display) -> AudioError {
    AudioError::new(value)
}
#[cfg(test)]
#[global_allocator]
static TEST_ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;
use score_core::{Instrument, Score};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct SfzPack {
    pub root: PathBuf,
    pub library: PathBuf,
    spec: manifest::PackSpec,
}

/// Existing VSCO callers keep their source-compatible name.
pub type VscoPack = SfzPack;

impl SfzPack {
    pub fn load(path: &Path) -> Result<Self, AudioError> {
        let spec = manifest::load_manifest(path)?;
        Ok(Self {
            root: spec.root.clone(),
            library: spec.engine.clone(),
            spec,
        })
    }

    /// Portable app bundles and development binaries both search their ancestors.
    pub fn discover() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        executable.ancestors().find_map(|directory| {
            Self::load(&directory.join("soundpacks/vsco-ce/manifest.json")).ok()
        })
    }

    pub fn license_path(&self) -> Option<&Path> {
        self.spec.license.as_deref()
    }

    /// Examine immediate package folders in each soundpacks directory. One bad
    /// manifest is reported but never hides another library.
    pub fn discover_all(extra_dirs: &[PathBuf]) -> (Vec<Self>, Vec<String>) {
        let mut roots = BTreeSet::new();
        if let Ok(executable) = std::env::current_exe() {
            for parent in executable.ancestors() {
                roots.insert(parent.join("soundpacks"));
            }
        }
        roots.extend(extra_dirs.iter().cloned());
        discover_in_roots(roots)
    }
}

fn discover_in_roots(roots: BTreeSet<PathBuf>) -> (Vec<SfzPack>, Vec<String>) {
    let mut manifests = BTreeSet::new();
    let mut errors = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        match std::fs::read_dir(&root) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path().join("manifest.json");
                            if path.is_file() {
                                manifests.insert(path.canonicalize().unwrap_or(path));
                            }
                        }
                        Err(cause) => errors.push(format!("{}: {cause}", root.display())),
                    }
                }
            }
            Err(cause) => errors.push(format!("{}: {cause}", root.display())),
        }
    }
    let (mut packs, mut ids) = (Vec::new(), BTreeSet::new());
    for path in manifests {
        match SfzPack::load(&path) {
            Ok(pack) if ids.insert(pack.id().to_owned()) => packs.push(pack),
            Ok(pack) => errors.push(format!(
                "Id SFZ duplicado {}: {}",
                pack.id(),
                path.display()
            )),
            Err(cause) => errors.push(format!("{}: {cause}", path.display())),
        }
    }
    packs.sort_by(|left, right| left.id().cmp(right.id()));
    errors.sort();
    (packs, errors)
}

/// Priority order is the vector order; the first package with a patch wins.
#[derive(Clone)]
pub struct CombinedPack {
    pub packs: Vec<SfzPack>,
}
impl CombinedPack {
    pub fn new(packs: Vec<SfzPack>) -> Self {
        Self { packs }
    }
}

pub(crate) fn first_pack_with(
    packs: &[SfzPack],
    instrument: Instrument,
    seat: manifest::Seat,
) -> Option<(&SfzPack, &manifest::PatchSpec)> {
    packs.iter().find_map(|pack| {
        manifest::patch_for(&pack.spec, instrument, seat).map(|patch| (pack, patch))
    })
}

/// The seat of every part: its players, and whether an earlier part plays
/// the same instrument family with the same players.
pub(crate) fn seats(score: &Score) -> Vec<(score_core::PartId, manifest::Seat)> {
    let family = |instrument| manifest::fallback_family(instrument).unwrap_or(instrument);
    let mut seen = Vec::new();
    score
        .parts()
        .iter()
        .map(|part| {
            let players = score.players(part.id());
            let key = (family(part.instrument()), players);
            let count = seen.iter().filter(|seen| **seen == key).count();
            seen.push(key);
            (
                part.id(),
                manifest::Seat {
                    players,
                    second: count % 2 == 1,
                },
            )
        })
        .collect()
}

impl SoundBank for CombinedPack {
    fn id(&self) -> &str {
        "sfz-combinadas"
    }
    fn name(&self) -> &str {
        "Bibliotecas combinadas"
    }
    fn description(&self) -> &str {
        "Primer paquete SFZ con cada instrumento según la prioridad elegida; GeneralUser cubre lo restante."
    }
    fn prepare(
        &self,
        score: &Score,
        audition: Instrument,
        rate: u32,
    ) -> Result<Box<dyn SoundEngine>, AudioError> {
        Ok(Box::new(NativeBank::new_many(
            &self.packs,
            score,
            audition,
            rate,
        )?))
    }
}

impl SoundBank for SfzPack {
    fn id(&self) -> &str {
        &self.spec.id
    }
    fn name(&self) -> &str {
        &self.spec.name
    }
    fn description(&self) -> &str {
        if self.spec.id == "vsco-ce" {
            "VSCO: sostenidos, pizzicato y muestras cortas (staccato en vientos/metales, spiccato en cuerdas) cuando están disponibles. GeneralUser cubre los sonidos y registros restantes."
        } else {
            "Paquete SFZ: instrumentos y articulaciones declarados en su manifiesto. GeneralUser cubre los sonidos y registros restantes."
        }
    }
    fn prepare(
        &self,
        score: &Score,
        audition: Instrument,
        rate: u32,
    ) -> Result<Box<dyn SoundEngine>, AudioError> {
        Ok(Box::new(NativeBank::new(self, score, audition, rate)?))
    }
}
