mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};
use tempfile::TempDir;

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn favorite(platform: Platform, stem: &str) -> Recent {
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

fn press(a: &mut App, btn: Btn) {
    common::through(a, btn);
}

fn go_to(a: &mut App, shelf: &str) {
    for _ in 0..8 {
        if a.shelf_platform_name() == Some(shelf) {
            return;
        }
        press(a, Btn::R1);
    }
    panic!("never reached the {shelf} shelf");
}

fn names(a: &mut App) -> Vec<&'static str> {
    let mut names = Vec::new();
    for _ in 0..4 {
        press(a, Btn::R1);
        names.extend(a.shelf_platform_name());
    }
    names
}

#[test]
fn y_on_the_shelf_hearts_the_cart_and_saves_it_without_a_banner() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    let stem = a.selected_stem().unwrap().to_string();
    press(&mut a, Btn::Y);
    assert_eq!(
        read_slot_state(d.path()).favorites,
        [favorite(Platform::Gba, &stem)]
    );
    assert_eq!(a.toast(), None);
    assert_eq!(
        a.selected_stem(),
        Some(stem.as_str()),
        "hearting moved the shelf"
    );
}

#[test]
fn the_favorites_shelf_only_shows_once_something_is_hearted() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    assert!(!names(&mut a).contains(&"Favorites"));
    go_to(&mut a, "Game Boy");
    press(&mut a, Btn::Y);
    go_to(&mut a, "Favorites");
    assert_eq!(a.selected_stem(), Some("Tetris"));
}

#[test]
fn favorites_from_every_system_share_one_shelf_in_name_order() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            favorites: vec![
                favorite(Platform::Gb, "Tetris"),
                favorite(Platform::Gba, "Fusion"),
                favorite(Platform::Gba, "Emerald"),
            ],
            ..SlotState::default()
        },
    );
    go_to(&mut a, "Favorites");
    let mut seen = Vec::new();
    for _ in 0..3 {
        seen.push(a.selected_stem().unwrap().to_string());
        a.apply(Action::ShelfRight);
        a.apply(Action::GbaUp(Btn::Right));
        for _ in 0..60 {
            a.update(1.0 / 60.0);
        }
    }
    assert_eq!(seen, ["Emerald", "Fusion", "Tetris"]);
}

#[test]
fn unhearting_on_the_favorites_shelf_takes_the_cart_off_it() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            favorites: vec![
                favorite(Platform::Gba, "Emerald"),
                favorite(Platform::Gb, "Tetris"),
            ],
            ..SlotState::default()
        },
    );
    go_to(&mut a, "Favorites");
    assert_eq!(a.selected_stem(), Some("Emerald"));
    press(&mut a, Btn::Y);
    assert_eq!(a.toast(), None);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    assert_eq!(a.selected_stem(), Some("Tetris"));
    assert_eq!(
        read_slot_state(d.path()).favorites,
        [favorite(Platform::Gb, "Tetris")]
    );
}

#[test]
fn unhearting_the_last_favorite_leaves_the_empty_shelf() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            favorites: vec![favorite(Platform::Gb, "Tetris")],
            ..SlotState::default()
        },
    );
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::Y);
    assert_ne!(a.shelf_platform_name(), Some("Favorites"));
    assert!(read_slot_state(d.path()).favorites.is_empty());
    assert!(!names(&mut a).contains(&"Favorites"));
}

#[test]
fn a_favorite_whose_rom_is_gone_is_left_off_the_shelf() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            favorites: vec![
                favorite(Platform::Gba, "Deleted"),
                favorite(Platform::Gba, "Fusion"),
            ],
            ..SlotState::default()
        },
    );
    go_to(&mut a, "Favorites");
    assert_eq!(a.selected_stem(), Some("Fusion"));
}
