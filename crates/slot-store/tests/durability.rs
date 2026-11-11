mod common;

use common::tmp_root;
use slot_store::{
    atomic_write, read_slot_state, sweep_temps, write_slot_state, Appearance, BezelPick,
    Blending, Core, GbPalette, Platform, Recent, Scale, Shader, SlotState, Stretch, WallpaperPick,
    WallpaperTint, Zone, FF_SPEEDS, FF_SPEED_DEFAULT,
};
use tempfile::tempdir;

#[test]
fn fork_ghosting_preferences_migrate_to_upstream_blending_and_survive_a_rewrite() {
    let d = tmp_root();
    let path = d.path().join("Config/slot.state");
    std::fs::write(
        &path,
        "cart=Emerald\ncart_platform=gba\nbrightness=3\nblue_light=1\nvolume=40\nmuted=1\nclock_set=1\nutc_offset_min=330\nhigh_refresh=0\nhome_wifi_enabled=1\nghosting_gba=0\nghosting_gb=1\nghosting_gbc=0\n",
    )
    .unwrap();
    let state = read_slot_state(d.path());
    assert_eq!(state.cart.as_deref(), Some("Emerald"));
    assert_eq!(state.cart_platform, Some(Platform::Gba));
    assert_eq!(state.brightness, 3);
    assert_eq!(state.volume, 40);
    assert!(state.muted);
    assert!(!state.high_refresh);
    assert_eq!(state.zone, Zone::parse("Asia/Kolkata").unwrap());
    assert_eq!(
        (state.blending_gba, state.blending_gb, state.blending_gbc),
        (Blending::Off, Blending::Mix, Blending::Off)
    );
    write_slot_state(d.path(), &state).unwrap();
    assert_eq!(read_slot_state(d.path()), state);
    assert!(!std::fs::read_to_string(path).unwrap().contains("ghosting_"));
}

#[test]
fn older_fork_cards_keep_mix_and_upstream_cards_keep_off() {
    for (extra, want) in [("high_refresh=1\n", Blending::Mix), ("", Blending::Off)] {
        let d = tmp_root();
        std::fs::write(
            d.path().join("Config/slot.state"),
            format!("cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n{extra}"),
        )
        .unwrap();
        let state = read_slot_state(d.path());
        assert_eq!(
            (state.blending_gba, state.blending_gb, state.blending_gbc),
            (want, want, want)
        );
    }
}

#[test]
fn upstream_blending_choices_override_legacy_preferences_in_either_order() {
    for (first, second) in [
        ("ghosting_gba=0\n", "blending_gba=ghosting\n"),
        ("blending_gba=ghosting\n", "ghosting_gba=0\n"),
    ] {
        let d = tmp_root();
        std::fs::write(
            d.path().join("Config/slot.state"),
            format!("cart=\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n{first}{second}"),
        )
        .unwrap();
        assert_eq!(read_slot_state(d.path()).blending_gba, Blending::Ghosting);
    }
}

#[test]
fn atomic_write_leaves_no_partial_file_and_no_temp_behind() {
    let d = tempdir().unwrap();
    let p = d.path().join("x.bin");
    atomic_write(&p, b"first").unwrap();
    atomic_write(&p, &vec![7u8; 4_000_000]).unwrap();
    assert_eq!(std::fs::read(&p).unwrap().len(), 4_000_000);
    let strays: Vec<_> = std::fs::read_dir(d.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() != "x.bin")
        .collect();
    assert!(strays.is_empty(), "temp files left behind: {strays:?}");
}

#[test]
fn a_temp_left_by_an_interrupted_write_is_swept_and_nothing_else() {
    let d = tempdir().unwrap();
    let deep = d.path().join("States/GBA/mgba/Emerald");
    std::fs::create_dir_all(&deep).unwrap();
    let stray = deep.join(".resume.state.4242.7.tmp");
    let mine = deep.join(format!(".resume.state.{}.0.tmp", std::process::id()));
    let keep = [
        deep.join("resume.state"),
        deep.join(".hidden.tmp"),
        deep.join("notes.4242.7.tmp"),
        d.path().join(".x.y.tmp"),
    ];
    for p in keep.iter().chain([&stray, &mine]) {
        std::fs::write(p, b"bytes").unwrap();
    }
    sweep_temps(d.path());
    assert!(
        !stray.exists(),
        "a temp from an earlier run was left behind"
    );
    assert!(mine.exists(), "swept a temp this run may still be writing");
    for p in &keep {
        assert!(p.exists(), "swept {}", p.display());
    }
}

#[test]
fn a_name_the_card_accepts_is_a_name_that_can_be_written() {
    let d = tempdir().unwrap();
    for len in [8usize, 100, 200, 239, 240, 245, 250, 251] {
        let p = d.path().join(format!("{}.sav", "a".repeat(len)));
        if std::fs::write(&p, b"control").is_err() {
            continue;
        }
        atomic_write(&p, b"save")
            .unwrap_or_else(|e| panic!("{len} character name: {e}, and a plain write took it"));
        assert_eq!(std::fs::read(&p).unwrap(), b"save");
    }
}

#[test]
fn a_long_multibyte_name_is_written_rather_than_panicking() {
    let d = tempdir().unwrap();
    for pad in 0..3 {
        let p = d
            .path()
            .join(format!("{}{}.sav", "x".repeat(pad), "ポ".repeat(82)));
        if std::fs::write(&p, b"control").is_err() {
            continue;
        }
        atomic_write(&p, b"save")
            .unwrap_or_else(|e| panic!("a Japanese cart name, pad {pad}: {e}"));
        assert_eq!(std::fs::read(&p).unwrap(), b"save");
    }
}

#[test]
fn corrupt_slot_state_reads_as_default_rather_than_panicking() {
    let d = tmp_root();
    std::fs::write(d.path().join("Config/slot.state"), b"\x00\xff not json").unwrap();
    assert_eq!(read_slot_state(d.path()), SlotState::default());
}

#[test]
fn slot_state_round_trips_including_a_stem_with_an_equals_sign() {
    let d = tmp_root();
    let s = SlotState {
        cart: Some("Cheats = On".into()),
        brightness: 0,
        dim: 2,
        blue_light: 9,
        volume: 71,
        muted: true,
        clock_set: true,
        gb_model: slot_store::GbModel::Light,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()), s);
}

#[test]
fn a_slot_state_missing_a_key_reads_as_default_not_half_populated() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/slot.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\n",
    )
    .unwrap();
    assert_eq!(read_slot_state(d.path()), SlotState::default());
}

#[test]
fn an_out_of_range_level_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=10\nblue_light=1\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=10\nvolume=50\n",
        "cart=\nbrightness=3\nblue_light=1\nvolume=101\n",
    ] {
        std::fs::write(d.path().join("Config/slot.state"), body).unwrap();
        assert_eq!(
            read_slot_state(d.path()),
            SlotState::default(),
            "accepted {body:?}"
        );
    }
}

#[test]
fn a_first_boot_is_neither_dark_nor_silent() {
    let d = tmp_root();
    let s = read_slot_state(d.path());
    assert!(s.cart.is_none());
    assert!(s.brightness > 0, "boots with the backlight off");
    assert!(s.volume > 0, "boots muted");
}

#[test]
fn slot_state_round_trips_a_zone_west_of_utc() {
    let d = tmp_root();
    let s = SlotState {
        cart: None,
        brightness: 5,
        blue_light: 0,
        volume: 60,
        muted: false,
        clock_set: true,
        zone: Zone::parse("America/St_Johns").unwrap(),
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()).zone, s.zone);
}

#[test]
fn a_line_the_reader_does_not_know_is_skipped() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/slot.state"),
        "cart=Emerald\nbrightness=3\nfrom_a_later_build=7\nblue_light=1\nvolume=40\nmuted=1\n\
         no equals sign at all\nclock_set=1\nutc_offset_min=-300\n",
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!(
        s.cart.as_deref(),
        Some("Emerald"),
        "the file was thrown away"
    );
    assert_eq!(
        (
            s.brightness,
            s.blue_light,
            s.volume,
            s.muted,
            s.clock_set,
            s.zone.offset_min(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            )
        ),
        (3, 1, 40, true, true, -300)
    );
}

/// What a card that has never been asked gets: the motor on, fast forward at the default,
/// silent, and the picture the core's own colours. The speed is the one constant here that has
/// moved — it was four, inherited from when gpSP ran its interpreter and could not serve more,
/// and is now six, chosen on the device.
///
/// Colour correction off is not an aesthetic preference being enshrined: it is what every card
/// already renders as, because both cores default their own option off and slot never set it.
/// A default of on would change the look of every existing library on the strength of an
/// update nobody asked for.
#[test]
fn a_first_boot_rumbles_and_fast_forwards_silently_at_the_default() {
    let s = SlotState::default();
    assert!(s.rumble, "boots with the motor off");
    assert_eq!(s.ff_speed, FF_SPEED_DEFAULT);
    assert!(!s.ff_sound, "boots with fast forward audible");
    assert!(
        !s.colour_correction_gba,
        "boots with the GBA picture tinted"
    );
    assert!(
        !s.colour_correction_gb,
        "boots with the Game Boy picture tinted"
    );
    assert_eq!(s.shader_gba, Shader::Grid);
    assert_eq!(s.shader_gb, Shader::Simpletex);
    assert!(
        s.high_refresh,
        "defaults to high refresh on a supported panel"
    );
}

#[test]
fn a_card_from_before_the_settings_keeps_all_its_values() {
    let d = tmp_root();
    std::fs::write(
        d.path().join("Config/slot.state"),
        "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=1\nclock_set=1\nutc_offset_min=-300\n",
    )
    .unwrap();
    assert_eq!(
        read_slot_state(d.path()),
        SlotState {
            cart: Some("Emerald".into()),
            cart_platform: None,
            brightness: 3,
            dim: 0,
            blue_light: 1,
            saturation: 0,
            contrast: 0,
            volume: 40,
            volume_hp: 40,
            muted: true,
            muted_hp: true,
            clock_set: true,
            zone: Zone::for_offset(
                -300,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            ),
            clock_24h: true,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction_gba: false,
            colour_correction_gb: false,
            colour_correction_gbc: false,
            shader_gba: Shader::Grid,
            shader_gb: Shader::Simpletex,
            shader_gbc: Shader::Simpletex,
            resume_insert: true,
            resume_power_on: true,
            turbo: true,
            rewind: true,
            high_refresh: true,
            gb_palettes: false,
            gb_palette: GbPalette::DEFAULT,
            appearance: Appearance::Dark,
            wallpaper: WallpaperPick::Default,
            wallpaper_tint: WallpaperTint::Off,
            show_recents: false,
            show_titles: false,
            left_stick_dpad: true,
            lid_off_min: None,
            wifi_off: false,
            network_time: false,
            bezel_gba: BezelPick::Ags102,
            bezel_gb: BezelPick::Ags102,
            bezel_gbc: BezelPick::Ags102,
            reflection_gba: true,
            reflection_gb: true,
            reflection_gbc: true,
            scale_gba: Scale::Fit,
            scale_gb: Scale::SpFrame,
            scale_gbc: Scale::SpFrame,
            stretch_gb: Stretch::Classic,
            stretch_gbc: Stretch::Classic,
            blending_gba: Blending::Off,
            blending_gb: Blending::Off,
            gb_model: slot_store::GbModel::Pocket,
            blending_gbc: Blending::Off,
            gb_core: Core::Mgba,
            recents: Vec::new(),
            favorites: Vec::new(),
            shelf: None,
            shelf_cart: None,
            ..SlotState::default()
        }
    );
}

#[test]
fn the_quick_menu_settings_round_trip_as_their_own_lines() {
    let d = tmp_root();
    let s = SlotState {
        clock_set: true,
        rumble: false,
        ff_speed: 8,
        ff_sound: true,
        colour_correction_gba: true,
        colour_correction_gb: false,
        shader_gba: Shader::Dot,
        shader_gb: Shader::Off,
        resume_insert: false,
        resume_power_on: true,
        turbo: false,
        rewind: false,
        high_refresh: true,
        gb_palettes: true,
        gb_palette: GbPalette::parse("GBC Dark Green →A").unwrap(),
        wallpaper_tint: WallpaperTint::Medium,
        colour_correction_gbc: true,
        shader_gbc: Shader::Shimmerless,
        saturation: 4,
        contrast: 7,
        bezel_gba: BezelPick::None,
        bezel_gb: BezelPick::Default,
        bezel_gbc: BezelPick::File("Pocket.png".into()),
        reflection_gb: false,
        blending_gba: Blending::Ghosting,
        blending_gb: Blending::Mix,
        shelf: Some("gba".into()),
        shelf_cart: Some(Recent {
            platform: Platform::Gba,
            stem: "Drill Dozer".into(),
        }),
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()), s);
    let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
    for line in [
        "rumble=0",
        "ff_speed=2",
        "ff_sound=1",
        "colour_correction_gba=1",
        "colour_correction_gb=0",
        "shader_gba=dot",
        "shader_gb=off",
        "resume_insert=0",
        "resume_power_on=1",
        "turbo=0",
        "rewind=0",
        "high_refresh=1",
        "gb_palettes=1",
        "gb_palette=GBC Dark Green →A",
        "appearance=dark",
        "wallpaper_tint=medium",
        "bezel_gba=none",
        "bezel_gb=default",
        "bezel_gbc=Pocket.png",
        "reflection_gba=1",
        "reflection_gb=0",
        "colour_correction_gbc=1",
        "shader_gbc=shimmerless",
        "saturation=4",
        "contrast=7",
        "blending_gba=ghosting",
        "blending_gb=mix",
        "blending_gbc=off",
        "shelf=gba",
        "shelf_cart=gba/Drill Dozer",
    ] {
        assert!(text.lines().any(|l| l == line), "no {line} in {text:?}");
    }
}

#[test]
fn a_card_that_still_carries_run_ahead_keeps_the_rest_of_its_values() {
    let d = tmp_root();
    let s = SlotState {
        clock_set: true,
        rewind: false,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let path = d.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("rewind=0\n", "rewind=0\nrunahead=1\n")).unwrap();
    assert!(std::fs::read_to_string(&path)
        .unwrap()
        .contains("runahead=1"));
    assert_eq!(read_slot_state(d.path()), s);
}

#[test]
fn an_old_card_uses_the_new_refresh_default_and_ignores_run_ahead() {
    let d = tmp_root();
    let state = SlotState {
        brightness: 7,
        clock_set: true,
        rewind: false,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &state).unwrap();
    let path = d.path().join("Config/slot.state");
    let old = std::fs::read_to_string(&path)
        .unwrap()
        .replace("high_refresh=1\n", "runahead=1\n");
    std::fs::write(&path, old).unwrap();
    let migrated = read_slot_state(d.path());
    assert!(migrated.high_refresh);
    assert_eq!(migrated.brightness, 7);
    assert!(migrated.clock_set);
    assert!(!migrated.rewind);
    write_slot_state(d.path(), &migrated).unwrap();
    assert_eq!(read_slot_state(d.path()), migrated);
    assert!(!std::fs::read_to_string(&path)
        .unwrap()
        .contains("runahead="));
}

#[test]
fn an_explicit_refresh_setting_is_retained() {
    for high_refresh in [false, true] {
        let d = tmp_root();
        let state = SlotState {
            high_refresh,
            ..SlotState::default()
        };
        write_slot_state(d.path(), &state).unwrap();
        let read = read_slot_state(d.path());
        assert_eq!(read.high_refresh, high_refresh);
    }
}

#[test]
fn every_speed_the_row_offers_round_trips_as_its_own_number() {
    assert_eq!(FF_SPEEDS, [0, 6, 8, 12, 16, 24]);
    for (speed, written) in FF_SPEEDS.into_iter().zip(["0", "1.5", "2", "3", "4", "6"]) {
        let d = tmp_root();
        let s = SlotState {
            clock_set: true,
            ff_speed: speed,
            ..SlotState::default()
        };
        write_slot_state(d.path(), &s).unwrap();
        assert_eq!(read_slot_state(d.path()), s, "{written}x did not come back");
        let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
        let want = format!("ff_speed={written}");
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn an_older_build_reads_the_new_speed_as_its_own_default() {
    assert!(
        "1.5".parse::<u8>().is_err(),
        "1.5x parses as a whole number, so an older build would read it as a speed"
    );
    assert_eq!(SlotState::default().ff_speed, FF_SPEED_DEFAULT);
    assert!(
        FF_SPEEDS.contains(&FF_SPEED_DEFAULT),
        "the default is not one of the speeds the row offers"
    );
}

#[test]
fn a_saturation_or_contrast_the_rows_do_not_offer_reads_as_off_and_keeps_the_rest() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for bad in [
        "saturation=10\ncontrast=10\n",
        "saturation=-1\ncontrast=-1\n",
        "saturation=on\ncontrast=on\n",
        "saturation=\ncontrast=\n",
    ] {
        std::fs::write(d.path().join("Config/slot.state"), format!("{known}{bad}")).unwrap();
        let s = read_slot_state(d.path());
        assert_eq!((s.saturation, s.contrast), (0, 0), "accepted {bad:?}");
        assert_eq!(
            (s.cart.as_deref(), s.brightness, s.volume),
            (Some("Emerald"), 3, 40),
            "{bad:?} took the rest of the card with it"
        );
    }
}

#[test]
fn an_out_of_range_setting_falls_back_to_its_default() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for bad in [
        "rumble=2\nff_speed=5\nff_sound=9\n",
        "rumble=\nff_speed=1\nff_sound=on\n",
        "rumble=-1\nff_speed=7\nff_sound=-1\n",
        "ff_speed=x\n",
        "colour_correction=2\n",
        "colour_correction=\n",
        "colour_correction=Auto\n",
        "colour_correction_gba=2\n",
        "colour_correction_gb=on\n",
        "ff_speed=5\n",
        "ff_speed=7\n",
        "ff_speed=8\n",
        "ff_speed=255\n",
        "ff_speed=1.25\n",
        "ff_speed=1.75\n",
        "ff_speed=2.0\n",
        "ff_speed=24\n",
    ] {
        std::fs::write(d.path().join("Config/slot.state"), format!("{known}{bad}")).unwrap();
        let s = read_slot_state(d.path());
        assert_eq!(
            (
                s.rumble,
                s.ff_speed,
                s.ff_sound,
                s.colour_correction_gba,
                s.colour_correction_gb
            ),
            (true, FF_SPEED_DEFAULT, false, false, false),
            "accepted {bad:?}"
        );
        assert_eq!(
            (s.cart.as_deref(), s.brightness, s.volume, s.clock_set),
            (Some("Emerald"), 3, 40, true),
            "{bad:?} took the rest of the card with it"
        );
    }
}

#[test]
fn every_platform_round_trips_as_its_own_line() {
    for platform in Platform::ALL {
        let d = tmp_root();
        let s = SlotState {
            cart: Some("Tetris".into()),
            cart_platform: Some(platform),
            clock_set: true,
            ..SlotState::default()
        };
        write_slot_state(d.path(), &s).unwrap();
        assert_eq!(
            read_slot_state(d.path()),
            s,
            "{platform:?} did not come back"
        );
        let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
        let want = format!("cart_platform={}", platform.dir_name().to_lowercase());
        assert!(text.lines().any(|l| l == want), "no {want} in {text:?}");
    }
}

#[test]
fn an_empty_slot_writes_an_empty_platform() {
    let d = tmp_root();
    let s = SlotState {
        clock_set: true,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let text = std::fs::read_to_string(d.path().join("Config/slot.state")).unwrap();
    assert!(
        text.lines().any(|l| l == "cart_platform="),
        "no empty cart_platform line in {text:?}"
    );
    assert_eq!(read_slot_state(d.path()).cart_platform, None);
}

fn ds_build() -> Option<Platform> {
    #[cfg(feature = "nds")]
    {
        Some(Platform::Nds)
    }
    #[cfg(not(feature = "nds"))]
    {
        None
    }
}

#[test]
fn an_unreadable_platform_reads_as_a_card_that_never_said() {
    let d = tmp_root();
    let known = "cart=Tetris\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (line, want) in [
        ("cart_platform=\n", None),
        ("cart_platform=nes\n", None),
        ("cart_platform=gameboy\n", None),
        ("cart_platform=0\n", None),
        ("cart_platform=nds\n", ds_build()),
        ("cart_platform=GBA\n", Some(Platform::Gba)),
        ("cart_platform=Gbc\n", Some(Platform::Gbc)),
    ] {
        std::fs::write(d.path().join("Config/slot.state"), format!("{known}{line}")).unwrap();
        let s = read_slot_state(d.path());
        assert_eq!(s.cart_platform, want, "read {line:?} wrong");
        assert_eq!(
            (s.cart.as_deref(), s.brightness, s.volume, s.clock_set),
            (Some("Tetris"), 3, 40, true),
            "{line:?} took the rest of the card with it"
        );
    }
}

#[test]
fn an_offset_outside_the_range_of_real_zones_reads_as_default() {
    let d = tmp_root();
    for body in [
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=900\n",
        "cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min=-780\n",
    ] {
        std::fs::write(d.path().join("Config/slot.state"), body).unwrap();
        assert_eq!(read_slot_state(d.path()), SlotState::default(), "{body}");
    }
}

#[test]
fn a_missing_or_unknown_palette_falls_back_to_dmg_green() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for bad in [
        "",
        "gb_palette=\n",
        "gb_palette=Purple\n",
        "gb_palettes=2\ngb_palette=dmg green\n",
    ] {
        std::fs::write(d.path().join("Config/slot.state"), format!("{known}{bad}")).unwrap();
        let s = read_slot_state(d.path());
        assert_eq!(
            s.cart.as_deref(),
            Some("Emerald"),
            "{bad:?} lost the other settings"
        );
        assert!(!s.gb_palettes, "{bad:?} turned palettes on");
        assert_eq!(s.gb_palette, GbPalette::DEFAULT, "{bad:?}");
    }
}

#[test]
fn a_card_with_one_colour_correction_setting_starts_both_systems_from_it() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (old, want) in [
        ("colour_correction=1\n", true),
        ("colour_correction=0\n", false),
    ] {
        std::fs::write(d.path().join("Config/slot.state"), format!("{known}{old}")).unwrap();
        let s = read_slot_state(d.path());
        assert_eq!(
            (s.colour_correction_gba, s.colour_correction_gb),
            (want, want),
            "{old:?}"
        );
    }
    std::fs::write(
        d.path().join("Config/slot.state"),
        format!("{known}colour_correction=1\ncolour_correction_gb=0\n"),
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!(
        (s.colour_correction_gba, s.colour_correction_gb),
        (true, false),
        "the old line overrode a system's own setting"
    );
}

#[test]
fn a_wallpaper_tint_it_does_not_know_reads_as_off() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    for (line, want) in [
        ("wallpaper_tint=high", WallpaperTint::High),
        ("wallpaper_tint=blinding", WallpaperTint::Off),
    ] {
        std::fs::write(
            d.path().join("Config/slot.state"),
            format!("{known}wallpaper=a.png\n{line}\n"),
        )
        .unwrap();
        let s = read_slot_state(d.path());
        assert_eq!(s.wallpaper, WallpaperPick::File("a.png".into()), "{line}");
        assert_eq!(s.wallpaper_tint, want, "{line}");
    }
}

#[test]
fn a_card_from_before_the_gbc_split_starts_gbc_from_the_gb_c_settings() {
    let d = tmp_root();
    let known = "cart=Emerald\nbrightness=3\nblue_light=1\nvolume=40\nmuted=0\nclock_set=1\nutc_offset_min=0\n";
    std::fs::write(
        d.path().join("Config/slot.state"),
        format!("{known}colour_correction_gb=1\nshader_gb=grid\n"),
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!((s.colour_correction_gb, s.shader_gb), (true, Shader::Grid));
    assert_eq!(
        (s.colour_correction_gbc, s.shader_gbc),
        (true, Shader::Grid)
    );
    std::fs::write(
        d.path().join("Config/slot.state"),
        format!("{known}colour_correction=1\n"),
    )
    .unwrap();
    assert!(read_slot_state(d.path()).colour_correction_gbc);
    std::fs::write(
        d.path().join("Config/slot.state"),
        format!("{known}colour_correction_gb=1\nshader_gb=grid\ncolour_correction_gbc=0\nshader_gbc=off\n"),
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!(
        (s.colour_correction_gbc, s.shader_gbc),
        (false, Shader::Off),
        "the GB settings overrode GBC's own"
    );
}

#[test]
fn lid_power_off_minutes_round_trip_and_drop_a_value_the_menu_does_not_offer() {
    let d = tmp_root();
    for min in [1, 3, 5, 10, 15, 30] {
        let s = SlotState {
            lid_off_min: Some(min),
            ..Default::default()
        };
        write_slot_state(d.path(), &s).unwrap();
        assert_eq!(read_slot_state(d.path()).lid_off_min, Some(min));
    }
    let path = d.path().join("Config").join("slot.state");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("lid_off_min=30", "lid_off_min=7");
    std::fs::write(&path, text).unwrap();
    assert_eq!(read_slot_state(d.path()).lid_off_min, None);
}

#[test]
fn scale_and_stretch_round_trip_and_fall_back() {
    let d = tmp_root();
    let s = SlotState {
        scale_gba: Scale::FullScreen,
        scale_gb: Scale::Integer,
        scale_gbc: Scale::Fit,
        stretch_gb: Stretch::FullScreen,
        stretch_gbc: Stretch::Classic,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()), s);
    let path = d.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    for line in [
        "scale_gba=full-screen",
        "scale_gb=integer",
        "scale_gbc=fit",
        "stretch_gb=full-screen",
        "stretch_gbc=classic",
    ] {
        assert!(text.lines().any(|l| l == line), "{line} missing");
    }
    let broken = text
        .replace("scale_gb=integer", "scale_gb=huge")
        .replace("stretch_gb=full-screen", "stretch_gb=");
    std::fs::write(&path, broken).unwrap();
    let back = read_slot_state(d.path());
    assert_eq!(
        (back.scale_gb, back.stretch_gb, back.scale_gba),
        (Scale::SpFrame, Stretch::Classic, Scale::FullScreen)
    );
    assert_eq!(Scale::SpFrame.name(), "sp-frame");
    assert_eq!(Scale::parse("sp-frame"), Some(Scale::SpFrame));
    assert_eq!(Scale::parse("default"), None);
}

#[test]
fn a_frame_blending_the_rows_do_not_offer_reads_as_off() {
    let d = tmp_root();
    let s = SlotState {
        clock_set: true,
        blending_gba: Blending::Mix,
        blending_gb: Blending::Mix,
        blending_gbc: Blending::Ghosting,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let path = d.path().join("Config/slot.state");
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        text.replace("blending_gba=mix", "blending_gba=lcd_ghosting_fast")
            .replace("blending_gb=mix", "blending_gb=9"),
    )
    .unwrap();
    let read = read_slot_state(d.path());
    assert_eq!(read.blending_gba, Blending::Off);
    assert_eq!(read.blending_gb, Blending::Off);
    assert_eq!(
        read.blending_gbc,
        Blending::Ghosting,
        "a good line was thrown away with the bad ones"
    );
    assert!(
        read.clock_set,
        "the rest of the card was thrown away with it"
    );
}

#[test]
fn a_card_saved_with_the_test_builds_transparent_shader_keeps_it_as_authentic() {
    assert_eq!(
        slot_store::Shader::parse("transparent"),
        Some(slot_store::Shader::Authentic)
    );
    assert_eq!(
        slot_store::Shader::parse("authentic"),
        Some(slot_store::Shader::Authentic)
    );
}
