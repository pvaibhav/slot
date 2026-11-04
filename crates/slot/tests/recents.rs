mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart, StubSnapshot};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};
use slot_ui::{QuickRow, QuickValue};
use tempfile::TempDir;

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn recent(platform: Platform, stem: &str) -> Recent {
    Recent {
        platform,
        stem: stem.into(),
    }
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

fn play(root: &Path, stem: &str, show_recents: bool) -> App {
    let mut a = boot_with(
        root,
        SlotState {
            cart: Some(stem.into()),
            show_recents,
            ..read_slot_state(root)
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
    a
}

fn eject(a: &mut App) {
    a.apply(Action::Eject);
    settle(a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

fn press(a: &mut App, btn: Btn) {
    common::through(a, btn);
}

fn flip_recents(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::General.position(&[]) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    for _ in 0..QuickRow::Recents.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(QuickRow::Recents));
    let was = a.quick_value(QuickRow::Recents);
    press(a, Btn::Right);
    let now = a.quick_value(QuickRow::Recents);
    assert!(
        matches!(
            (was, now),
            (Some(QuickValue::Hide), Some(QuickValue::Show))
                | (Some(QuickValue::Show), Some(QuickValue::Hide))
        ),
        "{was:?} then {now:?}"
    );
    while a.quick_menu().is_some() {
        press(a, Btn::B);
    }
}

#[test]
fn a_played_game_is_remembered_even_with_the_shelf_off() {
    let d = card();
    let mut a = play(d.path(), "Emerald", false);
    eject(&mut a);
    assert_eq!(
        read_slot_state(d.path()).recents,
        [recent(Platform::Gba, "Emerald")]
    );
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
}

#[test]
fn a_game_put_in_from_recents_comes_back_out_onto_recents_at_the_front() {
    let d = card();
    let mut s = read_slot_state(d.path());
    s.played(Platform::Gba, "Fusion");
    s.played(Platform::Gba, "Emerald");
    s.show_recents = true;
    write_slot_state(d.path(), &s).unwrap();
    let mut a = play(d.path(), "Fusion", true);
    eject(&mut a);
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    assert_eq!(
        read_slot_state(d.path()).recents,
        [
            recent(Platform::Gba, "Fusion"),
            recent(Platform::Gba, "Emerald")
        ]
    );
}

#[test]
fn a_game_put_in_from_its_own_shelf_comes_back_out_onto_that_shelf() {
    let d = card();
    let mut a = play(d.path(), "Fusion", true);
    eject(&mut a);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    while a.shelf_platform_name() != Some("Recents") {
        press(&mut a, Btn::R1);
    }
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn with_recents_on_the_card_boots_onto_the_last_game_played() {
    let d = card();
    let a = boot_with(
        d.path(),
        SlotState {
            show_recents: true,
            recents: vec![
                recent(Platform::Gb, "Tetris"),
                recent(Platform::Gba, "Emerald"),
            ],
            ..SlotState::default()
        },
    );
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    assert_eq!(a.selected_stem(), Some("Tetris"));
}

#[test]
fn with_recents_off_the_card_boots_as_it_always_has() {
    let d = card();
    let a = boot_with(
        d.path(),
        SlotState {
            recents: vec![recent(Platform::Gb, "Tetris")],
            ..SlotState::default()
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
}

#[test]
fn a_recent_game_whose_rom_is_gone_is_left_off_the_shelf() {
    let d = card();
    let a = boot_with(
        d.path(),
        SlotState {
            show_recents: true,
            recents: vec![
                recent(Platform::Gba, "Deleted"),
                recent(Platform::Gba, "Fusion"),
            ],
            ..SlotState::default()
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn turning_recents_on_and_off_in_the_menu_saves_and_adds_and_removes_the_shelf() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            recents: vec![recent(Platform::Gba, "Fusion")],
            ..SlotState::default()
        },
    );
    flip_recents(&mut a);
    assert!(read_slot_state(d.path()).show_recents);
    let mut names = Vec::new();
    for _ in 0..3 {
        press(&mut a, Btn::R1);
        names.extend(a.shelf_platform_name());
    }
    assert!(names.contains(&"Recents"), "{names:?}");
    while a.shelf_platform_name() != Some("Recents") {
        press(&mut a, Btn::R1);
    }
    flip_recents(&mut a);
    assert!(!read_slot_state(d.path()).show_recents);
    assert_ne!(
        a.shelf_platform_name(),
        Some("Recents"),
        "turning recents off left the carousel on an empty shelf"
    );
    for _ in 0..3 {
        press(&mut a, Btn::R1);
        assert_ne!(a.shelf_platform_name(), Some("Recents"));
    }
}

#[test]
fn a_one_game_card_is_still_a_one_game_card_with_recents_on() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = play(d.path(), "Emerald", true);
    a.apply(Action::Eject);
    settle(&mut a);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "the lone cart came out: {:?}",
        a.phase()
    );
}

fn three_recents(root: &Path) -> App {
    boot_with(
        root,
        SlotState {
            show_recents: true,
            recents: vec![
                recent(Platform::Gb, "Tetris"),
                recent(Platform::Gba, "Emerald"),
                recent(Platform::Gba, "Fusion"),
            ],
            ..SlotState::default()
        },
    )
}

#[test]
fn x_on_recents_takes_the_selected_game_off_and_selects_the_next() {
    let d = card();
    let mut a = three_recents(d.path());
    assert_eq!(a.selected_stem(), Some("Tetris"));
    press(&mut a, Btn::X);
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    assert_eq!(a.selected_stem(), Some("Emerald"));
    assert_eq!(
        read_slot_state(d.path()).recents,
        [
            recent(Platform::Gba, "Emerald"),
            recent(Platform::Gba, "Fusion")
        ]
    );
}

#[test]
fn x_on_a_console_shelf_leaves_recents_alone() {
    let d = card();
    let mut a = three_recents(d.path());
    while a.shelf_platform_name() != Some("Game Boy Advance") {
        press(&mut a, Btn::R1);
    }
    press(&mut a, Btn::X);
    assert_eq!(read_slot_state(d.path()).recents.len(), 3);
}

#[test]
fn removing_the_last_recent_moves_off_the_empty_shelf() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            show_recents: true,
            recents: vec![recent(Platform::Gba, "Fusion")],
            ..SlotState::default()
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Recents"));
    press(&mut a, Btn::X);
    assert!(read_slot_state(d.path()).recents.is_empty());
    assert_ne!(a.shelf_platform_name(), Some("Recents"));
    assert!(a.selected_stem().is_some());
}
