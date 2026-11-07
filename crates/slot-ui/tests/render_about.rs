use slot_gfx::{Canvas, Draw, TexId, OUT_H, OUT_W};
use slot_ui::{draw_backdrop, draw_sticker, sticker_face, wallpaper_face, StickerFields};

#[test]
fn render_about() {
    let Ok(out) = std::env::var("SCRATCH_PNG") else {
        return;
    };
    let model = std::env::var("SCRATCH_MODEL").unwrap_or_default();
    let paper = std::env::var("SCRATCH_WALL")
        .ok()
        .and_then(|p| wallpaper_face(std::path::Path::new(&p), OUT_W, OUT_H));
    let fields = StickerFields {
        battery: Some(87),
        serial: "04738857",
        version: "0.1.0",
        baseos: Some("1.5.2"),
        model: &model,
    };
    let label = match std::env::var("SCRATCH_DS").is_ok() {
        true => slot_ui::ds_sticker_face(&fields),
        false => sticker_face(&fields),
    };

    let mut list = Vec::new();
    draw_backdrop(
        Canvas::FRAMED,
        paper.as_ref().map(|_| (TexId::from_raw(0), OUT_W)),
        &mut list,
    );
    draw_sticker(
        Canvas::FRAMED,
        Some((TexId::from_raw(1), label.w, label.h)),
        &mut list,
    );

    let (w, h) = (OUT_W as usize, OUT_H as usize);
    let mut px = vec![0u8; w * h * 4];
    for p in px.chunks_mut(4) {
        p.copy_from_slice(&[0, 0, 0, 255]);
    }
    let blend = |dst: &mut [u8], c: [f32; 3], a: f32| {
        for k in 0..3 {
            dst[k] = (c[k] * 255.0 * a + dst[k] as f32 * (1.0 - a)) as u8;
        }
    };

    for d in &list {
        match *d {
            Draw::Rect {
                x,
                y,
                w: rw,
                h: rh,
                colour,
            } => {
                for yy in y as usize..((y + rh) as usize).min(h) {
                    for xx in x as usize..((x + rw) as usize).min(w) {
                        let at = (yy * w + xx) * 4;
                        blend(
                            &mut px[at..at + 4],
                            [colour[0], colour[1], colour[2]],
                            colour[3],
                        );
                    }
                }
            }
            Draw::Tex {
                x,
                y,
                w: tw,
                h: th,
                tex,
                alpha,
            } => {
                let (src, sw, sh) = match tex == TexId::from_raw(0) {
                    true => (paper.as_deref().unwrap_or(&[]), OUT_W, OUT_H),
                    false => (label.rgba.as_slice(), label.w, label.h),
                };
                if src.is_empty() {
                    continue;
                }
                for row in 0..th as usize {
                    for col in 0..tw as usize {
                        let (sx, sy) = (
                            col * sw as usize / tw as usize,
                            row * sh as usize / th as usize,
                        );
                        let s = (sy * sw as usize + sx) * 4;
                        let a = src[s + 3] as f32 / 255.0 * alpha;
                        let (dx, dy) = (x as usize + col, y as usize + row);
                        if a > 0.0 && dx < w && dy < h {
                            let at = (dy * w + dx) * 4;
                            let c = [
                                src[s] as f32 / 255.0,
                                src[s + 1] as f32 / 255.0,
                                src[s + 2] as f32 / 255.0,
                            ];
                            blend(&mut px[at..at + 4], c, a);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let f = std::fs::File::create(&out).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&px).unwrap();
    println!("wrote {out} ({} draws)", list.len());
}
