mod common;

use std::path::{Path, PathBuf};

use slot_retro::{ButtonMask, LibretroCore, RetroCore};

const SRAM: usize = 0x8000;
const START_CLOCK: usize = 0x2044;
const SAVED_RTC: usize = 0x2048;
const RTC_STATUS: usize = 0x0c60;
const BEFORE_CLEAR_CLOCK: usize = 10;

fn vendor(name: &str) -> PathBuf {
    common::repo_root().join("vendor").join(name)
}

fn run_core(dylib: &Path, dir: &Path, rom: &Path, sav: &[u8], frames: usize) -> Vec<u8> {
    let mut core = LibretroCore::open_with(dylib, dir, dir).expect("open core");
    core.load(rom).expect("core refused the cart");
    core.load_save_ram(sav).expect("core refused the save");
    let loaded = core.save_ram().expect("core gave up no save ram");
    eprintln!("  as loaded, tail: {}", hex(&loaded[SRAM..]));
    for _ in 0..frames {
        core.run_frame(ButtonMask::default());
    }
    core.save_ram().expect("core gave up no save ram")
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn show(label: &str, sav: &[u8]) {
    eprintln!(
        "{label}: {} bytes, rtc tail {} bytes",
        sav.len(),
        sav.len() - SRAM
    );
    eprintln!("  tail: {}", hex(&sav[SRAM..]));
    eprintln!(
        "  game start day/hour/min/sec: {:?}",
        &sav[START_CLOCK..START_CLOCK + 4]
    );
    eprintln!("  game wRTC: {}", hex(&sav[SAVED_RTC..SAVED_RTC + 8]));
    eprintln!("  sRTCStatusFlags: {:02x}", sav[RTC_STATUS]);
}

#[test]
fn mgba_crystal_save_loads_in_gambatte() {
    let _g = common::core_lock();
    let mgba = vendor("mgba_libretro.dylib");
    let gambatte = vendor("gambatte_libretro.dylib");
    let card_rom = common::repo_root().join("sdcard/Games/GBC/Pokemon - Crystal Version (USA).gbc");
    let card_sav = common::repo_root().join("sdcard/Saves/GBC/Pokemon - Crystal Version (USA).sav");
    if !mgba.exists() || !gambatte.exists() || !card_rom.exists() || !card_sav.exists() {
        eprintln!("no host cores or no Pokemon Crystal on the card, skipping");
        return;
    }
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = dir.path().join("Pokemon - Crystal Version (USA).gbc");
    std::fs::copy(&card_rom, &rom).expect("copy rom");
    let sav_path = dir.path().join("Pokemon - Crystal Version (USA).sav");
    std::fs::copy(&card_sav, &sav_path).expect("copy save");
    let card = std::fs::read(&sav_path).expect("read save");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    eprintln!("host unix time {now}");
    show("card", &card);

    let from_mgba = run_core(&mgba, dir.path(), &rom, &card, 600);
    show("mgba", &from_mgba);

    let started = std::time::Instant::now();
    let from_gambatte = run_core(&gambatte, dir.path(), &rom, &from_mgba, 600);
    eprintln!("gambatte ran 600 frames in {:?}", started.elapsed());
    show("gambatte", &from_gambatte);

    let back = run_core(&mgba, dir.path(), &rom, &from_gambatte, 600);
    show("mgba again", &back);

    assert_eq!(
        from_mgba[..SRAM],
        card[..SRAM],
        "mGBA changed the save ram on a title screen run"
    );
    assert_eq!(
        from_gambatte[..SRAM],
        from_mgba[..SRAM],
        "Gambatte did not keep mGBA's save ram"
    );
    assert_eq!(
        back[..SRAM],
        from_gambatte[..SRAM],
        "mGBA did not keep Gambatte's save ram"
    );
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn mgba_clock_at(tail: &[u8], at: u64) -> i64 {
    let reg = |i: usize| u64::from(tail[20 + i * 4]);
    let days = reg(3) + ((reg(4) & 1) << 8);
    let latched = reg(0) + 60 * reg(1) + 3_600 * reg(2) + 86_400 * days;
    let unix = u64::from_le_bytes(tail[40..48].try_into().unwrap());
    latched as i64 + at as i64 - unix as i64
}

#[test]
fn crystal_clock_survives_a_core_round_trip() {
    let _g = common::core_lock();
    let mgba = vendor("mgba_libretro.dylib");
    let gambatte = vendor("gambatte_libretro.dylib");
    let card_rom = common::repo_root().join("sdcard/Games/GBC/Pokemon - Crystal Version (USA).gbc");
    let card_sav = common::repo_root().join("sdcard/Saves/GBC/Pokemon - Crystal Version (USA).sav");
    if !mgba.exists() || !gambatte.exists() || !card_rom.exists() || !card_sav.exists() {
        eprintln!("no host cores or no Pokemon Crystal on the card, skipping");
        return;
    }
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = dir.path().join("Pokemon - Crystal Version (USA).gbc");
    std::fs::copy(&card_rom, &rom).expect("copy rom");

    let clock = 5 * 86_400 + 3 * 3_600 + 4 * 60 + 5;
    let start = unix_now();
    let mut sav = std::fs::read(&card_sav).expect("read save")[..SRAM].to_vec();
    for r in [0u32, 0, 0, 0, 0, 5, 4, 3, 5, 0] {
        sav.extend_from_slice(&r.to_le_bytes());
    }
    sav.extend_from_slice(&start.to_le_bytes());

    eprintln!("gambatte");
    let from_gambatte = run_core(&gambatte, dir.path(), &rom, &sav, BEFORE_CLEAR_CLOCK);
    eprintln!("  after, tail: {}", hex(&from_gambatte[SRAM..]));
    assert_eq!(from_gambatte.len(), SRAM + 8);

    eprintln!("mgba");
    let back = run_core(&mgba, dir.path(), &rom, &from_gambatte, BEFORE_CLEAR_CLOCK);
    let end = unix_now();
    eprintln!("  after, tail: {}", hex(&back[SRAM..]));
    assert_eq!(back.len(), SRAM + 48);

    let read = mgba_clock_at(&back[SRAM..], end);
    let expected = (clock + end - start) as i64;
    eprintln!(
        "mGBA clock {read} s, expected {expected} s, {} s of wall time",
        end - start
    );
    assert!(
        read.abs_diff(expected) <= 2,
        "the clock read {read} s after the round trip, expected {expected} s"
    );
}
