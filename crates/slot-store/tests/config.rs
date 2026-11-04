use slot_store::{move_config, read_slot_state};

const FILES: [(&str, &str); 5] = [
    ("slot.state", "slot.state"),
    ("selected_core.ini", "selected_core.txt"),
    ("video_mode.ini", "video_mode.txt"),
    ("cart_shell.ini", "cart_shell.txt"),
    ("retroachievements.toml", "retroachievements.toml"),
];

#[test]
fn the_players_files_move_from_system_to_config() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("System")).unwrap();
    for (f, _) in FILES {
        std::fs::write(d.path().join("System").join(f), f).unwrap();
    }
    std::fs::write(d.path().join("System/slot"), "binary").unwrap();

    move_config(d.path()).unwrap();

    for (f, to) in FILES {
        assert_eq!(
            std::fs::read_to_string(d.path().join("Config").join(to)).unwrap(),
            f
        );
        assert!(
            !d.path().join("System").join(f).exists(),
            "{f} was left behind"
        );
    }
    assert!(
        d.path().join("System/slot").exists(),
        "a shipped file moved"
    );
}

#[test]
fn a_file_already_in_config_is_not_overwritten() {
    let d = tempfile::tempdir().unwrap();
    for dir in ["System", "Config"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    std::fs::write(d.path().join("System/selected_core.ini"), "old").unwrap();
    std::fs::write(d.path().join("Config/selected_core.txt"), "new").unwrap();

    move_config(d.path()).unwrap();

    assert_eq!(
        std::fs::read_to_string(d.path().join("Config/selected_core.txt")).unwrap(),
        "new"
    );
}

#[test]
fn ini_settings_in_config_and_labels_become_txt() {
    let d = tempfile::tempdir().unwrap();
    for dir in ["Config", "Labels"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    for f in [
        "Config/selected_core.ini",
        "Config/video_mode.ini",
        "Config/cart_shell.ini",
        "Labels/cart_shell.ini",
    ] {
        std::fs::write(d.path().join(f), f).unwrap();
    }

    move_config(d.path()).unwrap();

    for f in [
        "Config/selected_core.ini",
        "Config/video_mode.ini",
        "Config/cart_shell.ini",
        "Labels/cart_shell.ini",
    ] {
        assert_eq!(
            std::fs::read_to_string(d.path().join(f.replace(".ini", ".txt"))).unwrap(),
            f
        );
        assert!(!d.path().join(f).exists(), "{f} was left behind");
    }
}

#[test]
fn a_txt_already_there_is_not_overwritten_by_its_ini() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Labels")).unwrap();
    std::fs::write(d.path().join("Labels/cart_shell.ini"), "old").unwrap();
    std::fs::write(d.path().join("Labels/cart_shell.txt"), "new").unwrap();

    move_config(d.path()).unwrap();

    assert_eq!(
        std::fs::read_to_string(d.path().join("Labels/cart_shell.txt")).unwrap(),
        "new"
    );
}

#[test]
fn settings_written_before_the_move_are_still_read_after_it() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("System")).unwrap();
    std::fs::write(
        d.path().join("System/slot.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=-240\n",
    )
    .unwrap();

    move_config(d.path()).unwrap();

    let s = read_slot_state(d.path());
    assert_eq!(
        (
            s.cart.as_deref(),
            s.volume,
            s.zone.offset_min(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            )
        ),
        (Some("Emerald"), 40, -240)
    );
}

#[test]
fn the_old_home_wifi_files_are_removed() {
    let d = tempfile::tempdir().unwrap();
    for dir in ["System", "Config", "System/slot-net/lib"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    for f in [
        "System/wifi.toml",
        "Config/wifi.toml",
        "Config/wifi.toml.example",
        "System/slot-services",
        "System/slot-net/lib/libnl.so",
        "Config/wifi.conf",
    ] {
        std::fs::write(d.path().join(f), f).unwrap();
    }

    move_config(d.path()).unwrap();

    for gone in [
        "System/wifi.toml",
        "Config/wifi.toml",
        "Config/wifi.toml.example",
        "System/slot-services",
        "System/slot-net",
    ] {
        assert!(!d.path().join(gone).exists(), "{gone} was left behind");
    }
    assert!(d.path().join("Config/wifi.conf").exists());
}
