//! Replaces the installed app with a verified archive's.
//!
//! On macOS the archive holds `Levare.app` and the whole bundle is swapped
//! (`bundle`). On Windows and Linux it holds a folder with the executable and
//! the sfizz library, and each of those files is swapped into the folder the
//! running executable is in (`folder`).
//!
//! Either way the archive is unpacked next to the installation (so moving it
//! into place is a rename on the same disk), checked, and swapped in: what is
//! replaced is renamed aside, the new files renamed into its place, and the
//! old ones put back if that fails. The running process keeps its files open,
//! so it can carry on until it restarts.

#[cfg(target_os = "macos")]
mod bundle;
mod folder;

use std::path::{Path, PathBuf};

use crate::Error;

/// Where the next update unpacks, next to the installation.
const STAGING: &str = ".levare-update";

/// The running app's installation, as it can be replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// `Levare.app`.
    #[cfg(target_os = "macos")]
    Bundle(PathBuf),
    /// The executable, in a folder with the libraries it loads.
    Folder(PathBuf),
}

impl Installation {
    /// A macOS app bundle, replaced as a whole.
    #[cfg(target_os = "macos")]
    pub fn app_bundle(bundle: impl Into<PathBuf>) -> Self {
        Self {
            kind: Kind::Bundle(bundle.into()),
        }
    }

    /// An executable in its own folder, as Levare ships on Windows and Linux:
    /// an update replaces the files of that folder the archive brings.
    pub fn folder(executable: impl Into<PathBuf>) -> Self {
        Self {
            kind: Kind::Folder(executable.into()),
        }
    }

    /// The running app's; an error when it cannot replace itself: not
    /// running from `Levare.app` on macOS, running from a cargo build, or
    /// installed in a folder Levare cannot write.
    pub fn current() -> Result<Self, Error> {
        #[cfg(target_os = "macos")]
        {
            bundle::current()
        }
        #[cfg(not(target_os = "macos"))]
        {
            folder::current()
        }
    }

    /// The installed bundle or executable; what to relaunch.
    pub fn path(&self) -> &Path {
        match &self.kind {
            #[cfg(target_os = "macos")]
            Kind::Bundle(path) => path,
            Kind::Folder(path) => path,
        }
    }

    /// Replaces the installation with the one in `archive` (already
    /// verified by [`crate::Updater::download`]) and returns what to
    /// relaunch.
    pub fn install(&self, archive: &Path) -> Result<PathBuf, Error> {
        let staging = self.parent()?.join(STAGING);
        remove(&staging);
        std::fs::create_dir_all(&staging).map_err(|e| writable(e, &staging))?;
        let result = match &self.kind {
            #[cfg(target_os = "macos")]
            Kind::Bundle(path) => bundle::install(path, archive, &staging),
            Kind::Folder(path) => folder::install(path, archive, &staging),
        };
        remove(&staging);
        result
    }

    /// Removes what an earlier update left: its staging folder and the
    /// replaced files, which Windows cannot delete while they are running.
    pub fn clean_up(&self) {
        let Ok(parent) = self.parent() else {
            return;
        };
        remove(&parent.join(STAGING));
        match &self.kind {
            #[cfg(target_os = "macos")]
            Kind::Bundle(path) => remove(&aside(path)),
            Kind::Folder(_) => folder::clean_up(parent),
        }
    }

    fn parent(&self) -> Result<&Path, Error> {
        self.path().parent().ok_or_else(|| {
            Error::Install(format!(
                "{} no está en ninguna carpeta",
                self.path().display()
            ))
        })
    }
}

/// Where `path` is moved while it is replaced: `.<name>.old` beside it.
fn aside(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.old"))
}

/// A permission error says which folder Levare could not write.
fn writable(error: std::io::Error, path: &Path) -> Error {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        let dir = path.parent().unwrap_or(path);
        Error::Install(format!("Levare no puede escribir en {}", dir.display()))
    } else {
        Error::Io(error)
    }
}

/// Removes a file or folder, if there.
fn remove(path: &Path) {
    let _ = if path.is_dir() && !path.is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_replaced_file_goes_beside_it() {
        assert_eq!(
            aside(Path::new("/opt/levare/levare")),
            Path::new("/opt/levare/.levare.old")
        );
    }
}
