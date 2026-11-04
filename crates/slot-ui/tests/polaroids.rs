use std::path::Path;

use slot_power::{Battery, Charge};
use slot_store::StateEntry;
use slot_ui::{
    photo_face, Draw, PhotoFace, Polaroids, Printed, TexId, OUT_W, PHOTO_H, PHOTO_W, PLATE_H,
};

fn entry(dir: &Path, stamp: &str) -> StateEntry {
    StateEntry {
        stamp: stamp.to_string(),
        state: dir.join(format!("{stamp}.state")),
        thumb: dir.join(format!("{stamp}.png")),
    }
}

fn switcher() -> Polaroids {
    Polaroids::new(Vec::new())
}

fn write_png(path: &Path, w: u32, h: u32, rgb: [u8; 3]) {
    let file = std::fs::File::create(path).expect("create png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().expect("png header");
    let pixels: Vec<u8> = std::iter::repeat_n(rgb, (w * h) as usize)
        .flatten()
        .collect();
    writer.write_image_data(&pixels).expect("png data");
}

fn pixel(face: &PhotoFace, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2]]
}

#[test]
fn relative_time_reads_off_the_stamp() {
    let now = "2026-08-09_14-36-05";
    assert_eq!(
        Polaroids::relative_time("2026-08-09_14-32-05", now, true),
        "4 min ago"
    );
    assert_eq!(
        Polaroids::relative_time("2026-08-09_13-36-05", now, true),
        "1 hr ago"
    );
    assert_eq!(
        Polaroids::relative_time("2026-08-09_11-36-05", now, true),
        "3 hr ago"
    );
}

#[test]
fn an_hour_before_midnight_is_not_yesterday() {
    assert_eq!(
        Polaroids::relative_time("2026-08-08_23-30-00", "2026-08-09_01-00-00", true),
        "1 hr ago"
    );
}

#[test]
fn a_stamp_ahead_of_the_clock_does_not_read_as_a_lifetime_ago() {
    assert_eq!(
        Polaroids::relative_time("2026-08-09_15-00-00", "2026-08-09_14-36-05", true),
        "just now"
    );
}

#[test]
fn an_entry_whose_thumbnail_is_missing_still_has_a_face() {
    let face = photo_face(&entry(Path::new("/nonexistent"), "2026-08-09_14-32-05"));
    assert_eq!((face.w, face.h), (PHOTO_W, PHOTO_H));
    assert!(
        face.rgba.chunks_exact(4).all(|p| p[3] == 255),
        "the blank photo is see through"
    );
}

#[test]
fn the_photo_fills_the_face_and_keeps_its_colours() {
    let d = tempfile::tempdir().expect("tempdir");
    let e = entry(d.path(), "2026-08-09_14-32-05");
    write_png(&e.thumb, PHOTO_W, PHOTO_H, [0xd0, 0x40, 0x20]);
    let face = photo_face(&e);
    for (x, y) in [
        (0, 0),
        (PHOTO_W / 2, PHOTO_H / 2),
        (PHOTO_W - 1, PHOTO_H - 1),
    ] {
        assert_eq!(pixel(&face, x, y), [0xd0, 0x40, 0x20], "at {x},{y}");
    }
}

#[test]
fn the_index_clamps_at_both_ends() {
    let d = Path::new("/nonexistent");
    let mut p = Polaroids::new(vec![
        entry(d, "2026-08-09_00-00-02"),
        entry(d, "2026-08-09_00-00-01"),
    ]);
    p.left();
    assert_eq!(p.index, 0);
    p.right();
    p.right();
    p.right();
    assert_eq!(p.index, 1);
    assert_eq!(
        p.selected().map(|e| e.stamp.as_str()),
        Some("2026-08-09_00-00-01")
    );
}

#[test]
fn the_gauge_stays_clear_of_the_title_on_the_left() {
    let p = switcher();
    let mut out = Vec::new();
    p.draw(
        Some(Battery {
            percent: 68,
            charge: Charge::Charging,
        }),
        Printed { face: None, w: 30 },
        Some(TexId::from_raw(7)),
        Printed::default(),
        &mut out,
    );
    let gauge: Vec<_> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, .. } | Draw::Tex { x, y, w, .. }
                if y < PLATE_H && w < OUT_W as f32 =>
            {
                Some((x, w))
            }
            _ => None,
        })
        .collect();
    assert!(!gauge.is_empty(), "no gauge drew at the left margin at all");
    let leftmost = gauge.iter().map(|(x, _)| *x).fold(f32::MAX, f32::min);
    assert_eq!(leftmost, 16.0, "the plate margin is 16");
    let rightmost = gauge.iter().map(|(x, w)| x + w).fold(0.0, f32::max);
    assert!(
        rightmost < 180.0,
        "the gauge reached into the title's texture"
    );
}

#[test]
fn the_clock_stays_clear_of_the_title_on_the_right() {
    let p = switcher();
    let mut out = Vec::new();
    p.draw(
        None,
        Printed::default(),
        None,
        Printed { face: None, w: 40 },
        &mut out,
    );
    let clock: Vec<_> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, .. } | Draw::Tex { x, y, w, .. } if y < PLATE_H && x > 540.0 => {
                Some((x, w))
            }
            _ => None,
        })
        .collect();
    assert!(!clock.is_empty(), "no clock drew past the title at all");
    let leftmost = clock.iter().map(|(x, _)| *x).fold(f32::MAX, f32::min);
    assert!(
        leftmost > 540.0,
        "the clock reached into the title's texture"
    );
    let rightmost = clock.iter().map(|(x, w)| x + w).fold(0.0, f32::max);
    assert_eq!(rightmost, OUT_W as f32 - 16.0, "the plate margin is 16");
}

#[test]
fn a_switcher_with_no_gauge_still_shows_its_clock() {
    let p = switcher();
    let mut out = Vec::new();
    p.draw(
        None,
        Printed::default(),
        None,
        Printed { face: None, w: 40 },
        &mut out,
    );
    assert!(
        out.iter()
            .any(|d| matches!(d, Draw::Rect { y, w, .. } if *y < PLATE_H && *w == 40.0)),
        "the clock's placeholder did not draw when there was no gauge"
    );
}

#[test]
fn a_shot_stamp_with_milliseconds_reads_as_relative_time() {
    assert_eq!(
        Polaroids::relative_time("2026-10-22_17-30-05-120", "2026-10-22_17-32-05", true),
        "2 min ago"
    );
}
