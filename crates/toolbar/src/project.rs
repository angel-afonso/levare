//! Projection of the static ribbon into what is drawn: labels from the
//! registry, keys from the active keymap, state from the app, uniform sizes
//! and how much room each group takes for the available width.

use crate::spec::{Control, ControlSize, GroupSpec, Role, TabId, TabSpec, Visual};
use score_keymap::{CommandId, Keymap, Registry, Status};

/// What the app knows about a command right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandState {
    pub enabled: bool,
    /// Active option, toggled mode or current value (selected duration…).
    pub checked: bool,
    /// Replaces the label while the state lasts («Terminar», «Pausar»). No
    /// wider than the control's ribbon name: the tile keeps the name's width.
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
/// fold decision does not need the toolkit's text system. The per-character
/// widths were measured on the ribbon's own names and keys in the system font
/// («Ambas manos», «⌘⌫» are the widest), so they never reach the tile's edge.
/// Escribir is the tightest tab: fully named it needs 1416.2 of the 1420 a
/// 1440 window leaves, so any wider text there drops a group's names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Narrowest named tile of a `ControlSize::Large` control.
    pub large: f32,
    /// Narrowest tile without its name.
    pub tile: f32,
    /// Narrowest tile with its name.
    pub labeled_tile: f32,
    /// Room on each side of a tile's text, border included.
    pub tile_padding: f32,
    /// Group titles (11px) and folded captions.
    pub char_width: f32,
    /// Names under the glyph (10.5px).
    pub caption_char_width: f32,
    /// Keys under the name (10px); modifier symbols are the widest.
    pub key_char_width: f32,
    /// Glyphs written as text («con sord.», «arm.»).
    pub glyph_char_width: f32,
    pub gap: f32,
    pub group_padding: f32,
    pub separator: f32,
    pub folded: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            large: 66.,
            tile: 36.,
            labeled_tile: 44.,
            tile_padding: 5.,
            char_width: 6.3,
            caption_char_width: 6.5,
            key_char_width: 9.,
            glyph_char_width: 8.,
            gap: 2.,
            group_padding: 0.,
            separator: 20.,
            folded: 64.,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ControlView {
    pub command: CommandId,
    pub size: ControlSize,
    pub role: Role,
    pub visual: Visual,
    pub symbol_only: bool,
    /// The ribbon's name for the control; a named tile is measured on it.
    pub name: &'static str,
    /// What the tile says now: `name`, or a state label that replaces it.
    pub label: String,
    /// Full registry label, for tooltips and accessibility.
    pub description: &'static str,
    /// What the control does, in one sentence, for its tooltip.
    pub hint: &'static str,
    pub keys: Option<String>,
    pub enabled: bool,
    pub checked: bool,
    pub pending: bool,
}

/// One slot of a group, left to right. Every control is a tile: its glyph
/// and, under it, its key; a `ControlSize::Large` one keeps its name too.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Tile(ControlView),
    /// A tile with its name over the key, when there is room for it.
    LabeledTile(ControlView),
    /// Consecutive choice tiles, drawn on one shared track. Named when expanded.
    Segment {
        controls: Vec<ControlView>,
        named: bool,
    },
}

/// How much room a group takes, like the Office ribbon's scaling: folded
/// into one button, normal (glyph and key), or expanded with named tiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupMode {
    Folded,
    Full,
    Expanded,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupView {
    pub title: &'static str,
    /// Drawn as a split button: the main part repeats the last option used.
    pub dropdown: bool,
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
    let name = control.label.unwrap_or(description);
    ControlView {
        command: control.command,
        size: control.size,
        role: control.role,
        visual: control.visual,
        symbol_only: control.symbol_only,
        name,
        label: state.label.unwrap_or_else(|| name.to_owned()),
        description,
        hint: control.hint,
        keys: keymap.label(control.command),
        enabled: state.enabled,
        checked: state.checked,
        pending: spec.is_some_and(|spec| spec.status == Status::Pending),
    }
}

fn text_width(text: &str, per_char: f32) -> f32 {
    text.chars().count() as f32 * per_char
}

fn items(controls: &[ControlView], expanded: bool) -> Vec<Item> {
    let mut items = Vec::new();
    for control in controls {
        if control.size == ControlSize::Tile && control.role == Role::Choice {
            let named = expanded && !control.symbol_only;
            match items.last_mut() {
                Some(Item::Segment { controls, .. }) => controls.push(control.clone()),
                _ => items.push(Item::Segment {
                    controls: vec![control.clone()],
                    named,
                }),
            }
            continue;
        }
        // A group's main control keeps its name at every size: unnamed, the
        // note-input toggle would look like Corchea two tiles away.
        let named = expanded || control.size == ControlSize::Large;
        items.push(if named && !control.symbol_only {
            Item::LabeledTile(control.clone())
        } else {
            Item::Tile(control.clone())
        });
    }
    items
}

/// Width of one tile: its glyph and, under it, the name when `named` and the
/// key on a line of its own. A text glyph can be wider than both. A state
/// label is measured with the name, so it never moves its neighbours.
pub fn tile_width(control: &ControlView, named: bool, metrics: &Metrics) -> f32 {
    let key = control
        .keys
        .as_deref()
        .map_or(0., |keys| text_width(keys, metrics.key_char_width));
    let (narrowest, text) = if named {
        let narrowest = if control.size == ControlSize::Large {
            metrics.large
        } else {
            metrics.labeled_tile
        };
        let name = text_width(control.name, metrics.caption_char_width)
            .max(text_width(&control.label, metrics.caption_char_width));
        (narrowest, name.max(key))
    } else {
        (metrics.tile, key)
    };
    let glyph = match control.visual {
        Visual::Text(text) | Visual::Label(text) => text_width(text, metrics.glyph_char_width),
        _ => 0.,
    };
    narrowest.max(text.max(glyph) + 2. * metrics.tile_padding)
}

impl Item {
    /// The renderer uses the same slot widths as the fold calculation.
    pub fn width(&self, metrics: &Metrics) -> f32 {
        match self {
            Item::Tile(control) => tile_width(control, false, metrics),
            Item::LabeledTile(control) => tile_width(control, true, metrics),
            Item::Segment { controls, named } => {
                controls
                    .iter()
                    .map(|control| tile_width(control, *named && !control.symbol_only, metrics))
                    .sum::<f32>()
                    + controls.len().saturating_sub(1) as f32 * 2.
                    + 4.
            }
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
    let full_items = items(&controls, false);
    let title = text_width(group.title, metrics.char_width);
    let width = row_width(&full_items, metrics).max(title) + metrics.group_padding;
    let expanded = row_width(&items(&controls, true), metrics).max(title) + metrics.group_padding;
    let folded_width = metrics
        .folded
        .max(text_width(group.title, metrics.char_width) + metrics.group_padding + 14.);
    let mut checked = controls.iter().filter(|control| control.checked);
    let visual = match (checked.next(), checked.next()) {
        (Some(control), None) => control.visual,
        _ => group.visual,
    };
    GroupView {
        title: group.title,
        dropdown: group.dropdown,
        visual,
        mode: if group.dropdown {
            GroupMode::Folded
        } else {
            GroupMode::Full
        },
        width: if group.dropdown { folded_width } else { width },
        // Expanding never makes a group narrower than its normal size.
        expanded_width: if group.dropdown {
            folded_width
        } else {
            expanded.max(width)
        },
        folded_width,
        items: if group.dropdown {
            Vec::new()
        } else {
            full_items
        },
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
/// never scrolls, so if the row still cannot fit every group uses its narrower
/// mode (full groups can be narrower than their folded captions).
/// Room to spare: expand the highest priority first (the leftmost among
/// equals), skipping a group that would not fit, so no space a group could
/// use is left empty. A group as wide expanded as normal (its title is wider
/// than its named tiles) shows its names for free.
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
        if groups[index].expanded < groups[index].full {
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
    for ((group, spec), mode) in groups
        .iter_mut()
        .zip(tab.groups)
        .zip(fit(&sizes, width, metrics))
    {
        let mode = if spec.dropdown {
            GroupMode::Folded
        } else {
            mode
        };
        group.mode = mode;
        match mode {
            GroupMode::Folded => group.items.clear(),
            GroupMode::Full => {}
            GroupMode::Expanded => group.items = items(&group.controls, true),
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
    use score_keymap::{Platform, Profile, UsLayout, UserKeymap, id};

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

    fn keymap(platform: Platform) -> Keymap {
        Keymap::resolve_for(
            &Registry::builtin(),
            Profile::sibelius_laptop_for(platform),
            &UserKeymap::new(),
            &UsLayout,
            platform,
        )
        .0
    }

    fn view(tab_id: TabId, width: f32, states: &Fake) -> ToolbarView {
        view_on(Platform::MacOs, tab_id, width, states)
    }

    /// Windows and Linux spell keys out (`Ctrl+Mayús+B`), so their tiles are
    /// wider.
    fn view_on(platform: Platform, tab_id: TabId, width: f32, states: &Fake) -> ToolbarView {
        project(
            tab(tab_id),
            &Registry::builtin(),
            &keymap(platform),
            states,
            width,
            &Metrics::default(),
        )
    }

    #[test]
    fn only_designed_dropdowns_are_split() {
        let states = Fake(Vec::new());
        let mut split = Vec::new();
        for tab in TABS {
            for group in view(tab.id, 5000., &states).groups {
                if group.dropdown {
                    split.push(group.title);
                }
            }
            for group in view(tab.id, 500., &states).groups {
                if group.mode == GroupMode::Folded && !split.contains(&group.title) {
                    assert!(!group.dropdown, "{}", group.title);
                }
            }
        }
        split.sort_unstable();
        assert_eq!(split, vec!["D.C. / D.S.", "Intervalo"]);
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
                    label: Some("Terminar".into()),
                    ..CommandState::default()
                },
            ),
        ]);
        let input = view(TabId::Write, 2000., &states);
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
        assert!(!find(&input, id::THIRTY_SECOND).pending);
        assert!(!find(&input, id::QUARTER).checked);
        let slur = find(&input, id::SLUR);
        assert_eq!(slur.label, "Terminar");
        assert_eq!(slur.name, "Expresión");
        assert_eq!(slur.keys.as_deref(), Some("S"));
        let articulations = view(TabId::Articulations, 2000., &states);
        assert_eq!(find(&articulations, id::STACCATO).keys, None);
        let score = view(TabId::Score, 2000., &states);
        assert_eq!(find(&score, id::ADD_INSTRUMENT).label, "Añadir…");
    }

    #[test]
    fn a_state_label_never_reflows_the_row() {
        // Pressing S mid-gesture must not move Ligadura or its neighbours.
        let metrics = Metrics::default();
        let row = |states: &Fake| {
            view(TabId::Write, 1420., states)
                .groups
                .iter()
                .map(|group| (group.mode, group.width_in(group.mode)))
                .collect::<Vec<_>>()
        };
        let open = Fake(vec![(
            id::SLUR,
            CommandState {
                checked: true,
                label: Some("Terminar".into()),
                ..CommandState::default()
            },
        )]);
        assert_eq!(row(&open), row(&Fake(vec![])));
        let slur = controls(&view(TabId::Write, 1420., &open))
            .into_iter()
            .find(|control| control.command == id::SLUR)
            .cloned()
            .unwrap();
        assert_eq!(slur.label, "Terminar");
        assert_eq!(
            tile_width(&slur, true, &metrics),
            text_width(slur.name, metrics.caption_char_width) + 2. * metrics.tile_padding
        );
    }

    #[test]
    fn choice_tiles_form_one_segment() {
        let width = full_width(&view(TabId::Write, 5000., &Fake(vec![])));
        let input = view(TabId::Write, width, &Fake(vec![]));
        let group = |title| input.groups.iter().find(|g| g.title == title).unwrap();
        let duration = group("Duración");
        let Item::Segment { controls, .. } = &duration.items[0] else {
            panic!("durations open with a segment");
        };
        let commands: Vec<_> = controls.iter().map(|c| c.command).collect();
        assert_eq!(
            commands,
            [
                id::WHOLE,
                id::HALF,
                id::QUARTER,
                id::EIGHTH,
                id::SIXTEENTH,
                id::THIRTY_SECOND
            ]
        );
        assert_eq!(duration.items.len(), 1);
        let modify = group("Modificar");
        assert!(
            modify
                .items
                .iter()
                .all(|item| matches!(item, Item::Tile(_)))
        );
        let voices = group("Voz");
        assert!(
            matches!(voices.items.as_slice(), [Item::Segment { controls, .. }] if controls.len() == 2)
        );
    }

    #[test]
    fn expanded_segments_are_named() {
        let input = view(TabId::Write, 5000., &Fake(vec![]));
        let duration = input.groups.iter().find(|g| g.title == "Duración").unwrap();
        assert_eq!(duration.mode, GroupMode::Expanded);
        assert!(matches!(
            &duration.items[0],
            Item::Segment { named: true, .. }
        ));
    }

    #[test]
    fn intervals_and_jumps_stay_selectors_at_every_window_width() {
        let states = Fake(vec![
            (id::INTERVAL_3, CommandState::checked(true)),
            (id::DAL_SEGNO, CommandState::checked(true)),
        ]);
        for width in [500., 900., 1420., 5000.] {
            for (tab_id, title, commands, selected) in [
                (
                    TabId::Write,
                    "Intervalo",
                    &[
                        id::INTERVAL_2,
                        id::INTERVAL_3,
                        id::INTERVAL_4,
                        id::INTERVAL_5,
                        id::INTERVAL_6,
                        id::INTERVAL_7,
                        id::INTERVAL_8,
                    ][..],
                    id::INTERVAL_3,
                ),
                (
                    TabId::Score,
                    "D.C. / D.S.",
                    &[
                        id::DA_CAPO,
                        id::DA_CAPO_AL_FINE,
                        id::DA_CAPO_AL_CODA,
                        id::DAL_SEGNO,
                        id::DAL_SEGNO_AL_FINE,
                        id::DAL_SEGNO_AL_CODA,
                        id::TO_CODA,
                    ][..],
                    id::DAL_SEGNO,
                ),
            ] {
                let row = view(tab_id, width, &states);
                let group = row
                    .groups
                    .iter()
                    .find(|group| group.title == title)
                    .unwrap();
                assert_eq!(group.mode, GroupMode::Folded, "{title} at {width}");
                assert!(group.items.is_empty());
                assert_eq!(
                    group
                        .controls
                        .iter()
                        .map(|control| control.command)
                        .collect::<Vec<_>>(),
                    commands
                );
                assert_eq!(
                    group
                        .controls
                        .iter()
                        .find(|control| control.checked)
                        .unwrap()
                        .command,
                    selected
                );
                assert_eq!(
                    group.visual,
                    group
                        .controls
                        .iter()
                        .find(|control| control.command == selected)
                        .unwrap()
                        .visual
                );
                assert!(group.width_in(group.mode) < 100.);
            }
        }
    }

    fn full_width(view: &ToolbarView) -> f32 {
        total(
            view.groups.iter().map(|group| group.width),
            &Metrics::default(),
        )
    }

    #[test]
    fn reference_width_keeps_priority_controls_and_all_folded_commands_available() {
        for spec in TABS {
            let row = view(spec.id, 1420., &Fake(vec![]));
            // Tabs by task are short enough that only designed dropdowns fold,
            // and a folded group still lists every command of its spec.
            for (group, source) in row.groups.iter().zip(spec.groups) {
                assert_eq!(
                    group.mode == GroupMode::Folded,
                    source.dropdown,
                    "{:?} needs {}: {}",
                    spec.id,
                    full_width(&row),
                    source.title
                );
                assert_eq!(
                    group
                        .controls
                        .iter()
                        .map(|control| control.command)
                        .collect::<Vec<_>>(),
                    source
                        .controls
                        .iter()
                        .map(|control| control.command)
                        .collect::<Vec<_>>()
                );
            }
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
        let input = view(TabId::Write, available, &Fake(vec![]));
        let duration = input
            .groups
            .iter()
            .find(|group| group.title == "Duración")
            .unwrap();
        assert!(
            duration
                .items
                .iter()
                .all(|item| matches!(item, Item::LabeledTile(_) | Item::Segment { .. }))
                || duration.mode == GroupMode::Full
        );
        assert!(
            input.width(&metrics) > full_width(&input),
            "Escribir uses more than its normal width"
        );
    }

    #[test]
    fn narrow_windows_fold_lowest_priority_first_and_fit_when_possible() {
        let metrics = Metrics::default();
        for width in [1100., 900., 700., 500.] {
            for spec in TABS.iter().filter(|spec| !spec.groups.is_empty()) {
                let folded = view(spec.id, width, &Fake(vec![]));
                // A compact group can be narrower than its folded caption.
                // Once every width-reducing fold is used, this is the smallest
                // possible row even when the window cannot contain it.
                let minimum = total(
                    folded
                        .groups
                        .iter()
                        .map(|group| group.width.min(group.folded_width)),
                    &metrics,
                );
                assert!(
                    folded.width(&metrics) <= width
                        || (minimum > width && (folded.width(&metrics) - minimum).abs() < 0.01),
                    "{:?} at {width}",
                    spec.id
                );
            }
        }
        let needed = full_width(&view(TabId::Review, 5000., &Fake(vec![])));
        let review = view(TabId::Review, needed - 1., &Fake(vec![]));
        for group in &review.groups {
            let expected = if group.title == "Comentarios" {
                GroupMode::Folded
            } else {
                GroupMode::Full
            };
            assert_eq!(
                group.mode, expected,
                "{}: the comments have the lowest priority",
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
        let separators = 2. * metrics.separator;
        assert_eq!(
            fit(&groups, 250. + separators, &metrics),
            vec![GroupMode::Full, GroupMode::Folded, GroupMode::Full]
        );
        assert_eq!(fit(&groups, 10., &metrics), vec![GroupMode::Folded; 3]);
        assert_eq!(
            fit(&groups, 425. + separators, &metrics),
            vec![GroupMode::Expanded, GroupMode::Full, GroupMode::Expanded],
            "highest priority first, then the leftmost of equals"
        );
        assert_eq!(
            fit(&groups, 450. + separators, &metrics),
            vec![GroupMode::Expanded; 3]
        );
        let titled = Sizes {
            expanded: 100.,
            ..sizes(1)
        };
        assert_eq!(fit(&[titled], 100., &metrics), vec![GroupMode::Expanded]);
    }

    /// The widths are measured on macOS's system font and ⌘-style keys.
    /// Windows and Linux spell keys out (`Ctrl+Mayús+B`), so there Escribir
    /// and Partitura drop one group's names at 1440; their fonts still need
    /// measuring.
    #[test]
    fn at_1440_every_tab_shows_names_and_keys() {
        let metrics = Metrics::default();
        let mut unnamed = Vec::new();
        for spec in TABS {
            let row = view_on(Platform::MacOs, spec.id, 1420., &Fake(vec![]));
            let titles: Vec<_> = row
                .groups
                .iter()
                .filter(|group| !group.dropdown && group.mode != GroupMode::Expanded)
                .map(|group| group.title)
                .collect();
            if !titles.is_empty() {
                unnamed.push(format!(
                    "{:?} needs {:.1}: {}",
                    spec.id,
                    expanded_width(&row),
                    titles.join(", ")
                ));
            }
            assert!(row.width(&metrics) <= 1420., "{:?}", spec.id);
        }
        assert!(unnamed.is_empty(), "{unnamed:#?}");
    }

    fn expanded_width(view: &ToolbarView) -> f32 {
        total(
            view.groups.iter().map(|group| group.expanded_width),
            &Metrics::default(),
        )
    }

    #[test]
    fn writing_keeps_durations_entry_and_accidentals_longest() {
        let last = ["Duración", "Entrada", "Alteración"];
        let middle = ["Voz", "Modificar"];
        let first = ["Ligadura", "Acorde", "Intervalo"];
        let priority = |title: &str| {
            tab(TabId::Write)
                .groups
                .iter()
                .find(|group| group.title == title)
                .unwrap()
                .priority
        };
        for (low, high) in [(&first[..], &middle[..]), (&middle[..], &last[..])] {
            for low in low {
                for high in high {
                    assert!(priority(low) < priority(high), "{low} before {high}");
                }
            }
        }
        let row = view(TabId::Write, 1000., &Fake(vec![]));
        for group in row
            .groups
            .iter()
            .filter(|group| last.contains(&group.title))
        {
            assert_ne!(group.mode, GroupMode::Folded, "{}", group.title);
        }
    }

    #[test]
    fn named_tiles_put_the_key_under_the_name() {
        let metrics = Metrics::default();
        let row = view(TabId::Playback, 5000., &Fake(vec![]));
        let practice = row
            .groups
            .iter()
            .find(|group| group.title == "Práctica")
            .unwrap();
        let half_speed = practice
            .controls
            .iter()
            .find(|control| control.command == id::HALF_SPEED)
            .unwrap();
        // «Media velocidad» over «⇧Espacio»: the name is the wider line.
        assert_eq!(
            tile_width(half_speed, true, &metrics),
            text_width(&half_speed.label, metrics.caption_char_width) + 2. * metrics.tile_padding
        );
    }

    #[test]
    fn narrow_windows_drop_names_before_folding() {
        let metrics = Metrics::default();
        for (platform, spec) in Platform::ALL
            .into_iter()
            .flat_map(|platform| TABS.iter().map(move |spec| (platform, spec)))
        {
            let row = view_on(platform, spec.id, 1000., &Fake(vec![]));
            let mut groups = row.groups.iter().filter(|group| !group.dropdown);
            if groups.clone().any(|group| group.mode == GroupMode::Folded) {
                assert!(
                    groups.all(|group| group.mode != GroupMode::Expanded),
                    "{:?}",
                    spec.id
                );
            }
            assert!(row.width(&metrics) <= 1000., "{:?}", spec.id);
        }
    }

    #[test]
    fn unnamed_tiles_keep_their_key() {
        let metrics = Metrics::default();
        let mut seen = 0;
        for (platform, spec) in Platform::ALL
            .into_iter()
            .flat_map(|platform| TABS.iter().map(move |spec| (platform, spec)))
        {
            let row = view_on(platform, spec.id, 0., &Fake(vec![]));
            let row = view_on(platform, spec.id, full_width(&row), &Fake(vec![]));
            for item in row.groups.iter().flat_map(|group| &group.items) {
                if let Item::Tile(control) = item
                    && let Some(keys) = &control.keys
                {
                    seen += 1;
                    assert!(
                        item.width(&metrics)
                            >= text_width(keys, metrics.key_char_width) + 2. * metrics.tile_padding,
                        "{}: {keys}",
                        control.label
                    );
                }
            }
        }
        assert!(seen > 0);
    }
}
