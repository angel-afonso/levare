//! Which registry commands the agent may run: those that change or walk the
//! music, so a new command of an allowed category reaches the agent unasked.
use score_keymap::{Category, CommandId, CommandSpec, Registry, Status, id};

const ALLOWED: [Category; 11] = [
    Category::Edit,
    Category::NoteInput,
    Category::Duration,
    Category::Accidentals,
    Category::Expression,
    Category::Notation,
    Category::Bars,
    Category::Instruments,
    Category::Navigation,
    Category::Review,
    Category::Playback,
];

/// Commands the agent never runs although their category is allowed (some
/// File and View ones are listed too, so the rule holds if they move): the
/// user's history, and everything that opens interface or needs the window.
pub const EXCLUDED: &[CommandId] = &[
    // The turn is one undo step; the history belongs to the user.
    id::UNDO,
    id::REDO,
    // Open a search field or a dialog (`AppView::run` needs the window).
    id::COMMAND_SEARCH,
    id::SCORE_INFO,
    id::GO_TO_BAR,
    id::PREFERENCES,
    // Toggles the window itself.
    id::FULL_SCREEN,
    // Ask to save and replace or close the document, ending the session.
    id::NEW,
    id::OPEN,
    id::CLOSE,
    id::QUIT,
    // Open a text field for the words; `ajustar texto` writes them.
    id::EXPRESSION_TEXT,
    id::TECHNIQUE_TEXT,
    id::LYRICS,
    id::CHORD_SYMBOL,
    id::NEW_COMMENT,
    id::EDIT_RANGE,
    // Open a picker; `ajustar` takes the value instead.
    id::TIME_SIGNATURE,
    id::KEY_SIGNATURE,
    id::CLEF,
    id::TEMPO_TEXT,
    id::ADD_INSTRUMENT,
    // Show or hide panels and overlays: the user's view, not the music.
    id::SHOW_COMMENTS,
    id::REVIEW_ISSUES,
    id::SHOW_RANGES,
    id::MIXER,
    id::SOUNDS,
    id::TOGGLE_INSTRUMENTS_PANEL,
    id::TOGGLE_INSPECTOR,
];

/// Ready commands of the categories that change or walk the music.
pub fn allowed_commands(registry: &Registry) -> Vec<&CommandSpec> {
    registry
        .iter()
        .filter(|spec| {
            ALLOWED.contains(&spec.category)
                && spec.status == Status::Ready
                && !EXCLUDED.contains(&spec.id)
        })
        .collect()
}

/// The tool that does what an excluded command would, for its error message.
pub(crate) fn alternative(command: CommandId) -> Option<&'static str> {
    Some(match command {
        id::EXPRESSION_TEXT | id::TECHNIQUE_TEXT => "ajustar con tipo texto",
        id::TIME_SIGNATURE => "ajustar con tipo compas_tiempo",
        id::KEY_SIGNATURE => "ajustar con tipo tonalidad",
        id::CLEF => "ajustar con tipo clave",
        id::TEMPO_TEXT => "ajustar con tipo tempo",
        id::ADD_INSTRUMENT => "ajustar con tipo anadir_partes",
        id::SCORE_INFO => "ajustar con tipo titulo",
        id::GO_TO_BAR => "seleccionar",
        _ => return None,
    })
}
