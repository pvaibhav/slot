mod common;

use std::sync::{Arc, Mutex, MutexGuard};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, KeyboardFor, Phase};
use slot::networks::Line;
use slot::wifi::{Cmd, Fake, Wifi};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

type Log = Arc<Mutex<Vec<String>>>;

fn booted(fake: Fake) -> (TempDir, App, Log) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    let log = fake.log.clone();
    a.set_wifi(Wifi::inline(Box::new(fake)));
    settle(&mut a);
    (d, a, log)
}

fn settle(a: &mut App) {
    for _ in 0..4 {
        let t = a.now();
        a.tick_ms(t + 1_000);
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn logged(log: &Log, line: &str) -> bool {
    log.lock().unwrap().iter().any(|l| l == line)
}

fn open_networks(a: &mut App) {
    a.apply(Action::QuickMenu);
    for row in [QuickRow::Network, QuickRow::Networks] {
        for _ in 0..12 {
            if a.quick_menu() == Some(row) {
                break;
            }
            press(a, Btn::Down);
        }
        assert_eq!(a.quick_menu(), Some(row), "{:?}", a.phase());
        if row == QuickRow::Network {
            press(a, Btn::A);
        }
    }
    press(a, Btn::A);
    assert!(
        matches!(a.phase(), Phase::Networks { .. }),
        "{:?}",
        a.phase()
    );
    settle(a);
}

fn lines(a: &App) -> Vec<Line> {
    a.network_screen().expect("no network screen").lines
}

fn labels(a: &App) -> Vec<String> {
    lines(a).into_iter().map(|l| l.label).collect()
}

fn selected_label(a: &App) -> String {
    let s = a.network_screen().unwrap();
    s.lines[s.selected].label.clone()
}

fn select(a: &mut App, label: &str) {
    for _ in 0..20 {
        if selected_label(a) == label {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("{label} is not on the list: {:?}", labels(a));
}

fn type_text(a: &mut App, text: &str) {
    for ch in text.chars() {
        let want = ch.to_string();
        let kb = a.keyboard().expect("no keyboard");
        let (r, c) = kb
            .rows()
            .iter()
            .enumerate()
            .find_map(|(r, row)| {
                row.iter()
                    .position(|cap| kb.label(*cap) == want)
                    .map(|c| (r, c))
            })
            .unwrap_or_else(|| panic!("no key for {ch}"));
        for _ in 0..12 {
            if a.keyboard().unwrap().row() == r {
                break;
            }
            press(a, Btn::Down);
        }
        for _ in 0..12 {
            if a.keyboard().unwrap().col() == c {
                break;
            }
            press(a, Btn::Right);
        }
        press(a, Btn::A);
    }
    assert!(a.keyboard().unwrap().text().ends_with(text));
}

fn line(label: &str, value: Option<&str>, lock: bool, bars: Option<u8>) -> Line {
    Line {
        label: label.into(),
        value: value.map(str::to_string),
        lock,
        bars,
    }
}

fn join(a: &mut App, ssid: &str, password: &str) {
    a.wifi_send(Cmd::Join {
        ssid: ssid.into(),
        password: password.into(),
        hidden: false,
    });
    settle(a);
}

#[test]
fn saved_networks_list_first_then_nearby_by_strength() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "CoffeeShop Guest", "");
    open_networks(&mut a);
    assert_eq!(
        lines(&a),
        vec![
            line("CoffeeShop Guest", Some("Connected"), false, None),
            line("Home", None, true, Some(3)),
            line("Neighbor 5G", None, true, Some(2)),
            line("xfinitywifi", None, false, Some(1)),
            line("Other Network", None, false, None),
        ]
    );
}

#[test]
fn the_list_scans_only_while_it_is_shown() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    let scans = |log: &Log| log.lock().unwrap().iter().filter(|l| *l == "scan").count();
    settle(&mut a);
    assert_eq!(scans(&log), 0);
    open_networks(&mut a);
    assert_eq!(scans(&log), 1);
    for _ in 0..12 {
        settle(&mut a);
    }
    assert!(scans(&log) >= 4, "{} scans", scans(&log));
    press(&mut a, Btn::B);
    let before = scans(&log);
    for _ in 0..12 {
        settle(&mut a);
    }
    assert_eq!(scans(&log), before);
}

#[test]
fn hidden_names_drop_and_duplicates_merge_into_the_strongest() {
    let _g = serial();
    let fake = Fake::new(
        vec![("", -40, true), ("Twin", -80, true), ("Twin", -50, true)],
        &[],
    );
    let (_d, mut a, _log) = booted(fake);
    open_networks(&mut a);
    assert_eq!(
        lines(&a),
        vec![
            line("Twin", None, true, Some(3)),
            line("Other Network", None, false, None),
        ]
    );
}

#[test]
fn the_selection_stays_on_its_name_across_a_rescan() {
    let _g = serial();
    let fake = Fake::demo();
    let nearby = fake.nearby.clone();
    let (_d, mut a, _log) = booted(fake);
    open_networks(&mut a);
    select(&mut a, "xfinitywifi");
    let before = a.network_screen().unwrap().selected;
    nearby
        .lock()
        .unwrap()
        .iter_mut()
        .find(|n| n.ssid == "xfinitywifi")
        .unwrap()
        .signal = -30;
    for _ in 0..4 {
        settle(&mut a);
    }
    let after = a.network_screen().unwrap();
    assert_eq!(after.lines[0].label, "xfinitywifi", "no rescan");
    assert_ne!(after.selected, before);
    assert_eq!(selected_label(&a), "xfinitywifi");
}

#[test]
fn a_secured_network_asks_for_a_password_then_lands_on_its_page() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    let kb = a.keyboard().expect("no keyboard");
    assert_eq!(kb.title(), "Password");
    assert_eq!(kb.subtitle(), Some("Home"));
    assert_eq!(kb.text(), "");
    assert_eq!(
        a.keyboard_for(),
        Some(&KeyboardFor::Password {
            ssid: "Home".into(),
            hidden: false
        })
    );
    type_text(&mut a, "password");
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert!(
        matches!(a.phase(), Phase::NetworkPage { ssid, .. } if ssid == "Home"),
        "{:?}",
        a.phase()
    );
    settle(&mut a);
    assert!(logged(&log, "add Home"));
    assert_eq!(lines(&a)[0], line("Status", Some("Connected"), false, None));
}

#[test]
fn an_open_network_joins_without_a_password() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "CoffeeShop Guest");
    press(&mut a, Btn::A);
    assert!(a.keyboard().is_none());
    assert!(matches!(a.phase(), Phase::NetworkPage { ssid, .. } if ssid == "CoffeeShop Guest"));
    settle(&mut a);
    assert!(logged(&log, "add CoffeeShop Guest"));
}

#[test]
fn a_saved_network_opens_its_page() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    open_networks(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    assert!(a.keyboard().is_none());
    assert!(matches!(a.phase(), Phase::NetworkPage { ssid, .. } if ssid == "Home"));
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Networks { name: Some(n) } if n == "Home"));
    assert_eq!(
        log.lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("add"))
            .count(),
        1
    );
}

#[test]
fn cancel_on_the_keyboard_saves_nothing() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Neighbor 5G");
    press(&mut a, Btn::A);
    type_text(&mut a, "secret99");
    press(&mut a, Btn::Y);
    assert!(a.keyboard().is_none());
    assert!(matches!(a.phase(), Phase::Networks { .. }));
    assert_eq!(selected_label(&a), "Neighbor 5G");
    settle(&mut a);
    assert!(!log.lock().unwrap().iter().any(|l| l.starts_with("add")));
}

#[test]
fn a_short_password_keeps_the_text_and_says_why() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    type_text(&mut a, "abc");
    press(&mut a, Btn::Start);
    assert_eq!(
        a.update_text().map(|t| t.0),
        Some("Passwords are 8 to 63 characters".to_string())
    );
    press(&mut a, Btn::B);
    assert_eq!(a.update_text(), None);
    assert_eq!(a.keyboard().map(|k| k.text()), Some("abc".to_string()));
    press(&mut a, Btn::Start);
    assert!(
        a.update_text().is_some(),
        "an empty secured password passed"
    );
    press(&mut a, Btn::B);
    settle(&mut a);
    assert!(!log.lock().unwrap().iter().any(|l| l.starts_with("add")));
}

#[test]
fn other_network_asks_name_then_password_and_saves_hidden() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Other Network");
    press(&mut a, Btn::A);
    assert_eq!(a.keyboard().map(|k| k.title()), Some("Network Name"));
    assert_eq!(a.keyboard_for(), Some(&KeyboardFor::NetworkName));
    type_text(&mut a, "attic");
    press(&mut a, Btn::Start);
    let kb = a.keyboard().expect("no password keyboard");
    assert_eq!(kb.title(), "Password");
    assert_eq!(kb.subtitle(), Some("Leave it empty for an open network"));
    assert_eq!(kb.text(), "");
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert!(matches!(a.phase(), Phase::NetworkPage { ssid, .. } if ssid == "attic"));
    settle(&mut a);
    assert!(
        logged(&log, "add attic hidden"),
        "{:?}",
        log.lock().unwrap()
    );
}

#[test]
fn an_empty_network_name_stays_on_the_keyboard() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Other Network");
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    assert_eq!(a.keyboard_for(), Some(&KeyboardFor::NetworkName));
}

fn on_home_page(a: &mut App) {
    open_networks(a);
    select(a, "Home");
    press(a, Btn::A);
    assert!(matches!(a.phase(), Phase::NetworkPage { ssid, .. } if ssid == "Home"));
}

fn select_row(a: &mut App, label: &str) {
    for _ in 0..10 {
        if selected_label(a) == label {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("no {label} row: {:?}", labels(a));
}

#[test]
fn the_network_page_shows_status_and_ip() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    settle(&mut a);
    assert_eq!(
        lines(&a),
        vec![
            line("Status", Some("Connected"), false, None),
            line("IP Address", Some("192.168.1.42"), false, None),
            line("Disconnect", None, false, None),
            line("Auto-Join", Some("On"), false, None),
            line("Forget", None, false, None),
        ]
    );
    select_row(&mut a, "Disconnect");
    press(&mut a, Btn::A);
    settle(&mut a);
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Not Connected"));
    assert_eq!(lines(&a)[1].value.as_deref(), Some("None"));
    assert_eq!(selected_label(&a), "Connect");
    press(&mut a, Btn::A);
    settle(&mut a);
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Connected"));
}

#[test]
fn a_wrong_password_shows_on_status() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_networks(&mut a);
    join(&mut a, "Home", "wrongpass");
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    for _ in 0..8 {
        settle(&mut a);
    }
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Wrong Password"));
}

#[test]
fn a_connected_networks_page_rows_are_exactly_the_five() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    settle(&mut a);
    assert_eq!(
        labels(&a),
        ["Status", "IP Address", "Disconnect", "Auto-Join", "Forget"]
    );
}

#[test]
fn auto_join_toggles() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    select_row(&mut a, "Auto-Join");
    for (btn, on) in [(Btn::A, "Off"), (Btn::Right, "On"), (Btn::Left, "Off")] {
        press(&mut a, btn);
        settle(&mut a);
        assert_eq!(selected_label(&a), "Auto-Join");
        let value = lines(&a)
            .into_iter()
            .find(|l| l.label == "Auto-Join")
            .and_then(|l| l.value);
        assert_eq!(value.as_deref(), Some(on), "{btn:?}");
    }
    assert!(logged(&log, "set_auto_join 0 false"));
    assert!(logged(&log, "set_auto_join 0 true"));
}

#[test]
fn forget_asks_first() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    select_row(&mut a, "Forget");
    press(&mut a, Btn::A);
    assert_eq!(a.forget_prompt(), Some("Forget Home?".to_string()));
    press(&mut a, Btn::B);
    assert_eq!(a.forget_prompt(), None);
    settle(&mut a);
    assert!(!logged(&log, "forget 0"));
    assert!(matches!(a.phase(), Phase::NetworkPage { .. }));
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    assert_eq!(a.forget_prompt(), None);
    settle(&mut a);
    assert!(logged(&log, "forget 0"));
    assert!(
        matches!(a.phase(), Phase::Networks { .. }),
        "{:?}",
        a.phase()
    );
    assert!(a.wifi().saved.is_empty());
}

#[test]
fn closing_the_lid_drops_the_keyboard() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    type_text(&mut a, "password");
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert!(a.keyboard().is_none());
    press(&mut a, Btn::Start);
    settle(&mut a);
    assert!(!logged(&log, "add Home"));
}

#[test]
fn closing_the_lid_drops_the_forget_prompt() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    select_row(&mut a, "Forget");
    press(&mut a, Btn::A);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    settle(&mut a);
    let before = a.selected_stem().map(str::to_string);
    press(&mut a, Btn::Right);
    settle(&mut a);
    assert_ne!(a.selected_stem().map(str::to_string), before);
    assert!(!logged(&log, "forget 0"));
}

#[test]
fn a_name_saved_twice_lists_once_and_forgets_both() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    join(&mut a, "Home", "password");
    assert_eq!(a.wifi().saved.len(), 2);
    open_networks(&mut a);
    assert_eq!(labels(&a).iter().filter(|l| *l == "Home").count(), 1);
    select(&mut a, "Home");
    press(&mut a, Btn::Down);
    assert_ne!(selected_label(&a), "Home");
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    select_row(&mut a, "Forget");
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    settle(&mut a);
    assert!(a.wifi().saved.is_empty(), "{:?}", a.wifi().saved);
}

#[test]
fn disconnect_is_hidden_when_wifi_is_off() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    open_networks(&mut a);
    a.set_wifi_off(true);
    settle(&mut a);
    let screen = a.network_screen().unwrap();
    assert_eq!(screen.lines, vec![line("Home", None, false, None)]);
    assert_eq!(screen.message, Some("Turn on Wi-Fi to see networks"));
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::NetworkPage { .. }));
    assert_eq!(
        labels(&a),
        [
            "Status",
            "IP Address",
            "Auto-Join",
            "Forget"
        ]
    );
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Wi-Fi Off"));
    assert!(!a.wifi().held);
}

#[test]
fn with_wifi_off_and_nothing_saved_the_list_only_says_so() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    a.set_wifi_off(true);
    log.lock().unwrap().clear();
    settle(&mut a);
    let screen = a.network_screen().unwrap();
    assert!(screen.lines.is_empty());
    assert_eq!(screen.message, Some("Turn on Wi-Fi to see networks"));
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Networks { .. }));
    assert!(a.keyboard().is_none());
    assert_eq!(
        log.lock().unwrap().iter().filter(|l| *l == "scan").count(),
        0
    );
}

#[test]
fn networks_holds_the_radio_without_a_wifi_conf() {
    let _g = serial();
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    let fake = Fake::demo();
    let log = fake.log.clone();
    a.set_wifi(Wifi::inline(Box::new(fake)));
    settle(&mut a);
    assert!(!logged(&log, "start"));
    open_networks(&mut a);
    assert!(a.wifi().held);
    assert!(labels(&a).contains(&"Home".to_string()));
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Shelf));
    settle(&mut a);
    assert!(!a.wifi().held);
}

#[test]
fn b_leaves_the_list_for_the_networks_row() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_networks(&mut a);
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Networks));
}

#[test]
fn leaving_networks_with_nothing_saved_releases_the_radio() {
    let _g = serial();
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_wifi(Wifi::inline(Box::new(Fake::demo())));
    settle(&mut a);
    open_networks(&mut a);
    assert!(a.wifi().held);
    std::fs::write(d.path().join("Config/wifi.conf"), b"update_config=1\n").unwrap();
    for _ in 0..3 {
        press(&mut a, Btn::B);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
    settle(&mut a);
    settle(&mut a);
    assert!(!a.wifi().held);
}

#[test]
fn forgetting_the_only_network_releases_the_radio_on_the_shelf() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "Home", "password");
    on_home_page(&mut a);
    select_row(&mut a, "Forget");
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    settle(&mut a);
    assert!(a.wifi().saved.is_empty());
    for _ in 0..3 {
        press(&mut a, Btn::B);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
    settle(&mut a);
    settle(&mut a);
    assert!(!a.wifi().held);
}
