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
        let articulation =
            |mark: Articulation| CommandState::checked(selected_articulation(self) == Some(mark));
        let dynamic = |mark: Dynamic| CommandState::checked(selected_dynamic(self) == Some(mark));
        let technique = |mark: PlayingTechnique| {
            CommandState::checked(
                editor
                    .selection()
                    .and_then(|id| editor.score().event(id))
                    .and_then(|event| event.notation.technique)
                    == Some(mark),
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

            id::STACCATO => articulation(Articulation::Staccato),
            id::TENUTO => articulation(Articulation::Tenuto),
            id::ACCENT => articulation(Articulation::Accent),
            id::MARCATO => articulation(Articulation::Marcato),
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

            id::DYNAMIC_PPP => dynamic(Dynamic::Ppp),
            id::DYNAMIC_PP => dynamic(Dynamic::Pp),
            id::DYNAMIC_P => dynamic(Dynamic::P),
            id::DYNAMIC_MP => dynamic(Dynamic::Mp),
            id::DYNAMIC_MF => dynamic(Dynamic::Mf),
            id::DYNAMIC_F => dynamic(Dynamic::F),
            id::DYNAMIC_FF => dynamic(Dynamic::Ff),
            id::DYNAMIC_FFF => dynamic(Dynamic::Fff),
            id::PIZZICATO => technique(PlayingTechnique::Pizzicato),
            id::ARCO => technique(PlayingTechnique::Arco),

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
