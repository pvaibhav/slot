mod common;

use common::{tmp_root_with_carts, tmp_root_with_gb_carts, tmp_root_with_gbc_carts, StubSnapshot};
use slot::app::{App, Phase};
use slot::video_mode::VideoMode;
use slot_gfx::{GameLayout, TexId, WHOLE_TEXTURE};
use slot_input::Action;
use slot_store::{write_slot_state, BezelPick, Platform, Scale, SlotState, Stretch};
use slot_ui::{BezelArt, QuickRow, QuickValue};
use tempfile::TempDir;

const GB_WINDOW: [f32; 4] = [40.0 / 240.0, 0.0, 160.0 / 240.0, 1.0];
const GB_CROP: [f32; 4] = [40.0 / 240.0, 8.0 / 160.0, 160.0 / 240.0, 144.0 / 160.0];

fn root_for(platform: Platform) -> TempDir {
    match platform {
        Platform::Gba => tmp_root_with_carts(&["Game", "Zzz"]),
        Platform::Gb => tmp_root_with_gb_carts(&["Game", "Zzz"]),
        Platform::Gbc => tmp_root_with_gbc_carts(&["Game", "Zzz"]),
        Platform::Gbav => {
            let d = tmp_root_with_carts(&["Zzz"]);
            let dir = d.path().join("Games").join(Platform::Gbav.dir_name());
            std::fs::create_dir_all(&dir).unwrap();
            let mut rom = vec![0u8; 0x100];
            rom[0xa0..0xa4].copy_from_slice(b"GAME");
            std::fs::write(dir.join("Game.gba"), rom).unwrap();
            d
        }
        #[cfg(feature = "nds")]
        Platform::Nds => {
            let d = tmp_root_with_carts(&["Zzz"]);
            let dir = d.path().join("Games").join(Platform::Nds.dir_name());
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("Game.nds"), vec![0u8; 0x200]).unwrap();
            d
        }
    }
}

fn playing(platform: Platform, panel: (u32, u32), state: SlotState) -> (TempDir, App) {
    let d = root_for(platform);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Game".into()),
            cart_platform: Some(platform),
            clock_set: true,
            ..state
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_panel(panel);
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "not playing: {:?}",
        a.phase()
    );
    assert_eq!(a.playing_platform(), platform);
    (d, a)
}

#[test]
fn defaults_place_every_platform_as_before() {
    for panel in [(720, 480), (640, 480)] {
        for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
            let (_d, mut a) = playing(platform, panel, SlotState::default());
            for mode in [VideoMode::Actual, VideoMode::Stretch] {
                a.set_video_mode(mode);
                let want = match (platform, mode) {
                    (Platform::Gba, _) | (_, VideoMode::Actual) => WHOLE_TEXTURE,
                    _ => GB_WINDOW,
                };
                assert_eq!(
                    a.game_layout(),
                    GameLayout::BAND,
                    "{platform:?} {mode:?} {panel:?}"
                );
                assert_eq!(a.source_rect(), want, "{platform:?} {mode:?} {panel:?}");
            }
        }
    }
}

#[test]
fn gb_integer_on_4_by_3_takes_the_panel_and_drops_the_strip() {
    let (_d, mut a) = playing(
        Platform::Gb,
        (640, 480),
        SlotState {
            bezel_gb: BezelPick::Ags102,
            scale_gb: Scale::Integer,
            ..SlotState::default()
        },
    );
    assert!(a.game_layout().panel);
    assert_eq!(a.source_rect(), GB_CROP);
    assert!(!a.sp_frame());
    a.apply(Action::Eject);
    for _ in 0..600 {
        assert!(!a.sp_frame(), "the strip came back during {:?}", a.phase());
        a.update(1.0 / 60.0);
        if matches!(a.phase(), Phase::Shelf) {
            break;
        }
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn stretch_full_screen_crops_the_border_and_hides_bezels() {
    for panel in [(720, 480), (640, 480)] {
        let (_d, mut a) = playing(
            Platform::Gb,
            panel,
            SlotState {
                bezel_gb: BezelPick::Default,
                stretch_gb: Stretch::FullScreen,
                ..SlotState::default()
            },
        );
        a.set_game_bezel(Some(TexId::from_raw(77)));
        a.set_video_mode(VideoMode::Stretch);
        assert_eq!(a.source_rect(), GB_CROP, "{panel:?}");
        assert_eq!(a.game_layout().panel, panel == (640, 480));
        assert_eq!(a.shown_game_bezel(), None, "{panel:?}");
    }
}

#[test]
fn a_gba_cart_ignores_a_stale_stretch() {
    let (_d, mut a) = playing(Platform::Gba, (640, 480), SlotState::default());
    a.set_game_bezel(Some(TexId::from_raw(77)));
    a.set_video_mode(VideoMode::Stretch);
    assert_eq!(a.source_rect(), WHOLE_TEXTURE);
    assert_eq!(a.game_layout(), GameLayout::BAND);
    assert_eq!(a.shown_game_bezel(), Some(TexId::from_raw(77)));
}

#[test]
fn gba_video_follows_the_gba_scale() {
    let (_d, a) = playing(
        Platform::Gbav,
        (640, 480),
        SlotState {
            scale_gba: Scale::FullScreen,
            ..SlotState::default()
        },
    );
    assert!(a.game_layout().panel);
    let (_d, a) = playing(
        Platform::Gbav,
        (640, 480),
        SlotState {
            scale_gb: Scale::Integer,
            ..SlotState::default()
        },
    );
    assert_eq!(a.game_layout(), GameLayout::BAND);
}

#[test]
fn in_game_rows_follow_the_playing_platform() {
    let (_d, a) = playing(Platform::Gba, (640, 480), SlotState::default());
    let hidden = a.in_game_hidden();
    assert!(!hidden.contains(&QuickRow::Scale));
    assert!(hidden.contains(&QuickRow::Stretch));
    assert_eq!(a.quick_value(QuickRow::Scale), Some(QuickValue::Fit));
    let (_d, a) = playing(Platform::Gb, (720, 480), SlotState::default());
    let hidden = a.in_game_hidden();
    assert!(!hidden.contains(&QuickRow::Scale));
    assert!(!hidden.contains(&QuickRow::Stretch));
    assert_eq!(a.quick_value(QuickRow::Stretch), Some(QuickValue::Classic));
}

#[test]
fn bezel_art_follows_the_layout() {
    for (panel, scale, want) in [
        ((720, 480), Scale::SpFrame, Some(BezelArt::Default)),
        ((640, 480), Scale::SpFrame, Some(BezelArt::Default)),
        ((720, 480), Scale::Fit, Some(BezelArt::Fit3x2)),
        ((640, 480), Scale::Fit, Some(BezelArt::Fit4x3)),
        ((640, 480), Scale::Integer, Some(BezelArt::Integer4x3)),
        ((720, 480), Scale::Integer, Some(BezelArt::Default)),
    ] {
        let (_d, a) = playing(
            Platform::Gb,
            panel,
            SlotState {
                bezel_gb: BezelPick::Default,
                scale_gb: scale,
                ..SlotState::default()
            },
        );
        assert_eq!(a.bezel_art(), want, "{panel:?} {scale:?}");
    }
    let (_d, mut a) = playing(
        Platform::Gb,
        (720, 480),
        SlotState {
            bezel_gb: BezelPick::Default,
            ..SlotState::default()
        },
    );
    a.set_video_mode(VideoMode::Stretch);
    assert_eq!(a.bezel_art(), None);
}

#[test]
fn a_saved_integer_on_3_by_2_draws_exactly_as_default() {
    let (_d, a) = playing(
        Platform::Gb,
        (720, 480),
        SlotState {
            scale_gb: Scale::Integer,
            ..SlotState::default()
        },
    );
    assert_eq!(a.game_layout(), GameLayout::BAND);
    assert_eq!(a.source_rect(), WHOLE_TEXTURE);
}

fn bezels_in(
    platform: Platform,
    panel: (u32, u32),
    scale: Scale,
    picked: &str,
    files: &[&str],
) -> (Vec<String>, BezelPick) {
    let pick = BezelPick::File(picked.into());
    let state = match platform {
        Platform::Gba => SlotState {
            bezel_gba: pick,
            scale_gba: scale,
            ..SlotState::default()
        },
        _ => SlotState {
            bezel_gb: pick,
            scale_gb: scale,
            ..SlotState::default()
        },
    };
    let d = root_for(platform);
    let dir = d.path().join("Bezels").join(platform.dir_name());
    std::fs::create_dir_all(&dir).unwrap();
    for f in files {
        std::fs::write(dir.join(f), b"").unwrap();
    }
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Game".into()),
            cart_platform: Some(platform),
            clock_set: true,
            ..state
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_panel(panel);
    let mut shown: Vec<String> = a
        .bezel_files(platform)
        .into_iter()
        .filter(|name| {
            slot_store::bezel_scale(name).is_none_or(|s| s == a.resolved_scale(platform))
        })
        .collect();
    shown.sort();
    (shown, a.resolved_bezel(platform))
}

const ALL: [&str; 4] = [
    "Shell_fit.png",
    "Shell_integer.png",
    "Shell_spscreen.png",
    "Wood.png",
];

#[test]
fn the_bezel_list_holds_plain_bezels_and_those_for_the_scale() {
    for (platform, panel, scale, want) in [
        (
            Platform::Gb,
            (640, 480),
            Scale::SpFrame,
            ["Shell_spscreen.png", "Wood.png"],
        ),
        (
            Platform::Gb,
            (640, 480),
            Scale::Integer,
            ["Shell_integer.png", "Wood.png"],
        ),
        (
            Platform::Gb,
            (640, 480),
            Scale::Fit,
            ["Shell_fit.png", "Wood.png"],
        ),
        (
            Platform::Gb,
            (720, 480),
            Scale::Integer,
            ["Shell_integer.png", "Wood.png"],
        ),
        (
            Platform::Gba,
            (640, 480),
            Scale::Fit,
            ["Shell_fit.png", "Wood.png"],
        ),
    ] {
        let (shown, _) = bezels_in(platform, panel, scale, "Wood.png", &ALL);
        assert_eq!(shown, want, "{platform:?} {panel:?} {scale:?}");
    }
}

#[test]
fn a_plain_bezel_stays_picked_in_every_scale() {
    for scale in [Scale::SpFrame, Scale::Integer, Scale::Fit] {
        let (_, pick) = bezels_in(Platform::Gb, (640, 480), scale, "Wood.png", &ALL);
        assert_eq!(pick, BezelPick::File("Wood.png".into()), "{scale:?}");
    }
}

#[test]
fn a_suffixed_bezel_follows_the_scale_to_its_matching_file() {
    let (_, pick) = bezels_in(
        Platform::Gb,
        (640, 480),
        Scale::Integer,
        "Shell_fit.png",
        &ALL,
    );
    assert_eq!(pick, BezelPick::File("Shell_integer.png".into()));
    let only_fit = ["Shell_fit.png", "Wood.png"];
    let (_, pick) = bezels_in(
        Platform::Gb,
        (640, 480),
        Scale::Integer,
        "Shell_fit.png",
        &only_fit,
    );
    assert_eq!(pick, BezelPick::None);
}
