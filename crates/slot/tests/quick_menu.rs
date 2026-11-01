mod common;

use common::{
    app_booting_at, app_booting_with_clock, app_playing_in, tmp_root_with_carts, Clock,
    CLOCK_IS_SET,
};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{
    read_slot_state, write_slot_state, Appearance, BezelPick, Blending, GbColors, GbPalette,
    Platform, Scale, Shader, SlotState, Stretch, WallpaperPick, Zone,
};
use slot_ui::{
    Draw, Icon, QuickMenuFaces, QuickRow, QuickValue, TexId, HINT_EDGE, MENU_PAD, MOUTH_H, OUT_H,
    OUT_W, QUICK_EDGE, QUICK_LABEL_X,
};
use tempfile::TempDir;

fn on_carousel_with(state: SlotState) -> (TempDir, App, Clock) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..state
        },
    )
    .expect("write slot.state");
    let (a, clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "not on the carousel: {:?}",
        a.phase()
    );
    (d, a, clock)
}

fn on_carousel() -> (TempDir, App, Clock) {
    on_carousel_with(SlotState::default())
}

fn new_york() -> Zone {
    Zone::parse("America/New_York").expect("New York is on the list")
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open_at(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    let mut path = vec![row];
    while let Some(parent) = path.last().and_then(|r| r.parent()) {
        path.push(parent);
    }
    for (depth, step) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(a, Btn::A);
        }
        for _ in 0..step.position(&a.hidden_rows()) {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.quick_menu(), Some(row), "the bar never reached {row:?}");
}

#[test]
fn menu_opens_the_quick_menu_with_its_top_row_selected_every_time() {
    let (_d, mut a, _) = on_carousel();
    a.apply(Action::QuickMenu);
    assert_eq!(a.quick_menu(), Some(QuickRow::MAIN[0]));
    press(&mut a, Btn::Down);
    a.apply(Action::QuickMenu);
    a.apply(Action::QuickMenu);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::MAIN[0]),
        "the menu opened where it was last left"
    );
}

#[test]
fn menu_or_b_closes_it_back_onto_the_carousel_where_you_were() {
    for close in [Action::QuickMenu, Action::GbaDown(Btn::B)] {
        let (_d, mut a, _) = on_carousel();
        press(&mut a, Btn::Right);
        assert_eq!(
            a.selected_stem(),
            Some("Fusion"),
            "the carousel never moved"
        );
        a.apply(Action::QuickMenu);
        a.apply(close);
        assert!(
            matches!(a.phase(), Phase::Shelf),
            "{close:?} left {:?}",
            a.phase()
        );
        assert_eq!(a.selected_stem(), Some("Fusion"), "{close:?} lost the cart");
    }
}

#[test]
fn the_quick_menu_is_only_on_the_carousel() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::QuickMenu);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "MENU raised the quick menu over a game: {:?}",
        a.phase()
    );
    assert_eq!(a.quick_menu(), None);
}

#[test]
fn wallpaper_cycles_none_random_shelf_default_blur_then_the_folder_and_saves() {
    let (d, mut a, _) = on_carousel();
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("b.png"), b"png").unwrap();
    std::fs::write(dir.join("Crt.png"), b"png").unwrap();
    std::fs::write(dir.join("a.png"), b"png").unwrap();
    open_at(&mut a, QuickRow::Wallpaper);
    assert_eq!(a.quick_menu(), Some(QuickRow::Wallpaper));
    assert_eq!(
        a.resolved_wallpaper(),
        WallpaperPick::Default,
        "a new card did not start on Default"
    );
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Blur);
    assert_eq!(a.resolved_wallpaper().label(), "DEFAULT BLUR");
    for name in ["a.png", "b.png", "Crt.png"] {
        press(&mut a, Btn::Right);
        assert_eq!(a.resolved_wallpaper(), WallpaperPick::File(name.into()));
    }
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::None);
    assert_eq!(read_slot_state(d.path()).wallpaper, WallpaperPick::None);
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Random);
    assert_eq!(a.resolved_wallpaper().label(), "Random");
    assert_eq!(read_slot_state(d.path()).wallpaper, WallpaperPick::Random);
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Shelf);
    assert_eq!(a.resolved_wallpaper().label(), "Shelf");
    assert_eq!(read_slot_state(d.path()).wallpaper, WallpaperPick::Shelf);
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Default);
    press(&mut a, Btn::Left);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Shelf);
    press(&mut a, Btn::Left);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Random);
    press(&mut a, Btn::Left);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::None);
    press(&mut a, Btn::A);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::Wallpaper),
        "wallpaper opened a page"
    );
}

fn random_card(names: &[&str]) -> TempDir {
    let (d, _, _) = on_carousel_with(SlotState {
        wallpaper: WallpaperPick::Random,
        ..SlotState::default()
    });
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), b"png").unwrap();
    }
    d
}

#[test]
fn random_shows_one_of_the_folder_and_keeps_it_for_the_boot() {
    let d = random_card(&["a.png", "b.png", "c.png"]);
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    let first = a.shown_wallpaper().expect("random showed nothing");
    assert!(
        ["a.png", "b.png", "c.png"].contains(&first.as_str()),
        "{first}"
    );
    for _ in 0..5 {
        assert_eq!(a.shown_wallpaper().as_deref(), Some(first.as_str()));
    }
}

#[test]
fn random_deals_every_wallpaper_once_before_a_repeat_across_boots() {
    let names = ["a.png", "b.png", "c.png"];
    let d = random_card(&names);
    let mut seen: Vec<String> = (0..names.len())
        .map(|_| {
            let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
            a.shown_wallpaper().expect("random showed nothing")
        })
        .collect();
    seen.sort();
    assert_eq!(seen, names);
}

#[test]
fn random_with_an_empty_folder_shows_the_default() {
    let d = random_card(&[]);
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert_eq!(a.resolved_wallpaper(), WallpaperPick::Random);
    assert_eq!(a.shown_wallpaper(), None);
}

#[test]
fn theme_switches_between_dark_and_light_and_saves() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Theme);
    assert_eq!(a.quick_menu(), Some(QuickRow::Theme));
    assert_eq!(a.quick_value(QuickRow::Theme), Some(QuickValue::Dark));
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::Theme), Some(QuickValue::Light));
    assert_eq!(read_slot_state(d.path()).appearance, Appearance::Light);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::Theme), "theme opened a page");
    press(&mut a, Btn::Left);
    assert_eq!(a.quick_value(QuickRow::Theme), Some(QuickValue::Dark));
    assert_eq!(read_slot_state(d.path()).appearance, Appearance::Dark);
}

#[test]
fn up_and_down_move_the_bar_and_wrap_at_the_ends() {
    let (_d, mut a, _) = on_carousel();
    a.apply(Action::QuickMenu);
    press(&mut a, Btn::Up);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::About),
        "up from the top did not wrap to the bottom"
    );
    for want in [
        QuickRow::General,
        QuickRow::Screen,
        QuickRow::Game,
        QuickRow::Controls,
        QuickRow::Network,
        QuickRow::About,
        QuickRow::General,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(a.quick_menu(), Some(want));
    }
}

#[test]
fn fast_forward_steps_through_its_speeds_wraps_at_both_ends_and_saves_each_one() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::FastForward);
    for (btn, want) in [
        (Btn::Left, 16),
        (Btn::Left, 12),
        (Btn::Left, 8),
        (Btn::Left, 6),
        (Btn::Left, 0),
        (Btn::Left, 24),
        (Btn::Right, 0),
        (Btn::Right, 6),
        (Btn::Right, 8),
        (Btn::Right, 12),
        (Btn::Right, 16),
        (Btn::Right, 24),
        (Btn::Right, 0),
    ] {
        press(&mut a, btn);
        assert_eq!(a.ff_speed(), want, "{btn:?}");
        assert_eq!(read_slot_state(d.path()).ff_speed, want, "not on the card");
        assert_eq!(
            a.quick_value(QuickRow::FastForward),
            QuickValue::speed(want)
        );
    }
}

#[test]
fn rumble_and_fast_forward_sound_flip_on_either_arrow_and_save() {
    let (d, mut a, _) = on_carousel();
    let card = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (s.ff_sound, s.rumble)
    };
    open_at(&mut a, QuickRow::FastForwardSound);
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (true, true));
    assert_eq!(
        a.quick_value(QuickRow::FastForwardSound),
        Some(QuickValue::On)
    );
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, true));
    for _ in QuickRow::FastForwardSound.position(&[])..QuickRow::Rumble.position(&[]) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, false));
    assert!(!a.rumble_enabled());
    assert_eq!(a.quick_value(QuickRow::Rumble), Some(QuickValue::Off));
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (false, true));
}

#[test]
fn each_systems_colour_correction_flips_on_either_arrow_and_saves_alone() {
    let saved = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (
            s.colour_correction_gba,
            s.colour_correction_gb,
            s.colour_correction_gbc,
        )
    };
    for row in [
        QuickRow::GbaColourCorrection,
        QuickRow::GbColourCorrection,
        QuickRow::GbcColourCorrection,
    ] {
        let (d, mut a, _) = on_carousel();
        open_at(&mut a, row);
        assert_eq!(
            a.quick_value(row),
            Some(QuickValue::Off),
            "{row:?} did not open on the default, which is off"
        );
        for (btn, want) in [
            (Btn::Right, true),
            (Btn::Left, false),
            (Btn::Left, true),
            (Btn::Right, false),
        ] {
            press(&mut a, btn);
            let both = match row {
                QuickRow::GbaColourCorrection => (want, false, false),
                QuickRow::GbColourCorrection => (false, want, false),
                _ => (false, false, want),
            };
            assert_eq!(saved(&d), both, "{row:?} {btn:?}");
            assert_eq!(a.quick_value(row), Some(QuickValue::flag(want)));
        }
    }
}

#[test]
fn the_game_page_flips_auto_load_eject_save_turbo_and_rewind_and_saves_each() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Rewind);
    let card = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (s.auto_load, s.eject_save, s.turbo, s.rewind)
    };
    assert_eq!(card(&d), (true, true, true, true));
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (true, true, true, false));
    assert!(!a.may_rewind(), "rewind still offered with it off");
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Left);
    assert_eq!(card(&d), (true, true, false, false));
    assert!(!a.turbo());
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::AutoLoad));
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, true, false, false));
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    assert_eq!(card(&d), (false, false, false, false));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Game));
}

#[test]
fn screen_opens_a_page_of_its_own_and_b_comes_back_to_it() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Screen);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::ScreenGeneral));
    for want in [
        QuickRow::ScreenGba,
        QuickRow::ScreenGbc,
        QuickRow::ScreenGb,
        QuickRow::ScreenGeneral,
        QuickRow::ScreenGba,
        QuickRow::ScreenGbc,
        QuickRow::ScreenGb,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(
            a.quick_menu(),
            Some(want),
            "the page did not wrap on itself"
        );
    }
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::GbShader));
    for want in [
        QuickRow::GbColourCorrection,
        QuickRow::GbBlending,
        QuickRow::GbPalette,
        QuickRow::GbBezel,
        QuickRow::GbScale,
        QuickRow::GbStretch,
        QuickRow::GbShader,
    ] {
        press(&mut a, Btn::Down);
        assert_eq!(
            a.quick_menu(),
            Some(want),
            "the page did not wrap on itself"
        );
    }
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::ScreenGb));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Screen));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), None);
}

#[test]
fn each_systems_frame_blending_steps_off_mix_lcd_ghosting_wraps_and_saves_alone() {
    let saved = |d: &TempDir| {
        let s = read_slot_state(d.path());
        (s.blending_gba, s.blending_gb, s.blending_gbc)
    };
    for row in [
        QuickRow::GbaBlending,
        QuickRow::GbBlending,
        QuickRow::GbcBlending,
    ] {
        let (d, mut a, _) = on_carousel();
        open_at(&mut a, row);
        assert_eq!(
            a.quick_value(row),
            Some(QuickValue::Off),
            "{row:?} did not open on the default, which is off"
        );
        for (btn, want, shown) in [
            (Btn::Right, Blending::Mix, QuickValue::Mix),
            (Btn::Right, Blending::Ghosting, QuickValue::Ghosting),
            (Btn::Right, Blending::Off, QuickValue::Off),
            (Btn::Left, Blending::Ghosting, QuickValue::Ghosting),
            (Btn::Left, Blending::Mix, QuickValue::Mix),
        ] {
            press(&mut a, btn);
            let off = Blending::Off;
            let alone = match row {
                QuickRow::GbaBlending => (want, off, off),
                QuickRow::GbBlending => (off, want, off),
                _ => (off, off, want),
            };
            assert_eq!(saved(&d), alone, "{row:?} {btn:?}");
            assert_eq!(a.quick_value(row), Some(shown), "{row:?} {btn:?}");
        }
    }
}

#[test]
fn each_platform_keeps_its_own_shader_wraps_and_starts_on_grid_and_simpletex() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::GbaShader);
    assert_eq!(a.quick_value(QuickRow::GbaShader), Some(QuickValue::Grid));
    assert_eq!(
        a.quick_value(QuickRow::GbShader),
        Some(QuickValue::Simpletex)
    );
    for (btn, want, shown) in [
        (Btn::Right, Shader::Dot, QuickValue::Dot),
        (Btn::Right, Shader::Shimmerless, QuickValue::Shimmerless),
        (Btn::Right, Shader::Off, QuickValue::Off),
        (Btn::Left, Shader::Shimmerless, QuickValue::Shimmerless),
        (Btn::Left, Shader::Dot, QuickValue::Dot),
        (Btn::Left, Shader::Grid, QuickValue::Grid),
        (Btn::Left, Shader::Lcd3x, QuickValue::Lcd3x),
        (Btn::Left, Shader::Off, QuickValue::Off),
    ] {
        press(&mut a, btn);
        assert_eq!(a.quick_value(QuickRow::GbaShader), Some(shown), "{btn:?}");
        assert_eq!(
            read_slot_state(d.path()).shader_gba,
            want,
            "{btn:?} never reached the card"
        );
    }
    assert_eq!(read_slot_state(d.path()).shader_gb, Shader::Simpletex);
    while a.quick_menu().is_some() {
        press(&mut a, Btn::B);
    }
    open_at(&mut a, QuickRow::GbShader);
    press(&mut a, Btn::Left);
    assert_eq!(read_slot_state(d.path()).shader_gb, Shader::Grid);
    assert_eq!(read_slot_state(d.path()).shader_gba, Shader::Off);
    assert_eq!(
        a.quick_value(QuickRow::GbcShader),
        Some(QuickValue::Simpletex)
    );
    while a.quick_menu().is_some() {
        press(&mut a, Btn::B);
    }
    open_at(&mut a, QuickRow::GbcShader);
    press(&mut a, Btn::Right);
    let s = read_slot_state(d.path());
    assert_eq!(
        (s.shader_gba, s.shader_gb, s.shader_gbc),
        (Shader::Off, Shader::Grid, Shader::Shimmerless)
    );
}

#[test]
fn colour_correction_leaves_the_settings_around_it_alone() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::GbaColourCorrection);
    press(&mut a, Btn::Right);
    let s = read_slot_state(d.path());
    assert!(s.colour_correction_gba, "the row never took");
    assert!(!s.colour_correction_gb, "the Game Boy picture changed too");
    assert_eq!(
        (s.ff_speed, s.ff_sound, s.rumble),
        (
            SlotState::default().ff_speed,
            SlotState::default().ff_sound,
            SlotState::default().rumble
        ),
        "the row reached a setting that is not its own"
    );
}

#[test]
fn the_arrows_change_nothing_on_a_row_that_opens() {
    let (d, mut a, _) = on_carousel();
    let before = std::fs::read(d.path().join("Config/slot.state")).expect("read slot.state");
    for row in [QuickRow::General, QuickRow::DateTime, QuickRow::About] {
        open_at(&mut a, row);
        press(&mut a, Btn::Left);
        press(&mut a, Btn::Right);
        assert_eq!(a.quick_menu(), Some(row), "an arrow left {row:?}");
        while a.quick_menu().is_some() {
            press(&mut a, Btn::B);
        }
    }
    assert_eq!(
        std::fs::read(d.path().join("Config/slot.state")).expect("read slot.state"),
        before,
        "an arrow on a row that opens wrote the card"
    );
}

#[test]
fn a_on_about_opens_the_label_and_b_comes_back_to_the_menu() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::About);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::About), "{:?}", a.phase());
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::About));
}

#[test]
fn x_on_about_restarts() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::About);
    press(&mut a, Btn::A);
    assert!(!a.restarting(), "opening About restarted");
    press(&mut a, Btn::X);
    assert!(
        a.restarting() && !a.powering_off(),
        "X on About did not restart"
    );
}

#[test]
fn select_start_on_about_still_restarts() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::About);
    press(&mut a, Btn::A);
    assert!(a.restart_chord());
    a.apply(Action::Restart);
    assert!(a.restarting() && !a.powering_off());
}

#[test]
fn the_about_footer_shows_back_and_x_restart() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::About);
    press(&mut a, Btn::A);
    let out = frame(&a);
    let (lx, _, _) = legend_box(&out, 400);
    assert!(lx.abs() < 1.0, "Back is not on the left: {lx}");
    let (rx, _, rw) = legend_box(&out, 404);
    let right = rx + rw - HINT_EDGE as f32;
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Restart is not on the right: {right}"
    );
}

#[test]
fn network_sits_between_controls_and_about() {
    let at = |row| QuickRow::MAIN.iter().position(|r| *r == row);
    assert_eq!(at(QuickRow::Network), at(QuickRow::Controls).map(|i| i + 1));
    assert_eq!(at(QuickRow::About), at(QuickRow::Network).map(|i| i + 1));
    assert_eq!(QuickRow::Network.label(), "Wireless");
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Network);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::WifiPower));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Network));
}

#[test]
fn fetch_missing_art_moved_to_network() {
    assert_eq!(
        QuickRow::NETWORK,
        [
            QuickRow::WifiPower,
            QuickRow::Networks,
            QuickRow::Server,
            QuickRow::FetchArt,
            QuickRow::CheckUpdates,
        ]
    );
    assert_eq!(
        QuickRow::NETWORK.map(QuickRow::label),
        [
            "Wi-Fi",
            "Network",
            "Gamebit",
            "Fetch Missing Art",
            "Check for Updates"
        ]
    );
    assert!(!QuickRow::GENERAL.contains(&QuickRow::FetchArt));
    assert_eq!(QuickRow::FetchArt.parent(), Some(QuickRow::Network));
    let (_d, mut a, _) = on_carousel();
    common::connect(&mut a);
    for row in QuickRow::NETWORK {
        assert!(!a.hidden_rows().contains(&row), "{row:?} is hidden");
    }
}

#[test]
fn wifi_row_toggles_with_a_and_left_right() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::WifiPower);
    assert_eq!(a.quick_value(QuickRow::WifiPower), Some(QuickValue::On));
    for (btn, off) in [
        (Btn::A, true),
        (Btn::A, false),
        (Btn::Left, true),
        (Btn::Right, false),
        (Btn::Right, true),
    ] {
        press(&mut a, btn);
        assert_eq!(a.wifi_is_off(), off, "{btn:?}");
        assert_eq!(
            a.quick_value(QuickRow::WifiPower),
            Some(QuickValue::flag(!off)),
            "{btn:?}"
        );
        assert_eq!(
            a.quick_menu(),
            Some(QuickRow::WifiPower),
            "{btn:?} moved the bar"
        );
    }
    assert!(read_slot_state(d.path()).wifi_off, "Off was not saved");
}

#[test]
fn wifi_off_leaves_only_the_wifi_row_on_the_network_page() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::WifiPower);
    press(&mut a, Btn::A);
    assert!(a.wifi_is_off());
    assert_eq!(
        QuickRow::WifiPower.shown(&a.hidden_rows()),
        [QuickRow::WifiPower]
    );
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::WifiPower));
    press(&mut a, Btn::A);
    assert!(QuickRow::WifiPower
        .shown(&a.hidden_rows())
        .contains(&QuickRow::Networks));
}

#[test]
fn the_network_row_waits_for_wlan0() {
    let (d, mut a, _) = on_carousel();
    let wlan0 = d.path().join("net/wlan0");
    a.set_wifi_probe(wlan0.clone());
    a.tick_ms(a.now() + 1);
    assert!(a.hidden_rows().contains(&QuickRow::Network));
    std::fs::create_dir_all(&wlan0).unwrap();
    a.tick_ms(a.now() + 1100);
    assert!(!a.hidden_rows().contains(&QuickRow::Network));
    std::fs::remove_dir(&wlan0).unwrap();
    a.tick_ms(a.now() + 1100);
    assert!(!a.hidden_rows().contains(&QuickRow::Network));
}

#[test]
fn the_networks_row_says_off_or_not_connected() {
    let (_d, mut a, _) = on_carousel();
    assert_eq!(a.networks_value(), "Not Connected");
    a.set_wifi_off(true);
    assert_eq!(a.networks_value(), "Off");
}

#[test]
fn fetch_missing_art_waits_for_a_connection() {
    let (_d, mut a, _) = on_carousel();
    assert!(a.hidden_rows().contains(&QuickRow::FetchArt));
    common::connect(&mut a);
    open_at(&mut a, QuickRow::FetchArt);
    press(&mut a, Btn::A);
    assert!(a.fetch_art().is_some(), "fetch did not open");
}

#[test]
fn a_on_date_and_time_opens_the_clock_at_the_time_it_already_has() {
    let (_d, mut a, _) = on_carousel_with(SlotState {
        zone: new_york(),
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    let picker = a.picker().expect("A on Date & Time did not open the clock");
    assert_eq!(picker.zone(), new_york(), "the city started over");
    assert_eq!(
        picker.secs(),
        CLOCK_IS_SET - CLOCK_IS_SET % 60,
        "the picker does not show the minute the clock is in"
    );
}

#[test]
fn b_on_the_clock_from_the_menu_saves_it_and_comes_back_to_the_menu() {
    let (d, mut a, clock) = on_carousel_with(SlotState {
        zone: new_york(),
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    let changed = a.picker().expect("not on the clock").secs() - (CLOCK_IS_SET - CLOCK_IS_SET % 60);
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::DateTime), "{:?}", a.phase());
    assert_eq!(
        clock.get(),
        CLOCK_IS_SET + changed,
        "the platform clock was not set"
    );
    let s = read_slot_state(d.path());
    assert_eq!(s.zone, new_york().east(), "the city was not saved");
    assert!(s.clock_set);
}

#[test]
fn b_on_the_first_boot_clock_saves_it_and_goes_on() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, _) = app_booting_with_clock(d.path());
    press(&mut a, Btn::B);
    assert!(
        !matches!(a.phase(), Phase::SetClock { .. }),
        "B did not leave the first boot clock"
    );
    assert!(
        read_slot_state(d.path()).clock_set,
        "B did not save the clock"
    );
}

#[test]
fn brightness_and_volume_still_answer_over_the_quick_menu() {
    let (d, mut a, _) = on_carousel();
    let icons: Vec<TexId> = (0..Icon::ALL.len())
        .map(|i| TexId::from_raw(700 + i))
        .collect();
    a.set_icon_faces(icons.clone());
    open_at(&mut a, QuickRow::FastForward);
    let before = read_slot_state(d.path());
    a.apply(Action::BrightnessUp);
    a.apply(Action::VolumeDown);
    a.flush_state();
    let after = read_slot_state(d.path());
    assert_eq!(
        (after.brightness, after.volume),
        (before.brightness + 1, before.volume - 5)
    );
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::FastForward),
        "a level moved the menu"
    );
    let out = frame(&a);
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if icons.contains(&tex))),
        "the level's bar is not drawn over the menu"
    );
}

fn fake_faces(a: &mut App) {
    let id = TexId::from_raw;
    a.set_quick_menu_faces(QuickMenuFaces {
        labels: (0..QuickRow::ALL.len())
            .map(|i| [(id(1000 + i), 200, 40), (id(1100 + i), 200, 40)])
            .collect(),
        pointer: (id(700), 32, 48),
        values: (0..QuickValue::ALL.len())
            .map(|i| [(id(2000 + i), 60, 40), (id(2100 + i), 60, 40)])
            .collect(),
        legend: [
            (id(400), 70),
            (id(401), 110),
            (id(402), 80),
            (id(403), 90),
            (id(404), 85),
            (id(405), 75),
            (id(406), 95),
            (id(407), 90),
        ],
        legend_light: [
            (id(400), 70),
            (id(401), 110),
            (id(402), 80),
            (id(403), 90),
            (id(404), 85),
            (id(405), 75),
            (id(406), 95),
            (id(407), 90),
        ],
        descriptions: [(id(410), 300, 44), (id(411), 340, 44)],
    });
    a.set_quick_clock_faces((id(500), 150, 40), (id(501), 150, 40));
    a.set_clock_legend_faces(
        [[
            (id(420), 80),
            (id(421), 110),
            (id(422), 90),
            (id(423), 100),
            (id(424), 100),
            (id(425), 80),
        ]; 2],
    );
    a.set_clock_title_face((id(430), 260, 40));
}

fn label(row: QuickRow, lit: bool) -> usize {
    if lit {
        1100 + row.index()
    } else {
        1000 + row.index()
    }
}

fn value(v: QuickValue, lit: bool) -> usize {
    if lit {
        2100 + v.index()
    } else {
        2000 + v.index()
    }
}

fn frame(a: &App) -> Vec<Draw> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out
}

fn placed(out: &[Draw], id: usize) -> Option<[f32; 4]> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == TexId::from_raw(id) => Some([x, y, w, h]),
        _ => None,
    })
}

fn drawn(out: &[Draw], id: usize) -> bool {
    placed(out, id).is_some()
}

#[test]
fn the_carousel_menu_keeps_the_empty_footer_where_the_shelf_left_it() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    for shelf in 0..6 {
        for index in 0..8 {
            a.set_cart_face(shelf, index, TexId::from_raw(79));
        }
    }
    a.set_slot_faces(TexId::from_raw(77), TexId::from_raw(78));
    let shelf = frame(&a);
    assert!(
        drawn(&shelf, 79),
        "the carousel lost its cart before the menu opened"
    );
    let shelf_mouth = strip_y(&a, 78);
    a.apply(Action::QuickMenu);
    let opened = strip_y(&a, 78);
    assert!(
        (opened - shelf_mouth).abs() < 1.0,
        "the carousel menu slid the footer: {shelf_mouth} -> {opened}"
    );
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    let parked = strip_y(&a, 78);
    assert!(
        (parked - (OUT_H as f32 - MOUTH_H)).abs() < 1.0,
        "the empty mouth is not the carousel strip: {parked}"
    );
    let open = frame(&a);
    assert!(
        open.iter()
            .any(|d| matches!(*d, Draw::Part { tex, .. } if tex == TexId::from_raw(77))),
        "the slot back is missing"
    );
    assert!(
        !drawn(&open, 79),
        "a cart is inserted in the footer while no game is playing"
    );
    let (lx, ly, _) = legend_box(&open, 400);
    let (rx, ry, rw) = legend_box(&open, 402);
    let footer = OUT_H as f32 - MOUTH_H;
    assert_eq!((ly, ry), (footer, footer));
    assert!(lx.abs() < 1.0, "Back is not on the left: {lx}");
    let right = rx + rw - HINT_EDGE as f32;
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Open is not on the right: {right}"
    );
    assert!(!drawn(&open, 403), "Resume is on the carousel menu");
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::FastForward));
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    let values = frame(&a);
    let (lx, ly, _) = legend_box(&values, 400);
    let (rx, ry, rw) = legend_box(&values, 401);
    let footer = OUT_H as f32 - MOUTH_H;
    assert_eq!((ly, ry), (footer, footer));
    assert!(lx.abs() < 1.0, "Back left the left edge: {lx}");
    let right = rx + rw - HINT_EDGE as f32;
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Change is not on the right: {right}"
    );
    assert!(!drawn(&values, 79), "the value page seated a cart");
}

fn strip_y(a: &App, id: usize) -> f32 {
    frame(a)
        .into_iter()
        .find_map(|d| match d {
            Draw::Part { y, tex, .. } if tex == TexId::from_raw(id) => Some(y),
            _ => None,
        })
        .unwrap_or_else(|| panic!("slot strip {id} is not drawn"))
}

fn legend_box(out: &[Draw], id: usize) -> (f32, f32, f32) {
    placed(out, id)
        .map(|b| (b[0], b[1], b[2]))
        .unwrap_or_else(|| panic!("legend {id} is not drawn"))
}

#[test]
fn the_legend_says_change_on_a_value_row_and_open_on_a_row_that_opens() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.apply(Action::QuickMenu);
    for row in QuickRow::MAIN {
        assert_eq!(a.quick_menu(), Some(row));
        let out = frame(&a);
        assert!(drawn(&out, 400), "no B BACK on {row:?}");
        assert_eq!(drawn(&out, 401), !row.opens(), "CHANGE on {row:?}");
        assert_eq!(drawn(&out, 402), row.opens(), "OPEN on {row:?}");
        press(&mut a, Btn::Down);
    }
}

#[test]
fn only_the_selected_rows_value_is_lit() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::FastForward);
    let out = frame(&a);
    assert!(
        drawn(&out, value(QuickValue::Speed6, true)),
        "6× is not lit"
    );
    assert!(
        drawn(&out, value(QuickValue::Off, false)),
        "sound's OFF is not grey"
    );
    assert!(
        drawn(&out, value(QuickValue::On, false)),
        "rumble's ON is not grey"
    );
    press(&mut a, Btn::B);
    for _ in QuickRow::General.position(&[])..QuickRow::Game.position(&[]) {
        press(&mut a, Btn::Up);
    }
    press(&mut a, Btn::A);
    for _ in 0..QuickRow::LidOff.position(&a.hidden_rows()) {
        press(&mut a, Btn::Down);
    }
    let out = frame(&a);
    assert!(drawn(&out, 500), "the date and time is not grey");

    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::DateTime));
    let out = frame(&a);
    assert!(drawn(&out, 501), "the date and time in hand is not lit");
}

#[test]
fn the_pointer_stands_left_of_the_selected_row_and_only_it_is_lit() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.apply(Action::QuickMenu);
    for row in QuickRow::MAIN {
        let out = frame(&a);
        let pointers: Vec<_> = out
            .iter()
            .filter(|d| matches!(d, Draw::Tex { tex, .. } if *tex == TexId::from_raw(700)))
            .collect();
        assert_eq!(pointers.len(), 1, "{row:?}: {} pointers", pointers.len());
        let [px, py, pw, ph] = placed(&out, 700).expect("the pointer");
        let [lx, ly, _, lh] = placed(&out, label(row, true)).expect("the lit label");
        assert!(
            px + pw <= lx + MENU_PAD as f32,
            "{row:?}: the pointer runs into its label"
        );
        assert_eq!(
            py + ph / 2.0,
            ly + lh / 2.0,
            "{row:?}: the pointer is off the row"
        );
        for other in QuickRow::MAIN.into_iter().filter(|r| *r != row) {
            assert!(drawn(&out, label(other, false)), "{other:?} is not grey");
            assert!(!drawn(&out, label(other, true)), "{other:?} is lit");
        }
        press(&mut a, Btn::Down);
    }
}

#[test]
fn labels_leave_room_for_the_pointer_and_values_end_thirty_two_pixels_in() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::FastForward);
    let out = frame(&a);
    let right = OUT_W as f32 - QUICK_EDGE;
    for row in QuickRow::FastForward.shown(&a.hidden_rows()) {
        let lit = row == QuickRow::FastForward;
        let [x, ..] = placed(&out, label(row, lit)).expect("a label was not drawn");
        assert_eq!(x + MENU_PAD as f32, QUICK_LABEL_X, "{row:?}'s label");
    }
    let [x, _, w, _] = placed(&out, value(QuickValue::On, false)).expect("rumble's value");
    assert_eq!(x + w - MENU_PAD as f32, right, "an unselected value");
    let [x, _, w, _] = placed(&out, value(QuickValue::Speed6, true)).expect("the selected value");
    assert_eq!(x + w - MENU_PAD as f32, right, "the selected value");
}

#[test]
fn only_the_first_boot_clock_says_set_the_clock() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut first, _) = app_booting_with_clock(d.path());
    fake_faces(&mut first);
    assert!(
        drawn(&frame(&first), 430),
        "first boot has no SET THE CLOCK"
    );

    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    assert!(
        a.picker().is_some() && !drawn(&frame(&a), 430),
        "the clock from the menu says SET THE CLOCK"
    );
}

#[test]
fn the_clock_offers_b_back_and_never_a_set() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    open_at(&mut a, QuickRow::DateTime);
    press(&mut a, Btn::A);
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut first, _) = app_booting_with_clock(d.path());
    fake_faces(&mut first);
    for (way, app, back) in [("the menu", &mut a, 400), ("first boot", &mut first, 425)] {
        let out = frame(app);
        assert!(drawn(&out, 423), "{way}: the date does not offer A CHANGE");
        assert!(!drawn(&out, 402), "{way}: the date still says A OPEN");
        assert!(drawn(&out, back), "{way}: no B BACK or B DONE");
        assert!(
            back == 400 || !drawn(&out, 400),
            "{way}: says B BACK where B saves and goes on"
        );
        press(app, Btn::Down);
        press(app, Btn::Down);
        let out = frame(app);
        assert!(drawn(&out, 402), "{way}: the city lost A OPEN");
        press(app, Btn::A);
        let out = frame(app);
        assert!(
            drawn(&out, 424),
            "{way}: the city list does not offer A SELECT"
        );
        assert!(!drawn(&out, 420), "{way}: the city list still says A DONE");
        press(app, Btn::B);
        assert!(!drawn(&out, 405), "{way}: the city still offers A SET");
        press(app, Btn::Down);
        let out = frame(app);
        assert!(drawn(&out, 401), "{way}: the 12/24 row lost CHANGE");
    }
}

#[test]
fn brightness_steps_from_zero_to_nine() {
    let (d, mut a, _) = on_carousel_with(SlotState {
        brightness: 3,
        ..SlotState::default()
    });
    open_at(&mut a, QuickRow::Brightness);
    assert_eq!(
        a.quick_value(QuickRow::Brightness),
        Some(QuickValue::Level3)
    );
    for (btn, want) in [
        (Btn::Left, 2),
        (Btn::Left, 1),
        (Btn::Left, 0),
        (Btn::Left, 0),
    ] {
        press(&mut a, btn);
        assert_eq!(a.brightness(), want, "{btn:?}");
    }
    for _ in 0..12 {
        press(&mut a, Btn::Right);
    }
    assert_eq!(a.brightness(), 9);
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).brightness, 9);
}

#[test]
fn blue_light_steps_off_to_nine() {
    let (_d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::BlueLight);
    assert_eq!(a.blue_light(), 0);
    assert_eq!(a.quick_value(QuickRow::BlueLight), Some(QuickValue::Off));
    press(&mut a, Btn::Right);
    assert_eq!(a.blue_light(), 1);
    assert_eq!(a.quick_value(QuickRow::BlueLight), Some(QuickValue::Level1));
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Left);
    assert_eq!(a.blue_light(), 0);
    for _ in 0..12 {
        press(&mut a, Btn::Right);
    }
    assert_eq!(a.blue_light(), 9);
}

#[test]
fn saturation_steps_off_to_nine_and_reaches_the_card() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Saturation);
    assert_eq!(a.saturation(), 0);
    assert_eq!(a.quick_value(QuickRow::Saturation), Some(QuickValue::Off));
    press(&mut a, Btn::Right);
    assert_eq!(a.saturation(), 1);
    assert_eq!(
        a.quick_value(QuickRow::Saturation),
        Some(QuickValue::Level1)
    );
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Left);
    assert_eq!(a.saturation(), 0);
    for _ in 0..12 {
        press(&mut a, Btn::Right);
    }
    assert_eq!(a.saturation(), 9);
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).saturation, 9);
}

#[test]
fn contrast_steps_off_to_nine_on_its_own_and_reaches_the_card() {
    let (d, mut a, _) = on_carousel();
    open_at(&mut a, QuickRow::Contrast);
    assert_eq!(a.contrast(), 0);
    assert_eq!(a.quick_value(QuickRow::Contrast), Some(QuickValue::Off));
    press(&mut a, Btn::Right);
    assert_eq!(a.contrast(), 1);
    assert_eq!(a.quick_value(QuickRow::Contrast), Some(QuickValue::Level1));
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Left);
    assert_eq!(a.contrast(), 0);
    for _ in 0..12 {
        press(&mut a, Btn::Right);
    }
    assert_eq!(a.contrast(), 9);
    assert_eq!(a.saturation(), 0, "contrast moved saturation");
    a.flush_state();
    let s = read_slot_state(d.path());
    assert_eq!((s.contrast, s.saturation), (9, 0));
}

#[test]
fn the_game_boy_page_steps_the_palette_from_auto() {
    let (d, mut a, _) = on_carousel();
    assert_eq!(
        QuickRow::SCREEN_GB,
        [
            QuickRow::GbShader,
            QuickRow::GbColourCorrection,
            QuickRow::GbBlending,
            QuickRow::GbPalette,
            QuickRow::GbBezel,
            QuickRow::GbScale,
            QuickRow::GbStretch,
        ]
    );
    open_at(&mut a, QuickRow::GbPalette);
    assert_eq!(a.gb_colors(), GbColors::Auto);
    assert_eq!(a.gb_colors_label(), "Auto");
    let grey = GbColors::Palette(GbPalette::parse("Grayscale").unwrap());
    let last = GbColors::Palette(GbPalette::parse("SGB 4-H").unwrap());
    for (btn, want) in [
        (Btn::Right, grey),
        (Btn::Left, GbColors::Auto),
        (Btn::Left, last),
    ] {
        press(&mut a, btn);
        assert_eq!(a.gb_colors(), want, "{btn:?}");
        assert_eq!(
            read_slot_state(d.path()).gb_colors(),
            want,
            "{btn:?} never reached the card"
        );
    }
}

#[test]
fn no_row_in_hand_runs_its_value_into_its_label() {
    let (_d, mut a) = on_own_wallpaper(slot_store::WallpaperTint::Off);
    let id = TexId::from_raw;
    a.set_quick_menu_faces(QuickMenuFaces {
        labels: QuickRow::ALL
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let w = slot_ui::quick_label_face(*r, true).w;
                [(id(1000 + i), w, 40), (id(1100 + i), w, 40)]
            })
            .collect(),
        pointer: (id(700), 32, 48),
        values: QuickValue::ALL
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let w = slot_ui::quick_value_face(v.text(), true).w;
                [(id(2000 + i), w, 40), (id(2100 + i), w, 40)]
            })
            .collect(),
        legend: [
            (id(400), 70),
            (id(401), 110),
            (id(402), 80),
            (id(403), 90),
            (id(404), 85),
            (id(405), 75),
            (id(406), 95),
            (id(407), 90),
        ],
        legend_light: [
            (id(400), 70),
            (id(401), 110),
            (id(402), 80),
            (id(403), 90),
            (id(404), 85),
            (id(405), 75),
            (id(406), 95),
            (id(407), 90),
        ],
        descriptions: [(id(410), 300, 44), (id(411), 340, 44)],
    });
    for page in [
        &QuickRow::SCREEN_GBA[..],
        &QuickRow::SCREEN_GBC[..],
        &QuickRow::SCREEN_GB[..],
        &QuickRow::GAME[..],
        &QuickRow::GENERAL[..],
    ] {
        for &row in page {
            if row.opens() || a.hidden_rows().contains(&row) {
                continue;
            }
            open_at(&mut a, row);
            let out = frame(&a);
            let [lx, _, lw, _] = placed(&out, 1100 + row.index()).expect("the lit label");
            let label_ends = lx + lw - MENU_PAD as f32;
            let lit_value = QuickValue::ALL
                .iter()
                .enumerate()
                .find_map(|(i, _)| placed(&out, 2100 + i));
            if let Some([vx, ..]) = lit_value {
                assert!(
                    vx + MENU_PAD as f32 - label_ends >= 16.0,
                    "{row:?} in hand puts its value {} px from its label",
                    vx + MENU_PAD as f32 - label_ends
                );
            }
            while a.quick_menu().is_some() {
                press(&mut a, Btn::B);
            }
        }
    }
}

#[test]
fn the_about_label_sits_on_the_same_tinted_ground_as_the_menu() {
    let (_d, mut a, _) = on_carousel();
    fake_faces(&mut a);
    a.set_ground(TexId::from_raw(800));
    a.set_blur(TexId::from_raw(801));
    open_at(&mut a, QuickRow::About);
    let menu = frame(&a);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::About), "{:?}", a.phase());
    let about = frame(&a);
    assert!(
        matches!(menu[0], Draw::Part { tex, .. } if tex == TexId::from_raw(801)),
        "the menu is not on the blurred wallpaper"
    );
    assert!(
        matches!(menu[1], Draw::Rect { colour, .. } if colour[..3] == [0.0; 3] && colour[3] > 0.5),
        "the menu's wallpaper is not tinted"
    );
    assert_eq!(
        about[..2],
        menu[..2],
        "the About label is not on the menu's tinted ground"
    );
}

fn on_own_wallpaper(tint: slot_store::WallpaperTint) -> (TempDir, App) {
    let (d, _, _) = on_carousel_with(SlotState {
        wallpaper: WallpaperPick::File("bright.png".into()),
        wallpaper_tint: tint,
        ..SlotState::default()
    });
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bright.png"), b"png").unwrap();
    let (mut a, _) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_ground(TexId::from_raw(800));
    a.set_wallpaper(TexId::from_raw(802));
    (d, a)
}

fn black_washes(draws: &[Draw]) -> Vec<f32> {
    draws
        .iter()
        .filter_map(|d| match d {
            Draw::Rect { x, y, w, h, colour }
                if *x == 0.0
                    && *y == 0.0
                    && *w == OUT_W as f32
                    && *h == slot_ui::OUT_H as f32
                    && colour[..3] == [0.0; 3] =>
            {
                Some(colour[3])
            }
            _ => None,
        })
        .collect()
}

#[test]
fn wallpaper_tint_hides_until_a_wallpaper_of_your_own_is_picked() {
    let (d, mut a, _) = on_carousel();
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.png"), b"png").unwrap();
    open_at(&mut a, QuickRow::Wallpaper);
    press(&mut a, Btn::Down);
    assert_eq!(
        a.quick_menu(),
        Some(QuickRow::Recents),
        "the tint row showed on the default wallpaper"
    );
    press(&mut a, Btn::Up);
    for (pick, shown) in [
        (WallpaperPick::Blur, false),
        (WallpaperPick::File("a.png".into()), true),
        (WallpaperPick::None, false),
        (WallpaperPick::Random, true),
        (WallpaperPick::Shelf, true),
        (WallpaperPick::Default, false),
    ] {
        press(&mut a, Btn::Right);
        assert_eq!(a.resolved_wallpaper(), pick);
        press(&mut a, Btn::Down);
        let below = if shown {
            QuickRow::WallpaperTint
        } else {
            QuickRow::Recents
        };
        assert_eq!(a.quick_menu(), Some(below), "below {pick:?}");
        press(&mut a, Btn::Up);
    }
}

#[test]
fn wallpaper_tint_steps_off_low_medium_high_and_saves() {
    use slot_store::WallpaperTint;
    let (d, mut a) = on_own_wallpaper(WallpaperTint::Off);
    open_at(&mut a, QuickRow::WallpaperTint);
    assert_eq!(
        a.quick_value(QuickRow::WallpaperTint),
        Some(QuickValue::Off)
    );
    for (value, tint) in [
        (QuickValue::Low, WallpaperTint::Low),
        (QuickValue::Medium, WallpaperTint::Medium),
        (QuickValue::High, WallpaperTint::High),
        (QuickValue::Off, WallpaperTint::Off),
    ] {
        press(&mut a, Btn::Right);
        assert_eq!(a.quick_value(QuickRow::WallpaperTint), Some(value));
        assert_eq!(read_slot_state(d.path()).wallpaper_tint, tint);
    }
    press(&mut a, Btn::Left);
    assert_eq!(
        a.quick_value(QuickRow::WallpaperTint),
        Some(QuickValue::High)
    );
}

#[test]
fn the_tint_darkens_your_own_wallpaper_on_the_shelf() {
    use slot_store::WallpaperTint;
    let (_d, a) = on_own_wallpaper(WallpaperTint::Off);
    assert_eq!(black_washes(&frame(&a)), Vec::<f32>::new());
    let mut seen = Vec::new();
    for tint in [
        WallpaperTint::Low,
        WallpaperTint::Medium,
        WallpaperTint::High,
    ] {
        let (_d, a) = on_own_wallpaper(tint);
        let draws = frame(&a);
        let wall = draws
            .iter()
            .position(|d| matches!(d, Draw::Part { tex, .. } if *tex == TexId::from_raw(802)))
            .expect("the wallpaper was not drawn");
        let washes = black_washes(&draws[wall..]);
        assert_eq!(washes.len(), 1, "{tint:?}: {washes:?}");
        seen.push(washes[0]);
    }
    assert!(
        seen.windows(2).all(|w| w[0] < w[1]),
        "each step is not darker than the last: {seen:?}"
    );
}

#[test]
fn the_tint_leaves_the_default_wallpaper_alone() {
    let (_d, mut a, _) = on_carousel_with(SlotState {
        wallpaper_tint: slot_store::WallpaperTint::High,
        ..SlotState::default()
    });
    a.set_ground(TexId::from_raw(800));
    assert_eq!(black_washes(&frame(&a)), Vec::<f32>::new());
}

#[test]
fn a_wide_panel_offers_gb_and_gbc_frames_from_none_through_the_card() {
    let (d, mut a, _) = on_carousel();
    std::fs::write(d.path().join("Bezels/GBC/Pocket.png"), b"png").unwrap();
    std::fs::write(d.path().join("Bezels/GBC/._Pocket.png"), b"no").unwrap();
    assert_eq!(a.resolved_bezel(Platform::Gbc), BezelPick::None);
    open_at(&mut a, QuickRow::GbcBezel);
    for want in [
        BezelPick::Default,
        BezelPick::File("Pocket.png".into()),
        BezelPick::None,
    ] {
        press(&mut a, Btn::Right);
        assert_eq!(a.resolved_bezel(Platform::Gbc), want);
    }
    press(&mut a, Btn::Left);
    assert_eq!(
        read_slot_state(d.path()).bezel_gbc,
        BezelPick::File("Pocket.png".into())
    );
    assert_eq!(a.resolved_bezel(Platform::Gb), BezelPick::None);
    press(&mut a, Btn::Down);
    assert_eq!(a.quick_menu(), Some(QuickRow::GbcScale));
    assert!(!a.sp_frame());
}

#[test]
fn a_four_by_three_panel_keeps_ags_102_until_another_frame_is_picked() {
    let (d, mut a, _) = on_carousel();
    a.set_panel((640, 480));
    for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        assert_eq!(
            a.resolved_bezel(platform),
            BezelPick::Ags102,
            "{platform:?}"
        );
    }
    assert!(a.sp_frame());
    open_at(&mut a, QuickRow::GbaBezel);
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_bezel(Platform::Gba), BezelPick::None);
    assert!(!a.sp_frame());
    press(&mut a, Btn::Right);
    assert_eq!(a.resolved_bezel(Platform::Gba), BezelPick::Ags102);
    while a.quick_menu().is_some() {
        press(&mut a, Btn::B);
    }
    open_at(&mut a, QuickRow::GbcBezel);
    press(&mut a, Btn::Left);
    assert_eq!(a.resolved_bezel(Platform::Gbc), BezelPick::Default);
    assert_eq!(read_slot_state(d.path()).bezel_gbc, BezelPick::Default);
    assert_eq!(read_slot_state(d.path()).bezel_gba, BezelPick::Ags102);
}

#[test]
fn gba_frames_from_the_card_show_only_on_a_four_by_three_panel() {
    let (d, mut a, _) = on_carousel();
    std::fs::write(d.path().join("Bezels/GBA/Classic.png"), b"png").unwrap();
    assert!(a.bezel_files(Platform::Gba).is_empty());
    a.set_panel((640, 480));
    assert_eq!(
        a.bezel_files(Platform::Gba),
        vec!["Classic.png".to_string()]
    );
    open_at(&mut a, QuickRow::GbaBezel);
    press(&mut a, Btn::Right);
    assert_eq!(
        a.resolved_bezel(Platform::Gba),
        BezelPick::File("Classic.png".into())
    );
    assert!(!a.sp_frame());
    assert_eq!(
        read_slot_state(d.path()).bezel_gba,
        BezelPick::File("Classic.png".into())
    );
    a.set_panel((720, 480));
    assert_eq!(a.resolved_bezel(Platform::Gba), BezelPick::None);
}

#[test]
fn a_frame_file_that_is_gone_reads_back_as_none() {
    let (_d, a, _) = on_carousel_with(SlotState {
        bezel_gb: BezelPick::File("gone.png".into()),
        ..SlotState::default()
    });
    assert_eq!(a.resolved_bezel(Platform::Gb), BezelPick::None);
}

#[test]
fn lid_power_off_steps_through_its_minutes_wraps_and_saves() {
    let (d, mut a, _) = on_carousel_with(SlotState {
        lid_off_min: Some(3),
        ..Default::default()
    });
    open_at(&mut a, QuickRow::LidOff);
    assert_eq!(a.lid_off_min(), 3);
    for want in [5, 10, 15, 30, 1, 3] {
        press(&mut a, Btn::Right);
        assert_eq!(a.lid_off_min(), want);
        assert_eq!(read_slot_state(d.path()).lid_off_min, Some(want));
    }
    press(&mut a, Btn::Left);
    assert_eq!(a.lid_off_min(), 1);
}

#[test]
fn scale_rows_offer_only_choices_that_differ() {
    let (_d, mut a, _) = on_carousel();
    a.set_panel((720, 480));
    assert_eq!(a.scale_choices(Platform::Gba), [Scale::Fit]);
    assert_eq!(a.scale_choices(Platform::Gb), [Scale::Integer, Scale::Fit]);
    assert_eq!(
        a.stretch_choices(Platform::Gb),
        [Stretch::Classic, Stretch::FullScreen]
    );
    let hidden = a.hidden_rows();
    assert!(hidden.contains(&QuickRow::GbaScale));
    assert!(!hidden.contains(&QuickRow::GbScale));
    assert!(!hidden.contains(&QuickRow::GbStretch));
    a.set_panel((640, 480));
    assert_eq!(
        a.scale_choices(Platform::Gba),
        [Scale::Fit, Scale::Integer, Scale::FullScreen]
    );
    assert_eq!(
        a.scale_choices(Platform::Gb),
        [Scale::SpFrame, Scale::Integer, Scale::Fit]
    );
    assert!(!a.hidden_rows().contains(&QuickRow::GbaScale));
}

#[test]
fn a_saved_scale_this_panel_lacks_shows_as_its_equal_and_is_kept() {
    let (d, mut a, _) = on_carousel();
    a.set_panel((720, 480));
    assert_eq!(a.resolved_scale(Platform::Gb), Scale::Integer);
    assert_eq!(a.quick_value(QuickRow::GbScale), Some(QuickValue::Integer));
    open_at(&mut a, QuickRow::GbStretch);
    press(&mut a, Btn::Right);
    assert_eq!(read_slot_state(d.path()).scale_gb, Scale::SpFrame);
    a.set_panel((640, 480));
    assert_eq!(a.resolved_scale(Platform::Gb), Scale::SpFrame);
    assert_eq!(a.quick_value(QuickRow::GbScale), Some(QuickValue::SpFrame));
}

#[test]
fn cycling_stretch_saves_and_applies() {
    let (d, mut a, _) = on_carousel();
    a.set_panel((640, 480));
    open_at(&mut a, QuickRow::GbStretch);
    assert_eq!(a.quick_menu(), Some(QuickRow::GbStretch));
    assert_eq!(
        a.quick_value(QuickRow::GbStretch),
        Some(QuickValue::Classic)
    );
    press(&mut a, Btn::Right);
    assert_eq!(read_slot_state(d.path()).stretch_gb, Stretch::FullScreen);
    assert_eq!(
        a.quick_value(QuickRow::GbStretch),
        Some(QuickValue::FullScreen)
    );
    a.set_platform(Platform::Gb);
    a.set_video_mode(slot::video_mode::VideoMode::Stretch);
    assert!(a.game_layout().panel);
    press(&mut a, Btn::Right);
    assert_eq!(read_slot_state(d.path()).stretch_gb, Stretch::Classic);
}

#[test]
fn fit_is_offered_on_both_panels() {
    let (_d, mut a, _) = on_carousel();
    a.set_panel((720, 480));
    assert_eq!(a.scale_choices(Platform::Gb), [Scale::Integer, Scale::Fit]);
    assert!(!a.hidden_rows().contains(&QuickRow::GbScale));
    a.set_panel((640, 480));
    assert_eq!(
        a.scale_choices(Platform::Gbc),
        [Scale::SpFrame, Scale::Integer, Scale::Fit]
    );
    assert_eq!(
        a.scale_choices(Platform::Gba),
        [Scale::Fit, Scale::Integer, Scale::FullScreen]
    );
}

#[test]
fn gba_skips_shimmerless_on_a_three_by_two_panel_and_keeps_it_on_four_by_three() {
    let (d, mut a, _) = on_carousel();
    a.set_panel((720, 480));
    open_at(&mut a, QuickRow::GbaShader);
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::GbaShader), Some(QuickValue::Dot));
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::GbaShader), Some(QuickValue::Off));
    assert_eq!(read_slot_state(d.path()).shader_gba, Shader::Off);
    press(&mut a, Btn::Left);
    assert_eq!(read_slot_state(d.path()).shader_gba, Shader::Dot);

    a.set_panel((640, 480));
    press(&mut a, Btn::Right);
    assert_eq!(
        a.quick_value(QuickRow::GbaShader),
        Some(QuickValue::Shimmerless)
    );
}

#[test]
fn game_boy_keeps_shimmerless_on_a_three_by_two_panel() {
    let (_d, mut a, _) = on_carousel();
    a.set_panel((720, 480));
    open_at(&mut a, QuickRow::GbShader);
    press(&mut a, Btn::Right);
    assert_eq!(
        a.quick_value(QuickRow::GbShader),
        Some(QuickValue::Shimmerless)
    );
}

#[test]
fn a_saved_gba_shimmerless_reads_as_off_on_three_by_two_and_survives_on_the_card() {
    let (d, mut a, _) = on_carousel_with(SlotState {
        shader_gba: Shader::Shimmerless,
        ..SlotState::default()
    });
    a.set_panel((720, 480));
    assert_eq!(a.quick_value(QuickRow::GbaShader), Some(QuickValue::Off));
    assert_eq!(read_slot_state(d.path()).shader_gba, Shader::Shimmerless);
    a.set_panel((640, 480));
    assert_eq!(
        a.quick_value(QuickRow::GbaShader),
        Some(QuickValue::Shimmerless)
    );
}
