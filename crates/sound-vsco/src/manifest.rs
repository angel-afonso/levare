//! Validated SFZ package descriptions; legacy VSCO manifests are converted here.
use crate::{AudioError, error};
use score_core::Instrument;
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

#[derive(Clone, Debug)]
pub struct PackSpec {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub engine: PathBuf,
    pub license: Option<PathBuf>,
    // Instrument has no Ord; JSON keys are sorted before conversion to this list.
    pub instruments: Vec<(Instrument, PatchSpec)>,
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
    instruments: Option<BTreeMap<String, RawPatch>>,
}

#[derive(Deserialize)]
struct RawPatch {
    sustain: PathBuf,
    pizzicato: Option<PathBuf>,
    staccato: Option<PathBuf>,
    #[serde(default)]
    dynamics: DynamicsMode,
    #[serde(default)]
    spread_layers: bool,
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
            instruments.push((
                instrument,
                PatchSpec {
                    sustain: patch.sustain,
                    pizzicato: patch.pizzicato,
                    staccato: patch.staccato,
                    dynamics: patch.dynamics,
                    spread_layers: patch.spread_layers,
                },
            ));
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
    for (_, patch) in &mut instruments {
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

pub fn patch_for(spec: &PackSpec, instrument: Instrument) -> Option<&PatchSpec> {
    spec.instruments
        .iter()
        .find(|(candidate, _)| *candidate == instrument)
        .or_else(|| {
            fallback_family(instrument).and_then(|fallback| {
                spec.instruments
                    .iter()
                    .find(|(candidate, _)| *candidate == fallback)
            })
        })
        .map(|(_, patch)| patch)
}

pub fn vsco_instruments() -> Vec<(Instrument, PatchSpec)> {
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
    entries
        .iter()
        .map(|&(instrument, sustain, pizzicato, staccato)| {
            (
                instrument,
                PatchSpec {
                    sustain: sustain.into(),
                    pizzicato: pizzicato.map(Into::into),
                    staccato: staccato.map(Into::into),
                    dynamics: DynamicsMode::Velocity,
                    spread_layers: instrument != Piano,
                },
            )
        })
        .collect()
}
