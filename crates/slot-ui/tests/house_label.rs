use slot_store::Platform;
use slot_ui::house_label::{stops, Label, BRIGHT};
use slot_ui::hue;

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

fn solid_png(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
    encode(w, h, &px.repeat((w * h) as usize))
}

fn decoded(png_bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    buf.truncate(info.buffer_size());
    (info.width, info.height, buf)
}

fn rgb_at(png_bytes: &[u8], x: u32, y: u32) -> [u8; 3] {
    let (w, _, buf) = decoded(png_bytes);
    let i = ((y * w + x) * 3) as usize;
    [buf[i], buf[i + 1], buf[i + 2]]
}

fn near(a: [u8; 3], b: [u8; 3], tol: i32) -> bool {
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| (*x as i32 - *y as i32).abs() <= tol)
}

#[test]
fn gba_logo_label_is_1280_by_661() {
    let label = Label::from_png(&logo_png(), [0x15, 0x65, 0x4c], Platform::Gba, None).unwrap();
    let (w, h, _) = decoded(&label.png());
    assert_eq!((w, h), (1280, 661));
}

#[test]
fn gb_real_label_fits_the_current_game_boy_well() {
    let label = Label::full(&solid_png(600, 528, [200, 40, 40, 255]), Platform::Gb).unwrap();
    let (w, h, _) = decoded(&label.png());
    assert_eq!((w, h), (1024, 913));
}

#[test]
fn fallback_hue_matches_the_shelf_hash() {
    assert_eq!(hue::fallback_hue("Kirby's Pinball Land (USA, Europe)"), 161);
}

#[test]
fn a_bright_logo_gets_the_deep_ground() {
    assert_eq!(stops(161, BRIGHT + 1.0).0, [0x15, 0x65, 0x4c]);
}

#[test]
fn the_top_left_corner_is_the_deep_stop() {
    let label = Label::from_png(&logo_png(), [0x2b, 0x3a, 0x88], Platform::Gba, None).unwrap();
    let corner = rgb_at(&label.png(), 0, 0);
    assert!(near(corner, [0x2b, 0x3a, 0x88], 2), "{corner:?}");
}

mod tag {
    use slot_store::Platform;
    use slot_ui::house_label::{logo_tag, untagged, Label};

    fn wheel() -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 40, 20);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&[255, 255, 255, 255].repeat(40 * 20))
                .unwrap();
        }
        out
    }

    #[test]
    fn a_composed_logo_label_says_what_it_is() {
        let label = Label::from_png(&wheel(), [0x12, 0x34, 0x56], Platform::Gba, None).unwrap();
        assert_eq!(logo_tag(&label.png()), Some([0x12, 0x34, 0x56]));
    }

    #[test]
    fn a_recolored_label_carries_the_new_color() {
        let mut label = Label::from_png(&wheel(), [1, 2, 3], Platform::Gba, None).unwrap();
        label.set_deep(&[9, 8, 7]);
        assert_eq!(logo_tag(&label.png()), Some([9, 8, 7]));
    }

    #[test]
    fn a_plain_png_has_no_tag() {
        assert_eq!(logo_tag(&wheel()), None);
        assert_eq!(logo_tag(b"not a png"), None);
    }

    #[test]
    fn stripping_leaves_the_bytes_a_plain_encode_makes() {
        let w = wheel();
        assert_eq!(untagged(&w), w);
        let tagged = Label::from_png(&w, [1, 2, 3], Platform::Gba, None).unwrap().png();
        let bare = untagged(&tagged);
        assert_eq!(logo_tag(&bare), None);
        assert!(bare.len() < tagged.len());
        let mut reader = png::Decoder::new(std::io::Cursor::new(&bare)).read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut buf).unwrap();
    }

    #[test]
    fn a_text_label_says_what_it_is() {
        use slot_ui::house_label::{label_tag, text_png, LabelTag};
        let png = text_png(&[10, 20, 30, 255].repeat(4), 2, 2, [10, 20, 30]);
        assert_eq!(label_tag(&png), Some(LabelTag::Text([10, 20, 30])));
        assert_eq!(logo_tag(&png), None);
    }

    #[test]
    fn a_real_label_has_no_tag() {
        let label = Label::full(&wheel(), Platform::Gba).unwrap();
        assert_eq!(logo_tag(&label.png()), None);
    }

    fn with_text(text: &[u8]) -> Vec<u8> {
        let w = wheel();
        let mut out = w[..33].to_vec();
        out.extend_from_slice(&(text.len() as u32).to_be_bytes());
        out.extend_from_slice(b"tEXt");
        out.extend_from_slice(text);
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&w[33..]);
        out
    }

    #[test]
    fn the_tag_real_labels_used_to_carry_is_ignored() {
        assert_eq!(logo_tag(&with_text(b"slot-label\0logo 000000")), None);
    }

    #[test]
    fn a_tag_with_no_text_is_no_tag() {
        assert_eq!(logo_tag(&with_text(b"slot-label")), None);
        assert_eq!(logo_tag(&with_text(b"slot-label\0")), None);
    }
}
