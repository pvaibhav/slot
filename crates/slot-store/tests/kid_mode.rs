use slot_store::{read_slot_state, write_slot_state, KidMode, SlotState};

fn card() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    d
}

#[test]
fn kid_mode_and_its_shelf_survive_a_restart() {
    let d = card();
    let s = SlotState {
        kid_mode: KidMode::OneShelf,
        kid_shelf: "collection/Puzzlers".into(),
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let back = read_slot_state(d.path());
    assert_eq!(back.kid_mode, KidMode::OneShelf);
    assert_eq!(back.kid_shelf, "collection/Puzzlers");
}

#[test]
fn shelves_survives_a_restart() {
    let d = card();
    let s = SlotState {
        kid_mode: KidMode::Shelves,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()).kid_mode, KidMode::Shelves);
}

#[test]
fn a_card_without_kid_lines_is_unlocked_on_all_games() {
    let d = card();
    write_slot_state(d.path(), &SlotState::default()).unwrap();
    let back = read_slot_state(d.path());
    assert_eq!(back.kid_mode, KidMode::Off);
    assert_eq!(back.kid_shelf, "all");
}

#[test]
fn an_unknown_kid_mode_reads_as_off() {
    let d = card();
    let s = SlotState {
        kid_mode: KidMode::Shelves,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let path = d.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("kid_mode=shelves\n"));
    std::fs::write(&path, text.replace("kid_mode=shelves", "kid_mode=toddler")).unwrap();
    assert_eq!(read_slot_state(d.path()).kid_mode, KidMode::Off);
}
