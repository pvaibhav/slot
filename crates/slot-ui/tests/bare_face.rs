use slot_store::{Cart, Outline, Platform, ShellChoice, ShellFinish};
use slot_ui::{bare_face, cart_face};

fn gba(finish: ShellFinish) -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: "Probe".into(),
        rom: "Games/GBA/Probe.gba".into(),
        label: None,
        code: String::new(),
        shell: Some(ShellChoice {
            outline: Outline::Auto,
            colour: [0xc2, 0x33, 0x2e],
            finish,
        }),
        title: "PROBE".into(),
    }
}

fn overlay(base: u8, color: u8) -> i32 {
    let (b, c) = (base as i32, color as i32);
    match b >= 128 {
        true => c + ((255 - c) * (b - 128) + 63) / 127,
        false => c - (c * (128 - b) + 64) / 128,
    }
}

fn neutral_at(x: u32, y: u32) -> [u8; 3] {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/gba_neutral.png");
    let mut dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).unwrap();
    let ch = info.color_type.samples() as u32;
    let i = ((y * info.width + x) * ch) as usize;
    [buf[i], buf[i + 1], buf[i + 2]]
}

fn at(face: &slot_ui::CartFace, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * face.w + x) * 4) as usize;
    [face.rgba[i], face.rgba[i + 1], face.rgba[i + 2], face.rgba[i + 3]]
}

#[test]
fn bare_well_is_the_shell_color() {
    let face = bare_face(&gba(ShellFinish::Solid));
    let px = at(&face, 180, 118);
    let base = neutral_at(180, 118);
    let want = [0, 1, 2].map(|k| overlay(base[k], [0xc2u8, 0x33, 0x2e][k]));
    for k in 0..3 {
        assert!((px[k] as i32 - want[k]).abs() <= 6, "{px:?} vs {want:?}");
    }
    assert_eq!(px[3], 255);
}

#[test]
fn bare_well_on_a_clear_shell_is_see_through() {
    let face = bare_face(&gba(ShellFinish::Clear));
    assert!(at(&face, 180, 118)[3] < 255);
}

#[test]
fn labelled_faces_keep_the_label() {
    let face = cart_face(&gba(ShellFinish::Solid));
    let bare = bare_face(&gba(ShellFinish::Solid));
    assert_ne!(at(&face, 180, 118), at(&bare, 180, 118));
    assert_eq!(at(&face, 10, 100), at(&bare, 10, 100));
}
