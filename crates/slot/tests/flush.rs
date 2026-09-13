mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use common::{app_playing_in, app_playing_with, tmp_root_with_carts};
use slot::app::Phase;
use slot::persist;
use slot::persist::Snapshot;
use slot_input::Action;
use slot_power::{Battery, Charge, LedState};
use slot_store::{read_slot_state, Core, Platform, StateRing};

fn unknown(percent: u8) -> Battery {
    Battery {
        percent,
        charge: Charge::Unknown,
    }
}

#[test]
fn power_press_edge_flushes_immediately_not_on_release() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerTap);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert!(
        r.read_resume().unwrap().is_some(),
        "flush must happen on press, a held power button is a hardware cutoff"
    );
}

#[test]
fn autosave_fires_at_sixty_seconds_and_not_before() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.tick_ms(59_999);
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none()
    );
    a.tick_ms(60_000);
    a.settle_saves();
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some()
    );
}

#[test]
fn battery_critical_flushes_and_powers_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(unknown(3));
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some()
    );
    assert!(a.powering_off());
}

#[test]
fn a_power_tap_dozes_and_a_second_one_wakes() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Doze { .. }));
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn holding_power_flushes_and_powers_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");

    a.apply(Action::PowerHold);
    assert!(
        a.powering_off(),
        "the hold begins shutdown without another button"
    );
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some(),
        "durable before shutdown"
    );

    assert_eq!(
        read_slot_state(d.path()).cart,
        Some("Emerald".into()),
        "power off is not an eject"
    );
}

/// Releasing a held button leaves the shutdown already in progress.
#[test]
fn a_release_after_the_hold_does_nothing_on_its_own() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerHold);
    a.apply(Action::PowerOff);
    assert!(a.powering_off());
}

#[test]
fn an_idle_doze_times_out_into_a_power_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Doze { .. }));
    a.on_doze_timeout();
    assert!(a.powering_off(), "the timeout powers off");
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some(),
        "and the tap that started the doze already made it durable"
    );
}

#[test]
fn a_battery_above_the_threshold_keeps_playing() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(unknown(20));
    assert!(!a.powering_off());
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_autosave_repeats_rather_than_firing_once() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (snapshot, flushes) = counting();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.tick_ms(60_000);
    assert_eq!(flushes.load(Ordering::Relaxed), 1);
    a.tick_ms(119_999);
    assert_eq!(flushes.load(Ordering::Relaxed), 1, "one write per minute");
    a.tick_ms(120_000);
    assert_eq!(flushes.load(Ordering::Relaxed), 2);
}

#[test]
fn a_flush_resets_the_autosave_clock() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (snapshot, flushes) = counting();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.tick_ms(30_000);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert_eq!(flushes.load(Ordering::Relaxed), 1);
    a.tick_ms(61_000);
    assert_eq!(
        flushes.load(Ordering::Relaxed),
        1,
        "the lid wrote 31 s ago, not 61"
    );
    a.tick_ms(90_000);
    assert_eq!(flushes.load(Ordering::Relaxed), 2);
}

fn counting() -> (Box<dyn Snapshot>, Arc<AtomicUsize>) {
    let flushes = Arc::new(AtomicUsize::new(0));
    (
        Box::new(CountingSnapshot {
            flushes: flushes.clone(),
        }),
        flushes,
    )
}

struct CountingSnapshot {
    flushes: Arc<AtomicUsize>,
}

impl Snapshot for CountingSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        Some(vec![self.flushes.fetch_add(1, Ordering::Relaxed) as u8; 64])
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        None
    }

    fn load(&self, _state: Vec<u8>) {}
}

fn at(percent: u8, charge: Charge) -> Battery {
    Battery { percent, charge }
}

#[test]
fn one_stem_on_two_platforms_writes_two_saves() {
    let d = tempfile::tempdir().unwrap();
    persist::write_sav(d.path(), Platform::Gba, "Tetris", &[1u8; 32]).unwrap();
    persist::write_sav(d.path(), Platform::Gb, "Tetris", &[2u8; 8]).unwrap();

    assert_eq!(
        persist::read_sav(d.path(), Platform::Gba, "Tetris").unwrap(),
        vec![1u8; 32]
    );
    assert_eq!(
        persist::read_sav(d.path(), Platform::Gb, "Tetris").unwrap(),
        vec![2u8; 8]
    );
    assert!(d.path().join("Saves/GBA/Tetris.sav").is_file());
    assert!(d.path().join("Saves/GB/Tetris.sav").is_file());
}

#[test]
fn a_critical_battery_on_a_charger_keeps_running() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(at(3, Charge::Charging));
    assert!(
        !a.powering_off(),
        "the cable is in, there is nothing to save from"
    );
}

#[test]
fn a_critical_battery_reading_full_keeps_running() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(at(3, Charge::Full));
    assert!(!a.powering_off());
}

#[test]
fn a_critical_battery_that_is_discharging_still_powers_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(at(3, Charge::Discharging));
    assert!(a.powering_off());
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some()
    );
}

#[test]
fn an_unknown_charge_state_powers_off_exactly_as_before() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_battery(at(3, Charge::Unknown));
    assert!(a.powering_off());
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some()
    );
}

#[test]
fn the_battery_is_read_the_moment_power_is_attached_not_ten_seconds_later() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, _charge, _percent) = common::app_playing_with_charge(d.path(), "Emerald");
    a.tick_ms(1);
    assert!(
        a.battery().is_some(),
        "the shelf is still blank a moment after power was attached"
    );
}

#[test]
fn the_charge_state_is_picked_up_inside_a_second() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, _percent) = common::app_playing_with_charge(d.path(), "Emerald");
    a.tick_ms(10_000);
    assert_eq!(a.battery().map(|b| b.charge), Some(Charge::Discharging));
    charge.store(2, Ordering::Relaxed);
    a.tick_ms(11_000);
    assert_eq!(
        a.battery().map(|b| b.charge),
        Some(Charge::Charging),
        "one second after the cable went in, not ten"
    );
}

#[test]
fn the_percent_survives_a_fast_tick_that_only_moved_the_charge_state() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = common::app_playing_with_charge(d.path(), "Emerald");
    a.tick_ms(10_000);
    assert_eq!(
        a.battery().map(|b| b.percent),
        Some(50),
        "percent as the slow tick cached it"
    );
    percent.store(9, Ordering::Relaxed);
    charge.store(2, Ordering::Relaxed);
    a.tick_ms(11_000);
    assert_eq!(
        a.battery().map(|b| b.percent),
        Some(50),
        "the fast tick must not have re-read a percent that moved after the slow tick cached it"
    );
}

#[test]
fn charging_outranks_low_so_a_flat_device_on_a_cable_is_not_red() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = common::app_playing_with_charge(d.path(), "Emerald");
    charge.store(2, Ordering::Relaxed);
    percent.store(3, Ordering::Relaxed);
    a.tick_ms(10_000);
    assert_eq!(a.led_state(), LedState::Charging);
}

#[test]
fn an_autosave_lands_once_the_writes_settle() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.tick_ms(60_000);
    a.settle_saves();
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_lid_close_right_after_an_autosave_leaves_the_lids_state() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (snapshot, _) = counting();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);
    a.tick_ms(60_000);
    a.apply(Action::LidClose);
    a.settle_saves();
    let state = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .read_resume()
        .unwrap()
        .expect("no resume written");
    assert_eq!(
        state,
        vec![1u8; 64],
        "the autosave landed over the lid's save"
    );
}
