//! Presentation model of the ribbon and the File menu, independent of GPUI.
//!
//! `spec` says what exists, `project` turns it into what is drawn for the
//! current state and window width, and `menu` does the same for File. The app
//! answers [`CommandStates`]; the GPUI layer only paints the views and emits
//! commands.

mod menu;
mod project;
mod spec;

pub use menu::{FILE_MENU, MenuEntry, MenuItemView, MenuRow, RecentItem, project_menu};
pub use project::{
    CommandState, CommandStates, ControlView, GroupMode, GroupView, Item, Metrics, ToolbarView,
    fit, project,
};
pub use spec::{Control, ControlSize, GroupSpec, TABS, TabId, TabSpec, Visual, tab};
