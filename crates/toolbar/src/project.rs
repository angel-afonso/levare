//! Projection of the static ribbon into what is drawn: labels from the
//! registry, keys from the active keymap, state from the app, uniform sizes
//! and how much room each group takes for the available width.

use crate::spec::{Control, ControlSize, GroupSpec, TabId, TabSpec, Visual};
use score_keymap::{CommandId, Keymap, Registry, Status};

/// What the app knows about a command right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandState {
    pub enabled: bool,
    /// Active option, toggled mode or current value (selected duration…).
    pub checked: bool,
    /// Replaces the label while the state lasts ("Terminar expresión").
    pub label: Option<String>,
}

impl Default for CommandState {
    fn default() -> Self {
        Self {
            enabled: true,
            checked: false,
            label: None,
        }
    }
}

impl CommandState {
    pub fn checked(checked: bool) -> Self {
        Self {
            checked,
            ..Self::default()
        }
    }
}

/// The application's side of the seam. Tests use a fake.
pub trait CommandStates {
    fn state(&self, command: CommandId) -> CommandState;
}

/// Sizes in logical pixels. Text widths are estimated per character, so the
/// fold decision does not need the toolkit's text system.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub large: f32,
    pub tile: f32,
    pub small_base: f32,
    pub char_width: f32,
    pub gap: f32,
    pub group_padding: f32,
    pub separator: f32,
    pub folded: f32,
    pub labeled_tile: f32,
    pub caption_char_width: f32,
    pub rows: usize,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            large: 58.,
            tile: 32.,
            small_base: 34.,
            char_width: 6.6,
            gap: 4.,
            group_padding: 16.,
            separator: 1.,
            folded: 64.,
            labeled_tile: 46.,
            caption_char_width: 5.6,
            rows: 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlView {
    pub command: CommandId,
    pub size: ControlSize,
    pub visual: Visual,
    pub label: String,
    /// Full registry label, for tooltips and accessibility.
    pub description: &'static str,
    pub keys: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub pending: bool,
}

/// One slot of a group, left to right. Consecutive small controls stack in
/// columns of `Metrics::rows`.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Large(ControlView),
    Tile(ControlView),
    /// A tile with its name under the glyph, when there is room for it.
    LabeledTile(ControlView),
    Column(Vec<ControlView>),
}

/// How much room a group takes, like the Office ribbon's scaling: folded
/// into one button, normal, or expanded with large buttons and named tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMode {
    Folded,
    Full,
    Expanded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupView {
    pub title: &'static str,
    pub visual: Visual,
    pub mode: GroupMode,
    /// The slots for `mode` (empty when folded).
    pub items: Vec<Item>,
    /// Every control, flat, for the folded group's dropdown.
    pub controls: Vec<ControlView>,
    pub width: f32,
    pub expanded_width: f32,
    pub folded_width: f32,
}

impl GroupView {
    pub fn width_in(&self, mode: GroupMode) -> f32 {
        match mode {
            GroupMode::Folded => self.folded_width,
            GroupMode::Full => self.width,
            GroupMode::Expanded => self.expanded_width,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarView {
    pub tab: TabId,
    pub groups: Vec<GroupView>,
}

impl ToolbarView {
    pub fn width(&self, metrics: &Metrics) -> f32 {
        total(
            self.groups.iter().map(|group| group.width_in(group.mode)),
            metrics,
        )
    }
}

fn total(widths: impl Iterator<Item = f32>, metrics: &Metrics) -> f32 {
    let mut count = 0.;
    let sum: f32 = widths.inspect(|_| count += 1.).sum();
    sum + (count - 1f32).max(0.) * metrics.separator
}

pub fn control_view(
    control: &Control,
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
) -> ControlView {
    let spec = registry.get(control.command.as_str());
    let description = spec.map_or(control.command.as_str(), |spec| spec.label);
    let state = states.state(control.command);
    ControlView {
        command: control.command,
        size: control.size,
        visual: control.visual,
        label: state
            .label
            .unwrap_or_else(|| control.label.unwrap_or(description).to_owned()),
        description,
        keys: keymap.label(control.command),
        enabled: state.enabled,
        checked: state.checked,
        pending: spec.is_some_and(|spec| spec.status == Status::Pending),
    }
}

fn text_width(text: &str, per_char: f32) -> f32 {
    text.chars().count() as f32 * per_char
}

fn small_width(control: &ControlView, metrics: &Metrics) -> f32 {
    metrics.small_base + text_width(&control.label, metrics.char_width)
}

fn items(controls: &[ControlView], metrics: &Metrics) -> Vec<Item> {
    let mut items = Vec::new();
    for control in controls {
        match control.size {
            ControlSize::Large => items.push(Item::Large(control.clone())),
            ControlSize::Tile => items.push(Item::Tile(control.clone())),
            ControlSize::Small => match items.last_mut() {
                Some(Item::Column(column)) if column.len() < metrics.rows => {
                    column.push(control.clone())
                }
                _ => items.push(Item::Column(vec![control.clone()])),
            },
        }
    }
    items
}

/// Expanded: every small control becomes a large button, every tile shows
/// its name.
fn expanded_items(controls: &[ControlView]) -> Vec<Item> {
    controls
        .iter()
        .map(|control| match control.size {
            ControlSize::Tile => Item::LabeledTile(control.clone()),
            ControlSize::Large | ControlSize::Small => Item::Large(control.clone()),
        })
        .collect()
}

impl Item {
    /// The renderer uses the same slot widths as the fold calculation.
    pub fn width(&self, metrics: &Metrics) -> f32 {
        match self {
            Item::Large(control) => metrics
                .large
                .max(text_width(&control.label, metrics.char_width) + 12.),
            Item::Tile(_) => metrics.tile,
            Item::LabeledTile(control) => metrics
                .labeled_tile
                .max(text_width(&control.label, metrics.caption_char_width) + 8.),
            Item::Column(column) => column
                .iter()
                .map(|control| small_width(control, metrics))
                .fold(0., f32::max),
        }
    }
}

fn row_width(items: &[Item], metrics: &Metrics) -> f32 {
    items.iter().map(|item| item.width(metrics)).sum::<f32>()
        + (items.len().saturating_sub(1)) as f32 * metrics.gap
}

fn group_view(
    group: &GroupSpec,
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
    metrics: &Metrics,
) -> GroupView {
    let controls: Vec<_> = group
        .controls
        .iter()
        .map(|control| control_view(control, registry, keymap, states))
        .collect();
    let items = items(&controls, metrics);
    let title = text_width(group.title, metrics.char_width * 0.9);
    let width = row_width(&items, metrics).max(title) + metrics.group_padding;
    let expanded =
        row_width(&expanded_items(&controls), metrics).max(title) + metrics.group_padding;
    GroupView {
        title: group.title,
        visual: group.visual,
        mode: GroupMode::Full,
        width,
        // Expanding never makes a group narrower than its normal size.
        expanded_width: expanded.max(width),
        folded_width: metrics.folded.max(title + metrics.group_padding),
        items,
        controls,
    }
}

/// The widths a group can take, and how important it is to keep it large.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub folded: f32,
    pub full: f32,
    pub expanded: f32,
    pub priority: u8,
}

/// Chooses each group's mode for a row `available` wide. Too narrow: fold
/// the lowest priority first (the rightmost among equals) until it fits; it
/// never scrolls, so if even the folded row is wider every group stays folded.
/// Room to spare: expand the highest priority first (the leftmost among
/// equals), skipping a group that would not fit, so no space a group could
/// use is left empty.
pub fn fit(groups: &[Sizes], available: f32, metrics: &Metrics) -> Vec<GroupMode> {
    let width = |modes: &[GroupMode]| {
        total(
            groups.iter().zip(modes).map(|(sizes, mode)| match mode {
                GroupMode::Folded => sizes.folded,
                GroupMode::Full => sizes.full,
                GroupMode::Expanded => sizes.expanded,
            }),
            metrics,
        )
    };
    let mut modes = vec![GroupMode::Full; groups.len()];
    if width(&modes) > available {
        while width(&modes) > available {
            let candidate = groups
                .iter()
                .enumerate()
                .filter(|(index, sizes)| {
                    modes[*index] == GroupMode::Full && sizes.folded < sizes.full
                })
                .min_by(|(a, pa), (b, pb)| pa.priority.cmp(&pb.priority).then(b.cmp(a)))
                .map(|(index, _)| index);
            match candidate {
                Some(index) => modes[index] = GroupMode::Folded,
                None => break,
            }
        }
        return modes;
    }
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by(|a, b| groups[*b].priority.cmp(&groups[*a].priority).then(a.cmp(b)));
    for index in order {
        if groups[index].expanded <= groups[index].full {
            continue;
        }
        modes[index] = GroupMode::Expanded;
        if width(&modes) > available {
            modes[index] = GroupMode::Full;
        }
    }
    modes
}

/// The ribbon row of `tab` for a window `width` wide.
pub fn project(
    tab: &TabSpec,
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
    width: f32,
    metrics: &Metrics,
) -> ToolbarView {
    let mut groups: Vec<_> = tab
        .groups
        .iter()
        .map(|group| group_view(group, registry, keymap, states, metrics))
        .collect();
    let sizes: Vec<_> = groups
        .iter()
        .zip(tab.groups)
        .map(|(view, spec)| Sizes {
            folded: view.folded_width,
            full: view.width,
            expanded: view.expanded_width,
            priority: spec.priority,
        })
        .collect();
    for (group, mode) in groups.iter_mut().zip(fit(&sizes, width, metrics)) {
        group.mode = mode;
        match mode {
            GroupMode::Folded => group.items.clear(),
            GroupMode::Full => {}
            GroupMode::Expanded => group.items = expanded_items(&group.controls),
        }
    }
    ToolbarView {
        tab: tab.id,
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{TABS, tab};
    use score_keymap::{Profile, UsLayout, UserKeymap, id};

    struct Fake(Vec<(CommandId, CommandState)>);
    impl CommandStates for Fake {
        fn state(&self, command: CommandId) -> CommandState {
            self.0
                .iter()
                .find(|(id, _)| *id == command)
                .map(|(_, state)| state.clone())
                .unwrap_or_default()
        }
    }

    fn keymap() -> Keymap {
        Keymap::resolve(
            &Registry::builtin(),
            Profile::sibelius_laptop(),
            &UserKeymap::new(),
            &UsLayout,
        )
        .0
    }

    fn view(tab_id: TabId, width: f32, states: &Fake) -> ToolbarView {
        project(
            tab(tab_id),
            &Registry::builtin(),
            &keymap(),
            states,
            width,
            &Metrics::default(),
        )
    }

    fn controls(view: &ToolbarView) -> Vec<&ControlView> {
        view.groups
            .iter()
            .flat_map(|group| &group.controls)
            .collect()
    }

    #[test]
    fn labels_keys_state_and_pending_come_from_their_sources() {
        let states = Fake(vec![
            (id::EIGHTH, CommandState::checked(true)),
            (
                id::SLUR,
                CommandState {
                    label: Some("Terminar expresión".into()),
                    ..CommandState::default()
                },
            ),
        ]);
        let input = view(TabId::NoteInput, 2000., &states);
        let find = |view: &ToolbarView, command| {
            controls(view)
                .into_iter()
                .find(|control| control.command == command)
                .cloned()
                .unwrap()
        };
        let eighth = find(&input, id::EIGHTH);
        assert!(eighth.checked);
        assert_eq!(eighth.keys.as_deref(), Some("3"));
        assert_eq!(eighth.label, "Corchea");
        assert!(find(&input, id::THIRTY_SECOND).pending);
        assert!(!find(&input, id::QUARTER).checked);
        // Articulations and slurs sit beside the durations, as in MuseScore.
        let slur = find(&input, id::SLUR);
        assert_eq!(slur.label, "Terminar expresión");
        assert_eq!(slur.keys.as_deref(), Some("S"));
        assert_eq!(find(&input, id::STACCATO).keys, None);
        let home = view(TabId::Home, 2000., &states);
        assert_eq!(find(&home, id::PASTE).label, "Pegar");
    }

    #[test]
    fn small_controls_stack_in_columns_of_three() {
        // At exactly the normal width nothing folds and nothing expands.
        let normal = |tab_id| {
            let width = full_width(&view(tab_id, 5000., &Fake(vec![])));
            view(tab_id, width, &Fake(vec![]))
        };
        let home = normal(TabId::Home);
        let clipboard = &home.groups[0];
        assert!(matches!(clipboard.items[0], Item::Large(_)));
        assert!(matches!(&clipboard.items[1], Item::Column(column) if column.len() == 3));
        let notation = normal(TabId::Notation);
        let dynamics = &notation.groups[1];
        assert_eq!(
            dynamics
                .items
                .iter()
                .filter(|item| matches!(item, Item::Tile(_)))
                .count(),
            6
        );
    }

    fn full_width(view: &ToolbarView) -> f32 {
        total(
            view.groups.iter().map(|group| group.width),
            &Metrics::default(),
        )
    }

    #[test]
    fn every_tab_fits_without_folding_at_the_reference_width() {
        for spec in TABS {
            let row = view(spec.id, 1420., &Fake(vec![]));
            assert!(
                row.groups
                    .iter()
                    .all(|group| group.mode != GroupMode::Folded),
                "{:?} needs {}",
                spec.id,
                full_width(&row)
            );
            assert!(row.width(&Metrics::default()) <= 1420.);
        }
    }

    #[test]
    fn spare_room_is_used_by_expanding_groups() {
        let metrics = Metrics::default();
        let available = 1600.;
        for spec in TABS.iter().filter(|spec| !spec.groups.is_empty()) {
            let row = view(spec.id, available, &Fake(vec![]));
            let used = row.width(&metrics);
            // Maximal: no group left at normal size could still expand.
            for group in row
                .groups
                .iter()
                .filter(|group| group.mode == GroupMode::Full)
            {
                assert!(
                    group.expanded_width <= group.width
                        || used - group.width + group.expanded_width > available,
                    "{:?}: {} could expand",
                    spec.id,
                    group.title
                );
            }
        }
        let input = view(TabId::NoteInput, available, &Fake(vec![]));
        let duration = input
            .groups
            .iter()
            .find(|group| group.title == "Duración")
            .unwrap();
        assert!(
            duration
                .items
                .iter()
                .all(|item| matches!(item, Item::LabeledTile(_)))
                || duration.mode == GroupMode::Full
        );
        assert!(
            input.width(&metrics) > full_width(&input),
            "Entrada uses more than its normal width"
        );
    }

    #[test]
    fn narrow_windows_fold_lowest_priority_first_and_never_exceed() {
        let metrics = Metrics::default();
        for width in [1100., 900., 700., 500.] {
            for spec in TABS.iter().filter(|spec| !spec.groups.is_empty()) {
                let folded = view(spec.id, width, &Fake(vec![]));
                let all_folded = folded
                    .groups
                    .iter()
                    .all(|group| group.mode == GroupMode::Folded);
                assert!(
                    folded.width(&metrics) <= width || all_folded,
                    "{:?} at {width}",
                    spec.id
                );
            }
        }
        let needed = full_width(&view(TabId::Home, 5000., &Fake(vec![])));
        let home = view(TabId::Home, needed - 1., &Fake(vec![]));
        for group in &home.groups {
            let expected = if group.title == "Selección" {
                GroupMode::Folded
            } else {
                GroupMode::Full
            };
            assert_eq!(
                group.mode, expected,
                "{}: selection has the lowest priority",
                group.title
            );
        }
    }

    #[test]
    fn fit_folds_from_the_right_and_expands_from_the_left() {
        let metrics = Metrics::default();
        let sizes = |priority| Sizes {
            folded: 50.,
            full: 100.,
            expanded: 150.,
            priority,
        };
        let groups = [sizes(1), sizes(1), sizes(5)];
        assert_eq!(
            fit(&groups, 260., &metrics),
            vec![GroupMode::Full, GroupMode::Folded, GroupMode::Full]
        );
        assert_eq!(fit(&groups, 10., &metrics), vec![GroupMode::Folded; 3]);
        assert_eq!(
            fit(&groups, 410., &metrics),
            vec![GroupMode::Expanded, GroupMode::Full, GroupMode::Expanded],
            "highest priority first, then the leftmost of equals"
        );
        assert_eq!(fit(&groups, 500., &metrics), vec![GroupMode::Expanded; 3]);
    }
}
