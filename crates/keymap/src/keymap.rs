//! Resolution of profile + user changes into the active bindings.

use crate::{
    CommandId, KeySequence, Layout, Platform, Profile, Registry, Scope, UserKeymap,
    commands::CommandSpec,
};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Profile,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub keys: KeySequence,
    pub command: CommandId,
    pub scope: Scope,
    pub origin: Origin,
    /// What the toolkit must match on this keyboard layout. `None` when the
    /// key cannot be typed here (numeric keypad on GPUI 0.2.2).
    pub platform: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    UnknownCommand(String),
    InvalidKeys {
        command: CommandId,
        keys: String,
    },
    Unsupported {
        command: CommandId,
        keys: KeySequence,
    },
    /// Ctrl+Alt with a character off macOS: see `Keystroke::is_altgr_for`.
    AltGr {
        command: CommandId,
        keys: KeySequence,
    },
    Conflict {
        keys: KeySequence,
        kept: CommandId,
        dropped: CommandId,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand(id) => write!(f, "comando desconocido «{id}»"),
            Self::InvalidKeys { command, keys } => {
                write!(f, "«{keys}» no es una combinación válida para «{command}»")
            }
            Self::Unsupported { command, keys } => write!(
                f,
                "{} («{command}») no se puede detectar con este teclado",
                keys.label()
            ),
            Self::AltGr { command, keys } => write!(
                f,
                "{} («{command}») se ignoró: Ctrl+Alt es AltGr en Windows y un atajo del \
                 escritorio en Linux; prueba con {}",
                keys.label(),
                keys.without_altgr().label()
            ),
            Self::Conflict {
                keys,
                kept,
                dropped,
            } => write!(
                f,
                "{} ya pertenece a «{kept}»; se ignoró para «{dropped}»",
                keys.label()
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Keymap {
    profile: Profile,
    bindings: Vec<Binding>,
    platform: Platform,
}

impl Keymap {
    pub fn resolve(
        registry: &Registry,
        profile: Profile,
        user: &UserKeymap,
        layout: &dyn Layout,
    ) -> (Self, Vec<Problem>) {
        Self::resolve_for(registry, profile, user, layout, Platform::current())
    }

    /// Resolves for `platform`: its GPUI key text, its conflicts and its
    /// labels. `profile` should be that platform's.
    pub fn resolve_for(
        registry: &Registry,
        profile: Profile,
        user: &UserKeymap,
        layout: &dyn Layout,
        platform: Platform,
    ) -> (Self, Vec<Problem>) {
        let mut problems: Vec<Problem> = user
            .overrides
            .keys()
            .filter(|id| registry.get(id).is_none())
            .map(|id| Problem::UnknownCommand(id.clone()))
            .collect();
        let mut candidates = Vec::new();
        for spec in registry.iter() {
            match user.overrides.get(spec.id.as_str()) {
                Some(keys) => {
                    for text in keys {
                        match KeySequence::parse(text) {
                            Ok(keys) => candidates.push((keys, *spec, Origin::User)),
                            Err(_) => problems.push(Problem::InvalidKeys {
                                command: spec.id,
                                keys: text.clone(),
                            }),
                        }
                    }
                }
                None => candidates.extend(
                    profile
                        .keys_for(spec.id)
                        .into_iter()
                        .map(|keys| (keys, *spec, Origin::Profile)),
                ),
            }
        }
        // The user's own choices win; within one origin the earlier entry wins.
        let mut order: Vec<usize> = (0..candidates.len()).collect();
        order.sort_by_key(|index| candidates[*index].2 == Origin::Profile);
        let mut accepted = vec![None; candidates.len()];
        for index in order {
            let (keys, spec, origin) = &candidates[index];
            if keys.is_altgr_for(platform) {
                problems.push(Problem::AltGr {
                    command: spec.id,
                    keys: keys.clone(),
                });
                continue;
            }
            let typed = keys.to_platform_for(layout, platform);
            if typed.is_none() {
                problems.push(Problem::Unsupported {
                    command: spec.id,
                    keys: keys.clone(),
                });
            }
            let clash = typed.as_ref().and_then(|typed| {
                accepted.iter().flatten().find(|binding: &&Binding| {
                    binding.platform.as_ref() == Some(typed) && binding.scope.overlaps(spec.scope)
                })
            });
            if let Some(existing) = clash {
                problems.push(Problem::Conflict {
                    keys: keys.clone(),
                    kept: existing.command,
                    dropped: spec.id,
                });
                continue;
            }
            accepted[index] = Some(Binding {
                keys: keys.clone(),
                command: spec.id,
                scope: spec.scope,
                origin: *origin,
                platform: typed,
            });
        }
        (
            Self {
                profile,
                bindings: accepted.into_iter().flatten().collect(),
                platform,
            },
            problems,
        )
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    pub fn keys_for(&self, command: CommandId) -> Vec<&KeySequence> {
        self.bindings
            .iter()
            .filter(|binding| binding.command == command)
            .map(|binding| &binding.keys)
            .collect()
    }

    /// The label buttons and menus show: the first binding of the command.
    pub fn label(&self, command: CommandId) -> Option<String> {
        self.keys_for(command)
            .first()
            .map(|keys| keys.label_for(self.platform))
    }

    /// Which other command already answers to `keys` where `spec` would work.
    pub fn conflict(
        &self,
        spec: &CommandSpec,
        keys: &KeySequence,
        layout: &dyn Layout,
    ) -> Option<CommandId> {
        let typed = keys.to_platform_for(layout, self.platform)?;
        self.bindings
            .iter()
            .find(|binding| {
                binding.command != spec.id
                    && binding.platform.as_deref() == Some(typed.as_str())
                    && binding.scope.overlaps(spec.scope)
            })
            .map(|binding| binding.command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UsLayout, commands::id};
    use Platform::{Linux, MacOs, Windows};

    struct Spanish;
    impl Layout for Spanish {
        fn shifted_digit(&self, digit: u8) -> Option<String> {
            "=!\"·$%&/()"
                .chars()
                .nth(usize::from(digit))
                .map(String::from)
        }
    }

    fn resolve_on(platform: Platform, user: &UserKeymap) -> (Keymap, Vec<Problem>) {
        Keymap::resolve_for(
            &Registry::builtin(),
            Profile::sibelius_laptop_for(platform),
            user,
            &UsLayout,
            platform,
        )
    }

    fn resolve(user: &UserKeymap) -> (Keymap, Vec<Problem>) {
        resolve_on(MacOs, user)
    }

    /// What GPUI matches for `command`'s first binding.
    fn typed(keymap: &Keymap, command: CommandId) -> Option<&str> {
        keymap
            .bindings()
            .iter()
            .find(|binding| binding.command == command)
            .and_then(|binding| binding.platform.as_deref())
    }

    #[test]
    fn the_sibelius_profile_is_complete_and_free_of_conflicts() {
        let registry = Registry::builtin();
        for platform in Platform::ALL {
            for (keys, command) in &Profile::sibelius_laptop_for(platform).bindings {
                assert!(
                    registry.get(command.as_str()).is_some(),
                    "{keys} → {command}"
                );
            }
            let layouts: [&dyn Layout; 2] = [&UsLayout, &Spanish];
            for layout in layouts {
                let (_, problems) = Keymap::resolve_for(
                    &registry,
                    Profile::sibelius_laptop_for(platform),
                    &UserKeymap::new(),
                    layout,
                    platform,
                );
                assert_eq!(problems, Vec::new(), "{platform:?}");
            }
        }
        let (keymap, _) = resolve(&UserKeymap::new());
        assert_eq!(keymap.label(id::QUARTER).as_deref(), Some("4"));
        assert_eq!(keymap.label(id::INTERVAL_3).as_deref(), Some("⇧3"));
        assert_eq!(keymap.label(id::REDO).as_deref(), Some("⌘Y"));
        assert_eq!(keymap.keys_for(id::REDO).len(), 2);
        assert_eq!(keymap.label(id::TIE).as_deref(), Some("↩"));
        assert!(keymap.label(id::STACCATO).is_none());
        let escape: Vec<_> = keymap
            .bindings()
            .iter()
            .filter(|binding| binding.keys.text() == "escape")
            .map(|binding| binding.command)
            .collect();
        assert_eq!(escape, vec![id::CLOSE_PANEL, id::ESCAPE]);
    }

    #[test]
    fn macos_keeps_its_command_key_shortcuts() {
        let (keymap, _) = resolve(&UserKeymap::new());
        for (command, text, label) in [
            (id::SAVE, "cmd-s", "⌘S"),
            (id::REDO, "cmd-y", "⌘Y"),
            (id::RESET_POSITION, "shift-cmd-p", "⇧⌘P"),
            (id::TAB_NOTE_INPUT, "alt-cmd-1", "⌥⌘1"),
            (id::TEMPO_TEXT, "alt-cmd-t", "⌥⌘T"),
            (id::GO_TO_BAR, "alt-cmd-g", "⌥⌘G"),
            (id::DELETE_BAR, "cmd-backspace", "⌘⌫"),
            (id::ZOOM_OUT, "cmd--", "⌘-"),
            (id::REPLAY, "alt-space", "⌥Espacio"),
            (id::FULL_SCREEN, "cmd-u", "⌘U"),
        ] {
            assert_eq!(typed(&keymap, command), Some(text), "{command}");
            assert_eq!(keymap.label(command).as_deref(), Some(label), "{command}");
        }
    }

    #[test]
    fn windows_and_linux_use_ctrl_and_their_own_conventions() {
        for platform in [Windows, Linux] {
            let (keymap, _) = resolve_on(platform, &UserKeymap::new());
            for (command, text, label) in [
                (id::SAVE, "ctrl-s", "Ctrl+S"),
                (id::REDO, "ctrl-y", "Ctrl+Y"),
                (id::RESET_POSITION, "ctrl-shift-p", "Ctrl+Mayús+P"),
                (id::TAB_NOTE_INPUT, "ctrl-!", "Ctrl+Mayús+1"),
                (id::TEMPO_TEXT, "ctrl-shift-t", "Ctrl+Mayús+T"),
                (id::GO_TO_BAR, "ctrl-g", "Ctrl+G"),
                (id::DELETE_BAR, "ctrl-backspace", "Ctrl+Retroceso"),
                (id::ZOOM_OUT, "ctrl--", "Ctrl+-"),
                (id::OCTAVE_UP, "ctrl-up", "Ctrl+↑"),
                (id::FULL_SCREEN, "f11", "F11"),
            ] {
                assert_eq!(
                    typed(&keymap, command),
                    Some(text),
                    "{platform:?} {command}"
                );
                assert_eq!(keymap.label(command).as_deref(), Some(label), "{command}");
            }
        }
        let (windows, _) = resolve_on(Windows, &UserKeymap::new());
        assert_eq!(windows.label(id::REPLAY).as_deref(), Some("Alt+Espacio"));
        let (linux, _) = resolve_on(Linux, &UserKeymap::new());
        assert_eq!(
            linux.label(id::REPLAY).as_deref(),
            Some("Mayús+Alt+Espacio")
        );
    }

    #[test]
    fn no_default_shortcut_needs_a_key_the_system_keeps() {
        // What the system takes before the app sees it, or what AltGr types.
        let reserved: &[(Platform, &[&str])] = &[
            (
                MacOs,
                &[
                    "cmd-tab",
                    "cmd-space",
                    "cmd-h",
                    "cmd-m",
                    "cmd-`",
                    "alt-cmd-escape",
                ],
            ),
            (
                Windows,
                &[
                    "alt-tab",
                    "alt-f4",
                    "alt-escape",
                    "ctrl-escape",
                    "ctrl-shift-escape",
                ],
            ),
            (
                Linux,
                &[
                    "alt-tab",
                    "alt-f2",
                    "alt-f4",
                    "alt-f7",
                    "alt-f8",
                    "alt-space",
                    "alt-`",
                ],
            ),
        ];
        for (platform, keys) in reserved {
            let (keymap, _) = resolve_on(*platform, &UserKeymap::new());
            for binding in keymap.bindings() {
                let text = binding.platform.as_deref().unwrap();
                assert!(!keys.contains(&text), "{platform:?}: {text}");
                if *platform != MacOs {
                    assert!(
                        !text.contains("cmd-"),
                        "{platform:?}: {text} is the Windows key"
                    );
                    assert!(
                        !(text.contains("ctrl-") && text.contains("alt-")),
                        "{platform:?}: {text} is AltGr or a desktop shortcut"
                    );
                }
            }
        }
    }

    #[test]
    fn the_shortcut_each_command_shows_needs_no_altgr() {
        // Characters Spanish, German or French keyboards only type with AltGr,
        // which Windows and Linux report as Ctrl+Alt.
        const ALTGR: &[char] = &['[', ']', '{', '}', '\\', '|', '@', '#', '~', '`'];
        for platform in Platform::ALL {
            let (keymap, _) = resolve_on(platform, &UserKeymap::new());
            for binding in keymap.bindings() {
                let shown = keymap.keys_for(binding.command)[0];
                assert!(
                    !shown.0.iter().any(|stroke| stroke.key.contains(ALTGR)),
                    "{platform:?}: {} shows {shown}",
                    binding.command
                );
            }
        }
        let (keymap, _) = resolve_on(Windows, &UserKeymap::new());
        assert_eq!(
            keymap.label(id::PLAYLINE_TO_START).as_deref(),
            Some("Ctrl+Inicio")
        );
        assert_eq!(typed(&keymap, id::PLAYLINE_TO_START), Some("ctrl-home"));
        assert_eq!(keymap.keys_for(id::PLAYLINE_TO_START).len(), 2);
    }

    #[test]
    fn tab_shortcuts_do_not_steal_voice_keys() {
        let (keymap, problems) = resolve(&UserKeymap::new());
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(keymap.label(id::VOICE_1).as_deref(), Some("⌥1"));
        assert_eq!(keymap.label(id::TAB_NOTE_INPUT).as_deref(), Some("⌥⌘1"));
    }

    #[test]
    fn user_changes_replace_or_remove_profile_keys() {
        let mut user = UserKeymap::new();
        user.set(id::STACCATO, &[KeySequence::parse("alt-s").unwrap()]);
        user.set(id::SLUR, &[]);
        let (keymap, problems) = resolve(&user);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(keymap.label(id::STACCATO).as_deref(), Some("⌥S"));
        assert!(keymap.label(id::SLUR).is_none());
        let origin = keymap
            .bindings()
            .iter()
            .find(|binding| binding.command == id::STACCATO)
            .unwrap()
            .origin;
        assert_eq!(origin, Origin::User);
    }

    #[test]
    fn user_keys_win_conflicts_and_problems_are_reported() {
        let mut user = UserKeymap::new();
        user.set(id::STACCATO, &[KeySequence::parse("secondary-b").unwrap()]);
        user.overrides
            .insert("no.existe".into(), vec!["secondary-j".into()]);
        user.overrides
            .insert(id::TENUTO.as_str().into(), vec!["hyper-x".into()]);
        let (keymap, problems) = resolve(&user);
        assert_eq!(keymap.label(id::STACCATO).as_deref(), Some("⌘B"));
        assert!(keymap.label(id::BAR_AT_END).is_none());
        assert!(problems.contains(&Problem::Conflict {
            keys: KeySequence::parse("secondary-b").unwrap(),
            kept: id::STACCATO,
            dropped: id::BAR_AT_END,
        }));
        assert!(problems.contains(&Problem::UnknownCommand("no.existe".into())));
        assert!(problems.contains(&Problem::InvalidKeys {
            command: id::TENUTO,
            keys: "hyper-x".into(),
        }));
    }

    #[test]
    fn a_file_saved_on_macos_with_cmd_keeps_working() {
        let mut user = UserKeymap::new();
        user.overrides
            .insert(id::STACCATO.as_str().into(), vec!["shift-cmd-k".into()]);
        for platform in Platform::ALL {
            let (keymap, problems) = resolve_on(platform, &user);
            assert!(problems.is_empty(), "{problems:?}");
            assert_eq!(
                keymap.label(id::STACCATO),
                Some(
                    KeySequence::parse("shift-secondary-k")
                        .unwrap()
                        .label_for(platform)
                )
            );
        }
        let (keymap, _) = resolve(&user);
        assert_eq!(typed(&keymap, id::STACCATO), Some("shift-cmd-k"));
    }

    #[test]
    fn a_command_key_and_option_shortcut_from_macos_is_skipped_elsewhere() {
        // ⌥⌘K saved on macOS is Ctrl+Alt+K elsewhere: AltGr on Windows.
        let mut user = UserKeymap::new();
        user.set(
            id::STACCATO,
            &[
                KeySequence::parse("alt-secondary-k").unwrap(),
                KeySequence::parse("alt-s").unwrap(),
            ],
        );
        let (keymap, problems) = resolve(&user);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(keymap.label(id::STACCATO).as_deref(), Some("⌥⌘K"));
        for platform in [Windows, Linux] {
            let (keymap, problems) = resolve_on(platform, &user);
            assert_eq!(
                problems,
                vec![Problem::AltGr {
                    command: id::STACCATO,
                    keys: KeySequence::parse("alt-secondary-k").unwrap(),
                }],
                "{platform:?}"
            );
            assert_eq!(keymap.label(id::STACCATO).as_deref(), Some("Alt+S"));
            assert!(
                keymap
                    .bindings()
                    .iter()
                    .all(|binding| !binding.keys.is_altgr_for(platform))
            );
        }
    }

    #[test]
    fn ctrl_and_the_command_key_are_one_key_off_macos() {
        // A user's Ctrl+B on Linux is the profile's Ctrl+B for Add bar.
        let (keymap, _) = resolve_on(Linux, &UserKeymap::new());
        let registry = Registry::builtin();
        let staccato = registry.get(id::STACCATO.as_str()).unwrap();
        assert_eq!(
            keymap.conflict(staccato, &KeySequence::parse("ctrl-b").unwrap(), &UsLayout),
            Some(id::BAR_AT_END)
        );
        let (keymap, _) = resolve(&UserKeymap::new());
        assert_eq!(
            keymap.conflict(staccato, &KeySequence::parse("ctrl-b").unwrap(), &UsLayout),
            None
        );
    }

    #[test]
    fn scopes_and_layout_decide_what_counts_as_a_conflict() {
        let (keymap, _) = resolve(&UserKeymap::new());
        let registry = Registry::builtin();
        let staccato = registry.get(id::STACCATO.as_str()).unwrap();
        // On a US layout Shift+6 produces "^": typing ^ would clash with ⇧6.
        assert_eq!(
            keymap.conflict(staccato, &KeySequence::parse("^").unwrap(), &UsLayout),
            Some(id::INTERVAL_6)
        );
        assert_eq!(
            keymap.conflict(staccato, &KeySequence::parse("n").unwrap(), &UsLayout),
            Some(id::TOGGLE_NOTE_INPUT)
        );
        assert_eq!(
            keymap.conflict(staccato, &KeySequence::parse("alt-s").unwrap(), &UsLayout),
            None
        );
        let close = registry.get(id::CLOSE_PANEL.as_str()).unwrap();
        assert_eq!(
            keymap.conflict(close, &KeySequence::parse("escape").unwrap(), &UsLayout),
            None
        );
        let mut user = UserKeymap::new();
        user.set(id::STACCATO, &[KeySequence::parse("numpad5").unwrap()]);
        let (_, problems) = resolve(&user);
        assert!(matches!(problems.as_slice(), [Problem::Unsupported { .. }]));
    }
}
