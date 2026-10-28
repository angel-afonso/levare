//! Replacing a file Levare writes (a score, an export, a settings file) with a
//! finished temporary copy written beside it, so a failed write never leaves
//! a truncated file behind.
//!
//! On Windows the rename fails for a moment while another program (OneDrive,
//! Defender, the search indexer) has the target open, so it is retried a few
//! times. Elsewhere it behaves like a plain `NamedTempFile::persist`.
use std::{
    io::{self, Write},
    path::Path,
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
    let mut temporary = NamedTempFile::new_in(directory_of(path)).map_err(|e| explain(e, path))?;
    temporary.write_all(bytes).map_err(|e| explain(e, path))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| explain(e, path))?;
    persist(temporary, path)
}

/// Replaces `path` with `temporary`. A read-only target is refused on every
/// platform, so a protected score is never overwritten. Errors carry a
/// message for the musician, in Spanish.
pub fn persist(temporary: NamedTempFile, path: &Path) -> io::Result<()> {
    if is_read_only(path) {
        return Err(explain(
            io::Error::from(io::ErrorKind::PermissionDenied),
            path,
        ));
    }
    let mut temporary = temporary;
    let mut attempt = 0;
    loop {
        match temporary.persist(path) {
            Ok(_) => return Ok(()),
            Err(failure) => match retry_delay(&failure.error, attempt, cfg!(windows)) {
                Some(delay) if !is_read_only(path) => {
                    thread::sleep(delay);
                    temporary = failure.file;
                    attempt += 1;
                }
                _ => return Err(explain(failure.error, path)),
            },
        }
    }
}

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

fn explain(error: io::Error, path: &Path) -> io::Error {
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let in_use = cfg!(windows) && matches!(error.raw_os_error(), Some(32 | 33));
    let message = if is_read_only(path) {
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
    io::Error::new(error.kind(), message)
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
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "original");
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&path, permissions).unwrap();
    }
}
