//! Optional sound packs. Loading and preparation happen before opening audio.
//! VSCO uses original SFZ files through sfizz; the bundled SF2 remains a fallback.
#[cfg(test)]
mod installed_tests;
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
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct VscoPack {
    pub root: PathBuf,
    pub library: PathBuf,
}

#[derive(Deserialize)]
struct Manifest {
    format: u32,
    kind: String,
    samples: PathBuf,
    engine: PathBuf,
}

impl VscoPack {
    pub fn load(path: &Path) -> Result<Self, AudioError> {
        let data = std::fs::read(path).map_err(error)?;
        let manifest: Manifest = serde_json::from_slice(&data).map_err(error)?;
        if manifest.format != 1 || manifest.kind != "vsco-ce" {
            return Err(error("Formato de paquete de sonido no compatible"));
        }
        let parent = path.parent().unwrap_or(Path::new("."));
        let pack = Self {
            root: parent.join(manifest.samples),
            library: parent.join(manifest.engine),
        };
        if !pack.library.is_file() || !pack.root.join("LICENSE").is_file() {
            return Err(error(
                "El paquete VSCO no está completo: falta el motor o las muestras",
            ));
        }
        for instrument in Instrument::ALL {
            if let Some(patch) = patches(instrument) {
                for file in std::iter::once(patch.normal).chain(patch.pizz) {
                    if !pack.root.join(file).is_file() {
                        return Err(error(format!("Falta el instrumento VSCO: {file}")));
                    }
                }
            }
        }
        Ok(pack)
    }

    /// Portable app bundles and development binaries both search their ancestors.
    pub fn discover() -> Option<Self> {
        let executable = std::env::current_exe().ok()?;
        executable.ancestors().find_map(|directory| {
            Self::load(&directory.join("soundpacks/vsco-ce/manifest.json")).ok()
        })
    }
}

impl SoundBank for VscoPack {
    fn id(&self) -> &'static str {
        "vsco-ce"
    }
    fn name(&self) -> &'static str {
        "VSCO CE · experimental"
    }
    fn description(&self) -> &'static str {
        "VSCO: sostenidos, pizzicato y muestras cortas (staccato en vientos/metales, spiccato en cuerdas) cuando están disponibles. GeneralUser cubre los sonidos y registros restantes."
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

struct Patches {
    normal: &'static str,
    pizz: Option<&'static str>,
    short: Option<&'static str>,
}
fn patches(instrument: Instrument) -> Option<Patches> {
    use Instrument::*;
    let (normal, pizz, short) = match instrument {
        Piano => ("UprightPiano.sfz", None, None),
        Violin => (
            "SViolinVib.sfz",
            Some("SViolinPizz.sfz"),
            Some("SViolinSpic.sfz"),
        ),
        Viola => (
            "ViolaEnsSusVib.sfz",
            Some("ViolaEnsPizz.sfz"),
            Some("ViolaEnsSpic.sfz"),
        ),
        Cello => (
            "CelloEnsSusVib.sfz",
            Some("CelloEnsPizz.sfz"),
            Some("CelloEnsSpic.sfz"),
        ),
        DoubleBass => (
            "ContrabassSusVB.sfz",
            Some("ContrabassPizz.sfz"),
            Some("ContrabassSpic.sfz"),
        ),
        Flute => ("FluteSusVib.sfz", None, Some("FluteStac.sfz")),
        Oboe => ("OboeSusVib.sfz", None, Some("OboeStac.sfz")),
        Clarinet => ("ClarinetSus.sfz", None, Some("ClarinetStac.sfz")),
        Bassoon => ("BassoonSus.sfz", None, Some("BassoonStac.sfz")),
        Trumpet => ("TrumpetSus.sfz", None, Some("TrumpetStac.sfz")),
        Horn => ("FHornSus.sfz", None, Some("FHornStac.sfz")),
        Trombone => ("TromboneSus.sfz", None, Some("TromboneStac.sfz")),
        Guitar => return None,
    };
    Some(Patches {
        normal,
        pizz,
        short,
    })
}
