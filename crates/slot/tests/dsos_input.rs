use std::fs;
use std::path::Path;

use slot::input::evdev::{Ev, Hat, Keymap, EV_ABS, EV_KEY};
use slot_input::{Btn, RawEvent};

fn key(code: u16, value: i32) -> Ev {
    Ev {
        kind: EV_KEY,
        code,
        value,
    }
}

fn abs(code: u16, value: i32) -> Ev {
    Ev {
        kind: EV_ABS,
        code,
        value,
    }
}

fn write(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

fn caps(codes: &[u16]) -> String {
    let top = codes
        .iter()
        .map(|c| usize::from(*c) / 64)
        .max()
        .unwrap_or(0);
    let mut words = vec![0u64; top + 1];
    for c in codes {
        words[usize::from(*c) / 64] |= 1 << (c % 64);
    }
    words
        .iter()
        .rev()
        .map(|w| format!("{w:x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn only_is_dsos_set_to_one_means_dsos() {
    assert_eq!(Keymap::from_is_dsos(Some("1")), Keymap::DsOs);
    for other in [None, Some("0"), Some(""), Some("yes")] {
        assert_eq!(Keymap::from_is_dsos(other), Keymap::BaseOs, "{other:?}");
    }
}

#[test]
fn the_flag_is_read_from_the_release_file() {
    let d = tempfile::tempdir().unwrap();
    let release = d.path().join("baseos-release");
    write(
        &release,
        "BASEOS_TARGET=rgdsplus\nBASEOS_PLATFORM=rk3568\nIS_DSOS=1\n",
    );
    assert_eq!(Keymap::of_release(None, &release), Keymap::DsOs);
}

#[test]
fn a_baseos_release_or_none_at_all_is_baseos() {
    let d = tempfile::tempdir().unwrap();
    let release = d.path().join("baseos-release");
    write(&release, "BASEOS_TARGET=rgsp\nBASEOS_PLATFORM=h700\n");
    assert_eq!(Keymap::of_release(None, &release), Keymap::BaseOs);
    assert_eq!(
        Keymap::of_release(None, &d.path().join("missing")),
        Keymap::BaseOs
    );
}

#[test]
fn the_environment_flag_wins_over_the_release_file() {
    let d = tempfile::tempdir().unwrap();
    let release = d.path().join("baseos-release");
    write(&release, "IS_DSOS=1\n");
    assert_eq!(
        Keymap::of_release(Some("0".into()), &release),
        Keymap::BaseOs
    );
}

#[test]
fn the_ds_plus_buttons_map_by_the_codes_it_sends() {
    let expected = [
        (0x131, Btn::A),
        (0x130, Btn::B),
        (0x133, Btn::X),
        (0x134, Btn::Y),
        (0x136, Btn::L1),
        (0x137, Btn::R1),
        (0x138, Btn::L2),
        (0x139, Btn::R2),
        (0x13a, Btn::Select),
        (0x13b, Btn::Start),
        (0x135, Btn::Menu),
        (115, Btn::VolUp),
        (114, Btn::VolDown),
        (116, Btn::Power),
    ];
    for (code, want) in expected {
        assert_eq!(Keymap::DsOs.btn(code), Some(want), "code {code:#x}");
    }
}

#[test]
fn f_and_the_stick_press_do_nothing_yet() {
    for code in [0x13c, 0x13d, 0x13e] {
        assert_eq!(Keymap::DsOs.btn(code), None, "code {code:#x}");
    }
}

#[test]
fn the_same_code_means_its_own_button_on_each_system() {
    assert_eq!(
        Keymap::DsOs.to_raw(key(0x130, 1)),
        Some(RawEvent::Down(Btn::B))
    );
    assert_eq!(
        Keymap::BaseOs.to_raw(key(0x130, 1)),
        Some(RawEvent::Down(Btn::A))
    );
    assert_eq!(
        Keymap::DsOs.to_raw(key(0x135, 0)),
        Some(RawEvent::Up(Btn::Menu))
    );
}

#[test]
fn the_ds_plus_d_pad_keys_press_the_directions() {
    let mut hat = Hat::new(Keymap::DsOs);
    for (code, dir) in [
        (0x220, Btn::Up),
        (0x221, Btn::Down),
        (0x222, Btn::Left),
        (0x223, Btn::Right),
    ] {
        assert!(hat.takes(key(code, 1)), "{dir:?}");
        assert_eq!(hat.feed(key(code, 1)), vec![RawEvent::Down(dir)]);
        assert_eq!(hat.feed(key(code, 0)), vec![RawEvent::Up(dir)]);
    }
}

#[test]
fn a_d_pad_key_repeat_is_not_a_press() {
    let mut hat = Hat::new(Keymap::DsOs);
    assert_eq!(hat.feed(key(0x220, 1)), vec![RawEvent::Down(Btn::Up)]);
    assert!(hat.feed(key(0x220, 2)).is_empty());
}

#[test]
fn rocking_the_d_pad_lets_go_of_the_first_way_first() {
    let mut hat = Hat::new(Keymap::DsOs);
    assert_eq!(hat.feed(key(0x222, 1)), vec![RawEvent::Down(Btn::Left)]);
    assert_eq!(
        hat.feed(key(0x223, 1)),
        vec![RawEvent::Up(Btn::Left), RawEvent::Down(Btn::Right)]
    );
    assert!(hat.feed(key(0x222, 0)).is_empty());
    assert_eq!(hat.feed(key(0x223, 0)), vec![RawEvent::Up(Btn::Right)]);
}

#[test]
fn baseos_takes_no_keys_into_the_d_pad() {
    let hat = Hat::new(Keymap::BaseOs);
    assert!(!hat.takes(key(0x220, 1)));
    assert!(!hat.takes(key(0x130, 1)));
    assert!(hat.takes(abs(0x10, 1)));
}

#[test]
fn the_ds_plus_stick_presses_the_d_pad_past_half_tilt() {
    let mut hat = Hat::new(Keymap::DsOs);
    hat.steer_with_stick(true);
    assert!(hat.feed(abs(0x01, -800)).is_empty());
    assert_eq!(hat.feed(abs(0x01, -1000)), vec![RawEvent::Down(Btn::Up)]);
    assert_eq!(hat.feed(abs(0x01, 0)), vec![RawEvent::Up(Btn::Up)]);
    assert_eq!(hat.feed(abs(0x00, 1800)), vec![RawEvent::Down(Btn::Right)]);
}

#[test]
fn the_second_stick_the_ds_plus_does_not_have_is_ignored() {
    let mut hat = Hat::new(Keymap::DsOs);
    hat.steer_with_stick(true);
    assert!(hat.feed(abs(0x03, -1800)).is_empty());
    assert!(hat.feed(abs(0x04, 1800)).is_empty());
}

#[test]
fn the_d_pad_held_through_a_stick_flick_stays_held_on_the_ds_plus() {
    let mut hat = Hat::new(Keymap::DsOs);
    hat.steer_with_stick(true);
    assert_eq!(hat.feed(key(0x220, 1)), vec![RawEvent::Down(Btn::Up)]);
    assert!(hat.feed(abs(0x01, -1800)).is_empty());
    assert!(hat.feed(abs(0x01, 0)).is_empty());
    assert_eq!(hat.feed(key(0x220, 0)), vec![RawEvent::Up(Btn::Up)]);
}

#[test]
fn the_ds_plus_always_has_a_stick() {
    assert!(Keymap::DsOs.has_stick(""));
    assert!(Keymap::DsOs.has_stick("rgdsplus"));
    assert!(Keymap::BaseOs.has_stick("rg35xxh"));
    assert!(!Keymap::BaseOs.has_stick("rgsp"));
}

#[test]
fn the_unlock_chord_reads_from_the_ds_plus_codes() {
    let mut bits = vec![0u8; 0x300 / 8 + 1];
    for code in [0x138u16, 0x139, 0x13a, 0x13b] {
        bits[(code / 8) as usize] |= 1 << (code % 8);
    }
    let held = Keymap::DsOs.held_from_bits(&bits);
    assert!(slot::kid::unlock_chord(&held), "{held:?}");
}

#[test]
fn the_back_button_node_is_opened_and_touch_is_not() {
    let d = tempfile::tempdir().unwrap();
    let (dev, sys) = (d.path().join("dev/input"), d.path().join("sys/class/input"));
    let nodes = [
        ("event0", "adc-keys-back", caps(&[0x135])),
        ("event1", "Goodix Capacitive TouchScreen", caps(&[0x14a])),
        ("event2", "retrogame_joypad", caps(&[0x130, 0x131, 0x220])),
    ];
    for (node, name, keys) in &nodes {
        write(&dev.join(node), "");
        write(&sys.join(node).join("device/name"), &format!("{name}\n"));
        write(
            &sys.join(node).join("device/capabilities/key"),
            &format!("{keys}\n"),
        );
    }
    let picked: Vec<_> = Keymap::DsOs
        .pick_devices(&dev, &sys)
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(picked, ["event0", "event2"]);
}

#[test]
fn a_ds_plus_d_pad_key_is_traced_as_its_direction() {
    let line = slot::input::trace::event_line(Keymap::DsOs, "event2", 9, key(0x221, 1));
    assert!(line.contains("Down"), "{line}");
    assert!(!line.contains("unmapped"), "{line}");
}
