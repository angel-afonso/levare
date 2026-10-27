//! GPUI side of the shortcut engine: one action per command, bindings with
//! key contexts, the user's file and the native menus.

use crate::keyboard::SystemLayout;
use gpui::{Action, App, KeyBinding, Menu, MenuItem};
use score_core::{Instrument, KeyMode, KeySignature};
use score_keymap::{
    CommandId, CommandSpec, KeySequence, Keymap, Profile, Registry, UserKeymap, id,
};
use std::path::PathBuf;

/// Runs a command from a key binding. Repeats of a held key are filtered for
/// commands that do not repeat.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct Run(pub CommandId);

/// Runs a command from a native menu item. Menu items only advertise ⌘/⌃
/// shortcuts: a single-key equivalent would let macOS fire the menu before
/// GPUI's key contexts and the Preferences recorder see the key.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct MenuRun(pub CommandId);

#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct AddInstrument(pub Instrument);

#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct ChooseKeySignature(pub KeySignature);

#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct ChooseDynamic(pub Option<score_core::Dynamic>);

/// Opens one of the recent files listed in Archivo › Abrir reciente.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = levare, no_json)]
pub struct OpenRecent(pub PathBuf);

pub struct Shortcuts {
    pub registry: Registry,
    pub keymap: Keymap,
    pub user: UserKeymap,
    pub layout: SystemLayout,
    pub path: Option<PathBuf>,
    /// Messages about the user's file and unusable keys, shown in Preferences.
    pub problems: Vec<String>,
    /// Snapshot of the recent files for the native Archivo menu.
    pub recent: Vec<PathBuf>,
}

impl Shortcuts {
    pub fn load(cx: &App) -> Self {
        let path = crate::data_path("atajos.json");
        let mut problems = Vec::new();
        let user = match path.as_deref().map(UserKeymap::load) {
            Some(Ok(user)) => user,
            Some(Err(error)) => {
                problems.push(format!("No se pudieron leer tus atajos: {error}"));
                UserKeymap::new()
            }
            None => UserKeymap::new(),
        };
        let mut shortcuts = Self {
            registry: Registry::builtin(),
            keymap: Keymap::resolve(
                &Registry::builtin(),
                Profile::sibelius_laptop(),
                &UserKeymap::new(),
                &SystemLayout::current(cx.keyboard_layout().name()),
            )
            .0,
            user,
            layout: SystemLayout::current(cx.keyboard_layout().name()),
            path,
            problems,
            recent: Vec::new(),
        };
        shortcuts.rebuild();
        shortcuts
    }

    fn profile(&self) -> Profile {
        self.user
            .profile
            .as_deref()
            .and_then(Profile::builtin)
            .unwrap_or_else(Profile::sibelius_laptop)
    }

    /// Resolves the profile and the user's changes for the current layout.
    pub fn rebuild(&mut self) {
        let (keymap, problems) =
            Keymap::resolve(&self.registry, self.profile(), &self.user, &self.layout);
        self.keymap = keymap;
        self.problems
            .retain(|problem| problem.starts_with("No se pudieron"));
        self.problems
            .extend(problems.into_iter().map(|problem| problem.to_string()));
    }

    pub fn refresh_layout(&mut self, cx: &App) {
        self.layout = SystemLayout::current(cx.keyboard_layout().name());
        self.rebuild();
    }

    pub fn spec(&self, command: CommandId) -> Option<&CommandSpec> {
        self.registry.get(command.as_str())
    }

    pub fn label(&self, command: CommandId) -> Option<String> {
        self.keymap.label(command)
    }

    /// How a status message names the key of `command`: its label on this
    /// platform, or the command's name in «» when the user removed its keys.
    pub fn hint(&self, command: CommandId) -> String {
        self.label(command).unwrap_or_else(|| {
            format!(
                "«{}»",
                self.spec(command)
                    .map_or(command.as_str(), |spec| spec.label)
            )
        })
    }

    /// Persists the user's changes and re-resolves the keymap.
    pub fn save(&mut self) -> Result<(), String> {
        self.rebuild();
        match &self.path {
            Some(path) => self.user.save(path).map_err(|error| error.to_string()),
            None => Err("No hay carpeta de datos para guardar los atajos".into()),
        }
    }

    pub fn key_bindings(&self) -> Vec<KeyBinding> {
        self.keymap
            .bindings()
            .iter()
            .filter_map(|binding| {
                let keys = binding.platform.as_ref()?;
                // GPUI panics on text it cannot parse; such keys are skipped.
                keys.split_whitespace()
                    .all(|stroke| gpui::Keystroke::parse(stroke).is_ok())
                    .then(|| {
                        KeyBinding::new(keys, Run(binding.command), Some(binding.scope.predicate()))
                    })
            })
            .collect()
    }

    /// Replaces every GPUI binding and rebuilds the menus so their shortcuts match.
    pub fn install(&self, cx: &mut App) {
        cx.clear_key_bindings();
        cx.bind_keys(self.key_bindings());
        cx.bind_keys(crate::ui::text_input::bindings());
        crate::menu_typography::install(self.menus(), cx);
    }

    fn item(&self, command: CommandId) -> MenuItem {
        let label = self
            .spec(command)
            .map_or(command.as_str(), |spec| spec.label);
        let keys = self.keymap.keys_for(command);
        let with_modifier = keys
            .first()
            .is_some_and(|keys| keys.0.iter().all(|stroke| stroke.secondary || stroke.ctrl));
        if with_modifier {
            MenuItem::action(label, Run(command))
        } else if let Some(keys) = keys.first() {
            MenuItem::action(format!("{label}  ({})", keys.label()), MenuRun(command))
        } else {
            MenuItem::action(label, MenuRun(command))
        }
    }

    fn items(&self, commands: &[Option<CommandId>]) -> Vec<MenuItem> {
        commands
            .iter()
            .map(|command| command.map_or_else(MenuItem::separator, |command| self.item(command)))
            .collect()
    }

    /// Archivo, from the same description as the in-window dropdown.
    fn file_menu(&self) -> Vec<MenuItem> {
        score_toolbar::FILE_MENU
            .iter()
            .map(|entry| match entry {
                score_toolbar::MenuEntry::Command(command) => self.item(*command),
                score_toolbar::MenuEntry::Separator => MenuItem::separator(),
                score_toolbar::MenuEntry::Recent => MenuItem::submenu(Menu {
                    name: "Abrir reciente".into(),
                    items: self
                        .recent
                        .iter()
                        .map(|path| MenuItem::action(recent_title(path), OpenRecent(path.clone())))
                        .chain((!self.recent.is_empty()).then(MenuItem::separator))
                        .chain([self.item(id::CLEAR_RECENT)])
                        .collect(),
                }),
            })
            .collect()
    }

    pub fn menus(&self) -> Vec<Menu> {
        let instruments = Instrument::ALL
            .into_iter()
            .map(|instrument| {
                MenuItem::action(
                    format!("Añadir {}", instrument.name()),
                    AddInstrument(instrument),
                )
            })
            .chain(self.items(&[
                None,
                Some(id::ADD_INSTRUMENT),
                Some(id::NEXT_INSTRUMENT),
                Some(id::PREVIOUS_INSTRUMENT),
                Some(id::REMOVE_INSTRUMENT),
                Some(id::DIVIDE_PART),
                Some(id::JOIN_PART),
                None,
                Some(id::RIGHT_HAND),
                Some(id::LEFT_HAND),
            ]))
            .collect();
        let keys = [KeyMode::Major, KeyMode::Minor]
            .into_iter()
            .enumerate()
            .flat_map(|(index, mode)| {
                let mut items = Vec::new();
                if index > 0 {
                    items.push(MenuItem::separator());
                }
                for fifths in [0, 1, 2, 3, 4, 5, 6, 7, -1, -2, -3, -4, -5, -6, -7] {
                    let key = KeySignature::new(fifths, mode).expect("valid key");
                    items.push(MenuItem::action(key.label(), ChooseKeySignature(key)));
                }
                items
            })
            .collect();
        let expression = self
            .items(&[
                Some(id::CLEF),
                Some(id::TEMPO_TEXT),
                Some(id::REHEARSAL_MARK),
                Some(id::REHEARSAL_LETTERS),
                Some(id::REHEARSAL_NUMBERS),
                Some(id::REHEARSAL_BAR_NUMBERS),
                None,
                Some(id::STACCATO),
                Some(id::TENUTO),
                Some(id::ACCENT),
                Some(id::MARCATO),
                Some(id::CLEAR_ARTICULATION),
                Some(id::FERMATA),
                Some(id::TREMOLO_1),
                Some(id::TREMOLO_2),
                Some(id::TREMOLO_3),
                None,
                Some(id::ARPEGGIO),
                Some(id::ARPEGGIO_UP),
                Some(id::ARPEGGIO_DOWN),
                Some(id::ARPEGGIO_BOTH_HANDS),
                None,
                Some(id::CRESCENDO),
                Some(id::DIMINUENDO),
                Some(id::REMOVE_CRESCENDO),
                Some(id::REMOVE_DIMINUENDO),
                None,
                Some(id::PIZZICATO),
                Some(id::ARCO),
                Some(id::CLEAR_TECHNIQUE),
                None,
                Some(id::HARMONIC_NATURAL),
                Some(id::HARMONIC_ARTIFICIAL),
                None,
                Some(id::DIVISI),
                Some(id::UNISON),
                None,
            ])
            .into_iter()
            .chain(
                score_core::Dynamic::ALL
                    .into_iter()
                    .map(|mark| MenuItem::action(mark.label(), ChooseDynamic(Some(mark)))),
            )
            .chain([
                MenuItem::action("Quitar dinámica", ChooseDynamic(None)),
                MenuItem::separator(),
            ])
            .chain(self.items(&[
                Some(id::TIE),
                Some(id::SLUR),
                Some(id::REMOVE_TIE),
                Some(id::REMOVE_SLUR),
            ]))
            .collect();
        vec![
            Menu {
                name: "Levare".into(),
                items: self.items(&[
                    Some(id::CHECK_FOR_UPDATES),
                    Some(id::RESTART_TO_UPDATE),
                    Some(id::AUTOMATIC_UPDATES),
                    None,
                    Some(id::PREFERENCES),
                    None,
                    Some(id::QUIT),
                ]),
            },
            Menu {
                name: "Archivo".into(),
                items: self.file_menu(),
            },
            Menu {
                name: "Edición".into(),
                items: self.items(&[
                    Some(id::UNDO),
                    Some(id::REDO),
                    None,
                    Some(id::CUT),
                    Some(id::COPY),
                    Some(id::PASTE),
                    Some(id::SELECT_ALL),
                    None,
                    Some(id::REPEAT),
                    Some(id::DELETE),
                    Some(id::REMOVE_TIME),
                    Some(id::RESET_POSITION),
                    None,
                    Some(id::BAR_AT_END),
                    Some(id::SINGLE_BAR),
                    Some(id::DELETE_BAR),
                    None,
                    Some(id::VOICE_1),
                    Some(id::VOICE_2),
                    Some(id::TOGGLE_NOTE_INPUT),
                    Some(id::CHORD_MODE),
                    None,
                    Some(id::DOT),
                    Some(id::TRIPLET),
                    Some(id::REMOVE_HIGHEST),
                    None,
                    Some(id::SHARP),
                    Some(id::FLAT),
                    Some(id::NATURAL),
                    Some(id::FOLLOW_KEY),
                    Some(id::UPPER_NOTE_OR_STAFF),
                    Some(id::LOWER_NOTE_OR_STAFF),
                ]),
            },
            Menu {
                name: "Expresión".into(),
                items: expression,
            },
            Menu {
                name: "Repeticiones".into(),
                items: self.items(&[
                    Some(id::REPEAT_START),
                    Some(id::REPEAT_END),
                    None,
                    Some(id::ENDING_1),
                    Some(id::ENDING_2),
                    Some(id::REMOVE_ENDING),
                    None,
                    Some(id::DOUBLE_BARLINE),
                    Some(id::FINAL_BARLINE),
                    None,
                    Some(id::SEGNO),
                    Some(id::CODA),
                    Some(id::TO_CODA),
                    Some(id::FINE),
                    None,
                    Some(id::DA_CAPO),
                    Some(id::DA_CAPO_AL_FINE),
                    Some(id::DA_CAPO_AL_CODA),
                    Some(id::DAL_SEGNO),
                    Some(id::DAL_SEGNO_AL_FINE),
                    Some(id::DAL_SEGNO_AL_CODA),
                    None,
                    Some(id::CLEAR_FORM),
                ]),
            },
            Menu {
                name: "Instrumentos".into(),
                items: instruments,
            },
            Menu {
                name: "Tonalidad".into(),
                items: keys,
            },
            Menu {
                name: "Reproducción".into(),
                items: self.items(&[
                    Some(id::PLAY),
                    Some(id::PLAY_FROM_SELECTION),
                    Some(id::PLAY_FROM_START),
                    Some(id::STOP),
                    None,
                    Some(id::MIXER),
                ]),
            },
            Menu {
                name: "Vista".into(),
                items: self.items(&[
                    Some(id::ZOOM_IN),
                    Some(id::ZOOM_OUT),
                    Some(id::ZOOM_ACTUAL),
                    Some(id::ZOOM_FIT),
                    None,
                    Some(id::PAGE_VIEW),
                    Some(id::CONTINUOUS_VIEW),
                    Some(id::SINGLE_SYSTEM_VIEW),
                    None,
                    Some(id::FULL_SCREEN),
                    Some(id::TOGGLE_INSTRUMENTS_PANEL),
                    Some(id::TOGGLE_INSPECTOR),
                    Some(id::TOGGLE_AGENT_PANEL),
                ]),
            },
        ]
    }

    /// The keys of `command` after adding or replacing `keys`.
    pub fn with_keys(&self, command: CommandId, keys: KeySequence, add: bool) -> Vec<KeySequence> {
        let mut list: Vec<KeySequence> = if add {
            self.keymap.keys_for(command).into_iter().cloned().collect()
        } else {
            Vec::new()
        };
        if !list.contains(&keys) {
            list.push(keys);
        }
        list
    }

    pub fn without_keys(&self, command: CommandId, keys: &KeySequence) -> Vec<KeySequence> {
        self.keymap
            .keys_for(command)
            .into_iter()
            .filter(|bound| *bound != keys)
            .cloned()
            .collect()
    }
}

/// File name of a recent score, for menus.
pub fn recent_title(path: &std::path::Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Folder of a recent score, with the home directory shortened to `~`.
pub fn recent_folder(path: &std::path::Path) -> String {
    let folder = path.parent().unwrap_or(path);
    match dirs::home_dir().and_then(|home| folder.strip_prefix(home).ok().map(PathBuf::from)) {
        Some(relative) if relative.as_os_str().is_empty() => "~".into(),
        Some(relative) => format!("~/{}", relative.display()),
        None => folder.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use score_keymap::{Layout, Platform, UsLayout};

    struct Spanish;
    impl Layout for Spanish {
        fn shifted_digit(&self, digit: u8) -> Option<String> {
            "=!\"·$%&/()"
                .chars()
                .nth(usize::from(digit))
                .map(String::from)
        }
    }

    #[test]
    fn every_default_binding_is_valid_gpui_text_on_us_and_spanish_layouts() {
        let layouts: [&dyn Layout; 2] = [&UsLayout, &Spanish];
        for (layout, platform) in layouts
            .into_iter()
            .flat_map(|layout| Platform::ALL.map(|platform| (layout, platform)))
        {
            let (keymap, problems) = Keymap::resolve_for(
                &Registry::builtin(),
                Profile::sibelius_laptop_for(platform),
                &UserKeymap::new(),
                layout,
                platform,
            );
            assert!(problems.is_empty(), "{problems:?}");
            for binding in keymap.bindings() {
                let text = binding.platform.as_ref().expect("typeable key");
                for stroke in text.split_whitespace() {
                    assert!(gpui::Keystroke::parse(stroke).is_ok(), "{text}");
                }
                gpui::KeyBindingContextPredicate::parse(binding.scope.predicate()).unwrap();
            }
        }
    }

    #[test]
    fn commands_that_left_the_ribbon_stay_in_the_native_menus() {
        // The clipboard and the panels left the ribbon (score-toolbar's
        // `nothing_leaves_the_app` lists them); Edición and Vista keep them.
        let shortcuts = Shortcuts {
            registry: Registry::builtin(),
            keymap: Keymap::resolve(
                &Registry::builtin(),
                Profile::sibelius_laptop(),
                &UserKeymap::new(),
                &UsLayout,
            )
            .0,
            user: UserKeymap::new(),
            layout: SystemLayout::current(""),
            path: None,
            problems: Vec::new(),
            recent: Vec::new(),
        };
        fn commands(items: &[MenuItem], found: &mut Vec<CommandId>) {
            for item in items {
                match item {
                    MenuItem::Action { action, .. } => found.extend(
                        action
                            .as_any()
                            .downcast_ref::<Run>()
                            .map(|run| run.0)
                            .or_else(|| action.as_any().downcast_ref::<MenuRun>().map(|run| run.0)),
                    ),
                    MenuItem::Submenu(menu) => commands(&menu.items, found),
                    _ => {}
                }
            }
        }
        let mut found = Vec::new();
        for menu in shortcuts.menus() {
            commands(&menu.items, &mut found);
        }
        for command in [
            id::PASTE,
            id::COPY,
            id::CUT,
            id::REPEAT,
            id::SELECT_ALL,
            id::TOGGLE_INSTRUMENTS_PANEL,
            id::TOGGLE_INSPECTOR,
            id::FULL_SCREEN,
        ] {
            assert!(found.contains(&command), "{command}");
        }
    }

    #[test]
    fn gpui_keystrokes_match_what_macos_reports() {
        let (keymap, _) = Keymap::resolve_for(
            &Registry::builtin(),
            Profile::sibelius_laptop_for(Platform::MacOs),
            &UserKeymap::new(),
            &Spanish,
            Platform::MacOs,
        );
        let platform = |command: CommandId| {
            keymap
                .bindings()
                .iter()
                .find(|binding| binding.command == command)
                .and_then(|binding| binding.platform.clone())
                .unwrap()
        };
        // Shift+3 on a Spanish layout arrives as "·" without Shift.
        let typed = gpui::Keystroke::parse("·").unwrap();
        assert_eq!(
            gpui::Keystroke::parse(&platform(id::INTERVAL_3)).unwrap(),
            typed
        );
        assert_eq!(
            gpui::Keystroke::parse(&platform(id::ADD_E)).unwrap(),
            gpui::Keystroke::parse("shift-e").unwrap()
        );
        assert_eq!(platform(id::ZOOM_OUT), "cmd--");
    }
}
