//! Built-in shortcut profiles.

use crate::{CommandId, KeyError, KeySequence, Platform, commands::id};

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
        Self::builtin_for(id, Platform::current())
    }

    pub fn builtin_for(id: &str, platform: Platform) -> Option<Self> {
        match id {
            Self::DEFAULT => Some(Self::sibelius_laptop_for(platform)),
            _ => None,
        }
    }

    pub fn sibelius_laptop() -> Self {
        Self::sibelius_laptop_for(Platform::current())
    }

    /// Sibelius' *Notebook (laptop)* feature set, which also works with a full
    /// keyboard: the main number row plays the role of the numeric keypad and
    /// Shift+1–9 adds intervals above. GPUI 0.2.2 cannot tell keypad digits
    /// from main-row digits, so both rows behave the same.
    pub fn sibelius_laptop_for(platform: Platform) -> Self {
        let own = match platform {
            Platform::MacOs => SIBELIUS_LAPTOP_MACOS,
            Platform::Windows => SIBELIUS_LAPTOP_WINDOWS,
            Platform::Linux => SIBELIUS_LAPTOP_LINUX,
        };
        Self::from_tables(
            Self::DEFAULT,
            "Sibelius · portátil",
            "Atajos de Sibelius para teclados sin bloque numérico; también funcionan con él.",
            &[SIBELIUS_LAPTOP, own],
        )
        .expect("built-in Sibelius shortcuts parse")
    }

    fn from_tables(
        id: &'static str,
        name: &'static str,
        description: &'static str,
        tables: &[&[(&str, CommandId)]],
    ) -> Result<Self, KeyError> {
        let bindings = tables
            .iter()
            .flat_map(|table| table.iter())
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

/// Shortcuts that are the same everywhere; `secondary` is ⌘ on macOS and Ctrl
/// on Windows and Linux.
#[rustfmt::skip]
const SIBELIUS_LAPTOP: &[(&str, CommandId)] = &[
    ("secondary-n", id::NEW),
    ("secondary-o", id::OPEN),
    ("secondary-s", id::SAVE),
    ("shift-secondary-s", id::SAVE_AS),
    ("secondary-w", id::CLOSE),
    ("secondary-q", id::QUIT),
    ("secondary-p", id::PRINT),
    ("secondary-,", id::PREFERENCES),
    ("escape", id::CLOSE_PANEL),

    ("secondary-z", id::UNDO),
    ("secondary-y", id::REDO),
    ("shift-secondary-z", id::REDO),
    ("secondary-x", id::CUT),
    ("secondary-c", id::COPY),
    ("secondary-v", id::PASTE),
    ("secondary-a", id::SELECT_ALL),
    ("backspace", id::DELETE),
    ("delete", id::DELETE),
    ("shift-secondary-p", id::RESET_POSITION),
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
    ("shift-secondary-f", id::FLEXI_TIME),

    ("1", id::THIRTY_SECOND),
    ("2", id::SIXTEENTH),
    ("3", id::EIGHTH),
    ("4", id::QUARTER),
    ("5", id::HALF),
    ("6", id::WHOLE),
    (".", id::DOT),
    ("secondary-3", id::TRIPLET),

    ("7", id::NATURAL),
    ("8", id::SHARP),
    ("9", id::FLAT),

    ("enter", id::TIE),
    ("s", id::SLUR),
    ("h", id::CRESCENDO),
    ("shift-h", id::DIMINUENDO),
    ("secondary-e", id::EXPRESSION_TEXT),
    ("secondary-f", id::COMMAND_SEARCH),
    ("secondary-t", id::TECHNIQUE_TEXT),
    ("secondary-l", id::LYRICS),
    ("secondary-k", id::CHORD_SYMBOL),
    ("secondary-r", id::REHEARSAL_MARK),

    ("k", id::KEY_SIGNATURE),
    ("t", id::TIME_SIGNATURE),
    ("q", id::CLEF),
    ("l", id::LINES),
    ("z", id::SYMBOL),

    ("secondary-b", id::BAR_AT_END),
    ("shift-secondary-b", id::SINGLE_BAR),
    ("secondary-backspace", id::DELETE_BAR),

    ("i", id::ADD_INSTRUMENT),

    ("shift-left",id::EXTEND_PREVIOUS),
    ("shift-right",id::EXTEND_NEXT),
    ("left", id::PREVIOUS),
    ("shift-tab", id::PREVIOUS),
    ("right", id::NEXT),
    ("tab", id::NEXT),
    ("up", id::STEP_UP),
    ("down", id::STEP_DOWN),
    ("secondary-up", id::OCTAVE_UP),
    ("secondary-down", id::OCTAVE_DOWN),
    ("alt-up", id::UPPER_NOTE_OR_STAFF),
    ("alt-down", id::LOWER_NOTE_OR_STAFF),
    ("secondary-left", id::BAR_START),
    ("secondary-right", id::NEXT_BAR),

    ("space", id::PLAY),
    ("p", id::PLAY_FROM_SELECTION),
    ("shift-o", id::STOP),
    ("secondary-[", id::PLAYLINE_TO_START),
    ("alt-p", id::LOOP),
    ("shift-space", id::HALF_SPEED),
    ("m", id::MIXER),

    ("secondary-+", id::ZOOM_IN),
    ("secondary-=", id::ZOOM_IN),
    ("secondary--", id::ZOOM_OUT),
    ("secondary-1", id::ZOOM_ACTUAL),
    ("secondary-0", id::ZOOM_FIT),
];

/// macOS keeps Sibelius' ⌥⌘ combinations.
#[rustfmt::skip]
const SIBELIUS_LAPTOP_MACOS: &[(&str, CommandId)] = &[
    ("alt-secondary-1", id::TAB_NOTE_INPUT),
    ("alt-secondary-2", id::TAB_NOTATION),
    ("alt-secondary-3", id::TAB_DYNAMICS),
    ("alt-secondary-4", id::TAB_TEXT),
    ("alt-secondary-5", id::TAB_HOME),
    ("alt-secondary-6", id::TAB_PLAYBACK),
    ("alt-secondary-7", id::TAB_REVIEW),
    ("alt-secondary-8", id::TAB_VIEW),
    ("alt-secondary-t", id::TEMPO_TEXT),
    ("alt-secondary-g", id::GO_TO_BAR),
    ("alt-space", id::REPLAY),
    ("secondary-u", id::FULL_SCREEN),
];

/// Ctrl+Alt is AltGr on Windows, which types @, # and | on Spanish and other
/// keyboards, and on Linux desktops it opens a terminal (Ctrl+Alt+T) or
/// switches workspaces. Ctrl+Mayús takes its place; full screen is F11.
#[rustfmt::skip]
const SIBELIUS_LAPTOP_WINDOWS: &[(&str, CommandId)] = &[
    ("shift-secondary-1", id::TAB_NOTE_INPUT),
    ("shift-secondary-2", id::TAB_NOTATION),
    ("shift-secondary-3", id::TAB_DYNAMICS),
    ("shift-secondary-4", id::TAB_TEXT),
    ("shift-secondary-5", id::TAB_HOME),
    ("shift-secondary-6", id::TAB_PLAYBACK),
    ("shift-secondary-7", id::TAB_REVIEW),
    ("shift-secondary-8", id::TAB_VIEW),
    ("shift-secondary-t", id::TEMPO_TEXT),
    ("secondary-g", id::GO_TO_BAR),
    ("alt-space", id::REPLAY),
    ("f11", id::FULL_SCREEN),
];

/// As on Windows, except Replay: GNOME, KDE and Xfce keep Alt+Space for the
/// window menu or the launcher, so it never reaches the app.
#[rustfmt::skip]
const SIBELIUS_LAPTOP_LINUX: &[(&str, CommandId)] = &[
    ("shift-secondary-1", id::TAB_NOTE_INPUT),
    ("shift-secondary-2", id::TAB_NOTATION),
    ("shift-secondary-3", id::TAB_DYNAMICS),
    ("shift-secondary-4", id::TAB_TEXT),
    ("shift-secondary-5", id::TAB_HOME),
    ("shift-secondary-6", id::TAB_PLAYBACK),
    ("shift-secondary-7", id::TAB_REVIEW),
    ("shift-secondary-8", id::TAB_VIEW),
    ("shift-secondary-t", id::TEMPO_TEXT),
    ("secondary-g", id::GO_TO_BAR),
    ("alt-shift-space", id::REPLAY),
    ("f11", id::FULL_SCREEN),
];
