//! The File menu as data, shared by the in-window dropdown and the native
//! macOS menu so both always list the same commands in the same order.
//!
//! Only macOS draws native menus. On Windows and Linux the in-window dropdown
//! is the only File menu, so it also carries what macOS keeps in the Levare app
//! menu (`IN_WINDOW_FILE_MENU`).

use crate::project::CommandStates;
use score_keymap::{CommandId, Keymap, Registry, Status, id};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEntry {
    Command(CommandId),
    Separator,
    /// The "Abrir reciente" submenu, filled with the app's recent files.
    Recent,
}

use MenuEntry::{Command as C, Recent, Separator as S};

pub const FILE_MENU: &[MenuEntry] = &[
    C(id::NEW),
    C(id::OPEN),
    Recent,
    S,
    C(id::SAVE),
    C(id::SAVE_AS),
    C(id::EXPORT),
    C(id::EXPORT_MUSICXML),
    C(id::PRINT),
    S,
    C(id::SCORE_INFO),
    C(id::PREFERENCES),
    S,
    C(id::CLOSE),
];

/// `FILE_MENU` plus the Levare app menu's items, for systems without a native
/// menu bar: updates, and quit at the end as Windows and Linux apps do.
pub const FILE_MENU_WITH_APP_ITEMS: &[MenuEntry] = &[
    C(id::NEW),
    C(id::OPEN),
    Recent,
    S,
    C(id::SAVE),
    C(id::SAVE_AS),
    C(id::EXPORT),
    C(id::EXPORT_MUSICXML),
    C(id::PRINT),
    S,
    C(id::SCORE_INFO),
    C(id::PREFERENCES),
    S,
    C(id::CHECK_FOR_UPDATES),
    C(id::RESTART_TO_UPDATE),
    C(id::AUTOMATIC_UPDATES),
    C(id::ABOUT),
    S,
    C(id::REPORT_PROBLEM),
    C(id::SHOW_DIAGNOSTICS),
    S,
    C(id::CLOSE),
    C(id::QUIT),
];

/// The File dropdown drawn in the window on this system.
pub const IN_WINDOW_FILE_MENU: &[MenuEntry] = if cfg!(target_os = "macos") {
    FILE_MENU
} else {
    FILE_MENU_WITH_APP_ITEMS
};

pub const NOTE_CONTEXT_MENU: &[MenuEntry] = &[
    C(id::CUT),
    C(id::COPY),
    C(id::PASTE),
    S,
    C(id::DELETE),
    C(id::REMOVE_TIME),
    C(id::REPEAT),
    S,
    C(id::TIE),
    C(id::SLUR),
    C(id::CRESCENDO),
    C(id::DIMINUENDO),
    S,
    C(id::STACCATO),
    C(id::ACCENT),
];

pub const MARK_CONTEXT_MENU: &[MenuEntry] = &[C(id::DELETE), S, C(id::UNDO)];
pub const BAR_MARK_CONTEXT_MENU: &[MenuEntry] = &[C(id::RESET_POSITION), S, C(id::UNDO)];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItemView {
    pub command: CommandId,
    pub label: String,
    pub keys: Option<String>,
    pub enabled: bool,
    pub pending: bool,
}

/// A recent file as the menu shows it: index into the app's list, name and
/// folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentItem {
    pub index: usize,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuRow {
    Item(MenuItemView),
    Separator,
    Recent {
        label: &'static str,
        files: Vec<RecentItem>,
        clear: MenuItemView,
    },
}

pub fn item_view(
    command: CommandId,
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
) -> MenuItemView {
    let spec = registry.get(command.as_str());
    let state = states.state(command);
    MenuItemView {
        command,
        label: state
            .label
            .unwrap_or_else(|| spec.map_or(command.as_str(), |spec| spec.label).to_owned()),
        keys: keymap.label(command),
        enabled: state.enabled,
        pending: spec.is_some_and(|spec| spec.status == Status::Pending),
    }
}

pub fn project_menu(
    entries: &[MenuEntry],
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
    recent: &[RecentItem],
) -> Vec<MenuRow> {
    entries
        .iter()
        .map(|entry| match entry {
            MenuEntry::Command(command) => {
                MenuRow::Item(item_view(*command, registry, keymap, states))
            }
            MenuEntry::Separator => MenuRow::Separator,
            MenuEntry::Recent => MenuRow::Recent {
                label: "Abrir reciente",
                files: recent.to_vec(),
                clear: MenuItemView {
                    enabled: !recent.is_empty(),
                    ..item_view(id::CLEAR_RECENT, registry, keymap, states)
                },
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::CommandState;
    use score_keymap::{Platform, Profile, UsLayout, UserKeymap};

    struct Defaults;
    impl CommandStates for Defaults {
        fn state(&self, _: CommandId) -> CommandState {
            CommandState::default()
        }
    }

    #[test]
    fn file_menu_lists_registered_commands_with_keys_and_recent_files() {
        let registry = Registry::builtin();
        let keymap = |platform| {
            Keymap::resolve_for(
                &registry,
                Profile::sibelius_laptop_for(platform),
                &UserKeymap::new(),
                &UsLayout,
                platform,
            )
            .0
        };
        let windows = project_menu(
            FILE_MENU,
            &registry,
            &keymap(Platform::Windows),
            &Defaults,
            &[],
        );
        let MenuRow::Item(new) = &windows[0] else {
            panic!("first row is New")
        };
        assert_eq!(new.keys.as_deref(), Some("Ctrl+N"));
        let keymap = keymap(Platform::MacOs);
        let recent = [RecentItem {
            index: 0,
            title: "Trío.partitura".into(),
            detail: "~/Partituras".into(),
        }];
        let rows = project_menu(FILE_MENU, &registry, &keymap, &Defaults, &recent);
        assert_eq!(rows.len(), FILE_MENU.len());
        let MenuRow::Item(new) = &rows[0] else {
            panic!("first row is New")
        };
        assert_eq!(new.label, "Nueva partitura");
        assert_eq!(new.keys.as_deref(), Some("⌘N"));
        let MenuRow::Recent { files, clear, .. } = &rows[2] else {
            panic!("third row is the recent submenu")
        };
        assert_eq!(files[0].title, "Trío.partitura");
        assert!(clear.enabled);
        let empty = project_menu(FILE_MENU, &registry, &keymap, &Defaults, &[]);
        let MenuRow::Recent { clear, .. } = &empty[2] else {
            panic!()
        };
        assert!(!clear.enabled, "nothing to clear");
        assert!(
            rows.iter()
                .any(|row| matches!(row, MenuRow::Item(item) if item.pending))
        );
    }

    #[test]
    fn without_native_menus_file_carries_the_app_menu() {
        let registry = Registry::builtin();
        let commands = |entries: &[MenuEntry]| -> Vec<CommandId> {
            entries
                .iter()
                .filter_map(|entry| match entry {
                    MenuEntry::Command(command) => Some(*command),
                    _ => None,
                })
                .collect()
        };
        let with_app = commands(FILE_MENU_WITH_APP_ITEMS);
        for command in commands(FILE_MENU) {
            assert!(with_app.contains(&command), "{command}");
        }
        for command in [
            id::PREFERENCES,
            id::CHECK_FOR_UPDATES,
            id::RESTART_TO_UPDATE,
            id::AUTOMATIC_UPDATES,
            id::ABOUT,
            id::REPORT_PROBLEM,
            id::SHOW_DIAGNOSTICS,
            id::QUIT,
        ] {
            assert!(with_app.contains(&command), "{command}");
        }
        for command in with_app {
            assert!(registry.get(command.as_str()).is_some(), "{command}");
        }
        assert_eq!(
            FILE_MENU_WITH_APP_ITEMS.last(),
            Some(&C(id::QUIT)),
            "quit ends the menu"
        );
        assert_eq!(IN_WINDOW_FILE_MENU == FILE_MENU, cfg!(target_os = "macos"));
    }

    #[test]
    fn context_menus_only_contain_registered_commands() {
        let registry = Registry::builtin();
        for entries in [NOTE_CONTEXT_MENU, MARK_CONTEXT_MENU, BAR_MARK_CONTEXT_MENU] {
            assert!(!entries.contains(&MenuEntry::Recent));
            for entry in entries {
                if let MenuEntry::Command(command) = entry {
                    assert!(registry.get(command.as_str()).is_some(), "{command}");
                }
            }
        }
        assert!(BAR_MARK_CONTEXT_MENU.contains(&C(id::RESET_POSITION)));
        assert!(!BAR_MARK_CONTEXT_MENU.contains(&C(id::DELETE)));
    }
}
