mod common;

use std::sync::{Arc, Mutex, MutexGuard};

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::update::{UpdateEvent, UpdateJob, UpdateJobs};
use slot::wifi::{Backend, Cmd, Fake, Wifi};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, SlotState};
use slot_ui::QuickRow;
use std::collections::VecDeque;
use tempfile::TempDir;

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

type Log = Arc<Mutex<Vec<String>>>;

fn booted(conf: bool, mut fake: Fake) -> (TempDir, App, Log) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    if conf {
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        fake.add("Home", "password", false);
    }
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    let log = fake.log.clone();
    a.set_wifi(Wifi::inline(Box::new(fake)));
    (d, a, log)
}

fn on_shelf() -> (TempDir, App, Log) {
    booted(true, Fake::demo())
}

fn count(log: &Log, line: &str) -> usize {
    log.lock().unwrap().iter().filter(|l| *l == line).count()
}

fn settle(a: &mut App) {
    for _ in 0..4 {
        let t = a.now();
        a.tick_ms(t + 1_000);
    }
}

fn frames(a: &mut App, n: usize) {
    for _ in 0..n {
        let t = a.now();
        a.tick_ms(t + 16);
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn join_home(a: &mut App) {
    a.wifi_send(Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    settle(a);
}

#[test]
fn boot_holds_the_radio_after_startup() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    a.tick_ms(a.now() + 1);
    assert_eq!(count(&log, "start"), 0);
    assert!(!a.wifi().held);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
    assert!(a.wifi().held);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
}

#[test]
fn connecting_at_init_holds_once() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    a.connect_wifi();
    a.tick_ms(a.now() + 1);
    assert!(a.wifi().held);
    settle(&mut a);
    a.connect_wifi();
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
}

#[test]
fn no_wifi_conf_never_holds() {
    let _g = serial();
    let (_d, mut a, log) = booted(false, Fake::demo());
    a.connect_wifi();
    settle(&mut a);
    assert_eq!(count(&log, "start"), 0);
}

#[test]
fn a_game_releases_it() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    a.apply(Action::Insert);
    frames(&mut a, 2);
    assert_eq!(count(&log, "stop"), 1);
    assert!(!a.wifi().held);
}

#[test]
fn sleep_releases_it_and_waking_holds_it_again() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    a.apply(Action::LidClose);
    frames(&mut a, 2);
    assert_eq!(count(&log, "stop"), 1);
    a.apply(Action::LidOpen);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 2);
    assert!(a.wifi().held);
}

#[test]
fn menus_keep_it() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    a.apply(Action::QuickMenu);
    settle(&mut a);
    assert_eq!(count(&log, "stop"), 0);
    assert_eq!(count(&log, "start"), 1);
}

#[test]
fn a_studio_fetch_keeps_the_radio_once_and_a_game_releases_it() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    join_home(&mut a);
    a.apply(Action::SelectTap);
    a.tick_ms(a.now() + 1);
    assert_eq!(count(&log, "start"), 1);
    a.close_studio();
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
    a.apply(Action::Insert);
    frames(&mut a, 2);
    assert_eq!(count(&log, "stop"), 1);
}

#[test]
fn a_failed_hold_tries_again_after_a_minute() {
    let _g = serial();
    let (_d, mut a, log) = booted(true, Fake::broken());
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
    assert!(!a.wifi().held);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
    for _ in 0..7 {
        let t = a.now();
        a.tick_ms(t + 10_000);
    }
    assert_eq!(count(&log, "start"), 2);
}

#[test]
fn the_status_bar_follows_the_connection() {
    let _g = serial();
    let (_d, mut a, _log) = on_shelf();
    let face = |n: usize| (slot_ui::TexId::from_raw(900 + n), 18, 14);
    a.set_wifi_faces([
        [face(0), face(1), face(2), face(3)],
        [face(4), face(5), face(6), face(7)],
    ]);
    settle(&mut a);
    assert!(!a.wifi_up());
    assert_eq!(wifi_icons(&a), 0);
    join_home(&mut a);
    assert!(a.wifi_up());
    assert_eq!(wifi_icons(&a), 1);
    a.apply(Action::Insert);
    frames(&mut a, 2);
    assert!(!a.wifi_up());
}

fn wifi_icons(a: &App) -> usize {
    let mut out = Vec::new();
    a.draw(&mut out);
    let ids: Vec<_> = (900..908).map(slot_ui::TexId::from_raw).collect();
    out.iter()
        .filter(|d| matches!(d, slot_ui::Draw::Tex { tex, .. } if ids.contains(tex)))
        .count()
}

#[derive(Default)]
struct Jobs {
    asked: Vec<UpdateJob>,
    events: VecDeque<UpdateEvent>,
}

#[derive(Clone, Default)]
struct FakeJobs(Arc<Mutex<Jobs>>);

impl UpdateJobs for FakeJobs {
    fn ask(&mut self, job: UpdateJob) {
        self.0.lock().unwrap().asked.push(job);
    }

    fn take(&mut self) -> Option<UpdateEvent> {
        self.0.lock().unwrap().events.pop_front()
    }
}

#[test]
fn an_ota_check_leaves_the_radio_held() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    let jobs = FakeJobs::default();
    a.set_update_jobs(Box::new(jobs.clone()));
    join_home(&mut a);
    assert!(a.wifi_up());
    a.apply(Action::QuickMenu);
    for row in [QuickRow::Network, QuickRow::CheckUpdates] {
        for _ in 0..12 {
            if a.quick_menu() == Some(row) {
                break;
            }
            press(&mut a, Btn::Down);
        }
        assert_eq!(a.quick_menu(), Some(row), "{:?}", a.hidden_rows());
        press(&mut a, Btn::A);
    }
    assert_eq!(jobs.0.lock().unwrap().asked, [UpdateJob::Check]);
    jobs.0
        .lock()
        .unwrap()
        .events
        .push_back(UpdateEvent::UpToDate);
    a.tick_ms(a.now() + 1);
    press(&mut a, Btn::B);
    settle(&mut a);
    assert!(a.wifi().held);
    assert!(a.wifi_up());
    assert_eq!(count(&log, "start"), 1);
    assert_eq!(count(&log, "stop"), 0);
}

fn booted_off(mut fake: Fake) -> (TempDir, App, Log) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            wifi_off: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    fake.add("Home", "password", false);
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    let log = fake.log.clone();
    a.set_wifi(Wifi::inline(Box::new(fake)));
    (d, a, log)
}

#[test]
fn wifi_off_never_holds_at_boot() {
    let _g = serial();
    let (_d, mut a, log) = booted_off(Fake::demo());
    a.connect_wifi();
    settle(&mut a);
    assert_eq!(count(&log, "start"), 0);
    assert!(!a.wifi().held);
}

#[test]
fn turning_wifi_off_blocks_the_radio() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    assert!(a.wifi().held);
    a.set_wifi_off(true);
    settle(&mut a);
    assert!(!a.wifi().held);
    assert_eq!(count(&log, "stop"), 1);
    assert_eq!(count(&log, "block"), 1);
    assert!(a.wifi_is_off());
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
}

#[test]
fn turning_it_back_on_holds_it() {
    let _g = serial();
    let (_d, mut a, log) = booted_off(Fake::demo());
    settle(&mut a);
    assert_eq!(count(&log, "start"), 0);
    a.set_wifi_off(false);
    a.tick_ms(a.now() + 1);
    assert_eq!(count(&log, "start"), 1);
    assert!(a.wifi().held);
    a.set_wifi_off(false);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 1);
}

#[test]
fn ending_link_play_with_wifi_off_blocks_the_radio_again() {
    let _g = serial();
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            wifi_off: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.set_wifi_off(true);
    let fake = Fake::demo();
    let blocked = fake.blocked.clone();
    a.set_wifi(Wifi::inline(Box::new(fake)));
    a.begin_link(0);
    assert!(!blocked.load(std::sync::atomic::Ordering::SeqCst));
    a.end_link();
    frames(&mut a, 2);
    assert!(blocked.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn wifi_off_blocks_the_radio_once_after_boot() {
    let _g = serial();
    let (_d, mut a, log) = booted_off(Fake::demo());
    settle(&mut a);
    settle(&mut a);
    assert_eq!(count(&log, "start"), 0);
    assert_eq!(count(&log, "block"), 1);
}

#[test]
fn the_studio_does_not_hold_the_radio_when_wifi_is_off() {
    let _g = serial();
    let (_d, mut a, log) = booted_off(Fake::demo());
    settle(&mut a);
    a.apply(Action::SelectTap);
    assert!(a.studio().is_some());
    settle(&mut a);
    assert_eq!(count(&log, "start"), 0);
    assert!(!a.wifi().held);
}

#[test]
fn a_held_radio_turned_off_stays_blocked_through_a_game() {
    let _g = serial();
    let (_d, mut a, log) = on_shelf();
    settle(&mut a);
    join_home(&mut a);
    a.apply(Action::SelectTap);
    assert!(a.studio().is_some());
    a.tick_ms(a.now() + 1);
    assert_eq!(count(&log, "start"), 1);
    assert!(a.wifi().held);
    a.set_wifi_off(true);
    press(&mut a, Btn::B);
    settle(&mut a);
    assert!(a.studio().is_none());
    assert!(!a.wifi().held);
    assert_eq!(count(&log, "block"), 1);
    a.apply(Action::Insert);
    frames(&mut a, 2);
    assert_eq!(count(&log, "start"), 1);
    assert_eq!(count(&log, "stop"), 1);
    assert!(!a.wifi().held);
}

#[test]
fn the_networks_row_waits_for_an_address() {
    let _g = serial();
    let fake = Fake::demo();
    let no_lease = fake.no_lease.clone();
    no_lease.store(true, std::sync::atomic::Ordering::SeqCst);
    let (_d, mut a, _log) = booted(true, fake);
    settle(&mut a);
    join_home(&mut a);
    assert_eq!(a.wifi().status.state, slot::wifi::State::Completed);
    assert!(!a.wifi_up());
    assert_eq!(a.networks_value(), "Not Connected");
    no_lease.store(false, std::sync::atomic::Ordering::SeqCst);
    settle(&mut a);
    assert!(a.wifi_up());
    assert_eq!(a.networks_value(), "Home");
}

#[test]
fn the_networks_row_names_the_joined_network() {
    let _g = serial();
    let (_d, mut a, _log) = on_shelf();
    settle(&mut a);
    assert_eq!(a.networks_value(), "Not Connected");
    join_home(&mut a);
    assert_eq!(a.networks_value(), "Home");
    a.set_wifi_off(true);
    assert_eq!(a.networks_value(), "Off");
}

#[test]
fn the_studio_offers_the_label_printer_only_while_connected() {
    let _g = serial();
    let (_d, mut a, _log) = booted(true, Fake::silent());
    settle(&mut a);
    assert!(!a.wifi_up());
    a.apply(Action::SelectTap);
    a.tick_ms(a.now() + 1);
    let rows = a.studio().expect("no studio").rows();
    assert!(!rows.contains(&slot::studio::Row::Label), "{rows:?}");
    a.close_studio();
    settle(&mut a);
    let (_d, mut a, _log) = on_shelf();
    settle(&mut a);
    join_home(&mut a);
    assert!(a.wifi_up());
    a.apply(Action::SelectTap);
    a.tick_ms(a.now() + 1);
    let rows = a.studio().expect("no studio").rows();
    assert!(rows.contains(&slot::studio::Row::Label), "{rows:?}");
}

fn with_network_time(on: bool) -> (TempDir, App, common::Clock) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            network_time: on,
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_wifi(Wifi::inline(Box::new(Fake::joined())));
    (d, a, clock)
}

fn open_clock(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::General.position(&[]) {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    for _ in 0..QuickRow::DateTime.position(&a.hidden_rows()) {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
}

#[test]
fn boot_hands_a_saved_network_time_to_the_system() {
    let (_d, _a, clock) = with_network_time(true);
    assert_eq!(clock.network_time(), Some(true));
}

#[test]
fn boot_hands_network_time_off_to_the_system() {
    let (_d, _a, clock) = with_network_time(false);
    assert_eq!(clock.network_time(), Some(false));
}

#[test]
fn the_clock_screen_offers_network_time_with_a_saved_network() {
    let (_d, mut a, _clock) = with_network_time(false);
    settle(&mut a);
    open_clock(&mut a);
    assert_eq!(a.picker().and_then(|p| p.network_time()), Some(false));
}

#[test]
fn the_clock_screen_keeps_quiet_without_a_saved_network() {
    let (_d, mut a, _log) = booted(false, Fake::demo());
    settle(&mut a);
    open_clock(&mut a);
    assert_eq!(a.picker().and_then(|p| p.network_time()), None);
}

#[test]
fn confirming_network_time_saves_it_and_turns_it_on() {
    let (d, mut a, clock) = with_network_time(false);
    settle(&mut a);
    open_clock(&mut a);
    a.apply(Action::GbaDown(Btn::A));
    a.confirm_clock();
    assert!(read_slot_state(d.path()).network_time);
    assert_eq!(clock.network_time(), Some(true));
}

#[test]
fn an_edit_left_behind_network_time_is_dropped() {
    let (_d, mut a, clock) = with_network_time(false);
    settle(&mut a);
    open_clock(&mut a);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Up));
    a.apply(Action::GbaDown(Btn::A));
    a.confirm_clock();
    assert_eq!(clock.get(), CLOCK_IS_SET);
}
