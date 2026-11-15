use slot::art::index::Look;
use slot::art::make::{logo_label, real_label, write_new};
use slot_store::Platform;

fn encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
    }
    out
}

fn logo_png() -> Vec<u8> {
    let (w, h) = (200u32, 80u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 15..65 {
        for x in 20..180 {
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, 255]);
        }
    }
    encode(w, h, &rgba)
}

fn corner(png_bytes: &[u8]) -> [u8; 3] {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    reader.next_frame(&mut buf).unwrap();
    [buf[0], buf[1], buf[2]]
}

fn near(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= 2)
}

#[test]
fn no_look_uses_the_shelf_hue() {
    let png = logo_label(
        &logo_png(),
        None,
        "Kirby's Pinball Land (USA, Europe)",
        Platform::Gb,
    )
    .unwrap();
    assert!(near(corner(&png), [0x15, 0x65, 0x4c]), "{:?}", corner(&png));
}

#[test]
fn a_sure_look_sets_the_ground() {
    let look = Look {
        ground: [0xf9, 0xca, 0x06],
        band: None,
    };
    let png = logo_label(&logo_png(), Some(&look), "Anything", Platform::Gba).unwrap();
    assert!(near(corner(&png), [0xf9, 0xca, 0x06]), "{:?}", corner(&png));
}

#[test]
fn html_is_not_a_label() {
    assert!(real_label(b"<html>404</html>", Platform::Gba).is_none());
    assert!(logo_label(b"<html>404</html>", None, "X", Platform::Gba).is_none());
}

#[test]
fn write_new_keeps_an_existing_label() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Labels/GB/Kirby.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"mine").unwrap();
    assert!(!write_new(&path, b"theirs").unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    assert!(!path.with_extension("png.part").exists());
}

#[test]
fn write_new_writes_when_absent() {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("Labels/GB/Kirby.png");
    assert!(write_new(&path, b"new").unwrap());
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

#[test]
fn a_jpeg_scan_makes_a_real_label() {
    let jpeg = include_bytes!("fixtures/solid_scan.jpg");
    let png = real_label(jpeg, Platform::Gba).unwrap();
    let c = corner(&png);
    assert!(
        (c[0] as i32 - 200).abs() <= 6 && (c[1] as i32 - 40).abs() <= 6,
        "{c:?}"
    );
}
