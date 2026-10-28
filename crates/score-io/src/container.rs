//! The `.levare` document: a ZIP whose first entry, `mimetype`, is stored
//! uncompressed so the file identifies itself, and whose `score.json` holds the
//! same versioned envelope as the older `.partitura` JSON files. Unknown entries
//! are ignored, so later versions can add to the container without breaking it.
use crate::{FileError, MAX_FILE_BYTES, error};
use std::io::{Cursor, Read, Write};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

pub const EXTENSION: &str = "levare";
pub const MIME_TYPE: &str = "application/vnd.levare.score";
/// Compressed, the largest score the model allows takes a few MiB.
pub const MAX_CONTAINER_BYTES: usize = 64 * 1024 * 1024;
const MAGIC: &[u8] = b"PK\x03\x04";
const SCORE: &str = "score.json";
const MAX_ENTRIES: usize = 64;

pub(crate) fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

/// A ZIP is a Levare document when its `mimetype` says so. Compressed
/// MusicXML also starts with a `mimetype` entry, naming MusicXML instead.
pub(crate) fn is_levare(bytes: &[u8]) -> bool {
    is_zip(bytes)
        && ZipArchive::new(Cursor::new(bytes))
            .ok()
            .and_then(|mut zip| read_mime_type(&mut zip).ok())
            .is_some_and(|mime| mime == MIME_TYPE)
}

pub(crate) fn pack(json: &[u8]) -> Result<Vec<u8>, FileError> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("mimetype", stored).map_err(error)?;
    zip.write_all(MIME_TYPE.as_bytes()).map_err(error)?;
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(SCORE, deflated).map_err(error)?;
    zip.write_all(json).map_err(error)?;
    let bytes = zip.finish().map_err(error)?.into_inner();
    if bytes.len() > MAX_CONTAINER_BYTES {
        return Err(error("La partitura supera el límite de 64 MiB"));
    }
    Ok(bytes)
}

/// Extracts `score.json` in memory, never reading more than the native limit
/// whatever the entry's header claims.
pub(crate) fn unpack(bytes: &[u8]) -> Result<Vec<u8>, FileError> {
    if bytes.len() > MAX_CONTAINER_BYTES {
        return Err(error("El archivo supera el límite de 64 MiB"));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(error)?;
    if zip.len() > MAX_ENTRIES {
        return Err(error("El archivo .levare contiene demasiadas entradas"));
    }
    if read_mime_type(&mut zip)? != MIME_TYPE {
        return Err(error("El archivo no es una partitura de Levare"));
    }
    let mut entry = zip
        .by_name(SCORE)
        .map_err(|_| error("El archivo .levare está dañado: no contiene la partitura"))?;
    if entry.size() > MAX_FILE_BYTES as u64 {
        return Err(error(
            "La partitura descomprimida supera el límite de 256 MiB",
        ));
    }
    let mut json = Vec::new();
    entry
        .by_ref()
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut json)
        .map_err(error)?;
    if json.len() > MAX_FILE_BYTES {
        return Err(error(
            "La partitura descomprimida supera el límite de 256 MiB",
        ));
    }
    Ok(json)
}

fn read_mime_type(zip: &mut ZipArchive<Cursor<&[u8]>>) -> Result<String, FileError> {
    let entry = zip
        .by_name("mimetype")
        .map_err(|_| error("El archivo no es una partitura de Levare"))?;
    let mut mime = String::new();
    entry
        .take(256)
        .read_to_string(&mut mime)
        .map_err(|_| error("El archivo no es una partitura de Levare"))?;
    Ok(mime.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode, encode, encode_file, open, save, tests::orchestral};

    fn container(entries: &[(&str, &[u8], CompressionMethod)]) -> Vec<u8> {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, method) in entries {
            zip.start_file(
                *name,
                SimpleFileOptions::default().compression_method(*method),
            )
            .unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn a_large_orchestral_score_round_trips_compressed() {
        let score = orchestral(420);
        let bytes = encode_file(&score).unwrap();
        assert!(bytes.starts_with(b"PK\x03\x04"));
        // The stored `mimetype` comes first, readable without unzipping.
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(&bytes[38..38 + MIME_TYPE.len()], MIME_TYPE.as_bytes());
        assert!(bytes.len() * 20 < encode(&score).unwrap().len());
        assert_eq!(decode(&bytes).unwrap(), score);
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tutti.levare");
        save(&path, &score).unwrap();
        let opened = open(&path).unwrap();
        assert!(!opened.imported);
        assert_eq!(opened.score, score);
    }

    #[test]
    fn unknown_entries_are_ignored() {
        let json = encode(&orchestral(2)).unwrap();
        let bytes = container(&[
            ("mimetype", MIME_TYPE.as_bytes(), CompressionMethod::Stored),
            ("thumbnail.png", b"later", CompressionMethod::Stored),
            (SCORE, &json, CompressionMethod::Deflated),
            ("notes/extra.json", b"{}", CompressionMethod::Deflated),
        ]);
        assert_eq!(decode(&bytes).unwrap(), orchestral(2));
    }

    #[test]
    fn rejects_a_container_without_a_score_or_with_an_oversized_one() {
        let missing = container(&[("mimetype", MIME_TYPE.as_bytes(), CompressionMethod::Stored)]);
        let message = decode(&missing).unwrap_err().to_string();
        assert!(message.contains("no contiene la partitura"), "{message}");
        // Zeros deflate about a thousandfold: a small file that would expand
        // beyond the limit is refused without being expanded in full.
        let bomb = vec![b' '; MAX_FILE_BYTES + 1];
        let bytes = container(&[
            ("mimetype", MIME_TYPE.as_bytes(), CompressionMethod::Stored),
            (SCORE, &bomb, CompressionMethod::Deflated),
        ]);
        drop(bomb);
        assert!(bytes.len() < MAX_CONTAINER_BYTES);
        let message = decode(&bytes).unwrap_err().to_string();
        assert!(message.contains("256 MiB"), "{message}");
        let many: Vec<_> = (0..=MAX_ENTRIES).map(|i| format!("{i}")).collect();
        let mut entries = vec![("mimetype", MIME_TYPE.as_bytes(), CompressionMethod::Stored)];
        entries.extend(
            many.iter()
                .map(|name| (name.as_str(), &b""[..], CompressionMethod::Stored)),
        );
        assert!(decode(&container(&entries)).is_err());
    }

    #[test]
    fn compressed_musicxml_is_still_imported() {
        assert!(!is_levare(&container(&[(
            "mimetype",
            b"application/vnd.recordare.musicxml",
            CompressionMethod::Stored
        )])));
        assert!(is_levare(&encode_file(&orchestral(1)).unwrap()));
    }
}
