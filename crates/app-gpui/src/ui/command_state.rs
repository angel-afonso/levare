//! The app's answer to the toolbar model: what each command looks like now.
//! One place for "is this option active?", used by the ribbon and File menu.

use super::*;
use score_core::NoteValue;
use score_toolbar::{CommandState, CommandStates};

impl CommandStates for AppView {
    fn state(&self, command: CommandId) -> CommandState {
        let editor = &self.document.editor;
        let duration = active_duration(self);
        let value = |note: NoteValue| CommandState::checked(duration.value() == note);
        let accidental = |sign: Accidental| {
            CommandState::checked(
                editor
                    .selected_pitch()
                    .map_or(self.entry_accidental == Some(sign), |pitch| {
                        pitch.accidental() == Some(sign)
                    }),
            )
        };
        let hairpin = |kind: ConnectionKind| {
            let open = self
                .pending_hairpin
                .is_some_and(|(_, pending)| pending == kind);
            CommandState {
                checked: open || selected_connection(self, kind),
                label: open.then(|| format!("Terminar {}", kind.label().to_lowercase())),
                ..CommandState::default()
            }
        };
        let enabled = |enabled: bool| CommandState {
            enabled,
            ..CommandState::default()
        };
        // Every point mark of the catalog: lit while the selection carries it.
        if let Some(kind) = score_toolbar::marks::kind(command).filter(|kind| kind.is_point()) {
            return CommandState::checked(
                editor
                    .selection()
                    .and_then(|id| editor.score().point_mark(id, kind.family()))
                    == Some(kind),
            );
        }
        match command {
            id::UNDO => enabled(editor.can_undo()),
            id::REDO => enabled(editor.can_redo()),
            id::REMOVE_INSTRUMENT => enabled(editor.score().parts().len() > 1),

            id::TOGGLE_NOTE_INPUT => CommandState::checked(self.ui.note_entry),
            id::STEP_TIME => CommandState::checked(true),
            id::CHORD_MODE => CommandState::checked(self.chord_entry),

            id::WHOLE => value(NoteValue::Whole),
            id::HALF => value(NoteValue::Half),
            id::QUARTER => value(NoteValue::Quarter),
            id::EIGHTH => value(NoteValue::Eighth),
            id::SIXTEENTH => value(NoteValue::Sixteenth),
            id::DOT => CommandState::checked(duration.is_dotted()),
            id::TRIPLET => CommandState::checked(duration.is_triplet()),

            id::NATURAL => accidental(Accidental::Natural),
            id::SHARP => accidental(Accidental::Sharp),
            id::FLAT => accidental(Accidental::Flat),

            id::TIE => CommandState::checked(selected_connection(self, ConnectionKind::Tie)),
            id::SLUR => CommandState {
                checked: self.pending_slur.is_some(),
                label: self
                    .pending_slur
                    .is_some()
                    .then(|| "Terminar expresión".to_owned()),
                ..CommandState::default()
            },
            id::CRESCENDO => hairpin(ConnectionKind::Crescendo),
            id::DIMINUENDO => hairpin(ConnectionKind::Diminuendo),

            id::KEY_SIGNATURE => CommandState::checked(self.key_picker),
            id::ADD_INSTRUMENT => CommandState::checked(self.instrument_picker.is_some()),

            id::PLAY => CommandState {
                checked: self.playback.is_playing(),
                label: self.playback.is_playing().then(|| "Pausar".to_owned()),
                ..CommandState::default()
            },
            id::AUDITION => CommandState::checked(self.playback.audition),
            id::MIXER => {
                CommandState::checked(self.ui.show_inspector && self.ui.inspector == "Mezclador")
            }

            id::TOGGLE_INSTRUMENTS_PANEL => CommandState::checked(self.ui.show_instruments),
            id::TOGGLE_INSPECTOR => CommandState::checked(self.ui.show_inspector),
            _ => CommandState::default(),
        }
    }
}
