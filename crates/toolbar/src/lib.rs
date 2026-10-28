//! Presentation model of the ribbon and the File menu, independent of GPUI.
//!
//! `spec` says what exists, `project` turns it into what is drawn for the
//! current state and window width, and `menu` does the same for File. The app
//! answers [`CommandStates`]; the GPUI layer only paints the views and emits
//! commands.

pub mod marks;
mod menu;
mod project;
mod search;
mod spec;

pub use menu::{
    BAR_MARK_CONTEXT_MENU, FILE_MENU, FILE_MENU_WITH_APP_ITEMS, IN_WINDOW_FILE_MENU,
    MARK_CONTEXT_MENU, MenuEntry, MenuItemView, MenuRow, NOTE_CONTEXT_MENU, RecentItem, item_view,
    project_menu,
};
pub use project::{
    CommandState, CommandStates, ControlView, GroupMode, GroupView, Item, Metrics, ToolbarView,
    fit, project, tile_width,
};
pub use search::{SearchResult, normalize, ribbon_place, search};
pub use spec::{Control, ControlSize, GroupSpec, Role, TABS, TabId, TabSpec, Visual, tab};
