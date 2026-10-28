use super::manifest::{
    DynamicsMode, PackSpec, Seat, load_manifest_with, patch_for, vsco_instruments,
};
use crate::AudioError;
use score_core::{Instrument, Players, Score};
use std::collections::BTreeSet;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn write(path: &Path, value: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, value).unwrap();
}

/// Stands in for the sfizz Levare ships.
fn levare_engine(dir: &Path) -> PathBuf {
    let engine = dir.join("levare-sfizz");
    write(&engine, "");
    engine
}

fn load_manifest(path: &Path) -> Result<PackSpec, AudioError> {
    load_manifest_with(path, Some(levare_engine(path.parent().unwrap())))
}

/// A pack that names an engine of its own, `engine.dylib`.
fn format_two(root: &Path) -> PathBuf {
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
    assert_eq!(pack.engine, dir.path().join("levare-sfizz"));
    assert_eq!(pack.license, Some(dir.path().join("LICENSE")));
    let patch = patch_for(&pack, Instrument::Violin, Seat::default()).unwrap();
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
    for patch in vsco_instruments()
        .iter()
        .flat_map(|(_, instrument)| instrument.patches())
    {
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
    let violin = patch_for(&pack, Instrument::Violin, Seat::default()).unwrap();
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
    assert!(patch_for(&pack, Instrument::ClarinetBb, Seat::default()).is_some());
}

#[test]
fn unsupported_guitar_has_no_patch() {
    let dir = tempfile::tempdir().unwrap();
    let pack = load_manifest(&format_two(dir.path())).unwrap();
    assert!(patch_for(&pack, Instrument::Guitar, Seat::default()).is_none());
}

/// The packs in `dir`, played through an engine outside it.
fn discover(dir: &Path) -> (Vec<crate::SfzPack>, Vec<String>) {
    let engine = levare_engine(dir);
    crate::discover_in_roots(BTreeSet::from([dir.to_path_buf()]), Some(engine))
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
    let (packs, errors) = discover(dir.path());
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
    let (packs, errors) = discover(dir.path());
    assert_eq!(packs.len(), 1);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("violin.sfz"));
}

#[test]
fn combined_priority_selects_the_first_package_with_a_patch() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    package(dir.path(), "b", "b", "Violin");
    let (mut packs, errors) = discover(dir.path());
    assert!(errors.is_empty());
    assert_eq!(
        crate::first_pack_with(&packs, Instrument::Violin, Seat::default())
            .unwrap()
            .0
            .spec
            .id,
        "a"
    );
    packs.reverse();
    assert_eq!(
        crate::first_pack_with(&packs, Instrument::Violin, Seat::default())
            .unwrap()
            .0
            .spec
            .id,
        "b"
    );
    assert!(crate::first_pack_with(&packs, Instrument::Guitar, Seat::default()).is_none());
}

#[test]
fn soloists_sections_and_second_parts_get_their_own_recordings() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    for file in ["violin-solo", "violin-2", "violin-solo-2"] {
        write(
            &dir.path().join(format!("samples/{file}.sfz")),
            "<region> key=60 sample=sample.wav\n",
        );
    }
    write(
        &path,
        r#"{"format":2,"kind":"sfz","id":"test","name":"Test SFZ","samples":"samples","engine":"engine.dylib","license":"LICENSE","instruments":{"Violin":{"sustain":"violin.sfz","dynamics":"cc1","second":{"sustain":"violin-2.sfz"},"solo":{"sustain":"violin-solo.sfz","dynamics":"velocity","second":{"sustain":"violin-solo-2.sfz"}}}}}"#,
    );
    let pack = load_manifest(&path).unwrap();
    let seat = |players, second| Seat { players, second };
    let sustain = |seat| {
        let patch = patch_for(&pack, Instrument::Violin, seat).unwrap();
        (patch.sustain.to_str().unwrap().to_owned(), patch.dynamics)
    };
    // Sections have no set of their own here, so they use the main one,
    // whose dynamics a second part inherits.
    assert_eq!(
        sustain(seat(Players::Section, false)),
        ("violin.sfz".into(), DynamicsMode::Cc1)
    );
    assert_eq!(
        sustain(seat(Players::Section, true)),
        ("violin-2.sfz".into(), DynamicsMode::Cc1)
    );
    assert_eq!(
        sustain(seat(Players::Solo, false)),
        ("violin-solo.sfz".into(), DynamicsMode::Velocity)
    );
    assert_eq!(
        sustain(seat(Players::Solo, true)),
        ("violin-solo-2.sfz".into(), DynamicsMode::Velocity)
    );

    fs::remove_file(dir.path().join("samples/violin-solo-2.sfz")).unwrap();
    assert!(
        load_manifest(&path)
            .unwrap_err()
            .to_string()
            .contains("violin-solo-2.sfz")
    );
}

#[test]
fn repeated_parts_alternate_recordings_within_their_players() {
    let score = Score::new(
        "".into(),
        &[
            Instrument::ClarinetBb,
            Instrument::ClarinetA,
            Instrument::Violin,
            Instrument::Violin,
            Instrument::Violin,
            Instrument::Viola,
            Instrument::Cello,
            Instrument::DoubleBass,
        ],
        1,
    )
    .unwrap();
    let seats: Vec<_> = crate::seats(&score)
        .into_iter()
        .map(|(_, seat)| (seat.players, seat.second))
        .collect();
    assert_eq!(
        seats,
        [
            (Players::Solo, false),
            (Players::Solo, true),
            (Players::Section, false),
            (Players::Section, true),
            (Players::Section, false),
            (Players::Section, false),
            (Players::Section, false),
            (Players::Section, false),
        ]
    );
}

#[test]
fn a_manifest_without_engine_uses_the_one_levare_provides() {
    let dir = tempfile::tempdir().unwrap();
    let path = format_two(dir.path());
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace(r#""engine":"engine.dylib","#, "");
    write(&path, &text);
    let engine = dir.path().join("bundled.dylib");
    write(&engine, "");
    let pack = super::manifest::load_manifest_with(&path, Some(engine.clone())).unwrap();
    assert_eq!(pack.engine, engine);
    assert!(
        super::manifest::load_manifest_with(&path, None)
            .unwrap_err()
            .to_string()
            .contains("sfizz")
    );
}

#[test]
fn hidden_folders_in_progress_are_not_packages() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    package(dir.path(), ".instalando-b", "b", "Violin");
    let (packs, errors) = discover(dir.path());
    assert!(errors.is_empty());
    assert_eq!(packs.len(), 1);
}

#[test]
fn sfizz_is_looked_for_next_to_the_executable_under_this_system_s_name() {
    let name = super::manifest::engine_file_name();
    #[cfg(target_os = "windows")]
    assert_eq!(name, "sfizz.dll");
    #[cfg(target_os = "linux")]
    assert_eq!(name, "libsfizz.so");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "libsfizz.dylib");
    let root = Path::new("levare");
    let executable = root.join("target").join("debug").join("levare");
    let candidates = super::manifest::engine_candidates(&executable, true);
    assert_eq!(candidates[0], root.join("target").join("debug").join(&name));
    let mut rest = candidates[1..].iter();
    if cfg!(target_os = "macos") {
        // Levare.app/Contents/MacOS/levare → Contents/Frameworks.
        assert_eq!(
            rest.next(),
            Some(&root.join("target").join("Frameworks").join(&name))
        );
    }
    let vsco = |dir: &Path| dir.join("soundpacks").join("vsco-ce").join(&name);
    assert_eq!(rest.next(), Some(&vsco(&root.join("target").join("debug"))));
    assert_eq!(rest.next(), Some(&vsco(&root.join("target"))));
    assert_eq!(rest.next(), Some(&vsco(root)));
}

#[test]
fn an_installed_app_looks_for_sfizz_only_in_its_own_folder() {
    let name = super::manifest::engine_file_name();
    let app = Path::new("apps").join("Levare");
    let candidates = super::manifest::engine_candidates(&app.join("bin").join("levare"), false);
    let mut expected = vec![app.join("bin").join(&name)];
    if cfg!(target_os = "macos") {
        expected.push(app.join("Frameworks").join(&name));
    }
    assert_eq!(candidates, expected);
}

#[test]
fn a_pack_never_chooses_the_engine_levare_loads() {
    let dir = tempfile::tempdir().unwrap();
    package(dir.path(), "a", "a", "Violin");
    // An absolute path to a library that exists, as a planted pack would name.
    let planted = dir.path().join("a").join("engine.dylib");
    let path = dir.path().join("a").join("manifest.json");
    let text = fs::read_to_string(&path).unwrap().replace(
        r#""engine":"engine.dylib""#,
        &format!(r#""engine":{}"#, serde_json::to_string(&planted).unwrap()),
    );
    write(&path, &text);
    let (packs, errors) = discover(dir.path());
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(packs[0].library, dir.path().join("levare-sfizz"));
    let (packs, errors) =
        crate::discover_in_roots(BTreeSet::from([dir.path().to_path_buf()]), None);
    assert!(packs.is_empty());
    assert!(errors[0].contains("sfizz"), "{errors:?}");
}

#[test]
fn an_installed_app_looks_for_packs_only_in_its_own_folders() {
    let app = Path::new("apps").join("Levare");
    let executable = app.join("bin").join("levare");
    #[cfg(target_os = "macos")]
    let own = app.join("Resources").join("soundpacks");
    #[cfg(not(target_os = "macos"))]
    let own = app.join("bin").join("soundpacks");
    assert_eq!(
        crate::pack_roots(&executable, false),
        std::slice::from_ref(&own)
    );
    let development = crate::pack_roots(&executable, true);
    assert_eq!(development[0], own);
    assert!(development.contains(&Path::new("apps").join("soundpacks")));
}

#[test]
fn pack_ids_are_plain_folder_names_and_never_the_combined_bank() {
    for id in ["vsco-ce", "Virtual_Playing-Orchestra3"] {
        assert!(super::manifest::valid_pack_id(id), "{id}");
    }
    for id in ["", "sfz-combinadas", "..", "a/b", "a\\b", "con espacio"] {
        assert!(!super::manifest::valid_pack_id(id), "{id}");
    }
}
