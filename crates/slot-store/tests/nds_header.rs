#![cfg(feature = "nds")]

use slot_store::read_banner;

const BANNER: usize = 0x1000;

fn rom_with_banner(title: &str, palette1: u16, pixel_index: u8) -> Vec<u8> {
    let mut rom = vec![0u8; BANNER + 0xA40];
    rom[..12].copy_from_slice(b"HEADERTITLE!");
    rom[0x68..0x6c].copy_from_slice(&(BANNER as u32).to_le_bytes());
    rom[BANNER] = 1;
    let byte = pixel_index | (pixel_index << 4);
    for b in &mut rom[BANNER + 0x20..BANNER + 0x220] {
        *b = byte;
    }
    rom[BANNER + 0x222..BANNER + 0x224].copy_from_slice(&palette1.to_le_bytes());
    let english = BANNER + 0x340;
    for (i, unit) in title.encode_utf16().enumerate() {
        rom[english + i * 2..english + i * 2 + 2].copy_from_slice(&unit.to_le_bytes());
    }
    rom
}

fn write(rom: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("Kart.nds");
    std::fs::write(&p, rom).unwrap();
    (d, p)
}

#[test]
fn reads_title_and_icon_from_a_banner() {
    let (_d, p) = write(&rom_with_banner("Hello\nWorld", 0x001f, 1));
    let banner = read_banner(&p).expect("no banner read");
    assert_eq!(banner.title, "Hello");
    assert_eq!(banner.icon_rgba.len(), 32 * 32 * 4);
    assert_eq!(&banner.icon_rgba[..4], &[255, 0, 0, 255]);
    assert_eq!(&banner.icon_rgba[(31 * 32 + 31) * 4..], &[255, 0, 0, 255]);
}

#[test]
fn color_zero_is_transparent() {
    let (_d, p) = write(&rom_with_banner("Hello", 0x001f, 0));
    let banner = read_banner(&p).unwrap();
    assert_eq!(banner.icon_rgba[3], 0);
}

#[test]
fn no_banner_falls_back_to_the_header_title() {
    let mut rom = rom_with_banner("Hello", 0x001f, 1);
    rom[0x68..0x6c].copy_from_slice(&0u32.to_le_bytes());
    let (_d, p) = write(&rom);
    let banner = read_banner(&p).unwrap();
    assert_eq!(banner.title, "HEADERTITLE!");
    assert!(banner.icon_rgba.chunks(4).all(|px| px[3] == 0));
}

#[test]
fn a_tiny_file_reads_nothing() {
    let (_d, p) = write(&[0u8; 8]);
    assert!(read_banner(&p).is_none());
}
