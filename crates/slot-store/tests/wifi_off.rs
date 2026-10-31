use slot_store::{read_slot_state, write_slot_state, SlotState};

#[test]
fn wifi_off_round_trips_and_is_absent_by_default() {
    assert!(!SlotState::default().wifi_off);
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    write_slot_state(d.path(), &SlotState::default()).unwrap();
    let path = d.path().join("Config/slot.state");
    assert!(!std::fs::read_to_string(&path).unwrap().contains("wifi_off"));
    assert!(!read_slot_state(d.path()).wifi_off);
    let off = SlotState {
        wifi_off: true,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &off).unwrap();
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .any(|l| l == "wifi_off=1"));
    assert!(read_slot_state(d.path()).wifi_off);
}
