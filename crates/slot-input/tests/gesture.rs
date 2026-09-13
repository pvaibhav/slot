use slot_input::RawEvent::{Down, Up};
use slot_input::{
    Action::*, Btn, Btn::*, Gestures, RawEvent, MENU_HOLD_MS, POWER_HOLD_MS, SELECT_CHORD_MS,
    SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};

#[test]
fn a_select_that_becomes_a_chord_never_reaches_the_game() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Btn::Up), 120), vec![BrightnessUp]);
    assert!(g.feed(Up(Btn::Up), 160).is_empty());
    assert!(g.feed(Up(Select), 900).is_empty());
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_chord_keeps_both_keys_from_the_game() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(R1), 50), vec![SaveState]);
    assert!(
        g.feed(Up(R1), 60).is_empty(),
        "the chord key reached the game"
    );
    assert!(
        g.feed(Up(Select), 70).is_empty(),
        "the chord's SELECT reached the game"
    );
}

#[test]
fn a_select_held_past_the_window_reaches_the_game_then() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(g.tick(SELECT_CHORD_MS - 1).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(g.tick(SELECT_CHORD_MS + 100).is_empty());
    assert_eq!(g.feed(Up(Select), 2_000), vec![GbaUp(Select)]);
}

#[test]
fn a_game_button_under_select_hands_select_over_first() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(A), 100), vec![GbaDown(Select), GbaDown(A)]);
    assert_eq!(g.feed(Up(A), 200), vec![GbaUp(A)]);
    assert!(
        g.tick(SELECT_CHORD_MS).is_empty(),
        "SELECT was pressed twice"
    );
    assert_eq!(g.feed(Up(Select), 900), vec![GbaUp(Select)]);
}

#[test]
fn a_held_select_keeps_chording_long_past_the_window() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Btn::Up), 100), vec![BrightnessUp]);
    g.feed(Up(Btn::Up), 150);
    assert_eq!(
        g.feed(Down(Btn::Up), SELECT_CHORD_MS + 500),
        vec![BrightnessUp],
        "the second nudge of the brightness reached the game as a direction"
    );
}

#[test]
fn select_chords_map_to_all_four_axes() {
    for (btn, want) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Right, BlueLightUp),
        (Left, BlueLightDown),
    ] {
        let mut g = Gestures::new();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

#[test]
fn a_select_tap_reaches_the_game_on_its_release() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select)]);
    assert!(g.tick(80 + SELECT_TAP_MS - 1).is_empty());
    assert_eq!(g.tick(80 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_select_press_inside_the_tap_window_hands_back_the_release_it_interrupted() {
    let mut g = Gestures::new();
    let mut log = Vec::new();
    log.extend(g.feed(Down(Select), 0));
    log.extend(g.feed(Up(Select), 20));
    log.extend(g.feed(Down(Select), 20 + SELECT_TAP_MS - 1));
    log.extend(g.feed(Down(Btn::Up), 100));
    log.extend(g.feed(Up(Btn::Up), 140));
    log.extend(g.feed(Up(Select), 200));
    for t in 200..2_000 {
        log.extend(g.tick(t));
    }
    let downs = log.iter().filter(|a| **a == GbaDown(Select)).count();
    let ups = log.iter().filter(|a| **a == GbaUp(Select)).count();
    assert_eq!(
        (downs, ups),
        (1, 1),
        "the core was handed {downs} SELECT press(es) and {ups} release(s): {log:?}"
    );
    assert_eq!(
        log.iter()
            .rfind(|a| matches!(a, GbaDown(Select) | GbaUp(Select))),
        Some(&GbaUp(Select)),
        "the pad was left holding SELECT: {log:?}"
    );
    assert!(
        log.contains(&BrightnessUp),
        "the chord off the second press stopped working: {log:?}"
    );
}

#[test]
fn menu_single_tap_opens_the_quick_menu() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

#[test]
fn a_menu_hold_is_not_also_a_tap() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 50).is_empty());
}

#[test]
fn menu_double_tap_opens_polaroids_immediately() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_hold_ejects_at_the_hold_time_and_not_before() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert!(g.tick(MENU_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
}

#[test]
fn menu_hold_released_early_ejects_nothing() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), MENU_HOLD_MS - 200);
    assert!(g.tick(3000).is_empty());
}

#[test]
fn r2_hold_is_momentary() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 900), vec![FfStop]);
}

#[test]
fn r2_double_tap_latches_and_single_press_clears() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    assert!(g.feed(Up(R2), 150).is_empty());
    g.feed(Down(R2), 5000);
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
}

#[test]
fn power_flushes_on_the_press_and_locks_on_the_release() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Btn::Power), 0), vec![PowerPress]);
    assert_eq!(g.feed(Up(Btn::Power), 80), vec![PowerTap]);
}

/// The hold fires once while POWER is down, without waiting for release.
#[test]
fn power_held_past_the_threshold_requests_shutdown_once() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(POWER_HOLD_MS), vec![PowerHold]);
    assert!(g.tick(4000).is_empty(), "the hold fires once, not per tick");
    assert_eq!(g.feed(Up(Btn::Power), 4500), vec![PowerOff]);
}

#[test]
fn a_press_just_short_of_the_threshold_is_a_lock() {
    let mut g = Gestures::new();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.feed(Up(Btn::Power), POWER_HOLD_MS - 1), vec![PowerTap]);
}

#[test]
fn a_press_the_rewind_turned_away_is_not_half_of_a_latch() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert!(
        g.feed(Down(R2), 50).is_empty(),
        "the rewind let a fast forward start under it"
    );
    assert!(g.feed(Up(R2), 100).is_empty());
    assert_eq!(g.feed(Up(L2), 150), vec![RewindStop]);
    assert_eq!(g.feed(Down(R2), 200), vec![FfStart]);
    assert!(!g.ff_latched(), "a single press latched fast forward");
    assert_eq!(
        g.feed(Up(R2), 400),
        vec![FfStop],
        "the finger came off R2 and the speed stayed"
    );
}

#[test]
fn rewind_beats_latched_fast_forward() {
    let mut g = Gestures::new();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    g.feed(Up(R2), 150);
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

#[test]
fn past_the_window_select_and_the_next_key_are_the_games() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(
        g.tick(400).is_empty(),
        "SELECT was handed over inside its window"
    );
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Down(Btn::Up), SELECT_CHORD_MS + 1),
        vec![GbaDown(Btn::Up)]
    );
}

#[test]
fn a_chord_landing_late_is_still_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    g.tick(400);
    assert_eq!(g.feed(Down(R1), 400), vec![SaveState]);
    assert!(
        g.tick(5_000).is_empty(),
        "the tick handed something over behind a late chord"
    );
}

#[test]
fn a_select_tap_too_short_to_be_polled_is_held_on_to() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 20), vec![GbaDown(Select)]);
    assert!(g.tick(20 + SELECT_TAP_MS - 1).is_empty());
    assert_eq!(g.tick(20 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(
        g.tick(5_000).is_empty(),
        "the release was handed over twice"
    );
}

#[test]
fn a_select_the_game_has_seen_ends_on_its_own_release() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert_eq!(
        g.feed(Up(Select), SELECT_CHORD_MS + SELECT_TAP_MS),
        vec![GbaUp(Select)]
    );
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn both_volume_keys_together_mute_once() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 80);
    assert!(out.contains(&MuteToggle), "the pair did not mute");
    assert!(
        g.tick(400).is_empty(),
        "it kept firing while both were held"
    );
}

#[test]
fn volume_keys_far_apart_are_not_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 400);
    assert!(!out.contains(&MuteToggle), "two separate presses muted");
}

#[test]
fn the_chord_arrives_behind_the_press_that_completed_it() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert_eq!(g.feed(Down(VolDown), 80), vec![VolumeDown, MuteToggle]);
}

#[test]
fn releasing_both_rearms_the_chord() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.feed(Down(VolDown), 80);
    g.feed(Up(VolUp), 200);
    g.feed(Up(VolDown), 220);
    g.feed(Down(VolUp), 1_000);
    assert!(g.feed(Down(VolDown), 1_050).contains(&MuteToggle));
}

#[test]
fn a_held_volume_key_repeats() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(VolUp), 0), vec![VolumeUp]);
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "it repeated before the ramp was due"
    );
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
    assert!(g.tick(VOLUME_REPEAT_DELAY_MS + 1).is_empty());
    assert_eq!(
        g.tick(VOLUME_REPEAT_DELAY_MS + VOLUME_REPEAT_MS),
        vec![VolumeUp]
    );
}

#[test]
fn a_released_volume_key_stops_repeating() {
    let mut g = Gestures::new();
    g.feed(Down(VolDown), 0);
    assert_eq!(g.tick(VOLUME_REPEAT_DELAY_MS), vec![VolumeDown]);
    assert!(g.feed(Up(VolDown), VOLUME_REPEAT_DELAY_MS + 10).is_empty());
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 4).is_empty(),
        "a key nobody is holding kept ramping"
    );
}

#[test]
fn the_mute_chord_does_not_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    assert!(g.feed(Down(VolDown), 50).contains(&MuteToggle));
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
        "the mute chord ramped the volume while it was held"
    );
}

#[test]
fn each_press_starts_its_own_ramp() {
    let mut g = Gestures::new();
    g.feed(Down(VolUp), 0);
    g.tick(VOLUME_REPEAT_DELAY_MS);
    g.feed(Up(VolUp), VOLUME_REPEAT_DELAY_MS + 5);
    assert_eq!(g.feed(Down(VolUp), 5_000), vec![VolumeUp]);
    assert!(
        g.tick(5_000 + VOLUME_REPEAT_DELAY_MS - 1).is_empty(),
        "the new press repeated early"
    );
    assert_eq!(g.tick(5_000 + VOLUME_REPEAT_DELAY_MS), vec![VolumeUp]);
}

#[test]
fn x_without_select_is_still_the_games_x() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(Btn::X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(Btn::X)]);
}

#[test]
fn select_and_menu_open_the_in_game_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.feed(Up(Menu), 150).is_empty(),
        "the release landed as a second gesture on top of the menu"
    );
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn a_chorded_menu_never_arms_the_eject_hold() {
    let mut g = Gestures::new();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.tick(50 + MENU_HOLD_MS + 1).is_empty(),
        "holding the menu open ejected the cart"
    );
}

#[test]
fn a_chord_after_a_recent_menu_tap_is_still_the_menu() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(
        g.feed(Down(Menu), 200),
        vec![GameMenu],
        "a recent tap turned the chord into the switcher"
    );
}

#[test]
fn menu_under_a_select_the_game_already_has_is_not_the_menu() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(
        g.feed(Down(Menu), 700).is_empty(),
        "a SELECT the game already owns still chorded"
    );
    assert_eq!(g.feed(Up(Menu), 800), vec![QuickMenu]);
}

#[test]
fn the_menu_button_still_works_after_a_chord() {
    let mut g = Gestures::new();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    g.feed(Down(Select), 150);
    assert_eq!(g.feed(Down(Menu), 200), vec![GameMenu]);
    assert!(g.feed(Up(Menu), 250).is_empty());
    assert!(g.feed(Up(Select), 260).is_empty());
    assert!(
        g.feed(Down(Menu), 400).is_empty(),
        "the tap before the chord was still standing as half of a double tap"
    );
    assert_eq!(
        g.feed(Up(Menu), 450),
        vec![QuickMenu],
        "the chord left the menu button dead"
    );
}

#[test]
fn select_and_y_toggles_colour_correction_and_costs_the_game_nothing() {
    let mut g = Gestures::new();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Y), 10), vec![ColourCorrectionToggle]);
    assert!(g.feed(Up(Y), 40).is_empty());
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn y_on_its_own_is_untouched_by_the_colour_chord() {
    let mut g = Gestures::new();
    assert_eq!(g.feed(Down(Y), 0), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
}
