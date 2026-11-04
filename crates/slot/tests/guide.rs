mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_guide_spot, GuideSpot};
use slot_ui::QuickRow;

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn with_guide(lines: usize) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let dir = d.path().join("Guides/GBA");
    std::fs::create_dir_all(&dir).unwrap();
    let text: String = (0..lines).map(|i| format!("line {i}\n")).collect();
    std::fs::write(dir.join("Emerald.txt"), text).unwrap();
    let a = app_playing_in(d.path(), "Emerald");
    (d, a)
}

fn to_row(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    for _ in 0..10 {
        if a.in_game_menu() == Some(row) {
            return;
        }
        press(a, Btn::Down);
    }
    panic!("{row:?} never came up, at {:?}", a.in_game_menu());
}

fn top(a: &App) -> u32 {
    a.guide().expect("the guide is open").spot().line
}

#[test]
fn a_game_with_no_guide_hides_the_row() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::Guide));
}

#[test]
fn the_guide_row_opens_the_guide_and_y_returns_to_the_row() {
    let (_d, mut a) = with_guide(200);
    to_row(&mut a, QuickRow::Guide);
    press(&mut a, Btn::A);
    assert!(a.guide().is_some());
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    assert_eq!(a.in_game_menu(), None);
    press(&mut a, Btn::Y);
    assert!(a.guide().is_none());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(QuickRow::Guide));
}

#[test]
fn cheats_and_a_guide_can_be_used_in_the_same_game() {
    let (d, mut a) = with_guide(200);
    let code = "02000000 0001";
    a.set_cheats(vec![slot::cheats::Cheat {
        name: "Test cheat".into(),
        code: code.into(),
        on: false,
    }]);
    to_row(&mut a, QuickRow::Cheats);
    press(&mut a, Btn::A);
    assert_eq!(a.in_game_menu(), Some(QuickRow::CheatList));
    press(&mut a, Btn::Right);
    assert_eq!(a.take_cheat_codes(), Some(vec![code.into()]));
    press(&mut a, Btn::B);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Cheats));
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Guide));
    press(&mut a, Btn::A);
    assert!(a.guide().is_some());
    press(&mut a, Btn::Y);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Guide));
    assert_eq!(
        std::fs::read_to_string(d.path().join("Config/Cheats/GBA/Emerald.txt"))
            .unwrap()
            .trim(),
        "Test cheat"
    );
}

#[test]
fn the_shortcut_opens_and_closes_the_guide_straight_to_the_game() {
    let (_d, mut a) = with_guide(200);
    a.apply(Action::Guide);
    assert!(a.guide().is_some());
    a.apply(Action::Guide);
    assert!(a.guide().is_none());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn the_shortcut_with_no_guide_does_nothing() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Guide);
    assert!(a.guide().is_none());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn the_pad_scrolls_and_the_shoulders_page() {
    let (_d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    assert_eq!(top(&a), 2);
    press(&mut a, Btn::Up);
    assert_eq!(top(&a), 1);
    let rows = a.guide().unwrap().rows(a.canvas()) as u32;
    press(&mut a, Btn::R1);
    assert_eq!(top(&a), rows);
    press(&mut a, Btn::L1);
    press(&mut a, Btn::L1);
    assert_eq!(top(&a), 0);
}

#[test]
fn the_spot_and_zoom_come_back_next_time() {
    let (d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    let zoom = a.guide().unwrap().spot().zoom;
    press(&mut a, Btn::R1);
    press(&mut a, Btn::A);
    let left = a.guide().unwrap().spot();
    assert_eq!(left.zoom, zoom + 1);
    press(&mut a, Btn::Y);
    let path = d.path().join("Guides/GBA/Emerald.txt");
    assert_eq!(read_guide_spot(d.path(), &path), Some(left));
    a.apply(Action::Guide);
    assert_eq!(a.guide().unwrap().spot(), left);
}

#[test]
fn the_guide_opens_at_its_saved_spot() {
    let (d, _) = with_guide(500);
    let path = d.path().join("Guides/GBA/Emerald.txt");
    let spot = GuideSpot {
        line: 300,
        col: 0,
        zoom: 1,
        wrap: false,
    };
    slot_store::write_guide_spot(d.path(), &path, spot).unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Guide);
    assert_eq!(a.guide().unwrap().spot(), spot);
}

#[test]
fn closing_the_lid_on_the_guide_keeps_the_spot() {
    let (d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    press(&mut a, Btn::R1);
    let left = a.guide().unwrap().spot();
    a.apply(Action::LidClose);
    assert!(a.guide().is_none());
    let path = d.path().join("Guides/GBA/Emerald.txt");
    assert_eq!(read_guide_spot(d.path(), &path), Some(left));
}

#[test]
fn powering_off_on_the_guide_keeps_the_spot() {
    let (d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    press(&mut a, Btn::R1);
    let left = a.guide().unwrap().spot();
    a.apply(Action::PowerHold);
    assert!(a.powering_off());
    let path = d.path().join("Guides/GBA/Emerald.txt");
    assert_eq!(read_guide_spot(d.path(), &path), Some(left));
}

#[test]
fn menu_shows_the_help_and_b_puts_it_away_before_leaving() {
    let (_d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    a.apply(Action::QuickMenu);
    assert!(a.guide().unwrap().help());
    press(&mut a, Btn::Down);
    assert_eq!(top(&a), 0, "the pad scrolled under the help");
    press(&mut a, Btn::Y);
    assert!(a.guide().is_some_and(|g| g.help()), "Y left the help");
    press(&mut a, Btn::B);
    assert!(a.guide().is_some_and(|g| !g.help()), "B left the guide");
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    assert!(!a.guide().unwrap().help());
    press(&mut a, Btn::Down);
    assert_eq!(top(&a), 1);
}

#[test]
fn a_double_tap_of_menu_does_not_leave_the_guide() {
    let (_d, mut a) = with_guide(500);
    a.apply(Action::Guide);
    a.apply(Action::Polaroids);
    assert!(a.guide().is_some());
}

#[test]
fn x_toggles_wrap_and_it_comes_back_next_time() {
    let (d, mut a) = with_guide(50);
    to_row(&mut a, QuickRow::Guide);
    press(&mut a, Btn::A);
    press(&mut a, Btn::X);
    assert!(a.guide().unwrap().wrap());
    press(&mut a, Btn::Y);
    let path = d.path().join("Guides/GBA/Emerald.txt");
    assert!(slot_store::read_guide_spot(d.path(), &path).unwrap().wrap);
}

#[test]
fn a_and_b_zoom_and_b_does_not_leave() {
    let (_d, mut a) = with_guide(50);
    a.apply(Action::Guide);
    let zoom = a.guide().unwrap().spot().zoom;
    press(&mut a, Btn::B);
    assert_eq!(a.guide().unwrap().spot().zoom, zoom - 1);
    press(&mut a, Btn::A);
    assert_eq!(a.guide().unwrap().spot().zoom, zoom);
}
