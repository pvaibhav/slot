use slot_ui::{sticker_face, StickerFields, STICKER_H, STICKER_W};

fn write(path: &str, px: &[u8], w: usize, h: usize) {
    let f = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(px).unwrap();
}

#[test]
fn render_sticker() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let model = std::env::var("SCRATCH_MODEL").unwrap_or_default();
    let face = sticker_face(&StickerFields {
        battery: Some(87),
        serial: "04738857",
        version: "0.1.0",
        baseos: Some("1.5.2"),
        model: &model,
    });
    let (w, h) = (STICKER_W as usize, STICKER_H as usize);
    write(&out, &face.rgba, w, h);

    if let Ok(crop) = std::env::var("SCRATCH_CROP") {
        let n: Vec<usize> = std::env::var("SCRATCH_BOX")
            .unwrap_or_else(|_| "0,0,200,60".into())
            .split(',')
            .map(|v| v.parse().unwrap())
            .collect();
        let (cx, cy, cw, ch) = (n[0], n[1], n[2], n[3]);
        let z = 5usize;
        let mut zoom = vec![0u8; cw * z * ch * z * 4];
        for y in 0..ch * z {
            for x in 0..cw * z {
                let (sx, sy) = (cx + x / z, cy + y / z);
                if sx < w && sy < h {
                    let s = (sy * w + sx) * 4;
                    let d = (y * cw * z + x) * 4;
                    zoom[d..d + 4].copy_from_slice(&face.rgba[s..s + 4]);
                }
            }
        }
        write(&crop, &zoom, cw * z, ch * z);
        println!("wrote {crop}");
    }
    println!("wrote {out}");
}
