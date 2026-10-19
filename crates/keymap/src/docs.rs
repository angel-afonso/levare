//! The shortcut reference in `docs/atajos.md`, generated from the registry so
//! the documentation cannot drift from the code.

use crate::{Category, Keymap, Registry, commands::Status};

pub fn markdown(registry: &Registry, keymap: &Keymap) -> String {
    let profile = keymap.profile();
    let mut text = format!(
        "# Atajos de teclado\n\n\
         Perfil por defecto: **{}**. {}\n\n\
         Puedes cambiarlos en **Partitura › Preferencias de atajos…** (⌘,). Tus cambios se \
         guardan aparte, en `atajos.json` dentro de la carpeta de datos de la aplicación.\n\n\
         Los comandos *pendientes* ya tienen su tecla reservada y muestran un aviso hasta \
         que la función exista.\n\n\
         > Archivo generado desde `crates/keymap`. Para actualizarlo: \
         `PARTITURA_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap`.\n",
        profile.name, profile.description
    );
    for category in Category::ALL {
        text.push_str(&format!(
            "\n## {}\n\n| Atajo | Comando | Estado |\n|---|---|---|\n",
            category.label()
        ));
        for spec in registry.in_category(category) {
            let keys = keymap
                .keys_for(spec.id)
                .iter()
                .map(|keys| format!("`{}`", keys.label()))
                .collect::<Vec<_>>();
            text.push_str(&format!(
                "| {} | {} | {} |\n",
                if keys.is_empty() {
                    "—".to_owned()
                } else {
                    keys.join(" · ")
                },
                spec.label,
                if spec.status == Status::Pending {
                    "Pendiente"
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
    use crate::{Profile, UsLayout, UserKeymap};
    use std::path::Path;

    #[test]
    fn docs_atajos_md_matches_the_registry() {
        let registry = Registry::builtin();
        let (keymap, _) = Keymap::resolve(
            &registry,
            Profile::sibelius_laptop(),
            &UserKeymap::new(),
            &UsLayout,
        );
        let expected = markdown(&registry, &keymap);
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/atajos.md");
        if std::env::var_os("PARTITURA_ACTUALIZAR_ATAJOS").is_some() {
            std::fs::write(&path, &expected).unwrap();
        }
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            current == expected,
            "docs/atajos.md está desactualizado; ejecuta \
             PARTITURA_ACTUALIZAR_ATAJOS=1 cargo test -p score-keymap"
        );
    }
}
