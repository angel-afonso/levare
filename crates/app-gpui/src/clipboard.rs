//! OS clipboard integration; never paste an internal stale copy over external text.
use super::*;
use score_edit::clipboard::{MAX_CLIPBOARD_BYTES, MusicalClipboard};

pub(super) fn decode_item(item: &ClipboardItem) -> Result<MusicalClipboard, String> {
    let metadata = item
        .metadata()
        .ok_or("El portapapeles no contiene música de Levare")?;
    if metadata.len() > MAX_CLIPBOARD_BYTES {
        return Err("El pasaje del portapapeles es demasiado grande".into());
    }
    let clip: MusicalClipboard = serde_json::from_str(metadata)
        .map_err(|_| "El contenido musical del portapapeles no es válido")?;
    clip.upgraded().map_err(|e| e.to_string())
}
impl AppView {
    pub(super) fn copy_music(&mut self, cut: bool, cx: &mut Context<Self>) {
        let result = self
            .document
            .editor
            .copy_selection()
            .map_err(|e| e.to_string())
            .and_then(|clip| {
                let metadata = serde_json::to_string(&clip).map_err(|e| e.to_string())?;
                if metadata.len() > MAX_CLIPBOARD_BYTES {
                    return Err("Selecciona un pasaje más corto para copiar".into());
                }
                cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                    clip.summary(),
                    metadata.clone(),
                ));
                if cut
                    && cx
                        .read_from_clipboard()
                        .and_then(|item| item.metadata().cloned())
                        .as_deref()
                        != Some(metadata.as_str())
                {
                    return Err("No se pudo verificar la copia; la música sigue en su sitio".into());
                }
                Ok(clip.events.len())
            });
        match result {
            Ok(count) => {
                if cut {
                    self.edit(Command::Delete, cx);
                    self.status = format!("Cortados {count} elementos. Elige un destino y pega");
                } else {
                    self.status = format!("Copiados {count} elementos musicales");
                }
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }
    pub(super) fn paste_music(&mut self, cx: &mut Context<Self>) {
        let clip = cx
            .read_from_clipboard()
            .ok_or("El portapapeles está vacío".to_owned())
            .and_then(|item| decode_item(&item));
        match clip.and_then(|clip| self.document.editor.paste(&clip).map_err(|e| e.to_string())) {
            Ok(count) => {
                self.playback.stop();
                self.pending_slur = None;
                self.pending_hairpin = None;
                self.reveal_selection = true;
                self.status = format!("Pegados {count} elementos musicales");
                self.sync_quit_snapshot();
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn clipboard_reader_rejects_external_text_invalid_versions_and_oversized_data() {
        assert!(decode_item(&ClipboardItem::new_string("texto ajeno".into())).is_err());
        assert!(
            decode_item(&ClipboardItem::new_string_with_metadata(
                "".into(),
                "{}".into()
            ))
            .is_err()
        );
        assert!(
            decode_item(&ClipboardItem::new_string_with_metadata(
                "".into(),
                " ".repeat(MAX_CLIPBOARD_BYTES + 1)
            ))
            .is_err()
        );
        let mut editor = score_edit::Editor::default();
        editor
            .execute(Command::Insert(editor.note(score_core::Step::C)))
            .unwrap();
        let mut clip = editor.copy_selection().unwrap();
        let item = ClipboardItem::new_string_with_json_metadata(clip.summary(), clip.clone());
        assert_eq!(decode_item(&item).unwrap(), clip);
        clip.version = 99;
        assert!(
            decode_item(&ClipboardItem::new_string_with_json_metadata(
                clip.summary(),
                clip
            ))
            .is_err()
        );
    }
}

#[cfg(test)]
mod playback_tests {
    use score_core::{
        Articulation, Duration, Dynamic, Event, Notation, Pitch, Pitches, Score, Step,
    };
    use score_edit::{Command, Editor};
    use score_playback::{Message, PlaybackPlan, Tempo};
    #[test]
    fn paste_preserves_staccato_sound_when_splitting_a_long_note_across_bars() {
        let event = Event {
            pitches: Pitches::single(Pitch::new(Step::C, 4)),
            duration: Duration::Whole,
            notation: Notation {
                harmonic: None,
                tremolo: None,
                division: None,
                arpeggio: None,
                technique: None,
                dynamic: Some(Dynamic::F),
                articulation: Some(Articulation::Staccato),
                fermata: false,
                offsets: Default::default(),
            },
        };
        let mut source = Editor::default();
        source.execute(Command::Insert(event)).unwrap();
        let clip = source.copy_selection().unwrap();
        let mut dest = Editor::new(Score::default()).unwrap();
        dest.execute(Command::Insert(Event {
            pitches: Pitches::default(),
            duration: Duration::Quarter,
            notation: Notation::default(),
        }))
        .unwrap();
        dest.paste(&clip).unwrap();
        let original = PlaybackPlan::new(source.score(), Tempo::default(), 48_000).unwrap();
        let pasted = PlaybackPlan::new(dest.score(), Tempo::default(), 48_000).unwrap();
        assert_eq!(original.messages.len(), 2);
        assert_eq!(pasted.messages.len(), 2);
        for (a, b) in original.messages.iter().zip(pasted.messages.iter()) {
            assert_eq!(a.message, b.message);
            assert_eq!(a.frame + 24_000, b.frame);
        }
        assert!(matches!(
            pasted.messages[0].message,
            Message::NoteOn(note) if note.level == 100
        ));
    }
}
