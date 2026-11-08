use std::path::Path;

use slot::overlay::usable;
use slot_store::{OverlayEntry, Platform};

fn put(root: &Path, dir: &str, name: &str, w: u32, h: u32) {
    let dir = root.join("Overlays").join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&vec![0u8; (w * h * 4) as usize])
        .unwrap();
    drop(writer);
    std::fs::write(dir.join(name), out).unwrap();
}

fn names(e: &[OverlayEntry]) -> Vec<&str> {
    e.iter().map(|e| e.name.as_str()).collect()
}

#[test]
fn a_full_screen_overlay_must_match_the_frame_exactly() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GBA", "glass.png", 720, 480);
    put(d.path(), "GBA", "small.png", 640, 480);
    let e = usable(d.path(), Platform::Gba, (720, 480), (720, 480));
    assert_eq!(names(&e), ["glass"]);
}

#[test]
fn a_tile_larger_than_the_picture_is_left_out() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GBA", "grid_tile.png", 4, 4);
    put(d.path(), "GBA", "huge_tile.png", 800, 10);
    let e = usable(d.path(), Platform::Gba, (720, 480), (720, 480));
    assert_eq!(names(&e), ["grid"]);
}

#[test]
fn a_part_that_does_not_fit_is_dropped_from_its_entry() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GBA", "lcd_tile.png", 3, 3);
    put(d.path(), "GBA", "lcd.png", 640, 480);
    let e = usable(d.path(), Platform::Gba, (720, 480), (720, 480));
    assert_eq!(e[0].tile.as_deref(), Some("lcd_tile.png"));
    assert_eq!(e[0].both, None);
}

#[test]
fn gbav_reads_the_gba_folder() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GBA", "grid_tile.png", 3, 3);
    let e = usable(d.path(), Platform::Gbav, (720, 480), (720, 480));
    assert_eq!(names(&e), ["grid"]);
}

#[test]
fn a_missing_folder_has_no_overlays() {
    let d = tempfile::tempdir().unwrap();
    assert!(usable(d.path(), Platform::Gb, (720, 480), (480, 432)).is_empty());
}

#[test]
fn hidden_and_broken_files_are_skipped() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GB", "._grid_tile.png", 3, 3);
    std::fs::write(d.path().join("Overlays/GB/broken_tile.png"), b"not a png").unwrap();
    assert!(usable(d.path(), Platform::Gb, (720, 480), (480, 432)).is_empty());
}

#[cfg(feature = "nds")]
#[test]
fn a_top_only_ds_set_keeps_its_top_part() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "NDS", "glass_top.png", 1024, 768);
    let e = usable(d.path(), Platform::Nds, (1024, 768), (1024, 768));
    assert_eq!(e[0].top.as_deref(), Some("glass_top.png"));
    assert_eq!(e[0].bottom, None);
}

#[test]
fn the_card_gets_an_overlays_folder_per_system() {
    for dir in ["Overlays", "Overlays/GBA", "Overlays/GB", "Overlays/GBC"] {
        assert!(slot::root::DIRS.contains(&dir), "{dir} missing");
    }
}

#[test]
fn a_top_suffix_on_one_screen_is_a_plain_full_overlay() {
    let d = tempfile::tempdir().unwrap();
    put(d.path(), "GBA", "desert_top.png", 720, 480);
    let e = usable(d.path(), Platform::Gba, (720, 480), (720, 480));
    assert_eq!(names(&e), ["desert_top"]);
}

#[test]
fn an_rgb_overlay_decodes_to_opaque_rgba() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("rgb.png");
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 2, 1);
    enc.set_color(png::ColorType::Rgb);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[10, 20, 30, 40, 50, 60]).unwrap();
    drop(writer);
    std::fs::write(&path, out).unwrap();
    let (w, h, rgba) = slot::overlay::decode_rgba(&path).unwrap();
    assert_eq!((w, h), (2, 1));
    assert_eq!(rgba, [10, 20, 30, 255, 40, 50, 60, 255]);
}

#[test]
fn a_gray_alpha_overlay_decodes_to_rgba() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("ga.png");
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 1, 1);
    enc.set_color(png::ColorType::GrayscaleAlpha);
    let mut writer = enc.write_header().unwrap();
    writer.write_image_data(&[90, 128]).unwrap();
    drop(writer);
    std::fs::write(&path, out).unwrap();
    assert_eq!(slot::overlay::decode_rgba(&path).unwrap().2, [90, 90, 90, 128]);
}
