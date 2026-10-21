//! Built-in shortcut profiles.

use crate::{CommandId, KeyError, KeySequence, commands::id};

#[derive(Clone, Debug)]
pub struct Profile {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub bindings: Vec<(KeySequence, CommandId)>,
}

impl Profile {
    pub const DEFAULT: &'static str = "sibelius-portatil";

    pub fn builtin(id: &str) -> Option<Self> {
        match id {
            Self::DEFAULT => Some(Self::sibelius_laptop()),
            _ => None,
        }
    }

    /// Sibelius' *Notebook (laptop)* feature set, which also works with a full
    /// keyboard: the main number row plays the role of the numeric keypad and
    /// Shift+1–9 adds intervals above. GPUI 0.2.2 cannot tell keypad digits
    /// from main-row digits, so both rows behave the same.
    pub fn sibelius_laptop() -> Self {
        Self::from_table(
            Self::DEFAULT,
            "Sibelius · portátil",
            "Atajos de Sibelius para teclados sin bloque numérico; también funcionan con él.",
            SIBELIUS_LAPTOP,
        )
        .expect("built-in Sibelius shortcuts parse")
    }

    fn from_table(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        table: &[(&str, CommandId)],
    ) -> Result<Self, KeyError> {
        let bindings = table
            .iter()
            .map(|(keys, command)| KeySequence::parse(keys).map(|keys| (keys, *command)))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            id,
            name,
            description,
            bindings,
        })
    }

    pub fn keys_for(&self, command: CommandId) -> Vec<KeySequence> {
        self.bindings
            .iter()
            .filter(|(_, bound)| *bound == command)
            .map(|(keys, _)| keys.clone())
            .collect()
    }
}

#[rustfmt::skip]
const SIBELIUS_LAPTOP: &[(&str, CommandId)] = &[
    ("cmd-n", id::NEW),
    ("cmd-o", id::OPEN),
    ("cmd-s", id::SAVE),
    ("shift-cmd-s", id::SAVE_AS),
    ("cmd-w", id::CLOSE),
    ("cmd-q", id::QUIT),
    ("cmd-p", id::PRINT),
    ("cmd-,", id::PREFERENCES),
    ("escape", id::CLOSE_PANEL),

    ("cmd-z", id::UNDO),
    ("cmd-y", id::REDO),
    ("shift-cmd-z", id::REDO),
    ("cmd-x", id::CUT),
    ("cmd-c", id::COPY),
    ("cmd-v", id::PASTE),
    ("cmd-a", id::SELECT_ALL),
    ("backspace", id::DELETE),
    ("delete", id::DELETE),
    ("shift-cmd-p", id::RESET_POSITION),
    ("escape", id::ESCAPE),
    ("r", id::REPEAT),

    ("n", id::TOGGLE_NOTE_INPUT),
    ("c", id::NOTE_C),
    ("d", id::NOTE_D),
    ("e", id::NOTE_E),
    ("f", id::NOTE_F),
    ("g", id::NOTE_G),
    ("a", id::NOTE_A),
    ("b", id::NOTE_B),
    ("shift-c", id::ADD_C),
    ("shift-d", id::ADD_D),
    ("shift-e", id::ADD_E),
    ("shift-f", id::ADD_F),
    ("shift-g", id::ADD_G),
    ("shift-a", id::ADD_A),
    ("shift-b", id::ADD_B),
    ("shift-1", id::INTERVAL_1),
    ("shift-2", id::INTERVAL_2),
    ("shift-3", id::INTERVAL_3),
    ("shift-4", id::INTERVAL_4),
    ("shift-5", id::INTERVAL_5),
    ("shift-6", id::INTERVAL_6),
    ("shift-7", id::INTERVAL_7),
    ("shift-8", id::INTERVAL_8),
    ("shift-9", id::INTERVAL_9),
    ("0", id::REST),
    ("alt-a", id::CHORD_MODE),
    ("alt-backspace", id::REMOVE_HIGHEST),
    ("alt-1", id::VOICE_1),
    ("alt-2", id::VOICE_2),
    ("alt-3", id::VOICE_3),
    ("alt-4", id::VOICE_4),
    (";", id::GRACE_NOTE),
    ("shift-cmd-f", id::FLEXI_TIME),

    ("1", id::THIRTY_SECOND),
    ("2", id::SIXTEENTH),
    ("3", id::EIGHTH),
    ("4", id::QUARTER),
    ("5", id::HALF),
    ("6", id::WHOLE),
    (".", id::DOT),
    ("cmd-3", id::TRIPLET),

    ("7", id::NATURAL),
    ("8", id::SHARP),
    ("9", id::FLAT),

    ("enter", id::TIE),
    ("s", id::SLUR),
    ("h", id::CRESCENDO),
    ("shift-h", id::DIMINUENDO),
    ("cmd-e", id::EXPRESSION_TEXT),
    ("cmd-f", id::COMMAND_SEARCH),
    ("alt-cmd-1", id::TAB_HOME),
    ("alt-cmd-2", id::TAB_NOTE_INPUT),
    ("alt-cmd-3", id::TAB_NOTATION),
    ("alt-cmd-4", id::TAB_TEXT),
    ("alt-cmd-5", id::TAB_PLAYBACK),
    ("alt-cmd-6", id::TAB_REVIEW),
    ("alt-cmd-7", id::TAB_VIEW),
    ("alt-cmd-8", id::TAB_LAYOUT),
    ("cmd-t", id::TECHNIQUE_TEXT),
    ("alt-cmd-t", id::TEMPO_TEXT),
    ("cmd-l", id::LYRICS),
    ("cmd-k", id::CHORD_SYMBOL),
    ("cmd-r", id::REHEARSAL_MARK),

    ("k", id::KEY_SIGNATURE),
    ("t", id::TIME_SIGNATURE),
    ("q", id::CLEF),
    ("l", id::LINES),
    ("z", id::SYMBOL),

    ("cmd-b", id::BAR_AT_END),
    ("shift-cmd-b", id::SINGLE_BAR),
    ("cmd-backspace", id::DELETE_BAR),
    ("alt-cmd-g", id::GO_TO_BAR),

    ("i", id::ADD_INSTRUMENT),

    ("shift-left",id::EXTEND_PREVIOUS),
    ("shift-right",id::EXTEND_NEXT),
    ("left", id::PREVIOUS),
    ("shift-tab", id::PREVIOUS),
    ("right", id::NEXT),
    ("tab", id::NEXT),
    ("up", id::STEP_UP),
    ("down", id::STEP_DOWN),
    ("cmd-up", id::OCTAVE_UP),
    ("cmd-down", id::OCTAVE_DOWN),
    ("alt-up", id::UPPER_NOTE_OR_STAFF),
    ("alt-down", id::LOWER_NOTE_OR_STAFF),
    ("cmd-left", id::BAR_START),
    ("cmd-right", id::NEXT_BAR),

    ("space", id::PLAY),
    ("p", id::PLAY_FROM_SELECTION),
    ("shift-o", id::STOP),
    ("cmd-[", id::PLAYLINE_TO_START),
    ("alt-p", id::LOOP),
    ("alt-space", id::REPLAY),
    ("shift-space", id::HALF_SPEED),
    ("m", id::MIXER),

    ("cmd-+", id::ZOOM_IN),
    ("cmd-=", id::ZOOM_IN),
    ("cmd--", id::ZOOM_OUT),
    ("cmd-1", id::ZOOM_ACTUAL),
    ("cmd-0", id::ZOOM_FIT),
    ("cmd-u", id::FULL_SCREEN),
];
