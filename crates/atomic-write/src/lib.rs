//! Replacing a file Levare writes (a score, an export, a settings file) with a
//! finished temporary copy written beside it, so a failed write never leaves
//! a truncated file behind.
//!
//! The copy takes the permissions of the file it replaces, a symbolic link is
//! written through so it keeps pointing at the same file, and on Unix the
//! folder is flushed after the rename so the new name survives a power cut.
//! On Windows the rename fails for a moment while another program (OneDrive,
//! Defender, the search indexer) has the target open, so it is retried a few
//! times.
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::Duration,
};
use tempfile::NamedTempFile;

/// Waits between attempts on Windows: about two thirds of a second in all.
const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(10),
    Duration::from_millis(30),
    Duration::from_millis(80),
    Duration::from_millis(200),
    Duration::from_millis(400),
];

/// Writes `bytes` to `path` in one step: into a temporary file beside it,
/// flushed to disk, then renamed over it by [`persist`].
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = temporary_for(path)?;
    temporary.write_all(bytes).map_err(|e| explain(e, path))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| explain(e, path))?;
    persist(temporary, path)
}

/// An empty temporary file in the folder of the file `path` names, for
/// writers that need a path to fill (Quartz) before [`persist`]. On Unix it
/// starts with the replaced file's permissions, so its contents are never
/// more exposed than the file's while they are written, or with those a new
/// file gets (the umask) when there is none yet. The owner can always write
/// it, so a writer that reopens it by path can fill it; [`persist`] refuses a
/// read-only target afterwards.
pub fn temporary_for(path: &Path) -> io::Result<NamedTempFile> {
    let target = resolve(path);
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(&target).map_or(0o666, |metadata| {
            (metadata.permissions().mode() & 0o777) | 0o200
        });
        builder.permissions(fs::Permissions::from_mode(mode));
    }
    builder
        .tempfile_in(directory_of(&target))
        .map_err(|e| explain(e, path))
}

/// Replaces `path` with `temporary`, which should come from
/// [`temporary_for`]: a temporary file on another disk cannot be renamed
/// over the target. A read-only target is refused on every platform, so a
/// protected score is never overwritten. Errors carry a message for the
/// musician, in Spanish.
pub fn persist(temporary: NamedTempFile, path: &Path) -> io::Result<()> {
    let target = resolve(path);
    if is_read_only(&target) {
        return Err(explain(
            io::Error::from(io::ErrorKind::PermissionDenied),
            path,
        ));
    }
    keep_permissions(temporary.as_file(), &target).map_err(|e| explain(e, path))?;
    let mut temporary = temporary;
    let mut attempt = 0;
    loop {
        match temporary.persist(&target) {
            Ok(_) => break,
            Err(failure) => match retry_delay(&failure.error, attempt, cfg!(windows)) {
                Some(delay) if !is_read_only(&target) => {
                    thread::sleep(delay);
                    temporary = failure.file;
                    attempt += 1;
                }
                _ => return Err(explain(failure.error, path)),
            },
        }
    }
    sync_directory(directory_of(&target));
    Ok(())
}

/// The file `path` names. A symbolic link is followed, so the rename
/// replaces the file it points at and the link stays a link.
fn resolve(path: &Path) -> PathBuf {
    let is_link = fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink());
    if !is_link {
        return path.to_owned();
    }
    dunce::canonicalize(path).unwrap_or_else(|_| match fs::read_link(path) {
        // A link to a file not created yet: relative to the link's folder.
        Ok(destination) => directory_of(path).join(destination),
        Err(_) => path.to_owned(),
    })
}

/// Gives the temporary file the replaced file's permissions, so saving a
/// shared score does not make it private.
#[cfg(unix)]
fn keep_permissions(temporary: &fs::File, target: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(target) {
        Ok(metadata) => {
            let mode = metadata.permissions().mode() & 0o777;
            temporary.set_permissions(fs::Permissions::from_mode(mode))
        }
        Err(_) => Ok(()),
    }
}

/// Windows: the only permission std exposes is read-only, refused before;
/// the file's ACL is inherited from the folder, like the target's.
#[cfg(not(unix))]
fn keep_permissions(_temporary: &fs::File, _target: &Path) -> io::Result<()> {
    Ok(())
}

/// Makes the rename itself durable. Best effort: the file is already in
/// place, and some file systems refuse to flush a folder.
#[cfg(unix)]
fn sync_directory(directory: &Path) {
    if let Ok(directory) = fs::File::open(directory) {
        let _ = directory.sync_all();
    }
}

/// Windows has no folder flush through std; NTFS journals the rename.
#[cfg(not(unix))]
fn sync_directory(_directory: &Path) {}

/// Whether `error` is Windows saying another program holds the file for
/// now, which waiting can fix. Always false elsewhere.
pub fn is_transient(error: &io::Error) -> bool {
    transient_on(error, cfg!(windows))
}

fn transient_on(error: &io::Error, windows: bool) -> bool {
    windows && error.raw_os_error().is_some_and(is_windows_transient)
}

/// How long to wait before attempt `attempt + 1`, or `None` to give up.
fn retry_delay(error: &io::Error, attempt: usize, windows: bool) -> Option<Duration> {
    transient_on(error, windows)
        .then(|| RETRY_DELAYS.get(attempt).copied())
        .flatten()
}

/// Codes Windows returns while another process has the file open.
fn is_windows_transient(code: i32) -> bool {
    const ERROR_ACCESS_DENIED: i32 = 5;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    matches!(
        code,
        ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION
    )
}

/// The folder a relative name such as `obra.partitura` lives in.
fn directory_of(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn is_read_only(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|metadata| metadata.permissions().readonly())
}

/// The message for the musician that some returned `io::Error`s carry, so a
/// caller can tell it from the system's own English text.
#[derive(Debug)]
pub struct Explanation {
    pub message: String,
    /// The target is protected against writing.
    pub read_only: bool,
}
impl std::fmt::Display for Explanation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Explanation {}

/// The [`Explanation`] inside `error`, if this crate added one.
pub fn explanation(error: &io::Error) -> Option<&Explanation> {
    error.get_ref()?.downcast_ref()
}

fn explain(error: io::Error, path: &Path) -> io::Error {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let in_use = cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33));
    let read_only = is_read_only(path);
    let message = if read_only {
        format!(
            "«{name}» es de solo lectura. Guarda con otro nombre o quita la protección contra escritura"
        )
    } else if in_use {
        format!("Otro programa está usando «{name}». Ciérralo y vuelve a intentarlo")
    } else if error.kind() == io::ErrorKind::PermissionDenied {
        format!(
            "No hay permiso para escribir «{name}». Puede que otro programa lo tenga abierto o que la carpeta esté protegida"
        )
    } else {
        return error;
    };
    io::Error::new(error.kind(), Explanation { message, read_only })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_with(directory: &Path, text: &str) -> NamedTempFile {
        let mut temporary = NamedTempFile::new_in(directory).unwrap();
        temporary.write_all(text.as_bytes()).unwrap();
        temporary
    }

    #[test]
    fn windows_retries_only_while_another_program_holds_the_file() {
        for code in [5, 32, 33] {
            let error = io::Error::from_raw_os_error(code);
            let delays: Vec<_> = (0..10)
                .map_while(|attempt| retry_delay(&error, attempt, true))
                .collect();
            assert_eq!(delays, RETRY_DELAYS);
            assert_eq!(retry_delay(&error, 0, false), None, "Unix never retries");
        }
        // Not found, path too long, disk full: retrying cannot help.
        for code in [2, 3, 112, 206] {
            let error = io::Error::from_raw_os_error(code);
            assert_eq!(retry_delay(&error, 0, true), None);
        }
        assert_eq!(retry_delay(&io::Error::other("x"), 0, true), None);
    }

    #[test]
    fn replaces_the_target_and_creates_a_new_one() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("partitura.json");
        persist(temporary_with(directory.path(), "uno"), &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "uno");
        persist(temporary_with(directory.path(), "dos"), &path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "dos");
    }

    #[test]
    fn writes_bytes_without_leaving_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("atajos.json");
        write(&path, b"{}").unwrap();
        write(&path, b"{\"version\": 1}").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"version\": 1}");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[cfg(unix)]
    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn saving_keeps_the_permissions_of_the_replaced_file() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Compartida.partitura");
        for permissions in [0o644, 0o664, 0o600] {
            std::fs::write(&path, "antes").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(permissions)).unwrap();
            write(&path, b"despues").unwrap();
            assert_eq!(mode(&path), permissions);
            persist(temporary_with(directory.path(), "otra vez"), &path).unwrap();
            assert_eq!(mode(&path), permissions);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_private_file_stays_private_while_its_copy_is_written() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Privada.partitura");
        std::fs::write(&path, "antes").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let temporary = temporary_for(&path).unwrap();
        assert_eq!(mode(temporary.path()), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_new_file_is_readable_like_any_other_the_user_creates() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Nueva.partitura");
        write(&path, b"x").unwrap();
        let ordinary = directory.path().join("ordinaria");
        std::fs::write(&ordinary, "x").unwrap();
        assert_eq!(mode(&path), mode(&ordinary), "umask, not 0600");
    }

    #[cfg(unix)]
    #[test]
    fn saving_through_a_symbolic_link_updates_the_file_it_names() {
        let directory = tempfile::tempdir().unwrap();
        let elsewhere = directory.path().join("obras");
        std::fs::create_dir(&elsewhere).unwrap();
        let real = elsewhere.join("Sinfonia.partitura");
        std::fs::write(&real, "antes").unwrap();
        let link = directory.path().join("Enlace.partitura");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write(&link, b"despues").unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "despues");

        persist(temporary_with(directory.path(), "otra vez"), &link).unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&real).unwrap(), "otra vez");

        // A relative link to a file that does not exist yet creates it.
        let dangling = directory.path().join("Pendiente.partitura");
        std::os::unix::fs::symlink("obras/Nueva.partitura", &dangling).unwrap();
        write(&dangling, b"nueva").unwrap();
        assert!(
            std::fs::symlink_metadata(&dangling)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_to_string(elsewhere.join("Nueva.partitura")).unwrap(),
            "nueva"
        );
    }

    #[test]
    fn a_read_only_target_is_kept_and_explained_in_spanish() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Protegida.partitura");
        std::fs::write(&path, "original").unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions.clone()).unwrap();
        let error = persist(temporary_with(directory.path(), "nueva"), &path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            error
                .to_string()
                .starts_with("«Protegida.partitura» es de solo lectura"),
            "{error}"
        );
        assert!(explanation(&error).is_some_and(|explained| explained.read_only));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
}
