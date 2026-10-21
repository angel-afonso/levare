//! Review observations follow stable event identities and never affect engraving.
use super::*;
use std::collections::HashSet;

pub const MAX_COMMENTS: usize = 512;
pub const MAX_COMMENT_CHARS: usize = 2000;
/// Conservative upper bound for comment metadata in the pretty-printed native
/// format. Leaves most of the document's 16 MiB allowance for the music.
const MAX_COMMENT_STORAGE: usize = 4 * 1024 * 1024;

fn storage_weight(text: &str, part_name: &str, anchor_count: usize) -> usize {
    // Fixed fields and worst-case event identities, including whitespace;
    // strings reserve twice their UTF-8 size for JSON escaping.
    256usize
        .saturating_add(text.len().saturating_mul(2))
        .saturating_add(part_name.len().saturating_mul(2))
        .saturating_add(anchor_count.saturating_mul(256))
}

fn storage_error() -> ScoreError {
    ScoreError("Los comentarios ocupan demasiado espacio. Reduce el pasaje o elimina alguno")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommentId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoreComment {
    pub id: CommentId,
    pub text: Arc<str>,
    pub resolved: bool,
    /// Original part identity/name remain available if its music is deleted.
    pub part: PartId,
    pub part_name: Arc<str>,
    pub anchors: Arc<[ElementId]>,
}

fn validate_text(text: &str) -> Result<(), ScoreError> {
    if text.trim().is_empty()
        || text.chars().count() > MAX_COMMENT_CHARS
        || text.chars().any(char::is_control)
    {
        return Err(ScoreError(
            "Escribe una observación de 1 a 2000 caracteres, sin saltos de línea",
        ));
    }
    Ok(())
}

impl Score {
    /// Review observations do not invalidate musical proofreading results.
    pub fn same_content_except_comments(&self, other: &Self) -> bool {
        let mut compared = self.clone();
        compared.comments = other.comments.clone();
        compared.next_comment_id = other.next_comment_id;
        compared == *other
    }

    pub fn comments(&self) -> &Vector<ScoreComment> {
        &self.comments
    }

    pub fn comment(&self, id: CommentId) -> Option<&ScoreComment> {
        self.comments.iter().find(|comment| comment.id == id)
    }

    /// Add or edit one observation atomically. An existing detached comment may
    /// be edited without music; new comments require a selected note or passage.
    pub fn save_comment(
        &mut self,
        id: Option<CommentId>,
        text: &str,
        anchors: &[ElementId],
    ) -> Result<bool, ScoreError> {
        let text = text.trim();
        validate_text(text)?;
        let index = id
            .map(|id| {
                self.comments
                    .iter()
                    .position(|comment| comment.id == id)
                    .ok_or(ScoreError("El comentario ya no existe"))
            })
            .transpose()?;
        if anchors.is_empty() && index.is_none() {
            return Err(ScoreError("Selecciona una nota o un pasaje para comentar"));
        }
        let mut seen = HashSet::new();
        for anchor in anchors {
            if self.event(*anchor).is_none() || !seen.insert(*anchor) {
                return Err(ScoreError("El vínculo del comentario no es válido"));
            }
        }
        let part = anchors
            .first()
            .map(|id| id.part)
            .or_else(|| index.map(|i| self.comments[i].part))
            .unwrap();
        if anchors.iter().any(|id| id.part != part) {
            return Err(ScoreError("Selecciona un pasaje de una sola parte"));
        }
        let part_name: Arc<str> = if self.part(part).is_some() {
            self.part_name(part).into()
        } else {
            self.comments[index.unwrap()].part_name.clone()
        };
        if part_name.len() > 1024 || part_name.chars().any(char::is_control) {
            return Err(ScoreError(
                "El nombre de la parte no es válido para el comentario",
            ));
        }
        let storage = self
            .comments
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != index)
            .fold(
                storage_weight(text, &part_name, anchors.len()),
                |total, (_, comment)| {
                    total.saturating_add(storage_weight(
                        &comment.text,
                        &comment.part_name,
                        comment.anchors.len(),
                    ))
                },
            );
        if storage > MAX_COMMENT_STORAGE {
            return Err(storage_error());
        }
        if let Some(index) = index {
            if self.comments[index].text.as_ref() == text
                && self.comments[index].anchors.as_ref() == anchors
            {
                return Ok(false);
            }
            let comment = &mut self.comments[index];
            comment.text = text.into();
            comment.anchors = anchors.into();
            comment.part = part;
            comment.part_name = part_name;
        } else {
            if self.comments.len() >= MAX_COMMENTS {
                return Err(ScoreError("Máximo de 512 comentarios por partitura"));
            }
            let next = self
                .next_comment_id
                .checked_add(1)
                .ok_or(ScoreError("Identificadores de comentario agotados"))?;
            self.comments.push_back(ScoreComment {
                id: CommentId(next),
                text: text.into(),
                resolved: false,
                part,
                part_name,
                anchors: anchors.into(),
            });
            self.next_comment_id = next;
        }
        Ok(true)
    }

    pub fn set_comment_resolved(
        &mut self,
        id: CommentId,
        resolved: bool,
    ) -> Result<bool, ScoreError> {
        let index = self
            .comments
            .iter()
            .position(|comment| comment.id == id)
            .ok_or(ScoreError("El comentario ya no existe"))?;
        if self.comments[index].resolved == resolved {
            return Ok(false);
        }
        self.comments[index].resolved = resolved;
        Ok(true)
    }

    pub fn remove_comment(&mut self, id: CommentId) -> Result<bool, ScoreError> {
        let index = self
            .comments
            .iter()
            .position(|comment| comment.id == id)
            .ok_or(ScoreError("El comentario ya no existe"))?;
        self.comments.remove(index);
        Ok(true)
    }

    /// Sorted live anchors for navigation; deleted notes do not erase the text.
    pub fn comment_anchors(&self, id: CommentId) -> Vec<ElementId> {
        let mut anchors: Vec<_> = self
            .comment(id)
            .into_iter()
            .flat_map(|comment| comment.anchors.iter())
            .copied()
            .filter(|id| self.event(*id).is_some())
            .collect();
        anchors.sort_by_cached_key(|id| {
            (
                id.staff,
                self.voice_of(*id).map_or(0, |voice| voice.number()),
                self.event_tick(*id).unwrap_or(0),
            )
        });
        anchors
    }

    pub(crate) fn remap_comments(&mut self, map: impl Fn(ElementId) -> Option<ElementId>) {
        for comment in &mut self.comments {
            if comment.anchors.iter().any(|id| map(*id) != Some(*id)) {
                let mut seen = HashSet::new();
                comment.anchors = comment
                    .anchors
                    .iter()
                    .filter_map(|id| map(*id))
                    .filter(|id| seen.insert(*id))
                    .collect::<Vec<_>>()
                    .into();
            }
        }
    }

    pub(super) fn prune_comments(&mut self) {
        if self.comments.is_empty() {
            return;
        }
        let invalid: HashSet<_> = self
            .comments
            .iter()
            .flat_map(|comment| comment.anchors.iter())
            .copied()
            .filter(|id| self.event(*id).is_none())
            .collect();
        if !invalid.is_empty() {
            self.remap_comments(|id| (!invalid.contains(&id)).then_some(id));
        }
    }

    pub(super) fn validate_comments(&self) -> Result<(), ScoreError> {
        if self.comments.len() > MAX_COMMENTS {
            return Err(ScoreError("Demasiados comentarios"));
        }
        let storage = self.comments.iter().fold(0usize, |total, comment| {
            total.saturating_add(storage_weight(
                &comment.text,
                &comment.part_name,
                comment.anchors.len(),
            ))
        });
        if storage > MAX_COMMENT_STORAGE {
            return Err(storage_error());
        }
        let mut ids = HashSet::new();
        for comment in &self.comments {
            validate_text(&comment.text)?;
            if comment.id.0 == 0
                || comment.id.0 > self.next_comment_id
                || !ids.insert(comment.id)
                || comment.part.0 >= self.next_part_id
                || comment.part_name.len() > 1024
                || comment.part_name.chars().any(char::is_control)
            {
                return Err(ScoreError("Comentario inválido o duplicado"));
            }
            let mut anchors = HashSet::new();
            if comment.anchors.iter().any(|id| {
                id.part != comment.part || self.event(*id).is_none() || !anchors.insert(*id)
            }) {
                return Err(ScoreError("El vínculo del comentario no es válido"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
