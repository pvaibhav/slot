use slot_store::{read_slot_state, write_slot_state, Core, SlotState};

fn root() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    d
}

fn write_line(d: &std::path::Path, line: Option<&str>) {
    write_slot_state(d, &SlotState::default()).unwrap();
    let path = d.join("Config/slot.state");
    let mut text: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("gb_core="))
        .map(str::to_string)
        .collect();
    if let Some(line) = line {
        text.push(line.to_string());
    }
    std::fs::write(&path, text.join("\n") + "\n").unwrap();
}

#[test]
fn a_new_card_defaults_game_boy_to_gambatte() {
    assert_eq!(SlotState::default().gb_core, Core::Gambatte);
    let d = root();
    assert_eq!(read_slot_state(d.path()).gb_core, Core::Gambatte);
}

#[test]
fn an_existing_state_without_the_line_reads_mgba() {
    let d = root();
    write_line(d.path(), None);
    let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
    assert!(!text.contains("gb_core"));
    assert_eq!(read_slot_state(d.path()).gb_core, Core::Mgba);
}

#[test]
fn gb_core_round_trips() {
    let d = root();
    write_slot_state(d.path(), &SlotState::default()).unwrap();
    let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
    assert!(text.lines().any(|l| l == "gb_core=gambatte"));
    assert_eq!(read_slot_state(d.path()).gb_core, Core::Gambatte);
    let mgba = SlotState {
        gb_core: Core::Mgba,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &mgba).unwrap();
    assert_eq!(read_slot_state(d.path()).gb_core, Core::Mgba);
}

#[test]
fn gb_core_gpsp_reads_mgba() {
    let d = root();
    write_line(d.path(), Some("gb_core=gpsp"));
    assert_eq!(read_slot_state(d.path()).gb_core, Core::Mgba);
}
