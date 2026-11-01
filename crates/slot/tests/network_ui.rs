mod common;

use std::sync::{Arc, Mutex, MutexGuard};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, KeyboardFor, Phase};
use slot::networks::{
    Line, NetPick, ENTER_PASSWORD, OTHER_NETWORK, SCANNING_MESSAGE, SCAN_NETWORKS,
};
use slot::wifi::{Backend, Cmd, Fake, Wifi};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, SlotState};
use slot_ui::{QuickRow, Toast};
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
fn scan_results_run_by_strength_without_saved_networks() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "CoffeeShop Guest", "");
    open_scan(&mut a);
    assert_eq!(
        lines(&a),
        vec![
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
    open_scan(&mut a);
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
    open_scan(&mut a);
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
    open_scan(&mut a);
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
fn the_list_starts_at_the_top_once_the_scan_lands() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_scan(&mut a);
    assert_eq!(labels(&a).last().unwrap(), "Other Network");
    assert_eq!(a.network_screen().unwrap().selected, 0);
    assert_eq!(selected_label(&a), "Home");
}

#[test]
fn other_network_keeps_the_pointer_across_a_rescan() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_scan(&mut a);
    select(&mut a, "Other Network");
    for _ in 0..4 {
        settle(&mut a);
    }
    assert_eq!(selected_label(&a), "Other Network");
}

#[test]
fn a_secured_network_asks_for_a_password_then_lands_on_its_page() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_scan(&mut a);
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
    open_scan(&mut a);
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
    assert!(matches!(a.phase(), Phase::Networks { pick: NetPick::Name(n) } if n == "Home"));
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
    open_scan(&mut a);
    select(&mut a, "Neighbor 5G");
    press(&mut a, Btn::A);
    type_text(&mut a, "secret99");
    press(&mut a, Btn::Y);
    assert!(a.keyboard().is_none());
    assert!(matches!(a.phase(), Phase::NetworkScan { .. }));
    assert_eq!(selected_label(&a), "Neighbor 5G");
    settle(&mut a);
    assert!(!log.lock().unwrap().iter().any(|l| l.starts_with("add")));
}

#[test]
fn a_password_over_63_characters_is_invalid() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_scan(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    type_text(&mut a, &"a".repeat(64));
    press(&mut a, Btn::Start);
    assert_eq!(
        a.update_text(),
        Some(("Invalid Password".to_string(), String::new()))
    );
}

#[test]
fn a_short_password_keeps_the_text_and_says_it_is_invalid() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_scan(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    type_text(&mut a, "abc");
    press(&mut a, Btn::Start);
    assert_eq!(
        a.update_text(),
        Some(("Invalid Password".to_string(), String::new()))
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
    open_scan(&mut a);
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
    open_scan(&mut a);
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
    open_scan(&mut a);
    enter(&mut a, "wrongpass");
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
    assert_eq!(a.toast(), Some(Toast::Forgotten));
    assert_eq!(a.forgotten_network(), Some("Home"));
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
    open_scan(&mut a);
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
    let mut fake = Fake::demo();
    fake.add("Home", "password", false);
    fake.add("Home", "password", false);
    let (_d, mut a, _log) = booted(fake);
    settle(&mut a);
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
    assert_eq!(labels(&a), ["Status", "IP Address", "Auto-Join", "Forget"]);
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
    open_scan(&mut a);
    assert!(a.wifi().held);
    assert!(labels(&a).contains(&"Home".to_string()));
    press(&mut a, Btn::B);
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

const TOOLS: [QuickRow; 3] = [QuickRow::Server, QuickRow::CheckUpdates, QuickRow::FetchArt];

#[test]
fn the_network_tools_show_only_while_connected() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    assert!(!a.wifi_up());
    let hidden = a.hidden_rows();
    assert!(TOOLS.iter().all(|r| hidden.contains(r)), "{hidden:?}");
    assert!(!hidden.contains(&QuickRow::Networks));
    join(&mut a, "CoffeeShop Guest", "");
    assert!(a.wifi_up());
    let hidden = a.hidden_rows();
    assert!(TOOLS.iter().all(|r| !hidden.contains(r)), "{hidden:?}");
}

#[test]
fn losing_the_connection_moves_the_pointer_off_a_tool() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    join(&mut a, "CoffeeShop Guest", "");
    a.apply(Action::QuickMenu);
    for _ in 0..12 {
        if a.quick_menu() == Some(QuickRow::Network) {
            break;
        }
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    for _ in 0..6 {
        if a.quick_menu() == Some(QuickRow::FetchArt) {
            break;
        }
        press(&mut a, Btn::Down);
    }
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::FetchArt),
        "{:?}",
        a.hidden_rows()
    );
    a.wifi_send(Cmd::Disconnect);
    settle(&mut a);
    assert!(!a.wifi_up());
    assert_eq!(a.quick_menu(), Some(QuickRow::Networks));
}

fn open_scan(a: &mut App) {
    open_networks(a);
    select(a, SCAN_NETWORKS);
    press(a, Btn::A);
    assert!(matches!(a.phase(), Phase::NetworkScan { .. }), "{:?}", a.phase());
    settle(a);
}

#[test]
fn the_network_page_is_saved_networks_then_the_scan_row() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::joined());
    open_networks(&mut a);
    assert_eq!(labels(&a), vec!["Home", SCAN_NETWORKS]);
    assert!(!logged(&log, "scan"));
}

#[test]
fn the_pointer_starts_on_the_connected_network() {
    let _g = serial();
    let fake = Fake::joined();
    let (_d, mut a, _log) = booted(fake);
    a.wifi_send(Cmd::Join {
        ssid: "CoffeeShop Guest".into(),
        password: String::new(),
        hidden: false,
    });
    settle(&mut a);
    open_networks(&mut a);
    assert_eq!(selected_label(&a), "CoffeeShop Guest");
}

#[test]
fn nothing_saved_leaves_only_the_scan_row() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_networks(&mut a);
    assert_eq!(labels(&a), vec![SCAN_NETWORKS]);
    assert!(!logged(&log, "scan"));
}

#[test]
fn the_scan_screen_says_so_until_the_results_land() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::joined());
    open_networks(&mut a);
    select(&mut a, SCAN_NETWORKS);
    press(&mut a, Btn::A);
    let s = a.network_screen().unwrap();
    assert!(s.lines.is_empty(), "{:?}", s.lines);
    assert_eq!(s.message, Some(SCANNING_MESSAGE));
    assert_eq!(a.network_action(), None);
    assert_eq!(a.network_back(), "Cancel");
    settle(&mut a);
    assert_eq!(a.network_screen().unwrap().message, None);
    assert_eq!(a.network_back(), "Back");
}

#[test]
fn scan_results_leave_out_saved_networks() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::joined());
    open_scan(&mut a);
    let found = labels(&a);
    assert!(!found.iter().any(|l| l == "Home"), "{found:?}");
    assert!(found.iter().any(|l| l == "Neighbor 5G"), "{found:?}");
    assert_eq!(found.last().map(String::as_str), Some(OTHER_NETWORK));
}

#[test]
fn b_cancels_the_scan_and_lands_on_the_scan_row() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::joined());
    open_networks(&mut a);
    select(&mut a, SCAN_NETWORKS);
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Networks { .. }), "{:?}", a.phase());
    assert_eq!(selected_label(&a), SCAN_NETWORKS);
    settle(&mut a);
    let scans = log.lock().unwrap().iter().filter(|l| *l == "scan").count();
    settle(&mut a);
    settle(&mut a);
    assert_eq!(log.lock().unwrap().iter().filter(|l| *l == "scan").count(), scans);
}

#[test]
fn a_second_scan_starts_from_the_message() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::joined());
    open_scan(&mut a);
    press(&mut a, Btn::B);
    settle(&mut a);
    select(&mut a, SCAN_NETWORKS);
    press(&mut a, Btn::A);
    assert_eq!(a.network_screen().unwrap().message, Some(SCANNING_MESSAGE));
}

#[test]
fn wifi_off_has_no_scan_row() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::joined());
    open_networks(&mut a);
    a.set_wifi_off(true);
    assert_eq!(labels(&a), vec!["Home"]);
}

fn page_labels(a: &App) -> Vec<String> {
    labels(a)
}

fn wait_failed(a: &mut App) {
    for _ in 0..8 {
        settle(a);
    }
}

fn enter(a: &mut App, password: &str) {
    select(a, "Home");
    press(a, Btn::A);
    type_text(a, password);
    press(a, Btn::Start);
    assert!(a.keyboard().is_none());
    wait_failed(a);
}

fn retype(a: &mut App, password: &str) {
    select(a, ENTER_PASSWORD);
    press(a, Btn::A);
    assert!(a.keyboard().is_some());
    type_text(a, password);
    press(a, Btn::Start);
    assert!(a.keyboard().is_none());
    wait_failed(a);
}

#[test]
fn a_new_network_with_a_wrong_password_is_not_saved() {
    let _g = serial();
    let (_d, mut a, log) = booted(Fake::demo());
    open_scan(&mut a);
    enter(&mut a, "wrongpass");
    assert!(a.wifi().saved.is_empty(), "{:?}", a.wifi().saved);
    assert!(logged(&log, "forget 0"), "{log:?}");
    let rows = page_labels(&a);
    assert!(rows.iter().any(|r| r == ENTER_PASSWORD), "{rows:?}");
    assert!(!rows.iter().any(|r| r == "Forget"), "{rows:?}");
    assert!(!rows.iter().any(|r| r == "Auto-Join"), "{rows:?}");
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Wrong Password"));
}

#[test]
fn entering_the_right_password_joins_and_saves_once() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    open_scan(&mut a);
    enter(&mut a, "wrongpass");
    retype(&mut a, "password");
    assert_eq!(a.wifi().saved.len(), 1);
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Connected"));
    assert!(page_labels(&a).iter().any(|r| r == "Forget"));
}

fn with_old_password() -> (TempDir, App, Log) {
    let mut fake = Fake::demo();
    let id = fake.add("Home", "oldpassword", false).unwrap();
    let (d, mut a, log) = booted(fake);
    a.wifi_send(Cmd::Connect(id));
    open_networks(&mut a);
    select(&mut a, "Home");
    press(&mut a, Btn::A);
    wait_failed(&mut a);
    (d, a, log)
}

#[test]
fn a_saved_network_that_rejects_its_password_stays_saved() {
    let _g = serial();
    let (_d, a, _log) = with_old_password();
    assert_eq!(a.wifi().saved.len(), 1);
    let rows = page_labels(&a);
    assert!(rows.iter().any(|r| r == ENTER_PASSWORD), "{rows:?}");
    assert!(rows.iter().any(|r| r == "Forget"), "{rows:?}");
}

#[test]
fn a_new_password_replaces_the_old_one() {
    let _g = serial();
    let (_d, mut a, _log) = with_old_password();
    retype(&mut a, "password");
    assert_eq!(a.wifi().saved.len(), 1);
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Connected"));
}

#[test]
fn a_second_wrong_password_keeps_the_saved_network() {
    let _g = serial();
    let (_d, mut a, _log) = with_old_password();
    retype(&mut a, "stillwrong");
    assert_eq!(a.wifi().saved.len(), 1);
    assert_eq!(lines(&a)[0].value.as_deref(), Some("Wrong Password"));
}

fn hold(a: &mut App, btn: Btn, ms: u64) {
    a.apply(Action::GbaDown(btn));
    let start = a.now();
    let mut t = start;
    while t < start + ms {
        t += 16;
        a.tick_ms(t);
    }
    a.apply(Action::GbaUp(btn));
}

fn on_keyboard(a: &mut App) {
    open_scan(a);
    select(a, "Neighbor 5G");
    press(a, Btn::A);
    assert!(a.keyboard().is_some());
}

#[test]
fn holding_a_direction_keeps_moving_on_the_keyboard() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    on_keyboard(&mut a);
    let col = a.keyboard().unwrap().col();
    a.apply(Action::GbaDown(Btn::Right));
    let (start, mut far) = (a.now(), col);
    for i in 1..60 {
        a.tick_ms(start + i * 16);
        far = far.max(a.keyboard().unwrap().col());
    }
    assert!(far > col + 1, "{col} to {far}");
}

#[test]
fn holding_b_keeps_deleting() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    on_keyboard(&mut a);
    type_text(&mut a, "secret99");
    hold(&mut a, Btn::B, 1000);
    assert!(a.keyboard().unwrap().text().len() < 7);
}

#[test]
fn holding_a_types_one_letter() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    on_keyboard(&mut a);
    hold(&mut a, Btn::A, 1000);
    assert_eq!(a.keyboard().unwrap().text().len(), 1);
}

#[test]
fn holding_a_on_delete_keeps_deleting() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    on_keyboard(&mut a);
    type_text(&mut a, "secret99");
    while a.keyboard().unwrap().row() != 0 {
        press(&mut a, Btn::Up);
    }
    while a.keyboard().unwrap().on_delete() == false {
        press(&mut a, Btn::Right);
    }
    hold(&mut a, Btn::A, 1000);
    assert!(a.keyboard().unwrap().text().len() < 7);
}

#[test]
fn b_held_to_close_the_help_does_not_delete() {
    let _g = serial();
    let (_d, mut a, _log) = booted(Fake::demo());
    on_keyboard(&mut a);
    type_text(&mut a, "secret99");
    press(&mut a, Btn::Menu);
    assert!(a.keyboard().unwrap().help());
    hold(&mut a, Btn::B, 1000);
    assert_eq!(a.keyboard().unwrap().text(), "secret99");
}
