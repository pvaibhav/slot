mod common;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{app_playing_with, boot, tmp_root_with_carts, StubSnapshot, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::session::Session;
use slot_input::{Action, Btn, RawEvent};
use slot_power::{Battery, Charge, LedState, Power, RepairResult};
use slot_ui::Toast;

struct Card {
    root: PathBuf,
    read_only: Arc<AtomicBool>,
    result: Option<RepairResult>,
    battery: Option<Battery>,
}

impl slot_power::Platform for Card {
    fn set_backlight(&mut self, _step: u8) {}
    fn battery(&self) -> Option<Battery> {
        self.battery
    }
    fn charge(&self) -> Charge {
        self.battery.map_or(Charge::Discharging, |b| b.charge)
    }
    fn set_led(&mut self, _state: LedState) {}
    fn poweroff(&mut self) -> ! {
        panic!("the test card never powers off")
    }
    fn restart(&mut self) -> ! {
        panic!("the test card never restarts")
    }
    fn root(&self) -> &Path {
        &self.root
    }
    fn now(&self) -> i64 {
        CLOCK_IS_SET
    }
    fn set_clock(&mut self, _secs: i64) {}
    fn set_rumble(&mut self, _strength: u16) {}
    fn card_read_only(&self) -> bool {
        self.read_only.load(Ordering::Relaxed)
    }
    fn take_repair_result(&mut self) -> Option<RepairResult> {
        self.result.take()
    }
}

fn power(root: &Path, result: Option<RepairResult>) -> (Power, Arc<AtomicBool>) {
    powered(root, result, None)
}

fn powered(
    root: &Path,
    result: Option<RepairResult>,
    battery: Option<Battery>,
) -> (Power, Arc<AtomicBool>) {
    let read_only = Arc::new(AtomicBool::new(false));
    let card = Card {
        root: root.to_path_buf(),
        read_only: read_only.clone(),
        result,
        battery,
    };
    (
        Power::new(Box::new(card), Duration::from_secs(300)),
        read_only,
    )
}

fn shelf(root: &Path) -> (App, Arc<AtomicBool>) {
    let mut a = boot(root);
    let (p, read_only) = power(root, None);
    a.set_power(p);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    (a, read_only)
}

fn read_only_now(a: &mut App, read_only: &AtomicBool) {
    read_only.store(true, Ordering::Relaxed);
    a.tick_ms(3_000);
    assert!(a.card_alarm());
}

#[test]
fn a_read_only_card_on_the_shelf_offers_a_repair() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let (mut a, read_only) = shelf(d.path());
    read_only_now(&mut a, &read_only);
    assert_eq!(a.alarm_toast(), Some(Toast::CardRepair));
    a.apply(Action::RepairCard);
    assert!(a.repair_prompt());
}

#[test]
fn a_low_battery_holds_the_repair_back_until_it_charges() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    for (charge, offered) in [(Charge::Discharging, false), (Charge::Charging, true)] {
        let mut a = boot(d.path());
        let battery = Battery {
            percent: 10,
            charge,
        };
        let (p, read_only) = powered(d.path(), None, Some(battery));
        a.set_power(p);
        read_only.store(true, Ordering::Relaxed);
        a.tick_ms(10_000);
        a.tick_ms(12_100);
        let toast = if offered {
            Toast::CardRepair
        } else {
            Toast::CardRepairCharge
        };
        assert_eq!(a.alarm_toast(), Some(toast), "{charge:?}");
        a.apply(Action::RepairCard);
        assert_eq!(a.repair_prompt(), offered, "{charge:?}");
    }
}

#[test]
fn not_now_closes_the_prompt_and_keeps_the_banner() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let (mut a, read_only) = shelf(d.path());
    read_only_now(&mut a, &read_only);
    a.apply(Action::RepairCard);
    a.apply(Action::GbaDown(Btn::B));
    assert!(!a.repair_prompt());
    assert!(!a.shutting_down());
    assert_eq!(a.alarm_toast(), Some(Toast::CardRepair));
}

#[test]
fn the_prompt_holds_the_shelf_still() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let (mut a, read_only) = shelf(d.path());
    read_only_now(&mut a, &read_only);
    let before = a.selected_stem().map(str::to_owned);
    a.apply(Action::RepairCard);
    a.apply(Action::ShelfRight);
    a.apply(Action::Insert);
    assert!(a.repair_prompt());
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(a.selected_stem().map(str::to_owned), before);
}

#[test]
fn repair_shows_its_screen_then_hands_over() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let (mut a, read_only) = shelf(d.path());
    read_only_now(&mut a, &read_only);
    a.apply(Action::RepairCard);
    a.apply(Action::GbaDown(Btn::A));
    assert!(!a.repair_prompt());
    assert!(a.shutting_down());
    assert!(
        !a.ready_to_repair(),
        "handed over before the screen said so"
    );
    a.tick_ms(3_400);
    assert!(a.ready_to_repair());
    assert!(!a.ready_to_restart());
}

#[test]
fn a_failed_write_alone_offers_no_repair() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let (mut a, _) = shelf(d.path());
    std::fs::remove_dir_all(d.path().join("Config")).ok();
    std::fs::write(d.path().join("Config"), b"").unwrap();
    a.apply(Action::Favorite);
    a.tick_ms(3_000);
    assert!(a.card_alarm());
    assert_eq!(a.alarm_toast(), Some(Toast::CardUnwritable));
    a.apply(Action::RepairCard);
    assert!(!a.repair_prompt());
}

#[test]
fn a_game_offers_the_repair_too() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with(d.path(), "Emerald", StubSnapshot::boxed());
    let (p, read_only) = power(d.path(), None);
    a.set_power(p);
    read_only_now(&mut a, &read_only);
    assert_eq!(a.alarm_toast(), Some(Toast::CardRepair));
    a.apply(Action::RepairCard);
    assert!(a.repair_prompt());
    a.apply(Action::GbaDown(Btn::A));
    a.tick_ms(3_400);
    assert!(a.ready_to_repair());
}

#[test]
fn the_boot_after_a_repair_says_how_it_went() {
    for (result, toast) in [
        (RepairResult::Repaired, Toast::CardRepaired),
        (RepairResult::Clean, Toast::CardRepaired),
        (RepairResult::Failed, Toast::CardRepairFailed),
    ] {
        let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
        let mut a = boot(d.path());
        let (p, _) = power(d.path(), Some(result));
        a.set_power(p);
        assert_eq!(a.toast(), Some(toast), "{result:?}");
    }
}

fn press(s: &mut Session, at: u64, b: Btn) {
    s.feed([RawEvent::Down(b)], at);
}

#[test]
fn select_and_start_together_open_the_repair() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    let (p, read_only) = power(d.path(), None);
    s.app_mut().set_power(p);
    read_only_now(s.app_mut(), &read_only);
    press(&mut s, 3_000, Btn::Select);
    press(&mut s, 3_050, Btn::Start);
    assert!(s.app().repair_prompt());
}

#[test]
fn a_healthy_card_leaves_select_and_start_alone() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    let (p, _) = power(d.path(), None);
    s.app_mut().set_power(p);
    s.app_mut().tick_ms(3_000);
    press(&mut s, 3_000, Btn::Select);
    press(&mut s, 3_050, Btn::Start);
    assert!(!s.app().repair_prompt());
}

#[test]
fn the_prompt_pauses_the_game_and_not_now_runs_it_again() {
    use slot::emu::Speed;
    use std::time::Instant;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    common::write_retail_header(&d, "Emerald", "POKEMON RUBY", "AXVE");
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut s = Session::boot(d.path().to_path_buf());
    let (p, read_only) = power(d.path(), None);
    s.app_mut().set_power(p);
    let mut now = 0;
    let step = |s: &mut Session, now: &mut u64, events: &[RawEvent]| {
        *now += 16;
        s.feed(events.iter().copied(), *now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    };
    let runs_at = |s: &mut Session, now: &mut u64, want: Speed| {
        let deadline = Instant::now() + Duration::from_secs(5);
        while s.observed_speed() != Some(want) {
            if Instant::now() >= deadline {
                return false;
            }
            step(s, now, &[]);
        }
        true
    };
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    read_only.store(true, Ordering::Relaxed);
    s.app_mut().tick_ms(now + 2_100);
    now += 2_100;
    s.feed([RawEvent::Down(Btn::Select)], now);
    s.feed([RawEvent::Down(Btn::Start)], now + 20);
    now += 20;
    assert!(s.app().repair_prompt());
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "the game ran on behind the prompt"
    );
    s.feed(
        [RawEvent::Up(Btn::Start), RawEvent::Up(Btn::Select)],
        now + 16,
    );
    s.feed([RawEvent::Down(Btn::B)], now + 32);
    now += 32;
    assert!(!s.app().repair_prompt());
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "not now left the game paused"
    );
}

#[test]
fn a_locked_kid_mode_card_shows_the_plain_toast_and_no_repair() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            kid_mode: slot_store::KidMode::Shelves,
            ..slot_store::SlotState::default()
        },
    )
    .unwrap();
    let (mut a, read_only) = shelf(d.path());
    read_only_now(&mut a, &read_only);
    assert_eq!(a.alarm_toast(), Some(Toast::CardUnwritable));
    assert!(!a.repair_offered());
    a.apply(Action::RepairCard);
    assert!(!a.repair_prompt());
}
