//! Windows and Linux: Levare is a folder holding the executable and the sfizz
//! library (`Levare\levare.exe` and `sfizz.dll`, `levare/levare` and
//! `libsfizz.so`). The archive holds the same folder; each of its files
//! replaces the one of the same name in the installed folder.
//!
//! A running executable or a loaded DLL cannot be overwritten or deleted on
//! Windows, but it can be renamed, so each replaced file is renamed aside to
//! `.<name>.old` first; those are removed on the next start.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Installation, aside, remove, writable};
use crate::{Error, archive};

/// The folder the running executable is in, when Levare can replace it.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(super) fn current() -> Result<Installation, Error> {
    let exe = std::env::current_exe()?;
    // Through any symlink (a `~/.local/bin/levare` link to the installed
    // one) to the real folder. Windows keeps the plain path: a `\\?\` one
    // would also be what the update relaunches.
    #[cfg(unix)]
    let exe = exe.canonicalize()?;
    let dir = exe
        .parent()
        .ok_or_else(|| Error::Install(format!("{} no está en ninguna carpeta", exe.display())))?;
    if is_cargo_build(dir) {
        return Err(Error::Install(format!(
            "Levare se está ejecutando desde una compilación de cargo ({}), que no se \
             actualiza sola",
            dir.display()
        )));
    }
    check_writable(dir)?;
    Ok(Installation::folder(exe))
}

/// `target/debug`, `target/release` and their per-target versions: cargo
/// keeps a `.cargo-lock` in each.
fn is_cargo_build(dir: &Path) -> bool {
    let profile = dir
        .file_name()
        .is_some_and(|name| name == "debug" || name == "release");
    let in_target = dir
        .ancestors()
        .skip(1)
        .any(|ancestor| ancestor.file_name().is_some_and(|name| name == "target"));
    dir.join(".cargo-lock").exists() || (profile && in_target)
}

/// An installation under `/usr` or `C:\Program Files` belongs to the system
/// or its package manager, and Levare cannot (and should not) replace it.
fn check_writable(dir: &Path) -> Result<(), Error> {
    let probe = dir.join(".levare-write-test");
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        // Left by a crash between creating and removing it: the folder is
        // writable.
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(_) => Err(Error::Install(format!(
            "Levare está instalado en {}, donde no puede escribir. Instala la versión nueva \
             desde la página de versiones, o mueve Levare a una carpeta tuya",
            dir.display()
        ))),
    }
}

pub(super) fn install(exe: &Path, archive: &Path, staging: &Path) -> Result<PathBuf, Error> {
    let dir = exe
        .parent()
        .ok_or_else(|| Error::Install(format!("{} no está en ninguna carpeta", exe.display())))?;
    let name = exe
        .file_name()
        .ok_or_else(|| Error::Install(format!("{} no es un ejecutable", exe.display())))?;
    archive::extract(archive, staging)?;
    let new_exe = find_file(staging, name)?;
    let payload = new_exe.parent().unwrap_or(staging);
    let mut names: Vec<OsString> = std::fs::read_dir(payload)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<_, _>>()?;
    names.sort();
    swap(dir, payload, &names)?;
    // Linux can delete a running executable; Windows leaves these for the
    // next start's clean-up.
    for name in &names {
        remove(&aside(&dir.join(name)));
    }
    Ok(exe.to_path_buf())
}

/// The file `name` at the top of `dir` or one folder down (the archive wraps
/// it in a `levare` or `Levare` folder, which on Linux has its name).
fn find_file(dir: &Path, name: &std::ffi::OsStr) -> Result<PathBuf, Error> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Ok(direct);
    }
    for entry in std::fs::read_dir(dir)? {
        let candidate = entry?.path().join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(Error::Install(format!(
        "la actualización no trae {}",
        name.to_string_lossy()
    )))
}

/// One file moved into the installation, and where what it replaced went.
struct Moved {
    target: PathBuf,
    aside: Option<PathBuf>,
}

/// Moves each of `names` from `payload` into `dir`, renaming what was there
/// aside first. On any failure every file moved so far is taken out and the
/// replaced ones are put back.
fn swap(dir: &Path, payload: &Path, names: &[OsString]) -> Result<(), Error> {
    let mut moved: Vec<Moved> = Vec::new();
    for name in names {
        let target = dir.join(name);
        match move_in(&payload.join(name), &target) {
            Ok(aside) => moved.push(Moved { target, aside }),
            Err(error) => {
                for done in moved.iter().rev() {
                    remove(&done.target);
                    if let Some(aside) = &done.aside {
                        let _ = std::fs::rename(aside, &done.target);
                    }
                }
                return Err(writable(error, &target));
            }
        }
    }
    Ok(())
}

/// Moves `new` to `target`, renaming what was there aside first and back if
/// the move fails. Returns where the replaced file went, if there was one.
fn move_in(new: &Path, target: &Path) -> std::io::Result<Option<PathBuf>> {
    let aside = aside(target);
    remove(&aside);
    let existed = target.symlink_metadata().is_ok();
    if existed {
        std::fs::rename(target, &aside)?;
    }
    if let Err(error) = std::fs::rename(new, target) {
        if existed {
            let _ = std::fs::rename(&aside, target);
        }
        return Err(error);
    }
    Ok(existed.then_some(aside))
}

/// Removes the `.<name>.old` files an update renamed aside, for the files of
/// `dir` that replaced them.
pub(super) fn clean_up(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        remove(&aside(&entry.path()));
    }
}

#[cfg(test)]
mod tests {
    use super::super::STAGING;
    use super::*;
    use crate::archive::tests::{tar_gz, zip};
    use crate::verify::tests::tempdir;

    const EXE: &str = if cfg!(windows) {
        "levare.exe"
    } else {
        "levare"
    };
    const LIB: &str = if cfg!(windows) {
        "sfizz.dll"
    } else {
        "libsfizz.so"
    };

    /// An installed folder with the executable and the library saying
    /// `version`, and a score the user keeps there.
    fn installed(dir: &Path, version: &str) -> PathBuf {
        let folder = dir.join("Levare");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(EXE), format!("exe {version}")).unwrap();
        std::fs::write(folder.join(LIB), format!("lib {version}")).unwrap();
        std::fs::write(folder.join("mine.partitura"), "score").unwrap();
        folder.join(EXE)
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// The release archive for this OS, `files` inside its top folder.
    fn release(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
        let (archive, top) = if cfg!(windows) {
            (
                dir.join("levare-v0.3.0-x86_64-pc-windows-msvc.zip"),
                "Levare",
            )
        } else {
            (
                dir.join("levare-v0.3.0-x86_64-unknown-linux-gnu.tar.gz"),
                "levare",
            )
        };
        let files: Vec<(String, &str)> = files
            .iter()
            .map(|(name, contents)| (format!("{top}/{name}"), *contents))
            .collect();
        let files: Vec<(&str, &str)> = files.iter().map(|(n, c)| (n.as_str(), *c)).collect();
        if cfg!(windows) {
            zip(&archive, &files);
        } else {
            tar_gz(&archive, &files);
        }
        archive
    }

    #[test]
    fn installs_the_executable_and_the_library() {
        let dir = tempdir::Dir::new();
        let exe = installed(dir.path(), "old");
        let folder = exe.parent().unwrap();
        let archive = release(
            dir.path(),
            &[(EXE, "exe new"), (LIB, "lib new"), ("README.txt", "readme")],
        );
        assert_eq!(Installation::folder(&exe).install(&archive).unwrap(), exe);
        assert_eq!(read(&exe), "exe new");
        assert_eq!(read(&folder.join(LIB)), "lib new");
        assert_eq!(read(&folder.join("README.txt")), "readme");
        assert_eq!(read(&folder.join("mine.partitura")), "score");
        let mut expected = vec![EXE, LIB, "README.txt", "mine.partitura"];
        expected.sort();
        assert_eq!(names(folder), expected);
    }

    #[test]
    fn installs_from_either_archive_format() {
        for format in ["zip", "tar.gz"] {
            let dir = tempdir::Dir::new();
            let exe = installed(dir.path(), "old");
            let archive = dir.path().join(format!("update.{format}"));
            let files = [("levare/levare", "exe new"), ("levare/libsfizz.so", "new")];
            if format == "zip" {
                zip(&archive, &files);
            } else {
                tar_gz(&archive, &files);
            }
            let exe = exe.with_file_name("levare");
            std::fs::write(&exe, "exe old").unwrap();
            Installation::folder(&exe).install(&archive).unwrap();
            assert_eq!(read(&exe), "exe new", "{format}");
        }
    }

    #[test]
    fn keeps_the_installation_when_the_archive_lacks_the_executable() {
        let dir = tempdir::Dir::new();
        let exe = installed(dir.path(), "old");
        let folder = exe.parent().unwrap();
        let before = names(folder);
        let archive = release(dir.path(), &[(LIB, "lib new")]);
        let err = Installation::folder(&exe).install(&archive).unwrap_err();
        assert!(
            matches!(&err, Error::Install(m) if m.contains("no trae")),
            "{err:?}"
        );
        assert_eq!(read(&exe), "exe old");
        assert_eq!(read(&folder.join(LIB)), "lib old");
        assert_eq!(names(folder), before);
    }

    #[test]
    fn puts_everything_back_when_a_file_cannot_move() {
        let dir = tempdir::Dir::new();
        let exe = installed(dir.path(), "old");
        let folder = exe.parent().unwrap();
        let before = names(folder);
        let payload = dir.path().join("payload");
        std::fs::create_dir_all(&payload).unwrap();
        std::fs::write(payload.join(EXE), "exe new").unwrap();
        std::fs::write(payload.join(LIB), "lib new").unwrap();
        // The executable and the library move; the third is missing.
        let names_in: Vec<OsString> = [EXE, LIB, "missing"].map(OsString::from).to_vec();
        let err = swap(folder, &payload, &names_in).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
        assert_eq!(read(&exe), "exe old");
        assert_eq!(read(&folder.join(LIB)), "lib old");
        assert_eq!(names(folder), before);
    }

    #[test]
    fn cleans_up_what_an_update_left() {
        let dir = tempdir::Dir::new();
        let exe = installed(dir.path(), "new");
        let folder = exe.parent().unwrap();
        std::fs::write(folder.join(format!(".{EXE}.old")), "exe old").unwrap();
        std::fs::write(folder.join(format!(".{LIB}.old")), "lib old").unwrap();
        std::fs::create_dir_all(folder.join(STAGING)).unwrap();
        // Not Levare's: kept.
        std::fs::write(folder.join(".other.old"), "").unwrap();
        Installation::folder(&exe).clean_up();
        let mut expected = vec![EXE, LIB, "mine.partitura", ".other.old"];
        expected.sort();
        assert_eq!(names(folder), expected);
    }

    #[test]
    fn a_cargo_build_does_not_update_itself() {
        let dir = tempdir::Dir::new();
        for profile in [
            "target/debug",
            "target/release",
            "target/x86_64-pc-windows-msvc/release",
        ] {
            assert!(is_cargo_build(&dir.path().join(profile)), "{profile}");
        }
        let custom = dir.path().join("out/custom");
        std::fs::create_dir_all(&custom).unwrap();
        std::fs::write(custom.join(".cargo-lock"), "").unwrap();
        assert!(is_cargo_build(&custom));
        assert!(!is_cargo_build(&dir.path().join("Levare")));
        assert!(!is_cargo_build(&dir.path().join("Apps/levare")));
    }

    #[test]
    fn a_writable_folder_can_be_updated() {
        let dir = tempdir::Dir::new();
        check_writable(dir.path()).unwrap();
        assert!(names(dir.path()).is_empty());
        let err = check_writable(&dir.path().join("missing")).unwrap_err();
        assert!(
            matches!(&err, Error::Install(m) if m.contains("no puede escribir")),
            "{err:?}"
        );
    }
}
