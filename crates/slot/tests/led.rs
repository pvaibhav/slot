mod common;

use std::sync::atomic::Ordering;

use common::{
    app_playing_in, app_playing_with_charge, app_playing_with_led, led_code, tmp_root_with_carts,
};
use slot_input::{Action, Btn};
use slot_power::LedState;

#[test]
fn the_fast_tick_actually_reaches_the_platforms_set_led() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent, led, _writes) = app_playing_with_led(d.path(), "Emerald");
    charge.store(2, Ordering::Relaxed);
    percent.store(50, Ordering::Relaxed);
    a.tick_ms(5_000);
    assert_eq!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Charging),
        "led_state computed a value the platform never heard about"
    );
}

#[test]
fn a_full_battery_reads_charged_not_running() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = app_playing_with_charge(d.path(), "Emerald");
    charge.store(3, Ordering::Relaxed);
    percent.store(100, Ordering::Relaxed);
    a.tick_ms(10_000);
    assert_eq!(a.led_state(), LedState::Charged);
}

#[test]
fn a_flat_battery_that_is_not_charging_reads_low() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = app_playing_with_charge(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(10, Ordering::Relaxed);
    a.tick_ms(10_000);
    assert_eq!(a.led_state(), LedState::Low);
}

#[test]
fn the_low_threshold_is_twenty_percent_not_just_a_number_comfortably_below_it() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = app_playing_with_charge(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);

    percent.store(20, Ordering::Relaxed);
    a.tick_ms(10_000);
    assert_eq!(
        a.led_state(),
        LedState::Low,
        "20% is still at the threshold"
    );

    percent.store(21, Ordering::Relaxed);
    a.tick_ms(20_000);
    assert_eq!(
        a.led_state(),
        LedState::Running,
        "21% is one point above the threshold"
    );
}

#[test]
fn no_reading_yet_reads_running_not_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.led_state(), LedState::Running);
}

#[test]
fn the_led_is_written_once_per_change_not_once_per_tick() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent, _led, writes) = app_playing_with_led(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(50, Ordering::Relaxed);
    a.tick_ms(2_000);
    let after_first = writes.load(Ordering::Relaxed);
    assert!(
        after_first > 0,
        "the very first tick never reached the platform at all"
    );
    for extra_s in 1..=10 {
        a.tick_ms(2_000 + extra_s * 1_000);
    }
    assert_eq!(
        writes.load(Ordering::Relaxed),
        after_first,
        "an unchanged LED state kept writing to the platform anyway"
    );
}

#[test]
fn power_off_leaves_the_led_off_rather_than_lit_through_shutdown() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent, led, _writes) = app_playing_with_led(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(50, Ordering::Relaxed);
    a.tick_ms(2_000);
    assert_ne!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the rig should start lit, or this test proves nothing"
    );
    a.apply(Action::PowerHold);
    assert_eq!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the LED was still reporting a running state after power_off"
    );
}

#[test]
fn a_doze_timeout_also_leaves_the_led_off_rather_than_lit_through_shutdown() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent, led, _writes) = app_playing_with_led(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(50, Ordering::Relaxed);
    a.tick_ms(2_000);
    assert_ne!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the rig should start lit, or this test proves nothing"
    );
    a.apply(Action::LidClose);
    a.on_doze_timeout();
    assert_eq!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the LED was still reporting a running state after the doze timeout"
    );
}

#[test]
fn the_slow_tick_actually_runs_the_power_off_policy_on_what_it_reads() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent) = app_playing_with_charge(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(3, Ordering::Relaxed);
    a.tick_ms(10_000);
    assert!(
        a.powering_off(),
        "the slow tick read a critical battery but never ran the power-off policy on it"
    );
}

#[test]
fn the_fast_tick_does_not_relight_the_case_through_a_shutdown() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, charge, percent, led, _writes) = app_playing_with_led(d.path(), "Emerald");
    charge.store(1, Ordering::Relaxed);
    percent.store(50, Ordering::Relaxed);
    a.tick_ms(2_000);

    a.apply(Action::PowerHold);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the choice darkens the case, or this test proves nothing"
    );

    a.tick_ms(3_000);
    assert_eq!(
        led.load(Ordering::Relaxed),
        led_code(LedState::Off),
        "the fast tick re-lit the case light on a device that is shutting down"
    );
}
