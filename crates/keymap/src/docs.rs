//! The shortcut reference in `docs/atajos.md`, generated from the registry so
//! the documentation cannot drift from the code.

use crate::{
    Category, Keymap, Layout, Platform, Profile, Registry, UserKeymap, commands::Status, id,
};

/// One keymap per platform, from the default profile and no user changes.
pub fn default_keymaps(registry: &Registry, layout: &dyn Layout) -> [Keymap; 3] {
    Platform::ALL.map(|platform| {
        Keymap::resolve_for(
            registry,
            Profile::sibelius_laptop_for(platform),
            &UserKeymap::new(),
            layout,
            platform,
        )
        .0
    })
}

/// `keymaps` holds macOS, Windows and Linux, in `Platform::ALL` order. Command
/// names and keys are written as the Spanish interface shows them.
pub fn markdown(registry: &Registry, keymaps: &[Keymap; 3]) -> String {
    let [macos, windows, linux] = keymaps;
    let profile = macos.profile();
    let preferences = |keymap: &Keymap| keymap.label(id::PREFERENCES).unwrap_or_default();
    let mut text = format!(
        "# Keyboard shortcuts\n\n\
         Default profile: **{}**. {}\n\n\
         Change them in **Archivo › Preferencias de atajos…** (`{}` on macOS, `{}` on Windows \
         and Linux). Your changes are kept apart, in `atajos.json` in the app's data folder, \
         and work on every platform: a shortcut saved with ⌘ on macOS uses Ctrl elsewhere. \
         One with ⌥⌘ and a letter, digit or symbol is left out on Windows and Linux, where \
         Ctrl+Alt is AltGr or a desktop shortcut; Preferences says so and suggests Ctrl+Mayús.\n\n\
         *Pending* commands already have their key and show a notice until the feature \
         exists.\n\n\
         > Generated from `crates/keymap`. To update it: \
         `LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.\n",
        profile.name,
        profile.description,
        preferences(macos),
        preferences(windows),
    );
    for category in Category::ALL {
        text.push_str(&format!(
            "\n## {}\n\n| macOS | Windows and Linux | Command | Status |\n|---|---|---|---|\n",
            category.label()
        ));
        for spec in registry.in_category(category) {
            let keys = |keymap: &Keymap| {
                let keys = keymap
                    .keys_for(spec.id)
                    .iter()
                    .map(|keys| format!("`{}`", keys.label_for(keymap.platform())))
                    .collect::<Vec<_>>();
                if keys.is_empty() {
                    "—".to_owned()
                } else {
                    keys.join(" · ")
                }
            };
            let (on_windows, on_linux) = (keys(windows), keys(linux));
            let elsewhere = if on_windows == on_linux {
                on_windows
            } else {
                format!("Windows: {on_windows} · Linux: {on_linux}")
            };
            text.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                keys(macos),
                elsewhere,
                spec.label,
                if spec.status == Status::Pending {
                    "Pending"
                } else {
                    ""
                }
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::UsLayout;
    use std::path::Path;

    #[test]
    fn docs_atajos_md_matches_the_registry() {
        let registry = Registry::builtin();
        let expected = markdown(&registry, &default_keymaps(&registry, &UsLayout));
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/atajos.md");
        if std::env::var_os("LEVARE_ACTUALIZAR_ATAJOS").is_some()
            || std::env::var_os("PARTITURA_ACTUALIZAR_ATAJOS").is_some()
        {
            std::fs::write(&path, &expected).unwrap();
        }
        // A Windows checkout may turn the file's line endings into CRLF.
        let current = std::fs::read_to_string(&path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert!(
            current == expected,
            "docs/atajos.md is out of date; run \
             LEVARE_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap"
        );
    }
}
