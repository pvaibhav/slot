use slot_store::{Cart, Platform};
use slot_ui::board_face;

#[test]
fn render_board() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let face = board_face(&Cart {
        platform: Platform::Gba,
        stem: "Pokemon - Emerald Version (USA, Europe)".into(),
        rom: "Games/GBA/Emerald.gba".into(),
        label: None,
        code: "BPEE".into(),
        shell: None,
        title: "POKEMON EMER".into(),
        sealed: false,
        tags: Vec::new(),
    });
    let f = std::fs::File::create(&out).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), face.w, face.h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&face.rgba)
        .unwrap();
    println!("wrote {out}");
}
