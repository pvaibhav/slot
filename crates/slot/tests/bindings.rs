mod common;

use std::path::Path;

use common::{app_playing_in, clocked, tmp_root_with_carts, write_gb_cart};
use slot::app::{App, Phase};
use slot::session::Session;
use slot_input::{Action, Btn, Millis, RawEvent};
use slot_store::{
    read_carousel, read_controls, read_shortcuts, write_carousel, write_shortcuts, Binding,
    CarouselAction, CarouselKeys, Controls, Device, GameButton, Modifier, Shortcut, ShortcutKeys,
};
use slot_ui::{QuickRow, Toast};
use tempfile::TempDir;

fn press(a: &mut App, b: Btn) {
    a.apply(Action::GbaDown(b));
    a.apply(Action::GbaUp(b));
}

fn in_game_at(a: &mut App, row: QuickRow) {
    for _ in 0..4 {
        if a.in_game_menu().is_none() {
            break;
        }
        press(a, Btn::B);
    }
    a.apply(Action::QuickMenu);
    let mut path = vec![row];
    while let Some(parent) = path.last().and_then(|r| r.parent_in_game()) {
        path.push(parent);
    }
    for (depth, step) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(a, Btn::A);
        }
        let hidden = a.in_game_hidden();
        for _ in 0..step.position(&hidden) {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.in_game_menu(), Some(row));
}

fn bind(a: &mut App, presses: &[RawEvent]) {
    press(a, Btn::A);
    assert!(a.capturing(), "A did not start waiting for a press");
    for ev in presses {
        a.capture_input(*ev);
    }
    assert!(!a.capturing(), "still waiting after {presses:?}");
}

fn playing(stem: &str) -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = app_playing_in(d.path(), stem);
    (d, a)
}

#[test]
fn a_console_button_binds_to_the_next_press_and_stays_on_its_row() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    assert_eq!(a.controls().game(GameButton::A), Binding::plain(Device::X));
    assert_eq!(a.in_game_menu(), Some(QuickRow::GameA));
}

#[test]
fn holding_menu_binds_menu_and_the_button() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameB);
    bind(&mut a, &[RawEvent::Down(Btn::Menu), RawEvent::Down(Btn::Y)]);
    assert_eq!(
        a.controls().game(GameButton::B),
        Binding::with(Modifier::Menu, Device::Y)
    );
}

#[test]
fn select_is_a_plain_button_on_controls() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameStart);
    bind(&mut a, &[RawEvent::Down(Btn::Select)]);
    assert_eq!(
        a.controls().game(GameButton::Start),
        Binding::plain(Device::Select)
    );
}

#[test]
fn x_clears_a_row_and_left_and_right_do_nothing() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Left);
    assert_eq!(a.controls(), Controls::default());
    press(&mut a, Btn::X);
    assert_eq!(a.controls().game(GameButton::A), None);
}

#[test]
fn y_asks_before_resetting_the_buttons_and_b_keeps_them() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::Y);
    assert!(a.reset_prompt());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    assert!(!a.reset_prompt());
    assert_eq!(a.in_game_menu(), Some(QuickRow::GameA));
    assert_eq!(a.controls().game(GameButton::A), Binding::plain(Device::X));
    assert_eq!(
        read_controls(d.path()).game(GameButton::A),
        Binding::plain(Device::X)
    );
}

#[test]
fn y_does_nothing_while_the_buttons_are_the_defaults() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::Y);
    assert!(!a.reset_prompt());
    assert_eq!(a.in_game_menu(), Some(QuickRow::GameA));
}

#[test]
fn a_on_the_reset_prompt_resets_every_button_and_nothing_else() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    in_game_at(&mut a, QuickRow::GameR);
    press(&mut a, Btn::X);
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::SaveState, None);
    write_shortcuts(d.path(), &keys).expect("write shortcuts");
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::Y);
    press(&mut a, Btn::A);
    assert!(!a.reset_prompt());
    assert_eq!(a.in_game_menu(), Some(QuickRow::GameA));
    assert_eq!(a.controls(), Controls::default());
    assert_eq!(read_controls(d.path()), Controls::default());
    assert_eq!(read_shortcuts(d.path()), keys);
}

#[test]
fn reset_all_on_the_shortcut_and_shelf_pages_resets_only_that_page() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfPrev));
    tap(&mut s, &mut now, Btn::X);
    tap(&mut s, &mut now, Btn::B);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
    tap(&mut s, &mut now, Btn::X);
    tap(&mut s, &mut now, Btn::Y);
    assert!(s.app().reset_prompt());
    tap(&mut s, &mut now, Btn::A);
    assert!(!s.app().reset_prompt());
    assert_eq!(read_shortcuts(d.path()), ShortcutKeys::default());
    assert_eq!(
        read_carousel(d.path()).get(CarouselAction::PreviousShelf),
        None
    );
    tap(&mut s, &mut now, Btn::B);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut s, &mut now, Btn::Up);
    }
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfPrev));
    tap(&mut s, &mut now, Btn::Y);
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(read_carousel(d.path()), CarouselKeys::default());
    assert_eq!(s.app().carousel(), CarouselKeys::default());
}

#[test]
fn a_change_in_game_is_saved_at_once_for_every_game() {
    let (d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    bind(&mut a, &[RawEvent::Down(Btn::X)]);
    assert_eq!(
        read_controls(d.path()).game(GameButton::A),
        Binding::plain(Device::X)
    );
    write_gb_cart(&d, "Tetris", "TETRIS");
    for stem in ["Fusion", "Tetris"] {
        assert_eq!(
            app_playing_in(d.path(), stem)
                .controls()
                .game(GameButton::A),
            Binding::plain(Device::X),
            "{stem}"
        );
    }
}

#[test]
fn the_in_game_settings_have_no_save_changes_page() {
    assert_eq!(
        QuickRow::SETTINGS,
        [
            QuickRow::PlayScreen,
            QuickRow::PlayGameplay,
            QuickRow::PlayControls
        ]
    );
}

#[test]
fn a_game_boy_has_no_l_or_r_rows() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    let a = app_playing_in(d.path(), "Tetris");
    let hidden = a.in_game_hidden();
    assert!(hidden.contains(&QuickRow::GameL));
    assert!(hidden.contains(&QuickRow::GameR));
}

const FRAME: Millis = 16;

fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
    *now += FRAME;
    s.feed(events.iter().copied(), *now);
    s.update(FRAME as f32 / 1000.0);
}

fn tap(s: &mut Session, now: &mut Millis, btn: Btn) {
    step(s, now, &[RawEvent::Down(btn)]);
    step(s, now, &[RawEvent::Up(btn)]);
    for _ in 0..25 {
        step(s, now, &[]);
    }
}

fn shelf_card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    clocked(d.path());
    d
}

fn on_shelf(root: &Path) -> (Session, Millis) {
    let mut s = Session::boot(root.to_path_buf());
    let mut now = 0;
    for _ in 0..30 {
        if matches!(s.app().phase(), Phase::Shelf) {
            return (s, now);
        }
        step(&mut s, &mut now, &[]);
    }
    panic!("never reached the shelf: {:?}", s.app().phase());
}

#[test]
fn the_controls_shelf_page_rebinds_shelf_switching_and_saves_at_once() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ControlsShelf));
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfPrev));
    tap(&mut s, &mut now, Btn::A);
    assert!(s.app().capturing());
    tap(&mut s, &mut now, Btn::X);
    assert!(!s.app().capturing());
    assert_eq!(
        read_carousel(d.path()).get(CarouselAction::PreviousShelf),
        Binding::plain(Device::X)
    );
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfPrev));
    tap(&mut s, &mut now, Btn::B);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ControlsShelf));
    tap(&mut s, &mut now, Btn::B);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::Controls));
    tap(&mut s, &mut now, Btn::B);
    assert!(matches!(s.app().phase(), Phase::Shelf));
    let before = s.app().shelf_platform_name().map(str::to_string);
    tap(&mut s, &mut now, Btn::L1);
    assert_eq!(
        s.app().shelf_platform_name(),
        before.as_deref(),
        "L1 still switched shelves"
    );
    tap(&mut s, &mut now, Btn::X);
    assert_ne!(
        s.app().shelf_platform_name(),
        before.as_deref(),
        "X did not switch shelves"
    );
}

#[test]
fn a_menu_chord_on_the_shelf_switches_shelves_without_opening_the_menu() {
    let d = shelf_card();
    let mut keys = CarouselKeys::default();
    keys.set(
        CarouselAction::NextShelf,
        Binding::with(Modifier::Menu, Device::R1),
    );
    write_carousel(d.path(), &keys).expect("write carousel");
    let (mut s, mut now) = on_shelf(d.path());
    let before = s.app().shelf_platform_name().map(str::to_string);
    tap(&mut s, &mut now, Btn::R1);
    assert_eq!(
        s.app().shelf_platform_name(),
        before.as_deref(),
        "plain R1 switched shelves"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Menu)]);
    tap(&mut s, &mut now, Btn::R1);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Menu)]);
    for _ in 0..25 {
        step(&mut s, &mut now, &[]);
    }
    assert_ne!(
        s.app().shelf_platform_name(),
        before.as_deref(),
        "MENU+R1 did not switch"
    );
    assert_eq!(s.app().quick_menu(), None, "the chord opened the menu");
}

#[test]
fn shortcuts_are_set_from_the_main_menu_and_saved_for_every_game_at_once() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    assert_eq!(s.app().quick_menu(), Some(QuickRow::Shortcuts));
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
    tap(&mut s, &mut now, Btn::A);
    assert!(s.app().capturing());
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    assert!(!s.app().capturing());
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
    tap(&mut s, &mut now, Btn::Down);
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    assert!(!s.app().capturing());
    let saved = read_shortcuts(d.path());
    assert_eq!(
        saved.get(Shortcut::SaveState),
        Binding::with(Modifier::Select, Device::X)
    );
    assert_eq!(
        saved.get(Shortcut::LoadState),
        Binding::plain(Device::Select)
    );
    for stem in ["Emerald", "Fusion"] {
        assert_eq!(app_playing_in(d.path(), stem).shortcuts(), saved, "{stem}");
    }
}

#[test]
fn the_in_game_settings_have_no_shortcuts_page() {
    assert!(!QuickRow::SETTINGS.contains(&QuickRow::Shortcuts));
    assert!(QuickRow::CONTROL_PAGES.contains(&QuickRow::Shortcuts));
}

#[test]
fn in_game_buttons_are_set_from_the_main_menu_and_reach_a_game() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::Up);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ControlsInGame));
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::GameUp));
    assert_eq!(
        QuickRow::GameUp.shown(&s.app().hidden_rows()),
        QuickRow::CONTROLS
    );
    for _ in 0..QuickRow::GameA.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::Y);
    assert!(!s.app().capturing());
    assert_eq!(
        read_controls(d.path()).game(GameButton::A),
        Binding::plain(Device::Y)
    );
    tap(&mut s, &mut now, Btn::B);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ControlsInGame));
    assert_eq!(
        app_playing_in(d.path(), "Tetris")
            .controls()
            .game(GameButton::A),
        Binding::plain(Device::Y)
    );
}

#[test]
fn turbo_b_moves_off_y_from_the_in_game_page() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::Up);
    tap(&mut s, &mut now, Btn::A);
    for _ in 0..QuickRow::GameTurboB.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    assert_eq!(s.app().quick_menu(), Some(QuickRow::GameTurboB));
    tap(&mut s, &mut now, Btn::A);
    assert!(s.app().capturing());
    tap(&mut s, &mut now, Btn::L2);
    assert!(!s.app().capturing());
    assert_eq!(
        read_controls(d.path()).game(GameButton::TurboB),
        Binding::plain(Device::L2)
    );
}

fn to_shortcuts(s: &mut Session, now: &mut Millis) {
    tap(s, now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(s, now, Btn::Down);
    }
    tap(s, now, Btn::A);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(s, now, Btn::Down);
    }
    assert_eq!(s.app().quick_menu(), Some(QuickRow::Shortcuts));
    tap(s, now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
}

#[test]
fn a_shortcut_binds_any_two_buttons_held_together() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    to_shortcuts(&mut s, &mut now);
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::R1)]);
    assert!(
        s.app().capturing(),
        "the first button of a pair was bound alone"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::L1)]);
    assert!(!s.app().capturing());
    step(&mut s, &mut now, &[RawEvent::Up(Btn::L1)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::R1)]);
    assert_eq!(
        read_shortcuts(d.path()).get(Shortcut::SaveState),
        Binding::pair(Device::L1, Device::R1)
    );
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScSave));
}

#[test]
fn a_shortcut_binds_one_button_on_its_release() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    to_shortcuts(&mut s, &mut now);
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Start)]);
    assert!(s.app().capturing());
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Start)]);
    assert!(!s.app().capturing());
    assert_eq!(
        read_shortcuts(d.path()).get(Shortcut::SaveState),
        Binding::plain(Device::Start)
    );
}

#[test]
fn a_console_button_still_binds_on_the_press() {
    let (_d, mut a) = playing("Emerald");
    in_game_at(&mut a, QuickRow::GameA);
    press(&mut a, Btn::A);
    a.capture_input(RawEvent::Down(Btn::L1));
    assert!(!a.capturing(), "a console button waited for a second press");
    assert_eq!(a.controls().game(GameButton::A), Binding::plain(Device::L1));
}

#[test]
fn a_shelf_key_pair_switches_shelves_only_with_both_held() {
    let d = shelf_card();
    let mut keys = CarouselKeys::default();
    keys.set(
        CarouselAction::NextShelf,
        Binding::pair(Device::X, Device::R2),
    );
    write_carousel(d.path(), &keys).expect("write carousel");
    let (mut s, mut now) = on_shelf(d.path());
    let before = s.app().shelf_platform_name().map(str::to_string);
    tap(&mut s, &mut now, Btn::R2);
    assert_eq!(s.app().shelf_platform_name(), before.as_deref(), "R2 alone switched");
    step(&mut s, &mut now, &[RawEvent::Down(Btn::R2)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::X)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::R2)]);
    for _ in 0..25 {
        step(&mut s, &mut now, &[]);
    }
    assert_ne!(s.app().shelf_platform_name(), before.as_deref(), "R2+X did not switch");
}

#[test]
fn a_shortcut_pair_reaches_the_gestures_as_a_pair() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::SaveState, Binding::pair(Device::R1, Device::L1));
    let table =
        slot::input::Table::new(&Controls::default(), &keys, &CarouselKeys::default(), true);
    assert!(table.entries().iter().any(|e| e.chord
        == slot_input::Chord {
            modifier: Some(slot_input::Mod::Button(Btn::L1)),
            button: Btn::R1,
        }
        && e.action == Action::SaveState));
}

fn bind_save_state(presses: &[RawEvent]) -> (TempDir, Session) {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    to_shortcuts(&mut s, &mut now);
    tap(&mut s, &mut now, Btn::A);
    for ev in presses {
        step(&mut s, &mut now, &[*ev]);
    }
    assert!(!s.app().capturing());
    (d, s)
}

#[test]
fn a_shortcut_on_a_console_button_names_it() {
    let (_d, s) = bind_save_state(&[RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)]);
    assert_eq!(s.app().toast(), Some(Toast::Duplicate(QuickRow::GameA)));
    assert_eq!(
        Toast::Duplicate(QuickRow::GameA).text(),
        "Same as Console A Button"
    );
}

#[test]
fn a_start_pair_clashes_only_with_a_single_on_start() {
    let mut keys = ShortcutKeys::default();
    keys.set(
        Shortcut::PaletteBack,
        Binding::pair(Device::Start, Device::L2),
    );
    keys.set(
        Shortcut::PaletteNext,
        Binding::pair(Device::Start, Device::R2),
    );
    keys.set(Shortcut::FastForward, Binding::plain(Device::L2));
    keys.set(Shortcut::Rewind, Binding::plain(Device::R2));
    let table =
        slot::input::Table::new(&Controls::default(), &keys, &CarouselKeys::default(), true);
    for row in [
        QuickRow::ScPaletteBack,
        QuickRow::ScPaletteNext,
        QuickRow::ScFastForward,
        QuickRow::ScRewind,
    ] {
        assert_eq!(
            table.clash(row),
            None,
            "{row:?}: L2 or R2 alone never beats START first"
        );
    }
    keys.set(Shortcut::Rewind, Binding::plain(Device::Start));
    let table =
        slot::input::Table::new(&Controls::default(), &keys, &CarouselKeys::default(), true);
    assert!(
        table.clash(QuickRow::ScRewind).is_some(),
        "START alone does block a START pair"
    );
}

#[test]
fn an_unordered_pair_still_clashes_with_a_single_on_either_button() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::SaveState, Binding::pair(Device::L1, Device::R1));
    keys.set(Shortcut::Rewind, Binding::plain(Device::R1));
    let table =
        slot::input::Table::new(&Controls::default(), &keys, &CarouselKeys::default(), true);
    assert_eq!(table.clash(QuickRow::ScSave), Some(QuickRow::ScRewind));
}

#[test]
fn a_pair_of_console_buttons_is_not_a_clash() {
    let (_d, s) = bind_save_state(&[
        RawEvent::Down(Btn::L1),
        RawEvent::Down(Btn::R1),
        RawEvent::Up(Btn::R1),
        RawEvent::Up(Btn::L1),
    ]);
    assert_eq!(s.app().toast(), None);
}

#[test]
fn a_pair_holding_another_shortcuts_button_names_it() {
    let (_d, s) = bind_save_state(&[
        RawEvent::Down(Btn::R2),
        RawEvent::Down(Btn::A),
        RawEvent::Up(Btn::A),
        RawEvent::Up(Btn::R2),
    ]);
    assert_eq!(
        s.app().toast(),
        Some(Toast::Duplicate(QuickRow::ScFastForward))
    );
    assert_eq!(
        Toast::Duplicate(QuickRow::ScFastForward).text(),
        "Same as Fast Forward"
    );
}

#[test]
fn the_default_bindings_flag_nothing() {
    let d = shelf_card();
    let (s, _) = on_shelf(d.path());
    for row in QuickRow::ALL.into_iter().filter(|r| r.binds()) {
        assert_eq!(s.app().clash(row), None, "{row:?} is flagged by default");
    }
}

#[test]
fn a_shelf_key_is_only_checked_against_shelf_keys() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::Down);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfNext));
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::B)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::B)]);
    assert_eq!(s.app().toast(), None, "the console B counted as a clash");
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ShelfNext));
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::L1)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::L1)]);
    assert_eq!(s.app().toast(), Some(Toast::Duplicate(QuickRow::ShelfPrev)));
    assert_eq!(
        read_carousel(d.path()).get(CarouselAction::NextShelf),
        Binding::plain(Device::L1),
        "a clash was not saved"
    );
}

#[test]
fn a_shelf_key_led_by_start_needs_start_first() {
    let mut keys = CarouselKeys::default();
    keys.set(
        CarouselAction::Favorite,
        Binding::pair(Device::Start, Device::A),
    );
    let table =
        slot::input::Table::new(&Controls::default(), &ShortcutKeys::default(), &keys, true);
    let shelf = || {
        let mut g = slot_input::Gestures::new();
        g.set_entries(table.entries());
        g.set_context(slot_input::Context::Shelf);
        g
    };
    let mut g = shelf();
    g.feed(RawEvent::Down(Btn::Start), 0);
    assert!(g
        .feed(RawEvent::Down(Btn::A), 40)
        .contains(&Action::Favorite));
    let mut g = shelf();
    g.feed(RawEvent::Down(Btn::A), 0);
    assert!(!g
        .feed(RawEvent::Down(Btn::Start), 40)
        .contains(&Action::Favorite));
}

fn shelf_with(keys: ShortcutKeys) -> (TempDir, Session, Millis) {
    let d = shelf_card();
    write_shortcuts(d.path(), &keys).expect("write shortcuts");
    let (s, now) = on_shelf(d.path());
    (d, s, now)
}

fn chord(s: &mut Session, now: &mut Millis, first: Btn, second: Btn) {
    step(s, now, &[RawEvent::Down(first)]);
    step(s, now, &[RawEvent::Down(second)]);
    step(s, now, &[RawEvent::Up(second)]);
    step(s, now, &[RawEvent::Up(first)]);
    for _ in 0..25 {
        step(s, now, &[]);
    }
}

#[test]
fn a_brightness_pair_on_the_shoulders_works_on_the_shelf_without_switching() {
    let mut keys = ShortcutKeys::default();
    keys.set(
        Shortcut::BrightnessUp,
        Binding::pair(Device::L1, Device::R1),
    );
    let (_d, mut s, mut now) = shelf_with(keys);
    let (shelf, bright) = (
        s.app().shelf_platform_name().map(str::to_string),
        s.app().brightness(),
    );
    chord(&mut s, &mut now, Btn::L1, Btn::R1);
    assert_eq!(s.app().brightness(), bright + 1);
    assert_eq!(
        s.app().shelf_platform_name(),
        shelf.as_deref(),
        "the chord switched shelves"
    );
    tap(&mut s, &mut now, Btn::L1);
    assert_ne!(
        s.app().shelf_platform_name(),
        shelf.as_deref(),
        "L1 alone no longer switches"
    );
}

#[test]
fn start_up_raises_brightness_on_the_shelf_and_start_alone_still_opens_the_chip() {
    let mut keys = ShortcutKeys::default();
    keys.set(
        Shortcut::BrightnessUp,
        Binding::pair(Device::Start, Device::Up),
    );
    let (_d, mut s, mut now) = shelf_with(keys);
    let bright = s.app().brightness();
    chord(&mut s, &mut now, Btn::Start, Btn::Up);
    assert_eq!(s.app().brightness(), bright + 1);
    assert!(s.app().core_picker().is_none(), "START+Up opened the chip");
    tap(&mut s, &mut now, Btn::Start);
    assert!(s.app().core_picker().is_some(), "START alone did nothing");
}

#[test]
fn brightness_on_a_shelf_key_does_both_on_the_shelf_and_is_flagged() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::BrightnessUp, Binding::plain(Device::R1));
    keys.set(Shortcut::BrightnessDown, Binding::plain(Device::L1));
    let (_d, mut s, mut now) = shelf_with(keys);
    let (shelf, bright) = (
        s.app().shelf_platform_name().map(str::to_string),
        s.app().brightness(),
    );
    tap(&mut s, &mut now, Btn::R1);
    assert_eq!(
        s.app().brightness(),
        bright + 1,
        "R1 did not raise brightness"
    );
    assert_ne!(
        s.app().shelf_platform_name(),
        shelf.as_deref(),
        "R1 did not switch shelves"
    );
    tap(&mut s, &mut now, Btn::L1);
    assert_eq!(s.app().brightness(), bright, "L1 did not lower brightness");
    assert_eq!(
        s.app().clash(QuickRow::ScBrightUp),
        Some(QuickRow::ShelfNext)
    );
    assert_eq!(
        s.app().clash(QuickRow::ShelfNext),
        Some(QuickRow::ScBrightUp)
    );
}

#[test]
fn with_turbo_off_its_rows_are_hidden_and_never_flagged() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::SaveState, Binding::plain(Device::X));
    let d = shelf_card();
    write_shortcuts(d.path(), &keys).expect("write shortcuts");
    let (s, _) = on_shelf(d.path());
    assert_eq!(s.app().clash(QuickRow::ScSave), Some(QuickRow::GameTurboA));
    let mut state = slot_store::read_slot_state(d.path());
    state.turbo = false;
    slot_store::write_slot_state(d.path(), &state).expect("write slot.state");
    let (s, _) = on_shelf(d.path());
    for row in [QuickRow::GameTurboA, QuickRow::GameTurboB] {
        assert!(s.app().hidden_rows().contains(&row), "{row:?} is shown");
        assert_eq!(s.app().clash(row), None);
    }
    assert_eq!(s.app().clash(QuickRow::ScSave), None);
}

#[test]
fn select_x_in_game_is_the_screenshot_action() {
    let table = slot::input::Table::new(
        &Controls::default(),
        &ShortcutKeys::default(),
        &CarouselKeys::default(),
        true,
    );
    assert!(table.entries().iter().any(|e| e.chord
        == slot_input::Chord {
            modifier: Some(slot_input::Mod::Select),
            button: Btn::X,
        }
        && e.action == Action::Screenshot));
}

#[test]
fn binding_fast_forward_toggle_to_r2_takes_it_from_fast_forward_without_a_toast() {
    let d = shelf_card();
    let (mut s, mut now) = on_shelf(d.path());
    to_shortcuts(&mut s, &mut now);
    for _ in 0..QuickRow::ScFastForwardToggle.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    assert_eq!(s.app().quick_menu(), Some(QuickRow::ScFastForwardToggle));
    tap(&mut s, &mut now, Btn::A);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::R2)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::R2)]);
    assert!(!s.app().capturing());
    assert_eq!(s.app().toast(), None);
    let keys = read_shortcuts(d.path());
    assert_eq!(
        keys.get(Shortcut::FastForwardToggle),
        Binding::plain(Device::R2)
    );
    assert_eq!(keys.get(Shortcut::FastForward), None);
}

#[test]
fn a_collection_binding_that_matches_a_shelf_binding_is_flagged() {
    let d = shelf_card();
    let mut keys = CarouselKeys::default();
    keys.set(CarouselAction::NextCollection, Binding::plain(Device::R1));
    write_carousel(d.path(), &keys).expect("write carousel");
    let (s, _) = on_shelf(d.path());
    assert_eq!(s.app().clash(QuickRow::CollNext), Some(QuickRow::ShelfNext));
}

#[test]
fn collection_defaults_reach_the_shelf_as_actions() {
    let d = shelf_card();
    let (s, _) = on_shelf(d.path());
    let entries = s.app().binding_table().entries();
    let next = entries
        .iter()
        .find(|e| e.action == Action::NextCollection)
        .expect("no Next entry");
    assert_eq!(
        next.chord,
        slot_input::Chord {
            modifier: None,
            button: Btn::R2,
        }
    );
    assert_eq!(next.contexts, slot_input::Contexts::SHELF);
    assert!(!entries.iter().any(|e| e.chord.modifier == Some(slot_input::Mod::Select)
        && e.contexts == slot_input::Contexts::SHELF));
    for row in QuickRow::COLLECTION_KEYS {
        assert_eq!(s.app().clash(row), None, "{row:?} clashes by default");
    }
}

#[test]
fn reset_all_on_the_collections_page_leaves_the_shelf_page_alone() {
    let d = shelf_card();
    let mut keys = CarouselKeys::default();
    keys.set(CarouselAction::PreviousShelf, Binding::plain(Device::X));
    keys.set(CarouselAction::NextCollection, Binding::plain(Device::Y));
    write_carousel(d.path(), &keys).expect("write carousel");
    let (mut s, mut now) = on_shelf(d.path());
    tap(&mut s, &mut now, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut s, &mut now, Btn::Down);
    }
    tap(&mut s, &mut now, Btn::A);
    tap(&mut s, &mut now, Btn::Down);
    tap(&mut s, &mut now, Btn::A);
    assert_eq!(s.app().quick_menu(), Some(QuickRow::CollPrev));
    tap(&mut s, &mut now, Btn::Y);
    assert!(s.app().reset_prompt());
    tap(&mut s, &mut now, Btn::A);
    let back = read_carousel(d.path());
    assert_eq!(
        back.get(CarouselAction::NextCollection),
        CarouselKeys::default().get(CarouselAction::NextCollection)
    );
    assert_eq!(
        back.get(CarouselAction::PreviousShelf),
        Binding::plain(Device::X)
    );
}
