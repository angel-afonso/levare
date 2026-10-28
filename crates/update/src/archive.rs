//! Unpacks the Windows (`.zip`) and Linux (`.tar.gz`) release archives.
//! Only regular files and folders come out: links and other special entries
//! are skipped, and so is any path that would land outside the folder.

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use crate::Error;

/// Unpacks `archive` into `dir`, by its extension.
pub(crate) fn extract(archive: &Path, dir: &Path) -> Result<(), Error> {
    let name = archive
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive, dir)
    } else if name.ends_with(".zip") {
        extract_zip(archive, dir)
    } else {
        Err(Error::Install(format!(
            "no se sabe descomprimir {}",
            archive.display()
        )))
    }
}

fn invalid(cause: impl std::fmt::Display) -> Error {
    Error::Install(format!("no se pudo descomprimir la actualización: {cause}"))
}

fn extract_tar_gz(archive: &Path, dir: &Path) -> Result<(), Error> {
    let gz = flate2::read::GzDecoder::new(BufReader::new(File::open(archive)?));
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries().map_err(invalid)? {
        let mut entry = entry.map_err(invalid)?;
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            continue;
        }
        // `unpack_in` refuses paths that leave `dir` and keeps the
        // executable bit.
        entry.unpack_in(dir).map_err(invalid)?;
    }
    Ok(())
}

fn extract_zip(archive: &Path, dir: &Path) -> Result<(), Error> {
    let mut zip = zip::ZipArchive::new(BufReader::new(File::open(archive)?)).map_err(invalid)?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).map_err(invalid)?;
        let Some(relative) = file.enclosed_name() else {
            continue;
        };
        if file.is_symlink() {
            continue;
        }
        let path = dir.join(relative);
        if file.is_dir() {
            std::fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = File::create(&path)?;
        std::io::copy(&mut file, &mut out).map_err(invalid)?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::Write;
    use std::path::PathBuf;

    use super::*;
    use crate::verify::tests::tempdir;

    /// A `.tar.gz` of `files` (path, contents), as the Linux release packs it.
    pub(crate) fn tar_gz(path: &Path, files: &[(&str, &str)]) {
        let gz =
            flate2::write::GzEncoder::new(File::create(path).unwrap(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        for (name, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append_data(&mut header, name, contents.as_bytes())
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    /// A `.zip` of `files` (path, contents), as the Windows release packs it.
    pub(crate) fn zip(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .unix_permissions(0o755);
        for (name, contents) in files {
            zip.start_file(*name, options).unwrap();
            zip.write_all(contents.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    fn read(path: PathBuf) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn unpacks_both_formats() {
        let dir = tempdir::Dir::new();
        let files = [("levare/levare", "exe"), ("levare/lib/x.so", "lib")];
        tar_gz(&dir.path().join("a.tar.gz"), &files);
        zip(&dir.path().join("a.zip"), &files);
        for archive in ["a.tar.gz", "a.zip"] {
            let out = dir.path().join(format!("out-{archive}"));
            std::fs::create_dir_all(&out).unwrap();
            extract(&dir.path().join(archive), &out).unwrap();
            assert_eq!(read(out.join("levare/levare")), "exe", "{archive}");
            assert_eq!(read(out.join("levare/lib/x.so")), "lib", "{archive}");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(out.join("levare/levare"))
                    .unwrap()
                    .permissions()
                    .mode();
                assert_eq!(mode & 0o111, 0o111, "{archive} keeps the executable bit");
            }
        }
    }

    #[test]
    fn skips_paths_that_leave_the_folder() {
        let dir = tempdir::Dir::new();
        let archive = dir.path().join("a.zip");
        zip(&archive, &[("../escaped", "x"), ("levare/levare", "exe")]);
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        extract(&archive, &out).unwrap();
        assert!(!dir.path().join("escaped").exists());
        assert!(out.join("levare/levare").exists());
    }

    #[test]
    fn refuses_an_unknown_format_or_a_damaged_archive() {
        let dir = tempdir::Dir::new();
        let out = dir.path();
        std::fs::write(out.join("a.dmg"), "x").unwrap();
        std::fs::write(out.join("b.zip"), "not a zip").unwrap();
        std::fs::write(out.join("c.tar.gz"), "not gzip").unwrap();
        for name in ["a.dmg", "b.zip", "c.tar.gz"] {
            assert!(
                matches!(extract(&out.join(name), out), Err(Error::Install(_))),
                "{name}"
            );
        }
    }
}
