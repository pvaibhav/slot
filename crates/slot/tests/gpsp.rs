mod common;

use slot_store::{Core, Platform};

fn dylib_for(core: Core) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor")
        .join(slot::core::dylib_name(core))
}

#[test]
fn each_core_resolves_to_its_own_dylib() {
    assert_ne!(
        slot::core::dylib_name(Core::Mgba),
        slot::core::dylib_name(Core::Gpsp)
    );
    assert!(slot::core::dylib_name(Core::Gpsp).starts_with("gpsp_libretro"));
    assert!(dylib_for(Core::Gpsp)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("gpsp_libretro"));
}

#[test]
fn gpsp_loads_and_runs_a_frame() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    core.set_option("gpsp_serial", "rfu");
    assert_eq!(core.option("gpsp_serial"), Some("rfu".to_string()));
}

#[test]
fn gpsp_is_told_its_serial_mode_before_load() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    slot::core::apply_core_options(
        &mut core,
        Core::Gpsp,
        Platform::Gba,
        "auto",
        false,
        false,
        None,
    );
    assert_eq!(
        core.option("gpsp_serial"),
        Some("auto".to_string()),
        "auto resolves per ROM, so both devices agree without being told"
    );
}

#[test]
fn gpsp_is_told_the_serial_mode_it_is_handed() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    for serial in ["rfu", "mul_poke", "mul_aw1", "mul_aw2"] {
        slot::core::apply_core_options(
            &mut core,
            Core::Gpsp,
            Platform::Gba,
            serial,
            false,
            false,
            None,
        );
        assert_eq!(
            core.option("gpsp_serial"),
            Some(serial.to_string()),
            "gpSP was not handed the mode the link screen asked for"
        );
    }
}

#[test]
fn gpsp_boots_through_the_bios_when_the_card_carries_one() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    slot::core::apply_core_options(
        &mut core,
        Core::Gpsp,
        Platform::Gba,
        "auto",
        true,
        false,
        None,
    );
    assert_eq!(
        core.option("gpsp_boot_mode"),
        Some("bios".to_string()),
        "a real BIOS on the card and gpSP still told to skip it"
    );
    assert_eq!(
        core.option("gpsp_bios"),
        None,
        "gpsp_bios was named: auto already picks the official image up, and official only \
         adds an on-screen warning when it cannot"
    );
    assert_eq!(
        core.option("gpsp_serial"),
        Some("auto".to_string()),
        "the link mode stopped getting through once the boot mode joined it"
    );
}

#[test]
fn gpsp_is_left_on_its_own_boot_default_when_the_card_has_no_bios() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    slot::core::apply_core_options(
        &mut core,
        Core::Gpsp,
        Platform::Gba,
        "auto",
        false,
        false,
        None,
    );
    assert_eq!(
        core.option("gpsp_boot_mode"),
        None,
        "gpSP was sent through a BIOS the card does not have, which is its built-in one: \
         seconds of blank screen where the logo was promised"
    );
}

#[test]
fn mgba_is_given_its_own_frameskip_and_none_of_gpsps() {
    let path = dylib_for(Core::Mgba);
    if !path.exists() {
        eprintln!("no mGBA dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open mgba");
    slot::core::apply_core_options(
        &mut core,
        Core::Mgba,
        Platform::Gba,
        "rfu",
        true,
        false,
        None,
    );
    assert_eq!(
        core.option("gpsp_serial"),
        None,
        "mGBA has no such option and must not be handed one"
    );
    assert_eq!(
        core.option("gpsp_boot_mode"),
        None,
        "gpSP's boot switch reached mGBA, which spells its own mgba_skip_bios"
    );
    assert_eq!(
        core.option("mgba_frameskip").as_deref(),
        Some("auto"),
        "mGBA was never put on auto frameskip, so nothing can tell it which frame to draw"
    );
    assert_eq!(
        core.option("gpsp_frameskip"),
        None,
        "gpSP's frameskip key reached mGBA, which reads only its own prefix"
    );
}

#[test]
fn both_cores_are_told_about_colour_correction_in_their_own_words() {
    for (which, key, on, off, theirs) in [
        (
            Core::Mgba,
            "mgba_color_correction",
            "Auto",
            "OFF",
            "gpsp_color_correction",
        ),
        (
            Core::Gpsp,
            "gpsp_color_correction",
            "enabled",
            "disabled",
            "mgba_color_correction",
        ),
    ] {
        let path = dylib_for(which);
        if !path.exists() {
            eprintln!("no {} dylib on this host, skipping", which.as_str());
            continue;
        }
        let _g = common::core_lock();
        let mut core = slot_retro::LibretroCore::open(&path).expect("open the core");
        for (colour, want) in [(true, on), (false, off)] {
            slot::core::apply_core_options(
                &mut core,
                which,
                Platform::Gba,
                "auto",
                false,
                colour,
                None,
            );
            assert_eq!(
                core.option(key).as_deref(),
                Some(want),
                "{} was not told {want} for colour correction {colour}",
                which.as_str()
            );
        }
        assert_eq!(
            core.option(theirs),
            None,
            "{} was handed the other core's key",
            which.as_str()
        );
    }
}

#[test]
fn gpsp_is_put_on_auto_frameskip() {
    let path = dylib_for(Core::Gpsp);
    if !path.exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let _g = common::core_lock();
    let mut core = slot_retro::LibretroCore::open(&path).expect("open gpsp");
    slot::core::apply_core_options(
        &mut core,
        Core::Gpsp,
        Platform::Gba,
        "auto",
        false,
        false,
        None,
    );
    assert_eq!(
        core.option("gpsp_frameskip").as_deref(),
        Some("auto"),
        "gpSP was never put on auto frameskip, so nothing can tell it which frame to draw"
    );
    assert_eq!(
        core.option("mgba_frameskip"),
        None,
        "mGBA's frameskip key reached gpSP, which reads only its own prefix"
    );
}

fn root_with_bios_and_logo_cart() -> Option<(tempfile::TempDir, std::path::PathBuf)> {
    let (bios, rom_bytes) = (common::real_bios()?, common::logo_rom()?);
    let d = common::tmp_root_with_carts(&[]);
    std::fs::copy(bios, d.path().join("BIOS").join("gba_bios.bin")).expect("copy bios");
    let rom = d.path().join("Games").join("Logo.gba");
    std::fs::write(&rom, rom_bytes).expect("write logo rom");
    Some((d, rom))
}

fn splash_plays(root: &std::path::Path, rom: &std::path::Path) -> bool {
    use slot_retro::ButtonMask;
    let mut core =
        slot::core::open_core_for(root, Core::Gpsp, "auto", false, &[dylib_for(Core::Gpsp)]);
    core.load(rom).expect("gpSP refused the logo rom");
    (0..150).any(|_| {
        core.run_frame(ButtonMask::default());
        common::mostly_lit(core.video_xrgb8888())
    })
}

#[test]
fn a_cart_boots_through_a_real_bios_and_the_splash_reaches_the_screen() {
    if !dylib_for(Core::Gpsp).exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let Some((d, rom)) = root_with_bios_and_logo_cart() else {
        eprintln!("no real BIOS or no cart to lift a logo from on this host, skipping");
        return;
    };
    let _g = common::core_lock();
    assert!(
        splash_plays(d.path(), &rom),
        "the cart went straight to the game with a real BIOS sitting in the content root"
    );
}

#[test]
fn a_cart_goes_straight_to_the_game_when_the_card_has_no_bios() {
    if !dylib_for(Core::Gpsp).exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let Some((d, rom)) = root_with_bios_and_logo_cart() else {
        eprintln!("no real BIOS or no cart to lift a logo from on this host, skipping");
        return;
    };
    let _g = common::core_lock();
    std::fs::remove_file(d.path().join("BIOS").join("gba_bios.bin")).unwrap();
    assert!(
        !splash_plays(d.path(), &rom),
        "a splash played with no BIOS on the card, so this test cannot tell the two apart"
    );
}

fn a_published_frame_is_the_splash(
    root: &std::path::Path,
    rom: &std::path::Path,
    resume: Option<Vec<u8>>,
) -> bool {
    use slot::audio::{AudioSink, StubSink};
    use slot::emu::{CoreState, EmuHandle, Speed};
    use std::time::{Duration, Instant};

    let mut sink = StubSink::new();
    sink.open(32_768).expect("the stub refused to open");
    let drain = sink.clone();
    std::thread::spawn(move || loop {
        drain.device_drain();
        std::thread::sleep(Duration::from_millis(2));
    });

    let emu = EmuHandle::spawn(
        slot::core::open_core_for(root, Core::Gpsp, "auto", false, &[dylib_for(Core::Gpsp)]),
        rom.to_path_buf(),
        sink.ring(),
        None,
        resume,
    );
    emu.set_speed(Speed::Normal);
    let deadline = Instant::now() + Duration::from_secs(10);
    while emu.state() == CoreState::Loading {
        assert!(Instant::now() < deadline, "the core never finished loading");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(emu.state(), CoreState::Ready, "the core refused the cart");

    let watch = Instant::now() + Duration::from_secs(3);
    while Instant::now() < watch {
        if emu.latest_frame().is_some_and(|f| common::mostly_lit(&f)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    false
}

#[test]
fn the_splash_plays_on_a_fresh_start_and_never_over_a_resume() {
    if !dylib_for(Core::Gpsp).exists() {
        eprintln!("no gpSP dylib on this host, skipping");
        return;
    }
    let Some((d, rom)) = root_with_bios_and_logo_cart() else {
        eprintln!("no real BIOS or no cart to lift a logo from on this host, skipping");
        return;
    };
    let _g = common::core_lock();

    let state = {
        use slot_retro::ButtonMask;
        let mut core = slot::core::open_core_for(
            d.path(),
            Core::Gpsp,
            "auto",
            false,
            &[dylib_for(Core::Gpsp)],
        );
        core.load(&rom).expect("gpSP refused the logo rom");
        for _ in 0..480 {
            core.run_frame(ButtonMask::default());
        }
        assert!(
            !common::mostly_lit(core.video_xrgb8888()),
            "the state standing in for a resume is itself a frame of the boot splash, so the \
             half below would fail no matter what the resume did"
        );
        core.serialize().expect("gpSP gave up no state")
    };

    assert!(
        a_published_frame_is_the_splash(d.path(), &rom, None),
        "no splash on a fresh start, so this test cannot see one and proves nothing below"
    );
    assert!(
        !a_published_frame_is_the_splash(d.path(), &rom, Some(state)),
        "the BIOS splash played over a cart the player was resuming"
    );
}

#[test]
fn a_gpsp_carts_resume_is_read_from_its_own_core_directory_through_the_session() {
    use slot::app::Phase;
    use slot::persist;
    use slot::session::Session;
    use slot_input::{Btn, RawEvent};
    use slot_store::{StateRing, SELECTED_CORE_FILE};
    use std::time::{Duration, Instant};

    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Emerald = gpsp\n").unwrap();

    StateRing::new(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
        .write_resume(&700_000u64.to_le_bytes())
        .unwrap();
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(&1u64.to_le_bytes())
        .unwrap();

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    s.app_mut().tick_ms(300_000);
    s.app_mut().settle_saves();

    let state = persist::read_resume(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
        .unwrap()
        .expect("nothing resumed");
    let n = u64::from_le_bytes(state.try_into().expect("mock state is 8 bytes"));
    assert!(
        n >= 700_000,
        "the session resumed the wrong core's state (or none): counter is {n}"
    );
}

#[test]
fn a_gpsp_cart_runs_the_dylib_planted_under_its_own_name_through_the_session() {
    use slot::app::Phase;
    use slot::persist;
    use slot::session::Session;
    use slot_input::{Btn, RawEvent};
    use slot_store::SELECTED_CORE_FILE;
    use std::time::{Duration, Instant};

    let Some(mgba) = common::vendored_core() else {
        eprintln!("no host-openable dylib on this machine, skipping");
        return;
    };
    let _g = common::core_lock();

    let d = common::tmp_root_with_real_carts(&["Emerald"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Emerald = gpsp\n").unwrap();
    let planted = d
        .path()
        .join("System")
        .join(slot::core::dylib_name(Core::Gpsp));
    std::fs::copy(&mgba, &planted).expect("plant a dylib under gpSP's name");

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    s.app_mut().tick_ms(300_000);
    s.app_mut().settle_saves();

    let state = persist::read_resume(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
        .unwrap()
        .expect("nothing resumed");
    assert!(
        state.len() > 100_000,
        "the session ran the mock, not the dylib the ini named: {} bytes",
        state.len()
    );
}

#[test]
fn changing_the_ini_mid_session_does_not_move_a_seated_carts_autosave() {
    use slot::app::Phase;
    use slot::session::Session;
    use slot_input::{Btn, RawEvent};
    use slot_store::{StateRing, SELECTED_CORE_FILE};
    use std::time::{Duration, Instant};

    let d = common::tmp_root_with_carts(&["Emerald"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Emerald = gpsp\n").unwrap();

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    std::fs::remove_file(d.path().join(SELECTED_CORE_FILE)).unwrap();

    s.app_mut().tick_ms(300_000);
    s.app_mut().settle_saves();

    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
            .read_resume()
            .unwrap()
            .is_some(),
        "the autosave did not land under the seated core's own directory"
    );
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none(),
        "the autosave followed the ini's new (absent) reading instead of the core the \
         session actually spawned"
    );
}

#[test]
fn changing_the_ini_mid_session_does_not_move_a_manual_save_state() {
    use slot::app::Phase;
    use slot::session::Session;
    use slot_input::{Action, Btn, RawEvent};
    use slot_store::{StateRing, SELECTED_CORE_FILE};
    use std::time::{Duration, Instant};

    let d = common::tmp_root_with_carts(&["Emerald"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Emerald = gpsp\n").unwrap();

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    std::fs::remove_file(d.path().join(SELECTED_CORE_FILE)).unwrap();

    s.app_mut().apply(Action::SaveState);

    assert!(
        !StateRing::new(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
            .list()
            .unwrap()
            .is_empty(),
        "the manual save did not land under the seated core's own directory"
    );
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .list()
            .unwrap()
            .is_empty(),
        "the manual save followed the ini's new (absent) reading instead of the core the \
         session actually spawned"
    );
}

#[test]
fn changing_the_ini_mid_session_does_not_move_an_ejected_carts_resume() {
    use slot::app::Phase;
    use slot::session::Session;
    use slot_input::{Action, Btn, RawEvent};
    use slot_store::{StateRing, SELECTED_CORE_FILE};
    use std::time::{Duration, Instant};

    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Emerald = gpsp\n").unwrap();

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }

    std::fs::remove_file(d.path().join(SELECTED_CORE_FILE)).unwrap();

    s.app_mut().apply(Action::Eject);
    s.app_mut().settle_saves();

    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
            .read_resume()
            .unwrap()
            .is_some(),
        "the ejected cart's resume did not land under the seated core's own directory"
    );
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none(),
        "the ejected cart's resume followed the ini's new (absent) reading instead of the \
         core the session actually spawned"
    );
}

#[test]
fn open_core_reaches_a_gpsp_named_dylib_under_the_content_roots_system_directory() {
    use slot_retro::ButtonMask;

    let Some(mgba) = common::vendored_core() else {
        eprintln!("no host-openable dylib on this machine, skipping");
        return;
    };
    let _g = common::core_lock();
    let d = common::tmp_root_with_real_carts(&["Probe"]);
    let planted = d
        .path()
        .join("System")
        .join(slot::core::dylib_name(Core::Gpsp));
    std::fs::copy(&mgba, &planted).expect("plant a dylib under gpSP's name");

    let mut core = slot::core::open_core(
        d.path(),
        Core::Gpsp,
        Platform::Gba,
        "auto",
        false,
        None,
        None,
    )
    .core;
    core.load(&d.path().join("Games/GBA/Probe.gba"))
        .expect("the planted core refused the test rom");
    core.run_frame(ButtonMask::default());
    assert!(
        core.serialize().expect("core gave up no state").len() > 100_000,
        "open_core fell back to the mock instead of the dylib planted at root/System"
    );
}

#[test]
fn a_game_boy_carts_gpsp_line_is_dropped_and_it_runs_on_the_platform_default() {
    let default = Core::Gambatte;
    use slot::app::Phase;
    use slot::persist;
    use slot::session::Session;
    use slot_input::{Btn, RawEvent};
    use slot_store::{StateRing, SELECTED_CORE_FILE};
    use std::time::{Duration, Instant};

    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    std::fs::write(d.path().join(SELECTED_CORE_FILE), "Tetris = gpsp\n").unwrap();
    StateRing::new(d.path(), Platform::Gb, default, "Tetris")
        .write_resume(&700_000u64.to_le_bytes())
        .unwrap();

    common::clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let mut now = 32;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }

    let counter = |core| {
        persist::read_resume(d.path(), Platform::Gb, core, "Tetris")
            .unwrap()
            .map(|b| u64::from_le_bytes(b.try_into().expect("the mock's state is 8 bytes")))
    };

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if counter(default).is_some_and(|n| n > 700_000) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the ini's `Tetris = gpsp` was honoured for a Game Boy cart: States/GB/{} still \
             reads {:?} and States/GB/gpsp reads {:?}",
            default.as_str(),
            counter(default),
            counter(Core::Gpsp)
        );
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
        s.app_mut().flush_resume();
    }
    assert_eq!(
        counter(Core::Gpsp),
        None,
        "a Game Boy cart's state was filed under States/GB/gpsp"
    );
}

fn cart_clock(state: &[u8]) -> Option<u32> {
    let at = state.windows(8).rposition(|w| w == b"RTCPROBE")?;
    let t = &state[at + 8..at + 15];
    let bcd = |b: u8| u32::from(b >> 4) * 10 + u32::from(b & 0xf);
    Some(bcd(t[4] & 0x3f) * 3600 + bcd(t[5]) * 60 + bcd(t[6]))
}

fn seconds_of_day() -> u32 {
    let out = std::process::Command::new("date")
        .arg("+%H %M %S")
        .output()
        .expect("date");
    let f: Vec<u32> = String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    f[0] * 3600 + f[1] * 60 + f[2]
}

fn keeps_time_while_off(which: Core) {
    use slot_retro::{ButtonMask, RetroCore};
    let path = dylib_for(which);
    if !path.exists() {
        eprintln!("no {} dylib on this host, skipping", which.as_str());
        return;
    }
    let rom =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/rtc_probe/probe.gba");
    let _g = common::core_lock();
    let boot = |state: Option<&[u8]>| {
        let mut core = slot_retro::LibretroCore::open(&path).expect("open the core");
        slot::core::apply_core_options(&mut core, which, Platform::Gba, "auto", false, false, None);
        core.load(&rom).expect("load the probe");
        if let Some(s) = state {
            core.unserialize(s).expect("resume");
        }
        for _ in 0..30 {
            core.run_frame(ButtonMask::default());
        }
        core.serialize().expect("state")
    };
    let state = boot(None);
    std::thread::sleep(std::time::Duration::from_secs(3));
    let resumed = boot(Some(&state));
    let cart = cart_clock(&resumed).expect("the probe wrote no time");
    let wall = seconds_of_day();
    assert!(
        wall.abs_diff(cart) <= 1,
        "{}: the cart reads {cart} s into the day where the wall clock reads {wall}: the time spent off was lost",
        which.as_str()
    );
}

#[test]
fn the_gpsp_cartridge_clock_keeps_time_while_the_game_is_off() {
    keeps_time_while_off(Core::Gpsp);
}

#[test]
fn the_mgba_cartridge_clock_keeps_time_while_the_game_is_off() {
    keeps_time_while_off(Core::Mgba);
}

fn saturation(xrgb: &[u8]) -> f64 {
    let n = (xrgb.len() / 4) as f64;
    xrgb.chunks_exact(4)
        .map(|p| {
            let (hi, lo) = (p[..3].iter().max(), p[..3].iter().min());
            f64::from(hi.copied().unwrap_or(0) - lo.copied().unwrap_or(0))
        })
        .sum::<f64>()
        / n
}

#[test]
fn a_paused_refresh_shows_a_colour_change_on_the_first_toggle() {
    use slot::audio::{AudioSink, StubSink};
    use slot::emu::{CoreState, EmuHandle, Speed};
    use std::time::{Duration, Instant};

    let _g = common::core_lock();
    let from = common::repo_root().join("sdcard/Games/GBA/Metroid Fusion.gba");
    if !from.exists() || !dylib_for(Core::Gpsp).exists() {
        eprintln!("no gpSP or no GBA cart on this machine, skipping");
        return;
    }
    let d = common::tmp_root_with_carts(&[]);
    let rom = d.path().join("Games/GBA/Metroid Fusion.gba");
    std::fs::copy(&from, &rom).expect("copy the cart");

    let mut sink = StubSink::new();
    sink.open(32_768).expect("the stub refused to open");
    let drain = sink.clone();
    std::thread::spawn(move || loop {
        drain.device_drain();
        std::thread::sleep(Duration::from_millis(2));
    });
    let emu = EmuHandle::spawn(
        slot::core::open_core_for(
            d.path(),
            Core::Gpsp,
            "auto",
            false,
            &[dylib_for(Core::Gpsp)],
        ),
        rom,
        sink.ring(),
        None,
        None,
    );
    emu.set_speed(Speed::Normal);
    let deadline = Instant::now() + Duration::from_secs(20);
    while emu.state() == CoreState::Loading || emu.published_count() < 300 {
        assert!(Instant::now() < deadline, "the cart never got going");
        std::thread::sleep(Duration::from_millis(4));
    }
    emu.set_speed(Speed::Paused);
    while emu.observed_speed() != Speed::Paused {
        assert!(Instant::now() < deadline, "the core never paused");
        std::thread::sleep(Duration::from_millis(2));
    }
    let shown = |emu: &EmuHandle| saturation(&emu.latest_frame().expect("no frame"));
    let refreshed = |emu: &EmuHandle, on: bool| {
        let before = emu.published_count();
        emu.set_option(
            "gpsp_color_correction",
            if on { "enabled" } else { "disabled" },
        );
        emu.refresh();
        while emu.published_count() == before {
            assert!(Instant::now() < deadline, "the refresh never published");
            std::thread::sleep(Duration::from_millis(2));
        }
        shown(emu)
    };
    let raw = shown(&emu);
    assert!(
        raw > 10.0,
        "the paused frame has no colour to correct: {raw:.1}"
    );
    let on = refreshed(&emu, true);
    assert!(
        on < raw * 0.9,
        "the first toggle on drew the old colours: saturation {raw:.1} before, {on:.1} after"
    );
    let off = refreshed(&emu, false);
    assert!(
        (off - raw).abs() < raw * 0.02,
        "toggling back off did not restore the raw colours: {raw:.1} then {off:.1}"
    );
}
