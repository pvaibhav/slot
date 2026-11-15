mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::App;
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, ShelfMode, SlotState};
use slot_ui::{QuickRow, QuickValue, Toast};
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

fn bare_card() -> TempDir {
    let d = tmp_root_with_carts(&["Apotris", "Drill Dozer"]);
    write_gb_cart(&d, "Catrap", "CATRAP");
    d
}

fn card() -> TempDir {
    let d = bare_card();
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

fn name(a: &App) -> Option<String> {
    a.shelf_platform_name().map(str::to_string)
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

#[test]
fn r2_on_favorites_shows_the_next_collection_and_wraps() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    press(&mut a, Btn::L2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
}

#[test]
fn l2_and_r2_do_nothing_off_the_stack() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Game Boy Advance");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    press(&mut a, Btn::L2);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    assert_eq!(a.toast(), None);
}

#[test]
fn collections_never_join_the_l1_r1_row_in_all_mode() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    let mut seen = Vec::new();
    for _ in 0..10 {
        press(&mut a, Btn::R1);
        seen.extend(name(&a));
    }
    assert!(
        !seen.iter().any(|n| n == "Puzzlers" || n == "Dozers"),
        "{seen:?}"
    );
    assert!(seen.iter().any(|n| n == "Favorites"), "{seen:?}");
}

#[test]
fn the_stack_remembers_its_collection_across_shelves_and_reboots() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    press(&mut a, Btn::R1);
    assert_ne!(a.shelf_platform_name(), Some("Puzzlers"));
    press(&mut a, Btn::L1);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    press(&mut a, Btn::R1);
    a.flush_state();
    drop(a);
    let mut a = App::boot(d.path());
    go_to(&mut a, "Puzzlers");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
}

#[test]
fn a_reboot_on_a_collection_comes_back_to_it() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    a.flush_state();
    drop(a);
    let a = App::boot(d.path());
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn a_collection_shelf_keeps_its_file_order_across_systems() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_stems(), ["Apotris", "Catrap"]);
}

#[test]
fn an_empty_collection_is_skipped_by_l2_r2() {
    let d = card();
    put(d.path(), "Empty.txt", "");
    put(d.path(), "order.txt", "Puzzlers\nEmpty\nDozers\n");
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    let mut seen = Vec::new();
    for _ in 0..6 {
        press(&mut a, Btn::R2);
        seen.extend(name(&a));
    }
    assert!(!seen.iter().any(|n| n == "Empty"), "{seen:?}");
    assert_eq!(seen[..3], ["Puzzlers", "Dozers", "Favorites"]);
}

#[test]
fn with_no_favorites_the_first_collection_takes_their_place() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    let mut seen = Vec::new();
    for _ in 0..6 {
        press(&mut a, Btn::R1);
        seen.extend(name(&a));
    }
    assert!(seen.iter().any(|n| n == "Puzzlers"), "{seen:?}");
    assert!(
        !seen.iter().any(|n| n == "Favorites" || n == "Dozers"),
        "{seen:?}"
    );
    go_to(&mut a, "Puzzlers");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn unfavoriting_the_last_game_keeps_the_view_on_the_stack() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::Y);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn a_missing_rom_is_hidden_and_its_line_kept() {
    let d = card();
    put(
        d.path(),
        "Puzzlers.txt",
        "gba/Apotris\ngba/Gone\ngb/Catrap\n",
    );
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_stems(), ["Apotris", "Catrap"]);
    assert_eq!(read(d.path(), "Puzzlers.txt").lines().count(), 3);
}

#[test]
fn a_card_without_collections_behaves_as_before() {
    let d = bare_card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    assert_eq!(a.toast(), None);
}

fn collections_mode(state: SlotState) -> SlotState {
    SlotState {
        shelf_mode: ShelfMode::Collections,
        ..state
    }
}

fn row(a: &mut App) -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..8 {
        press(a, Btn::R1);
        seen.extend(name(a));
    }
    seen
}

fn open_shelf_mode(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::General.position(&[]) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    for _ in 0..QuickRow::ShelfMode.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(QuickRow::ShelfMode));
}

fn flip_shelf_mode(a: &mut App) {
    open_shelf_mode(a);
    press(a, Btn::Right);
    for _ in 0..3 {
        press(a, Btn::B);
    }
}

#[test]
fn collections_mode_flattens_collections_into_the_row() {
    let d = card();
    let mut a = boot_with(d.path(), collections_mode(one_favorite()));
    go_to(&mut a, "Favorites");
    let seen = row(&mut a);
    assert_eq!(seen[..3], ["Puzzlers", "Dozers", "Favorites"], "{seen:?}");
    assert!(
        !seen
            .iter()
            .any(|n| n == "Game Boy Advance" || n == "Game Boy"),
        "{seen:?}"
    );
}

#[test]
fn l2_r2_do_nothing_in_collections_mode() {
    let d = card();
    let mut a = boot_with(d.path(), collections_mode(one_favorite()));
    go_to(&mut a, "Puzzlers");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    press(&mut a, Btn::L2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn collections_mode_hides_an_empty_favorites() {
    let d = card();
    let mut a = boot_with(d.path(), collections_mode(SlotState::default()));
    let seen = row(&mut a);
    assert!(
        seen.iter().all(|n| n == "Puzzlers" || n == "Dozers"),
        "{seen:?}"
    );
}

#[test]
fn changing_shelf_mode_keeps_the_shelf_when_it_still_exists() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    flip_shelf_mode(&mut a);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    flip_shelf_mode(&mut a);
    go_to(&mut a, "Game Boy Advance");
    flip_shelf_mode(&mut a);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
}

#[test]
fn leaving_collections_mode_on_favorites_keeps_the_stack_on_favorites() {
    let d = card();
    let mut a = boot_with(
        d.path(),
        collections_mode(SlotState {
            collection: Some("Puzzlers".into()),
            ..one_favorite()
        }),
    );
    go_to(&mut a, "Favorites");
    flip_shelf_mode(&mut a);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    press(&mut a, Btn::R1);
    press(&mut a, Btn::L1);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
}

#[test]
fn the_shelf_mode_row_reads_and_cycles() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    open_shelf_mode(&mut a);
    assert_eq!(a.quick_value(QuickRow::ShelfMode), Some(QuickValue::All));
    press(&mut a, Btn::Right);
    assert_eq!(
        a.quick_value(QuickRow::ShelfMode),
        Some(QuickValue::Collections)
    );
    assert_eq!(read_slot_state(d.path()).shelf_mode, ShelfMode::Collections);
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::ShelfMode), Some(QuickValue::All));
}

fn on_puzzlers(d: &TempDir) -> App {
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    a
}

fn hold_y(a: &mut App) {
    common::hold(a, Btn::Y, slot_input::FAVORITE_HOLD_MS);
}

#[test]
fn holding_y_on_a_collection_opens_remove() {
    let d = card();
    let mut a = on_puzzlers(&d);
    assert_eq!(a.selected_stem(), Some("Apotris"));
    hold_y(&mut a);
    assert_eq!(a.removing(), Some(("Puzzlers", false)));
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
}

#[test]
fn remove_takes_it_out_and_centers_the_next_game() {
    let d = card();
    let mut a = on_puzzlers(&d);
    hold_y(&mut a);
    press(&mut a, Btn::A);
    assert!(a.edit().is_none());
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gb/Catrap\n");
    assert_eq!(a.toast(), Some(Toast::Collection));
    assert_eq!(a.collection_said().unwrap().1, "Removed from Puzzlers");
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    assert_eq!(a.shelf_stems(), ["Catrap"]);
    assert_eq!(a.selected_stem(), Some("Catrap"));
    assert_eq!(
        read_slot_state(d.path()).favorites,
        [game(Platform::Gba, "Drill Dozer")]
    );
}

#[test]
fn removing_the_last_cart_on_a_shelf_centers_the_first() {
    let d = card();
    let mut a = on_puzzlers(&d);
    press(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Catrap"));
    hold_y(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\n");
    assert_eq!(a.selected_stem(), Some("Apotris"));
}

#[test]
fn keep_changes_nothing() {
    let d = card();
    let mut a = on_puzzlers(&d);
    hold_y(&mut a);
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
    assert_eq!(a.selected_stem(), Some("Apotris"));
}

#[test]
fn tapping_y_on_a_collection_favorites_and_never_removes() {
    let d = card();
    let mut a = on_puzzlers(&d);
    press(&mut a, Btn::Y);
    a.flush_state();
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
    assert!(read_slot_state(d.path())
        .favorites
        .contains(&game(Platform::Gba, "Apotris")));
    assert!(a.edit().is_none());
}

#[test]
fn y_elsewhere_still_favorites() {
    let d = card();
    let mut a = boot_with(d.path(), one_favorite());
    go_to(&mut a, "Game Boy");
    press(&mut a, Btn::Y);
    assert!(read_slot_state(d.path())
        .favorites
        .contains(&game(Platform::Gb, "Catrap")));
    assert_eq!(a.toast(), None);
}

#[test]
fn removing_the_last_game_deletes_the_collection() {
    let d = card();
    let mut a = on_puzzlers(&d);
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    hold_y(&mut a);
    assert_eq!(a.removing(), Some(("Dozers", true)));
    press(&mut a, Btn::A);
    assert!(!d.path().join("Collections/Dozers.txt").exists());
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\n");
    assert_eq!(a.collection_names(), ["Puzzlers"]);
    assert_eq!(a.collection_said().unwrap().1, "Removed from Dozers");
    let mut seen = Vec::new();
    for _ in 0..4 {
        press(&mut a, Btn::R2);
        seen.extend(name(&a));
    }
    assert!(!seen.iter().any(|n| n == "Dozers"), "{seen:?}");
}

#[test]
fn removing_the_last_game_stays_on_the_stack() {
    let d = card();
    let mut a = on_puzzlers(&d);
    press(&mut a, Btn::R2);
    hold_y(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
}

#[test]
fn removing_the_last_game_with_no_favorites_moves_to_the_next_collection() {
    let d = card();
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Puzzlers");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Dozers"));
    hold_y(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn removing_the_last_game_anywhere_leaves_the_stack_for_the_row() {
    let d = bare_card();
    put(d.path(), "Dozers.txt", "gba/Drill Dozer\n");
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Dozers");
    hold_y(&mut a);
    press(&mut a, Btn::A);
    let shelf = a.shelf_platform_name().map(str::to_string);
    assert!(
        matches!(shelf.as_deref(), Some("Game Boy" | "Game Boy Advance")),
        "{shelf:?}"
    );
}

#[test]
fn removing_the_last_game_in_collections_mode_goes_to_the_next_shelf() {
    let d = card();
    let mut a = boot_with(d.path(), collections_mode(one_favorite()));
    go_to(&mut a, "Dozers");
    hold_y(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
}

#[test]
fn collections_mode_never_boots_onto_a_hidden_system_shelf() {
    let d = card();
    let a = boot_with(
        d.path(),
        SlotState {
            shelf: Some("gba".into()),
            shelf_cart: Some(game(Platform::Gba, "Apotris")),
            ..collections_mode(one_favorite())
        },
    );
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
}

fn mixed() -> TempDir {
    let d = tmp_root_with_carts(&["Apotris", "Drill Dozer", "Fusion", "Boktai"]);
    write_gb_cart(&d, "Catrap", "CATRAP");
    put(
        d.path(),
        "Mixed.txt",
        "gba/Apotris\ngba/Drill Dozer\ngba/Fusion\ngba/Boktai\ngb/Catrap\n",
    );
    d
}

fn on_mixed(d: &TempDir) -> App {
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Mixed");
    assert_eq!(a.selected_stem(), Some("Apotris"));
    a
}

fn settle(a: &mut App, ms: u64) {
    for _ in 0..ms / 10 {
        a.update(0.010);
    }
}

fn hold_dir(a: &mut App, btn: Btn, ms: u64) {
    a.apply(slot_input::Action::GbaDown(btn));
    settle(a, ms);
    a.apply(slot_input::Action::GbaUp(btn));
}

const MIXED: [&str; 5] = ["Apotris", "Drill Dozer", "Fusion", "Boktai", "Catrap"];

#[test]
fn x_on_a_collection_picks_up_at_once() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::X);
    assert_eq!(a.moving(), Some("Move".to_string()));
}

#[test]
fn a_tap_right_moves_one_spot() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::X);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    assert_eq!(
        a.shelf_stems(),
        ["Drill Dozer", "Apotris", "Fusion", "Boktai", "Catrap"]
    );
    assert_eq!(a.selected_stem(), Some("Apotris"));
}

#[test]
fn holding_right_keeps_moving_then_settles_on_the_next_spot() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::X);
    a.apply(slot_input::Action::GbaDown(Btn::Right));
    settle(&mut a, 600);
    assert_eq!(a.shelf_stems()[1], "Apotris");
    a.apply(slot_input::Action::GbaUp(Btn::Right));
    settle(&mut a, 1000);
    assert_eq!(
        a.shelf_stems(),
        ["Drill Dozer", "Fusion", "Apotris", "Boktai", "Catrap"]
    );
    assert_eq!(a.selected_stem(), Some("Apotris"));
}

#[test]
fn moving_past_the_end_wraps_to_the_front() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::Left);
    assert_eq!(a.selected_stem(), Some("Catrap"));
    press(&mut a, Btn::X);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    assert_eq!(a.shelf_stems()[0], "Catrap");
    assert_eq!(a.selected_stem(), Some("Catrap"));
}

#[test]
fn a_drops_and_writes_the_order() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::X);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    assert_eq!(
        read(d.path(), "Mixed.txt"),
        "gba/Apotris\ngba/Drill Dozer\ngba/Fusion\ngba/Boktai\ngb/Catrap\n"
    );
    press(&mut a, Btn::A);
    assert!(a.edit().is_none());
    assert_eq!(
        read(d.path(), "Mixed.txt"),
        "gba/Drill Dozer\ngba/Apotris\ngba/Fusion\ngba/Boktai\ngb/Catrap\n"
    );
}

#[test]
fn b_puts_it_back() {
    let d = mixed();
    let mut a = on_mixed(&d);
    press(&mut a, Btn::X);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(a.shelf_stems(), MIXED);
    assert_eq!(a.selected_stem(), Some("Apotris"));
    assert_eq!(
        read(d.path(), "Mixed.txt"),
        "gba/Apotris\ngba/Drill Dozer\ngba/Fusion\ngba/Boktai\ngb/Catrap\n"
    );
}

fn four() -> TempDir {
    let d = bare_card();
    put(d.path(), "Racing.txt", "gba/Drill Dozer\n");
    put(d.path(), "Empty.txt", "");
    put(d.path(), "Puzzlers.txt", "gba/Apotris\ngb/Catrap\n");
    put(d.path(), "Dozers.txt", "gba/Drill Dozer\n");
    put(d.path(), "order.txt", "Racing\nEmpty\nPuzzlers\nDozers\n");
    d
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

fn hold_x(a: &mut App) {
    common::hold(a, Btn::X, slot_input::FAVORITE_HOLD_MS);
}

#[test]
fn holding_x_moves_the_collection() {
    let d = four();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Puzzlers");
    hold_x(&mut a);
    assert_eq!(a.moving(), Some("Move Puzzlers".to_string()));
}

#[test]
fn releasing_x_keeps_the_collection_move_open() {
    let d = four();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Puzzlers");
    hold_x(&mut a);
    settle(&mut a, 500);
    assert_eq!(a.moving(), Some("Move Puzzlers".to_string()));
}

#[test]
fn a_rewrites_order_and_stays_on_the_moved_collection() {
    let d = four();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Puzzlers");
    hold_x(&mut a);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    press(&mut a, Btn::A);
    assert!(a.edit().is_none());
    assert_eq!(
        read(d.path(), "order.txt"),
        "Racing\nEmpty\nDozers\nPuzzlers\n"
    );
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    assert_eq!(a.shelf_stems(), ["Apotris", "Catrap"]);
}

#[test]
fn an_empty_collection_keeps_its_place_across_the_wrap() {
    let d = four();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Racing");
    hold_x(&mut a);
    hold_dir(&mut a, Btn::Left, 20);
    settle(&mut a, 1000);
    press(&mut a, Btn::A);
    assert_eq!(
        read(d.path(), "order.txt"),
        "Dozers\nPuzzlers\nRacing\nEmpty\n"
    );
}

#[test]
fn collections_mode_row_follows_the_new_order() {
    let d = four();
    let mut a = boot_with(d.path(), collections_mode(one_favorite()));
    go_to(&mut a, "Puzzlers");
    hold_x(&mut a);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    press(&mut a, Btn::A);
    let seen = row(&mut a);
    assert_eq!(seen[..3], ["Favorites", "Racing", "Dozers"], "{seen:?}");
}

#[test]
fn b_restores_the_collection_order() {
    let d = four();
    let mut a = boot_with(d.path(), one_favorite());
    flip_to(&mut a, "Puzzlers");
    hold_x(&mut a);
    hold_dir(&mut a, Btn::Right, 20);
    settle(&mut a, 1000);
    press(&mut a, Btn::B);
    assert!(a.edit().is_none());
    assert_eq!(
        read(d.path(), "order.txt"),
        "Racing\nEmpty\nPuzzlers\nDozers\n"
    );
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
}

#[test]
fn the_old_select_chords_do_nothing_on_a_collection() {
    let d = card();
    let mut a = on_puzzlers(&d);
    for btn in [Btn::L1, Btn::R1, Btn::L2, Btn::R2] {
        common::chord(&mut a, Btn::Select, btn);
    }
    assert_eq!(read(d.path(), "Puzzlers.txt"), "gba/Apotris\ngb/Catrap\n");
    assert_eq!(read(d.path(), "order.txt"), "Puzzlers\nDozers\n");
}

#[test]
fn holding_a_direction_in_a_one_game_collection_never_moves_the_cart() {
    let d = bare_card();
    put(d.path(), "Solo.txt", "gba/Apotris\n");
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Solo");
    press(&mut a, Btn::X);
    a.apply(slot_input::Action::GbaDown(Btn::Right));
    settle(&mut a, 600);
    let off = a.move_offset().expect("not moving");
    assert!(off.abs() < 1e-4, "the lone cart drifted {off}");
}

fn wanted(a: &App) -> Vec<(usize, usize)> {
    a.face_wants().iter().map(|w| (w.shelf, w.index)).collect()
}

fn big_card() -> TempDir {
    let stems: Vec<String> = (0..30).map(|i| format!("Game {i:02}")).collect();
    let refs: Vec<&str> = stems.iter().map(String::as_str).collect();
    let d = tmp_root_with_carts(&refs);
    put(
        d.path(),
        "Puzzlers.txt",
        "gba/Game 15\ngba/Game 16\ngba/Game 17\ngba/Game 18\n",
    );
    put(d.path(), "order.txt", "Puzzlers\n");
    d
}

#[test]
fn keep_holds_on_to_the_faces_the_remove_screen_drew() {
    let d = big_card();
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Puzzlers");
    hold_y(&mut a);
    assert_eq!(a.removing(), Some(("Puzzlers", false)));
    let during = wanted(&a);
    press(&mut a, Btn::B);
    assert_eq!(a.removing(), None);
    let after = wanted(&a);
    let lost: Vec<_> = during.iter().filter(|s| !after.contains(s)).collect();
    assert!(lost.is_empty(), "dropped {lost:?}");
}

#[test]
fn remove_holds_on_to_the_faces_the_remove_screen_drew() {
    let d = big_card();
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Puzzlers");
    hold_y(&mut a);
    let during = wanted(&a);
    press(&mut a, Btn::A);
    assert_eq!(a.removing(), None);
    let after = wanted(&a);
    let game = during
        .iter()
        .filter(|s| !after.contains(s))
        .filter_map(|&(s, i)| a.face_cart(s, i).map(|c| c.stem.clone()))
        .collect::<Vec<_>>();
    assert!(game.is_empty(), "dropped {game:?}");
}

#[test]
fn the_remove_screen_draws_carts_whose_faces_are_already_wanted() {
    let d = big_card();
    let mut a = boot_with(d.path(), SlotState::default());
    go_to(&mut a, "Puzzlers");
    let browsing = wanted(&a);
    hold_y(&mut a);
    let extra: Vec<_> = wanted(&a)
        .into_iter()
        .filter(|s| !browsing.contains(s))
        .collect();
    assert!(
        extra.is_empty(),
        "the remove screen needs new faces {extra:?}"
    );
}
