mod common;

use slot::app::{App, Phase};
use slot_gfx::{Canvas, Draw};
use slot_input::Action;
use slot_store::{Cart, Platform};
use slot_ui::MOUTH_H;

const PANEL_4_3: (u32, u32) = (640, 480);
const NATIVE: Canvas = Canvas { w: 640.0, h: 480.0 };

fn app_with_carts(stems: &[&str], panel: (u32, u32)) -> App {
    let mut a = App::new(
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
    );
    a.set_panel(panel);
    a
}

fn tick(a: &mut App) {
    a.update(1.0 / 60.0);
}

fn playing(panel: (u32, u32)) -> App {
    let mut a = app_with_carts(&["Emerald", "Zzz"], panel);
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        tick(&mut a);
    }
    a
}

#[test]
fn the_shelf_and_a_game_share_the_panel_canvas_on_a_4_by_3_panel() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    assert!(!a.shows_game());
    assert_eq!(a.canvas(), NATIVE);
    a.apply(Action::Insert);
    assert!(!a.shows_game(), "the cart going in left the shelf");
    let a = playing(PANEL_4_3);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert!(a.shows_game());
    assert_eq!(a.canvas(), NATIVE);
}

#[test]
fn the_in_game_menu_hides_the_game_once_it_shows() {
    let mut a = playing(PANEL_4_3);
    a.apply(Action::QuickMenu);
    assert!(a.shows_game(), "the overlay left before the menu showed");
    for _ in 0..30 {
        tick(&mut a);
    }
    assert!(!a.shows_game());
    assert_eq!(a.canvas(), NATIVE);
}

#[test]
fn the_photos_over_a_game_still_show_it() {
    let mut a = playing(PANEL_4_3);
    a.apply(Action::Polaroids);
    if matches!(a.phase(), Phase::Polaroids { .. }) {
        assert!(a.shows_game(), "the save state photos");
    }
}

#[test]
fn the_shelf_menu_does_not_show_a_game() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    a.apply(Action::QuickMenu);
    assert!(matches!(a.phase(), Phase::QuickMenu { .. }));
    assert!(!a.shows_game());
}

#[test]
fn every_screen_is_the_framed_canvas_on_a_720_panel() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], (720, 480));
    assert_eq!(a.canvas(), Canvas::FRAMED);
    a.apply(Action::QuickMenu);
    assert_eq!(a.canvas(), Canvas::FRAMED);
    let mut a = playing((720, 480));
    assert_eq!(a.canvas(), Canvas::FRAMED);
    a.apply(Action::QuickMenu);
    for _ in 0..30 {
        tick(&mut a);
    }
    assert_eq!(
        a.canvas(),
        Canvas::FRAMED,
        "the in-game menu on a 720 panel"
    );
}

#[test]
fn the_overlay_rises_while_the_screen_powers_on() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    a.apply(Action::Insert);
    a.on_core_ready();
    while !matches!(a.phase(), Phase::Playing { .. }) {
        assert_eq!(a.overlay_slide(), 0.0, "the overlay rose before the game");
        tick(&mut a);
    }
    let mut frames = 0;
    while a.overlay_slide() < 1.0 {
        tick(&mut a);
        frames += 1;
        assert!(
            frames <= 16,
            "the overlay took more than the power-on to rise"
        );
    }
    assert!(
        frames >= 12,
        "the overlay rose in {frames} frames, faster than the screen"
    );
}

#[test]
fn the_exit_powers_off_then_drops_the_overlay_then_leaves_the_game() {
    let mut a = playing(PANEL_4_3);
    a.apply(Action::Eject);
    while a.screen_power() > 0.0 {
        assert_eq!(
            a.overlay_slide(),
            1.0,
            "the overlay moved before the screen was off"
        );
        assert!(a.shows_game());
        assert_eq!(
            a.screen_footer(),
            0.0,
            "the framed close scaled around a footer that is not on this canvas"
        );
        tick(&mut a);
    }
    let mut frames = 0;
    while a.overlay_slide() > 0.0 {
        assert!(a.shows_game(), "the shelf showed under a sliding overlay");
        tick(&mut a);
        frames += 1;
        assert!(frames <= 16, "the overlay took longer than it took to rise");
    }
    assert!(frames >= 12, "the overlay dropped in {frames} frames");
    assert!(!a.shows_game());
    assert!(
        matches!(a.phase(), Phase::Ejecting { .. }),
        "the cart left before the shelf showed"
    );
}

#[test]
fn the_panel_is_black_behind_the_game_as_it_powers_on_a_4_by_3_panel() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    a.apply(Action::Insert);
    a.on_core_ready();
    while !matches!(a.phase(), Phase::Playing { .. }) {
        tick(&mut a);
    }
    tick(&mut a);
    assert!(a.screen_power() < 1.0);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        matches!(out.first(),
            Some(Draw::Rect { x, y, w, h, colour: [0.0, 0.0, 0.0, 1.0] })
                if *x == 0.0 && *y == 0.0 && *w == NATIVE.w && *h == NATIVE.h),
        "{:?}",
        out.first()
    );
    assert!(
        !out.iter()
            .any(|d| matches!(d, Draw::Tex { .. } | Draw::Part { .. })),
        "the shelf shows behind the game"
    );
}

#[test]
fn the_shelf_still_shows_behind_the_power_on_on_a_720_panel() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], (720, 480));
    a.set_ground(slot_gfx::TexId::from_raw(900));
    a.apply(Action::Insert);
    a.on_core_ready();
    while !matches!(a.phase(), Phase::Playing { .. }) {
        tick(&mut a);
    }
    tick(&mut a);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        out.iter()
            .any(|d| matches!(d, Draw::Part { tex, .. } if *tex == slot_gfx::TexId::from_raw(900))),
        "the 720 power-on lost the wallpaper behind it"
    );
}

fn eject_until_shelf(a: &mut App, mut each: impl FnMut(&App)) {
    a.apply(Action::Eject);
    for _ in 0..240 {
        each(a);
        if matches!(a.phase(), Phase::Shelf) {
            return;
        }
        tick(a);
    }
    panic!("the cart never came out");
}

#[test]
fn ejecting_a_cart_that_is_still_going_in_keeps_the_shelf() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    a.apply(Action::Insert);
    for _ in 0..10 {
        tick(&mut a);
    }
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    eject_until_shelf(&mut a, |a| {
        assert_eq!(
            a.overlay_slide(),
            0.0,
            "the overlay rose over a cart going in"
        );
        assert!(!a.shows_game(), "the shelf showed the game");
    });
}

#[test]
fn exit_from_the_in_game_menu_never_brings_the_overlay_back() {
    let mut a = playing(PANEL_4_3);
    a.apply(Action::QuickMenu);
    for _ in 0..30 {
        tick(&mut a);
    }
    assert!(!a.shows_game());
    let mut powering = 0;
    eject_until_shelf(&mut a, |a| {
        assert_eq!(
            a.overlay_slide(),
            0.0,
            "the overlay popped in on the way out"
        );
        assert!(!a.shows_game());
        if matches!(a.phase(), Phase::Ejecting { .. }) && a.screen_power() > 0.0 {
            powering += 1;
            assert_eq!(
                a.screen_footer(),
                MOUTH_H,
                "the picture scaled through the carousel footer"
            );
            let mut out = Vec::new();
            a.draw(&mut out);
            assert!(
                matches!(
                    out.first(),
                    Some(Draw::Rect {
                        colour: [0.0, 0.0, 0.0, 1.0],
                        ..
                    })
                ),
                "the game powered off over something other than black"
            );
            assert!(
                !out.iter().any(|d| matches!(d, Draw::Part { .. })),
                "the shelf showed behind the game powering off"
            );
        }
    });
    assert!(powering > 0, "the screen never powered off");
}

#[test]
fn ejecting_while_the_overlay_rises_never_raises_it_further() {
    let mut a = app_with_carts(&["Emerald", "Zzz"], PANEL_4_3);
    a.apply(Action::Insert);
    a.on_core_ready();
    while !matches!(a.phase(), Phase::Playing { .. }) {
        tick(&mut a);
    }
    for _ in 0..5 {
        tick(&mut a);
    }
    let mut high = a.overlay_slide();
    assert!(high > 0.0 && high < 1.0, "not mid-rise: {high}");
    eject_until_shelf(&mut a, |a| {
        let now = a.overlay_slide();
        assert!(
            now <= high + 1e-6,
            "the overlay jumped from {high} to {now}"
        );
        high = now;
    });
}

fn plates(out: &[Draw]) -> Vec<(f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, h, .. } if x == 0.0 && h == slot_ui::PLATE_H => Some((y, w)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_hud_over_a_game_spans_the_top_of_a_4_by_3_panel() {
    let mut a = playing(PANEL_4_3);
    a.apply(Action::BrightnessUp);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert_eq!(plates(&out), vec![(0.0, NATIVE.w)]);
}

#[test]
fn the_photo_bars_over_a_game_span_a_4_by_3_panel() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let mut a = common::app_playing_in(d.path(), "Emerald");
    a.set_panel(PANEL_4_3);
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::Polaroids, 1_100);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    let mut out = Vec::new();
    a.draw(&mut out);
    assert_eq!(
        plates(&out)[..2],
        [(0.0, NATIVE.w), (NATIVE.h - slot_ui::PLATE_H, NATIVE.w)]
    );
}

#[test]
fn the_hud_spans_a_720_panel() {
    let mut a = playing((720, 480));
    a.apply(Action::BrightnessUp);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert_eq!(plates(&out), vec![(0.0, Canvas::FRAMED.w)]);
}

fn strip_at(a: &App, strip: slot_gfx::TexId) -> Option<(f32, f32)> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter().find_map(|d| match *d {
        Draw::Tex { y, w, tex, .. } if tex == strip => Some((y, w)),
        _ => None,
    })
}

#[test]
fn the_sp_strip_covers_a_4_by_3_panel_over_a_game_and_drops_on_the_way_out() {
    let strip = slot_gfx::TexId::from_raw(9_999);
    let mut a = playing(PANEL_4_3);
    a.set_sp_strip(Some(strip));
    assert_eq!(strip_at(&a, strip), Some((0.0, NATIVE.w)));
    a.apply(Action::Eject);
    let mut last = 0.0;
    for _ in 0..200 {
        tick(&mut a);
        if let Some((y, _)) = strip_at(&a, strip) {
            assert!(y >= last, "the strip rose on the way out");
            last = y;
        }
    }
    assert!(last > 0.0, "the strip never dropped");
    assert_eq!(strip_at(&a, strip), None);
}

#[test]
fn the_in_game_menu_hides_the_sp_strip() {
    let strip = slot_gfx::TexId::from_raw(9_999);
    let mut a = playing(PANEL_4_3);
    a.set_sp_strip(Some(strip));
    a.apply(Action::QuickMenu);
    for _ in 0..30 {
        tick(&mut a);
    }
    assert_eq!(strip_at(&a, strip), None);
}

#[test]
fn a_2x_host_builds_bezels_at_the_canvas_size() {
    let a = app_with_carts(&["Emerald", "Zzz"], (1280, 960));
    assert_eq!(a.game_bezel_size(), (640, 480));
}

#[test]
fn an_unlisted_canvas_gets_the_placeholder_not_a_stretched_bezel() {
    let a = playing((1000, 700));
    assert!(a.placeholder_bezel().is_some());
    assert_eq!(a.bezel_art(), None);
    assert!(playing((720, 480)).placeholder_bezel().is_none());
    assert!(playing((640, 480)).placeholder_bezel().is_none());
}
