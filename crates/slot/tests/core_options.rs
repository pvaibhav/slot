mod common;

use slot::link_kind::{serial_option, LinkKind};
use slot_retro::LibretroCore;
use slot_store::{Core, Platform};

fn dylib_for(core: Core) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor")
        .join(slot::core::dylib_name(core))
}

fn every_serial_mode() -> Vec<&'static str> {
    let carts = [
        ("BPEE", "POKEMON EMER"),
        ("AWRE", "ADVANCEWARS"),
        ("AW2E", "ADVANCE WARS2"),
        ("AMFE", "METROID4"),
    ];
    let mut modes = Vec::new();
    for (code, title) in carts {
        for auto in [LinkKind::Cable, LinkKind::Wireless] {
            for chosen in [LinkKind::Cable, LinkKind::Wireless] {
                let mode = serial_option(chosen, auto, code, title);
                if !modes.contains(&mode) {
                    modes.push(mode);
                }
            }
        }
    }
    modes
}

fn every_option_is_one_the_core_has(core: &LibretroCore, what: &str) {
    let declared = core.declared_options();
    assert!(
        !declared.is_empty(),
        "{what}: the core declared no options at all, so this test would pass on anything"
    );
    let set = core.options();
    assert!(!set.is_empty(), "{what}: no options were set");
    for (key, value) in set {
        let Some(values) = declared.get(&key) else {
            let mut known: Vec<_> = declared.keys().cloned().collect();
            known.sort();
            panic!("{what}: the core declares no option {key:?}. It declares: {known:?}");
        };
        assert!(
            values.contains(&value),
            "{what}: the core declares {key:?} as {values:?}, and slot set it to {value:?}"
        );
    }
}

#[test]
fn every_option_slot_sets_is_one_the_core_declares() {
    let _g = common::core_lock();
    let mut ran = 0;
    for which in Core::ALL {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        for serial in every_serial_mode() {
            for bios in [false, true] {
                for (colour, platform) in [false, true]
                    .into_iter()
                    .flat_map(|c| Platform::ALL.map(|p| (c, p)))
                    .filter(|(_, p)| which.runs(*p))
                {
                    for palette in
                        std::iter::once(None).chain(slot_store::GbColors::all().map(Some))
                    {
                        let mut core = LibretroCore::open(&path).expect("open core");
                        slot::core::apply_core_options(
                            &mut core, which, platform, serial, bios, colour, palette,
                        );
                        every_option_is_one_the_core_has(
                            &core,
                            &format!(
                                "{} {platform:?} serial={serial} bios={bios} colour={colour} \
                                 palette={palette:?}",
                                which.as_str()
                            ),
                        );
                        ran += 1;
                    }
                }
            }
        }
    }
    if ran == 0 {
        eprintln!("no cores on this host, nothing was checked");
    }
}

#[test]
fn both_cores_declare_a_colour_correction_option() {
    let _g = common::core_lock();
    for (which, key) in [
        (Core::Mgba, "mgba_color_correction"),
        (Core::Gpsp, "gpsp_color_correction"),
    ] {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let core = LibretroCore::open(&path).expect("open core");
        assert!(
            core.declared_options().contains_key(key),
            "{} declares no {key}",
            which.as_str()
        );
    }
}

#[test]
fn a_cable_session_leaves_the_bios_to_the_card() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_link_options(&mut core, Core::Mgba, 1);
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_use_bios").map(String::as_str),
        None,
        "link mode overrode the card's BIOS"
    );
    assert_eq!(set.get("mgba_link").map(String::as_str), Some("on"));
    assert_eq!(
        set.get("mgba_link_player").map(String::as_str),
        Some("1"),
        "the port this device drives did not reach the core"
    );
    every_option_is_one_the_core_has(&core, "mgba cable session");
}

#[test]
fn a_cable_session_sets_nothing_on_gpsp() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpsp dylib on this host, skipping");
        return;
    }
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_link_options(&mut core, Core::Gpsp, 0);
    assert!(
        core.options().is_empty(),
        "a cable session reached into gpSP, which has no cable to offer"
    );
}

#[test]
fn a_named_palette_forces_a_plain_game_boy_without_hardware_presets() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let p = slot_store::GbPalette::parse("SGB 1-A").unwrap();
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gb,
        "auto",
        false,
        false,
        Some(slot_store::GbColors::Palette(p)),
    );
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_gb_model").map(String::as_str),
        Some("Game Boy")
    );
    assert_eq!(
        set.get("mgba_gb_colors_preset").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        set.get("mgba_gb_colors").map(String::as_str),
        Some("SGB 1-A")
    );
    drop(core);

    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gb,
        "auto",
        false,
        false,
        None,
    );
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_gb_model").map(String::as_str),
        Some("Autodetect")
    );
    assert_eq!(
        set.get("mgba_gb_colors_preset").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        set.get("mgba_gb_colors").map(String::as_str),
        Some("GBC Dark Green →A"),
        "palettes off changed what a Game Boy cart gets today"
    );
}

#[test]
fn only_a_cart_without_the_colour_flag_is_game_boy_only() {
    let d = tempfile::tempdir().unwrap();
    let rom = |name: &str, cgb: u8| {
        let mut bytes = vec![0u8; 0x8000];
        bytes[0x143] = cgb;
        let p = d.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        p
    };
    assert!(slot::core::dmg_only(&rom("dmg.gb", 0x00)));
    assert!(!slot::core::dmg_only(&rom("dual.gbc", 0x80)));
    assert!(!slot::core::dmg_only(&rom("cgb.gbc", 0xC0)));
    assert!(!slot::core::dmg_only(&d.path().join("missing.gb")));
}

#[test]
fn a_linked_game_never_takes_a_named_palette() {
    let d = tempfile::tempdir().unwrap();
    let mut bytes = vec![0u8; 0x8000];
    bytes[0x146] = 0x03;
    let rom = d.path().join("sgb.gb");
    std::fs::write(&rom, bytes).unwrap();
    let p = slot_store::GbColors::Palette(slot_store::GbPalette::DEFAULT);
    let gb = slot_store::Platform::Gb;
    assert_eq!(slot::core::palette_for(p, gb, &rom, None), Some(p));
    for player in [0, 1] {
        assert_eq!(
            slot::core::palette_for(p, gb, &rom, Some(player)),
            None,
            "player {player}: a linked pair would run two models of one cart"
        );
    }
    assert_eq!(
        slot::core::palette_for(slot_store::GbColors::Auto, gb, &rom, None),
        Some(slot_store::GbColors::Auto)
    );
    assert_eq!(
        slot::core::palette_for(p, slot_store::Platform::Gba, &rom, None),
        None
    );
}

#[test]
fn a_colour_game_keeps_autodetect() {
    let d = tempfile::tempdir().unwrap();
    let mut bytes = vec![0u8; 0x8000];
    bytes[0x143] = 0x80;
    let rom = d.path().join("dual.gbc");
    std::fs::write(&rom, bytes).unwrap();
    for platform in [slot_store::Platform::Gb, slot_store::Platform::Gbc] {
        assert_eq!(
            slot::core::palette_for(slot_store::GbColors::Auto, platform, &rom, None),
            None,
            "{platform:?}"
        );
    }
}

#[test]
fn an_original_game_boy_game_runs_on_the_plain_model_with_auto() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let mut core = LibretroCore::open(&path).expect("open core");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gb,
        "auto",
        false,
        false,
        Some(slot_store::GbColors::Auto),
    );
    let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
    assert_eq!(
        set.get("mgba_gb_model").map(String::as_str),
        Some("Game Boy")
    );
    assert_eq!(
        set.get("mgba_gb_colors_preset").map(String::as_str),
        Some("0")
    );
    assert_eq!(set.get("mgba_gb_colors").map(String::as_str), Some("Auto"));
}

#[test]
fn mgba_answers_the_game_boy_player_logo_except_for_gba_video() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    for (platform, code, want, why) in [
        (
            Platform::Gba,
            "BPPE",
            "ON",
            "Pokemon Pinball Ruby and Sapphire only rumbles on a Game Boy Player",
        ),
        (
            Platform::Gba,
            "MSSE",
            "OFF",
            "a GBA Video cart refuses to play on a Game Boy Player",
        ),
        (
            Platform::Gbav,
            "BPEE",
            "OFF",
            "the GBA Video shelf holds only videos",
        ),
    ] {
        let mut bytes = vec![0u8; 0x100];
        bytes[0xac..0xb0].copy_from_slice(code.as_bytes());
        let rom = d.path().join(format!("{code}.gba"));
        std::fs::write(&rom, bytes).unwrap();
        let mut core = LibretroCore::open(&path).expect("open core");
        slot::core::apply_core_options(&mut core, Core::Mgba, platform, "auto", false, false, None);
        slot::core::apply_game_boy_player(&mut core, Core::Mgba, platform, &rom);
        let set: std::collections::HashMap<String, String> = core.options().into_iter().collect();
        assert_eq!(
            set.get("mgba_force_gbp").map(String::as_str),
            Some(want),
            "{platform:?} {code}: {why}"
        );
    }
}

#[test]
fn the_blending_off_slot_sets_is_one_the_core_declares() {
    let _g = common::core_lock();
    for which in Core::ALL {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let platform = test_platform(which);
        let mut core = LibretroCore::open(&path).expect("open core");
        slot::core::apply_core_options(&mut core, which, platform, "auto", false, false, None);
        slot::core::apply_blending_off(&mut core, which);
        every_option_is_one_the_core_has(&core, &format!("{} blending off", which.as_str()));
        let (key, value) = slot::core::blending_off(which);
        assert!(
            core.options().iter().any(|(k, v)| k == key && v == value),
            "{} was never told {key}={value}",
            which.as_str()
        );
    }
}

#[test]
fn both_cores_declare_a_frame_blending_option() {
    let _g = common::core_lock();
    for (which, key) in [
        (Core::Mgba, "mgba_interframe_blending"),
        (Core::Gpsp, "gpsp_frame_mixing"),
    ] {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let core = LibretroCore::open(&path).expect("open core");
        assert!(
            core.declared_options().contains_key(key),
            "{} declares no {key}",
            which.as_str()
        );
    }
}

fn test_platform(which: Core) -> Platform {
    if which.runs(Platform::Gba) {
        Platform::Gba
    } else {
        Platform::Gb
    }
}

#[test]
fn the_sunlight_follows_the_clock_dark_at_night_and_brightest_at_noon() {
    let at = |h: i64, m: i64| slot::core::sunlight_at(h * 3600 + m * 60);
    assert_eq!(at(0, 0), 0);
    assert_eq!(at(5, 59), 0);
    assert_eq!(at(6, 0), 0);
    assert_eq!(at(9, 0), 4);
    assert_eq!(at(12, 0), 8);
    assert_eq!(at(15, 0), 4);
    assert_eq!(at(18, 0), 0);
    assert_eq!(at(23, 59), 0);
    assert_eq!(slot::core::sunlight_at(3 * 86_400 + 12 * 3600), 8);
    assert_eq!(slot::core::sunlight_at(-12 * 3600), 8);
}

#[test]
fn mgba_takes_every_sunlight_level_slot_can_send() {
    let _g = common::core_lock();
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    }
    let core = LibretroCore::open(&path).expect("open core");
    let levels = core
        .declared_options()
        .get(slot::core::SUNLIGHT_OPTION)
        .cloned()
        .expect("mgba declares no solar sensor option");
    for level in 0..=slot::core::SUNLIGHT_MAX {
        assert!(
            levels.contains(&level.to_string()),
            "mgba has no sunlight level {level} in {levels:?}"
        );
    }
}
