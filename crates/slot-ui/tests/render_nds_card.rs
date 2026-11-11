#![cfg(feature = "nds")]

use slot_store::{Cart, Platform};
use slot_ui::{cart_box, cart_face, label_size};

const BANNER: usize = 0x1000;

fn ds_rom(dir: &std::path::Path) -> std::path::PathBuf {
    let mut rom = vec![0u8; BANNER + 0xA40];
    rom[..12].copy_from_slice(b"KART        ");
    rom[0x68..0x6c].copy_from_slice(&(BANNER as u32).to_le_bytes());
    for b in &mut rom[BANNER + 0x20..BANNER + 0x220] {
        *b = 0x11;
    }
    rom[BANNER + 0x222..BANNER + 0x224].copy_from_slice(&0x001fu16.to_le_bytes());
    for (i, unit) in "Kart".encode_utf16().enumerate() {
        rom[BANNER + 0x340 + i * 2..BANNER + 0x342 + i * 2].copy_from_slice(&unit.to_le_bytes());
    }
    let p = dir.join("Kart.nds");
    std::fs::write(&p, rom).unwrap();
    p
}

fn ds_cart(dir: &std::path::Path, label: Option<std::path::PathBuf>) -> Cart {
    Cart {
        platform: Platform::Nds,
        stem: "Kart".into(),
        rom: ds_rom(dir),
        label,
        title: String::new(),
        code: String::new(),
        shell: None,
        sealed: false,
    }
}

#[test]
fn a_ds_card_is_33_by_35() {
    let (w, h) = cart_box(Platform::Nds);
    assert!(
        ((w as f32 / h as f32) - 33.0 / 35.0).abs() < 0.01,
        "{w}x{h}"
    );
    let d = tempfile::tempdir().unwrap();
    let face = cart_face(&ds_cart(d.path(), None));
    assert_eq!((face.w, face.h), (w, h));
    let opaque = |x: u32, y: u32| face.rgba[((y * w + x) * 4 + 3) as usize] > 128;
    assert!(opaque(w / 2, h / 2) && opaque(w / 2, 2) && opaque(2, h / 2));
    assert!(!opaque(0, 0), "the card's corner is square, not rounded");
}

#[test]
fn a_ds_card_without_a_label_shows_its_banner_icon() {
    let d = tempfile::tempdir().unwrap();
    let face = cart_face(&ds_cart(d.path(), None));
    let red = face
        .rgba
        .chunks_exact(4)
        .filter(|p| p[0] > 240 && p[1] < 16 && p[2] < 16 && p[3] == 255)
        .count();
    assert!(
        red >= 64 * 64,
        "the banner icon is missing or too small: {red} red pixels"
    );
}

#[test]
fn a_ds_label_is_nearly_square() {
    let d = tempfile::tempdir().unwrap();
    let (lw, lh) = label_size(&ds_cart(d.path(), None));
    let r = lw as f32 / lh as f32;
    assert!((0.85..1.0).contains(&r), "label {lw}x{lh}");
    let (hw, hh) = slot_ui::house_label::size(Platform::Nds);
    assert!(
        ((hw as f32 / hh as f32) - r).abs() < 0.01,
        "house label {hw}x{hh} vs well {lw}x{lh}"
    );
}

#[test]
fn a_ds_card_is_as_tall_as_a_gba_cart_and_33_35ths_as_wide() {
    let (gw, gh) = cart_box(Platform::Gba);
    let (w, h) = cart_box(Platform::Nds);
    assert_eq!(h, gh, "a DS card and a GBA cart are both 35 mm tall");
    assert!(
        (w as i32 - (h as f32 * 33.0 / 35.0).round() as i32).abs() <= 1,
        "{w}x{h}"
    );
    assert!(
        w < gw * 2 / 3,
        "the DS card is {w} wide next to a {gw} GBA cart"
    );
    let (sw, sh) = slot_ui::seated_box(Platform::Nds);
    let (gsw, gsh) = slot_ui::seated_box(Platform::Gba);
    assert_eq!(sh, gsh);
    assert!(
        (sw as f32 / sh as f32 - w as f32 / h as f32).abs() < 0.02,
        "seated {sw}x{sh}"
    );
    assert!(sw < gsw);
}

#[test]
fn the_ds_slot_is_narrower_than_the_gba_slot_and_keeps_its_ends() {
    let gba = slot_ui::slot_back();
    let ds = slot_ui::ds_slot_back();
    assert_eq!(gba.len(), ds.len());
    let w = slot_ui::SLOT_W as usize;
    let row = |px: &[u8], y: usize, x: usize| px[(y * w + x) * 4..(y * w + x) * 4 + 4].to_vec();
    for y in [5usize, 30, 50] {
        for x in [10usize, 200, 520, 710] {
            assert_eq!(
                row(&gba, y, x),
                row(&ds, y, x),
                "the strip changed outside the mouth at {x},{y}"
            );
        }
    }
    assert_ne!(gba, ds, "the DS slot is the GBA slot");
    let (sw, _) = slot_ui::seated_box(Platform::Nds);
    assert!(
        slot_ui::DS_MOUTH_W >= sw + 8 && (slot_ui::DS_MOUTH_W as f32) < slot_ui::MOUTH_W,
        "mouth {}",
        slot_ui::DS_MOUTH_W
    );
}
