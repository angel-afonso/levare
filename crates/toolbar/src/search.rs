//! Command search: every registered command, ranked for a typed query, with
//! where it lives in the ribbon so the result also teaches the app.
use crate::{
    TABS,
    menu::{MenuItemView, item_view},
    project::CommandStates,
};
use score_keymap::{CommandId, Keymap, Registry, Scope, Status, id};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchResult {
    pub item: MenuItemView,
    /// "Entrada › Duración", or the category when the ribbon lacks it.
    pub place: String,
}

/// Lowercase without accents or diaeresis, so "Calderón" matches "calderon".
pub fn normalize(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

/// Where `command` sits in the ribbon, if it does: "{tab} › {group}".
pub fn ribbon_place(command: CommandId) -> Option<String> {
    TABS.iter().find_map(|tab| {
        tab.groups
            .iter()
            .find(|group| group.controls.iter().any(|c| c.command == command))
            .map(|group| format!("{} › {}", tab.id.label(), group.title))
    })
}

/// Score of one query word against a command, or `None` when it is absent.
fn word_score(word: &str, label: &str, elsewhere: &str) -> Option<u32> {
    if label.split_whitespace().any(|w| w.starts_with(word)) {
        Some(3)
    } else if label.contains(word) {
        Some(2)
    } else if elsewhere.contains(word) {
        Some(1)
    } else {
        None
    }
}

/// Up to `limit` commands matching every word of `query`, best first: ready
/// before pending, then by score, then those in the ribbon, then registry order.
pub fn search(
    query: &str,
    registry: &Registry,
    keymap: &Keymap,
    states: &dyn CommandStates,
    limit: usize,
) -> Vec<SearchResult> {
    let query = normalize(query);
    let words: Vec<&str> = query.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<(bool, u32, bool, usize, SearchResult)> = Vec::new();
    for (index, spec) in registry.iter().enumerate() {
        if spec.scope == Scope::Modal || spec.id == id::ESCAPE {
            continue;
        }
        let ribbon = ribbon_place(spec.id);
        let place = ribbon
            .clone()
            .unwrap_or_else(|| spec.category.label().to_owned());
        let label = normalize(spec.label);
        let elsewhere = normalize(&format!("{place} {}", spec.category.label()));
        let mut total = 0;
        let mut all = true;
        for word in &words {
            match word_score(word, &label, &elsewhere) {
                Some(score) => total += score,
                None => {
                    all = false;
                    break;
                }
            }
        }
        if all {
            found.push((
                spec.status == Status::Pending,
                total,
                ribbon.is_none(),
                index,
                SearchResult {
                    item: item_view(spec.id, registry, keymap, states),
                    place,
                },
            ));
        }
    }
    found.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(b.1.cmp(&a.1))
            .then(a.2.cmp(&b.2))
            .then(a.3.cmp(&b.3))
    });
    found
        .into_iter()
        .take(limit)
        .map(|(.., result)| result)
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

    fn run(query: &str) -> Vec<SearchResult> {
        let registry = Registry::builtin();
        let (keymap, _) = Keymap::resolve(
            &registry,
            Profile::sibelius_laptop(),
            &UserKeymap::new(),
            &UsLayout,
        );
        search(query, &registry, &keymap, &Defaults, 10)
    }

    #[test]
    fn normalize_strips_accents_and_case() {
        assert_eq!(normalize("Calderón ÜÑ"), "calderon un");
    }

    #[test]
    fn a_prefix_finds_the_command_and_says_where_it_lives() {
        let results = run("stacc");
        assert_eq!(results[0].item.command, id::STACCATO);
        assert_eq!(results[0].place, "Entrada › Articulaciones y ligaduras");
    }

    #[test]
    fn accents_do_not_matter() {
        assert!(
            run("calderon")
                .iter()
                .any(|r| r.item.command == id::FERMATA)
        );
    }

    #[test]
    fn every_word_must_match() {
        assert_eq!(run("ligadura union")[0].item.command, id::TIE);
        assert!(run("zzzz").is_empty());
        assert!(run("   ").is_empty());
    }

    #[test]
    fn ready_commands_come_before_pending_ones() {
        let results = run("texto");
        assert!(
            results
                .iter()
                .any(|r| r.item.command == id::EXPRESSION_TEXT)
        );
        let first_pending = results.iter().position(|r| r.item.pending).unwrap();
        assert!(results[first_pending..].iter().all(|r| r.item.pending));
        assert!(first_pending > 0, "ready results exist for texto");
    }

    #[test]
    fn modal_and_escape_commands_are_never_offered() {
        for query in ["cerrar", "escape", "panel", "e", "a"] {
            for r in run(query) {
                assert_ne!(r.item.command, id::CLOSE_PANEL, "{query}");
                assert_ne!(r.item.command, id::ESCAPE, "{query}");
            }
        }
    }

    #[test]
    fn commands_outside_the_ribbon_show_their_category() {
        assert_eq!(ribbon_place(id::GO_TO_BAR), None);
        let results = run("ir al compas");
        let go = results
            .iter()
            .find(|r| r.item.command == id::GO_TO_BAR)
            .expect("go to bar is found");
        assert_eq!(go.place, "Compases");
    }
}
