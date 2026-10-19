//! The File menu as data, shared by the in-window dropdown and the native
//! macOS menu so both always list the same commands in the same order.

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
    C(id::PRINT),
    S,
    C(id::PREFERENCES),
    S,
    C(id::CLOSE),
];

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
    use score_keymap::{Profile, UsLayout, UserKeymap};

    struct Defaults;
    impl CommandStates for Defaults {
        fn state(&self, _: CommandId) -> CommandState {
            CommandState::default()
        }
    }

    #[test]
    fn file_menu_lists_registered_commands_with_keys_and_recent_files() {
        let registry = Registry::builtin();
        let (keymap, _) = Keymap::resolve(
            &registry,
            Profile::sibelius_laptop(),
            &UserKeymap::new(),
            &UsLayout,
        );
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
}
