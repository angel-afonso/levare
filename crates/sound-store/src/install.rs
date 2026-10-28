//! Download, verify, unpack and swap a pack into the soundpacks folder.
use crate::{Archive, Entry, StoreError, error};
use score_platform::sound_engine::SoundBank;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::Duration;

/// Written by Levare into every pack it installs; packs without it were
/// copied by hand and are never removed from the app.
const STAMP: &str = ".levare-paquete.json";

/// Only these files, and names without extension such as LICENSE, are
/// unpacked; anything else, libraries and scripts included, is skipped.
/// Nothing unpacked is ever marked executable.
const ALLOWED: [&str; 12] = [
    "sfz", "wav", "flac", "ogg", "aif", "aiff", "txt", "md", "htm", "html", "pdf", "json",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Downloading,
    Unpacking,
    Checking,
}

/// Shared between the installing thread and the interface.
#[derive(Debug, Default)]
pub struct Progress {
    done: AtomicU64,
    total: AtomicU64,
    stage: AtomicU8,
    cancelled: AtomicBool,
}

impl Progress {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Downloaded and expected bytes.
    pub fn bytes(&self) -> (u64, u64) {
        (
            self.done.load(Ordering::Relaxed),
            self.total.load(Ordering::Relaxed),
        )
    }

    pub fn stage(&self) -> Stage {
        match self.stage.load(Ordering::Relaxed) {
            0 => Stage::Downloading,
            1 => Stage::Unpacking,
            _ => Stage::Checking,
        }
    }

    fn set_stage(&self, stage: Stage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    fn check(&self) -> Result<(), StoreError> {
        if self.is_cancelled() {
            Err(error("Instalación cancelada"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Installed {
    pub id: String,
    pub version: u32,
}

/// Packs installed from the catalog under `root`.
pub fn installed(root: &Path) -> Vec<Installed> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut packs: Vec<Installed> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|entry| stamp(&entry.path()))
        .collect();
    packs.sort_by(|left, right| left.id.cmp(&right.id));
    packs
}

fn stamp(pack: &Path) -> Option<Installed> {
    serde_json::from_slice(&fs::read(pack.join(STAMP)).ok()?).ok()
}

/// Install `entry` as `root/<id>`, replacing an older version only once the
/// new one validates. `engine` is the sfizz library Levare ships.
pub fn install(
    entry: &Entry,
    root: &Path,
    engine: &Path,
    progress: &Progress,
) -> Result<PathBuf, StoreError> {
    prepare_install(entry, root, engine, progress)?.finish()
}

/// A pack downloaded, unpacked and validated beside the soundpacks, ready
/// to take the place of `root/<id>`.
#[derive(Debug)]
#[must_use = "the pack is installed only by `finish`"]
pub struct Prepared {
    staging: PathBuf,
    target: PathBuf,
    previous: PathBuf,
}

impl Prepared {
    /// Swap the new pack in. Playback must have let go of the old one
    /// first: Windows cannot move a folder whose files are open.
    pub fn finish(self) -> Result<PathBuf, StoreError> {
        let Self {
            staging,
            target,
            previous,
        } = self;
        let _ = remove_dir_all(&previous);
        if target.exists() {
            retry(|| fs::rename(&target, &previous)).map_err(io)?;
        }
        if let Err(cause) = retry(|| fs::rename(&staging, &target)) {
            let _ = retry(|| fs::rename(&previous, &target));
            return Err(io(cause));
        }
        // The staging folder was hidden; the pack is not.
        set_hidden(&target, false);
        let _ = remove_dir_all(&previous);
        Ok(target)
    }
}

/// Everything but the swap of [`install`], which may take minutes: the old
/// version keeps playing meanwhile.
pub fn prepare_install(
    entry: &Entry,
    root: &Path,
    engine: &Path,
    progress: &Progress,
) -> Result<Prepared, StoreError> {
    let valid_id = !entry.id.is_empty()
        && entry
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !valid_id {
        return Err(error(format!(
            "Identificador de paquete no válido: {}",
            entry.id
        )));
    }
    if entry
        .manifest
        .as_ref()
        .is_some_and(|manifest| manifest.get("engine").is_some())
    {
        return Err(error("El paquete intenta usar su propio motor de sonido"));
    }
    fs::create_dir_all(root).map_err(io)?;
    let needed = entry.download_size() + entry.installed_size;
    if let Some(free) = free_space(root)
        && free < needed
    {
        return Err(error(format!(
            "Hacen falta {} libres; quedan {}",
            size_label(needed),
            size_label(free)
        )));
    }
    let staging = root.join(format!(".instalando-{}", entry.id));
    let downloads = root.join(".descargas");
    let result = prepare(entry, &staging, &downloads, engine, progress);
    for (index, _) in entry.archives.iter().enumerate() {
        let _ = fs::remove_file(download_path(&downloads, entry, index));
    }
    if let Err(cause) = result {
        let _ = remove_dir_all(&staging);
        return Err(cause);
    }
    Ok(Prepared {
        staging,
        target: root.join(&entry.id),
        previous: root.join(format!(".antiguo-{}", entry.id)),
    })
}

fn prepare(
    entry: &Entry,
    staging: &Path,
    downloads: &Path,
    engine: &Path,
    progress: &Progress,
) -> Result<(), StoreError> {
    let _ = remove_dir_all(staging);
    fs::create_dir_all(staging).map_err(io)?;
    set_hidden(staging, true);
    fs::create_dir_all(downloads).map_err(io)?;
    set_hidden(downloads, true);
    progress
        .total
        .store(entry.download_size(), Ordering::Relaxed);
    // A little room for archives that round their sizes.
    let mut budget = entry.installed_size + entry.installed_size / 10 + (1 << 20);
    for (index, archive) in entry.archives.iter().enumerate() {
        let zip = download_path(downloads, entry, index);
        progress.set_stage(Stage::Downloading);
        download(archive, &zip, progress)?;
        progress.set_stage(Stage::Unpacking);
        unpack(&zip, archive, staging, &mut budget, progress)?;
        let _ = fs::remove_file(&zip);
    }
    progress.set_stage(Stage::Checking);
    let manifest = staging.join("manifest.json");
    match &entry.manifest {
        Some(value) => {
            let text =
                serde_json::to_vec_pretty(value).map_err(|cause| error(cause.to_string()))?;
            fs::write(&manifest, text).map_err(io)?;
        }
        None => {
            let text =
                fs::read(&manifest).map_err(|_| error("Al paquete le falta manifest.json"))?;
            let value: serde_json::Value =
                serde_json::from_slice(&text).map_err(|cause| error(cause.to_string()))?;
            if value.get("engine").is_some() {
                return Err(error("El paquete intenta usar su propio motor de sonido"));
            }
        }
    }
    let stamp = Installed {
        id: entry.id.clone(),
        version: entry.version,
    };
    fs::write(
        staging.join(STAMP),
        serde_json::to_vec(&stamp).expect("serializable stamp"),
    )
    .map_err(io)?;
    let pack = score_sound_vsco::SfzPack::load_with_engine(&manifest, engine)
        .map_err(|cause| error(format!("El paquete no es válido: {cause}")))?;
    if pack.id() != entry.id {
        return Err(error("El paquete descargado no corresponde al catálogo"));
    }
    Ok(())
}

fn download_path(downloads: &Path, entry: &Entry, index: usize) -> PathBuf {
    downloads.join(format!("{}-{index}.zip", entry.id))
}

/// Stream `archive` to `path`, hashing as it arrives. A stalled connection
/// fails after a minute without data; a large one never times out.
fn download(archive: &Archive, path: &Path, progress: &Progress) -> Result<(), StoreError> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("Levare/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(network)?;
    let mut response = client
        .get(&archive.url)
        .send()
        .and_then(|response| response.error_for_status())
        .map_err(network)?;
    let mut file = File::create(path).map_err(io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    let mut received = 0u64;
    loop {
        progress.check()?;
        let read = response
            .read(&mut buffer)
            .map_err(|cause| error(format!("La descarga se interrumpió: {cause}")))?;
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > archive.size {
            return Err(error("La descarga es mayor de lo anunciado"));
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read]).map_err(io)?;
        progress.done.fetch_add(read as u64, Ordering::Relaxed);
    }
    if received != archive.size {
        return Err(error("La descarga quedó incompleta"));
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if !digest.eq_ignore_ascii_case(&archive.sha256) {
        return Err(error(
            "La descarga no coincide con la publicada (SHA-256); no se instala",
        ));
    }
    file.sync_all().map_err(io)
}

/// Unpack the allowed files under `archive.strip` into `staging/archive.into`.
/// A path that escapes the folder rejects the whole pack.
fn unpack(
    zip: &Path,
    archive: &Archive,
    staging: &Path,
    budget: &mut u64,
    progress: &Progress,
) -> Result<(), StoreError> {
    let invalid = |cause: zip::result::ZipError| {
        error(format!(
            "El archivo descargado no es un zip válido: {cause}"
        ))
    };
    let mut files =
        zip::ZipArchive::new(BufReader::new(File::open(zip).map_err(io)?)).map_err(invalid)?;
    let destination = staging.join(&archive.into);
    for index in 0..files.len() {
        progress.check()?;
        let mut file = files.by_index(index).map_err(invalid)?;
        let name = file
            .enclosed_name()
            .ok_or_else(|| error("El paquete contiene rutas no válidas"))?;
        let Ok(relative) = name.strip_prefix(&archive.strip) else {
            continue;
        };
        let hidden = relative.components().any(|part| {
            let part = part.as_os_str().to_string_lossy();
            part.starts_with('.') || part == "__MACOSX"
        });
        if relative.as_os_str().is_empty() || hidden || file.is_symlink() {
            continue;
        }
        let path = destination.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&path).map_err(io)?;
            continue;
        }
        let allowed = path.extension().is_none_or(|extension| {
            extension
                .to_str()
                .is_some_and(|extension| ALLOWED.contains(&extension.to_ascii_lowercase().as_str()))
        });
        if !allowed {
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io)?;
        }
        let mut output = File::create(&path).map_err(io)?;
        // The declared size can lie; count what is actually written.
        let written = std::io::copy(&mut (&mut file).take(*budget + 1), &mut output).map_err(io)?;
        *budget = budget
            .checked_sub(written)
            .ok_or_else(|| error("El paquete ocupa más de lo anunciado"))?;
    }
    Ok(())
}

/// Remove a pack installed from the catalog.
pub fn uninstall(id: &str, root: &Path) -> Result<(), StoreError> {
    let path = root.join(id);
    if stamp(&path).is_none_or(|stamp| stamp.id != id) {
        return Err(error(
            "Solo se pueden quitar desde Levare los paquetes instalados desde Levare",
        ));
    }
    remove_dir_all(&path).map_err(io)
}

/// On Windows, Defender, the search indexer or a backup client may hold a
/// file of a pack for a moment after it is written or read; renaming or
/// removing its folder then fails with access denied or a sharing violation.
/// Try again a few times before giving up. Elsewhere, once.
fn retry<T>(mut operation: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let waits: &[u64] = if cfg!(windows) {
        &[50, 100, 200, 400, 800]
    } else {
        &[]
    };
    for wait in waits {
        match operation() {
            Err(cause) if transient(&cause) => std::thread::sleep(Duration::from_millis(*wait)),
            done => return done,
        }
    }
    operation()
}

/// Another program holding a file, as when saving one, or a folder still
/// emptying. Only Windows retries: elsewhere `waits` is empty.
fn transient(cause: &std::io::Error) -> bool {
    atomic_write::is_transient(cause) || cause.kind() == std::io::ErrorKind::DirectoryNotEmpty
}

fn remove_dir_all(path: &Path) -> std::io::Result<()> {
    retry(|| match fs::remove_dir_all(path) {
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => Ok(()),
        done => done,
    })
}

/// Scratch folders start with a dot, hidden on macOS and Linux; Windows
/// needs the attribute, and keeps it through a rename.
#[cfg(windows)]
fn set_hidden(path: &Path, hidden: bool) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: a NUL-terminated wide path; failure only changes the look.
    unsafe {
        let attributes = GetFileAttributesW(wide.as_ptr());
        if attributes != INVALID_FILE_ATTRIBUTES {
            let attributes = if hidden {
                attributes | FILE_ATTRIBUTE_HIDDEN
            } else {
                attributes & !FILE_ATTRIBUTE_HIDDEN
            };
            SetFileAttributesW(wide.as_ptr(), attributes);
        }
    }
}

#[cfg(not(windows))]
fn set_hidden(_: &Path, _: bool) {}

#[cfg(unix)]
fn free_space(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs only writes into the zeroed struct it is given.
    let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: a valid NUL-terminated path and a writable statvfs.
    if unsafe { libc::statvfs(path.as_ptr(), &mut stats) } != 0 {
        return None;
    }
    // `f_bavail` is u32 on macOS and already u64 on Linux.
    #[allow(clippy::useless_conversion)]
    let available = u64::from(stats.f_bavail);
    available.checked_mul(stats.f_frsize)
}

#[cfg(windows)]
fn free_space(path: &Path) -> Option<u64> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut available = 0u64;
    // SAFETY: a NUL-terminated wide path, one writable u64, two nulls the
    // function accepts.
    let done = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (done != 0).then_some(available)
}

/// Unknown elsewhere, so the install goes ahead without the check.
#[cfg(not(any(unix, windows)))]
fn free_space(_: &Path) -> Option<u64> {
    None
}

/// "461 MB", "1,2 GB".
pub fn size_label(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9).replace('.', ",")
    } else {
        format!("{} MB", (bytes as f64 / 1e6).round() as u64)
    }
}

fn io(cause: std::io::Error) -> StoreError {
    if matches!(
        cause.kind(),
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded
    ) {
        error("No queda espacio en el disco")
    } else {
        error(cause.to_string())
    }
}

fn network(cause: reqwest::Error) -> StoreError {
    error(format!("No se pudo descargar: {cause}"))
}

#[cfg(test)]
mod disk_tests {
    use super::*;

    #[test]
    fn a_full_disk_is_named_on_every_system() {
        for kind in [
            std::io::ErrorKind::StorageFull,
            std::io::ErrorKind::QuotaExceeded,
        ] {
            assert_eq!(io(kind.into()).0, "No queda espacio en el disco");
        }
        assert!(free_space(&std::env::temp_dir()).is_some_and(|free| free > 0));
    }

    #[test]
    fn a_folder_already_gone_counts_as_removed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(remove_dir_all(&dir.path().join("nada")).is_ok());
    }
}
