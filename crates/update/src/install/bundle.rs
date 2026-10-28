//! macOS: the archive holds `Levare.app`, which replaces the installed
//! bundle as a whole once its code signature checks out.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Installation, aside, remove, writable};
use crate::Error;

/// What the release archive calls the app.
const PAYLOAD: &str = "Levare.app";

/// The bundle the running executable is in.
pub(super) fn current() -> Result<Installation, Error> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let bundle = exe
        .ancestors()
        .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or_else(|| Error::Install("Levare no se está ejecutando desde Levare.app".into()))?;
    Ok(Installation::app_bundle(bundle))
}

pub(super) fn install(bundle: &Path, archive: &Path, staging: &Path) -> Result<PathBuf, Error> {
    extract(archive, staging)?;
    let payload = find(staging, PAYLOAD.as_ref())?;
    check_code_signature(&payload, bundle)?;
    replace(bundle, &payload, &aside(bundle))?;
    Ok(bundle.to_path_buf())
}

/// `name` at the top of `dir` or one folder down (the archive wraps the app
/// in a folder).
fn find(dir: &Path, name: &std::ffi::OsStr) -> Result<PathBuf, Error> {
    let direct = dir.join(name);
    if direct.exists() {
        return Ok(direct);
    }
    for entry in std::fs::read_dir(dir)? {
        let candidate = entry?.path().join(name);
        if candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(Error::Install(format!(
        "la actualización no trae {}",
        name.to_string_lossy()
    )))
}

/// Unpacks the `.zip` into `dir`; ditto keeps the bundle's symlinks,
/// permissions and signature.
fn extract(archive: &Path, dir: &Path) -> Result<(), Error> {
    let mut command = Command::new("/usr/bin/ditto");
    command.arg("-x").arg("-k").arg(archive).arg(dir);
    run(&mut command, "descomprimir la actualización")
}

/// Moves `new` to `target`, moving what was there to `aside` first and back
/// if the move fails, then removes the replaced app.
fn replace(target: &Path, new: &Path, aside: &Path) -> Result<(), Error> {
    remove(aside);
    std::fs::rename(target, aside).map_err(|e| writable(e, target))?;
    if let Err(error) = std::fs::rename(new, target) {
        let _ = std::fs::rename(aside, target);
        return Err(writable(error, target));
    }
    remove(aside);
    Ok(())
}

/// The new app must be validly signed, and by the same team as the running
/// one when that one has a team (an ad-hoc signed build has none).
fn check_code_signature(new: &Path, current: &Path) -> Result<(), Error> {
    let mut verify = Command::new("/usr/bin/codesign");
    verify.args(["--verify", "--deep", "--strict"]).arg(new);
    run(&mut verify, "verificar la firma de código de la app nueva")?;
    let Some(team) = team_identifier(current)? else {
        return Ok(());
    };
    match team_identifier(new)? {
        Some(new_team) if new_team == team => Ok(()),
        other => Err(Error::Install(format!(
            "la app nueva está firmada por {}, no por {team}",
            other.as_deref().unwrap_or("ningún equipo")
        ))),
    }
}

/// The Team ID in `app`'s signature, `None` when unsigned or ad hoc.
fn team_identifier(app: &Path) -> Result<Option<String>, Error> {
    let output = Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=2"])
        .arg(app)
        .output()?;
    // codesign prints the details on stderr, and fails for unsigned code.
    if !output.status.success() {
        return Ok(None);
    }
    Ok(parse_team_identifier(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

fn parse_team_identifier(details: &str) -> Option<String> {
    details
        .lines()
        .find_map(|line| line.strip_prefix("TeamIdentifier="))
        .map(str::trim)
        .filter(|team| !team.is_empty() && *team != "not set")
        .map(str::to_string)
}

fn run(command: &mut Command, what: &str) -> Result<(), Error> {
    let output = command.output()?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::Install(format!(
        "no se pudo {what}: {}",
        stderr.trim()
    )))
}

#[cfg(test)]
mod tests {
    use super::super::STAGING;
    use super::*;
    use crate::verify::tests::tempdir;

    /// A minimal app bundle whose `Contents/Resources/version` says `contents`, signed
    /// ad hoc when `sign`.
    fn bundle(path: &Path, contents: &str, sign: bool) {
        std::fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
        std::fs::create_dir_all(path.join("Contents/Resources")).unwrap();
        std::fs::write(path.join("Contents/Resources/version"), contents).unwrap();
        if !sign {
            return;
        }
        std::fs::write(
            path.join("Contents/Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>levare</string>
<key>CFBundleIdentifier</key><string>dev.partitura.editor.test</string>
<key>CFBundlePackageType</key><string>APPL</string>
</dict></plist>
"#,
        )
        .unwrap();
        std::fs::copy("/usr/bin/true", path.join("Contents/MacOS/levare")).unwrap();
        run(
            Command::new("/usr/bin/codesign")
                .args(["--force", "--sign", "-"])
                .arg(path),
            "sign the test bundle",
        )
        .unwrap();
    }

    fn version_of(bundle: &Path) -> String {
        std::fs::read_to_string(bundle.join("Contents/Resources/version")).unwrap()
    }

    /// An archive like the release's, made with the same tool: the app in a
    /// `levare-<version>-<target>` folder.
    fn release_archive(dir: &Path, name: &str, contents: &str, sign: bool) -> PathBuf {
        let folder = dir.join("source/levare-v0.3.0-universal-apple-darwin");
        bundle(&folder.join(name), contents, sign);
        let archive = dir.join("archive.zip");
        run(
            Command::new("/usr/bin/ditto")
                .args(["-c", "-k", "--keepParent"])
                .arg(&folder)
                .arg(&archive),
            "pack the test archive",
        )
        .unwrap();
        archive
    }

    #[test]
    fn replaces_a_bundle_and_removes_the_old_one() {
        let dir = tempdir::Dir::new();
        let (target, new, aside) = (
            dir.path().join("Levare.app"),
            dir.path().join("stage/Levare.app"),
            dir.path().join(".Levare.app.old"),
        );
        bundle(&target, "old", false);
        bundle(&new, "new", false);
        replace(&target, &new, &aside).unwrap();
        assert_eq!(version_of(&target), "new");
        assert!(!new.exists() && !aside.exists());
    }

    #[test]
    fn puts_the_old_app_back_when_the_new_one_cannot_move() {
        let dir = tempdir::Dir::new();
        let (target, aside) = (
            dir.path().join("Levare.app"),
            dir.path().join(".Levare.app.old"),
        );
        bundle(&target, "old", false);
        let err = replace(&target, &dir.path().join("missing"), &aside).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err:?}");
        assert_eq!(version_of(&target), "old");
        assert!(!aside.exists());
    }

    #[test]
    fn finds_the_app_inside_the_archive_folder() {
        let dir = tempdir::Dir::new();
        let folder = dir.path().join("levare-v0.3.0-universal-apple-darwin");
        std::fs::create_dir_all(folder.join("Levare.app")).unwrap();
        std::fs::write(dir.path().join("README.md"), "").unwrap();
        assert_eq!(
            find(dir.path(), "Levare.app".as_ref()).unwrap(),
            folder.join("Levare.app")
        );
        assert!(matches!(
            find(dir.path(), "Other.app".as_ref()),
            Err(Error::Install(_))
        ));
    }

    #[test]
    fn reads_the_team_identifier() {
        let signed = "Executable=/Applications/Levare.app/Contents/MacOS/levare\n\
                      Authority=Developer ID Application: Someone (C6GRS8VQVL)\n\
                      TeamIdentifier=C6GRS8VQVL\n";
        assert_eq!(parse_team_identifier(signed).as_deref(), Some("C6GRS8VQVL"));
        assert_eq!(
            parse_team_identifier("Signature=adhoc\nTeamIdentifier=not set\n"),
            None
        );
        assert_eq!(parse_team_identifier("Signature=adhoc\n"), None);
    }

    #[test]
    fn installs_a_signed_app_from_an_archive() {
        let dir = tempdir::Dir::new();
        let installed = dir.path().join("Applications/Levare.app");
        bundle(&installed, "old", true);
        let archive = release_archive(dir.path(), "Levare.app", "new", true);
        let installation = Installation::app_bundle(&installed);
        assert_eq!(installation.install(&archive).unwrap(), installed);
        assert_eq!(version_of(&installed), "new");
        // Nothing else is left next to it.
        let left: Vec<_> = std::fs::read_dir(installed.parent().unwrap())
            .unwrap()
            .collect();
        assert_eq!(left.len(), 1);
    }

    #[test]
    fn refuses_an_unsigned_app() {
        let dir = tempdir::Dir::new();
        let installed = dir.path().join("Applications/Levare.app");
        bundle(&installed, "old", false);
        let archive = release_archive(dir.path(), "Levare.app", "new", false);
        let err = Installation::app_bundle(&installed)
            .install(&archive)
            .unwrap_err();
        assert!(
            matches!(&err, Error::Install(m) if m.contains("firma de código")),
            "{err:?}"
        );
        assert_eq!(version_of(&installed), "old");
        assert!(!installed.parent().unwrap().join(STAGING).exists());
    }

    #[test]
    fn keeps_the_installed_app_when_the_archive_lacks_it() {
        let dir = tempdir::Dir::new();
        let installed = dir.path().join("Applications/Levare.app");
        bundle(&installed, "old", false);
        let archive = release_archive(dir.path(), "Other.app", "new", true);
        let err = Installation::app_bundle(&installed)
            .install(&archive)
            .unwrap_err();
        assert!(matches!(err, Error::Install(_)), "{err:?}");
        assert_eq!(version_of(&installed), "old");
    }

    #[test]
    fn cleans_up_what_an_update_left() {
        let dir = tempdir::Dir::new();
        let installed = dir.path().join("Levare.app");
        bundle(&installed, "", false);
        std::fs::create_dir_all(dir.path().join(STAGING)).unwrap();
        std::fs::create_dir_all(dir.path().join(".Levare.app.old")).unwrap();
        Installation::app_bundle(&installed).clean_up();
        let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(left.len(), 1);
        assert!(installed.exists());
    }
}
