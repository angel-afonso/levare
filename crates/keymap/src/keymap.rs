//! Resolution of profile + user changes into the active bindings.

use crate::{
    CommandId, KeySequence, Layout, Profile, Registry, Scope, UserKeymap, commands::CommandSpec,
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
}

impl Keymap {
    pub fn resolve(
        registry: &Registry,
        profile: Profile,
        user: &UserKeymap,
        layout: &dyn Layout,
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
            let platform = keys.to_platform(layout);
            if platform.is_none() {
                problems.push(Problem::Unsupported {
                    command: spec.id,
                    keys: keys.clone(),
                });
            }
            let clash = platform.as_ref().and_then(|platform| {
                accepted.iter().flatten().find(|binding: &&Binding| {
                    binding.platform.as_ref() == Some(platform)
                        && binding.scope.overlaps(spec.scope)
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
                platform,
            });
        }
        (
            Self {
                profile,
                bindings: accepted.into_iter().flatten().collect(),
            },
            problems,
        )
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
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
        self.keys_for(command).first().map(|keys| keys.label())
    }

    /// Which other command already answers to `keys` where `spec` would work.
    pub fn conflict(
        &self,
        spec: &CommandSpec,
        keys: &KeySequence,
        layout: &dyn Layout,
    ) -> Option<CommandId> {
        let platform = keys.to_platform(layout)?;
        self.bindings
            .iter()
            .find(|binding| {
                binding.command != spec.id
                    && binding.platform.as_deref() == Some(platform.as_str())
                    && binding.scope.overlaps(spec.scope)
            })
            .map(|binding| binding.command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{UsLayout, commands::id};

    fn resolve(user: &UserKeymap) -> (Keymap, Vec<Problem>) {
        Keymap::resolve(
            &Registry::builtin(),
            Profile::sibelius_laptop(),
            user,
            &UsLayout,
        )
    }

    #[test]
    fn the_sibelius_profile_is_complete_and_free_of_conflicts() {
        let registry = Registry::builtin();
        for (keys, command) in &Profile::sibelius_laptop().bindings {
            assert!(
                registry.get(command.as_str()).is_some(),
                "{keys} → {command}"
            );
        }
        let (keymap, problems) = resolve(&UserKeymap::new());
        assert_eq!(problems, Vec::new());
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
        user.set(id::STACCATO, &[KeySequence::parse("cmd-b").unwrap()]);
        user.overrides
            .insert("no.existe".into(), vec!["cmd-j".into()]);
        user.overrides
            .insert(id::TENUTO.as_str().into(), vec!["hyper-x".into()]);
        let (keymap, problems) = resolve(&user);
        assert_eq!(keymap.label(id::STACCATO).as_deref(), Some("⌘B"));
        assert!(keymap.label(id::BAR_AT_END).is_none());
        assert!(problems.contains(&Problem::Conflict {
            keys: KeySequence::parse("cmd-b").unwrap(),
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
