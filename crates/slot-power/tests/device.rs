use std::fs;
use std::path::{Path, PathBuf};

use slot_power::{motor_change, rumble_node, Battery, Charge, DevicePlatform, LedState, Platform};
use tempfile::TempDir;

fn sysfs_with(max_brightness: &str, capacity: &str, status: Option<&str>) -> TempDir {
    let d = tempfile::tempdir().unwrap();
    let bl = d.path().join("class/backlight/backlight-lcd0");
    fs::create_dir_all(&bl).unwrap();
    fs::write(bl.join("max_brightness"), format!("{max_brightness}\n")).unwrap();
    fs::write(bl.join("brightness"), "5\n").unwrap();
    let supply = d.path().join("class/power_supply");
    fs::create_dir_all(supply.join("axp2202-usb")).unwrap();
    fs::write(supply.join("axp2202-usb/type"), "USB\n").unwrap();
    fs::create_dir_all(supply.join("axp2202-battery")).unwrap();
    fs::write(supply.join("axp2202-battery/type"), "Battery\n").unwrap();
    fs::write(
        supply.join("axp2202-battery/capacity"),
        format!("{capacity}\n"),
    )
    .unwrap();
    if let Some(s) = status {
        fs::write(supply.join("axp2202-battery/status"), format!("{s}\n")).unwrap();
    }
    d
}

fn sysfs(max_brightness: &str, capacity: &str) -> TempDir {
    sysfs_with(max_brightness, capacity, Some("Discharging"))
}

fn charge_of(status: Option<&str>) -> Charge {
    platform(&sysfs_with("255", "87", status)).charge()
}

fn brightness(d: &TempDir) -> u32 {
    let text = fs::read_to_string(
        d.path()
            .join("class/backlight/backlight-lcd0")
            .join("brightness"),
    )
    .unwrap();
    text.trim().parse().unwrap()
}

fn platform(d: &TempDir) -> DevicePlatform {
    DevicePlatform::probe(d.path(), PathBuf::from("/mnt/sdcard"))
}

#[test]
fn the_backlight_steps_span_whatever_range_the_kernel_reports() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    p.set_backlight(10);
    assert_eq!(brightness(&d), 255);
    p.set_backlight(6);
    let mid = brightness(&d);
    assert!((40..=110).contains(&mid), "step 6 of 10 wrote {mid} of 255");
}

#[test]
fn step_one_is_the_dimmest_value_the_rg34xx_lights() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    p.set_backlight(1);
    assert_eq!(brightness(&d), 8);
}

#[test]
fn the_steps_follow_the_curve_from_the_floor_to_full() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    let wrote: Vec<u32> = (0..=10)
        .map(|step| {
            p.set_backlight(step);
            brightness(&d)
        })
        .collect();
    assert_eq!(wrote, [0, 8, 13, 26, 44, 68, 96, 129, 167, 209, 255]);
}

#[test]
fn every_step_is_brighter_than_the_one_below() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    let mut last = 0;
    for step in 1..=10 {
        p.set_backlight(step);
        let now = brightness(&d);
        assert!(now > last, "step {step} wrote {now}, not above {last}");
        last = now;
    }
}

#[test]
fn a_small_range_still_lights_step_one() {
    let d = sysfs("10", "87");
    let mut p = platform(&d);
    p.set_backlight(1);
    assert_eq!(brightness(&d), 1);
    p.set_backlight(10);
    assert_eq!(brightness(&d), 10);
}

#[test]
fn step_zero_is_dark() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    p.set_backlight(0);
    assert_eq!(brightness(&d), 0);
}

#[test]
fn the_battery_is_found_past_the_supplies_that_are_not_one() {
    let d = sysfs("255", "87");
    assert_eq!(platform(&d).battery().map(|b| b.percent), Some(87));
}

#[test]
fn a_gauge_that_reads_as_nonsense_is_no_reading_rather_than_zero() {
    let d = sysfs("255", "not a number");
    assert!(platform(&d).battery().is_none());
}

#[test]
fn a_tree_with_neither_a_panel_nor_a_gauge_still_boots() {
    let d = tempfile::tempdir().unwrap();
    let mut p = platform(&d);
    p.set_backlight(9);
    assert!(p.battery().is_none());
}

#[test]
fn the_three_states_the_kernel_names_are_taken_at_face_value() {
    assert_eq!(charge_of(Some("Charging")), Charge::Charging);
    assert_eq!(charge_of(Some("Discharging")), Charge::Discharging);
    assert_eq!(charge_of(Some("Full")), Charge::Full);
}

#[test]
fn everything_else_is_unknown_rather_than_a_guess() {
    assert_eq!(charge_of(Some("Not charging")), Charge::Unknown);
    assert_eq!(charge_of(Some("Unknown")), Charge::Unknown);
    assert_eq!(charge_of(Some("")), Charge::Unknown);
    assert_eq!(charge_of(None), Charge::Unknown);
}

#[test]
fn the_gauge_and_the_charge_state_come_back_together() {
    let d = sysfs_with("255", "87", Some("Charging"));
    assert_eq!(
        platform(&d).battery(),
        Some(Battery {
            percent: 87,
            charge: Charge::Charging,
        })
    );
}

#[test]
fn a_tree_with_no_gauge_reads_as_unknown_rather_than_absent() {
    let d = tempfile::tempdir().unwrap();
    let p = platform(&d);
    assert_eq!(p.battery(), None);
    assert_eq!(p.charge(), Charge::Unknown);
}

#[test]
fn the_probe_reports_what_it_found() {
    let report = platform(&sysfs("255", "87")).report();
    assert!(report.contains("backlight-lcd0"), "{report}");
    assert!(report.contains("255"), "the range was left out: {report}");
    assert!(report.contains("axp2202-battery"), "{report}");
}

#[test]
fn a_probe_that_found_nothing_says_so_rather_than_saying_nothing() {
    let d = tempfile::tempdir().unwrap();
    let report = DevicePlatform::probe(d.path(), PathBuf::from("/mnt/sdcard")).report();
    assert!(report.contains("no backlight"), "{report}");
    assert!(report.contains("no battery"), "{report}");
    assert!(report.contains("no rtc"), "{report}");
    assert!(report.contains("no motor"), "{report}");
}

#[test]
fn the_report_says_whether_there_is_an_rtc() {
    let d = sysfs("255", "87");
    fs::create_dir_all(d.path().join("class/rtc/rtc0")).unwrap();
    assert!(platform(&d).report().contains("rtc0"));
}

fn dispdbg_tree() -> TempDir {
    let d = tempfile::tempdir().unwrap();
    let dbg = d.path().join("kernel/debug/dispdbg");
    fs::create_dir_all(&dbg).unwrap();
    for f in ["name", "command", "param", "start"] {
        fs::write(dbg.join(f), "").unwrap();
    }
    d
}

fn dispdbg(d: &TempDir, file: &str) -> String {
    fs::read_to_string(d.path().join("kernel/debug/dispdbg").join(file))
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn a_tree_with_no_backlight_class_drives_the_panel_through_dispdbg() {
    let d = dispdbg_tree();
    let mut p = DevicePlatform::probe(d.path(), PathBuf::from("/mnt/sdcard"));
    p.set_backlight(10);
    assert_eq!(dispdbg(&d, "name"), "lcd0");
    assert_eq!(dispdbg(&d, "command"), "setbl");
    assert_eq!(
        dispdbg(&d, "param"),
        "255",
        "top step is not full brightness"
    );
    assert_eq!(dispdbg(&d, "start"), "1", "the write was never committed");
    p.set_backlight(0);
    assert_eq!(dispdbg(&d, "param"), "0", "step zero has to be dark");
}

#[test]
fn a_backlight_class_still_wins_over_dispdbg() {
    let d = sysfs("255", "87");
    let dbg = d.path().join("kernel/debug/dispdbg");
    fs::create_dir_all(&dbg).unwrap();
    fs::write(dbg.join("param"), "").unwrap();
    platform(&d).set_backlight(10);
    assert_eq!(brightness(&d), 255);
    assert_eq!(dispdbg(&d, "param"), "", "dispdbg was written to as well");
}

#[test]
fn the_motor_is_found_by_capability_rather_than_by_position() {
    let d = tempfile::tempdir().unwrap();
    for (node, ff) in [("event0", "0"), ("event1", "107030000 0"), ("event2", "0")] {
        let caps = d
            .path()
            .join("class/input")
            .join(node)
            .join("device/capabilities");
        fs::create_dir_all(&caps).unwrap();
        fs::write(caps.join("ff"), format!("{ff}\n")).unwrap();
    }
    assert_eq!(rumble_node(d.path()).as_deref(), Some("event1"));
}

#[test]
fn a_tree_with_no_force_feedback_has_no_motor() {
    let d = tempfile::tempdir().unwrap();
    let caps = d.path().join("class/input/event0/device/capabilities");
    fs::create_dir_all(&caps).unwrap();
    fs::write(caps.join("ff"), "20000 0\n").unwrap();
    assert!(rumble_node(d.path()).is_none());
}

#[test]
fn only_the_edge_between_still_and_moving_reaches_the_motor() {
    assert_eq!(motor_change(0, false), None);
    assert_eq!(motor_change(1, false), Some(true));
    assert_eq!(motor_change(65_535, true), None);
    assert_eq!(motor_change(1, true), None);
    assert_eq!(motor_change(0, true), Some(false));
}

fn leds(d: &TempDir, name: &str, attrs: &[(&str, &str)]) {
    let led = d.path().join("class/leds").join(name);
    fs::create_dir_all(&led).unwrap();
    for (k, v) in attrs {
        fs::write(led.join(k), format!("{v}\n")).unwrap();
    }
}

fn read(d: &TempDir, path: &str) -> String {
    fs::read_to_string(d.path().join("class/leds").join(path))
        .unwrap()
        .trim()
        .to_string()
}

#[test]
fn a_multicolour_led_gets_a_colour_per_state() {
    let d = sysfs("255", "87");
    leds(
        &d,
        "power",
        &[("multi_intensity", "0 0 0"), ("brightness", "0")],
    );
    let mut p = platform(&d);
    p.set_led(LedState::Charging);
    let amber = read(&d, "power/multi_intensity");
    p.set_led(LedState::Low);
    let red = read(&d, "power/multi_intensity");
    p.set_led(LedState::Running);
    let green = read(&d, "power/multi_intensity");
    assert_ne!(amber, red);
    assert_ne!(red, green);
    assert_ne!(amber, green);
}

#[test]
fn a_mono_led_is_on_except_when_the_battery_is_low() {
    let d = sysfs("255", "87");
    leds(
        &d,
        "power",
        &[("brightness", "0"), ("max_brightness", "255")],
    );
    let mut p = platform(&d);
    p.set_led(LedState::Running);
    assert_eq!(read(&d, "power/brightness"), "255");
    p.set_led(LedState::Charging);
    assert_eq!(read(&d, "power/brightness"), "255");
    p.set_led(LedState::Low);
    assert_eq!(read(&d, "power/brightness"), "0");
}

#[test]
fn a_tree_with_no_led_is_a_silent_no_op() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    p.set_led(LedState::Charging);
    p.set_led(LedState::Low);
    assert!(!d.path().join("class/leds").exists());
}

#[test]
fn relinking_adb_rebinds_the_gadget_to_the_controller() {
    let d = tempfile::tempdir().unwrap();
    let udc = d.path().join("class/udc/5100000.udc-controller");
    fs::create_dir_all(&udc).unwrap();
    let gadget = d.path().join("kernel/config/usb_gadget/g1");
    fs::create_dir_all(&gadget).unwrap();
    fs::write(gadget.join("UDC"), "\n").unwrap();

    let mut p = DevicePlatform::probe(d.path(), PathBuf::from("/mnt/sdcard"));
    assert!(
        p.relink_adb(),
        "the gadget and a controller were both there"
    );
    assert_eq!(
        fs::read_to_string(gadget.join("UDC")).unwrap().trim(),
        "5100000.udc-controller"
    );
}

#[test]
fn relinking_adb_without_a_gadget_does_nothing() {
    let d = tempfile::tempdir().unwrap();
    fs::create_dir_all(d.path().join("class/udc/5100000.udc-controller")).unwrap();
    let mut p = DevicePlatform::probe(d.path(), PathBuf::from("/mnt/sdcard"));
    assert!(!p.relink_adb());
}

fn with_jack(d: &TempDir, level: &str) {
    let dir = d.path().join("kernel/debug");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("gpio"),
        format!(
            "gpiochip0: GPIOs 0-31, parent: platform/pio, pio:\n gpio-259 (                    |Headphone detection ) in  {level}    \n gpio-260 (                    |usb_id_det          ) in  lo    \n"
        ),
    )
    .unwrap();
}

#[test]
fn headphones_follow_the_detection_pin() {
    let d = sysfs("255", "87");
    assert!(!platform(&d).headphones(), "no gpio file reads as speaker");
    with_jack(&d, "hi");
    assert!(platform(&d).headphones());
    with_jack(&d, "lo");
    assert!(!platform(&d).headphones());
}

const MOUNTS: &str = "\
/dev/mmcblk0p5 / ext4 rw,noatime 0 0
/dev/mmcblk0p7 /mnt/sdcard vfat ro,noatime,fmask=0022,errors=remount-ro 0 0
tmpfs /mnt/sdcard/System/slot tmpfs rw,relatime 0 0
/dev/mmcblk1p1 /mnt/other exfat rw,noatime,errors=remount-ro 0 0
";

#[test]
fn a_card_the_kernel_remounted_read_only_reads_as_read_only() {
    assert!(slot_power::read_only_in(MOUNTS, Path::new("/mnt/sdcard")));
}

#[test]
fn the_closest_mount_decides_not_the_root() {
    assert!(!slot_power::read_only_in(MOUNTS, Path::new("/mnt/other")));
    assert!(!slot_power::read_only_in(
        MOUNTS,
        Path::new("/mnt/sdcard/System/slot")
    ));
}

#[test]
fn an_option_that_merely_mentions_ro_is_not_read_only() {
    let m = "/dev/x /mnt/card vfat rw,errors=remount-ro 0 0\n";
    assert!(!slot_power::read_only_in(m, Path::new("/mnt/card")));
}

#[test]
fn the_kernel_log_keeps_only_card_trouble() {
    let dmesg = "\
[    1.561571] mmc0: host does not support reading read-only switch, assuming write-enable
[    2.240176] [EXFAT] trying to mount...
[    2.305657] [EXFAT] mounted successfully
[    2.900000] Error reading temperature for gpu thermal zone: -11
[   40.675342] FAT-fs (mmcblk0p7): Volume was not properly unmounted. Some data may be corrupt. Please run fsck.
[  612.000001] FAT-fs (mmcblk0p7): error, invalid access to FAT (entry 0x0001a2b3)
[  612.000002] FAT-fs (mmcblk0p7): Filesystem has been set read-only
[  700.000000] mmcblk1: r/w command failed, status = 0x80000900
[  700.000001] blk_update_request: I/O error, dev mmcblk1, sector 1234
";
    let kept = slot_power::card_kernel_lines(dmesg);
    assert_eq!(kept.len(), 7, "{kept:#?}");
    assert!(kept.iter().all(|l| !l.contains("read-only switch")));
    assert!(kept.iter().all(|l| !l.contains("thermal")));
}

#[test]
fn the_kernel_log_keeps_the_latest_card_lines() {
    let dmesg: String = (0..100)
        .map(|n| format!("[{n}] FAT-fs (mmcblk0p7): line {n}\n"))
        .collect();
    let kept = slot_power::card_kernel_lines(&dmesg);
    assert_eq!(kept.len(), 40);
    assert_eq!(kept.last().unwrap(), "[99] FAT-fs (mmcblk0p7): line 99");
}

#[test]
fn the_card_is_found_with_its_device_and_filesystem() {
    let m = slot_power::card_mount_in(MOUNTS, Path::new("/mnt/sdcard")).unwrap();
    assert_eq!(m.device, "/dev/mmcblk0p7");
    assert_eq!(m.point, Path::new("/mnt/sdcard"));
    assert_eq!(m.fs, "vfat");
    let other = slot_power::card_mount_in(MOUNTS, Path::new("/mnt/other/Saves")).unwrap();
    assert_eq!(other.device, "/dev/mmcblk1p1");
    assert_eq!(other.point, Path::new("/mnt/other"));
    assert_eq!(other.fs, "exfat");
}

#[test]
fn a_card_on_no_block_device_is_not_one_to_repair() {
    assert!(slot_power::card_mount_in(MOUNTS, Path::new("/mnt/sdcard/System/slot")).is_none());
}

#[test]
fn the_repair_is_staged_off_the_card() {
    let card = tempfile::tempdir().unwrap();
    let stage = tempfile::tempdir().unwrap();
    let system = card.path().join("System");
    fs::create_dir_all(&system).unwrap();
    fs::write(system.join("repair.sh"), "#!/bin/sh\n").unwrap();
    fs::write(system.join("fsck.exfat"), "elf").unwrap();
    let script = slot_power::stage_repair(card.path(), stage.path()).unwrap();
    assert_eq!(script, stage.path().join("repair.sh"));
    assert_eq!(fs::read_to_string(&script).unwrap(), "#!/bin/sh\n");
    assert_eq!(
        fs::read_to_string(stage.path().join("fsck.exfat")).unwrap(),
        "elf"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(stage.path().join("fsck.exfat"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(
            mode & 0o111,
            0,
            "fsck.exfat was staged without its execute bit"
        );
    }
}

#[test]
fn a_card_without_the_script_cannot_stage_a_repair() {
    let card = tempfile::tempdir().unwrap();
    let stage = tempfile::tempdir().unwrap();
    assert!(slot_power::stage_repair(card.path(), stage.path()).is_err());
}

#[test]
fn the_repair_result_is_read_once() {
    use slot_power::RepairResult;
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("slot-repair.result");
    assert_eq!(slot_power::take_repair_result(&file), None);
    for (text, want) in [
        ("repaired\n", RepairResult::Repaired),
        ("clean\n", RepairResult::Clean),
        ("failed\n", RepairResult::Failed),
        ("busy\n", RepairResult::Failed),
        ("unsupported\n", RepairResult::Failed),
    ] {
        fs::write(&file, text).unwrap();
        assert_eq!(slot_power::take_repair_result(&file), Some(want), "{text}");
        assert!(!file.exists(), "{text} was left to be shown again");
    }
}

fn second_panel(d: &TempDir, max: &str) -> PathBuf {
    let bl = d.path().join("class/backlight/backlight-lcd1");
    fs::create_dir_all(&bl).unwrap();
    fs::write(bl.join("max_brightness"), format!("{max}\n")).unwrap();
    fs::write(bl.join("brightness"), "7\n").unwrap();
    bl.join("brightness")
}

fn read_value(file: &PathBuf) -> u32 {
    fs::read_to_string(file).unwrap().trim().parse().unwrap()
}

#[test]
fn closing_the_lid_darkens_both_screens() {
    let d = sysfs("1666", "87");
    let top = second_panel(&d, "1666");
    let mut p = platform(&d);
    p.set_backlight(0);
    assert_eq!(brightness(&d), 0);
    assert_eq!(read_value(&top), 0);
}

#[test]
fn brightness_moves_both_screens_on_their_own_scales() {
    let d = sysfs("255", "87");
    let top = second_panel(&d, "1666");
    let mut p = platform(&d);
    p.set_backlight(10);
    assert_eq!(brightness(&d), 255);
    assert_eq!(read_value(&top), 1666);
}

fn sleep_script(d: &TempDir, body: &str) -> PathBuf {
    let script = d.path().join("sleep.sh");
    fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

fn lines_after(file: &Path, want: usize) -> usize {
    for _ in 0..200 {
        let n = fs::read_to_string(file)
            .map(|t| t.lines().count())
            .unwrap_or(0);
        if n >= want {
            return n;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fs::read_to_string(file)
        .map(|t| t.lines().count())
        .unwrap_or(0)
}

#[test]
fn sleep_runs_the_command_the_os_names() {
    let d = sysfs("255", "87");
    let mark = d.path().join("slept");
    let script = sleep_script(&d, &format!("echo slept >> {}", mark.display()));
    let mut p = platform(&d).with_sleep(script);
    p.sleep();
    assert_eq!(lines_after(&mark, 1), 1);
}

#[test]
fn a_sleep_still_running_is_not_started_again() {
    let d = sysfs("255", "87");
    let mark = d.path().join("slept");
    let script = sleep_script(&d, &format!("echo slept >> {}; sleep 0.3", mark.display()));
    let mut p = platform(&d).with_sleep(script);
    p.sleep();
    lines_after(&mark, 1);
    p.sleep();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert_eq!(lines_after(&mark, 1), 1);
    p.sleep();
    assert_eq!(lines_after(&mark, 2), 2);
    std::thread::sleep(std::time::Duration::from_millis(400));
}

#[test]
fn without_an_os_sleep_command_sleep_does_nothing() {
    let d = sysfs("255", "87");
    let mut p = platform(&d);
    p.sleep();
}

#[test]
fn the_charging_led_is_left_to_the_kernel() {
    let d = sysfs("255", "87");
    for name in ["amber:charging", "green:power", "mmc0::", "red:status"] {
        leds(&d, name, &[("brightness", "0"), ("max_brightness", "255")]);
    }
    let mut p = platform(&d);
    p.set_led(LedState::Running);
    p.set_led(LedState::Charging);
    assert_eq!(read(&d, "amber:charging/brightness"), "0", "slot drove the kernel's charging LED");
    assert_eq!(read(&d, "green:power/brightness"), "255");
}
