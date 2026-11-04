use std::path::Path;

pub fn cover(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode(path)?;
    cover_rgba(&src, sw, sh, w, h)
}

pub fn cover_bytes(bytes: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = decode_png_bytes(bytes)?;
    cover_rgba(&src, sw, sh, w, h)
}

pub(crate) fn cover_rgba(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Option<Vec<u8>> {
    if sw == 0 || sh == 0 {
        return None;
    }

    let scale = (w as f32 / sw as f32).max(h as f32 / sh as f32);
    let ox = (sw as f32 - w as f32 / scale) / 2.0;
    let oy = (sh as f32 - h as f32 / scale) / 2.0;

    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let y0 = oy + y as f32 / scale;
        let y1 = oy + (y + 1) as f32 / scale;
        for x in 0..w {
            let x0 = ox + x as f32 / scale;
            let x1 = ox + (x + 1) as f32 / scale;
            let px = box_average(src, sw, sh, x0, y0, x1, y1);
            out[((y * w + x) * 4) as usize..][..4].copy_from_slice(&px);
        }
    }
    Some(out)
}

fn box_average(src: &[u8], sw: u32, sh: u32, x0: f32, y0: f32, x1: f32, y1: f32) -> [u8; 4] {
    let xa = (x0.floor().max(0.0) as u32).min(sw - 1);
    let ya = (y0.floor().max(0.0) as u32).min(sh - 1);
    let xb = ((x1.ceil() as u32).max(xa + 1)).min(sw);
    let yb = ((y1.ceil() as u32).max(ya + 1)).min(sh);

    let mut acc = [0u32; 4];
    let mut n = 0u32;
    for y in ya..yb {
        for x in xa..xb {
            let i = ((y * sw + x) * 4) as usize;
            for c in 0..4 {
                acc[c] += src[i + c] as u32;
            }
            n += 1;
        }
    }
    let mut out = [0u8; 4];
    for c in 0..4 {
        out[c] = ((acc[c] + n / 2) / n) as u8;
    }
    out
}

pub fn decode(path: &Path) -> Option<(Vec<u8>, u32, u32)> {
    let bytes = std::fs::read(path).ok()?;
    decode_png_bytes(&bytes)
}

pub fn decode_png_bytes(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let orientation = exif_chunk(bytes).and_then(exif_orientation).unwrap_or(1);
    let mut dec = png::Decoder::new(bytes);
    dec.set_transformations(png::Transformations::normalize_to_color8());
    dec.set_limits(png::Limits { bytes: 64 << 20 });

    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let src = &buf[..info.buffer_size()];
    let n = (info.width * info.height) as usize;

    let mut rgba = vec![255u8; n * 4];
    match info.color_type {
        png::ColorType::Rgba => rgba.copy_from_slice(src),
        png::ColorType::Rgb => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].copy_from_slice(&src[i * 3..i * 3 + 3]);
            }
        }
        png::ColorType::Grayscale => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for i in 0..n {
                rgba[i * 4..i * 4 + 3].fill(src[i * 2]);
                rgba[i * 4 + 3] = src[i * 2 + 1];
            }
        }
        png::ColorType::Indexed => return None,
    }
    Some(orient(rgba, info.width, info.height, orientation))
}

fn exif_chunk(png: &[u8]) -> Option<&[u8]> {
    let mut at = 8;
    loop {
        let len = u32::from_be_bytes(png.get(at..at + 4)?.try_into().ok()?) as usize;
        let kind = png.get(at + 4..at + 8)?;
        match kind {
            b"eXIf" => return png.get(at + 8..at + 8 + len),
            b"IDAT" | b"IEND" => return None,
            _ => at += 12 + len,
        }
    }
}

fn exif_orientation(exif: &[u8]) -> Option<u16> {
    let big = match exif.get(..2)? {
        b"MM" => true,
        b"II" => false,
        _ => return None,
    };
    let u16_at = |i: usize| {
        let b: [u8; 2] = exif.get(i..i + 2)?.try_into().ok()?;
        Some(if big {
            u16::from_be_bytes(b)
        } else {
            u16::from_le_bytes(b)
        })
    };
    let u32_at = |i: usize| {
        let b: [u8; 4] = exif.get(i..i + 4)?.try_into().ok()?;
        Some(if big {
            u32::from_be_bytes(b)
        } else {
            u32::from_le_bytes(b)
        })
    };
    let ifd = u32_at(4)? as usize;
    let count = u16_at(ifd)? as usize;
    (0..count)
        .map(|n| ifd + 2 + n * 12)
        .find(|&e| u16_at(e) == Some(0x0112))
        .and_then(|e| u16_at(e + 8))
}

fn orient(src: Vec<u8>, w: u32, h: u32, orientation: u16) -> (Vec<u8>, u32, u32) {
    if !(2..=8).contains(&orientation) {
        return (src, w, h);
    }
    let (dw, dh) = if orientation >= 5 { (h, w) } else { (w, h) };
    let mut out = vec![0u8; src.len()];
    for y in 0..dh {
        for x in 0..dw {
            let (sx, sy) = match orientation {
                2 => (w - 1 - x, y),
                3 => (w - 1 - x, h - 1 - y),
                4 => (x, h - 1 - y),
                5 => (y, x),
                6 => (y, h - 1 - x),
                7 => (w - 1 - y, h - 1 - x),
                _ => (w - 1 - y, x),
            };
            let s = ((sy * w + sx) * 4) as usize;
            let d = ((y * dw + x) * 4) as usize;
            out[d..d + 4].copy_from_slice(&src[s..s + 4]);
        }
    }
    (out, dw, dh)
}

pub(crate) fn render_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)?;
    let size = tree.size();
    let scale =
        resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, scale, &mut pixmap.as_mut());
    let mut rgba = pixmap.data().to_vec();
    unpremultiply(&mut rgba);
    Some(rgba)
}

fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((*c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8;
        }
    }
}
