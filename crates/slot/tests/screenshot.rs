mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::App;
use slot_input::Action;
use slot_store::{Platform, ShotDir};
use slot_ui::Toast;

fn decoded(path: &std::path::Path) -> (u32, u32, png::ColorType) {
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(path).expect("open shot"),
    ));
    let reader = decoder.read_info().expect("png header");
    let info = reader.info();
    (info.width, info.height, info.color_type)
}

#[test]
fn select_x_while_playing_asks_for_a_shot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    a.apply(Action::Screenshot);
    assert!(a.shot_requested());
}

#[test]
fn select_x_before_the_game_shows_is_ignored() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Screenshot);
    assert!(
        !a.shot_requested(),
        "a shot asked for while loading waits for any game"
    );
}

#[test]
fn no_shot_from_the_menu_or_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut shelf = common::boot(d.path());
    shelf.apply(Action::Screenshot);
    assert!(!shelf.shot_requested(), "the shelf asked for a shot");

    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    assert!(a.in_game_menu().is_some());
    a.apply(Action::Screenshot);
    assert!(!a.shot_requested(), "the in-game menu asked for a shot");
}

#[test]
fn a_taken_shot_lands_in_its_folder_and_toasts() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Screenshot);
    a.shot_taken(4, 2, vec![0x80; 4 * 2 * 4]);
    a.flush_shots();
    let shots = ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .expect("list");
    assert_eq!(shots.len(), 1);
    assert_eq!(decoded(&shots[0].path), (4, 2, png::ColorType::Rgb));
    assert_eq!(a.toast(), Some(Toast::ShotCaptured));
    assert!(!a.shot_requested());
}

#[test]
fn two_shots_in_one_second_are_two_files() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.shot_taken(4, 2, vec![0x80; 4 * 2 * 4]);
    a.shot_taken(4, 2, vec![0x40; 4 * 2 * 4]);
    a.flush_shots();
    let shots = ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .expect("list");
    assert_eq!(shots.len(), 2);
    assert_ne!(shots[0].stamp, shots[1].stamp);
    assert!(shots.iter().all(|s| s.stamp.len() == 23), "{shots:?}");
}

#[test]
fn a_gallery_whose_shots_vanished_closes_and_hides_the_row() {
    let (d, mut a) = shots(2);
    a.apply(Action::QuickMenu);
    for _ in 0..6 {
        if a.in_game_menu() == Some(slot_ui::QuickRow::Screenshots) {
            break;
        }
        press(&mut a, slot_input::Btn::Down);
    }
    assert_eq!(a.in_game_menu(), Some(slot_ui::QuickRow::Screenshots));
    for e in ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap()
    {
        std::fs::remove_file(e.path).unwrap();
    }
    press(&mut a, slot_input::Btn::A);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while a.shot_viewer().is_some() {
        assert!(
            std::time::Instant::now() < until,
            "the empty gallery stayed open"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
        a.tick_ms(a.now() + 1);
    }
    assert!(matches!(a.phase(), slot::app::Phase::Playing { .. }));
    let hidden = a.in_game_hidden();
    assert!(hidden.contains(&slot_ui::QuickRow::Screenshots));
    let row = a.in_game_menu().expect("the menu closed");
    assert!(!hidden.contains(&row), "the cursor sits on hidden {row:?}");
}

fn press(a: &mut App, btn: slot_input::Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn two_shots() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.shot_taken(4, 2, vec![0x80; 32]);
    a.shot_taken(4, 2, vec![0x40; 32]);
    a.flush_shots();
    (d, a)
}

fn open_viewer(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..6 {
        if a.in_game_menu() == Some(slot_ui::QuickRow::Screenshots) {
            break;
        }
        press(a, slot_input::Btn::Down);
    }
    assert_eq!(a.in_game_menu(), Some(slot_ui::QuickRow::Screenshots));
    press(a, slot_input::Btn::A);
    assert!(a.shot_viewer().is_some(), "the viewer did not open");
    wait_listed(a);
}

fn wait_listed(a: &mut App) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while a.shot_viewer().is_some_and(|v| v.is_empty()) {
        assert!(std::time::Instant::now() < until, "the list never arrived");
        std::thread::sleep(std::time::Duration::from_millis(2));
        a.tick_ms(a.now() + 1);
    }
}

#[test]
fn screenshots_opens_newest_first_and_b_returns_to_the_row() {
    let (d, mut a) = two_shots();
    let newest = ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap()[0]
        .stamp
        .clone();
    open_viewer(&mut a);
    assert_eq!(
        a.shot_viewer()
            .and_then(|v| v.selected())
            .map(|e| e.stamp.clone()),
        Some(newest)
    );
    press(&mut a, slot_input::Btn::B);
    assert!(matches!(a.phase(), slot::app::Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(slot_ui::QuickRow::Screenshots));
}

fn shots(n: usize) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    for i in 0..n {
        a.shot_taken(4, 2, vec![i as u8; 32]);
    }
    a.flush_shots();
    (d, a)
}

fn mode(a: &App) -> Option<slot_ui::ShotMode> {
    a.shot_viewer().map(|v| v.mode())
}

fn index(a: &App) -> Option<usize> {
    a.shot_viewer().map(|v| v.index)
}

fn face(a: &mut App) {
    a.set_shot_face(Some((slot_ui::TexId::from_raw(7), 720, 480)));
}

#[test]
fn the_gallery_opens_on_the_newest_in_the_grid() {
    let (_d, mut a) = shots(3);
    open_viewer(&mut a);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Grid));
    assert_eq!(index(&a), Some(0));
}

#[test]
fn a_opens_full_and_b_goes_back_to_the_grid_then_the_menu() {
    let (_d, mut a) = shots(5);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Right);
    press(&mut a, slot_input::Btn::A);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Full));
    press(&mut a, slot_input::Btn::B);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Grid));
    assert_eq!(index(&a), Some(1));
    press(&mut a, slot_input::Btn::B);
    assert!(matches!(a.phase(), slot::app::Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(slot_ui::QuickRow::Screenshots));
}

#[test]
fn left_and_right_step_in_full_but_pan_when_zoomed() {
    let (_d, mut a) = shots(2);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::A);
    press(&mut a, slot_input::Btn::Right);
    assert_eq!(index(&a), Some(1));
    face(&mut a);
    press(&mut a, slot_input::Btn::A);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Zoomed));
    press(&mut a, slot_input::Btn::Left);
    assert_eq!(index(&a), Some(1));
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Zoomed));
    press(&mut a, slot_input::Btn::B);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Full));
}

#[test]
fn y_does_nothing_while_zoomed() {
    let (_d, mut a) = shots(1);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::A);
    face(&mut a);
    press(&mut a, slot_input::Btn::A);
    press(&mut a, slot_input::Btn::Y);
    assert!(!a.delete_prompt());
}

#[test]
fn a_held_direction_repeats_in_the_grid() {
    let (_d, mut a) = shots(7);
    open_viewer(&mut a);
    let t0 = a.now();
    a.apply(Action::GbaDown(slot_input::Btn::Right));
    assert_eq!(index(&a), Some(1));
    for t in [500, 700, 900] {
        a.tick_ms(t0 + t);
    }
    assert_eq!(index(&a), Some(4));
    a.apply(Action::GbaUp(slot_input::Btn::Right));
    a.tick_ms(t0 + 1_500);
    assert_eq!(index(&a), Some(4));
}

#[test]
fn deleting_in_full_shows_the_next_older() {
    let (d, mut a) = shots(3);
    let oldest = ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap()[2]
        .stamp
        .clone();
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::A);
    press(&mut a, slot_input::Btn::Right);
    press(&mut a, slot_input::Btn::Y);
    press(&mut a, slot_input::Btn::X);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Full));
    assert_eq!(index(&a), Some(1));
    assert_eq!(
        a.shot_viewer()
            .and_then(|v| v.selected())
            .map(|e| e.stamp.clone()),
        Some(oldest)
    );
}

#[test]
fn the_viewer_asks_for_the_selected_shot_only_outside_the_grid() {
    let (d, mut a) = shots(2);
    let list = ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap();
    open_viewer(&mut a);
    assert_eq!(a.shot_wants_full(), None);
    press(&mut a, slot_input::Btn::A);
    assert_eq!(a.shot_wants_full(), Some(list[0].path.as_path()));
    press(&mut a, slot_input::Btn::Right);
    assert_eq!(a.shot_wants_full(), Some(list[1].path.as_path()));
}

#[test]
fn deleting_the_last_shot_closes_and_hides_the_row() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.shot_taken(4, 2, vec![0x80; 32]);
    a.flush_shots();
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Y);
    assert!(a.delete_prompt());
    press(&mut a, slot_input::Btn::X);
    a.flush_shots();
    assert!(ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap()
        .is_empty());
    assert!(matches!(a.phase(), slot::app::Phase::Playing { .. }));
    let hidden = a.in_game_hidden();
    assert!(hidden.contains(&slot_ui::QuickRow::Screenshots));
    let row = a.in_game_menu().expect("the menu closed");
    assert!(!hidden.contains(&row), "the cursor sits on hidden {row:?}");
}

#[test]
fn y_then_b_keeps_the_shot() {
    let (d, mut a) = two_shots();
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Y);
    press(&mut a, slot_input::Btn::B);
    assert!(!a.delete_prompt());
    assert_eq!(
        ShotDir::new(d.path(), Platform::Gba, "Emerald")
            .list()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn settling_saves_also_writes_a_queued_shot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.shot_taken(720, 480, vec![0x80; 720 * 480 * 4]);
    a.settle_saves();
    assert_eq!(
        ShotDir::new(d.path(), Platform::Gba, "Emerald")
            .list()
            .unwrap()
            .len(),
        1,
        "power off would drop the shot still in the queue"
    );
}

#[test]
fn a_direction_held_into_full_screen_does_not_keep_stepping() {
    let (_d, mut a) = shots(5);
    open_viewer(&mut a);
    let t0 = a.now();
    a.apply(Action::GbaDown(slot_input::Btn::Right));
    press(&mut a, slot_input::Btn::A);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Full));
    for t in [500, 700, 900] {
        a.tick_ms(t0 + t);
    }
    assert_eq!(index(&a), Some(1));
}

fn listed(d: &tempfile::TempDir) -> usize {
    ShotDir::new(d.path(), Platform::Gba, "Emerald")
        .list()
        .unwrap()
        .len()
}

fn tick_at(a: &mut App, at: &[usize]) {
    for &i in at {
        while index(a).unwrap() < i {
            press(a, slot_input::Btn::Right);
        }
        press(a, slot_input::Btn::A);
    }
}

#[test]
fn select_starts_multi_select_and_b_leaves_it() {
    let (_d, mut a) = shots(3);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    assert!(a.shot_viewer().is_some_and(|v| v.selecting()));
    press(&mut a, slot_input::Btn::A);
    assert_eq!(mode(&a), Some(slot_ui::ShotMode::Grid));
    assert_eq!(a.shot_viewer().map(|v| v.ticked_count()), Some(1));
    press(&mut a, slot_input::Btn::B);
    assert!(a.shot_viewer().is_some_and(|v| !v.selecting()));
    assert!(matches!(a.phase(), slot::app::Phase::Polaroids { .. }));
    press(&mut a, slot_input::Btn::Select);
    press(&mut a, slot_input::Btn::Select);
    assert!(a.shot_viewer().is_some_and(|v| !v.selecting()));
}

#[test]
fn y_with_nothing_ticked_asks_nothing() {
    let (_d, mut a) = shots(3);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    press(&mut a, slot_input::Btn::Y);
    assert!(!a.delete_prompt());
}

#[test]
fn deleting_the_ticked_shots_removes_them_all() {
    let (d, mut a) = shots(5);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    tick_at(&mut a, &[0, 2, 3]);
    press(&mut a, slot_input::Btn::Y);
    assert!(a.delete_prompt());
    assert_eq!(a.shots_to_delete(), 3);
    press(&mut a, slot_input::Btn::X);
    a.flush_shots();
    assert_eq!(listed(&d), 2);
    assert_eq!(a.shot_viewer().map(|v| v.entries.len()), Some(2));
    assert!(a.shot_viewer().is_some_and(|v| !v.selecting()));
}

#[test]
fn b_at_the_prompt_keeps_the_ticked_shots() {
    let (d, mut a) = shots(3);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    tick_at(&mut a, &[0, 1]);
    press(&mut a, slot_input::Btn::Y);
    press(&mut a, slot_input::Btn::B);
    a.flush_shots();
    assert_eq!(listed(&d), 3);
    assert_eq!(a.shot_viewer().map(|v| v.ticked_count()), Some(2));
}

#[test]
fn deleting_every_shot_closes_and_hides_the_row() {
    let (d, mut a) = shots(4);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    tick_at(&mut a, &[0, 1, 2, 3]);
    press(&mut a, slot_input::Btn::Y);
    press(&mut a, slot_input::Btn::X);
    assert!(matches!(a.phase(), slot::app::Phase::Playing { .. }));
    assert!(a.in_game_hidden().contains(&slot_ui::QuickRow::Screenshots));
    a.flush_shots();
    assert_eq!(listed(&d), 0);
}

#[test]
fn reopening_after_a_delete_never_lists_a_deleted_shot() {
    let (_d, mut a) = shots(6);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Select);
    tick_at(&mut a, &[0, 1, 2]);
    press(&mut a, slot_input::Btn::Y);
    press(&mut a, slot_input::Btn::X);
    press(&mut a, slot_input::Btn::B);
    press(&mut a, slot_input::Btn::A);
    wait_listed(&mut a);
    assert_eq!(a.shot_viewer().map(|v| v.entries.len()), Some(3));
}

#[test]
fn each_open_has_its_own_number() {
    let (_d, mut a) = shots(2);
    open_viewer(&mut a);
    let first = a.shot_opens();
    press(&mut a, slot_input::Btn::B);
    press(&mut a, slot_input::Btn::A);
    assert!(a.shot_viewer().is_some());
    assert_eq!(a.shot_opens(), first + 1);
}

#[test]
fn the_prompt_words_one_shot_and_many() {
    let (_d, mut a) = shots(4);
    open_viewer(&mut a);
    press(&mut a, slot_input::Btn::Y);
    assert_eq!(
        a.delete_shot_lines(),
        Some((
            "Delete this screenshot?".to_string(),
            "It cannot be brought back."
        ))
    );
    press(&mut a, slot_input::Btn::B);
    press(&mut a, slot_input::Btn::Select);
    tick_at(&mut a, &[0, 1, 2]);
    press(&mut a, slot_input::Btn::Y);
    assert_eq!(
        a.delete_shot_lines(),
        Some((
            "Delete 3 screenshots?".to_string(),
            "They cannot be brought back."
        ))
    );
}

#[test]
fn opening_the_menu_clears_the_toast() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.shot_taken(4, 2, vec![0x80; 32]);
    assert_eq!(a.toast(), Some(Toast::ShotCaptured));
    a.apply(Action::QuickMenu);
    assert_eq!(a.toast(), None);
}

#[test]
fn a_shot_the_card_refused_says_it_failed() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    std::fs::create_dir_all(d.path().join("Screenshots/GBA")).unwrap();
    std::fs::write(
        d.path().join("Screenshots/GBA/Emerald"),
        b"a file, not a folder",
    )
    .unwrap();
    a.shot_taken(4, 2, vec![0x80; 32]);
    a.flush_shots();
    a.tick_ms(a.now() + 1);
    assert_eq!(a.toast(), Some(Toast::ShotFailed));
}

#[test]
fn a_pending_shot_is_dropped_on_eject() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    a.apply(Action::Screenshot);
    assert!(a.shot_requested());
    a.apply(Action::Eject);
    assert!(!a.shot_requested(), "the shot would fire on the next game");
}

#[test]
fn a_pending_shot_is_dropped_on_doze() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    a.apply(Action::Screenshot);
    assert!(a.shot_requested());
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), slot::app::Phase::Doze { .. }));
    a.apply(Action::PowerTap);
    assert!(!a.shot_requested(), "the shot would fire on waking");
}
