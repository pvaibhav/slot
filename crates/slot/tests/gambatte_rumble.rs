mod common;

use std::path::PathBuf;

use slot::core::open_core_for_palette;
use slot_retro::ButtonMask;
use slot_store::{Core, Platform};

fn rumble_cart(dir: &std::path::Path) -> PathBuf {
    let mut rom = vec![0u8; 0x8000];
    rom[0x100..0x104].copy_from_slice(&[0x00, 0xc3, 0x50, 0x01]);
    rom[0x134..0x13d].copy_from_slice(b"RUMBLETST");
    rom[0x147] = 0x1c;
    rom[0x148] = 0x00;
    rom[0x149] = 0x00;
    rom[0x150..0x157].copy_from_slice(&[0x3e, 0x08, 0xea, 0x00, 0x40, 0x18, 0xf9]);
    let sum = rom[0x134..0x14d]
        .iter()
        .fold(0u8, |a, b| a.wrapping_sub(*b).wrapping_sub(1));
    rom[0x14d] = sum;
    let path = dir.join("rumble.gb");
    std::fs::write(&path, &rom).expect("write test rom");
    path
}

#[test]
fn a_rumble_cart_on_gambatte_reaches_the_motor() {
    let _g = common::core_lock();
    let dylib = common::repo_root().join("vendor/gambatte_libretro.dylib");
    if !dylib.exists() {
        eprintln!("no host gambatte, skipping");
        return;
    }
    let root = tempfile::tempdir().expect("tempdir");
    let rom = rumble_cart(root.path());
    let mut core = open_core_for_palette(
        root.path(),
        Core::Gambatte,
        Platform::Gb,
        "",
        false,
        None,
        &[dylib],
    );
    core.load(&rom).expect("gambatte refused the cart");
    let rumble = core.rumble();
    let mut felt = 0;
    for _ in 0..10 {
        core.run_frame(ButtonMask::default());
        felt = felt.max(rumble.strength());
    }
    assert!(
        felt > 0,
        "the cart drove its motor and gambatte passed nothing on"
    );
}
