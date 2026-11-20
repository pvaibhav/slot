#![cfg(feature = "nds")]

mod common;

use common::{tmp_root_with_carts, StubSnapshot};
use slot::app::{App, Phase};
use slot_store::{write_slot_state, Platform, SlotState};

fn playing_ds() -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Kart".into()),
            cart_platform: Some(Platform::Nds),
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_panel((1024, 768));
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    assert_eq!(a.playing_platform(), Platform::Nds);
    (d, a)
}

#[test]
fn a_ds_game_does_not_fast_forward() {
    let (_d, a) = playing_ds();
    assert!(
        !a.may_fast_forward(),
        "a DS frame costs about 10.5 ms on the RG DS+, so fast forward only ever reached 1x"
    );
}

fn press(a: &mut App, b: slot_input::Btn) {
    a.apply(slot_input::Action::GbaDown(b));
    a.apply(slot_input::Action::GbaUp(b));
}

#[test]
fn a_ds_game_hides_fast_forward_and_rewind_and_gameplay_opens_on_turbo() {
    use slot_input::Btn;
    use slot_ui::QuickRow;
    let (_d, mut a) = playing_ds();
    let hidden = a.in_game_hidden();
    for row in [
        QuickRow::FastForward,
        QuickRow::FastForwardSound,
        QuickRow::Rewind,
    ] {
        assert!(
            hidden.contains(&row),
            "{row:?} is still offered in a DS game"
        );
    }
    a.apply(slot_input::Action::QuickMenu);
    for _ in 0..QuickRow::Settings.position(&a.in_game_hidden()) {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.in_game_menu(), Some(QuickRow::Settings));
    press(&mut a, Btn::A);
    common::reach(
        &mut a,
        QuickRow::PlayGameplay,
        |a| a.in_game_menu(),
        |a| press(a, Btn::Down),
    );
    press(&mut a, Btn::A);
    assert_eq!(a.in_game_menu(), Some(QuickRow::Rumble));
}

#[test]
fn a_ds_game_has_no_screen_shader_and_hides_the_row() {
    let (_d, a) = playing_ds();
    assert_eq!(a.screen_shader(), slot_store::Shader::Off);
    assert!(
        a.in_game_hidden().contains(&slot_ui::QuickRow::Shader),
        "the Shader row is still offered in a DS game"
    );
}

fn overlay_tile(root: &std::path::Path, dir: &str, name: &str, n: u32) {
    let dir = root.join("Overlays").join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, n, n);
    enc.set_color(png::ColorType::Rgba);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&vec![0u8; (n * n * 4) as usize])
        .unwrap();
    drop(writer);
    std::fs::write(dir.join(name), out).unwrap();
}

#[test]
fn the_in_game_overlay_row_picks_the_ds_overlay_and_leaves_the_gba_one() {
    use slot_input::Btn;
    use slot_ui::QuickRow;
    let (d, mut a) = playing_ds();
    overlay_tile(d.path(), "NDS", "grid_tile.png", 4);
    overlay_tile(d.path(), "GBA", "lcd_tile.png", 3);
    assert!(
        !a.in_game_hidden().contains(&QuickRow::Overlay),
        "no Overlay row in a DS game"
    );
    a.apply(slot_input::Action::QuickMenu);
    for _ in 0..QuickRow::Settings.position(&a.in_game_hidden()) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    for _ in 0..QuickRow::Overlay.position(&a.in_game_hidden()) {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.in_game_menu(), Some(QuickRow::Overlay));
    press(&mut a, Btn::Right);
    assert_eq!(
        a.resolved_overlay(Platform::Nds).map(|e| e.name).as_deref(),
        Some("grid")
    );
    assert_eq!(a.resolved_overlay(Platform::Gba), None);
}
