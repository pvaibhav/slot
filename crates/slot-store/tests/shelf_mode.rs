use slot_store::{read_slot_state, write_slot_state, ShelfMode, SlotState};

fn card() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    d
}

#[test]
fn shelf_mode_defaults_to_all() {
    assert_eq!(SlotState::default().shelf_mode, ShelfMode::All);
}

#[test]
fn shelf_mode_survives_a_restart() {
    let d = card();
    let s = SlotState {
        shelf_mode: ShelfMode::Collections,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()).shelf_mode, ShelfMode::Collections);
}

#[test]
fn an_unknown_shelf_mode_reads_as_all() {
    let d = card();
    write_slot_state(d.path(), &SlotState::default()).unwrap();
    let path = d.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap() + "shelf_mode=sideways\n";
    std::fs::write(&path, text).unwrap();
    assert_eq!(read_slot_state(d.path()).shelf_mode, ShelfMode::All);
}

#[test]
fn the_remembered_collection_survives_a_restart() {
    let d = card();
    let s = SlotState {
        collection: Some("Dragon Ball".into()),
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(
        read_slot_state(d.path()).collection.as_deref(),
        Some("Dragon Ball")
    );
    write_slot_state(d.path(), &SlotState::default()).unwrap();
    assert_eq!(read_slot_state(d.path()).collection, None);
}

#[test]
fn the_gba_shelf_last_used_round_trips() {
    let d = card();
    let state = slot_store::SlotState {
        gba_video: true,
        ..slot_store::SlotState::default()
    };
    slot_store::write_slot_state(d.path(), &state).unwrap();
    assert!(slot_store::read_slot_state(d.path()).gba_video);
    assert!(!slot_store::SlotState::default().gba_video);
}
