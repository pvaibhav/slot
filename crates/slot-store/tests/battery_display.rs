use slot_store::{read_slot_state, write_slot_state, BatteryDisplay, SlotState};

#[test]
fn the_battery_starts_with_both_icon_and_percent() {
    assert_eq!(SlotState::default().battery_display, BatteryDisplay::Both);
}

#[test]
fn each_battery_display_survives_a_restart() {
    for display in BatteryDisplay::ALL {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        let s = SlotState {
            battery_display: display,
            ..SlotState::default()
        };
        write_slot_state(d.path(), &s).unwrap();
        assert_eq!(read_slot_state(d.path()).battery_display, display);
    }
}

#[test]
fn a_card_written_before_the_setting_shows_both() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    let s = SlotState {
        battery_display: BatteryDisplay::Icon,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let path = d.path().join("Config/slot.state");
    let old: String = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("battery_display="))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&path, old).unwrap();
    assert_eq!(
        read_slot_state(d.path()).battery_display,
        BatteryDisplay::Both
    );
}

#[test]
fn stepping_cycles_through_all_three_both_ways() {
    let mut d = BatteryDisplay::Both;
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(d);
        d = d.step(true);
    }
    assert_eq!(d, BatteryDisplay::Both);
    assert_eq!(seen, BatteryDisplay::ALL);
    assert_eq!(BatteryDisplay::Both.step(false), BatteryDisplay::Percent);
}
