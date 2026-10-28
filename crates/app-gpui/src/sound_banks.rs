//! Composition root: discover optional SFZ packs and their saved priority.
use score_platform::sound_engine::SoundBank;
use std::sync::Arc;

pub struct Catalog {
    pub banks: Vec<Arc<dyn SoundBank>>,
    pub errors: Vec<String>,
    pub priority: Vec<String>,
    #[cfg(feature = "vsco")]
    pub packs: Vec<score_sound_vsco::SfzPack>,
}

pub fn discover() -> Catalog {
    #[cfg(feature = "vsco")]
    {
        let extra: Vec<_> = crate::data_path("soundpacks").into_iter().collect();
        let (packs, mut errors) = score_sound_vsco::SfzPack::discover_all(&extra);
        let saved = match read_priority() {
            Ok(priority) => priority,
            Err(cause) => {
                errors.push(cause);
                Vec::new()
            }
        };
        let priority = ordered_ids(&packs, &saved);
        let banks = banks_from_packs(&packs, &priority);
        Catalog {
            banks,
            errors,
            priority,
            packs,
        }
    }
    #[cfg(not(feature = "vsco"))]
    {
        Catalog {
            banks: Vec::new(),
            errors: Vec::new(),
            priority: Vec::new(),
        }
    }
}

#[cfg(feature = "vsco")]
fn ordered_ids(packs: &[score_sound_vsco::SfzPack], saved: &[String]) -> Vec<String> {
    let mut ids: Vec<_> = packs.iter().map(|pack| pack.id().to_owned()).collect();
    ids.sort_by(|left, right| {
        saved
            .iter()
            .position(|id| id == left)
            .unwrap_or(usize::MAX)
            .cmp(
                &saved
                    .iter()
                    .position(|id| id == right)
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| left.cmp(right))
    });
    ids
}

#[cfg(feature = "vsco")]
pub fn banks_from_packs(
    packs: &[score_sound_vsco::SfzPack],
    priority: &[String],
) -> Vec<Arc<dyn SoundBank>> {
    let mut banks: Vec<Arc<dyn SoundBank>> = packs
        .iter()
        .cloned()
        .map(|pack| Arc::new(pack) as Arc<dyn SoundBank>)
        .collect();
    if packs.len() >= 2 {
        let ordered: Vec<_> = priority
            .iter()
            .filter_map(|id| packs.iter().find(|pack| pack.id() == id).cloned())
            .collect();
        banks.push(Arc::new(score_sound_vsco::CombinedPack::new(ordered)));
    }
    banks
}

#[cfg(feature = "vsco")]
pub fn index_by_id(banks: &[Arc<dyn SoundBank>], id: &str) -> Option<usize> {
    banks.iter().position(|bank| bank.id() == id)
}

#[cfg(feature = "vsco")]
fn read_priority() -> Result<Vec<String>, String> {
    let Some(path) = crate::data_path("sonidos.json") else {
        return Ok(Vec::new());
    };
    read_priority_at(&path)
}

#[cfg(feature = "vsco")]
fn read_priority_at(path: &std::path::Path) -> Result<Vec<String>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(cause) => return Err(format!("{}: {cause}", path.display())),
    };
    let data: serde_json::Value =
        serde_json::from_str(&text).map_err(|cause| format!("{}: {cause}", path.display()))?;
    if data["version"].as_u64() != Some(1) {
        return Err(format!(
            "{}: versión de prioridad no compatible",
            path.display()
        ));
    }
    let entries = data["priority"]
        .as_array()
        .ok_or_else(|| format!("{}: falta la lista priority", path.display()))?;
    let mut ids = Vec::new();
    for entry in entries {
        let id = entry
            .as_str()
            .ok_or_else(|| format!("{}: priority contiene un id no válido", path.display()))?;
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

#[cfg(feature = "vsco")]
pub fn save_priority(priority: &[String]) -> Result<(), String> {
    let path =
        crate::data_path("sonidos.json").ok_or("No hay carpeta de datos para sonidos.json")?;
    save_priority_at(&path, priority)
}

#[cfg(feature = "vsco")]
fn save_priority_at(path: &std::path::Path, priority: &[String]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|cause| format!("{}: {cause}", path.display()))?;
    }
    let data = serde_json::json!({"version":1,"priority":priority});
    let text = serde_json::to_string_pretty(&data).map_err(|cause| cause.to_string())? + "\n";
    atomic_write::write(path, text.as_bytes())
        .map_err(|cause| format!("{}: {cause}", path.display()))
}

#[cfg(all(test, feature = "vsco"))]
mod tests {
    use super::*;
    use std::{
        fs,
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "levare-sfz-priority-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }
    fn pack(root: &Path, id: &str) -> score_sound_vsco::SfzPack {
        let dir = root.join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("engine.dylib"), "").unwrap();
        fs::write(dir.join("violin.sfz"), "<region> key=60 sample=a.wav").unwrap();
        fs::write(dir.join("manifest.json"), format!(r#"{{"format":2,"kind":"sfz","id":"{id}","name":"{id}","samples":".","engine":"engine.dylib","instruments":{{"Violin":{{"sustain":"violin.sfz"}}}}}}"#)).unwrap();
        score_sound_vsco::SfzPack::load_with_engine(
            &dir.join("manifest.json"),
            &dir.join("engine.dylib"),
        )
        .unwrap()
    }

    #[test]
    fn priority_file_defaults_and_round_trips_version_one() {
        let root = fixture();
        let path = root.join("config/sonidos.json");
        assert!(read_priority_at(&path).unwrap().is_empty());
        let ids = vec!["vpo".into(), "vsco-ce".into()];
        save_priority_at(&path, &ids).unwrap();
        assert_eq!(read_priority_at(&path).unwrap(), ids);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// Dotfile managers keep `sonidos.json` as a link into their own folder.
    #[cfg(unix)]
    #[test]
    fn saving_the_priority_through_a_symbolic_link_keeps_the_link() {
        let root = fixture();
        let real = root.join("dotfiles/sonidos.json");
        save_priority_at(&real, &["vpo".into()]).unwrap();
        let path = root.join("sonidos.json");
        std::os::unix::fs::symlink(&real, &path).unwrap();
        save_priority_at(&path, &["vsco-ce".into()]).unwrap();
        assert!(fs::symlink_metadata(&path).unwrap().is_symlink());
        assert_eq!(read_priority_at(&real).unwrap(), vec!["vsco-ce".to_owned()]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_or_future_priority_file_reports_an_error() {
        let root = fixture();
        let path = root.join("sonidos.json");
        fs::write(&path, "{bad").unwrap();
        assert!(
            read_priority_at(&path)
                .unwrap_err()
                .contains("sonidos.json")
        );
        fs::write(&path, r#"{"version":2,"priority":[]}"#).unwrap();
        assert!(read_priority_at(&path).unwrap_err().contains("versión"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saved_priority_precedes_unlisted_ids_and_selection_survives_rebuild() {
        let root = fixture();
        let packs = vec![pack(&root, "a"), pack(&root, "b"), pack(&root, "c")];
        let priority = ordered_ids(&packs, &["c".into()]);
        assert_eq!(priority, ["c", "a", "b"]);
        let initial = banks_from_packs(&packs, &priority);
        let selected = initial[index_by_id(&initial, "sfz-combinadas").unwrap()]
            .id()
            .to_owned();
        let changed = banks_from_packs(&packs, &["b".into(), "a".into(), "c".into()]);
        assert_eq!(
            changed[index_by_id(&changed, &selected).unwrap()].id(),
            "sfz-combinadas"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
