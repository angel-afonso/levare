use crate::panels::{PanelMemory, PartsMode, RightTab, load, save};
use crate::ui::UiState;

#[test]
fn missing_file_gives_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let memory = load(&directory.path().join("paneles.json"));
    assert_eq!(memory, PanelMemory::default());
    assert!(memory.show_parts && memory.show_right);
    assert_eq!(memory.right, RightTab::Selection);
    assert_eq!(memory.parts_mode, PartsMode::List);
    assert!(memory.folded.is_empty());
}

#[test]
fn corrupt_file_gives_defaults() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("paneles.json");
    for text in ["", "{", "not json", "{\"right\": \"nowhere\"}", "[1, 2]"] {
        std::fs::write(&path, text).unwrap();
        assert_eq!(load(&path), PanelMemory::default(), "{text:?}");
    }
}

#[test]
fn round_trip_keeps_every_field() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("sub").join("paneles.json");
    let memory = PanelMemory {
        show_parts: false,
        show_right: false,
        right: RightTab::Review,
        parts_mode: PartsMode::Mix,
        folded: vec!["Cuerda".into(), "Metales".into()],
    };
    save(&path, &memory).unwrap();
    assert_eq!(load(&path), memory);
}

#[test]
fn unknown_fields_are_ignored() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("paneles.json");
    std::fs::write(
        &path,
        r#"{"show_parts": false, "right": "sound", "future": {"a": 1}}"#,
    )
    .unwrap();
    let memory = load(&path);
    assert!(!memory.show_parts);
    assert_eq!(memory.right, RightTab::Sound);
    assert!(memory.show_right);
}

#[test]
fn the_window_state_gives_back_what_it_restored() {
    let memory = PanelMemory {
        show_parts: false,
        show_right: true,
        right: RightTab::Sound,
        parts_mode: PartsMode::Mix,
        folded: vec!["Cuerda".into(), "Maderas".into()],
    };
    assert_eq!(UiState::restore(memory.clone()).panel_memory(), memory);
    assert_eq!(
        UiState::restore(PanelMemory::default()).panel_memory(),
        PanelMemory::default()
    );
}
