mod common;

use common::{app_playing_in, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{Blending, Core, Platform, Shader, StateRing};
use slot_ui::{
    Draw, QuickMenuFaces, QuickRow, QuickValue, TexId, HINT_EDGE, MOUTH_H, OUT_H, OUT_W,
};
use tempfile::TempDir;

fn playing() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = app_playing_in(d.path(), "Emerald");
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn go_to(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    let path: &[QuickRow] = match row.page() {
        p if p == QuickRow::PLAY => &[],
        p if p == QuickRow::SETTINGS => &[QuickRow::Settings],
        p if p == QuickRow::PLAY_SCREEN => &[QuickRow::Settings, QuickRow::PlayScreen],
        _ => &[QuickRow::Settings, QuickRow::PlayGameplay],
    };
    for step in path.iter().copied().chain([row]) {
        for _ in 0..12 {
            if a.in_game_menu() == Some(step) {
                break;
            }
            press(a, Btn::Down);
        }
        assert_eq!(a.in_game_menu(), Some(step), "never reached {step:?}");
        if step != row {
            press(a, Btn::A);
        }
    }
}

#[test]
fn a_tap_of_menu_in_game_opens_on_resume_over_the_running_cart() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
    assert!(matches!(a.phase(), Phase::Playing { cart } if cart == "Emerald"));
    assert_eq!(a.quick_menu(), None);
}

#[test]
fn resume_and_b_each_close_it_back_into_the_game() {
    for close in [Btn::A, Btn::B] {
        let (_d, mut a) = playing();
        a.apply(Action::QuickMenu);
        press(&mut a, close);
        assert_eq!(a.in_game_menu(), None, "{close:?}");
        assert!(matches!(a.phase(), Phase::Playing { .. }));
    }
}

#[test]
fn a_tap_of_menu_closes_it_from_any_page() {
    let (_d, mut a) = playing();
    go_to(&mut a, QuickRow::Rumble);
    a.apply(Action::QuickMenu);
    assert_eq!(a.in_game_menu(), None);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn b_climbs_the_in_game_pages_and_never_the_shelf_ones() {
    let (_d, mut a) = playing();
    go_to(&mut a, QuickRow::Rumble);
    press(&mut a, Btn::B);
    assert_eq!(a.in_game_menu(), Some(QuickRow::PlayGameplay));
    press(&mut a, Btn::B);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Settings));
    press(&mut a, Btn::B);
    assert_eq!(a.in_game_menu(), None);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn screen_shows_the_palette_only_when_palettes_are_live() {
    let (_d, mut a) = playing();
    let empty = [
        QuickRow::States,
        QuickRow::Screenshots,
        QuickRow::Guide,
        QuickRow::UndoSave,
        QuickRow::UndoLoad,
        QuickRow::UndoReset,
    ];
    assert_eq!(
        a.in_game_hidden(),
        [
            &[
                QuickRow::Palette,
                QuickRow::Bezel,
                QuickRow::Overlay,
                QuickRow::Scale,
                QuickRow::Stretch,
                QuickRow::LeftStick
            ][..],
            &empty
        ]
        .concat()
    );
    a.set_palette_live(true);
    assert_eq!(
        a.in_game_hidden(),
        [
            &[
                QuickRow::Bezel,
                QuickRow::Overlay,
                QuickRow::Scale,
                QuickRow::Stretch,
                QuickRow::LeftStick
            ][..],
            &empty
        ]
        .concat()
    );
}

#[test]
fn shader_edits_the_running_platforms_shader() {
    let (_d, mut a) = playing();
    let before = a.screen_shader();
    go_to(&mut a, QuickRow::Shader);
    press(&mut a, Btn::Right);
    assert_ne!(a.screen_shader(), before);
    assert_ne!(a.screen_shader(), Shader::Off);
    assert_eq!(
        a.quick_value(QuickRow::Shader),
        a.quick_value(QuickRow::GbaShader)
    );
}

#[test]
fn colour_correction_toggles_the_running_platform_and_asks_the_core() {
    let (_d, mut a) = playing();
    let before = a.colour_correction();
    go_to(&mut a, QuickRow::ColourCorrection);
    press(&mut a, Btn::Right);
    assert_eq!(a.colour_correction(), !before);
    assert_eq!(a.take_colour_correction(), Some(!before));
}

#[test]
fn frame_blending_steps_the_running_platform_and_asks_the_core() {
    let (_d, mut a) = playing();
    go_to(&mut a, QuickRow::Blending);
    assert_eq!(a.quick_value(QuickRow::Blending), Some(QuickValue::Off));
    for want in [Blending::Mix, Blending::Ghosting, Blending::Off] {
        press(&mut a, Btn::Right);
        assert_eq!(a.blending_for(Platform::Gba), want);
        assert_eq!(a.take_blending(), Some(want));
        assert_eq!(
            a.quick_value(QuickRow::Blending),
            a.quick_value(QuickRow::GbaBlending)
        );
    }
}

#[test]
fn on_gpsp_frame_blending_offers_off_and_mix_and_never_lcd_ghosting() {
    let (_d, mut a) = playing();
    a.set_core(Core::Gpsp);
    go_to(&mut a, QuickRow::Blending);
    let mut seen = Vec::new();
    for btn in [Btn::Right, Btn::Right, Btn::Right, Btn::Left, Btn::Left] {
        press(&mut a, btn);
        seen.push(a.quick_value(QuickRow::Blending).expect("no value"));
        assert_ne!(a.take_blending(), Some(Blending::Ghosting));
    }
    assert_eq!(
        seen,
        [
            QuickValue::Mix,
            QuickValue::Off,
            QuickValue::Mix,
            QuickValue::Off,
            QuickValue::Mix
        ]
    );
}

#[test]
fn an_lcd_ghosting_chosen_for_gba_reads_as_mix_on_gpsp() {
    let (_d, mut a) = playing();
    go_to(&mut a, QuickRow::Blending);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::Right);
    assert_eq!(a.blending_for(Platform::Gba), Blending::Ghosting);
    a.set_core(Core::Gpsp);
    assert_eq!(a.quick_value(QuickRow::Blending), Some(QuickValue::Mix));
    assert_eq!(a.blending_in_play(), Blending::Mix);
    assert_eq!(
        a.quick_value(QuickRow::GbaBlending),
        Some(QuickValue::Ghosting),
        "the console's own setting was changed just by playing on gpSP"
    );
}

#[test]
fn palette_steps_the_palette_and_asks_the_core() {
    let (_d, mut a) = playing();
    a.set_palette_live(true);
    let before = a.gb_colors_label();
    go_to(&mut a, QuickRow::Palette);
    press(&mut a, Btn::Right);
    assert_ne!(a.gb_colors_label(), before);
    assert!(a.take_gb_colors().is_some());
}

#[test]
fn the_in_game_screen_page_ends_with_brightness_blue_light_saturation_and_contrast() {
    let (d, mut a) = playing();
    let (power, backlight) = common::panel(d.path(), std::time::Duration::from_secs(300));
    a.set_power(power);
    let shown = QuickRow::PlayBrightness.shown(&a.in_game_hidden());
    assert_eq!(
        shown[shown.len() - 4..],
        [
            QuickRow::PlayBrightness,
            QuickRow::PlayBlueLight,
            QuickRow::PlaySaturation,
            QuickRow::PlayContrast
        ]
    );
    let before = a.brightness();
    go_to(&mut a, QuickRow::PlayBrightness);
    press(&mut a, Btn::Right);
    assert_eq!(a.brightness(), before + 1);
    assert_eq!(
        backlight.load(std::sync::atomic::Ordering::Relaxed),
        before + 2,
        "the row moved the number and not the backlight"
    );
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::PlayBlueLight));
    press(&mut a, Btn::Right);
    assert_eq!(a.blue_light(), 1);
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::PlaySaturation));
    press(&mut a, Btn::Right);
    assert_eq!(a.saturation(), 1);
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::PlayContrast));
    press(&mut a, Btn::Right);
    assert_eq!(a.contrast(), 1);
    assert_eq!(a.saturation(), 1);
}

#[test]
fn the_palette_row_shows_with_auto_in_a_game_boy_only_game() {
    let (_d, mut a) = playing();
    a.set_palette_live(true);
    assert_eq!(a.gb_colors(), slot_store::GbColors::Auto);
    assert!(!a.in_game_hidden().contains(&QuickRow::Palette));
    go_to(&mut a, QuickRow::Palette);
    assert_eq!(a.gb_colors_label(), "Auto");
    press(&mut a, Btn::Right);
    let grey = slot_store::GbColors::Palette(slot_store::GbPalette::parse("Grayscale").unwrap());
    assert_eq!(a.gb_colors(), grey);
    assert_eq!(a.take_gb_colors(), Some(grey));
}

#[test]
fn states_is_hidden_until_a_state_is_saved() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::States));
    a.apply(Action::QuickMenu);
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    assert!(!a.in_game_hidden().contains(&QuickRow::States));
}

#[test]
fn states_is_hidden_while_linked() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    a.begin_link(0);
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::States));
    assert!(a.in_game_hidden().contains(&QuickRow::Screenshots));
}

#[test]
fn screenshots_is_hidden_until_a_shot_is_taken() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::Screenshots));
    a.apply(Action::QuickMenu);
    a.shot_taken(4, 2, vec![0x80; 32]);
    a.flush_shots();
    a.apply(Action::QuickMenu);
    assert!(!a.in_game_hidden().contains(&QuickRow::Screenshots));
    let at = QuickRow::PLAY.iter().position(|r| *r == QuickRow::States);
    assert_eq!(
        at.map(|i| QuickRow::PLAY[i + 1]),
        Some(QuickRow::Screenshots),
        "Screenshots does not follow States"
    );
}

#[test]
fn a_state_under_another_core_does_not_show_states() {
    let (d, mut a) = playing();
    StateRing::new(d.path(), Platform::Gba, slot_store::Core::Gpsp, "Emerald")
        .push(&[1; 8], b"png", "2026-10-22_17-30-05")
        .expect("push");
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::States));
}

#[test]
fn states_opens_the_switcher_and_b_comes_back_to_the_menu() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(QuickRow::States));
}

#[test]
fn loading_from_states_lands_in_the_game_with_no_menu() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn undoing_a_load_from_states_lands_in_the_game_with_no_menu() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    assert_eq!(a.undo_label(), Some("undo load"));
    press(&mut a, Btn::X);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn undo_save_sits_under_resume_only_after_a_save() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::UndoSave));
    assert!(a.in_game_hidden().contains(&QuickRow::UndoLoad));
    a.apply(Action::QuickMenu);
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    assert!(!a.in_game_hidden().contains(&QuickRow::UndoSave));
    assert!(a.in_game_hidden().contains(&QuickRow::UndoLoad));
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::UndoSave));
}

#[test]
fn undo_save_from_the_menu_takes_the_save_back_and_resumes() {
    let (d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::UndoSave);
    press(&mut a, Btn::A);
    let r = StateRing::new(d.path(), Platform::Gba, a.core(), "Emerald");
    assert!(r.list().unwrap().is_empty(), "the save was not undone");
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
    assert!(!a.undo_available(a.now()));
}

#[test]
fn undo_load_from_the_menu_replaces_undo_save_and_resumes() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    a.apply(Action::LoadState);
    a.apply(Action::QuickMenu);
    assert!(a.in_game_hidden().contains(&QuickRow::UndoSave));
    assert!(!a.in_game_hidden().contains(&QuickRow::UndoLoad));
    a.apply(Action::QuickMenu);
    go_to(&mut a, QuickRow::UndoLoad);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
    assert!(!a.undo_available(a.now()));
}

#[test]
fn a_reset_puts_undo_reset_under_resume() {
    let (_d, mut a) = playing();
    a.apply(Action::Reset);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.apply(Action::QuickMenu);
    assert!(!a.in_game_hidden().contains(&QuickRow::UndoReset));
    press(&mut a, Btn::Down);
    assert_eq!(a.in_game_menu(), Some(QuickRow::UndoReset));
    press(&mut a, Btn::A);
    assert_eq!(a.in_game_menu(), None);
    assert!(!a.undo_available(a.now()));
}

#[test]
fn an_undo_row_that_expires_under_the_cursor_hands_it_to_resume() {
    let (_d, mut a) = playing();
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::QuickMenu, 1_100);
    a.apply_at(Action::GbaDown(Btn::Down), 1_200);
    a.apply_at(Action::GbaUp(Btn::Down), 1_250);
    assert_eq!(a.in_game_menu(), Some(QuickRow::UndoSave));
    a.tick_ms(40_000);
    assert!(a.in_game_hidden().contains(&QuickRow::UndoSave));
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
}

#[test]
fn a_double_tap_inside_the_menu_opens_states() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
}

#[test]
fn a_double_tap_in_game_opens_the_switcher_and_b_goes_back_to_the_game() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn a_refused_double_tap_in_game_leaves_the_game_playing() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn a_double_tap_inside_the_menu_comes_back_to_the_menu() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::Settings);
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    press(&mut a, Btn::B);
    assert_eq!(a.in_game_menu(), Some(QuickRow::States));
}

#[test]
fn dozing_in_states_wakes_back_to_the_menu_and_forgets_it_after() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Doze { .. }));
    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(QuickRow::States));
    press(&mut a, Btn::B);
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    press(&mut a, Btn::B);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn the_menu_waits_for_the_screen_to_finish_powering_on() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    a.apply(Action::PowerTap);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::PowerTap);
    a.update(1.0 / 60.0);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
    assert!(a.screen_power() < 1.0, "the screen was already fully on");
    let mut out = Vec::new();
    a.draw(&mut out);
    let scrim = out.iter().any(
        |d| matches!(d, slot_ui::Draw::Rect { colour, .. } if (colour[3] - 0.8).abs() < 0.001),
    );
    assert!(
        !scrim,
        "the menu drew over the slot while the screen powered on"
    );
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(out.iter().any(|d| {
        matches!(d, slot_ui::Draw::Rect { colour, .. } if (colour[3] - 0.8).abs() < 0.001)
    }));
}

#[test]
fn exit_game_ejects_like_holding_menu() {
    let (_d, mut a) = playing();
    go_to(&mut a, QuickRow::ExitGame);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn holding_menu_with_the_menu_open_ejects_and_drops_it() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn play_chords_do_nothing_while_the_menu_is_open() {
    let (d, mut a) = playing();
    let shader = a.screen_shader();
    let colour = a.colour_correction();
    a.apply(Action::QuickMenu);
    a.apply(Action::SaveState);
    a.apply(Action::ShaderNext);
    a.apply(Action::ColourCorrectionToggle);
    assert_eq!(a.screen_shader(), shader);
    assert_eq!(a.colour_correction(), colour);
    let ring = StateRing::new(d.path(), Platform::Gba, a.core(), "Emerald");
    assert!(ring.list().unwrap_or_default().is_empty());
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
}

#[test]
fn the_shelf_menu_still_opens_from_the_shelf() {
    let (_d, mut a) = playing();
    a.apply(Action::Eject);
    for _ in 0..240 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    a.apply(Action::QuickMenu);
    assert_eq!(a.quick_menu(), Some(QuickRow::MAIN[0]));
    assert_eq!(a.in_game_menu(), None);
}

#[test]
fn exit_game_is_not_offered_with_only_one_cart() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::QuickMenu);
    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(a.in_game_menu().expect("the menu closed"));
        press(&mut a, Btn::Down);
    }
    assert_eq!(
        seen,
        [
            QuickRow::Resume,
            QuickRow::UndoSave,
            QuickRow::States,
            QuickRow::Settings,
            QuickRow::Resume
        ]
    );
    assert!(a.in_game_hidden().contains(&QuickRow::ExitGame));
}

#[test]
fn exit_game_is_offered_with_two_carts() {
    let (_d, a) = playing();
    assert!(!a.in_game_hidden().contains(&QuickRow::ExitGame));
}

const CART: TexId = TexId::from_raw(77);
const OUT_H_F: f32 = slot_ui::OUT_H as f32;

fn paint_carts(a: &mut App) {
    for shelf in 0..6 {
        for index in 0..8 {
            a.set_cart_face(shelf, index, CART);
        }
    }
}

fn frames(a: &mut App, n: u32) {
    for _ in 0..n {
        a.update(1.0 / 60.0);
    }
}

fn cart_y(a: &App) -> f32 {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter()
        .filter_map(|d| match *d {
            Draw::Tex { y, tex, .. } if tex == CART => Some(y),
            _ => None,
        })
        .fold(None, |best, y| Some(best.map_or(y, |b: f32| b.max(y))))
        .expect("the seated cart is not drawn")
}

fn legend_at(a: &App, id: usize) -> (f32, f32, f32) {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter()
        .find_map(|d| match *d {
            Draw::Tex { x, y, w, tex, .. } if tex == TexId::from_raw(id) => Some((x, y, w)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("legend {id} is not drawn"))
}

fn menu_faces(a: &mut App) {
    let id = TexId::from_raw;
    a.set_quick_menu_faces(QuickMenuFaces {
        labels: (0..QuickRow::ALL.len())
            .map(|i| [(id(100 + i), 80, 40), (id(130 + i), 80, 40)])
            .collect(),
        pointer: (id(700), 16, 40),
        values: Vec::new(),
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
}

#[test]
fn the_pause_menu_slides_the_carousel_footer_up_and_back_down() {
    let (_d, mut a) = playing();
    paint_carts(&mut a);
    a.apply(Action::QuickMenu);
    frames(&mut a, 14);
    let rising = cart_y(&a);
    frames(&mut a, 20);
    let open = cart_y(&a);
    assert!(
        open < rising - 20.0,
        "the footer did not rise: {rising} -> {open}"
    );
    assert!(
        open >= OUT_H_F - MOUTH_H - 1.0,
        "the cart stands above the footer at {open}"
    );
    a.apply(Action::QuickMenu);
    frames(&mut a, 6);
    let falling = cart_y(&a);
    assert!(
        falling > open + 8.0,
        "closing the menu left the footer at {falling}, open was {open}"
    );
    frames(&mut a, 20);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        !out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == CART)),
        "the footer stayed up after the menu closed"
    );
}

#[test]
fn pause_legends_sit_on_the_link_line_at_either_end() {
    let (_d, mut a) = playing();
    menu_faces(&mut a);
    a.apply(Action::QuickMenu);
    frames(&mut a, 30);
    let (lx, ly, _) = legend_at(&a, 403);
    let (rx, ry, rw) = legend_at(&a, 402);
    let footer = OUT_H as f32 - MOUTH_H;
    assert_eq!(ly, footer, "Resume does not fill the footer");
    assert_eq!(ry, footer, "Open does not fill the footer");
    assert!(lx.abs() < 1.0, "Resume is not on the left: {lx}");
    let right = rx + rw - HINT_EDGE as f32;
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Open is not on the right: {right}"
    );
    assert!(legend_missing(&a, 400), "Back is drawn beside Resume");
}

fn legend_missing(a: &App, id: usize) -> bool {
    let mut out = Vec::new();
    a.draw(&mut out);
    !out.iter()
        .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(id)))
}

#[test]
fn a_value_row_puts_change_on_the_right_of_the_footer() {
    let (_d, mut a) = playing();
    menu_faces(&mut a);
    go_to(&mut a, QuickRow::Shader);
    frames(&mut a, 30);
    let (lx, ly, _) = legend_at(&a, 400);
    let (rx, ry, rw) = legend_at(&a, 401);
    assert_eq!((ly, ry), (OUT_H as f32 - MOUTH_H, OUT_H as f32 - MOUTH_H));
    assert!(lx.abs() < 1.0, "Back is not on the left: {lx}");
    let right = rx + rw - HINT_EDGE as f32;
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Change is not on the right: {right}"
    );
    assert!(
        legend_missing(&a, 403),
        "Resume stayed up off the play page"
    );
}

#[test]
fn closing_the_game_from_the_menu_keeps_the_footer_still_above_the_scaling_picture() {
    let (_d, mut a) = playing();
    paint_carts(&mut a);
    assert_eq!(a.screen_footer(), 0.0);
    a.apply(Action::QuickMenu);
    frames(&mut a, 34);
    a.apply(Action::Eject);
    let mut seen = 0.0;
    let mut y: Option<f32> = None;
    for _ in 0..40 {
        if a.screen_power() == 0.0 {
            break;
        }
        assert_eq!(a.screen_footer(), MOUTH_H);
        let at = cart_y(&a);
        if let Some(prev) = y {
            assert!(
                (at - prev).abs() < 0.5,
                "the footer moved from {prev} to {at} while the picture scaled"
            );
        }
        y = Some(at);
        seen = a.screen_power();
        a.update(1.0 / 60.0);
    }
    assert!(
        seen < 1.0 && seen > 0.0,
        "the picture never scaled, last {seen}"
    );
    let parked = y.expect("no footer during the close");
    assert!(
        parked >= OUT_H_F - MOUTH_H - 1.0,
        "the cart left the carousel footer at {parked}"
    );
}

#[test]
fn a_hotkey_eject_slides_the_footer_up_while_the_picture_closes() {
    let (_d, mut a) = playing();
    paint_carts(&mut a);
    a.apply(Action::Eject);
    let mut seen = Vec::new();
    for _ in 0..40 {
        a.update(1.0 / 60.0);
        if a.screen_power() == 0.0 {
            break;
        }
        seen.push(a.screen_footer());
    }
    assert!(
        seen.len() > 2,
        "the picture closed in {} frames",
        seen.len()
    );
    assert!(
        seen[0] < MOUTH_H / 2.0,
        "the footer jumped up to {} on the first frame",
        seen[0]
    );
    assert!(
        seen.windows(2).all(|w| w[1] >= w[0]),
        "the footer did not rise steadily: {seen:?}"
    );
    let last = *seen.last().unwrap();
    assert!(
        last > MOUTH_H * 0.8,
        "the footer was only at {last} when the picture went"
    );
}

fn cursor_on_a_shown_row(a: &App) {
    let row = a.in_game_menu().expect("the menu closed");
    assert!(
        !a.in_game_hidden().contains(&row),
        "the cursor sits on hidden {row:?}"
    );
}

#[test]
fn deleting_the_last_state_closes_and_hides_the_row() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    press(&mut a, Btn::Y);
    press(&mut a, Btn::X);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert!(a.in_game_hidden().contains(&QuickRow::States));
    cursor_on_a_shown_row(&a);
}

#[test]
fn undoing_the_only_save_from_states_hides_the_row() {
    let (_d, mut a) = playing();
    a.apply(Action::SaveState);
    go_to(&mut a, QuickRow::States);
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::Polaroids { .. }));
    press(&mut a, Btn::X);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert!(a.in_game_hidden().contains(&QuickRow::States));
    cursor_on_a_shown_row(&a);
}

#[test]
fn a_refused_double_tap_in_the_menu_with_no_states_stays_on_a_shown_row() {
    let (_d, mut a) = playing();
    a.apply(Action::QuickMenu);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Resume));
    a.apply(Action::QuickMenu);
    a.apply(Action::Polaroids);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    cursor_on_a_shown_row(&a);
}
