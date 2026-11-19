mod common;

use common::{app_booting_at, tmp_root_with_carts, tmp_root_with_gbc_carts};
use slot::app::{App, Phase};
use slot_input::Action;
use slot_store::{parse_stamp, write_slot_state, Core, Platform, SlotState, Zone};
use slot_ui::{Icon, Toast};
use tempfile::TempDir;

fn new_york() -> Zone {
    Zone::ALL
        .into_iter()
        .find(|z| z.name() == "New York")
        .expect("New York on the list")
}

fn seated_at(stamp: &str, core: Core, platform: Platform) -> (TempDir, App, common::Clock) {
    let d = match platform {
        Platform::Gbc => tmp_root_with_gbc_carts(&["Boktai", "Zzz"]),
        _ => tmp_root_with_carts(&["Boktai", "Zzz"]),
    };
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            zone: new_york(),
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, clock) = app_booting_at(d.path(), parse_stamp(stamp).expect("a stamp"));
    a.apply(Action::Insert);
    a.set_core(core);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(
        a.playing_platform(),
        platform,
        "the cart seated on another shelf"
    );
    (d, a, clock)
}

#[test]
fn an_mgba_game_sees_the_sun_the_clock_says() {
    let (_d, a, _) = seated_at("2026-07-04_16-00-00", Core::Mgba, Platform::Gba);
    assert_eq!(a.sunlight(), Some(8), "noon in New York was not bright");
    let (_d, a, _) = seated_at("2026-07-04_04-00-00", Core::Mgba, Platform::Gba);
    assert_eq!(a.sunlight(), Some(0), "midnight in New York had sun");
}

#[test]
fn the_sun_moves_with_the_clock_while_the_game_runs() {
    let (_d, a, clock) = seated_at("2026-07-04_13-00-00", Core::Mgba, Platform::Gba);
    assert_eq!(a.sunlight(), Some(4));
    clock.advance(3 * 3600);
    assert_eq!(a.sunlight(), Some(8));
}

#[test]
fn an_armgba_game_sees_the_sun_too_but_not_on_a_game_boy_shelf() {
    let (_d, a, _) = seated_at("2026-07-04_16-00-00", Core::Armgba, Platform::Gba);
    assert_eq!(a.sunlight(), Some(8));
    let (_d, a, _) = seated_at("2026-07-04_16-00-00", Core::Armgba, Platform::Gbc);
    assert_eq!(a.sunlight(), None);
}

#[test]
fn gpsp_and_game_boy_games_get_no_sun() {
    let (_d, a, _) = seated_at("2026-07-04_16-00-00", Core::Gpsp, Platform::Gba);
    assert_eq!(a.sunlight(), None);
    let (_d, a, _) = seated_at("2026-07-04_16-00-00", Core::Mgba, Platform::Gbc);
    assert_eq!(a.sunlight(), None);
}

#[test]
fn the_shortcuts_step_the_sun_and_show_it_and_the_clock_no_longer_moves_it() {
    let (_d, mut a, clock) = seated_at("2026-07-04_13-00-00", Core::Mgba, Platform::Gba);
    a.apply(Action::SunlightUp);
    assert_eq!(a.sunlight(), Some(5));
    assert_eq!(a.hud_icon(), Icon::Sunlight);
    clock.advance(3 * 3600);
    assert_eq!(
        a.sunlight(),
        Some(5),
        "the clock overrode a sun picked by hand"
    );
    for _ in 0..20 {
        a.apply(Action::SunlightUp);
    }
    assert_eq!(a.sunlight(), Some(10));
    for _ in 0..20 {
        a.apply(Action::SunlightDown);
    }
    assert_eq!(a.sunlight(), Some(0));
}

#[test]
fn a_new_game_goes_back_to_the_clock() {
    let (_d, mut a, _) = seated_at("2026-07-04_13-00-00", Core::Mgba, Platform::Gba);
    a.apply(Action::SunlightDown);
    assert_eq!(a.sunlight(), Some(3));
    a.apply(Action::Eject);
    for _ in 0..2_000 {
        if matches!(a.phase(), Phase::Shelf) {
            break;
        }
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "the cart never came out: {:?}",
        a.phase()
    );
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(
        a.sunlight(),
        Some(4),
        "the sun picked by hand outlived its game"
    );
}

#[test]
fn the_shortcuts_on_gpsp_say_to_switch_to_mgba() {
    let (_d, mut a, _) = seated_at("2026-07-04_16-00-00", Core::Gpsp, Platform::Gba);
    a.apply(Action::SunlightUp);
    assert_eq!(a.toast(), Some(Toast::NeedsMgba));
    assert_eq!(a.sunlight(), None);
}
