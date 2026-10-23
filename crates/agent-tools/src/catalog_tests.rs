use super::*;
use score_keymap::{Category, Registry, Status, id};

#[test]
fn catalog_excludes_file_view_undo_and_dialogs() {
    let registry = Registry::builtin();
    let allowed = allowed_commands(&registry);
    assert!(!allowed.is_empty());
    for spec in &allowed {
        assert!(
            ![Category::File, Category::View].contains(&spec.category),
            "{}",
            spec.id
        );
        assert_eq!(spec.status, Status::Ready, "{}", spec.id);
        assert!(!EXCLUDED.contains(&spec.id), "{}", spec.id);
    }
    let ids: Vec<_> = allowed.iter().map(|spec| spec.id).collect();
    for command in [id::UNDO, id::REDO, id::GO_TO_BAR, id::EXPRESSION_TEXT] {
        assert!(!ids.contains(&command), "{command}");
    }
    for command in EXCLUDED {
        assert!(
            registry.get(command.as_str()).is_some(),
            "{command} no está en el registro"
        );
    }
    // A command of an allowed category is available without extra work.
    assert!(ids.contains(&id::TRILL));
    assert!(ids.contains(&id::PLAY));
    assert!(ids.contains(&id::REHEARSAL_MARK));
}

#[test]
fn every_allowed_category_contributes_commands() {
    let registry = Registry::builtin();
    let allowed = allowed_commands(&registry);
    for category in Category::ALL {
        let present = allowed.iter().any(|spec| spec.category == category);
        let expected = ![Category::File, Category::View].contains(&category);
        assert_eq!(present, expected, "{}", category.label());
    }
}
