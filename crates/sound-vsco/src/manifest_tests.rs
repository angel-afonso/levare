use super::manifest::{DynamicsMode, load_manifest, patch_for, vsco_instruments};
use score_core::Instrument;
use std::collections::BTreeSet;
use std::{fs, path::Path};

fn write(path: &Path, value: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, value).unwrap();
}

fn format_two(root: &Path) -> std::path::PathBuf {
    write(&root.join("engine.dylib"), "");
    write(&root.join("LICENSE"), "test license");
    write(
        &root.join("samples/violin.sfz"),
        "<region> key=60 sample=sample.wav\n",
    );
    let manifest = root.join("manifest.json");
    write(
        &manifest,
        r#"{"format":2,"kind":"sfz","id":"test","name":"Test SFZ","samples":"samples","engine":"engine.dylib","license":"LICENSE","instruments":{"Violin":{"sustain":"violin.sfz","dynamics":"cc1"}}}"#,
    );
    manifest
}

#[test]
fn format_two_manifest_loads_paths_and_cc1() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    let pack = load_manifest(&path).unwrap();
    assert_eq!(pack.id, "test");
    assert_eq!(pack.name, "Test SFZ");
    assert_eq!(pack.engine, dir.path().join("engine.dylib"));
    assert_eq!(pack.license, Some(dir.path().join("LICENSE")));
    let patch = patch_for(&pack, Instrument::Violin).unwrap();
    assert_eq!(patch.dynamics, DynamicsMode::Cc1);
    assert!(!patch.spread_layers);
}

#[test]
fn missing_sustain_names_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    fs::remove_file(dir.path().join("samples/violin.sfz")).unwrap();
    assert!(
        load_manifest(&path)
            .unwrap_err()
            .to_string()
            .contains("violin.sfz")
    );
}

#[test]
fn unknown_format_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"format\":2", "\"format\":9");
    write(&path, &text);
    assert!(load_manifest(&path).is_err());
}

#[test]
fn legacy_violin_keeps_all_three_original_patches() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("engine.dylib"), "");
    write(&dir.path().join("samples/LICENSE"), "test license");
    for (_, patch) in vsco_instruments() {
        for file in std::iter::once(&patch.sustain).chain(patch.pizzicato.as_ref()) {
            write(
                &dir.path().join("samples").join(file),
                "<region> key=60 sample=sample.wav\n",
            );
        }
    }
    write(
        &dir.path().join("samples/SViolinSpic.sfz"),
        "<region> key=60 sample=sample.wav\n",
    );
    let path = dir.path().join("manifest.json");
    write(
        &path,
        r#"{"format":1,"kind":"vsco-ce","samples":"samples","engine":"engine.dylib"}"#,
    );
    let pack = load_manifest(&path).unwrap();
    let violin = patch_for(&pack, Instrument::Violin).unwrap();
    assert_eq!(violin.sustain, Path::new("SViolinVib.sfz"));
    assert_eq!(
        violin.pizzicato.as_deref(),
        Some(Path::new("SViolinPizz.sfz"))
    );
    assert_eq!(
        violin.staccato.as_deref(),
        Some(Path::new("SViolinSpic.sfz"))
    );
    assert!(violin.spread_layers);
}

#[test]
fn clarinet_alias_uses_the_family_patch() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("Violin", "Clarinet");
    write(&path, &text);
    let pack = load_manifest(&path).unwrap();
    assert!(patch_for(&pack, Instrument::ClarinetBb).is_some());
}

#[test]
fn unsupported_guitar_has_no_patch() {
    let dir = tempfile::tempdir().unwrap();
    let pack = load_manifest(&format_two(dir.path())).unwrap();
    assert!(patch_for(&pack, Instrument::Guitar).is_none());
}

fn package(dir: &Path, folder: &str, id: &str, instrument: &str) {
    let root = dir.join(folder);
    let path = format_two(&root);
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"id\":\"test\"", &format!("\"id\":\"{id}\""))
        .replace("\"Violin\"", &format!("\"{instrument}\""));
    write(&path, &text);
}

#[test]
fn discovery_finds_two_distinct_packages() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    package(dir.path(), "b", "b", "Flute");
    let (packs, errors) = crate::discover_in_roots(BTreeSet::from([dir.path().to_path_buf()]));
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(
        packs
            .iter()
            .map(|pack| pack.spec.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
}

#[test]
fn one_broken_manifest_does_not_hide_a_good_package() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    package(dir.path(), "b", "b", "Flute");
    fs::remove_file(dir.path().join("b/samples/violin.sfz")).unwrap();
    let (packs, errors) = crate::discover_in_roots(BTreeSet::from([dir.path().to_path_buf()]));
    assert_eq!(packs.len(), 1);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("violin.sfz"));
}

#[test]
fn combined_priority_selects_the_first_package_with_a_patch() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    package(dir.path(), "b", "b", "Violin");
    let (mut packs, errors) = crate::discover_in_roots(BTreeSet::from([dir.path().to_path_buf()]));
    assert!(errors.is_empty());
    assert_eq!(
        crate::first_pack_with(&packs, Instrument::Violin)
            .unwrap()
            .0
            .spec
            .id,
        "a"
    );
    packs.reverse();
    assert_eq!(
        crate::first_pack_with(&packs, Instrument::Violin)
            .unwrap()
            .0
            .spec
            .id,
        "b"
    );
    assert!(crate::first_pack_with(&packs, Instrument::Guitar).is_none());
}
