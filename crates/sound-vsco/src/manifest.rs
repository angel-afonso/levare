//! Validated SFZ package descriptions; legacy VSCO manifests are converted here.
use crate::{AudioError, error};
use score_core::{Instrument, Players};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicsMode {
    #[default]
    Velocity,
    Cc1,
}

#[derive(Clone, Debug)]
pub struct PatchSpec {
    pub sustain: PathBuf,
    pub pizzicato: Option<PathBuf>,
    pub staccato: Option<PathBuf>,
    pub dynamics: DynamicsMode,
    pub spread_layers: bool,
}

/// One set of recordings, and another for a second part of the same
/// instrument, so two violin sections never play identical samples.
#[derive(Clone, Debug)]
pub struct Variant {
    pub first: PatchSpec,
    pub second: Option<PatchSpec>,
}

/// An instrument's recordings: `main` unless the pack has a set for the
/// part's solo or section players.
#[derive(Clone, Debug)]
pub struct InstrumentSpec {
    pub main: Variant,
    pub solo: Option<Variant>,
    pub section: Option<Variant>,
}

impl InstrumentSpec {
    #[cfg(test)]
    pub fn patches(&self) -> impl Iterator<Item = &PatchSpec> {
        [Some(&self.main), self.solo.as_ref(), self.section.as_ref()]
            .into_iter()
            .flatten()
            .flat_map(|variant| std::iter::once(&variant.first).chain(variant.second.as_ref()))
    }

    fn patches_mut(&mut self) -> impl Iterator<Item = &mut PatchSpec> {
        [
            Some(&mut self.main),
            self.solo.as_mut(),
            self.section.as_mut(),
        ]
        .into_iter()
        .flatten()
        .flat_map(|variant| std::iter::once(&mut variant.first).chain(variant.second.as_mut()))
    }
}

impl From<PatchSpec> for InstrumentSpec {
    fn from(first: PatchSpec) -> Self {
        Self {
            main: Variant {
                first,
                second: None,
            },
            solo: None,
            section: None,
        }
    }
}

/// Who plays a part, as a pack needs it to choose recordings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seat {
    pub players: Players,
    /// An earlier part plays the same instrument with the same players.
    pub second: bool,
}

impl Default for Seat {
    fn default() -> Self {
        Self {
            players: Players::Solo,
            second: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PackSpec {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub engine: PathBuf,
    pub license: Option<PathBuf>,
    // Instrument has no Ord; JSON keys are sorted before conversion to this list.
    pub instruments: Vec<(Instrument, InstrumentSpec)>,
}

#[derive(Deserialize)]
struct Manifest {
    format: u32,
    kind: String,
    samples: PathBuf,
    engine: PathBuf,
    id: Option<String>,
    name: Option<String>,
    license: Option<PathBuf>,
    instruments: Option<BTreeMap<String, RawInstrument>>,
}

#[derive(Deserialize)]
struct RawInstrument {
    #[serde(flatten)]
    main: RawVariant,
    solo: Option<RawVariant>,
    section: Option<RawVariant>,
}

#[derive(Deserialize)]
struct RawVariant {
    #[serde(flatten)]
    patch: RawPatch,
    second: Option<RawPatch>,
}

/// `dynamics` and `spread_layers` left out follow the recordings this set
/// varies: the instrument's main set, or the first part's for a second part.
#[derive(Deserialize)]
struct RawPatch {
    sustain: PathBuf,
    pizzicato: Option<PathBuf>,
    staccato: Option<PathBuf>,
    dynamics: Option<DynamicsMode>,
    spread_layers: Option<bool>,
}

impl RawPatch {
    fn spec(self, dynamics: DynamicsMode, spread_layers: bool) -> PatchSpec {
        PatchSpec {
            sustain: self.sustain,
            pizzicato: self.pizzicato,
            staccato: self.staccato,
            dynamics: self.dynamics.unwrap_or(dynamics),
            spread_layers: self.spread_layers.unwrap_or(spread_layers),
        }
    }
}

impl RawInstrument {
    fn spec(self) -> InstrumentSpec {
        let dynamics = self.main.patch.dynamics.unwrap_or_default();
        let spread = self.main.patch.spread_layers.unwrap_or(false);
        let variant = |raw: RawVariant| {
            let first = raw.patch.spec(dynamics, spread);
            let second = raw
                .second
                .map(|patch| patch.spec(first.dynamics, first.spread_layers));
            Variant { first, second }
        };
        InstrumentSpec {
            main: variant(self.main),
            solo: self.solo.map(variant),
            section: self.section.map(variant),
        }
    }
}

pub fn load_manifest(path: &Path) -> Result<PackSpec, AudioError> {
    let data = std::fs::read(path).map_err(error)?;
    let raw: Manifest = serde_json::from_slice(&data).map_err(error)?;
    let parent = path.parent().unwrap_or(Path::new("."));
    let legacy = raw.format == 1 && raw.kind == "vsco-ce";
    if !legacy && (raw.format != 2 || raw.kind != "sfz") {
        return Err(error("Formato de paquete de sonido no compatible"));
    }
    let (id, name, mut instruments) = if legacy {
        (
            "vsco-ce".to_owned(),
            "VSCO CE · experimental".to_owned(),
            vsco_instruments(),
        )
    } else {
        let id = raw.id.ok_or_else(|| error("Falta id del paquete SFZ"))?;
        if id.is_empty()
            || id == "sfz-combinadas"
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(error(format!("Id de paquete SFZ no válido: {id}")));
        }
        let name = raw
            .name
            .filter(|name| !name.trim().is_empty())
            .ok_or_else(|| error("Falta name del paquete SFZ"))?;
        let mut instruments = Vec::new();
        for (key, patch) in raw
            .instruments
            .ok_or_else(|| error("Falta instruments del paquete SFZ"))?
        {
            let instrument: Instrument =
                serde_json::from_value(serde_json::Value::String(key.clone()))
                    .map_err(|_| error(format!("Instrumento SFZ desconocido: {key}")))?;
            instruments.push((instrument, patch.spec()));
        }
        (id, name, instruments)
    };
    let root = parent.join(raw.samples);
    let engine = parent.join(raw.engine);
    if !engine.is_file() {
        return Err(error(format!("Falta motor SFZ: {}", engine.display())));
    }
    let license = if legacy {
        Some(root.join("LICENSE"))
    } else {
        raw.license.map(|path| parent.join(path))
    };
    if let Some(path) = &license
        && !path.is_file()
    {
        return Err(error(format!("Falta licencia SFZ: {}", path.display())));
    }
    for patch in instruments
        .iter_mut()
        .flat_map(|(_, instrument)| instrument.patches_mut())
    {
        for file in std::iter::once(&patch.sustain).chain(patch.pizzicato.iter()) {
            let path = root.join(file);
            if !path.is_file() {
                return Err(error(format!("Falta instrumento SFZ: {}", path.display())));
            }
        }
        if legacy {
            patch.staccato = patch
                .staccato
                .take()
                .filter(|path| root.join(path).is_file());
        } else if let Some(file) = &patch.staccato {
            let path = root.join(file);
            if !path.is_file() {
                return Err(error(format!("Falta instrumento SFZ: {}", path.display())));
            }
        }
    }
    Ok(PackSpec {
        id,
        name,
        root,
        engine,
        license,
        instruments,
    })
}

pub fn fallback_family(instrument: Instrument) -> Option<Instrument> {
    use Instrument::*;
    match instrument {
        ClarinetBb | ClarinetA | ClarinetEb => Some(Clarinet),
        HornF => Some(Horn),
        TrumpetBb | TrumpetD | PiccoloTrumpet | Cornet => Some(Trumpet),
        _ => None,
    }
}

/// The recordings for a part: its players' set when the pack has one, and
/// the second recording for a repeated part when there is one.
pub fn patch_for(spec: &PackSpec, instrument: Instrument, seat: Seat) -> Option<&PatchSpec> {
    let (_, found) = spec
        .instruments
        .iter()
        .find(|(candidate, _)| *candidate == instrument)
        .or_else(|| {
            fallback_family(instrument).and_then(|fallback| {
                spec.instruments
                    .iter()
                    .find(|(candidate, _)| *candidate == fallback)
            })
        })?;
    let variant = match seat.players {
        Players::Solo => found.solo.as_ref(),
        Players::Section => found.section.as_ref(),
    }
    .unwrap_or(&found.main);
    Some(
        variant
            .second
            .as_ref()
            .filter(|_| seat.second)
            .unwrap_or(&variant.first),
    )
}

pub fn vsco_instruments() -> Vec<(Instrument, InstrumentSpec)> {
    use Instrument::*;
    let entries: &[(Instrument, &str, Option<&str>, Option<&str>)] = &[
        (Piano, "UprightPiano.sfz", None, None),
        (
            Violin,
            "SViolinVib.sfz",
            Some("SViolinPizz.sfz"),
            Some("SViolinSpic.sfz"),
        ),
        (
            Viola,
            "ViolaEnsSusVib.sfz",
            Some("ViolaEnsPizz.sfz"),
            Some("ViolaEnsSpic.sfz"),
        ),
        (
            Cello,
            "CelloEnsSusVib.sfz",
            Some("CelloEnsPizz.sfz"),
            Some("CelloEnsSpic.sfz"),
        ),
        (
            DoubleBass,
            "ContrabassSusVB.sfz",
            Some("ContrabassPizz.sfz"),
            Some("ContrabassSpic.sfz"),
        ),
        (Flute, "FluteSusVib.sfz", None, Some("FluteStac.sfz")),
        (Oboe, "OboeSusVib.sfz", None, Some("OboeStac.sfz")),
        (Clarinet, "ClarinetSus.sfz", None, Some("ClarinetStac.sfz")),
        (Bassoon, "BassoonSus.sfz", None, Some("BassoonStac.sfz")),
        (Trumpet, "TrumpetSus.sfz", None, Some("TrumpetStac.sfz")),
        (Horn, "FHornSus.sfz", None, Some("FHornStac.sfz")),
        (Trombone, "TromboneSus.sfz", None, Some("TromboneStac.sfz")),
    ];
    let patch =
        |sustain: &str, pizzicato: Option<&str>, staccato: Option<&str>, layers| PatchSpec {
            sustain: sustain.into(),
            pizzicato: pizzicato.map(Into::into),
            staccato: staccato.map(Into::into),
            dynamics: DynamicsMode::Velocity,
            spread_layers: layers,
        };
    entries
        .iter()
        .map(|&(instrument, sustain, pizzicato, staccato)| {
            let mut spec =
                InstrumentSpec::from(patch(sustain, pizzicato, staccato, instrument != Piano));
            // The main violin is a soloist; VSCO also recorded a violin section.
            if instrument == Violin {
                spec.section = Some(Variant {
                    first: patch(
                        "ViolinEnsSusVib.sfz",
                        Some("ViolinEnsPizz.sfz"),
                        Some("ViolinEnsSpic.sfz"),
                        true,
                    ),
                    second: None,
                });
            }
            (instrument, spec)
        })
        .collect()
}
