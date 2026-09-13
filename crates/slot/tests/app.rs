mod common;

use std::path::Path;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use slot::app::{App, Phase, EJECT_S, INSERT_S, SEATED_AT};
use slot::audio::Sfx;
use slot::emu::{EmuHandle, Speed};
use slot::session::Session;
use slot::video_mode::{video_mode_for, VideoMode, VIDEO_MODE_FILE};
use slot_gfx::WHOLE_TEXTURE;
use slot_input::{Action, Btn, RawEvent};
use slot_retro::ButtonMask;
use slot_store::{write_slot_state, Cart, Core, Platform, SlotState};
use slot_ui::{
    board_at, grown, lid_at, lid_from, on_board, opening, shelf_cart_at, Draw, Icon, Placed, TexId,
    BOARD_W, CART_H, CART_W, CHIP_H, CHIP_U, CHIP_V, CHIP_W, HINT_EDGE, HINT_H, LID_TURN, SLIDE_UP,
    SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};

fn play(a: &mut App) {
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaUp(Btn::A));
}

fn app_with_carts(stems: &[&str]) -> App {
    App::new(
        stems
            .iter()
            .map(|stem| Cart {
                platform: Platform::Gba,
                stem: (*stem).to_string(),
                rom: format!("Games/GBA/{stem}.gba").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: stem.to_uppercase(),
            })
            .collect(),
    )
}

fn is_mouth(d: &Draw) -> bool {
    match *d {
        Draw::Rect { colour, .. } => (0..3).all(|i| (colour[i] - opening()[i]).abs() < 0.001),
        _ => false,
    }
}

fn playing(stem: &str) -> App {
    let mut a = app_with_carts(&[stem, "Zzz"]);
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a
}

#[test]
fn insert_waits_for_the_core_even_after_the_animation_floor() {
    let mut a = app_with_carts(&["Emerald"]);
    a.apply(Action::Insert);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "advanced without the core"
    );
    a.on_core_ready();
    a.update(1.0 / 60.0);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn insert_does_not_advance_before_the_animation_floor_even_if_the_core_is_instant() {
    let mut a = app_with_carts(&["Emerald"]);
    a.apply(Action::Insert);
    a.on_core_ready();
    a.update(1.0 / 60.0);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
}

fn seconds_until(a: &mut App, done: fn(&App) -> bool) -> f32 {
    let mut t = 0.0;
    while !done(a) && t < 5.0 {
        a.update(1.0 / 60.0);
        t += 1.0 / 60.0;
    }
    t
}

#[test]
fn the_insert_reads_as_a_push_and_the_eject_takes_the_same_time() {
    let mut a = app_with_carts(&["Emerald", "Zzz"]);
    a.apply(Action::Insert);
    a.on_core_ready();
    let insert = seconds_until(&mut a, |a| matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::Eject);
    let eject = seconds_until(&mut a, |a| matches!(a.phase(), Phase::Shelf));
    assert!(insert >= 0.4, "insert is {insert}s, still a wipe");
    assert!(
        eject > EJECT_S,
        "the eject is {eject}s, so the cart moved before the picture was out"
    );
}

#[test]
fn the_game_does_not_appear_the_instant_the_cart_seats() {
    let mut a = app_with_carts(&["Emerald"]);
    a.apply(Action::Insert);
    a.on_core_ready();
    while a.seat() < 1.0 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "revealed on the same frame it seated"
    );
    let beat = ((INSERT_S - SEATED_AT) * 60.0).ceil() as u32 + 1;
    for _ in 0..beat {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn the_game_does_not_draw_during_the_insert() {
    let d = common::tmp_root_with_real_carts(&["Emerald", "Fusion"]);
    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A), RawEvent::Up(Btn::A)], 16);
    for i in 0..120 {
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
        if matches!(s.app().phase(), Phase::Inserting { .. }) {
            assert!(
                !s.game_visible(),
                "frame {i}: the game is playing behind the cart"
            );
        }
    }
    assert!(
        s.game_visible(),
        "the core never published, so the insert proved nothing"
    );
}

#[test]
fn the_reveal_waits_for_the_power_on_to_finish() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let mut a = App::boot(d.path());
    a.apply(Action::Insert);
    a.on_core_ready();
    while a.seat() < 1.0 {
        a.update(1.0 / 60.0);
    }
    assert!(
        a.screen_power() < 1.0,
        "the screen was already on when the cart landed"
    );
    for _ in 0..20 {
        a.update(1.0 / 60.0);
    }
    assert!((a.screen_power() - 1.0).abs() < 0.01);
}

#[test]
fn the_cart_sounds_when_it_reaches_the_slot_and_not_when_it_starts_moving() {
    let mut a = app_with_carts(&["Emerald", "Zzz"]);
    a.apply(Action::Insert);
    a.update(1.0 / 60.0);
    assert_eq!(a.take_sfx(), None, "it sounded before it touched anything");
    let mut heard = None;
    while a.seat() < 1.0 && heard.is_none() {
        a.update(1.0 / 60.0);
        heard = a.take_sfx();
    }
    assert_eq!(heard, Some(Sfx::Insert));
}

#[test]
fn a_resumed_cart_makes_no_sound() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(a.take_sfx(), None);
}

#[test]
fn the_cart_sounds_as_it_comes_free_and_not_before_the_screen_is_out() {
    let mut a = playing("Emerald");
    a.take_sfx();
    a.apply(Action::Eject);
    assert_eq!(a.take_sfx(), None, "it sounded over a live picture");
    let mut heard = None;
    for _ in 0..120 {
        a.update(1.0 / 60.0);
        if let Some(s) = a.take_sfx() {
            heard = Some(s);
            break;
        }
    }
    assert_eq!(heard, Some(Sfx::Eject));
    assert_eq!(a.screen_power(), 0.0, "the picture was still going out");
}

#[test]
fn a_cart_that_fails_to_load_returns_to_the_shelf() {
    let mut a = app_with_carts(&["Broken"]);
    a.apply(Action::Insert);
    a.on_core_failed();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn a_refused_cart_pushes_back_out_from_where_it_caught() {
    let mut a = app_with_carts(&["Broken"]);
    a.apply(Action::Insert);
    for _ in 0..6 {
        a.update(1.0 / 60.0);
    }
    let caught = a.seat();
    assert!(
        caught > 0.05 && caught < 0.95,
        "test needs a part seated cart, got {caught}"
    );
    a.on_core_failed();
    assert!(
        (a.seat() - caught).abs() < 1e-3,
        "cart jumped from {caught} to {}",
        a.seat()
    );
}

#[test]
fn an_empty_shelf_has_nothing_to_insert() {
    let mut a = app_with_carts(&[]);
    a.apply(Action::Insert);
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn face_buttons_drive_the_shelf_only_while_it_is_showing() {
    let mut a = app_with_carts(&["Emerald", "Wars"]);
    a.apply(Action::GbaDown(Btn::Right));
    play(&mut a);
    let Phase::Inserting { cart, .. } = a.phase() else {
        panic!("A on the shelf did not insert: {:?}", a.phase())
    };
    assert_eq!(cart, "Wars");

    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::GbaDown(Btn::Left));
    a.apply(Action::Eject);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::Insert);
    let Phase::Inserting { cart, .. } = a.phase() else {
        panic!("insert after eject did nothing: {:?}", a.phase())
    };
    assert_eq!(cart, "Wars", "the game's d-pad moved the shelf behind it");
}

#[test]
fn a_held_direction_walks_the_shelf_and_a_release_stops_it() {
    let mut a = app_with_carts(&["A", "B", "C", "D", "E", "F", "G"]);
    a.apply(Action::GbaDown(Btn::Right));
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::GbaUp(Btn::Right));
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    play(&mut a);
    let Phase::Inserting { cart, .. } = a.phase() else {
        panic!("A on the shelf did not insert: {:?}", a.phase())
    };
    assert_eq!(cart, "C", "one press and one repeat, then nothing");
}

fn app_with_platforms(carts: &[(Platform, &str)]) -> App {
    App::new(
        carts
            .iter()
            .map(|(platform, stem)| Cart {
                platform: *platform,
                stem: (*stem).to_string(),
                rom: format!(
                    "Games/{}/{stem}.{}",
                    platform.dir_name(),
                    platform.extensions()[0]
                )
                .into(),
                label: None,
                code: String::new(),
                shell: None,
                title: stem.to_uppercase(),
            })
            .collect(),
    )
}

fn tap(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

#[test]
fn the_shoulders_ring_over_the_shelves() {
    let mut a = app_with_platforms(&[(Platform::Gba, "Emerald"), (Platform::Gb, "Tetris")]);
    assert_eq!(a.selected_stem(), Some("Emerald"));
    tap(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Tetris"),
        "R1 did not reach the Game Boy shelf"
    );
    tap(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Emerald"),
        "the ring did not come back round"
    );
    tap(&mut a, Btn::L1);
    assert_eq!(
        a.selected_stem(),
        Some("Tetris"),
        "L1 did not reach the Game Boy shelf"
    );
}

#[test]
fn a_colour_cart_stands_on_a_shelf_of_its_own() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Emerald"),
        (Platform::Gb, "Tetris"),
        (Platform::Gbc, "Chromatic"),
    ]);
    tap(&mut a, Btn::R1);
    assert_eq!(a.selected_stem(), Some("Tetris"));
    tap(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Chromatic"),
        "the Colour cart shares the Game Boy shelf"
    );
    tap(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Emerald"),
        "three shelves did not ring back round to the first"
    );
    tap(&mut a, Btn::L1);
    assert_eq!(a.selected_stem(), Some("Chromatic"));
}

#[test]
fn every_shelf_keeps_the_cart_it_was_left_on() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Emerald"),
        (Platform::Gba, "Fusion"),
        (Platform::Gb, "Tetris"),
        (Platform::Gb, "Zelda"),
    ]);
    tap(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Fusion"));
    tap(&mut a, Btn::R1);
    assert_eq!(a.selected_stem(), Some("Tetris"));
    tap(&mut a, Btn::Right);
    assert_eq!(a.selected_stem(), Some("Zelda"));

    tap(&mut a, Btn::L1);
    assert_eq!(
        a.selected_stem(),
        Some("Fusion"),
        "the Game Boy Advance shelf forgot where it was"
    );
    tap(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Zelda"),
        "the Game Boy shelf forgot where it was"
    );
}

#[test]
fn a_shelf_comes_back_settled_where_it_was_left() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Aaa"),
        (Platform::Gba, "Bbb"),
        (Platform::Gba, "Ccc"),
        (Platform::Gb, "Tetris"),
    ]);
    let faces: Vec<TexId> = (0..4).map(|i| TexId::from_raw(700 + i)).collect();
    a.set_faces(faces.clone());
    tap(&mut a, Btn::Right);
    tap(&mut a, Btn::Right);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    let settled = tex_at(&frame(&a), faces[2])
        .expect("no face for the selected cart")
        .1;

    tap(&mut a, Btn::R1);
    assert!(
        tex_at(&frame(&a), faces[3]).is_some(),
        "the Game Boy cart has no face of its own"
    );
    tap(&mut a, Btn::L1);
    a.update(1.0 / 60.0);
    let back = tex_at(&frame(&a), faces[2])
        .expect("no face on the way back")
        .1;
    assert!(
        (back[0] - settled[0]).abs() < 1.0,
        "the row came back at {} rather than settled at {}",
        back[0],
        settled[0]
    );
}

#[test]
fn the_shoulders_do_nothing_when_there_is_one_shelf_to_be_on() {
    let mut a = app_with_platforms(&[(Platform::Gba, "Emerald"), (Platform::Gba, "Fusion")]);
    for btn in [Btn::L1, Btn::R1] {
        tap(&mut a, btn);
        assert_eq!(
            a.selected_stem(),
            Some("Emerald"),
            "{btn:?} moved a shelf with no neighbour"
        );
        assert_eq!(
            a.toast(),
            None,
            "{btn:?} said something about doing nothing"
        );
        assert_eq!(
            a.shelf_shake(),
            0.0,
            "{btn:?} refused where it should have done nothing"
        );
    }
}

#[test]
fn the_switch_changes_the_name_on_the_band_and_says_nothing() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Emerald"),
        (Platform::Gb, "Tetris"),
        (Platform::Gbc, "Chromatic"),
    ]);
    assert_eq!(
        a.shelf_platform_name(),
        Some("Game Boy Advance"),
        "the card did not open on the GBA"
    );
    for (btn, want) in [
        (Btn::R1, "Game Boy"),
        (Btn::R1, "Game Boy Color"),
        (Btn::R1, "Game Boy Advance"),
        (Btn::L1, "Game Boy Color"),
    ] {
        a.apply_at(Action::GbaDown(btn), 1_000);
        assert_eq!(
            a.shelf_platform_name(),
            Some(want),
            "{btn:?} left the wrong name on the case"
        );
        assert_eq!(a.toast(), None, "{btn:?} put a banner up over the carts");
    }
    a.update(5.0);
    assert_eq!(
        a.shelf_platform_name(),
        Some("Game Boy Color"),
        "the name faded"
    );
}

#[test]
fn a_card_on_one_shelf_names_nothing_because_there_is_nowhere_to_switch_to() {
    let mut alone = app_with_platforms(&[(Platform::Gba, "Emerald"), (Platform::Gba, "Fusion")]);
    assert_eq!(
        alone.shelf_platform_name(),
        None,
        "a card with only Game Boy Advance carts named its platform anyway"
    );
    for btn in [Btn::L1, Btn::R1] {
        alone.apply_at(Action::GbaDown(btn), 1_000);
        assert_eq!(
            alone.shelf_platform_name(),
            None,
            "{btn:?} named a shelf on a card that has one"
        );
    }

    let two = app_with_platforms(&[(Platform::Gba, "Emerald"), (Platform::Gb, "Tetris")]);
    assert_eq!(
        two.shelf_platform_name(),
        Some("Game Boy Advance"),
        "a card with two shelves did not say which one it was on"
    );
}

#[test]
fn the_carousel_opens_on_a_shelf_that_has_carts() {
    let a = app_with_platforms(&[(Platform::Gbc, "Chromatic")]);
    assert_eq!(a.selected_stem(), Some("Chromatic"));
}

#[test]
fn the_shoulders_belong_to_the_game_while_one_is_playing() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Emerald"),
        (Platform::Gba, "Fusion"),
        (Platform::Gb, "Tetris"),
    ]);
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    tap(&mut a, Btn::R1);
    assert_eq!(a.toast(), None, "the game's shoulder button named a shelf");
    assert_eq!(
        a.selected_stem(),
        Some("Emerald"),
        "the game's shoulder button switched the shelf behind it"
    );
}

#[test]
fn a_held_direction_does_not_follow_the_shelf_it_was_pressed_on() {
    let mut a = app_with_platforms(&[
        (Platform::Gba, "Aaa"),
        (Platform::Gba, "Bbb"),
        (Platform::Gba, "Ccc"),
        (Platform::Gb, "Tetris"),
    ]);
    a.apply_at(Action::GbaDown(Btn::Right), 0);
    assert_eq!(a.selected_stem(), Some("Bbb"));
    a.apply_at(Action::GbaDown(Btn::R1), 10);
    a.apply_at(Action::GbaDown(Btn::L1), 20);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(
        a.selected_stem(),
        Some("Bbb"),
        "the row walked on by itself once the shelf came back"
    );
}

#[test]
fn a_cart_in_flight_is_not_also_left_standing_on_the_shelf() {
    let mut a = app_with_carts(&["Emerald"]);
    a.apply(Action::Insert);
    a.update(0.2);
    let mut out = Vec::new();
    a.draw(&mut out);
    let carts = out
        .iter()
        .filter(|d| match **d {
            Draw::Rect { w, h, .. } | Draw::Tex { w, h, .. } | Draw::Turned { w, h, .. } => {
                w > 200.0
                    && w <= CART_W as f32 + 0.01
                    && (w / h - CART_W as f32 / CART_H as f32).abs() < 0.05
            }
            Draw::Game | Draw::Shot { .. } => false,
        })
        .count();
    assert_eq!(
        carts, 1,
        "the shelf still holds the cart the slot is taking"
    );
}

#[test]
fn the_shelf_shows_the_empty_slot() {
    let a = app_with_carts(&["Emerald", "Zzz"]);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        out.iter().any(is_mouth),
        "the shelf has no slot, so the cart has nowhere visible to go"
    );
}

#[test]
fn inserting_still_has_a_mouth_to_go_into() {
    let mut a = app_with_carts(&["Emerald"]);
    a.apply(Action::Insert);
    a.update(1.0 / 60.0);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        out.iter().any(is_mouth),
        "the cart has nothing to slide into"
    );
}

#[test]
fn eject_returns_to_the_shelf_only_once_the_cart_is_out() {
    let mut a = playing("Emerald");
    a.apply(Action::Eject);
    dark(&mut a);
    while a.seat() > 0.0 {
        assert!(
            matches!(a.phase(), Phase::Ejecting { .. }),
            "the shelf came back with the cart {} of the way in",
            a.seat()
        );
        a.update(1.0 / 60.0);
    }
    a.update(1.0 / 60.0);
    assert!(matches!(a.phase(), Phase::Shelf));
}

fn dark(a: &mut App) {
    for _ in 0..300 {
        if a.screen_power() == 0.0 {
            return;
        }
        a.update(1.0 / 60.0);
    }
    panic!("the screen never went dark");
}

fn lists_game(a: &App) -> bool {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter().any(|d| matches!(d, Draw::Game))
}

#[test]
fn the_game_layer_is_listed_only_once_the_screen_is_up() {
    let mut a = app_with_carts(&["Emerald", "Zzz"]);
    a.set_game_ready(true);
    a.apply(Action::Insert);
    a.on_core_ready();
    while a.seat() < 1.0 {
        a.update(1.0 / 60.0);
        assert!(
            !lists_game(&a),
            "the picture is drawn while the cart is still going in"
        );
    }
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    assert!(lists_game(&a), "the game never reached the draw list");
}

#[test]
fn the_cart_waits_for_the_screen_to_go_dark() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.apply(Action::Eject);
    while a.screen_power() > 0.0 {
        assert_eq!(
            a.seat(),
            1.0,
            "the cart started leaving while the screen was still lit"
        );
        a.update(1.0 / 60.0);
        assert!(a.now() < 5_000, "the screen never went dark");
    }
    for _ in 0..40 {
        a.update(1.0 / 60.0);
    }
    assert!(
        a.seat() < 1.0,
        "the cart never left once the screen was dark"
    );
}

#[test]
fn eject_is_the_insert_backwards() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.apply(Action::Eject);
    let first = a.screen_power();
    a.update(1.0 / 60.0);
    assert!(a.screen_power() < first, "the screen is not closing");
}

#[test]
fn the_quick_menu_opens_from_the_shelf_and_nowhere_else() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut s, _motor) = common::session_with_platform(d.path());

    assert!(
        matches!(s.app().phase(), slot::app::Phase::Shelf),
        "not on the shelf: {:?}",
        s.app().phase()
    );
    s.app_mut().apply(slot_input::Action::QuickMenu);
    assert!(matches!(
        s.app().phase(),
        slot::app::Phase::QuickMenu { .. }
    ));

    s.app_mut()
        .apply(slot_input::Action::GbaDown(slot_input::Btn::B));
    s.app_mut().apply(slot_input::Action::Insert);
    s.app_mut().apply(slot_input::Action::QuickMenu);
    assert!(
        !matches!(s.app().phase(), slot::app::Phase::QuickMenu { .. }),
        "a menu opened over a seated cart"
    );
}

#[test]
fn both_b_and_menu_take_the_about_screen_back_to_the_quick_menu() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    for out in [
        slot_input::Action::GbaDown(slot_input::Btn::B),
        slot_input::Action::QuickMenu,
    ] {
        let (mut s, _motor) = common::session_with_platform(d.path());
        s.app_mut().apply(slot_input::Action::QuickMenu);
        for _ in 0..slot_ui::QuickRow::About.index() {
            s.app_mut()
                .apply(slot_input::Action::GbaDown(slot_input::Btn::Down));
        }
        s.app_mut()
            .apply(slot_input::Action::GbaDown(slot_input::Btn::A));
        assert!(matches!(s.app().phase(), slot::app::Phase::About));
        s.app_mut().apply(out);
        assert_eq!(
            s.app().quick_menu(),
            Some(slot_ui::QuickRow::About),
            "{out:?} did not take the label back to the menu"
        );
    }
}

fn on_shelf(stems: &[&str]) -> (tempfile::TempDir, App) {
    booted_on_shelf(common::tmp_root_with_carts(stems))
}

fn on_shelf_gb(stems: &[&str]) -> (tempfile::TempDir, App) {
    booted_on_shelf(common::tmp_root_with_gb_carts(stems))
}

fn booted_on_shelf(d: tempfile::TempDir) -> (tempfile::TempDir, App) {
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let app = App::boot(d.path());
    (d, app)
}

fn let_it_close(app: &mut App) {
    app.update(0.4);
}

fn let_it_hop(app: &mut App) {
    app.update(0.25);
}

fn start_on(mut pressed: App, mut untouched: App) -> (Option<Core>, f32, bool) {
    fake_picker_faces(&mut pressed);
    fake_picker_faces(&mut untouched);
    pressed.apply(Action::GbaDown(Btn::Start));
    let_it_hop(&mut pressed);
    let_it_hop(&mut untouched);
    let moved = frame(&pressed) != frame(&untouched);
    (pressed.core_picker(), pressed.shelf_shake(), moved)
}

#[test]
fn start_opens_no_picker_on_a_game_boy_cart() {
    let (_d, gb) = on_shelf_gb(&["Tetris", "Zzz"]);
    let (_twin, gb_twin) = on_shelf_gb(&["Tetris", "Zzz"]);
    let (picker, shake, moved) = start_on(gb, gb_twin);
    assert_eq!(picker, None, "a Game Boy cart opened the GBA picker");
    assert_eq!(
        shake, 0.0,
        "START on a Game Boy cart was answered with a refusal shake"
    );
    assert!(!moved, "START changed what the shelf draws");

    let (_d, gba) = on_shelf(&["Emerald", "Zzz"]);
    let (_twin, gba_twin) = on_shelf(&["Emerald", "Zzz"]);
    let (picker, _, moved) = start_on(gba, gba_twin);
    assert_eq!(picker, Some(Core::Mgba), "START stopped opening the picker");
    assert!(moved, "the picker opened and the shelf drew the same frame");
}

#[test]
fn start_on_the_shelf_opens_the_core_picker_on_the_carts_current_core() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    assert_eq!(app.selected_stem(), Some("Emerald"));

    app.apply(Action::GbaDown(Btn::Start));
    assert_eq!(
        app.core_picker(),
        Some(Core::Mgba),
        "a cart with no line of its own runs the default core"
    );
    app.apply(Action::GbaDown(Btn::B));
    let_it_close(&mut app);

    slot_store::write_selected_core(d.path(), "Emerald", Core::Gpsp).unwrap();
    app.apply(Action::GbaDown(Btn::Start));
    assert_eq!(
        app.core_picker(),
        Some(Core::Gpsp),
        "the chip should start in the core the cart already uses"
    );
}

#[test]
fn choosing_a_core_writes_it_and_closes() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));

    assert_eq!(slot_store::core_for(d.path(), "Emerald"), Core::Gpsp);
    assert_eq!(
        slot_store::core_for(d.path(), "Zzz"),
        Core::Mgba,
        "the choice landed on a cart the shelf was not on"
    );
    let_it_close(&mut app);
    assert_eq!(
        app.core_picker(),
        None,
        "the picker stayed open after a choice"
    );
}

#[test]
fn b_closes_the_picker_without_writing() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::B));
    let_it_close(&mut app);

    assert_eq!(app.core_picker(), None);
    assert_eq!(
        slot_store::core_for(d.path(), "Emerald"),
        Core::Mgba,
        "backing out of the picker still changed the cart"
    );
}

#[test]
fn the_chip_goes_where_the_arrow_points_and_does_not_wrap() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));

    app.apply(Action::GbaDown(Btn::Left));
    assert_eq!(
        app.core_picker(),
        Some(Core::Mgba),
        "left from mGBA wrapped round"
    );
    assert_ne!(
        app.core_picker_chip().unwrap().shake,
        0.0,
        "a press toward the chip's own socket went unanswered"
    );
    assert_eq!(
        app.shelf_shake(),
        0.0,
        "the shelf shook as well as the chip"
    );

    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(app.core_picker(), Some(Core::Gpsp));
    let_it_hop(&mut app);
    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(
        app.core_picker(),
        Some(Core::Gpsp),
        "right from gpSP wrapped round"
    );
}

#[test]
fn a_shelf_refusal_does_not_shake_once_the_picker_takes_over() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    app.refuse();
    app.apply(Action::GbaDown(Btn::Start));
    assert_eq!(
        app.shelf_shake(),
        0.0,
        "the shelf shook under an open picker"
    );
}

#[test]
fn back_mid_hop_turns_round_and_onward_does_nothing() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.update(0.05);

    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(app.core_picker(), Some(Core::Gpsp));
    assert_eq!(
        app.core_picker_chip().unwrap().shake,
        0.0,
        "onward mid-hop was refused"
    );

    app.apply(Action::GbaDown(Btn::Left));
    assert_eq!(
        app.core_picker(),
        Some(Core::Mgba),
        "back mid-hop did not turn the chip round"
    );
}

#[test]
fn a_mid_hop_writes_where_the_chip_is_heading() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.update(0.05);
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(slot_store::core_for(d.path(), "Emerald"), Core::Gpsp);
}

#[test]
fn the_a_that_saved_does_not_start_the_cart() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));
    let_it_close(&mut app);
    assert_eq!(app.core_picker(), None);

    app.apply(Action::GbaUp(Btn::A));
    assert!(
        matches!(app.phase(), Phase::Shelf),
        "releasing the A that saved inserted the cart: {:?}",
        app.phase()
    );
}

#[test]
fn presses_during_the_close_and_keys_the_picker_does_not_use_do_nothing() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    for key in [Btn::Up, Btn::Down, Btn::Start, Btn::Select] {
        app.apply(Action::GbaDown(key));
        assert_eq!(
            app.core_picker(),
            Some(Core::Mgba),
            "{key:?} moved the chip"
        );
    }

    app.apply(Action::GbaDown(Btn::B));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));
    let_it_close(&mut app);
    assert_eq!(app.core_picker(), None);
    assert_eq!(
        slot_store::core_for(d.path(), "Emerald"),
        Core::Mgba,
        "a press during the close wrote a core"
    );
}

#[test]
fn shutting_the_lid_closes_the_picker_without_writing() {
    let (d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::LidClose);
    assert_eq!(app.core_picker(), None, "the picker survived the lid");
    assert_eq!(slot_store::core_for(d.path(), "Emerald"), Core::Mgba);
}

#[test]
fn the_picker_swallows_the_shelf_arrows() {
    let (_d, mut app) = on_shelf(&["Emerald", "Metroid Fusion"]);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::B));
    assert_eq!(
        app.selected_stem(),
        Some("Emerald"),
        "the shelf moved underneath an open picker"
    );
}

#[test]
fn opening_the_picker_lets_go_of_a_play_hold_already_armed() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    app.apply(Action::GbaDown(Btn::A));
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaUp(Btn::A));
    app.update(0.6);
    assert!(
        matches!(app.phase(), Phase::Shelf),
        "the held A inserted the cart under the open picker: {:?}",
        app.phase()
    );
}

#[test]
fn opening_the_picker_lets_go_of_a_direction_still_held() {
    let (_d, mut app) = on_shelf(&["Emerald", "Metroid Fusion", "Zzz"]);
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::Start));
    app.update(0.6);
    assert_eq!(
        app.selected_stem(),
        Some("Metroid Fusion"),
        "the shelf moved underneath the open picker"
    );
}

#[test]
fn the_picker_does_not_open_on_an_empty_shelf() {
    let (_d, mut app) = on_shelf(&[]);
    app.apply(Action::GbaDown(Btn::Start));
    assert_eq!(app.core_picker(), None);
}

#[test]
fn select_on_the_shelf_leaves_the_picker_shut_so_it_can_still_chord() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);

    app.apply(Action::GbaDown(Btn::Select));
    assert_eq!(
        app.core_picker(),
        None,
        "SELECT must stay free for brightness and blue light on the shelf"
    );
}

struct PickerFaces {
    board: TexId,
    lid: TexId,
    sockets: [TexId; 2],
    chips: [TexId; 2],
    blank: TexId,
    shadow: TexId,
    legend: [(TexId, u32); 3],
}

fn fake_boot_faces(app: &mut App) -> PickerFaces {
    let id = TexId::from_raw;
    let f = PickerFaces {
        board: id(900),
        lid: id(901),
        sockets: [id(902), id(903)],
        chips: [id(904), id(905)],
        blank: id(906),
        shadow: id(907),
        legend: [(id(908), 60), (id(909), 90), (id(910), 80)],
    };
    app.set_core_part_faces(f.sockets.to_vec(), f.chips.to_vec(), f.blank, f.shadow);
    app.set_core_legend_faces(f.legend.to_vec());
    f
}

fn fake_picker_faces(app: &mut App) -> PickerFaces {
    let f = fake_boot_faces(app);
    app.set_core_board_faces(f.board, f.lid);
    f
}

fn resting_cart(out: &[Draw]) -> f32 {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, w, .. } | Draw::Tex { x, w, .. } => Some((x, w)),
            _ => None,
        })
        .find(|(_, w)| (w - CART_W as f32).abs() < 0.01)
        .expect("no cart standing on the row")
        .0
}

fn frame(app: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    app.draw(&mut out);
    out
}

fn tex_at(out: &[Draw], want: TexId) -> Option<(usize, [f32; 4])> {
    out.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == want => Some((i, [x, y, w, h])),
        _ => None,
    })
}

fn tex_all(out: &[Draw], want: TexId) -> Vec<(usize, [f32; 4])> {
    out.iter()
        .enumerate()
        .filter_map(|(i, d)| match *d {
            Draw::Tex {
                x, y, w, h, tex, ..
            } if tex == want => Some((i, [x, y, w, h])),
            _ => None,
        })
        .collect()
}

fn turned_at(out: &[Draw], want: TexId) -> Option<(usize, [f32; 4], f32)> {
    out.iter().enumerate().find_map(|(i, d)| match *d {
        Draw::Turned {
            x,
            y,
            w,
            h,
            tex,
            turn,
            ..
        } if tex == want => Some((i, [x, y, w, h], turn)),
        _ => None,
    })
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(p, q)| (p - q).abs() < 0.01)
}

fn alpha_of(out: &[Draw], want: TexId) -> Option<f32> {
    out.iter().find_map(|d| match *d {
        Draw::Tex { tex, alpha, .. } if tex == want => Some(alpha),
        _ => None,
    })
}

fn assert_parts_on_board(out: &[Draw], f: &PickerFaces, board: Placed, when: &str) {
    let zoom = board.w / BOARD_W as f32;
    for (i, socket) in f.sockets.iter().enumerate() {
        let (x, y) = on_board(board, SOCKET_U[i], SOCKET_V);
        let (_, at) = tex_at(out, *socket).unwrap_or_else(|| panic!("a socket is missing {when}"));
        assert!(
            near(
                at,
                [
                    x.round(),
                    y.round(),
                    SOCKET_W as f32 * zoom,
                    SOCKET_H as f32 * zoom
                ]
            ),
            "socket {i} at {at:?} {when}"
        );
    }
    let (cx, cy) = on_board(board, CHIP_U[0], CHIP_V);
    let want_chip = grown(
        Placed {
            x: cx,
            y: cy,
            w: CHIP_W as f32 * zoom,
            h: CHIP_H as f32 * zoom,
        },
        TURN_PAD as f32 * zoom,
    );
    let (_, chip, _) =
        turned_at(out, f.chips[0]).unwrap_or_else(|| panic!("no seated chip {when}"));
    assert!(
        near(
            chip,
            [
                want_chip.x.round(),
                want_chip.y.round(),
                want_chip.w,
                want_chip.h
            ]
        ),
        "the chip is not riding the board {when}: {chip:?}"
    );
}

fn carts_standing(out: &[Draw], board: TexId) -> usize {
    out.iter()
        .filter(|d| match **d {
            Draw::Rect { w, .. } => (w - CART_W as f32).abs() < 0.01,
            Draw::Tex { w, tex, .. } => tex != board && (w - CART_W as f32).abs() < 0.01,
            _ => false,
        })
        .count()
}

fn assert_plain_shelf(out: &[Draw], f: &PickerFaces, when: &str) {
    assert!(
        !out.iter().any(|d| matches!(d, Draw::Turned { .. })),
        "a turned face is drawn {when}"
    );
    let picker = [
        f.board,
        f.lid,
        f.sockets[0],
        f.sockets[1],
        f.chips[0],
        f.chips[1],
        f.blank,
        f.shadow,
        f.legend[0].0,
        f.legend[1].0,
        f.legend[2].0,
    ];
    let drawn: Vec<TexId> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex { tex, .. } if picker.contains(&tex) => Some(tex),
            _ => None,
        })
        .collect();
    assert!(drawn.is_empty(), "the picker drew {drawn:?} {when}");
    assert_eq!(
        carts_standing(out, f.board),
        1,
        "the highlighted cart is not standing in the row {when}"
    );
}

fn let_it_open(app: &mut App) {
    app.update(0.5);
}

#[test]
fn the_open_cart_rests_over_the_shelf_with_its_lid_turned() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    let out = frame(&app);

    let rest = board_at(1.0);
    let (board_i, board) = tex_at(&out, f.board).expect("no board");
    assert!(
        near(board, [rest.x, rest.y, rest.w, rest.h]),
        "board at {board:?}"
    );
    let mouth = out
        .iter()
        .rposition(is_mouth)
        .expect("no slot on the shelf");
    assert!(board_i > mouth, "the board is drawn under the shelf");

    for (i, socket) in f.sockets.iter().enumerate() {
        let (x, y) = on_board(rest, SOCKET_U[i], SOCKET_V);
        let (_, at) = tex_at(&out, *socket).expect("a socket is missing");
        assert!(
            near(at, [x.round(), y.round(), SOCKET_W as f32, SOCKET_H as f32]),
            "socket {i} at {at:?}"
        );
    }

    let (chip_i, chip, chip_turn) =
        turned_at(&out, f.chips[0]).expect("the chip is not seated in mGBA");
    assert_eq!(chip_turn, 0.0, "a seated chip is tipped");
    let (cx, cy) = on_board(rest, CHIP_U[0], CHIP_V);
    let want_chip = grown(
        Placed {
            x: cx,
            y: cy,
            w: CHIP_W as f32,
            h: CHIP_H as f32,
        },
        TURN_PAD as f32,
    );
    assert!(
        near(
            chip,
            [
                want_chip.x.round(),
                want_chip.y.round(),
                want_chip.w,
                want_chip.h
            ]
        ),
        "the seated chip is not in mGBA's socket: {chip:?}"
    );
    assert!(turned_at(&out, f.blank).is_none(), "a blank chip at rest");

    let (lid_rest, turn) = lid_at(1.0);
    let want = grown(lid_rest, TURN_PAD as f32 * lid_rest.w / CART_W as f32);
    let (lid_i, lid, lid_turn) = turned_at(&out, f.lid).expect("no lid");
    assert!(
        near(lid, [want.x, want.y, want.w, want.h]),
        "lid at {lid:?}"
    );
    assert_eq!((lid_turn, turn), (LID_TURN, LID_TURN));
    assert!(lid_i > chip_i, "the chip is drawn over the lid");

    let shadows = tex_all(&out, f.shadow);
    assert_eq!(
        shadows.len(),
        1,
        "expected the lid's shadow alone, got {shadows:?}"
    );
    let (shadow_i, s) = shadows[0];
    assert!(
        (s[0] + s[2] / 2.0 - 360.0).abs() < 0.01 && (s[1] + s[3] / 2.0 - 140.0).abs() < 0.01,
        "the lid's shadow is not under it: {s:?}"
    );
    assert!(shadow_i < lid_i, "the lid's shadow is drawn over the lid");

    let [cancel, swap, choose] = f.legend;
    let at = |tex| tex_at(&out, tex).expect("a legend hint is missing").1;
    let seen = |w: u32| (w - HINT_EDGE) as f32;
    assert_eq!(at(cancel.0), [174.0, 386.0, cancel.1 as f32, HINT_H as f32]);
    assert_eq!(
        at(swap.0)[0] + seen(swap.1) / 2.0,
        360.0,
        "Swap is off the panel's centre"
    );
    assert_eq!(
        at(choose.0)[0] + seen(choose.1),
        546.0,
        "Choose does not end at the cart's edge"
    );
    assert_eq!((at(swap.0)[1], at(choose.0)[1]), (386.0, 386.0));
}

#[test]
fn the_sockets_and_the_seated_chip_rest_on_whole_pixels() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    let out = frame(&app);

    let whole = |r: [f32; 4]| r[0].fract() == 0.0 && r[1].fract() == 0.0;
    for (i, socket) in f.sockets.iter().enumerate() {
        let (_, at) = tex_at(&out, *socket).expect("a socket is missing");
        assert!(whole(at), "socket {i} is off the pixel grid at {at:?}");
    }
    let (_, chip, _) = turned_at(&out, f.chips[0]).expect("the chip is not seated in mGBA");
    assert!(
        whole(chip),
        "the seated chip is off the pixel grid at {chip:?}"
    );
}

#[test]
fn the_neighbours_dim_to_a_quarter_while_a_cart_is_open() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let faces = vec![TexId::from_raw(920), TexId::from_raw(921)];
    app.set_faces(faces.clone());
    fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    let out = frame(&app);

    let alpha = out
        .iter()
        .find_map(|d| match *d {
            Draw::Tex { tex, alpha, .. } if tex == faces[1] => Some(alpha),
            _ => None,
        })
        .expect("the neighbour is not on screen while the cart is open");
    assert!(
        (alpha - 0.25).abs() <= 0.01,
        "the neighbour's face is at {alpha}, not a quarter"
    );
}

#[test]
fn the_seated_chip_moves_to_the_socket_it_hopped_to() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    app.apply(Action::GbaDown(Btn::Right));
    let_it_hop(&mut app);
    let out = frame(&app);

    let (x, y) = on_board(board_at(1.0), CHIP_U[1], CHIP_V);
    let want = grown(
        Placed {
            x,
            y,
            w: CHIP_W as f32,
            h: CHIP_H as f32,
        },
        TURN_PAD as f32,
    );
    let (_, chip, _) = turned_at(&out, f.chips[1]).expect("the chip did not land in gpSP");
    assert!(
        near(chip, [want.x.round(), want.y.round(), want.w, want.h]),
        "the gpSP chip is not in gpSP's socket: {chip:?}"
    );
    assert!(
        turned_at(&out, f.chips[0]).is_none(),
        "mGBA's chip is still drawn once the hop lands in gpSP"
    );
}

#[test]
fn the_highlighted_cart_becomes_the_lid_rather_than_a_second_cart() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    let rest = resting_cart(&frame(&app));
    app.apply(Action::GbaDown(Btn::Start));
    let out = frame(&app);

    let standing = out
        .iter()
        .filter(|d| match **d {
            Draw::Rect { w, .. } => (w - CART_W as f32).abs() < 0.01,
            Draw::Tex { w, tex, .. } if tex != f.board => (w - CART_W as f32).abs() < 0.01,
            _ => false,
        })
        .count();
    assert_eq!(
        standing, 0,
        "the row still draws the cart whose lid is coming off"
    );

    let (_, lid, turn) = turned_at(&out, f.lid).expect("no lid");
    let want = grown(lid_from(shelf_cart_at(rest, 1.0), 0.0).0, TURN_PAD as f32);
    assert!(
        near(lid, [want.x, want.y, want.w, want.h]),
        "the lid starts at {lid:?}"
    );
    assert_eq!(turn, 0.0);

    app.update(0.0805);
    let slid = frame(&app);
    let (_, lid, turn) = turned_at(&slid, f.lid).expect("the lid vanished mid-slide");
    assert_eq!(turn, 0.0, "the lid turned while it slid");
    let shelf = shelf_cart_at(rest, 1.0);
    let want = grown(
        Placed {
            y: shelf.y - SLIDE_UP * 0.5,
            ..shelf
        },
        TURN_PAD as f32,
    );
    assert!(
        near(lid, [want.x, want.y, want.w, want.h]),
        "the lid is not half way up its slide: {lid:?}"
    );
    let (_, board) = tex_at(&slid, f.board).expect("no board mid-slide");
    assert_eq!(
        board,
        [shelf.x, shelf.y, shelf.w, shelf.h],
        "the back half moved while the front slid"
    );
    assert_eq!(
        alpha_of(&slid, f.board),
        Some(1.0),
        "the back half is not opaque under the front"
    );
    assert_parts_on_board(&slid, &f, shelf, "mid-slide");

    app.update(0.21);
    let lifting = frame(&app);
    let (_, lid, turn) = turned_at(&lifting, f.lid).expect("the lid vanished mid-lift");
    assert!(
        turn < 0.0 && turn > LID_TURN,
        "the lid is not turning: {turn}"
    );
    assert!(
        lid[1] < lid_at(0.0).0.y && lid[1] > lid_at(1.0).0.y,
        "the lid is not on its way up: {lid:?}"
    );
    let (_, [bx, by, bw, bh]) = tex_at(&lifting, f.board).expect("no board mid-lift");
    assert!(
        bw > board_at(0.0).w && bw < board_at(1.0).w,
        "the board is not growing: {bw}"
    );
    assert_eq!(
        alpha_of(&lifting, f.board),
        Some(1.0),
        "the back half faded mid-lift"
    );
    assert_parts_on_board(
        &lifting,
        &f,
        Placed {
            x: bx,
            y: by,
            w: bw,
            h: bh,
        },
        "mid-lift",
    );
}

#[test]
fn mid_hop_the_chip_is_blank_tipped_and_off_the_board() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    let (_, seated, _) = turned_at(&frame(&app), f.chips[0]).expect("no seated chip");

    app.apply(Action::GbaDown(Btn::Right));
    app.update(0.09);
    let out = frame(&app);
    let (_, flying, tip) = turned_at(&out, f.blank).expect("no chip in flight");
    assert!(tip > 0.0, "a chip heading right does not lean right");
    assert!(flying[1] < seated[1], "the chip is not off the board");
    let board = board_at(1.0);
    let left = on_board(board, CHIP_U[0], CHIP_V).0 + CHIP_W as f32 / 2.0;
    let right = on_board(board, CHIP_U[1], CHIP_V).0 + CHIP_W as f32 / 2.0;
    let mid = flying[0] + flying[2] / 2.0;
    assert!(
        mid > left && mid < right,
        "the flying chip is not between the sockets: {mid} not in ({left}, {right})"
    );
    assert_eq!(
        tex_all(&out, f.shadow).len(),
        2,
        "nothing under the chip in flight, beside the lid's own shadow"
    );
    assert!(
        f.chips.iter().all(|c| turned_at(&out, *c).is_none()),
        "a named chip is drawn mid-hop"
    );
    for socket in f.sockets {
        assert!(tex_at(&out, socket).is_some(), "a socket is hidden mid-hop");
    }
}

#[test]
fn the_chip_alone_shakes_when_refused() {
    let (_d, mut app) = on_shelf(&["Emerald", "Metroid Fusion", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    let before = frame(&app);
    app.apply(Action::GbaDown(Btn::Left));
    let after = frame(&app);

    let (_, still, _) = turned_at(&before, f.chips[0]).unwrap();
    let (_, shaken, _) = turned_at(&after, f.chips[0]).unwrap();
    assert_ne!(still[0], shaken[0], "the chip did not move");
    let rest = |out: &[Draw]| -> Vec<Draw> {
        out.iter()
            .filter(|d| !matches!(d, Draw::Turned { tex, .. } if *tex == f.chips[0]))
            .copied()
            .collect()
    };
    assert_eq!(
        rest(&before),
        rest(&after),
        "something besides the chip moved"
    );
}

#[test]
fn closing_puts_the_cart_back_on_the_shelf() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    let_it_open(&mut app);
    app.apply(Action::GbaDown(Btn::B));

    app.update(0.1);
    let partway = frame(&app);
    let (_, lid, turn) = turned_at(&partway, f.lid).expect("the lid vanished mid-close");
    assert!(
        turn < 0.0 && turn > LID_TURN,
        "the lid is not turning level: {turn}"
    );
    assert!(lid[1] > lid_at(1.0).0.y, "the lid is not coming back down");
    let (_, [bx, by, bw, bh]) = tex_at(&partway, f.board).expect("the board vanished mid-close");
    assert!(
        bw < board_at(1.0).w && bw > board_at(0.0).w,
        "the board is not shrinking: {bw}"
    );
    assert_eq!(
        alpha_of(&partway, f.board),
        Some(1.0),
        "the back half faded mid-close"
    );

    assert_parts_on_board(
        &partway,
        &f,
        Placed {
            x: bx,
            y: by,
            w: bw,
            h: bh,
        },
        "mid-close",
    );

    let_it_close(&mut app);
    let out = frame(&app);

    assert!(
        tex_at(&out, f.board).is_none(),
        "the board outlived the close"
    );
    assert!(
        turned_at(&out, f.lid).is_none(),
        "the lid outlived the close"
    );
    let standing = out
        .iter()
        .filter(|d| {
            matches!(**d, Draw::Rect { w, .. } | Draw::Tex { w, .. }
                if (w - CART_W as f32).abs() < 0.01)
        })
        .count();
    assert_eq!(standing, 1, "the cart did not go back on the shelf");
}

#[test]
fn the_open_waits_on_the_shelf_for_its_faces() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_boot_faces(&mut app);
    let rest = resting_cart(&frame(&app));
    app.apply(Action::GbaDown(Btn::Start));
    app.update(0.3);
    assert_plain_shelf(&frame(&app), &f, "while its faces are built");

    app.set_core_board_faces(f.board, f.lid);
    app.update(0.016);
    let out = frame(&app);
    let shelf = grown(shelf_cart_at(rest, 1.0), TURN_PAD as f32);
    let (_, lid, _) = turned_at(&out, f.lid).expect("no lid once the faces arrived");
    assert!(
        near(lid, [shelf.x, shelf.y, shelf.w, shelf.h]),
        "the open used up the wait: the lid is at {lid:?}"
    );
    assert_eq!(
        carts_standing(&out, f.board),
        0,
        "the row still draws the cart whose lid is coming off"
    );

    let_it_open(&mut app);
    let (rest, _) = lid_at(1.0);
    let rest = grown(rest, TURN_PAD as f32 * rest.w / CART_W as f32);
    let (_, lid, _) = turned_at(&frame(&app), f.lid).expect("no lid once open");
    assert!(
        near(lid, [rest.x, rest.y, rest.w, rest.h]),
        "the lid is not at rest: {lid:?}"
    );
}

#[test]
fn the_open_starts_anyway_when_the_faces_never_come() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaUp(Btn::Right));
    assert_eq!(app.selected_stem(), Some("Zzz"));
    let f = fake_picker_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Left));
    app.apply(Action::GbaUp(Btn::Left));
    assert_eq!(app.selected_stem(), Some("Emerald"));

    app.apply(Action::GbaDown(Btn::Start));
    app.update(1.499);
    assert_plain_shelf(&frame(&app), &f, "a hair under the cap");

    app.update(0.002);
    assert_eq!(
        carts_standing(&frame(&app), f.board),
        0,
        "the open never started once the cap ran out"
    );

    let_it_open(&mut app);
    let out = frame(&app);
    assert!(
        tex_at(&out, f.board).is_none(),
        "the fallback open drew the other cart's board"
    );
    assert!(
        turned_at(&out, f.lid).is_none(),
        "the fallback open drew the other cart's lid"
    );
    for tex in [f.sockets[0], f.sockets[1], f.chips[0], f.chips[1], f.blank] {
        assert!(
            tex_at(&out, tex).is_none() && turned_at(&out, tex).is_none(),
            "the fallback open drew a socket or chip quad"
        );
    }
}

#[test]
fn the_fallback_open_lifts_the_shelfs_own_face_for_the_lid() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let shelf_faces = [TexId::from_raw(950), TexId::from_raw(951)];
    app.set_faces(shelf_faces.to_vec());
    app.apply(Action::GbaDown(Btn::Start));
    app.update(1.501);
    let_it_open(&mut app);

    let (rest, _) = lid_at(1.0);
    let (_, lid, turn) =
        turned_at(&frame(&app), shelf_faces[0]).expect("no lid drawn from the shelf's own face");
    assert!(
        near(lid, [rest.x, rest.y, rest.w, rest.h]),
        "the fallback lid is not at rest: {lid:?}"
    );
    assert_eq!(turn, LID_TURN, "the fallback lid is not turned");
}

#[test]
fn the_real_faces_replace_the_fallback_once_they_arrive() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let shelf_faces = [TexId::from_raw(950), TexId::from_raw(951)];
    app.set_faces(shelf_faces.to_vec());
    app.apply(Action::GbaDown(Btn::Start));
    app.update(1.501);
    let_it_open(&mut app);
    assert!(
        turned_at(&frame(&app), shelf_faces[0]).is_some(),
        "the fallback lid should be up before the real faces arrive"
    );

    let f = fake_picker_faces(&mut app);
    app.update(0.016);
    let out = frame(&app);
    assert!(tex_at(&out, f.board).is_some(), "the board never arrived");
    assert!(
        turned_at(&out, f.lid).is_some(),
        "the picker's own lid never arrived"
    );
}

#[test]
fn arrows_pressed_while_the_cart_waits_move_nothing() {
    let (_d, mut app) = on_shelf(&["Emerald", "Zzz"]);
    let f = fake_boot_faces(&mut app);
    app.apply(Action::GbaDown(Btn::Start));
    app.apply(Action::GbaDown(Btn::Right));
    app.set_core_board_faces(f.board, f.lid);
    app.update(0.016);
    assert_eq!(
        app.core_picker(),
        Some(Core::Mgba),
        "a hop ran while the cart waited"
    );

    let_it_open(&mut app);
    let out = frame(&app);
    assert!(
        turned_at(&out, f.chips[0]).is_some(),
        "the chip is not seated in mGBA once the cart is open"
    );
    assert!(
        turned_at(&out, f.chips[1]).is_none(),
        "the chip opened in gpSP"
    );
}

fn session_playing(root: &Path) -> Session {
    common::clocked(root);
    let mut s = Session::boot(root.to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    s
}

const GB_WINDOW: [f32; 4] = [40.0 / 240.0, 8.0 / 160.0, 160.0 / 240.0, 144.0 / 160.0];

fn pad(s: &Session) -> u16 {
    s.emu().expect("no core in the slot").input().0
}

#[test]
fn l_and_r_change_the_mode_on_a_game_boy_cart_and_not_on_a_gba_one() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    let mut s = session_playing(d.path());
    assert_eq!(
        s.app().source_rect(),
        WHOLE_TEXTURE,
        "it did not open actual size"
    );

    s.feed([RawEvent::Down(Btn::L1)], 100);
    assert_eq!(
        s.app().source_rect(),
        GB_WINDOW,
        "L did not stretch the picture"
    );
    assert_eq!(pad(&s) & ButtonMask::L, 0, "L reached the game as well");
    s.feed([RawEvent::Up(Btn::L1)], 120);

    s.feed([RawEvent::Down(Btn::R1)], 200);
    assert_eq!(
        s.app().source_rect(),
        WHOLE_TEXTURE,
        "R did not give it back"
    );
    assert_eq!(pad(&s) & ButtonMask::R, 0, "R reached the game as well");
    s.feed([RawEvent::Up(Btn::R1)], 220);

    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut s = session_playing(d.path());
    s.feed([RawEvent::Down(Btn::L1)], 100);
    assert_ne!(pad(&s) & ButtonMask::L, 0, "the GBA lost its own L");
    s.feed([RawEvent::Down(Btn::R1)], 120);
    assert_ne!(pad(&s) & ButtonMask::R, 0, "the GBA lost its own R");
    assert_eq!(
        s.app().source_rect(),
        WHOLE_TEXTURE,
        "a GBA picture moved when its shoulders were pressed"
    );
}

#[test]
fn a_shoulder_held_across_the_insert_does_not_stick_on_the_pad() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::L1)], 16);
    s.feed([RawEvent::Down(Btn::A)], 32);
    s.feed([RawEvent::Up(Btn::A)], 48);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    s.feed([RawEvent::Up(Btn::L1)], 1000);
    assert_eq!(
        pad(&s) & ButtonMask::L,
        0,
        "the pad is still holding L after the finger came off it"
    );
}

#[test]
fn a_shoulder_still_held_when_the_cart_seats_never_reaches_a_game_boy_core() {
    for (label, root, want_held) in [
        (
            "Game Boy",
            common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
            false,
        ),
        (
            "Game Boy Advance",
            common::tmp_root_with_carts(&["Emerald", "Zzz"]),
            true,
        ),
    ] {
        common::clocked(root.path());
        let mut s = Session::boot(root.path().to_path_buf());
        s.feed([RawEvent::Down(Btn::L1)], 16);
        s.feed([RawEvent::Down(Btn::A)], 32);
        s.feed([RawEvent::Up(Btn::A)], 48);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut now = 48;
        while !matches!(s.app().phase(), Phase::Playing { .. }) {
            assert!(Instant::now() < deadline, "{label}: the cart never seated");
            now += 16;
            s.feed([], now);
            s.update(1.0 / 60.0);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            pad(&s) & ButtonMask::L != 0,
            want_held,
            "{label}: the core is holding L {} the cart seated under a thumb that never moved",
            if want_held {
                "nowhere after"
            } else {
                "ever since"
            }
        );
    }
}

#[test]
fn the_picture_mode_is_remembered_per_cart() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    let mut s = session_playing(d.path());
    s.feed([RawEvent::Down(Btn::L1)], 100);
    s.feed([RawEvent::Up(Btn::L1)], 120);
    assert_eq!(video_mode_for(d.path(), "Tetris"), VideoMode::Stretch);
    assert_eq!(
        video_mode_for(d.path(), "Zzz"),
        VideoMode::Actual,
        "the press was recorded against another cart as well"
    );
    drop(s);

    let mut s = session_playing(d.path());
    assert_eq!(
        s.app().source_rect(),
        GB_WINDOW,
        "the cart did not come back stretched"
    );
    s.feed([RawEvent::Down(Btn::R1)], 100);
    assert_eq!(video_mode_for(d.path(), "Tetris"), VideoMode::Actual);
    assert_eq!(s.app().source_rect(), WHOLE_TEXTURE);
}

#[test]
fn a_cart_with_no_line_reads_as_actual_size() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    assert!(
        !d.path().join(VIDEO_MODE_FILE).exists(),
        "the fixture already wrote the file"
    );
    assert_eq!(video_mode_for(d.path(), "Tetris"), VideoMode::Actual);

    let s = session_playing(d.path());
    assert_eq!(
        s.app().source_rect(),
        WHOLE_TEXTURE,
        "a cart with no line did not open at actual size"
    );
}

/// Once a hold starts shutdown, further buttons cannot reach the paused game.
#[test]
fn buttons_during_shutdown_never_reach_the_game() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut s = session_playing(d.path());
    s.feed([RawEvent::Down(Btn::Power)], 1000);
    s.feed([], 2100);
    assert!(s.app().powering_off(), "the hold never started shutdown");
    s.feed([RawEvent::Up(Btn::Power)], 2110);
    s.feed([RawEvent::Down(Btn::Down)], 2200);
    assert_eq!(pad(&s) & ButtonMask::DOWN, 0);
    s.feed([RawEvent::Down(Btn::B)], 2300);
    assert!(s.app().powering_off(), "B cancelled shutdown");
    assert_eq!(pad(&s) & ButtonMask::B, 0);
}

#[test]
fn a_chorded_select_never_reaches_the_pad() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut s = session_playing(d.path());
    let (power, backlight) = common::panel(d.path(), Duration::from_secs(180));
    s.app_mut().set_power(power);
    let lit = backlight.load(Ordering::Relaxed);
    assert!(lit > 0 && lit < 9, "the level has to have room to move");

    s.feed([RawEvent::Down(Btn::Select)], 1000);
    assert_eq!(
        pad(&s) & ButtonMask::SELECT,
        0,
        "the game got a SELECT that may yet be a chord"
    );

    s.feed([RawEvent::Down(Btn::Up)], 1120);
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        lit + 1,
        "SELECT+Up stopped being the brightness chord"
    );
    assert_eq!(
        pad(&s) & ButtonMask::UP,
        0,
        "the chord's second key reached the game as well as firing the chord"
    );
    s.feed([RawEvent::Up(Btn::Up)], 1200);
    assert_eq!(
        pad(&s) & ButtonMask::UP,
        0,
        "the release of a swallowed press reached the game on its own"
    );

    s.feed([RawEvent::Up(Btn::Select)], 1400);
    assert_eq!(
        pad(&s) & ButtonMask::SELECT,
        0,
        "the chord's SELECT reached the game on its release"
    );
}

#[test]
fn a_latched_fast_forward_does_not_outlive_the_cart() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut s = session_playing(d.path());
    s.feed([RawEvent::Down(Btn::R2)], 1000);
    s.feed([RawEvent::Up(Btn::R2)], 1050);
    s.feed([RawEvent::Down(Btn::R2)], 1150);
    s.feed([RawEvent::Up(Btn::R2)], 1200);
    let mut now = run(&mut s, 1200, 2);
    assert_eq!(
        s.app().ff_badge(),
        Some(Icon::FastForwardLatched),
        "the latch never took"
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while s.emu().map(EmuHandle::observed_speed) != Some(Speed::Fast) {
        assert!(Instant::now() < deadline, "the worker never ran fast");
        now = run(&mut s, now, 1);
    }
    s.feed([RawEvent::Down(Btn::Menu)], now + 16);
    now = run(&mut s, now + 1100, 250);
    s.feed([RawEvent::Up(Btn::Menu)], now + 16);
    now += 32;
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "the cart never came out"
    );
    s.feed([RawEvent::Down(Btn::A)], now);
    s.feed([RawEvent::Up(Btn::A)], now + 16);
    now += 32;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    run(&mut s, now, 60);
    assert_eq!(
        s.app().ff_badge(),
        None,
        "the next cart came up with the last cart's latch still showing"
    );
    assert_ne!(
        s.emu().map(EmuHandle::observed_speed),
        Some(Speed::Fast),
        "the next cart came up fast forwarding off a latch the last one was left holding"
    );
}

fn run(s: &mut Session, from: u64, frames: u64) -> u64 {
    let mut now = from;
    for _ in 0..frames {
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    now
}

#[test]
fn the_colour_shortcut_reaches_the_core_already_running() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut s, _motor) = common::session_with_platform(d.path());
    s.app_mut().apply(slot_input::Action::Insert);

    let before = s.app().colour_correction();
    s.app_mut()
        .apply(slot_input::Action::ColourCorrectionToggle);

    assert_eq!(
        s.app().colour_correction(),
        !before,
        "the shortcut did not flip the setting"
    );
    assert_eq!(
        s.app_mut().take_colour_correction(),
        Some(!before),
        "the change was written down but never queued for the core already running, which is \
         the whole of what this shortcut is for"
    );
}

#[test]
fn the_colour_shortcut_names_the_state_it_arrived_at() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut s, _motor) = common::session_with_platform(d.path());
    s.app_mut().apply(slot_input::Action::Insert);

    for _ in 0..2 {
        let want = match s.app().colour_correction() {
            true => slot_ui::Toast::ColourOff,
            false => slot_ui::Toast::ColourOn,
        };
        s.app_mut()
            .apply(slot_input::Action::ColourCorrectionToggle);
        assert_eq!(
            s.app().toast(),
            Some(want),
            "the banner named the state the toggle left, not the one it reached"
        );
    }
}

fn slot_text_alpha(a: &App, tex: slot_ui::TexId) -> f32 {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter()
        .find_map(|d| match *d {
            Draw::Tex { tex: t, alpha, .. } if t == tex => Some(alpha),
            _ => None,
        })
        .unwrap_or(0.0)
}

#[test]
fn a_letter_jump_shows_the_letter_in_the_slot_and_fades_like_the_shelf_name() {
    let mut a = app_with_carts(&["Alpha", "Bravo"]);
    let face = slot_ui::TexId::from_raw(77);
    assert_eq!(
        a.slot_text(),
        None,
        "a one shelf card printed something in the slot"
    );

    a.apply(Action::GbaDown(Btn::Down));
    assert_eq!(a.slot_text().as_deref(), Some("B"));
    a.set_shelf_platform_face(face, 20);
    a.update(1.0 / 60.0);
    a.update(0.5);
    assert!(slot_text_alpha(&a, face) > 0.0, "the letter never showed");
    a.update(5.0);
    assert_eq!(slot_text_alpha(&a, face), 0.0, "the letter never faded");

    a.apply(Action::GbaDown(Btn::Left));
    a.apply(Action::GbaDown(Btn::Down));
    assert_eq!(a.slot_text().as_deref(), Some("B"));
    a.update(1.0 / 60.0);
    a.update(0.5);
    assert!(
        slot_text_alpha(&a, face) > 0.0,
        "jumping to the letter already printed did not show it again"
    );
}
