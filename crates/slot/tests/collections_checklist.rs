mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::App;
use slot::editing::Edit;
use slot_input::{Btn, FAVORITE_HOLD_MS};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};
use tempfile::TempDir;

fn game(platform: Platform, stem: &str) -> Recent {
    Recent {
        platform,
        stem: stem.into(),
    }
}

fn put(root: &Path, name: &str, body: &str) {
    let dir = root.join("Collections");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
}

fn read(root: &Path, name: &str) -> String {
    std::fs::read_to_string(root.join("Collections").join(name)).unwrap()
}

fn bare() -> TempDir {
    let d = tmp_root_with_carts(&["Apotris", "Drill Dozer", "Fusion"]);
    write_gb_cart(&d, "Catrap", "CATRAP");
    d
}

fn card() -> TempDir {
    let d = bare();
    put(d.path(), "Puzzlers.txt", "gba/Apotris\ngb/Catrap\n");
    put(d.path(), "Dozers.txt", "gba/Drill Dozer\n");
    put(d.path(), "order.txt", "Puzzlers\nDozers\n");
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

fn one_favorite() -> SlotState {
    SlotState {
        favorites: vec![game(Platform::Gba, "Drill Dozer")],
        ..SlotState::default()
    }
}

fn press(a: &mut App, btn: Btn) {
    common::through(a, btn);
}

fn go_to(a: &mut App, shelf: &str) {
    for _ in 0..12 {
        if a.shelf_platform_name() == Some(shelf) {
            return;
        }
        press(a, Btn::R1);
    }
    panic!("never reached the {shelf} shelf");
}

fn flip_to(a: &mut App, shelf: &str) {
    go_to(a, "Favorites");
    for _ in 0..6 {
        if a.shelf_platform_name() == Some(shelf) {
            return;
        }
        press(a, Btn::R2);
    }
    panic!("never flipped to {shelf}");
}

fn on_cart(a: &mut App, shelf: &str, stem: &str) {
    go_to(a, shelf);
    for _ in 0..4 {
        if a.selected_stem() == Some(stem) {
            return;
        }
        press(a, Btn::Right);
    }
    assert_eq!(a.selected_stem(), Some(stem));
}

fn hold(a: &mut App, btn: Btn) {
    common::hold(a, btn, FAVORITE_HOLD_MS);
}

fn to_choice(a: &mut App, name: &str) {
    for _ in 0..6 {
        if a.picked_choice() == Some(name) {
            return;
        }
        press(a, Btn::Right);
    }
    panic!("no {name} choice: {:?}", a.choice_names());
}

fn to_keyboard(a: &mut App) {
    on_cart(a, "Game Boy Advance", "Fusion");
    hold(a, Btn::Y);
    to_choice(a, "New Collection");
    press(a, Btn::A);
}

fn find(a: &App, want: &str) -> Option<(usize, usize)> {
    let kb = a.keyboard().expect("no keyboard");
    kb.rows().iter().enumerate().find_map(|(r, row)| {
        row.iter()
            .position(|cap| kb.label(*cap) == want)
            .map(|c| (r, c))
    })
}

fn key(a: &mut App, want: &str) {
    if find(a, want).is_none() {
        let flip = if a.keyboard().unwrap().symbols() { "abc" } else { "#+=" };
        key(a, flip);
    }
    let (r, c) = find(a, want).unwrap_or_else(|| panic!("no {want} key"));
    for _ in 0..12 {
        if a.keyboard().unwrap().row() == r {
            break;
        }
        press(a, Btn::Down);
    }
    for _ in 0..12 {
        if a.keyboard().unwrap().col() == c {
            break;
        }
        press(a, Btn::Right);
    }
    press(a, Btn::A);
}

fn type_text(a: &mut App, text: &str) {
    for ch in text.chars() {
        match ch {
            ' ' => key(a, "Space"),
            _ => key(a, &ch.to_string()),
        }
    }
    assert!(a.keyboard().unwrap().text().ends_with(text));
}

#[test]
fn l2_r2_on_the_stack_never_toast() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    for _ in 0..3 {
        press(&mut a, Btn::R2);
        assert_eq!(a.toast(), None);
    }
    press(&mut a, Btn::L2);
    assert_eq!(a.toast(), None);
}

#[test]
fn l1_r1_from_a_collection_move_as_if_on_favorites() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R1);
    let right = a.shelf_platform_name().map(str::to_string);
    press(&mut a, Btn::L1);
    press(&mut a, Btn::L1);
    let left = a.shelf_platform_name().map(str::to_string);
    press(&mut a, Btn::R1);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    press(&mut a, Btn::R2);
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    press(&mut a, Btn::R1);
    assert_eq!(a.shelf_platform_name().map(str::to_string), right);
    press(&mut a, Btn::L1);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    press(&mut a, Btn::L1);
    assert_eq!(a.shelf_platform_name().map(str::to_string), left);
}

#[test]
fn a_y_tap_favorites_and_leaves_no_hold_behind() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    press(&mut a, Btn::Y);
    assert!(!a.pressing());
    assert!(a.edit().is_none());
    a.flush_state();
    assert!(read_slot_state(d.path())
        .favorites
        .contains(&game(Platform::Gba, "Fusion")));
}

#[test]
fn with_no_collections_only_new_collection_is_offered() {
    let d = bare();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold(&mut a, Btn::Y);
    assert_eq!(a.choice_names(), ["New Collection"]);
}

#[test]
fn with_one_collection_it_sits_beside_new_collection() {
    let d = bare();
    put(d.path(), "Puzzlers.txt", "gba/Apotris\n");
    put(d.path(), "order.txt", "Puzzlers\n");
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold(&mut a, Btn::Y);
    assert_eq!(a.choice_names(), ["Puzzlers", "New Collection"]);
}

#[test]
fn favorites_is_never_offered() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold(&mut a, Btn::Y);
    assert!(!a.choice_names().iter().any(|n| n == "Favorites"));
}

#[test]
fn the_keyboard_is_titled_new_collection() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    assert_eq!(a.keyboard().unwrap().title(), "New Collection");
}

#[test]
fn a_first_collection_names_and_adds() {
    let d = bare();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, "racing");
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert_eq!(read(d.path(), "racing.txt"), "gba/Fusion\n");
    assert_eq!(read(d.path(), "order.txt"), "racing\n");
    assert_eq!(a.collection_said().unwrap().1, "Added to racing");
}

#[test]
fn a_taken_name_in_another_case_is_refused() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, "dozers");
    press(&mut a, Btn::Start);
    let kb = a.keyboard().unwrap();
    assert!(kb.alert());
    assert_eq!(
        kb.subtitle(),
        Some("Name already exists, use a different name")
    );
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nDozers\n");
}

#[test]
fn every_name_that_cannot_be_a_file_is_refused() {
    for bad in ["", "order", ".hidden", "trail.", "a:b"] {
        let d = card();
        let mut a = boot_with(d.path(), one_favorite());
        to_keyboard(&mut a);
        if !bad.is_empty() {
            type_text(&mut a, bad);
        }
        press(&mut a, Btn::Start);
        let kb = a
            .keyboard()
            .unwrap_or_else(|| panic!("{bad:?} closed the keyboard"));
        assert_eq!(
            kb.subtitle(),
            Some("Name not allowed, use a different name"),
            "{bad:?}"
        );
        assert_eq!(kb.text(), bad);
        assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nDozers\n", "{bad:?}");
    }
}

#[test]
fn spaces_around_a_name_are_trimmed() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, " racing ");
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert_eq!(read(d.path(), "racing.txt"), "gba/Fusion\n");
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nDozers\nracing\n");
}

#[test]
fn a_refusal_keeps_the_text_the_key_and_shift() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, "dozers");
    key(&mut a, "Shift");
    let before = {
        let kb = a.keyboard().unwrap();
        (kb.row(), kb.col(), kb.shift(), kb.text())
    };
    assert!(before.2, "Shift did not turn on");
    press(&mut a, Btn::Start);
    let kb = a.keyboard().unwrap();
    assert!(kb.alert());
    assert_eq!((kb.row(), kb.col(), kb.shift(), kb.text()), before);
}

#[test]
fn removing_the_last_game_keeps_the_wallpaper() {
    let d = card();
    let walls = d.path().join("Wallpapers/Collections");
    std::fs::create_dir_all(&walls).unwrap();
    std::fs::write(walls.join("Dozers.png"), b"png").unwrap();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Dozers");
    hold(&mut a, Btn::Y);
    assert_eq!(a.removing(), Some(("Dozers", true)));
    press(&mut a, Btn::A);
    assert!(!d.path().join("Collections/Dozers.txt").exists());
    assert!(walls.join("Dozers.png").exists());
}

#[test]
fn a_deleted_collection_is_gone_from_the_add_screen() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Dozers");
    hold(&mut a, Btn::Y);
    press(&mut a, Btn::A);
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold(&mut a, Btn::Y);
    assert!(matches!(a.edit(), Some(Edit::Choosing { .. })));
    assert!(!a.choice_names().iter().any(|n| n == "Dozers"));
}

#[test]
fn a_collection_whose_games_are_all_missing_stays_on_the_card_hidden() {
    let d = card();
    put(d.path(), "Gone.txt", "gba/Lost\ngb/Missing\n");
    put(d.path(), "order.txt", "Puzzlers\nGone\nDozers\n");
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    let mut seen = Vec::new();
    for _ in 0..4 {
        press(&mut a, Btn::R2);
        seen.extend(a.shelf_platform_name().map(str::to_string));
    }
    assert!(!seen.iter().any(|n| n == "Gone"), "{seen:?}");
    assert_eq!(read(d.path(), "Gone.txt"), "gba/Lost\ngb/Missing\n");
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nGone\nDozers\n");
}

#[test]
fn the_tapped_game_move_is_dropped_when_the_hold_lifts_the_collection() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Puzzlers");
    let stems: Vec<String> = a.shelf_stems().iter().map(|s| s.to_string()).collect();
    hold(&mut a, Btn::X);
    assert_eq!(a.moving(), Some("Move Puzzlers".to_string()));
    assert!(matches!(a.edit(), Some(Edit::MovingCollection { .. })));
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_stems().iter().map(|s| s.to_string()).collect::<Vec<_>>(), stems);
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nDozers\n");
}
