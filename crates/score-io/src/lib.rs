//! Versioned native documents. Export formats will use independent adapters.
#[cfg(test)]
mod breaks_tests;
#[cfg(test)]
mod clef_tests;
#[cfg(test)]
mod comment_tests;
mod container;
mod error;
pub mod musicxml;
mod names;
#[cfg(test)]
mod range_tests;
mod recent;
mod recovery;
pub use container::{EXTENSION, MAX_CONTAINER_BYTES, MIME_TYPE};
use error::error;
pub use error::{FileError, FileErrorKind};
pub use names::file_name;
pub use recent::RecentFiles;
pub use recovery::{
    RecoveredScore, Recovery, load_recovery, save_emergency_copy, set_aside_recovery,
};
use score_core::Score;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::Path};

pub const SCHEMA_VERSION: u32 = 29;
/// Enough for the largest score the model allows (`MAX_PARTS` parts of
/// `MAX_MEASURES` bars at orchestral density is about 60 MiB of compact JSON),
/// including the indented JSON that older versions wrote.
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct NativeFile {
    format: String,
    version: u32,
    score: Score,
}

/// The versioned JSON envelope, as stored inside a `.levare` file and in the
/// recovery file.
pub fn encode(score: &Score) -> Result<Vec<u8>, FileError> {
    score.validate()?;
    // Compact: indentation alone made orchestral scores four times larger.
    let bytes = serde_json::to_vec(&NativeFile {
        format: "partitura".into(),
        version: SCHEMA_VERSION,
        score: score.clone(),
    })
    .map_err(FileError::internal)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(FileError::too_large(
            "La partitura supera el límite de 256 MiB",
        ));
    }
    Ok(bytes)
}

/// A compressed `.levare` document, as Save writes it.
pub fn encode_file(score: &Score) -> Result<Vec<u8>, FileError> {
    container::pack(&encode(score)?)
}

/// Reads a `.levare` document or the JSON of an older `.partitura` file,
/// recognised by their contents rather than their extension.
pub fn decode(bytes: &[u8]) -> Result<Score, FileError> {
    if container::is_zip(bytes) {
        decode_json(&container::unpack(bytes)?)
    } else {
        decode_json(bytes)
    }
}

fn decode_json(bytes: &[u8]) -> Result<Score, FileError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(FileError::too_large(
            "El archivo supera el límite de 256 MiB",
        ));
    }
    // Check the envelope before deserializing a possibly incompatible score.
    let mut envelope: serde_json::Value =
        serde_json::from_slice(bytes).map_err(FileError::damaged)?;
    if envelope["format"] != "partitura" {
        return Err(FileError::damaged("missing \"format\": \"partitura\""));
    }
    if let Some(version) = envelope["version"]
        .as_u64()
        .filter(|&version| version > u64::from(SCHEMA_VERSION))
    {
        return Err(FileError::newer(version));
    }
    if envelope["version"].as_u64() == Some(1) {
        let score = envelope["score"]
            .as_object_mut()
            .ok_or_else(|| error("Partitura inválida"))?;
        let measures = score
            .remove("measures")
            .ok_or_else(|| error("Compases inválidos"))?;
        score.insert(
            "parts".into(),
            serde_json::json!([{ "id": 0, "instrument": "Piano", "measures": measures }]),
        );
        score.insert("next_part_id".into(), 1.into());
        envelope["version"] = 2.into();
    }
    if envelope["version"].as_u64() == Some(2) {
        let parts = envelope
            .get_mut("score")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|score| score.get_mut("parts"))
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| error("Instrumentos inválidos"))?;
        if parts.is_empty() || parts.len() > score_core::MAX_PARTS {
            return Err(error("Cantidad de instrumentos inválida"));
        }
        for part in parts {
            let part = part
                .as_object_mut()
                .ok_or_else(|| error("Instrumento inválido"))?;
            let measures = part
                .remove("measures")
                .ok_or_else(|| error("Compases inválidos"))?;
            let count = measures
                .as_array()
                .ok_or_else(|| error("Compases inválidos"))?
                .len();
            if !(1..=score_core::MAX_MEASURES).contains(&count) {
                return Err(error("Cantidad de compases inválida"));
            }
            let mut staves = vec![serde_json::json!({"id": "Upper", "measures": measures})];
            if part
                .get("instrument")
                .is_some_and(|instrument| instrument == "Piano")
            {
                let empty = serde_json::to_value(score_core::Measure::default())
                    .map_err(FileError::internal)?;
                staves.push(serde_json::json!({"id": "Lower", "measures": vec![empty; count]}));
            }
            part.insert("staves".into(), staves.into());
        }
        envelope["version"] = 3.into();
    }
    if envelope["version"].as_u64() == Some(3) {
        let parts = envelope
            .get_mut("score")
            .and_then(serde_json::Value::as_object_mut)
            .and_then(|score| score.get_mut("parts"))
            .and_then(serde_json::Value::as_array_mut)
            .ok_or_else(|| error("Instrumentos inválidos"))?;
        for part in parts {
            let staves = part
                .get_mut("staves")
                .and_then(serde_json::Value::as_array_mut)
                .ok_or_else(|| error("Pentagramas inválidos"))?;
            for staff in staves {
                let measures = staff
                    .get_mut("measures")
                    .and_then(serde_json::Value::as_array_mut)
                    .ok_or_else(|| error("Compases inválidos"))?;
                for measure in measures {
                    let events = measure
                        .get_mut("events")
                        .and_then(serde_json::Value::as_array_mut)
                        .ok_or_else(|| error("Notas inválidas"))?;
                    for slot in events {
                        if let Some(value) = slot.get_mut("value")
                            && !value.is_null()
                        {
                            let event = value
                                .as_object_mut()
                                .ok_or_else(|| error("Nota inválida"))?;
                            let pitch = event
                                .remove("pitch")
                                .ok_or_else(|| error("Altura de nota inválida"))?;
                            let pitches = if pitch.is_null() {
                                Vec::new()
                            } else {
                                vec![pitch]
                            };
                            event.insert("pitches".into(), pitches.into());
                        }
                    }
                }
            }
        }
        envelope["version"] = 4.into();
    }
    if envelope["version"].as_u64() == Some(4) {
        // Missing accidental means the original natural pitch.
        envelope["version"] = 5.into();
    }
    if envelope["version"].as_u64() == Some(5) {
        // Older scores used natural pitches unless explicitly altered: C major
        // preserves every existing note, chord and event identity.
        envelope["score"]
            .as_object_mut()
            .ok_or_else(|| error("Partitura inválida"))?
            .insert(
                "key_signature".into(),
                serde_json::to_value(score_core::KeySignature::default())
                    .map_err(FileError::internal)?,
            );
        envelope["version"] = 6.into();
    }
    if envelope["version"].as_u64() == Some(6) {
        // Expressive marks default to absent in older files.
        envelope["version"] = 7.into();
    }
    if envelope["version"].as_u64() == Some(7) {
        // Missing technique keeps the original bowed instrument sound.
        envelope["version"] = 8.into();
    }
    if envelope["version"].as_u64() == Some(8) {
        // Existing connections remain unchanged; regulators are new connection kinds.
        envelope["version"] = 9.into();
    }
    if envelope["version"].as_u64() == Some(9) {
        // Ordinary duration strings retain their meaning; ticks are not serialized.
        envelope["version"] = 10.into();
    }
    if envelope["version"].as_u64() == Some(10) {
        // Missing per-bar signatures retain the original global meter and key.
        envelope["version"] = 11.into();
    }
    if envelope["version"].as_u64() == Some(11) {
        // Missing written tempo defaults to quarter = 120, without later changes.
        envelope["version"] = 12.into();
    }
    if envelope["version"].as_u64() == Some(12) {
        // Arpeggios are optional; older notes remain simultaneous.
        envelope["version"] = 13.into();
    }
    if envelope["version"].as_u64() == Some(13) {
        // Existing arpeggios keep their single-staff scope.
        envelope["version"] = 14.into();
    }
    if envelope["version"].as_u64() == Some(14) {
        // Empty second voices and undivided parts preserve every previous document.
        envelope["version"] = 15.into();
    }
    if envelope["version"].as_u64() == Some(15) {
        // Empty passage maps retain the existing whole-part presentation.
        envelope["version"] = 16.into();
    }
    if envelope["version"].as_u64() == Some(16) {
        if let Some(parts) = envelope
            .get_mut("score")
            .and_then(|v| v.get_mut("parts"))
            .and_then(|v| v.as_array_mut())
        {
            for part in parts {
                if let Some(changes) = part
                    .get_mut("divisi_changes")
                    .and_then(|v| v.as_object_mut())
                {
                    for value in changes.values_mut() {
                        *value = serde_json::json!({"0": value.clone()});
                    }
                }
            }
        }
        envelope["version"] = 17.into();
    }
    if matches!(envelope["version"].as_u64(), Some(17 | 18)) {
        // Harmonics default to absent in earlier documents.
        envelope["version"] = 19.into();
    }
    if envelope["version"].as_u64() == Some(19) {
        // Form defaults to absent in earlier documents.
        envelope["version"] = 20.into();
    }
    if envelope["version"].as_u64() == Some(20) {
        // Tremolos default to absent in earlier documents.
        envelope["version"] = 21.into();
    }
    if envelope["version"].as_u64() == Some(21) {
        // Rehearsal marks default to absent; letters remain the default style.
        envelope["version"] = 22.into();
    }
    if envelope["version"].as_u64() == Some(22) {
        // Older parts inherit the instrument's clef, with no explicit overrides.
        envelope["version"] = 23.into();
    }
    if envelope["version"].as_u64() == Some(23) {
        // Marks default to their engraved position.
        envelope["version"] = 24.into();
    }
    if envelope["version"].as_u64() == Some(24) {
        // Bar marks default to their engraved position.
        envelope["version"] = 25.into();
    }
    if envelope["version"].as_u64() == Some(25) {
        // Parts without a custom advisory range use the instrument defaults.
        envelope["version"] = 26.into();
    }
    if envelope["version"].as_u64() == Some(26) {
        // Older scores have no review comments.
        envelope["version"] = 27.into();
    }
    if envelope["version"].as_u64() == Some(27) {
        // Older scores have no layout breaks.
        envelope["version"] = 28.into();
    }
    if envelope["version"].as_u64() == Some(28) {
        // Grace notes, octave lines, pedal, trills, glissandi and staff texts
        // default to absent; thirty-seconds did not exist.
        envelope["version"] = SCHEMA_VERSION.into();
    }
    if envelope["version"].as_u64() != Some(u64::from(SCHEMA_VERSION)) {
        return Err(error("Versión de archivo no compatible"));
    }
    let file: NativeFile = serde_json::from_value(envelope).map_err(FileError::damaged)?;
    file.score.validate()?;
    Ok(file.score)
}

pub fn load(path: &Path) -> Result<Score, FileError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

pub struct OpenedScore {
    pub score: Score,
    pub imported: bool,
    pub warnings: Vec<String>,
}

/// Detect a `.levare` document, older native JSON, plain MusicXML or
/// compressed MXL by their contents.
/// Imported documents have no native save path; the UI must request a new one.
pub fn open(path: &Path) -> Result<OpenedScore, FileError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take((MAX_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(FileError::too_large(
            "El archivo supera el límite de 256 MiB",
        ));
    }
    if container::is_levare(&bytes)
        || bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) == Some(b'{')
    {
        return Ok(OpenedScore {
            score: decode(&bytes)?,
            imported: false,
            warnings: Vec::new(),
        });
    }
    let result = musicxml::decode(&bytes)?;
    Ok(OpenedScore {
        score: result.score,
        imported: true,
        warnings: result.warnings,
    })
}

/// Write beside the destination, flush, then replace atomically. A failed write
/// never truncates the previous document.
pub fn save(path: &Path, score: &Score) -> Result<(), FileError> {
    let bytes = encode_file(score)?;
    Ok(atomic_write::write(path, &bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_core::{Duration, Event, Pitch, Step};

    #[test]
    fn malformed_form_document_returns_an_error_without_panicking() {
        let mut data: serde_json::Value =
            serde_json::from_slice(&encode(&Score::default()).unwrap()).unwrap();
        data["score"]["parts"] = serde_json::json!([]);
        data["score"]["form"] = serde_json::json!({"0": {}});
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
    }

    #[test]
    fn form_round_trips_and_version_19_files_open() {
        let mut s = Score::default();
        s.set_bar_form(
            1,
            score_core::BarForm {
                repeat_end: Some(2),
                jump: Some(score_core::Jump::DalSegnoAlCoda),
                ..Default::default()
            },
        )
        .unwrap();
        s.set_ending(score_core::Ending {
            start: 1,
            end: 2,
            numbers: vec![1],
        })
        .unwrap();
        assert_eq!(decode(&encode(&s).unwrap()).unwrap(), s);
        let old = decode(include_bytes!("../../../examples/trio.partitura")).unwrap();
        assert_eq!(old.play_order(true), old.play_order(false));
    }

    #[test]
    fn header_credits_round_trip_and_old_files_open_without_them() {
        let mut score = Score::default();
        let credits = score_core::Credits {
            subtitle: "Para cuerdas".into(),
            composer: "Fanny Mendelssohn".into(),
            year: "1846".into(),
            copyright: "© Dominio público".into(),
            ..Default::default()
        };
        score.set_header("Notturno", &credits).unwrap();
        let back = decode(&encode(&score).unwrap()).unwrap();
        assert_eq!(back.credits(), &credits);
        let old = include_bytes!("../../../examples/trio.partitura");
        assert_eq!(
            decode(old).unwrap().credits(),
            &score_core::Credits::default()
        );
    }

    #[test]
    fn old_scores_keep_their_sound_and_key_signatures_round_trip_with_validation() {
        use score_core::{KeyMode, KeySignature};
        for bytes in [
            include_bytes!("../../../examples/primera-melodia.partitura").as_slice(),
            include_bytes!("../../../examples/trio.partitura").as_slice(),
            include_bytes!("../../../examples/piano-dos-manos.partitura").as_slice(),
            include_bytes!("../../../examples/piano-acordes.partitura").as_slice(),
            include_bytes!("../../../examples/piano-alteraciones.partitura").as_slice(),
        ] {
            let mut score = decode(bytes).unwrap();
            assert_eq!(score.key_signature(), KeySignature::default());
            let before = score.clone();
            score.set_key_signature(KeySignature::new(-3, KeyMode::Minor).unwrap());
            let data = encode(&score).unwrap();
            assert_eq!(decode(&data).unwrap(), score);
            for malformed_key in [
                serde_json::json!({"fifths":8,"mode":"Major"}),
                serde_json::json!({"fifths":-8,"mode":"Minor"}),
                serde_json::json!({"fifths":1,"mode":"Invalid"}),
                serde_json::json!({"fifths":1.5,"mode":"Major"}),
                serde_json::json!(null),
            ] {
                let mut invalid: serde_json::Value = serde_json::from_slice(&data).unwrap();
                invalid["score"]["key_signature"] = malformed_key;
                assert!(decode(&serde_json::to_vec(&invalid).unwrap()).is_err());
            }
            score.set_key_signature(KeySignature::default());
            assert_eq!(score, before);
        }
    }

    #[test]
    fn legacy_chords_stay_natural_and_new_accidentals_round_trip() {
        let mut score =
            decode(include_bytes!("../../../examples/piano-acordes.partitura")).unwrap();
        let part = score.parts()[0].id();
        for staff in [score_core::StaffId::Upper, score_core::StaffId::Lower] {
            let event = score.part(part).unwrap().staff(staff).unwrap().measures()[0]
                .id_at(0)
                .unwrap();
            let id = score_core::ElementId {
                part,
                staff,
                measure: 0,
                event,
            };
            assert!(
                score
                    .event(id)
                    .unwrap()
                    .pitches
                    .iter()
                    .all(|p| p.accidental().is_none())
            );
            for (index, sign) in [
                score_core::Accidental::Sharp,
                score_core::Accidental::Flat,
                score_core::Accidental::Natural,
            ]
            .into_iter()
            .enumerate()
            {
                score.set_accidental(id, index, Some(sign)).unwrap();
            }
        }
        let bytes = encode(&score).unwrap();
        assert_eq!(decode(&bytes).unwrap(), score);
        let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(data["version"], SCHEMA_VERSION);
        data["score"]["parts"][0]["staves"][0]["measures"][0]["events"][1]["value"]["pitches"][0]
            ["accidental"] = "DoubleSharp".into();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
    }

    #[test]
    fn chord_files_round_trip_both_hands_and_reject_duplicate_or_oversized_sets() {
        let mut score = decode(include_bytes!(
            "../../../examples/piano-dos-manos.partitura"
        ))
        .unwrap();
        let piano = score.parts()[0].id();
        let mut ids = Vec::new();
        for staff in [score_core::StaffId::Upper, score_core::StaffId::Lower] {
            let event = score.part(piano).unwrap().staff(staff).unwrap().measures()[0]
                .id_at(0)
                .unwrap();
            let id = score_core::ElementId {
                part: piano,
                staff,
                measure: 0,
                event,
            };
            let octave = if staff == score_core::StaffId::Upper {
                4
            } else {
                3
            };
            score
                .add_pitch(id, Pitch::new(Step::E, octave).unwrap())
                .unwrap();
            score
                .add_pitch(id, Pitch::new(Step::G, octave).unwrap())
                .unwrap();
            ids.push(id);
        }
        let bytes = encode(&score).unwrap();
        let loaded = decode(&bytes).unwrap();
        assert_eq!(loaded, score);
        assert!(
            ids.into_iter()
                .all(|id| loaded.event(id).unwrap().pitches.len() == 3)
        );
        let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        for pitches in [
            serde_json::json!([{"step":"C", "octave":4}, {"step":"C", "octave":4}]),
            serde_json::json!(
                (0..9)
                    .map(|octave| serde_json::json!({"step":"C", "octave":octave}))
                    .collect::<Vec<_>>()
            ),
        ] {
            let mut malformed = data.clone();
            malformed["score"]["parts"][0]["staves"][0]["measures"][0]["events"][1]["value"]["pitches"] =
                pitches;
            assert!(decode(&serde_json::to_vec(&malformed).unwrap()).is_err());
        }
    }

    #[test]
    fn malformed_legacy_envelopes_return_errors_instead_of_panicking() {
        for version in [1, 2, 3, 4, 5, 6] {
            for score in [
                serde_json::json!([]),
                serde_json::json!("invalid"),
                serde_json::json!(false),
                serde_json::json!(null),
                serde_json::json!(17),
            ] {
                let bytes = serde_json::to_vec(&serde_json::json!({ "format": "partitura", "version": version, "score": score })).unwrap();
                assert!(decode(&bytes).is_err());
            }
        }
    }

    #[test]
    fn saves_both_hands_and_migrates_legacy_pianos_without_moving_notes() {
        let example = decode(include_bytes!(
            "../../../examples/piano-dos-manos.partitura"
        ))
        .unwrap();
        assert_eq!(example.parts().len(), 1);
        assert!(
            example.parts()[0]
                .staves()
                .iter()
                .all(|staff| !staff.measures()[0].is_empty())
        );
        let mut score = decode(include_bytes!("../../../examples/trio.partitura")).unwrap(); // Schema 2.
        let piano = score.parts()[0].id();
        assert_eq!(score.parts()[0].staves().len(), 2);
        let upper_before = score.parts()[0].measures().clone();
        assert!(
            score.parts()[0]
                .staff(score_core::StaffId::Lower)
                .unwrap()
                .measures()
                .iter()
                .all(|bar| bar.is_empty())
        );
        let left = score
            .insert_on(
                piano,
                score_core::StaffId::Lower,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Whole,
                },
            )
            .unwrap();
        let bytes = encode(&score).unwrap();
        let loaded = decode(&bytes).unwrap();
        assert_eq!(loaded.parts()[0].measures(), &upper_before);
        assert_eq!(loaded.event(left), score.event(left));
        assert_eq!(loaded, score);
        let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        data["score"]["parts"][0]["staves"]
            .as_array_mut()
            .unwrap()
            .clear();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
        let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        data["score"]["parts"][0]["staves"][1]["id"] = "Upper".into();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
        let mut data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        data["score"]["parts"][0]["staves"][1]["measures"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
    }

    #[test]
    fn opens_legacy_piano_and_round_trips_an_ensemble() {
        let mut score = decode(include_bytes!(
            "../../../examples/primera-melodia.partitura"
        ))
        .unwrap();
        assert_eq!(score.parts().len(), 1);
        assert_eq!(score.parts()[0].instrument(), score_core::Instrument::Piano);
        assert!(!score.measures()[0].is_empty());
        let part = score.add_part(score_core::Instrument::Cello).unwrap();
        let id = score
            .insert_in(
                part,
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::C, 3)),
                    duration: Duration::Half,
                },
            )
            .unwrap();
        let loaded = decode(&encode(&score).unwrap()).unwrap();
        assert_eq!(loaded, score);
        assert_eq!(loaded.event(id), score.event(id));
        assert_eq!(
            loaded.part(part).unwrap().instrument(),
            score_core::Instrument::Cello
        );
    }

    #[test]
    fn rejects_duplicate_parts_and_misaligned_bar_counts() {
        let mut score = Score::default();
        score.add_part(score_core::Instrument::Violin).unwrap();
        let data: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
        let mut duplicate = data.clone();
        duplicate["score"]["parts"][1]["id"] = duplicate["score"]["parts"][0]["id"].clone();
        assert!(decode(&serde_json::to_vec(&duplicate).unwrap()).is_err());
        let mut shorter = data.clone();
        shorter["score"]["parts"][1]["staves"][0]["measures"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(decode(&serde_json::to_vec(&shorter).unwrap()).is_err());
        let mut invalid_id = data;
        invalid_id["score"]["next_part_id"] = 0.into();
        assert!(decode(&serde_json::to_vec(&invalid_id).unwrap()).is_err());
    }

    #[test]
    fn file_round_trip_preserves_notes_ids_and_replaces_atomically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("melodia.partitura");
        let mut score = Score::default();
        save(&path, &score).unwrap();
        let id = score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(Pitch::new(Step::A, 4)),
                    duration: Duration::Half,
                },
            )
            .unwrap();
        save(&path, &score).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded, score);
        assert_eq!(loaded.event(id), score.event(id));
    }

    /// 32 parts with a quarter-note line in every bar, like a dense tutti.
    pub(crate) fn orchestral(bars: usize) -> Score {
        let instruments: Vec<_> = score_core::Instrument::ALL
            .iter()
            .copied()
            .cycle()
            .take(32)
            .collect();
        let mut score = Score::new("Tutti".into(), &instruments, bars).unwrap();
        let parts: Vec<_> = score.parts().iter().map(|part| part.id()).collect();
        for part in parts {
            for bar in 0..bars {
                for beat in 0..4 {
                    let step = [Step::C, Step::E, Step::G, Step::B][(bar + beat) % 4];
                    score
                        .insert_on(
                            part,
                            score_core::StaffId::Upper,
                            bar,
                            beat,
                            Event {
                                notation: Default::default(),
                                pitches: score_core::Pitches::single(Pitch::new(step, 4)),
                                duration: Duration::Quarter,
                            },
                        )
                        .unwrap();
                }
            }
        }
        score
    }

    #[test]
    fn saves_an_orchestral_score_too_large_for_indented_json() {
        let score = orchestral(420);
        let pretty = serde_json::to_vec_pretty(&NativeFile {
            format: "partitura".into(),
            version: SCHEMA_VERSION,
            score: score.clone(),
        })
        .unwrap();
        assert!(pretty.len() > 16 * 1024 * 1024, "{}", pretty.len());
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tutti.partitura");
        save(&path, &score).unwrap();
        assert_eq!(load(&path).unwrap(), score);
        // Files written indented by older versions still open.
        assert_eq!(decode(&pretty).unwrap(), score);
    }

    #[test]
    fn rejects_future_versions_malformed_time_and_dangling_events() {
        let encoded = encode(&Score::default()).unwrap();
        let mut data: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        data["version"] = 999.into();
        data["score"] = serde_json::Value::Null;
        let newer = decode(&serde_json::to_vec(&data).unwrap()).unwrap_err();
        assert_eq!(newer.kind(), FileErrorKind::NewerVersion);
        assert_eq!(
            newer.to_string(),
            "la partitura se creó con una versión más reciente de Levare (formato 999); actualiza Levare para abrirla"
        );
        // The same inside a .levare written by that newer version.
        let packed = container::pack(&serde_json::to_vec(&data).unwrap()).unwrap();
        assert_eq!(
            decode(&packed).unwrap_err().kind(),
            FileErrorKind::NewerVersion
        );
        let mut data: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        data["score"]["time"]["denominator"] = 0.into();
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
        assert!(decode(b"not json").is_err());
        assert!(decode(&vec![b' '; MAX_FILE_BYTES + 1]).is_err());
        let mut score = Score::default();
        score
            .insert(
                0,
                0,
                Event {
                    notation: Default::default(),
                    pitches: score_core::Pitches::single(None),
                    duration: Duration::Quarter,
                },
            )
            .unwrap();
        let mut data: serde_json::Value = serde_json::from_slice(&encode(&score).unwrap()).unwrap();
        data["score"]["parts"][0]["staves"][0]["measures"][0]["order"] = serde_json::json!([]);
        assert!(decode(&serde_json::to_vec(&data).unwrap()).is_err());
    }
}

#[cfg(test)]
mod expression_tests;

#[cfg(test)]
mod voice_tests;

#[cfg(test)]
mod harmonic_tests;

#[cfg(test)]
mod orchestra_tests;

#[cfg(test)]
mod sung_tests;
#[cfg(test)]
mod text_tests;
#[cfg(test)]
mod tremolo_tests;

#[cfg(test)]
mod rehearsal_tests;

#[cfg(test)]
mod offset_tests;
