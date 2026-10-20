//! A selected score and its complete instrumental parts, published as one set.
use score_core::{PartId, Score};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Selection {
    pub full_score: bool,
    pub parts: Vec<PartId>,
    pub pdf: bool,
    pub musicxml: bool,
}

impl Selection {
    pub fn all(score: &Score, musicxml: bool) -> Self {
        Self {
            full_score: true,
            parts: score.parts().iter().map(|part| part.id()).collect(),
            pdf: !musicxml,
            musicxml,
        }
    }
    pub fn file_count(&self) -> usize {
        (usize::from(self.full_score) + self.parts.len())
            * (usize::from(self.pdf) + usize::from(self.musicxml))
    }
    pub fn validate(&self, score: &Score) -> Result<(), String> {
        score.validate().map_err(|error| error.to_string())?;
        if !self.full_score && self.parts.is_empty() {
            return Err("Selecciona la partitura general o al menos una parte".into());
        }
        if !self.pdf && !self.musicxml {
            return Err("Selecciona PDF, MusicXML o ambos formatos".into());
        }
        let mut seen = std::collections::HashSet::new();
        if self
            .parts
            .iter()
            .any(|id| score.part(*id).is_none() || !seen.insert(*id))
        {
            return Err("La selección de partes ya no coincide con la partitura".into());
        }
        Ok(())
    }
}

/// A readable component with a bounded UTF-8 byte length on macOS filesystems.
pub fn folder_name(title: &str) -> String {
    format!("{} - Exportación", component(title, 120))
}

fn component(value: &str, limit: usize) -> String {
    let clean: String = value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '/' | '\\' | ':') {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let mut end = clean.len().min(limit);
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    let clean = clean[..end].trim().trim_matches('.').trim();
    if clean.is_empty() {
        "Partitura".into()
    } else {
        clean.into()
    }
}

/// Generates every file before publishing a new directory. Existing destinations
/// are never merged or replaced, so a failed export cannot damage an older set.
pub fn export(
    score: &Score,
    selection: &Selection,
    destination: &Path,
) -> Result<Vec<PathBuf>, String> {
    selection.validate(score)?;
    if destination.exists() {
        return Err("Ya existe esa carpeta. Elige un nombre nuevo para la exportación".into());
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = tempfile::tempdir_in(parent).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    let mut render = |name: String, score: &Score, part: bool| -> Result<(), String> {
        for (enabled, extension) in [(selection.pdf, "pdf"), (selection.musicxml, "musicxml")] {
            if !enabled {
                continue;
            }
            let filename = format!("{name}.{extension}");
            let path = staging.path().join(&filename);
            if extension == "pdf" {
                if part {
                    score_pdf::export_part(score, &path)
                } else {
                    score_pdf::export(score, &path)
                }
                .map(|_| ())?;
            } else {
                score_io::musicxml::export(&path, score).map_err(|e| e.to_string())?;
            }
            files.push(filename);
        }
        Ok(())
    };
    if selection.full_score {
        render("00 - Partitura general".into(), score, false)?;
    }
    // Score order is authoritative even when the user selected parts out of order.
    for (index, part) in score.parts().iter().enumerate() {
        if !selection.parts.contains(&part.id()) {
            continue;
        }
        let extracted = score.extract_part(part.id()).map_err(|e| e.to_string())?;
        let name = component(&score.part_name(part.id()), 128);
        render(format!("{:02} - {name}", index + 1), &extracted, true)?;
    }
    // This operation also protects against another process choosing the same
    // destination during rendering. Only our newly created directory is populated.
    std::fs::create_dir(destination)
        .map_err(|e| format!("No se pudo crear la carpeta de exportación: {e}"))?;
    let mut published = Vec::new();
    for filename in files {
        let path = destination.join(&filename);
        if let Err(error) = std::fs::rename(staging.path().join(&filename), &path) {
            for path in &published {
                let _ = std::fs::remove_file(path);
            }
            let _ = std::fs::remove_dir(destination);
            return Err(format!("No se pudo completar la exportación: {error}"));
        }
        published.push(path);
    }
    Ok(published)
}

#[cfg(test)]
mod tests;
