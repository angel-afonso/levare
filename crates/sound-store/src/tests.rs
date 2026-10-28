use super::*;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use zip::write::SimpleFileOptions;

/// Serves `body` to every request until the test ends.
fn serve(body: Vec<u8>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                line.clear();
            }
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    format!("http://{address}/paquete.zip")
}

fn zip(files: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, text) in files {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(text.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

const MANIFEST: &str = r#"{"format":2,"kind":"sfz","id":"prueba","name":"Prueba","samples":"samples","license":"LICENSE","instruments":{"Violin":{"sustain":"violin.sfz"}}}"#;
const SFZ: &str = "<region> key=60 sample=violin.flac\n";

fn hosted(files: &[(&str, &str)], version: u32) -> (Entry, Vec<u8>) {
    let body = zip(files);
    let entry = Entry {
        id: "prueba".into(),
        name: "Prueba".into(),
        description: String::new(),
        version,
        author: "Levare".into(),
        license: License {
            name: "CC0".into(),
            url: "https://creativecommons.org/publicdomain/zero/1.0/".into(),
        },
        installed_size: 4096,
        archives: vec![Archive {
            url: serve(body.clone()),
            sha256: sha(&body),
            size: body.len() as u64,
            strip: String::new(),
            into: String::new(),
        }],
        manifest: None,
    };
    (entry, body)
}

fn good_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("manifest.json", MANIFEST),
        ("LICENSE", "CC0"),
        ("samples/violin.sfz", SFZ),
        ("samples/violin.flac", "audio"),
    ]
}

struct Setup {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    engine: std::path::PathBuf,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("soundpacks");
    let engine = dir.path().join("libsfizz.dylib");
    std::fs::write(&engine, "").unwrap();
    Setup {
        _dir: dir,
        root,
        engine,
    }
}

/// Nothing but the finished packs remains in the soundpacks folder.
fn leftovers(root: &Path) -> Vec<String> {
    let mut names: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".descargas")
        .collect();
    names.sort();
    names
}

#[test]
fn a_hosted_pack_installs_verified_lists_and_uninstalls() {
    let setup = setup();
    let (entry, body) = hosted(&good_files(), 1);
    let progress = Progress::default();
    let path = install(&entry, &setup.root, &setup.engine, &progress).unwrap();
    assert_eq!(path, setup.root.join("prueba"));
    assert!(path.join("samples/violin.flac").is_file());
    assert_eq!(progress.bytes(), (body.len() as u64, body.len() as u64));
    assert_eq!(
        installed(&setup.root),
        [Installed {
            id: "prueba".into(),
            version: 1
        }]
    );
    assert_eq!(leftovers(&setup.root), ["prueba"]);
    let found =
        score_sound_vsco::SfzPack::load_with_engine(&path.join("manifest.json"), &setup.engine);
    assert!(found.is_ok());

    uninstall("prueba", &setup.root).unwrap();
    assert!(installed(&setup.root).is_empty());
    assert!(!path.exists());
}

#[test]
fn a_new_version_replaces_the_old_one_only_once_it_validates() {
    let setup = setup();
    let (first, _) = hosted(&good_files(), 1);
    install(&first, &setup.root, &setup.engine, &Progress::default()).unwrap();
    // A broken update leaves the installed version alone.
    let mut broken_files = good_files();
    broken_files.retain(|(name, _)| *name != "samples/violin.sfz");
    let (broken, _) = hosted(&broken_files, 2);
    assert!(install(&broken, &setup.root, &setup.engine, &Progress::default()).is_err());
    assert_eq!(installed(&setup.root)[0].version, 1);
    assert_eq!(leftovers(&setup.root), ["prueba"]);

    let (second, _) = hosted(&good_files(), 2);
    install(&second, &setup.root, &setup.engine, &Progress::default()).unwrap();
    assert_eq!(installed(&setup.root)[0].version, 2);
    assert_eq!(leftovers(&setup.root), ["prueba"]);
}

#[test]
fn a_download_that_does_not_match_its_checksum_is_discarded() {
    let setup = setup();
    let (mut entry, _) = hosted(&good_files(), 1);
    entry.archives[0].sha256 = "0".repeat(64);
    let failure = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap_err();
    assert!(failure.0.contains("SHA-256"), "{failure}");
    assert!(leftovers(&setup.root).is_empty());
}

#[test]
fn paths_that_escape_the_pack_reject_it() {
    let setup = setup();
    let mut files = good_files();
    files.push(("../fuera.txt", "x"));
    let (entry, _) = hosted(&files, 1);
    let failure = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap_err();
    assert!(failure.0.contains("rutas"), "{failure}");
    assert!(!setup.root.parent().unwrap().join("fuera.txt").exists());
    assert!(leftovers(&setup.root).is_empty());
}

#[test]
fn a_pack_never_brings_its_own_engine_or_executables() {
    let setup = setup();
    let with_engine = MANIFEST.replace(r#""license""#, r#""engine":"x.dylib","license""#);
    let mut files = good_files();
    files[0] = ("manifest.json", &with_engine);
    let (entry, _) = hosted(&files, 1);
    let failure = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap_err();
    assert!(failure.0.contains("motor"), "{failure}");

    let mut files = good_files();
    files.push(("samples/libsfizz.dylib", "code"));
    files.push(("samples/run.sh", "code"));
    let (entry, _) = hosted(&files, 1);
    let path = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap();
    assert!(!path.join("samples/libsfizz.dylib").exists());
    assert!(!path.join("samples/run.sh").exists());
}

#[test]
fn a_catalog_entry_cannot_take_the_combined_bank_s_id() {
    let setup = setup();
    let (mut entry, _) = hosted(&good_files(), 1);
    entry.id = "sfz-combinadas".into();
    let failure = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap_err();
    assert!(failure.0.contains("Identificador"), "{failure}");
    assert!(!setup.root.join("sfz-combinadas").exists());
}

#[test]
fn a_cancelled_install_leaves_nothing_behind() {
    let setup = setup();
    let (entry, _) = hosted(&good_files(), 1);
    let progress = Progress::default();
    progress.cancel();
    let failure = install(&entry, &setup.root, &setup.engine, &progress).unwrap_err();
    assert_eq!(failure.0, "Instalación cancelada");
    assert!(leftovers(&setup.root).is_empty());
}

#[test]
fn a_recipe_unpacks_the_authors_folder_and_adds_levares_manifest() {
    let setup = setup();
    let files = [
        ("Biblioteca-1.0/violin.sfz", SFZ),
        ("Biblioteca-1.0/violin.flac", "audio"),
        ("Biblioteca-1.0/LICENSE.txt", "CC0"),
        ("Leeme.txt", "fuera de la carpeta"),
    ];
    let (mut entry, _) = hosted(&files, 1);
    entry.archives[0].strip = "Biblioteca-1.0".into();
    entry.archives[0].into = "Biblioteca".into();
    entry.manifest = Some(serde_json::json!({
        "format": 2, "kind": "sfz", "id": "prueba", "name": "Prueba",
        "samples": "Biblioteca", "license": "Biblioteca/LICENSE.txt",
        "instruments": {"Violin": {"sustain": "violin.sfz"}}
    }));
    let path = install(&entry, &setup.root, &setup.engine, &Progress::default()).unwrap();
    assert!(path.join("Biblioteca/violin.flac").is_file());
    assert!(!path.join("Leeme.txt").exists());
    assert!(path.join("manifest.json").is_file());

    entry.manifest.as_mut().unwrap()["engine"] = "x.dylib".into();
    assert!(install(&entry, &setup.root, &setup.engine, &Progress::default()).is_err());
}

#[test]
fn packs_copied_by_hand_are_listed_by_discovery_but_never_removed_here() {
    let setup = setup();
    let manual = setup.root.join("manual");
    std::fs::create_dir_all(&manual).unwrap();
    std::fs::write(manual.join("manifest.json"), MANIFEST).unwrap();
    assert!(installed(&setup.root).is_empty());
    assert!(uninstall("manual", &setup.root).is_err());
    assert!(manual.exists());
}

#[test]
fn the_builtin_catalog_is_valid_and_brings_no_engine() {
    let catalog = Catalog::builtin();
    let mut ids: Vec<_> = catalog
        .packs
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), catalog.packs.len());
    for entry in &catalog.packs {
        assert!(!entry.archives.is_empty(), "{}", entry.id);
        assert!(entry.installed_size > 0, "{}", entry.id);
        for archive in &entry.archives {
            assert!(archive.url.starts_with("https://"), "{}", archive.url);
            assert_eq!(archive.sha256.len(), 64, "{}", entry.id);
            assert!(archive.size > 0, "{}", entry.id);
        }
        assert!(
            entry
                .manifest
                .as_ref()
                .is_none_or(|manifest| manifest.get("engine").is_none())
        );
    }
}

#[test]
fn sizes_read_in_megabytes_and_gigabytes() {
    assert_eq!(size_label(461_000_000), "461 MB");
    assert_eq!(size_label(1_234_000_000), "1,2 GB");
}

#[test]
#[ignore = "requires target/soundpacks from scripts/build_soundpack_vsco.py and the local sfizz"]
fn the_built_vsco_pack_installs_and_every_patch_loads_in_sfizz() {
    use score_platform::sound_engine::SoundBank;
    let built = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/soundpacks/vsco-alta-calidad-1.zip");
    let body = std::fs::read(&built).unwrap();
    let mut entry = Catalog::builtin()
        .entry("vsco-alta-calidad")
        .unwrap()
        .clone();
    assert_eq!(
        entry.archives[0].sha256,
        sha(&body),
        "rebuild changed the zip"
    );
    entry.archives[0].url = serve(body);
    let dir = tempfile::tempdir().unwrap();
    let engine = score_sound_vsco::bundled_engine().expect("local sfizz");
    let progress = Progress::default();
    let path = install(&entry, dir.path(), &engine, &progress).unwrap();
    let pack = score_sound_vsco::SfzPack::load(&path.join("manifest.json")).unwrap();
    use score_core::Instrument::*;
    let score = score_core::Score::new(
        "".into(),
        &[
            Piano, Flute, Oboe, ClarinetBb, Bassoon, HornF, TrumpetBb, Trombone, Violin, Violin,
            Viola, Cello, DoubleBass,
        ],
        1,
    )
    .unwrap();
    assert!(pack.prepare(&score, Violin, 48_000).is_ok());
}

#[test]
#[ignore = "requires the VPO downloads in target/descargas-vpo and the local sfizz"]
fn the_vpo_recipe_installs_from_the_authors_files_and_every_patch_loads() {
    use score_platform::sound_engine::SoundBank;
    let downloads = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/descargas-vpo");
    let mut entry = Catalog::builtin()
        .entry("virtual-playing-orchestra")
        .unwrap()
        .clone();
    for (archive, file) in entry.archives.iter_mut().zip([
        "Virtual-Playing-Orchestra3-2-wave-files.zip",
        "Virtual-Playing-Orchestra3-3-performance-scripts.zip",
    ]) {
        let body = std::fs::read(downloads.join(file)).unwrap();
        assert_eq!(archive.sha256, sha(&body), "{file}");
        archive.url = serve(body);
    }
    let dir = tempfile::tempdir().unwrap();
    let engine = score_sound_vsco::bundled_engine().expect("local sfizz");
    let path = install(&entry, dir.path(), &engine, &Progress::default()).unwrap();
    assert!(path.join("Virtual-Playing-Orchestra3/libs").is_dir());
    let pack = score_sound_vsco::SfzPack::load(&path.join("manifest.json")).unwrap();
    use score_core::Instrument::*;
    let score = score_core::Score::new(
        "".into(),
        &[
            Flute, Oboe, ClarinetBb, Bassoon, HornF, TrumpetBb, Trombone, Tuba, Harp, Timpani,
            Violin, Violin, Viola, Cello, DoubleBass,
        ],
        1,
    )
    .unwrap();
    assert!(pack.prepare(&score, Violin, 48_000).is_ok());
}
