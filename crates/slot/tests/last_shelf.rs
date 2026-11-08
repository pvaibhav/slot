mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart, StubSnapshot};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};
use tempfile::TempDir;

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Drill Dozer", "Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn boot_with(root: &Path, state: SlotState) -> App {
    write_slot_state(
        root,
        &SlotState {
            clock_set: true,
            ..state
        },
    )
    .expect("write slot.state");
    App::boot(root)
}

fn settle(a: &mut App) {
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
}

fn press(a: &mut App, btn: Btn) {
    common::through(a, btn);
    settle(a);
}

fn power_off(a: &mut App) {
    a.apply(Action::PowerHold);
    assert!(a.powering_off());
}

fn recent(platform: Platform, stem: &str) -> Recent {
    Recent {
        platform,
        stem: stem.into(),
    }
}

#[test]
fn powering_off_on_a_cart_boots_back_onto_its_shelf_and_cart() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Fusion"));
    power_off(&mut a);

    let a = App::boot(d.path());
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn the_remembered_shelf_wins_over_recents() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            show_recents: true,
            recents: vec![recent(Platform::Gba, "Emerald")],
            ..SlotState::default()
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    while a.shelf_platform_name() != Some("Game Boy") {
        press(&mut a, Btn::R1);
    }
    power_off(&mut a);

    let a = App::boot(d.path());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy"));
    assert_eq!(a.selected_stem(), Some("Tetris"));
}

#[test]
fn a_remembered_cart_the_card_no_longer_holds_boots_as_before() {
    let d = card();
    let a = boot_with(
        d.path(),
        SlotState {
            shelf: Some("gba".into()),
            shelf_cart: Some(recent(Platform::Gba, "Deleted")),
            ..SlotState::default()
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Drill Dozer"));
}

#[test]
fn a_game_powered_off_from_its_own_shelf_ejects_back_onto_that_shelf() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            cart: Some("Fusion".into()),
            cart_platform: Some(Platform::Gba),
            show_recents: true,
            recents: vec![recent(Platform::Gba, "Fusion")],
            shelf: Some("gba".into()),
            shelf_cart: Some(recent(Platform::Gba, "Fusion")),
            ..SlotState::default()
        },
    );
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_ready();
    settle(&mut a);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    a.apply(Action::Eject);
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    assert_eq!(read_slot_state(d.path()).cart, None);
}

#[test]
fn l1_r1_alone_is_remembered_without_a_clean_power_off() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    press(&mut a, Btn::R1);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy"));
    a.flush_state();
    drop(a);

    let a = App::boot(d.path());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy"));
    assert_eq!(a.selected_stem(), Some("Tetris"));
}
