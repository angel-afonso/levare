//! The start view: a new score from an ensemble template, a recent score or
//! an example, offered when Levare opens with nothing to show and from
//! Archivo › Nueva partitura desde plantilla…. Drawn by `ui::start_screen`.

use crate::app_paths::data_path;
use crate::{AppView, Next};
use gpui::{Context, Keystroke, ScrollHandle, Window};
use score_core::{Score, TEMPLATES};
use score_keymap::{CommandId, id};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// An example score that ships inside Levare and opens as an untitled copy.
pub struct Example {
    pub name: &'static str,
    pub detail: &'static str,
    bytes: &'static [u8],
}

/// A few small scores from the repository's `examples`, written for Levare.
pub const EXAMPLES: &[Example] = &[
    Example {
        name: "Piano a dos manos",
        detail: "Piano · Mano derecha y mano izquierda",
        bytes: include_bytes!("../../../examples/piano-dos-manos.partitura"),
    },
    Example {
        name: "Ligaduras y expresión",
        detail: "Piano · Dinámicas, articulaciones y ligaduras",
        bytes: include_bytes!("../../../examples/piano-expresion.partitura"),
    },
    Example {
        name: "Trío",
        detail: "Flauta, violoncello y piano",
        bytes: include_bytes!("../../../examples/trio.partitura"),
    },
    Example {
        name: "Cuarteto en Sol",
        detail: "Flauta y cuarteto de cuerda",
        bytes: include_bytes!("../../../examples/cuarteto-en-sol.partitura"),
    },
];

impl Example {
    pub fn score(&self) -> Result<Score, score_io::FileError> {
        score_io::decode(self.bytes)
    }
}

/// A score that replaces the open one without coming from a file: it opens
/// untitled, so saving it asks where.
#[derive(Clone)]
pub struct Fresh {
    pub score: Score,
    pub status: String,
}

/// One thing the view offers, in the order Tab walks them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Template(usize),
    Recent(usize),
    Example(usize),
    Open,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Move {
    Next,
    Previous,
    Left,
    Right,
}

/// Every entry, in order, with `recent` scores in the list.
fn entries(recent: usize) -> Vec<Entry> {
    (0..TEMPLATES.len())
        .map(Entry::Template)
        .chain((0..recent).map(Entry::Recent))
        .chain((0..EXAMPLES.len()).map(Entry::Example))
        .chain([Entry::Open, Entry::Empty])
        .collect()
}

/// Where a key takes the selection. Up and down stop at the ends, Tab goes
/// round; left and right cross between the templates and the column of
/// recent scores and examples, or between the two buttons.
fn step(selected: Entry, movement: Move, recent: usize) -> Entry {
    let all = entries(recent);
    let index = all.iter().position(|entry| *entry == selected).unwrap_or(0);
    let side = recent + EXAMPLES.len();
    let beside = |row: usize| {
        if row < recent {
            Entry::Recent(row)
        } else {
            Entry::Example(row - recent)
        }
    };
    match movement {
        Move::Next => all[(index + 1).min(all.len() - 1)],
        Move::Previous => all[index.saturating_sub(1)],
        Move::Right => match selected {
            Entry::Template(row) if side > 0 => beside(row.min(side - 1)),
            Entry::Open => Entry::Empty,
            other => other,
        },
        Move::Left => match selected {
            Entry::Recent(row) => Entry::Template(row.min(TEMPLATES.len() - 1)),
            Entry::Example(row) => Entry::Template((recent + row).min(TEMPLATES.len() - 1)),
            Entry::Empty => Entry::Open,
            other => other,
        },
    }
}

/// The entry after `selected`, going round at the end.
fn tab(selected: Entry, backwards: bool, recent: usize) -> Entry {
    let all = entries(recent);
    let index = all.iter().position(|entry| *entry == selected).unwrap_or(0);
    let next = if backwards {
        index.checked_sub(1).unwrap_or(all.len() - 1)
    } else {
        (index + 1) % all.len()
    };
    all[next]
}

/// Where each column scrolls to show `entry`: the templates, or the recent
/// scores and examples, whose captions are children too.
fn scroll_rows(entry: Entry, recent: usize) -> Option<(bool, usize)> {
    match entry {
        Entry::Template(row) => Some((true, row + 1)),
        // «Recientes», its rows (or the line saying there are none),
        // «Ejemplos» and its rows.
        Entry::Recent(row) => Some((false, row + 1)),
        Entry::Example(row) => Some((false, recent.max(1) + 2 + row)),
        Entry::Open | Entry::Empty => None,
    }
}

/// What is chosen once the recent score at `index` left the list, with
/// `left` scores still in it: the one that took its place, the new last one,
/// or the first example.
fn after_removal(index: usize, left: usize) -> Entry {
    if index < left {
        Entry::Recent(index)
    } else if left > 0 {
        Entry::Recent(left - 1)
    } else {
        Entry::Example(0)
    }
}

pub struct StartScreen {
    pub selected: Entry,
    /// «Mostrar al abrir Levare», as `inicio.json` holds it.
    pub show_at_launch: bool,
    /// Said in the view, which covers the status bar: a recent score that
    /// is gone, an example that would not open.
    pub message: Option<String>,
    pub templates: ScrollHandle,
    pub sources: ScrollHandle,
}

impl StartScreen {
    fn new(recent: usize, show_at_launch: bool) -> Self {
        Self {
            // Enter opens the last score; Esc leaves the empty one.
            selected: if recent > 0 {
                Entry::Recent(0)
            } else {
                Entry::Template(0)
            },
            show_at_launch,
            message: None,
            templates: ScrollHandle::new(),
            sources: ScrollHandle::new(),
        }
    }

    /// The view Levare opens with, unless a file was named on its command
    /// line or the musician turned it off.
    pub fn at_launch(recent: usize) -> Option<Self> {
        let show = shown_at_launch(settings_path().as_deref());
        (show && !crate::system_open::launched_with_files()).then(|| Self::new(recent, show))
    }

    fn select(&mut self, entry: Entry, recent: usize) {
        self.selected = entry;
        if let Some((templates, row)) = scroll_rows(entry, recent) {
            if templates {
                self.templates.scroll_to_item(row);
            } else {
                self.sources.scroll_to_item(row);
            }
        }
    }
}

/// `inicio.json` in the data folder.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    mostrar_al_iniciar: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mostrar_al_iniciar: true,
        }
    }
}

fn settings_path() -> Option<PathBuf> {
    data_path("inicio.json")
}

/// A missing or damaged file shows the view: it is never worth an error.
fn shown_at_launch(path: Option<&Path>) -> bool {
    path.and_then(|path| fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default()
        .mostrar_al_iniciar
}

fn remember(path: &Path, show: bool) -> io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let settings = Settings {
        mostrar_al_iniciar: show,
    };
    atomic_write::write(path, &serde_json::to_vec_pretty(&settings)?)
}

impl AppView {
    /// Archivo › Nueva partitura desde plantilla….
    pub(crate) fn open_start(&mut self, cx: &mut Context<Self>) {
        if self.start.is_some()
            || self.busy
            || self.confirming
            || self.agent_locked()
            || self.preferences.is_some()
            || self.instrument_picker.is_some()
        {
            return;
        }
        let show = shown_at_launch(settings_path().as_deref());
        self.start = Some(StartScreen::new(self.recent.files().len(), show));
        self.ui.file_menu = false;
        self.ui.recent_menu = false;
        self.ui.open_group = None;
        self.playback.stop();
        cx.notify();
    }

    /// Back to the score underneath: at launch, the empty one.
    pub(crate) fn close_start(&mut self, cx: &mut Context<Self>) {
        if self.start.take().is_some() {
            cx.notify();
        }
    }

    pub(crate) fn toggle_start_at_launch(&mut self, cx: &mut Context<Self>) {
        let Some(start) = &mut self.start else {
            return;
        };
        let show = !start.show_at_launch;
        start.show_at_launch = show;
        start.message = settings_path()
            .ok_or_else(|| io::Error::other("no se encontró la carpeta de datos de Levare"))
            .and_then(|path| remember(&path, show))
            .err()
            .map(|error| format!("No se pudo guardar la preferencia: {error}"));
        cx.notify();
    }

    pub(crate) fn choose_start(
        &mut self,
        entry: Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let recent = self.recent.files().len();
        if let Some(start) = &mut self.start {
            start.select(entry, recent);
            start.message = None;
        }
        match entry {
            Entry::Template(index) => {
                let Some(template) = TEMPLATES.get(index) else {
                    return;
                };
                match template.score() {
                    Ok(score) => {
                        let status = format!(
                            "Nueva partitura · {} · haz clic en el título para cambiarlo",
                            template.name
                        );
                        self.request_next(
                            Next::Fresh(Box::new(Fresh { score, status })),
                            window,
                            cx,
                        )
                    }
                    Err(error) => self.start_message(error.to_string(), cx),
                }
            }
            Entry::Recent(index) => {
                let Some(path) = self.recent.files().get(index).cloned() else {
                    return;
                };
                if path.exists() {
                    self.request_next(Next::OpenPath(path), window, cx);
                } else {
                    self.recent.remove(&path);
                    self.save_recent(cx);
                    let name = crate::shortcuts::recent_title(&path);
                    let left = self.recent.files().len();
                    if let Some(start) = &mut self.start {
                        start.select(after_removal(index, left), left);
                    }
                    self.start_message(
                        format!("«{name}» ya no está donde se guardó; se quitó de Recientes."),
                        cx,
                    );
                }
            }
            Entry::Example(index) => {
                let Some(example) = EXAMPLES.get(index) else {
                    return;
                };
                match example.score() {
                    Ok(score) => {
                        let status = format!(
                            "Ejemplo «{}» abierto como copia sin título: guárdalo para conservar \
                             tus cambios",
                            example.name
                        );
                        self.request_next(
                            Next::Fresh(Box::new(Fresh { score, status })),
                            window,
                            cx,
                        )
                    }
                    Err(error) => {
                        self.start_message(format!("No se pudo abrir el ejemplo: {error}"), cx)
                    }
                }
            }
            Entry::Open => self.request_next(Next::Open, window, cx),
            Entry::Empty => self.request_next(Next::New, window, cx),
        }
    }

    fn start_message(&mut self, message: String, cx: &mut Context<Self>) {
        if let Some(start) = &mut self.start {
            start.message = Some(message);
        }
        cx.notify();
    }

    /// A key while the view is open. It takes the keys without a command
    /// modifier; the others (⌘O, ⌘Q, the command search) go on to `run`.
    pub(crate) fn start_key(
        &mut self,
        stroke: &Keystroke,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let modifiers = stroke.modifiers;
        if modifiers.platform || modifiers.control || modifiers.alt || modifiers.function {
            return false;
        }
        let recent = self.recent.files().len();
        let Some(start) = &mut self.start else {
            return false;
        };
        let selected = start.selected;
        let next = match stroke.key.as_str() {
            "escape" => {
                self.close_start(cx);
                return true;
            }
            "enter" | "space" => {
                self.choose_start(selected, window, cx);
                return true;
            }
            "tab" => tab(selected, modifiers.shift, recent),
            "down" => step(selected, Move::Next, recent),
            "up" => step(selected, Move::Previous, recent),
            "left" => step(selected, Move::Left, recent),
            "right" => step(selected, Move::Right, recent),
            _ => return true,
        };
        start.select(next, recent);
        cx.notify();
        true
    }

    /// Whether `command` runs with the view open. Opening a score keeps it
    /// until the score is open, so a cancelled dialog comes back to it; Esc
    /// closes it; any other command closes it and runs.
    pub(crate) fn start_allows(&mut self, command: CommandId, cx: &mut Context<Self>) -> bool {
        if self.start.is_none() {
            return true;
        }
        if [id::OPEN, id::START_SCREEN, id::CLOSE, id::QUIT].contains(&command) {
            return true;
        }
        self.close_start(cx);
        ![id::ESCAPE, id::CLOSE_PANEL].contains(&command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_examples_open_as_untitled_copies() {
        for example in EXAMPLES {
            let score = example
                .score()
                .unwrap_or_else(|error| panic!("{}: {error}", example.name));
            let document = score_document::Document::new(score)
                .unwrap_or_else(|error| panic!("{}: {error}", example.name));
            assert!(!document.is_dirty(), "{}", example.name);
            assert!(!example.detail.is_empty());
        }
        let total: usize = EXAMPLES.iter().map(|example| example.bytes.len()).sum();
        assert!(total < 200_000, "the examples stay small: {total} bytes");
    }

    #[test]
    fn arrows_walk_the_entries_and_stop_at_the_ends() {
        let first = Entry::Template(0);
        assert_eq!(step(first, Move::Previous, 2), first);
        assert_eq!(step(first, Move::Next, 2), Entry::Template(1));
        let last_template = Entry::Template(TEMPLATES.len() - 1);
        assert_eq!(step(last_template, Move::Next, 2), Entry::Recent(0));
        assert_eq!(step(last_template, Move::Next, 0), Entry::Example(0));
        assert_eq!(step(Entry::Recent(1), Move::Next, 2), Entry::Example(0));
        let last_example = Entry::Example(EXAMPLES.len() - 1);
        assert_eq!(step(last_example, Move::Next, 2), Entry::Open);
        assert_eq!(step(Entry::Open, Move::Next, 2), Entry::Empty);
        assert_eq!(step(Entry::Empty, Move::Next, 2), Entry::Empty);
    }

    #[test]
    fn left_and_right_cross_columns() {
        assert_eq!(step(Entry::Template(1), Move::Right, 3), Entry::Recent(1));
        assert_eq!(step(Entry::Template(4), Move::Right, 3), Entry::Example(1));
        assert_eq!(step(Entry::Template(1), Move::Right, 0), Entry::Example(1));
        let far = Entry::Template(TEMPLATES.len() - 1);
        assert_eq!(
            step(far, Move::Right, 0),
            Entry::Example(EXAMPLES.len() - 1),
            "the last row of the shorter column"
        );
        assert_eq!(step(Entry::Recent(2), Move::Left, 3), Entry::Template(2));
        assert_eq!(step(Entry::Example(1), Move::Left, 3), Entry::Template(4));
        assert_eq!(step(Entry::Template(0), Move::Left, 3), Entry::Template(0));
        assert_eq!(step(Entry::Recent(0), Move::Right, 3), Entry::Recent(0));
        assert_eq!(step(Entry::Open, Move::Right, 3), Entry::Empty);
        assert_eq!(step(Entry::Empty, Move::Left, 3), Entry::Open);
    }

    #[test]
    fn tab_goes_round() {
        assert_eq!(tab(Entry::Empty, false, 1), Entry::Template(0));
        assert_eq!(tab(Entry::Template(0), true, 1), Entry::Empty);
        assert_eq!(tab(Entry::Recent(0), false, 1), Entry::Example(0));
        assert_eq!(entries(3).len(), TEMPLATES.len() + 3 + EXAMPLES.len() + 2);
    }

    #[test]
    fn the_command_is_registered_in_archivo_and_found_by_its_name() {
        let registry = score_keymap::Registry::builtin();
        let spec = registry.get(id::START_SCREEN.as_str()).unwrap();
        assert_eq!(spec.category, score_keymap::Category::File);
        assert_eq!(spec.label, "Nueva partitura desde plantilla…");
        assert!(
            score_toolbar::IN_WINDOW_FILE_MENU
                .contains(&score_toolbar::MenuEntry::Command(id::START_SCREEN))
        );
        assert!(agent_tools::EXCLUDED.contains(&id::START_SCREEN));
    }

    #[test]
    fn a_missing_recent_score_hands_the_choice_on() {
        assert_eq!(after_removal(0, 2), Entry::Recent(0));
        assert_eq!(after_removal(2, 2), Entry::Recent(1));
        assert_eq!(after_removal(0, 0), Entry::Example(0));
    }

    #[test]
    fn recent_scores_that_are_gone_are_not_offered() {
        let directory = tempfile::tempdir().unwrap();
        let kept = directory.path().join("kept.levare");
        let gone = directory.path().join("gone.levare");
        fs::write(&kept, "{}").unwrap();
        fs::write(&gone, "{}").unwrap();
        let mut recent = score_io::RecentFiles::new();
        recent.touch(&gone);
        recent.touch(&kept);
        let list = directory.path().join("recientes.json");
        recent.save(&list).unwrap();
        fs::remove_file(&gone).unwrap();
        let loaded = score_io::RecentFiles::load(&list).unwrap();
        assert_eq!(loaded.files().len(), 1, "the view lists what is there");
        assert_eq!(
            StartScreen::new(loaded.files().len(), true).selected,
            Entry::Recent(0)
        );
    }

    #[test]
    fn the_first_choice_is_the_last_score_if_there_is_one() {
        assert_eq!(StartScreen::new(2, true).selected, Entry::Recent(0));
        assert_eq!(StartScreen::new(0, true).selected, Entry::Template(0));
    }

    #[test]
    fn scrolling_counts_the_captions() {
        assert_eq!(scroll_rows(Entry::Template(0), 0), Some((true, 1)));
        assert_eq!(scroll_rows(Entry::Recent(2), 3), Some((false, 3)));
        assert_eq!(scroll_rows(Entry::Example(0), 3), Some((false, 5)));
        // With no recent scores, one line says so.
        assert_eq!(scroll_rows(Entry::Example(1), 0), Some((false, 4)));
        assert_eq!(scroll_rows(Entry::Open, 0), None);
    }

    #[test]
    fn the_preference_is_kept_in_inicio_json() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("Levare").join("inicio.json");
        assert!(shown_at_launch(Some(&path)), "shown until turned off");
        assert!(shown_at_launch(None));
        remember(&path, false).unwrap();
        assert!(!shown_at_launch(Some(&path)));
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"mostrar_al_iniciar\": false"), "{text}");
        remember(&path, true).unwrap();
        assert!(shown_at_launch(Some(&path)));
        fs::write(&path, "{ roto").unwrap();
        assert!(
            shown_at_launch(Some(&path)),
            "a damaged file shows the view"
        );
        fs::write(&path, "{}").unwrap();
        assert!(shown_at_launch(Some(&path)));
    }
}
