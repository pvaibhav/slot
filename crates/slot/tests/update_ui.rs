mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{app_booting_at, connect, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::build_info::Build;
use slot::core_picker::{CLOSE_MS, HOP_MS, OPEN_MS};
use slot::update::{Kind, Release, UpdateEvent, UpdateFail, UpdateJob, UpdateJobs};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

#[derive(Default)]
struct Log {
    asked: Vec<UpdateJob>,
    events: VecDeque<UpdateEvent>,
    cancels: usize,
    committed: bool,
}

#[derive(Clone, Default)]
struct FakeJobs(Arc<Mutex<Log>>);

impl UpdateJobs for FakeJobs {
    fn ask(&mut self, job: UpdateJob) {
        self.0.lock().unwrap().asked.push(job);
    }

    fn take(&mut self) -> Option<UpdateEvent> {
        self.0.lock().unwrap().events.pop_front()
    }

    fn cancel(&mut self) -> bool {
        let mut log = self.0.lock().unwrap();
        log.cancels += 1;
        !log.committed
    }
}

impl FakeJobs {
    fn asked(&self) -> Vec<UpdateJob> {
        self.0.lock().unwrap().asked.clone()
    }

    fn cancels(&self) -> usize {
        self.0.lock().unwrap().cancels
    }

    fn commit(&self) {
        self.0.lock().unwrap().committed = true;
    }

    fn send(&self, a: &mut App, event: UpdateEvent) {
        self.0.lock().unwrap().events.push_back(event);
        a.tick_ms(1);
    }
}

fn big_release() -> Release {
    Release {
        size: 8_000,
        ..release()
    }
}

fn release() -> Release {
    Release {
        kind: Kind::Slot,
        running: Build::current().version.into(),
        version: "99.0.0".into(),
        url: "https://example.invalid/slot.zip".into(),
        sha256: [0; 32],
        size: 1,
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn on_updates() -> (TempDir, App, FakeJobs) {
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
    let jobs = FakeJobs::default();
    a.set_update_jobs(Box::new(jobs.clone()));
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    connect(&mut a);
    a.apply(Action::QuickMenu);
    while a.quick_menu() != Some(QuickRow::Network) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    for _ in 0..6 {
        if a.quick_menu() == Some(QuickRow::CheckUpdates) {
            break;
        }
        press(&mut a, Btn::Down);
    }
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::CheckUpdates),
        "{:?}",
        a.hidden_rows()
    );
    (d, a, jobs)
}

fn on_updates_row(a: &App) -> bool {
    !a.update_shown() && a.quick_menu() == Some(QuickRow::CheckUpdates)
}

fn advance(a: &mut App, ms: u64) {
    for _ in 0..ms / 25 {
        a.tick_ms(a.now() + 25);
    }
}

fn settle(a: &mut App) {
    for _ in 0..60 {
        a.tick_ms(a.now() + 50);
        a.update(0.05);
    }
}

fn text(a: &App) -> Option<(String, String)> {
    a.update_text()
}

#[test]
fn check_for_updates_opens_the_ota_scene() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
    assert!(a.update_shown());
    assert!(!a.restarting());
}

#[test]
fn y_checks_and_a_newer_release_offers_install() {
    let (_d, mut a, jobs) = on_updates();
    assert!(a.update_offered());
    press(&mut a, Btn::A);
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
    assert_eq!(
        text(&a),
        Some(("Checking for updates".into(), String::new()))
    );
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    assert_eq!(
        text(&a),
        Some(("An update is available".into(), String::new()))
    );
    press(&mut a, Btn::A);
    assert_eq!(
        jobs.asked(),
        [UpdateJob::Check, UpdateJob::Install(release())]
    );
    assert_eq!(text(&a), None);
    assert!(a.update_shown());
    assert_eq!(a.update_progress(), Some(0.0));
}

#[test]
fn progress_counts_bytes_against_the_release_size() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![big_release()]));
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Progress(2_000));
    assert_eq!(a.update_progress(), Some(0.25));
    jobs.send(&mut a, UpdateEvent::Progress(8_000));
    assert_eq!(a.update_progress(), Some(1.0));
}

#[test]
fn staged_closes_the_cart_seats_it_and_then_relaunches() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![big_release()]));
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Staged);
    assert!(!a.relaunching(), "relaunched before the cart closed");
    let staged = a.now();
    let mut sounds = Vec::new();
    for _ in 0..60 {
        a.tick_ms(a.now() + 50);
        a.update(0.05);
        sounds.extend(a.take_sfx());
        if a.relaunching() {
            let insert_ms = (slot::app::INSERT_S * 1000.0) as u64;
            let open_ms = (slot::app::POWER_ON_S * 1000.0) as u64;
            let took = a.now() - staged;
            let picker_ms = OPEN_MS + 1000 + HOP_MS + CLOSE_MS;
            assert!(
                took >= picker_ms + insert_ms + open_ms,
                "relaunched after only {took} ms, before the {insert_ms} ms insert and {open_ms} ms screen opening finished"
            );
            break;
        }
    }
    assert!(a.relaunching(), "never relaunched after the cart closed");
    assert!(!a.restarting(), "an update reboots instead of relaunching");
    assert_eq!(sounds, [slot::audio::Sfx::Insert]);
}

#[test]
fn cancel_returns_to_about() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    press(&mut a, Btn::B);
    assert!(on_updates_row(&a));
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
}

#[test]
fn up_to_date_says_so_and_b_returns_to_about() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::UpToDate);
    let running = Build::current().version;
    assert_eq!(
        text(&a),
        Some((
            "slot is up to date".into(),
            format!("{running} is the latest version.")
        ))
    );
    press(&mut a, Btn::B);
    assert!(on_updates_row(&a));
}

#[test]
fn b_does_not_cancel_a_download() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(a.update_shown());
    assert_eq!(a.update_progress(), Some(0.0));
}

#[test]
fn a_clock_failure_says_to_set_the_clock() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Failed(UpdateFail::Clock));
    assert_eq!(
        text(&a),
        Some((
            "Could not check for updates".into(),
            "Set the date and time, then try again.".into()
        ))
    );
}

#[test]
fn every_failure_has_its_own_words() {
    let want = [
        (
            UpdateFail::NoWifi,
            "Could not join Wi-Fi",
            "Check Wi-Fi in the General menu.",
        ),
        (
            UpdateFail::Offline,
            "Could not connect",
            "Check the network has internet access.",
        ),
        (
            UpdateFail::Download,
            "Download failed",
            "Nothing was changed. Try again later.",
        ),
        (
            UpdateFail::Checksum,
            "The download was damaged",
            "Nothing was changed. Try again later.",
        ),
        (
            UpdateFail::Bad,
            "The update could not be read",
            "Nothing was changed. Try again later.",
        ),
        (
            UpdateFail::Space,
            "Not enough space on the card",
            "Free some space, then try again.",
        ),
    ];
    for (fail, title, detail) in want {
        let (_d, mut a, jobs) = on_updates();
        press(&mut a, Btn::A);
        jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
        press(&mut a, Btn::A);
        jobs.send(&mut a, UpdateEvent::Failed(fail.clone()));
        assert_eq!(text(&a), Some((title.into(), detail.into())), "{fail:?}");
        press(&mut a, Btn::B);
        assert!(!a.update_shown());
    }
}

#[test]
fn a_check_that_cannot_reach_the_server_says_to_try_again_later() {
    for fail in [
        UpdateFail::Offline,
        UpdateFail::Download,
        UpdateFail::Bad,
        UpdateFail::Checksum,
    ] {
        let (_d, mut a, jobs) = on_updates();
        press(&mut a, Btn::A);
        jobs.send(&mut a, UpdateEvent::Failed(fail.clone()));
        assert_eq!(
            text(&a),
            Some((
                "Could not fetch update info".into(),
                "Try again later.".into()
            )),
            "{fail:?}"
        );
        press(&mut a, Btn::B);
        assert!(!a.update_shown());
    }
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Failed(UpdateFail::NoWifi));
    assert_eq!(
        text(&a),
        Some((
            "Could not join Wi-Fi".into(),
            "Check Wi-Fi in the General menu.".into()
        ))
    );
}

#[test]
fn leaving_the_network_page_mid_check_drops_the_result() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(on_updates_row(&a));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Network));
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    assert!(!a.update_shown());
    assert_eq!(a.quick_menu(), Some(QuickRow::Network));
}

#[test]
fn no_update_while_linked() {
    let (_d, mut a, jobs) = on_updates();
    a.begin_link(1);
    assert!(!a.update_offered());
    press(&mut a, Btn::A);
    assert!(jobs.asked().is_empty());
}

#[test]
fn closing_the_lid_mid_check_drops_the_prompt() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    assert!(!a.update_shown());
}

#[test]
fn closing_the_lid_mid_download_still_relaunches() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    press(&mut a, Btn::A);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    jobs.send(&mut a, UpdateEvent::Staged);
    settle(&mut a);
    assert!(a.relaunching());
}

#[test]
fn a_second_check_waits_for_the_first_to_answer() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(!a.update_offered());
    press(&mut a, Btn::A);
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
    assert!(!a.update_shown());
    jobs.send(&mut a, UpdateEvent::Failed(UpdateFail::Offline));
    assert!(!a.update_shown());
    assert!(a.update_offered());
    press(&mut a, Btn::A);
    assert_eq!(jobs.asked(), [UpdateJob::Check, UpdateJob::Check]);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Staged);
    settle(&mut a);
    assert!(a.relaunching());
}

#[test]
fn x_on_about_restarts() {
    for wifi in [true, false] {
        let (_d, mut s) = session_on_about(wifi);
        let a = s.app_mut();
        a.apply(Action::GbaDown(Btn::X));
        a.apply(Action::GbaUp(Btn::X));
        assert!(a.restarting(), "wifi.conf {wifi}");
        assert!(!a.update_shown(), "wifi.conf {wifi}");
    }
}

fn session_on_about(wifi: bool) -> (TempDir, slot::session::Session) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    if wifi {
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    }
    let (mut s, _motor) = common::session_with_platform(d.path());
    s.app_mut().set_update_jobs(Box::new(FakeJobs::default()));
    s.app_mut().apply(Action::QuickMenu);
    s.app_mut().apply(Action::GbaDown(Btn::Up));
    s.app_mut().apply(Action::GbaUp(Btn::Up));
    s.app_mut().apply(Action::GbaDown(Btn::A));
    s.app_mut().apply(Action::GbaUp(Btn::A));
    assert!(matches!(s.app().phase(), Phase::About));
    (d, s)
}

fn select_start(s: &mut slot::session::Session) {
    use slot_input::RawEvent;
    s.feed([RawEvent::Down(Btn::Select)], 10);
    s.feed([RawEvent::Down(Btn::Start)], 20);
    s.feed([RawEvent::Up(Btn::Start)], 30);
    s.feed([RawEvent::Up(Btn::Select)], 40);
}

#[test]
fn select_start_restarts_on_updates() {
    for wifi in [true, false] {
        let (_d, mut s) = session_on_about(wifi);
        select_start(&mut s);
        assert!(s.app().restarting(), "wifi.conf {wifi}");
    }
}

#[test]
fn select_start_does_not_restart_on_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut s, _motor) = common::session_with_platform(d.path());
    assert!(matches!(s.app().phase(), Phase::Shelf));
    select_start(&mut s);
    assert!(!s.app().restarting());
}

#[test]
fn the_chip_hops_from_the_old_version_to_the_new_before_the_pins_clean() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![big_release()]));
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Progress(4_000));
    assert_eq!(
        a.update_chip(),
        Some(0),
        "the chip did not start in the old version's socket"
    );
    assert_eq!(
        a.update_clean(),
        Some(0.0),
        "pins cleaned before the chip moved"
    );
    advance(&mut a, OPEN_MS + 900);
    assert_eq!(
        a.update_chip(),
        Some(0),
        "the chip moved before the open cart had sat for a second"
    );
    advance(&mut a, 100 + HOP_MS + 100);
    assert_eq!(
        a.update_chip(),
        Some(1),
        "the chip did not land in the new version's socket"
    );
    assert_eq!(a.update_clean(), Some(0.5));
}

#[test]
fn the_sockets_are_labelled_old_then_new() {
    let (_d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Available(vec![release()]));
    let running = Build::current().version;
    assert_eq!(
        a.update_versions(),
        Some((format!("v{running}"), "v99.0.0".to_string()))
    );
}

fn baseos() -> Release {
    Release {
        kind: Kind::BaseOs,
        running: "1.3.0".into(),
        version: "1.4.0".into(),
        url: "https://example.invalid/baseos-rgsp-1.4.0.bosupd".into(),
        sha256: [0; 32],
        size: 8_000,
    }
}

fn offered_both() -> (TempDir, App, FakeJobs) {
    let (d, mut a, jobs) = on_updates();
    press(&mut a, Btn::A);
    jobs.send(
        &mut a,
        UpdateEvent::Available(vec![big_release(), baseos()]),
    );
    (d, a, jobs)
}

#[test]
fn two_offers_start_on_slot_and_right_picks_baseos() {
    let (_d, mut a, _jobs) = offered_both();
    assert_eq!(a.update_chosen(), Some(Kind::Slot));
    press(&mut a, Btn::Right);
    assert_eq!(a.update_chosen(), Some(Kind::BaseOs));
    press(&mut a, Btn::Right);
    assert_eq!(a.update_chosen(), Some(Kind::BaseOs));
    press(&mut a, Btn::Left);
    assert_eq!(a.update_chosen(), Some(Kind::Slot));
}

#[test]
fn a_installs_only_the_chosen_cart() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert_eq!(
        jobs.asked(),
        [UpdateJob::Check, UpdateJob::Install(baseos())]
    );
}

fn low_battery(d: &TempDir, a: &mut App) {
    let (power, _) =
        common::panel_with_battery(d.path(), std::time::Duration::from_secs(300), 1, 10);
    a.set_power(power);
    advance(a, 6_000);
    assert_eq!(a.battery().map(|b| b.percent), Some(10));
}

#[test]
fn a_low_battery_warns_before_a_baseos_install() {
    let (d, mut a, jobs) = offered_both();
    low_battery(&d, &mut a);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
    assert_eq!(text(&a).unwrap().0, "Battery is low");
    press(&mut a, Btn::A);
    assert_eq!(
        jobs.asked(),
        [UpdateJob::Check, UpdateJob::Install(baseos())]
    );
}

#[test]
fn b_on_the_battery_warning_installs_nothing() {
    let (d, mut a, jobs) = offered_both();
    low_battery(&d, &mut a);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    press(&mut a, Btn::B);
    assert!(on_updates_row(&a));
    assert_eq!(jobs.asked(), [UpdateJob::Check]);
}

#[test]
fn a_low_battery_does_not_warn_for_slot() {
    let (d, mut a, jobs) = offered_both();
    low_battery(&d, &mut a);
    press(&mut a, Btn::A);
    assert_eq!(
        jobs.asked(),
        [UpdateJob::Check, UpdateJob::Install(big_release())]
    );
}

#[test]
fn a_staged_baseos_update_restarts_instead_of_relaunching() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Staged);
    for _ in 0..80 {
        a.tick_ms(a.now() + 50);
        a.update(0.05);
    }
    assert!(a.restarting(), "a BaseOS update must reboot");
    assert!(!a.relaunching());
}

#[test]
fn a_second_check_offers_both_again_from_slot() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::B);
    press(&mut a, Btn::A);
    jobs.send(
        &mut a,
        UpdateEvent::Available(vec![big_release(), baseos()]),
    );
    assert_eq!(a.update_chosen(), Some(Kind::Slot));
    assert_eq!(a.update_text().unwrap().0, "Updates are available");
}

#[test]
fn the_sockets_name_the_running_baseos() {
    let (_d, mut a, _jobs) = offered_both();
    press(&mut a, Btn::Right);
    assert_eq!(
        a.update_versions(),
        Some(("v1.3.0".to_string(), "v1.4.0".to_string()))
    );
}

#[test]
fn a_refused_baseos_update_shows_its_reason() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    jobs.send(
        &mut a,
        UpdateEvent::Failed(UpdateFail::Rejected("geometry does not fit".into())),
    );
    assert_eq!(
        text(&a),
        Some((
            "BaseOS refused the update".to_string(),
            "geometry does not fit".to_string()
        ))
    );
}

#[test]
fn b_while_downloading_cancels_and_goes_back_to_the_carts() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    jobs.send(&mut a, UpdateEvent::Progress(4_000));
    advance(&mut a, OPEN_MS + 100);
    press(&mut a, Btn::B);
    assert_eq!(jobs.cancels(), 1);
    assert!(a.update_shown());
    advance(&mut a, CLOSE_MS + 100);
    assert_eq!(text(&a).unwrap().0, "Updates are available");
    assert_eq!(a.update_chosen(), Some(Kind::BaseOs));
    assert_eq!(
        jobs.asked(),
        [UpdateJob::Check, UpdateJob::Install(baseos())]
    );
}

#[test]
fn a_cancelled_download_never_shows_a_failure() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::A);
    advance(&mut a, OPEN_MS + 100);
    press(&mut a, Btn::B);
    jobs.send(&mut a, UpdateEvent::Cancelled);
    advance(&mut a, CLOSE_MS + 100);
    assert_eq!(text(&a).unwrap().0, "Updates are available");
    assert_eq!(a.update_chosen(), Some(Kind::Slot));
}

#[test]
fn b_after_the_download_is_committed_keeps_installing() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::A);
    advance(&mut a, OPEN_MS + 100);
    jobs.commit();
    press(&mut a, Btn::B);
    assert_eq!(jobs.cancels(), 1);
    advance(&mut a, CLOSE_MS + 100);
    assert!(a.update_progress().is_some());
    jobs.send(&mut a, UpdateEvent::Staged);
    assert_eq!(a.update_progress(), Some(1.0));
}

#[test]
fn a_second_install_after_a_cancel_starts_a_fresh_download() {
    let (_d, mut a, jobs) = offered_both();
    press(&mut a, Btn::A);
    advance(&mut a, OPEN_MS + 100);
    press(&mut a, Btn::B);
    advance(&mut a, CLOSE_MS + 100);
    press(&mut a, Btn::A);
    assert_eq!(a.update_progress(), Some(0.0));
    jobs.send(&mut a, UpdateEvent::Cancelled);
    assert_eq!(a.update_progress(), Some(0.0));
}

#[test]
fn a_wifi_drop_mid_check_keeps_the_check_state() {
    let (_d, mut a, _jobs) = on_updates();
    press(&mut a, Btn::A);
    assert_eq!(
        a.update_text(),
        Some(("Checking for updates".into(), String::new()))
    );
    a.wifi_send(slot::wifi::Cmd::Disconnect);
    for _ in 0..3 {
        a.tick_ms(a.now() + 1_000);
    }
    assert!(!a.wifi_up());
    assert_eq!(a.quick_menu(), Some(QuickRow::CheckUpdates));
    assert_eq!(
        a.update_text(),
        Some(("Checking for updates".into(), String::new()))
    );
}
