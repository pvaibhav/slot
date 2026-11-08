mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::App;
use slot::editing::Edit;
use slot_input::{Btn, FAVORITE_HOLD_MS, SELECT_HOLD_MS};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, ShelfMode, SlotState};
use slot_ui::Toast;
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

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Apotris", "Drill Dozer", "Fusion"]);
    write_gb_cart(&d, "Catrap", "CATRAP");
    put(d.path(), "Puzzlers.txt", "gba/Apotris\ngb/Catrap\n");
    put(d.path(), "Empty.txt", "");
    put(d.path(), "order.txt", "Puzzlers\nEmpty\n");
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

fn select_hold(a: &mut App) {
    common::hold(a, Btn::Select, SELECT_HOLD_MS);
    if let Some(Edit::Selecting { checked, .. }) = a.edit() {
        if !checked.is_empty() {
            press(a, Btn::A);
        }
    }
}

fn checked(a: &App) -> Vec<Recent> {
    match a.edit() {
        Some(Edit::Selecting { checked, .. }) => checked.clone(),
        other => panic!("not selecting: {other:?}"),
    }
}

fn hold_y(a: &mut App) {
    common::hold(a, Btn::Y, FAVORITE_HOLD_MS);
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

fn to_choice(a: &mut App, name: &str) {
    for _ in 0..6 {
        if a.picked_choice() == Some(name) {
            return;
        }
        press(a, Btn::Right);
    }
    panic!("no {name} choice: {:?}", a.choice_names());
}

fn choosing(a: &App) -> bool {
    matches!(a.edit(), Some(Edit::Choosing { .. }))
}

fn check_on(a: &mut App, shelf: &str, stem: &str) {
    go_to(a, shelf);
    for _ in 0..4 {
        if a.selected_stem() == Some(stem) {
            break;
        }
        press(a, Btn::Right);
    }
    assert_eq!(a.selected_stem(), Some(stem));
    press(a, Btn::A);
}

fn type_text(a: &mut App, text: &str) {
    for ch in text.chars() {
        let want = ch.to_string();
        let kb = a.keyboard().expect("no keyboard");
        let (r, c) = kb
            .rows()
            .iter()
            .enumerate()
            .find_map(|(r, row)| {
                row.iter()
                    .position(|cap| kb.label(*cap) == want)
                    .map(|c| (r, c))
            })
            .unwrap_or_else(|| panic!("no key for {ch}"));
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
    assert!(a.keyboard().unwrap().text().ends_with(text));
}

#[test]
fn holding_select_starts_with_the_cart_in_front_checked() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    assert!(a.edit().is_none());
    common::hold(&mut a, Btn::Select, SELECT_HOLD_MS);
    assert_eq!(checked(&a), [game(Platform::Gba, "Fusion")]);
    assert!(a.studio().is_none());
}

#[test]
fn a_checks_and_unchecks_the_cart_in_front() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    assert_eq!(checked(&a), [game(Platform::Gb, "Catrap")]);
    press(&mut a, Btn::A);
    assert!(checked(&a).is_empty());
}

#[test]
fn checks_survive_changing_shelves() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    check_on(&mut a, "Game Boy Advance", "Apotris");
    assert_eq!(
        checked(&a),
        [game(Platform::Gb, "Catrap"), game(Platform::Gba, "Apotris")]
    );
}

#[test]
fn start_with_nothing_checked_does_nothing() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    press(&mut a, Btn::Start);
    assert!(checked(&a).is_empty());
}

#[test]
fn b_while_selecting_clears_and_ends() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
}

#[test]
fn holding_y_opens_the_add_screen_with_that_game() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    assert!(choosing(&a));
    assert_eq!(a.picked_count(), Some(1));
    assert_eq!(a.choice_names(), ["Puzzlers", "New Collection", "Empty"]);
    assert!(!read_slot_state(d.path())
        .favorites
        .contains(&game(Platform::Gba, "Fusion")));
}

#[test]
fn tapping_y_still_favorites_on_a_collection_shelf() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    assert_eq!(a.selected_stem(), Some("Apotris"));
    press(&mut a, Btn::Y);
    a.flush_state();
    assert!(read_slot_state(d.path())
        .favorites
        .contains(&game(Platform::Gba, "Apotris")));
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
    assert!(a.edit().is_none());
}

#[test]
fn left_and_right_pick_and_wrap() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    assert_eq!(a.picked_choice(), Some("New Collection"));
    press(&mut a, Btn::Right);
    assert_eq!(a.picked_choice(), Some("Empty"));
    press(&mut a, Btn::Right);
    assert_eq!(a.picked_choice(), Some("Puzzlers"));
    press(&mut a, Btn::Left);
    assert_eq!(a.picked_choice(), Some("Empty"));
}

#[test]
fn a_adds_and_returns_to_the_cart_you_were_on() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    to_choice(&mut a, "Puzzlers");
    press(&mut a, Btn::A);
    assert_eq!(
        read(d.path(), "Puzzlers.txt"),
        "gba/Apotris\ngb/Catrap\ngba/Fusion\n"
    );
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
    assert_eq!(a.toast(), Some(Toast::Collection));
    assert_eq!(a.collection_said().unwrap().1, "Added to Puzzlers");
}

#[test]
fn the_screen_opens_on_new_collection_after_adding_to_one() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    to_choice(&mut a, "Empty");
    press(&mut a, Btn::A);
    on_cart(&mut a, "Game Boy Advance", "Apotris");
    hold_y(&mut a);
    assert_eq!(a.picked_choice(), Some("New Collection"));
}

#[test]
fn three_ticked_with_one_already_in_adds_two() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy Advance", "Apotris");
    check_on(&mut a, "Game Boy Advance", "Drill Dozer");
    check_on(&mut a, "Game Boy Advance", "Fusion");
    press(&mut a, Btn::Start);
    to_choice(&mut a, "Puzzlers");
    press(&mut a, Btn::A);
    assert_eq!(
        read(d.path(), "Puzzlers.txt"),
        "gba/Apotris\ngb/Catrap\ngba/Drill Dozer\ngba/Fusion\n"
    );
    assert_eq!(a.collection_said().unwrap().1, "Added 2 to Puzzlers");
}

#[test]
fn a_collection_holding_every_ticked_game_is_not_offered() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    check_on(&mut a, "Game Boy Advance", "Apotris");
    press(&mut a, Btn::Start);
    assert_eq!(a.choice_names(), ["Empty", "New Collection"]);
}

#[test]
fn b_in_multi_returns_to_selecting_where_start_was_pressed_with_ticks_kept() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    check_on(&mut a, "Game Boy Advance", "Fusion");
    press(&mut a, Btn::Start);
    assert!(choosing(&a));
    press(&mut a, Btn::B);
    assert_eq!(
        checked(&a),
        [game(Platform::Gb, "Catrap"), game(Platform::Gba, "Fusion")]
    );
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn b_with_one_game_returns_to_the_shelf() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn collections_mode_from_a_system_shelf_lands_on_the_destination() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            shelf_mode: ShelfMode::Collections,
            ..one_favorite()
        },
    );
    select_hold(&mut a);
    check_on(&mut a, "Game Boy Advance", "Fusion");
    press(&mut a, Btn::Start);
    to_choice(&mut a, "Puzzlers");
    press(&mut a, Btn::A);
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

#[test]
fn collections_mode_shows_platforms_while_picking() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        SlotState {
            shelf_mode: ShelfMode::Collections,
            ..one_favorite()
        },
    );
    select_hold(&mut a);
    go_to(&mut a, "Game Boy Advance");
    press(&mut a, Btn::B);
    assert!(a.edit().is_some());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    go_to(&mut a, "Puzzlers");
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    let mut seen = Vec::new();
    for _ in 0..6 {
        press(&mut a, Btn::R1);
        seen.extend(a.shelf_platform_name().map(str::to_string));
    }
    assert!(!seen.iter().any(|n| n == "Game Boy Advance"), "{seen:?}");
}

#[test]
fn new_collection_names_and_adds() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    to_choice(&mut a, "New Collection");
    press(&mut a, Btn::A);
    assert!(a.keyboard().is_some());
    type_text(&mut a, "racing");
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert_eq!(read(d.path(), "racing.txt"), "gba/Fusion\n");
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nEmpty\nracing\n");
    assert_eq!(a.collection_said().unwrap().1, "Added to racing");
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.selected_stem(), Some("Fusion"));
}

fn to_keyboard(a: &mut App) {
    on_cart(a, "Game Boy Advance", "Fusion");
    hold_y(a);
    to_choice(a, "New Collection");
    press(a, Btn::A);
}

#[test]
fn a_bad_name_keeps_the_keyboard_open() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, "order");
    press(&mut a, Btn::Start);
    assert!(!d.path().join("Collections/order.txt.txt").exists());
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nEmpty\n");
    assert_ne!(a.toast(), Some(Toast::Collection));
    let kb = a.keyboard().unwrap();
    assert_eq!(kb.text(), "order");
    assert_eq!(kb.subtitle(), Some("Name not allowed, use a different name"));
}

#[test]
fn a_name_already_taken_says_so() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    type_text(&mut a, "empty");
    let at = {
        let kb = a.keyboard().unwrap();
        (kb.row(), kb.col())
    };
    press(&mut a, Btn::Start);
    let kb = a.keyboard().unwrap();
    assert_eq!((kb.row(), kb.col()), at, "the selected key moved");
    assert!(kb.alert());
    assert_eq!(kb.text(), "empty");
    assert_eq!(
        kb.subtitle(),
        Some("Name already exists, use a different name")
    );
}

#[test]
fn cancelling_the_keyboard_returns_to_the_add_screen() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    to_keyboard(&mut a);
    press(&mut a, Btn::Y);
    assert!(a.keyboard().is_none());
    assert!(choosing(&a));
    assert_eq!(a.picked_choice(), Some("New Collection"));
}

#[test]
fn select_hold_with_the_core_picker_open_does_nothing() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Game Boy Advance");
    press(&mut a, Btn::Start);
    assert!(a.core_picker().is_some());
    a.apply(slot_input::Action::SelectHold);
    assert!(a.edit().is_none());
}

#[test]
fn start_while_selecting_never_opens_the_core_picker() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Game Boy Advance");
    select_hold(&mut a);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    assert_eq!(a.core_picker(), None);
    assert!(choosing(&a));
}

#[test]
fn a_while_selecting_never_inserts_the_cart() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Game Boy Advance");
    select_hold(&mut a);
    press(&mut a, Btn::A);
    assert!(
        matches!(a.phase(), slot::app::Phase::Shelf),
        "{:?}",
        a.phase()
    );
}

#[test]
fn the_bundle_asks_for_the_faces_of_what_it_carries() {
    let d = card();
    let gbc = d.path().join("Games/GBC");
    std::fs::create_dir_all(&gbc).unwrap();
    let mut rom = vec![0u8; 0x150];
    rom[0x134..0x134 + 7].copy_from_slice(b"CRYSTAL");
    rom[0x143] = 0x80;
    std::fs::write(gbc.join("Crystal.gbc"), rom).unwrap();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy Color", "Crystal");
    go_to(&mut a, "Game Boy Advance");
    press(&mut a, Btn::Start);
    let wanted: Vec<String> = a
        .face_wants()
        .iter()
        .filter_map(|w| a.face_cart(w.shelf, w.index).map(|c| c.stem.clone()))
        .collect();
    assert!(wanted.iter().any(|s| s == "Crystal"), "{wanted:?}");
}

#[test]
fn the_picked_collection_shows_its_art_and_new_collection_shows_yours() {
    let d = card();
    let papers = d.path().join("Wallpapers");
    std::fs::create_dir_all(papers.join("Collections")).unwrap();
    std::fs::write(papers.join("duck.png"), b"").unwrap();
    std::fs::write(papers.join("Collections/Empty.png"), b"").unwrap();
    let mut a = boot_with(
        d.path(),
        SlotState {
            wallpaper: slot_store::WallpaperPick::File("duck.png".into()),
            ..one_favorite()
        },
    );
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    to_choice(&mut a, "Empty");
    assert_eq!(
        a.shown_wallpaper().as_deref(),
        Some("Collections/Empty.png")
    );
    to_choice(&mut a, "New Collection");
    assert_eq!(a.shown_wallpaper().as_deref(), Some("duck.png"));
}

#[test]
fn the_add_screen_asks_for_the_faces_in_every_stack() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    let wanted: Vec<String> = a
        .face_wants()
        .iter()
        .filter_map(|w| a.face_cart(w.shelf, w.index).map(|c| c.stem.clone()))
        .collect();
    for stem in ["Fusion", "Apotris", "Catrap"] {
        assert!(wanted.iter().any(|s| s == stem), "{stem} missing from {wanted:?}");
    }
}

#[test]
fn the_add_exit_shows_the_stack_as_it_was_before_adding() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    hold_y(&mut a);
    to_choice(&mut a, "Puzzlers");
    press(&mut a, Btn::A);
    assert_eq!(
        a.exit_stack(),
        Some(vec!["Apotris".to_string(), "Catrap".to_string()])
    );
    assert_eq!(a.exit_cart(), Some("Fusion".to_string()));
}

#[test]
fn a_multi_add_exit_never_flies_one_cart_home() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    select_hold(&mut a);
    check_on(&mut a, "Game Boy", "Catrap");
    check_on(&mut a, "Game Boy Advance", "Fusion");
    press(&mut a, Btn::Start);
    to_choice(&mut a, "Empty");
    press(&mut a, Btn::A);
    assert!(a.exit_stack().is_some());
    assert_eq!(a.exit_cart(), None);
}

#[test]
fn leaving_the_shelf_with_y_held_drops_the_jiggle() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    on_cart(&mut a, "Game Boy Advance", "Fusion");
    a.apply(slot_input::Action::GbaDown(Btn::Y));
    assert!(a.pressing());
    press(&mut a, Btn::A);
    for _ in 0..30 {
        a.update(0.016);
    }
    assert!(!matches!(a.phase(), slot::app::Phase::Shelf), "{:?}", a.phase());
    assert!(!a.pressing());
}
