use slot_input::RawEvent::{Down, Up};
use slot_input::{
    Action::*, Btn, Btn::*, Chord, Context, Gestures, Mod, RawEvent, MENU_HOLD_MS, POWER_HOLD_MS,
    SELECT_CHORD_MS, SELECT_TAP_MS, VOLUME_REPEAT_DELAY_MS, VOLUME_REPEAT_MS,
};

fn today() -> Vec<(Chord, slot_input::Action)> {
    let select = |button| Chord {
        modifier: Some(Mod::Select),
        button,
    };
    let plain = |button| Chord {
        modifier: None,
        button,
    };
    vec![
        (select(R1), SaveState),
        (select(L1), LoadState),
        (select(Btn::Up), BrightnessUp),
        (select(Btn::Down), BrightnessDown),
        (select(Btn::Left), BlueLightDown),
        (select(Btn::Right), BlueLightUp),
        (select(Y), ColourCorrectionToggle),
        (select(L2), PalettePrev),
        (select(R2), PaletteNext),
        (select(A), ShaderNext),
        (select(B), ShaderPrev),
        (plain(R2), FfStart),
        (plain(L2), RewindStart),
    ]
}

fn gestures() -> Gestures {
    let mut g = Gestures::new();
    g.set_shortcuts(today());
    g.set_playing(true);
    g
}

fn menu_display() -> Gestures {
    let mut g = gestures();
    let mut shortcuts = today();
    for (button, action) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Btn::Left, BlueLightDown),
        (Btn::Right, BlueLightUp),
    ] {
        shortcuts.retain(|(_, a)| *a != action);
        shortcuts.push((
            Chord {
                modifier: Some(Mod::Menu),
                button,
            },
            action,
        ));
    }
    g.set_shortcuts(shortcuts);
    g
}

#[test]
fn a_select_that_becomes_a_chord_never_reaches_the_game() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Btn::Up), 120), vec![BrightnessUp]);
    assert!(g.feed(Up(Btn::Up), 160).is_empty());
    assert!(g.feed(Up(Select), 900).is_empty());
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_chord_keeps_both_keys_from_the_game() {
    let mut g = gestures();
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
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(g.tick(SELECT_CHORD_MS - 1).is_empty());
    assert_eq!(g.tick(SELECT_CHORD_MS), vec![GbaDown(Select)]);
    assert!(g.tick(SELECT_CHORD_MS + 100).is_empty());
    assert_eq!(g.feed(Up(Select), 2_000), vec![GbaUp(Select)]);
}

#[test]
fn a_game_button_under_select_hands_select_over_first() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(
        g.feed(Down(Start), 100),
        vec![GbaDown(Select), GbaDown(Start)]
    );
    assert_eq!(g.feed(Up(Start), 200), vec![GbaUp(Start)]);
    assert!(
        g.tick(SELECT_CHORD_MS).is_empty(),
        "SELECT was pressed twice"
    );
    assert_eq!(g.feed(Up(Select), 900), vec![GbaUp(Select)]);
}

#[test]
fn a_held_select_keeps_chording_long_past_the_window() {
    let mut g = gestures();
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
        let mut g = gestures();
        g.feed(RawEvent::Down(Select), 0);
        assert_eq!(g.feed(RawEvent::Down(btn), 10), vec![want]);
    }
}

#[test]
fn a_select_tap_reaches_the_game_on_its_release() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select)]);
    assert!(g.tick(80 + SELECT_TAP_MS - 1).is_empty());
    assert_eq!(g.tick(80 + SELECT_TAP_MS), vec![GbaUp(Select)]);
    assert!(g.tick(5_000).is_empty());
}

#[test]
fn a_select_press_inside_the_tap_window_hands_back_the_release_it_interrupted() {
    let mut g = gestures();
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
    let mut g = gestures();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

#[test]
fn menu_single_tap_opens_the_quick_menu_with_menu_dpad_display_enabled() {
    let mut g = menu_display();
    assert!(g.feed(Down(Menu), 0).is_empty(), "acted on the press");
    assert_eq!(g.feed(Up(Menu), 100), vec![QuickMenu]);
    assert!(g.tick(451).is_empty(), "fired a second time on the timer");
}

#[test]
fn a_menu_hold_is_not_also_a_tap() {
    let mut g = gestures();
    g.feed(Down(Menu), 0);
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 50).is_empty());
}

#[test]
fn menu_double_tap_opens_polaroids_immediately() {
    let mut g = gestures();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_double_tap_opens_polaroids_immediately_with_menu_dpad_display_enabled() {
    let mut g = menu_display();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), 100);
    assert_eq!(g.feed(Down(Menu), 300), vec![Polaroids]);
}

#[test]
fn menu_hold_ejects_at_the_hold_time_and_not_before() {
    let mut g = gestures();
    g.feed(Down(Menu), 0);
    assert!(g.tick(MENU_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(MENU_HOLD_MS), vec![Eject]);
}

#[test]
fn menu_hold_released_early_ejects_nothing() {
    let mut g = gestures();
    g.feed(Down(Menu), 0);
    g.feed(Up(Menu), MENU_HOLD_MS - 200);
    assert!(g.tick(3000).is_empty());
}

#[test]
fn r2_hold_is_momentary() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Up(R2), 900), vec![FfStop]);
}

#[test]
fn r2_double_tap_latches_and_single_press_clears() {
    let mut g = gestures();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    assert!(g.feed(Up(R2), 150).is_empty());
    g.feed(Down(R2), 5000);
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
}

fn with_toggle() -> Gestures {
    let mut g = gestures();
    let mut shortcuts = today();
    shortcuts.push((
        Chord {
            modifier: None,
            button: R1,
        },
        FfToggle,
    ));
    g.set_shortcuts(shortcuts);
    g
}

#[test]
fn a_toggle_press_starts_fast_forward_and_the_next_press_stops_it() {
    let mut g = with_toggle();
    assert_eq!(g.feed(Down(R1), 0), vec![FfStart]);
    assert!(
        g.feed(Up(R1), 50).is_empty(),
        "the release stopped the toggle"
    );
    assert!(g.ff_latched(), "a toggled run is not held on like a latch");
    assert_eq!(g.feed(Down(R1), 5000), vec![FfStop]);
    assert!(g.feed(Up(R1), 5050).is_empty());
    assert!(!g.ff_latched());
}

#[test]
fn the_fast_forward_button_stops_a_toggled_run() {
    let mut g = with_toggle();
    g.feed(Down(R1), 0);
    g.feed(Up(R1), 50);
    assert!(g.feed(Down(R2), 5000).is_empty());
    assert_eq!(g.feed(Up(R2), 5050), vec![FfStop]);
    assert_eq!(
        g.feed(Down(R1), 6000),
        vec![FfStart],
        "the toggle lost its place"
    );
}

#[test]
fn the_toggle_stops_a_held_fast_forward() {
    let mut g = with_toggle();
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Down(R1), 100), vec![FfStop]);
    assert!(g.feed(Up(R1), 150).is_empty());
    assert!(
        g.feed(Up(R2), 200).is_empty(),
        "letting go of R2 reached the game"
    );
}

#[test]
fn rewind_stops_a_toggled_run() {
    let mut g = with_toggle();
    g.feed(Down(R1), 0);
    g.feed(Up(R1), 50);
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

#[test]
fn a_toggle_press_during_a_rewind_does_nothing() {
    let mut g = with_toggle();
    g.feed(Down(L2), 0);
    assert!(g.feed(Down(R1), 50).is_empty());
    assert!(g.feed(Up(R1), 100).is_empty());
    assert_eq!(g.feed(Up(L2), 150), vec![RewindStop]);
    assert!(!g.ff_latched());
}

#[test]
fn power_flushes_on_the_press_and_locks_on_the_release() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(Btn::Power), 0), vec![PowerPress]);
    assert_eq!(g.feed(Up(Btn::Power), 80), vec![PowerTap]);
}

#[test]
fn power_held_past_the_threshold_raises_the_menu_and_the_release_does_nothing() {
    let mut g = gestures();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.tick(POWER_HOLD_MS), vec![PowerHold]);
    assert!(g.tick(4000).is_empty(), "the hold fires once, not per tick");
    assert_eq!(g.feed(Up(Btn::Power), 4500), vec![PowerOff]);
}

#[test]
fn a_press_just_short_of_the_threshold_is_a_lock() {
    let mut g = gestures();
    g.feed(Down(Btn::Power), 0);
    assert!(g.tick(POWER_HOLD_MS - 1).is_empty());
    assert_eq!(g.feed(Up(Btn::Power), POWER_HOLD_MS - 1), vec![PowerTap]);
}

#[test]
fn a_press_the_rewind_turned_away_is_not_half_of_a_latch() {
    let mut g = gestures();
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
    let mut g = gestures();
    g.feed(Down(R2), 0);
    g.feed(Up(R2), 50);
    g.feed(Down(R2), 100);
    g.feed(Up(R2), 150);
    assert_eq!(g.feed(Down(L2), 200), vec![FfStop, RewindStart]);
}

#[test]
fn past_the_window_select_and_the_next_key_are_the_games() {
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
    g.feed(Down(VolUp), 0);
    let out = g.feed(Down(VolDown), 400);
    assert!(!out.contains(&MuteToggle), "two separate presses muted");
}

#[test]
fn the_chord_arrives_behind_the_press_that_completed_it() {
    let mut g = gestures();
    g.feed(Down(VolUp), 0);
    assert_eq!(g.feed(Down(VolDown), 80), vec![VolumeDown, MuteToggle]);
}

#[test]
fn releasing_both_rearms_the_chord() {
    let mut g = gestures();
    g.feed(Down(VolUp), 0);
    g.feed(Down(VolDown), 80);
    g.feed(Up(VolUp), 200);
    g.feed(Up(VolDown), 220);
    g.feed(Down(VolUp), 1_000);
    assert!(g.feed(Down(VolDown), 1_050).contains(&MuteToggle));
}

#[test]
fn a_held_volume_key_repeats() {
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
    g.feed(Down(VolUp), 0);
    assert!(g.feed(Down(VolDown), 50).contains(&MuteToggle));
    assert!(
        g.tick(VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
        "the mute chord ramped the volume while it was held"
    );
}

#[test]
fn each_press_starts_its_own_ramp() {
    let mut g = gestures();
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
    let mut g = gestures();
    assert_eq!(g.feed(Down(X), 0), vec![GbaDown(Btn::X)]);
    assert_eq!(g.feed(Up(X), 40), vec![GbaUp(Btn::X)]);
}

#[test]
fn select_and_menu_open_the_in_game_menu() {
    let mut g = gestures();
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
    let mut g = gestures();
    g.feed(Down(Select), 0);
    assert_eq!(g.feed(Down(Menu), 50), vec![GameMenu]);
    assert!(
        g.tick(50 + MENU_HOLD_MS + 1).is_empty(),
        "holding the menu open ejected the cart"
    );
}

#[test]
fn a_chord_after_a_recent_menu_tap_is_still_the_menu() {
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
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
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(Y), 10), vec![ColourCorrectionToggle]);
    assert!(g.feed(Up(Y), 40).is_empty());
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn y_on_its_own_is_untouched_by_the_colour_chord() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(Y), 0), vec![GbaDown(Y)]);
    assert_eq!(g.feed(Up(Y), 40), vec![GbaUp(Y)]);
}

#[test]
fn select_and_l2_or_r2_steps_the_palette_without_rewinding_or_fast_forwarding() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(R2), 10), vec![PaletteNext]);
    assert!(g.feed(Up(R2), 40).is_empty());
    assert_eq!(g.feed(Down(L2), 60), vec![PalettePrev]);
    assert!(g.feed(Up(L2), 90).is_empty());
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn l2_and_r2_on_their_own_still_rewind_and_fast_forward() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(L2), 0), vec![RewindStart]);
    assert_eq!(g.feed(Up(L2), 40), vec![RewindStop]);
    assert!(!g.feed(Down(R2), 60).contains(&PaletteNext));
    assert!(!g.feed(Up(R2), 90).contains(&PaletteNext));
}

#[test]
fn select_and_a_or_b_steps_the_shader_and_costs_the_game_nothing() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(A), 10), vec![ShaderNext]);
    assert!(g.feed(Up(A), 40).is_empty());
    assert_eq!(g.feed(Down(B), 60), vec![ShaderPrev]);
    assert!(g.feed(Up(B), 90).is_empty());
    assert!(g.feed(Up(Select), 200).is_empty());
}

#[test]
fn a_and_b_on_their_own_are_untouched_by_the_shader_chord() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(A), 0), vec![GbaDown(A)]);
    assert_eq!(g.feed(Up(A), 40), vec![GbaUp(A)]);
    assert_eq!(g.feed(Down(B), 60), vec![GbaDown(B)]);
    assert_eq!(g.feed(Up(B), 90), vec![GbaUp(B)]);
}

#[test]
fn x_under_select_reaches_the_game() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Down(X), 10), vec![GbaDown(Select), GbaDown(X)]);
}

#[test]
fn select_and_either_volume_key_mutes_for_a_volume_wheel_with_no_chord() {
    for vol in [VolUp, VolDown] {
        let mut g = gestures();
        assert!(g.feed(Down(Select), 0).is_empty());
        assert_eq!(g.feed(Down(vol), 100), vec![MuteToggle], "{vol:?}");
        assert!(
            g.tick(100 + VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
            "{vol:?} ramped the volume under SELECT"
        );
        assert!(g.feed(Up(vol), 900).is_empty(), "{vol:?}");
        assert!(
            g.feed(Up(Select), 950).is_empty(),
            "the mute chord's SELECT reached the game"
        );
    }
}

#[test]
fn select_and_either_volume_key_still_mutes_with_menu_dpad_display_enabled() {
    for vol in [VolUp, VolDown] {
        let mut g = menu_display();
        assert!(g.feed(Down(Select), 0).is_empty());
        assert_eq!(g.feed(Down(vol), 100), vec![MuteToggle], "{vol:?}");
        assert!(
            g.tick(100 + VOLUME_REPEAT_DELAY_MS * 3).is_empty(),
            "{vol:?} ramped the volume under SELECT"
        );
        assert!(g.feed(Up(vol), 900).is_empty(), "{vol:?}");
        assert!(
            g.feed(Up(Select), 950).is_empty(),
            "the mute chord's SELECT reached the game"
        );
    }
}

#[test]
fn menu_and_a_button_with_no_shortcut_is_a_menu_chord() {
    let mut g = gestures();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Down(Btn::Up), 100), vec![MenuChordDown(Btn::Up)]);
    assert_eq!(g.feed(Up(Btn::Up), 150), vec![MenuChordUp(Btn::Up)]);
    assert!(g.feed(Up(Menu), 200).is_empty(), "the chord's MENU tapped");
    assert!(
        g.tick(MENU_HOLD_MS + 500).is_empty(),
        "the chord's MENU ejected"
    );
}

#[test]
fn menu_and_dpad_change_the_display_when_configured() {
    for (direction, want) in [
        (Btn::Up, BrightnessUp),
        (Btn::Down, BrightnessDown),
        (Btn::Left, BlueLightDown),
        (Btn::Right, BlueLightUp),
    ] {
        let mut g = menu_display();
        assert!(g.feed(Down(Menu), 0).is_empty());
        assert_eq!(g.feed(Down(direction), 100), vec![want], "{direction:?}");
        assert!(g.feed(Up(direction), 150).is_empty(), "{direction:?}");
        assert!(g.feed(Up(Menu), 200).is_empty());
    }
}

#[test]
fn menu_dpad_display_chord_consumes_repeated_directions_and_cancels_eject() {
    let mut g = menu_display();
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Down(Btn::Up), 100), vec![BrightnessUp]);
    assert!(g.feed(Up(Btn::Up), 150).is_empty());
    assert_eq!(g.feed(Down(Btn::Right), 200), vec![BlueLightUp]);
    assert!(g.feed(Up(Btn::Right), 250).is_empty());
    assert!(
        g.tick(MENU_HOLD_MS + 500).is_empty(),
        "the display chord ejected"
    );
    assert!(g.feed(Up(Menu), MENU_HOLD_MS + 600).is_empty());
}

#[test]
fn menu_dpad_display_chord_resets_when_menu_is_released() {
    let mut g = menu_display();
    g.feed(Down(Menu), 0);
    g.feed(Down(Btn::Up), 100);
    g.feed(Up(Btn::Up), 150);
    g.feed(Up(Menu), 200);
    g.feed(Down(Menu), 300);
    assert_eq!(g.tick(300 + MENU_HOLD_MS), vec![Eject]);
}

#[test]
fn a_single_button_shortcut_waits_for_play() {
    let mut g = gestures();
    g.set_playing(false);
    assert_eq!(g.feed(Down(R2), 0), vec![GbaDown(R2)]);
    assert_eq!(g.feed(Up(R2), 50), vec![GbaUp(R2)]);
}

#[test]
fn a_single_button_shortcut_keeps_its_button_from_the_game() {
    let mut g = gestures();
    let mut shortcuts = today();
    shortcuts.retain(|(_, a)| *a != SaveState);
    shortcuts.push((
        Chord {
            modifier: None,
            button: X,
        },
        SaveState,
    ));
    g.set_shortcuts(shortcuts);
    assert_eq!(g.feed(Down(X), 0), vec![SaveState]);
    assert!(g.feed(Up(X), 50).is_empty(), "X's release reached the game");
    assert_eq!(g.feed(Down(R1), 60), vec![GbaDown(R1)]);
}

#[test]
fn fast_forward_on_a_menu_chord_lasts_while_held() {
    let mut g = gestures();
    let mut shortcuts = today();
    shortcuts.retain(|(_, a)| *a != FfStart);
    shortcuts.push((
        Chord {
            modifier: Some(Mod::Menu),
            button: R1,
        },
        FfStart,
    ));
    g.set_shortcuts(shortcuts);
    g.feed(Down(Menu), 0);
    assert_eq!(g.feed(Down(R1), 100), vec![FfStart]);
    assert_eq!(g.feed(Up(R1), 400), vec![FfStop]);
    assert!(g.feed(Up(Menu), 450).is_empty());
    assert_eq!(g.feed(Down(R2), 500), vec![GbaDown(R2)]);
}

fn paired() -> Gestures {
    let mut g = gestures();
    let mut shortcuts = today();
    shortcuts.push((
        Chord {
            modifier: Some(Mod::Button(L1)),
            button: R1,
        },
        SaveState,
    ));
    shortcuts.push((
        Chord {
            modifier: Some(Mod::Button(Start)),
            button: A,
        },
        FfStart,
    ));
    g.set_shortcuts(shortcuts);
    g
}

#[test]
fn a_pair_fires_on_its_second_button_and_the_game_keeps_the_first() {
    let mut g = paired();
    assert_eq!(g.feed(Down(L1), 0), vec![GbaDown(L1)]);
    assert_eq!(g.feed(Down(R1), 40), vec![SaveState]);
    assert!(
        g.feed(Up(R1), 80).is_empty(),
        "the second button reached the game"
    );
    assert_eq!(g.feed(Up(L1), 90), vec![GbaUp(L1)]);
}

#[test]
fn a_pair_fires_in_either_order() {
    let mut g = paired();
    assert_eq!(g.feed(Down(R1), 0), vec![GbaDown(R1)]);
    assert_eq!(g.feed(Down(L1), 40), vec![SaveState]);
    assert!(g.feed(Up(L1), 80).is_empty());
    assert_eq!(g.feed(Up(R1), 90), vec![GbaUp(R1)]);
}

#[test]
fn either_half_of_a_pair_alone_is_the_games() {
    let mut g = paired();
    assert_eq!(g.feed(Down(R1), 0), vec![GbaDown(R1)]);
    assert_eq!(g.feed(Up(R1), 40), vec![GbaUp(R1)]);
    assert_eq!(g.feed(Down(L1), 100), vec![GbaDown(L1)]);
    assert_eq!(g.feed(Up(L1), 140), vec![GbaUp(L1)]);
}

#[test]
fn a_pair_does_nothing_outside_a_game() {
    let mut g = paired();
    g.set_playing(false);
    assert_eq!(g.feed(Down(L1), 0), vec![GbaDown(L1)]);
    assert_eq!(g.feed(Down(R1), 40), vec![GbaDown(R1)]);
}

#[test]
fn a_held_pair_fast_forwards_until_its_second_button_lets_go() {
    let mut g = paired();
    assert_eq!(g.feed(Down(Start), 0), vec![GbaDown(Start)]);
    assert_eq!(g.feed(Down(A), 40), vec![FfStart]);
    assert_eq!(g.feed(Up(A), 400), vec![FfStop]);
    assert_eq!(g.feed(Up(Start), 500), vec![GbaUp(Start)]);
}

#[test]
fn select_pressed_second_is_not_a_chord() {
    let mut g = gestures();
    assert_eq!(g.feed(Down(R1), 0), vec![GbaDown(R1)]);
    assert!(
        !g.feed(Down(Select), 40).contains(&SaveState),
        "SELECT second saved a state"
    );
}

#[test]
fn start_pressed_second_is_not_a_chord() {
    let mut g = paired();
    assert_eq!(g.feed(Down(A), 0), vec![GbaDown(A)]);
    assert_eq!(g.feed(Down(Start), 40), vec![GbaDown(Start)]);
}

#[test]
fn a_button_running_its_own_shortcut_does_not_start_a_pair() {
    let mut g = paired();
    let mut shortcuts = today();
    shortcuts.push((
        Chord {
            modifier: Some(Mod::Button(L1)),
            button: R2,
        },
        SaveState,
    ));
    g.set_shortcuts(shortcuts);
    assert_eq!(g.feed(Down(R2), 0), vec![FfStart]);
    assert_eq!(g.feed(Down(L1), 40), vec![GbaDown(L1)]);
    assert_eq!(g.feed(Up(L1), 80), vec![GbaUp(L1)]);
    assert_eq!(g.feed(Up(R2), 400), vec![FfStop]);
    assert_eq!(g.feed(Down(L1), 500), vec![GbaDown(L1)]);
    assert_eq!(g.feed(Down(R2), 540), vec![SaveState]);
}

fn entry(
    modifier: Option<Mod>,
    button: Btn,
    action: slot_input::Action,
    contexts: slot_input::Contexts,
) -> slot_input::Entry {
    slot_input::Entry {
        chord: Chord { modifier, button },
        action,
        contexts,
    }
}

fn on_shelf(entries: Vec<slot_input::Entry>) -> Gestures {
    let mut g = Gestures::new();
    g.set_entries(entries);
    g.set_context(slot_input::Context::Shelf);
    g
}

#[test]
fn on_the_shelf_a_single_fires_and_the_press_still_goes_through() {
    let shelf = slot_input::Contexts::SHELF;
    let mut g = on_shelf(vec![
        entry(None, R1, BrightnessUp, shelf),
        entry(None, R1, NextShelf, shelf),
    ]);
    assert_eq!(
        g.feed(Down(R1), 0),
        vec![BrightnessUp, NextShelf, GbaDown(R1)]
    );
    assert_eq!(g.feed(Up(R1), 40), vec![GbaUp(R1)]);
}

#[test]
fn on_the_shelf_a_chords_first_button_waits_for_its_release() {
    let shelf = slot_input::Contexts::SHELF;
    let mut g = on_shelf(vec![
        entry(Some(Mod::Button(L1)), R1, BrightnessUp, shelf),
        entry(None, L1, PreviousShelf, shelf),
    ]);
    assert!(
        g.feed(Down(L1), 0).is_empty(),
        "L1 acted before R1 could join it"
    );
    assert_eq!(g.feed(Down(R1), 40), vec![BrightnessUp]);
    assert!(g.feed(Up(R1), 80).is_empty());
    assert!(
        g.feed(Up(L1), 90).is_empty(),
        "L1 switched shelves after the chord"
    );
    assert!(g.feed(Down(L1), 200).is_empty());
    assert_eq!(
        g.feed(Up(L1), 240),
        vec![PreviousShelf, GbaDown(L1), GbaUp(L1)]
    );
}

#[test]
fn a_start_chord_on_the_shelf_holds_start_back_until_it_is_let_go() {
    let all = slot_input::Contexts::ALL;
    let mut g = on_shelf(vec![entry(
        Some(Mod::Button(Start)),
        Btn::Up,
        BrightnessUp,
        all,
    )]);
    assert!(g.feed(Down(Start), 0).is_empty());
    assert_eq!(g.feed(Down(Btn::Up), 40), vec![BrightnessUp]);
    g.feed(Up(Btn::Up), 80);
    assert!(g.feed(Up(Start), 90).is_empty());
    assert!(
        g.feed(Down(Btn::Up), 200) == vec![GbaDown(Btn::Up)],
        "Up waited though it only ever comes second"
    );
    g.feed(Up(Btn::Up), 240);
    assert!(g.feed(Down(Start), 300).is_empty());
    assert_eq!(g.feed(Up(Start), 340), vec![GbaDown(Start), GbaUp(Start)]);
}

#[test]
fn in_a_menu_a_single_button_shortcut_does_nothing() {
    let mut g = Gestures::new();
    g.set_entries(vec![entry(
        None,
        A,
        BrightnessUp,
        slot_input::Contexts::GAME.with(slot_input::Contexts::SHELF),
    )]);
    g.set_context(slot_input::Context::Menu);
    assert_eq!(g.feed(Down(A), 0), vec![GbaDown(A)]);
}

#[test]
fn a_game_only_shortcut_does_nothing_on_the_shelf() {
    let mut g = on_shelf(vec![entry(None, R2, FfStart, slot_input::Contexts::GAME)]);
    assert_eq!(g.feed(Down(R2), 0), vec![GbaDown(R2)]);
}

#[test]
fn a_press_seen_during_a_capture_leaves_nothing_behind() {
    let mut g = paired();
    g.observe(Down(L1));
    g.observe(Up(L1));
    assert_eq!(
        g.feed(Down(R1), 0),
        vec![GbaDown(R1)],
        "a stale L1 completed the pair"
    );
}

#[test]
fn select_tap_on_the_shelf() {
    let mut g = gestures();
    g.set_context(Context::Shelf);
    assert!(g.feed(Down(Select), 0).is_empty());
    assert_eq!(g.feed(Up(Select), 80), vec![GbaDown(Select), SelectTap]);
}

#[test]
fn no_select_tap_in_game() {
    let mut g = gestures();
    assert!(g.feed(Down(Select), 0).is_empty());
    assert!(!g.feed(Up(Select), 80).contains(&SelectTap));
}

#[test]
fn a_select_combo_is_no_tap() {
    let mut g = gestures();
    g.set_context(Context::Shelf);
    let mut log = g.feed(Down(Select), 0);
    log.extend(g.feed(Down(Btn::X), 40));
    log.extend(g.feed(Up(Btn::X), 80));
    log.extend(g.feed(Up(Select), 120));
    assert!(!log.contains(&SelectTap), "{log:?}");
}

#[test]
fn a_select_hold_is_no_tap() {
    let mut g = gestures();
    g.set_context(Context::Shelf);
    let mut log = g.feed(Down(Select), 0);
    log.extend(g.tick(700));
    log.extend(g.feed(Up(Select), 800));
    assert!(!log.contains(&SelectTap), "{log:?}");
}
