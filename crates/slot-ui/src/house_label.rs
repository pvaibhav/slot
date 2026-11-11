use resvg::tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};
use slot_store::Platform;

use crate::{art, hue};

const GLANCE_W: usize = 32;
const GLANCE_H: usize = 16;
const GLANCE_TOLERANCE: u8 = 16;
const TAG_KEY: &str = "slot-label";

pub const W: u32 = 1280;
pub const H: u32 = 661;

pub const GB_W: u32 = 1024;
pub const GB_H: u32 = GB_W * crate::cart::GB_LABEL_H / crate::cart::GB_LABEL_W;
#[cfg(feature = "nds")]
pub const DS_W: u32 = 1024;
#[cfg(feature = "nds")]
pub const DS_H: u32 = 1104;

pub fn size(platform: Platform) -> (u32, u32) {
    match platform {
        Platform::Gba | Platform::Gbav => (W, H),
        #[cfg(feature = "nds")]
        Platform::Nds => (DS_W, DS_H),
        Platform::Gb | Platform::Gbc => (GB_W, GB_H),
    }
}

const BOX_W_OF: f32 = 920.0 / W as f32;
const BOX_H_OF: f32 = 423.0 / 640.0;
const TALL_TOP_OF: f32 = 70.0 / 640.0;
const MID_ROW_OF: f32 = 320.0 / 640.0;
const ALPHA_EDGE: u8 = 16;

pub const BRIGHT: f32 = 110.0;

pub fn stops(hue: u16, logo_luma: f32) -> ([u8; 3], [u8; 3]) {
    let h = hue as f32;
    if logo_luma < BRIGHT {
        (
            hue::hsl_to_rgb(h, 0.55, 0.62),
            hue::hsl_to_rgb(h, 0.50, 0.84),
        )
    } else {
        let h = if (OLIVE_LO..=OLIVE_HI).contains(&hue) {
            AMBER
        } else {
            h
        };
        (
            hue::hsl_to_rgb(h, 0.65, 0.24),
            hue::hsl_to_rgb(h, 0.60, 0.46),
        )
    }
}

const OLIVE_LO: u16 = 45;
const OLIVE_HI: u16 = 75;
const AMBER: f32 = 38.0;

pub fn pale_for(deep: [u8; 3]) -> [u8; 3] {
    let (h, s, l) = hue::rgb_to_hsl(deep);
    hue::hsl_to_rgb(h, (s - 0.05).clamp(0.0, 1.0), (l + 0.22).clamp(0.0, 1.0))
}

pub fn gradient(lw: u32, lh: u32, deep: [u8; 3], pale: [u8; 3]) -> Vec<u8> {
    let mut px = Vec::with_capacity((lw * lh * 4) as usize);
    for y in 0..lh {
        for x in 0..lw {
            let t = 0.88 * x as f32 / lw as f32 + 0.12 * y as f32 / lh as f32;
            for c in 0..3 {
                px.push((deep[c] as f32 + (pale[c] as f32 - deep[c] as f32) * t).round() as u8);
            }
            px.push(255);
        }
    }
    px
}

pub fn trim(rgba: &[u8], w: u32, h: u32) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for y in 0..h {
        for x in 0..w {
            if rgba[((y * w + x) * 4 + 3) as usize] < ALPHA_EDGE {
                continue;
            }
            bounds = Some(match bounds {
                None => (x, y, x + 1, y + 1),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)),
            });
        }
    }
    bounds
}

pub fn placement(lw: u32, lh: u32, w: u32, h: u32) -> (f32, f32, f32) {
    let by_w = BOX_W_OF * lw as f32 / w as f32;
    let by_h = BOX_H_OF * lh as f32 / h as f32;
    let scale = by_w.min(by_h);
    let x = (lw as f32 - w as f32 * scale) / 2.0;
    let y = if by_h <= by_w {
        TALL_TOP_OF * lh as f32
    } else {
        MID_ROW_OF * lh as f32 - h as f32 * scale / 2.0
    };
    (scale, x, y)
}

fn premultiplied(logo: &[u8], w: u32, (x0, y0, x1, y1): (u32, u32, u32, u32)) -> Vec<u8> {
    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for y in y0..y1 {
        for x in x0..x1 {
            let i = ((y * w + x) * 4) as usize;
            let a = logo[i + 3] as u32;
            for c in 0..3 {
                out.push(((logo[i + c] as u32 * a + 127) / 255) as u8);
            }
            out.push(a as u8);
        }
    }
    out
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Band {
    pub edge: Edge,
    pub size: f32,
    pub colour: [u8; 3],
}

impl Band {
    pub fn parse(edge: &str, size: f32, colour: &[u8]) -> Option<Band> {
        let edge = match edge {
            "top" => Edge::Top,
            "bottom" => Edge::Bottom,
            "left" => Edge::Left,
            "right" => Edge::Right,
            _ => return None,
        };
        (size > 0.0 && size < 0.5 && colour.len() >= 3).then(|| Band {
            edge,
            size,
            colour: [colour[0], colour[1], colour[2]],
        })
    }

    fn strip(&self, lw: u32, lh: u32) -> (u32, u32, u32, u32) {
        let (x0, y0, x1, y1) = shown(lw, lh);
        let (tw, th) = (self.across(x1 - x0), self.across(y1 - y0));
        match self.edge {
            Edge::Top => (0, 0, lw, y0 + th),
            Edge::Bottom => (0, y1 - th, lw, lh),
            Edge::Left => (0, 0, x0 + tw, lh),
            Edge::Right => (x1 - tw, 0, lw, lh),
        }
    }

    fn rest(&self, lw: u32, lh: u32) -> (u32, u32, u32, u32) {
        let (x0, y0, x1, y1) = shown(lw, lh);
        let (tw, th) = (self.across(x1 - x0), self.across(y1 - y0));
        match self.edge {
            Edge::Top => (x0, y0 + th, x1 - x0, y1 - y0 - th),
            Edge::Bottom => (x0, y0, x1 - x0, y1 - y0 - th),
            Edge::Left => (x0 + tw, y0, x1 - x0 - tw, y1 - y0),
            Edge::Right => (x0, y0, x1 - x0 - tw, y1 - y0),
        }
    }

    fn across(&self, n: u32) -> u32 {
        ((n as f32 * self.size).round() as u32).min(n)
    }
}

fn shown(lw: u32, lh: u32) -> (u32, u32, u32, u32) {
    let (ww, wh) = if (lw, lh) == (W, H) {
        (crate::cart::LABEL_W, crate::cart::LABEL_H)
    } else {
        (crate::cart::GB_LABEL_W, crate::cart::GB_LABEL_H)
    };
    let well = ww as f32 / wh as f32;
    if lw as f32 / lh as f32 > well {
        let vw = ((lh as f32 * well).round() as u32).min(lw);
        let x0 = (lw - vw) / 2;
        (x0, 0, x0 + vw, lh)
    } else {
        let vh = ((lw as f32 / well).round() as u32).min(lh);
        let y0 = (lh - vh) / 2;
        (0, y0, lw, y0 + vh)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn compose(
    logo: &[u8],
    w: u32,
    h: u32,
    lw: u32,
    lh: u32,
    deep: [u8; 3],
    pale: [u8; 3],
    band: Option<Band>,
) -> Vec<u8> {
    let mut ground = gradient(lw, lh, deep, pale);
    if let Some(b) = band {
        let (x0, y0, x1, y1) = b.strip(lw, lh);
        for y in y0..y1 {
            for x in x0..x1 {
                let i = ((y * lw + x) * 4) as usize;
                ground[i..i + 3].copy_from_slice(&b.colour);
            }
        }
    }
    let Some(bounds) = trim(logo, w, h) else {
        return ground;
    };
    let (tw, th) = (bounds.2 - bounds.0, bounds.3 - bounds.1);
    let Some(src) = IntSize::from_wh(tw, th)
        .and_then(|size| Pixmap::from_vec(premultiplied(logo, w, bounds), size))
    else {
        return ground;
    };
    let size = IntSize::from_wh(lw, lh).expect("the label's size is not zero");
    let mut dst = Pixmap::from_vec(ground, size).expect("the gradient is the label's size");
    let (ax, ay, aw, ah) = band.map_or((0, 0, lw, lh), |b| b.rest(lw, lh));
    let (scale, x, y) = placement(aw, ah, tw, th);
    let paint = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..PixmapPaint::default()
    };
    dst.draw_pixmap(
        0,
        0,
        src.as_ref(),
        &paint,
        Transform::from_row(scale, 0.0, 0.0, scale, x + ax as f32, y + ay as f32),
        None,
    );
    dst.take()
}

fn chunks(png: &[u8]) -> Option<Vec<(&[u8], std::ops::Range<usize>)>> {
    const SIGNATURE: usize = 8;
    if png.len() < SIGNATURE || png[..SIGNATURE] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] {
        return None;
    }
    let mut out = Vec::new();
    let mut at = SIGNATURE;
    while at + 12 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().ok()?) as usize;
        let end = at.checked_add(12)?.checked_add(len)?;
        if end > png.len() {
            return None;
        }
        out.push((&png[at + 4..at + 8], at..end));
        at = end;
    }
    Some(out)
}

fn is_tag(png: &[u8], kind: &[u8], at: &std::ops::Range<usize>) -> bool {
    kind == b"tEXt" && png[at.start + 8..at.end - 4].starts_with(TAG_KEY.as_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelTag {
    Logo([u8; 3]),
    Text([u8; 3]),
}

pub fn label_tag(png: &[u8]) -> Option<LabelTag> {
    let list = chunks(png)?;
    let (_, at) = list.iter().find(|(kind, at)| is_tag(png, kind, at))?;
    let text = std::str::from_utf8(png.get(at.start + 8 + TAG_KEY.len() + 1..at.end - 4)?).ok()?;
    let (kind, hex) = text.split_once(' ')?;
    let [_, r, g, b] = u32::from_str_radix(hex, 16).ok()?.to_be_bytes();
    match kind {
        "logo" if [r, g, b] == [0; 3] => None,
        "logo" => Some(LabelTag::Logo([r, g, b])),
        "text" => Some(LabelTag::Text([r, g, b])),
        _ => None,
    }
}

pub fn logo_tag(png: &[u8]) -> Option<[u8; 3]> {
    match label_tag(png)? {
        LabelTag::Logo(ground) => Some(ground),
        LabelTag::Text(_) => None,
    }
}

pub fn text_png(rgba: &[u8], w: u32, h: u32, colour: [u8; 3]) -> Vec<u8> {
    let [r, g, b] = colour;
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.add_text_chunk(TAG_KEY.to_string(), format!("text {r:02x}{g:02x}{b:02x}"))
            .expect("the tag is valid Latin-1");
        enc.write_header()
            .expect("a PNG header writes into a Vec")
            .write_image_data(rgba)
            .expect("PNG data writes into a Vec");
    }
    out
}

pub fn glance(png: &[u8]) -> Option<Vec<u8>> {
    let (rgba, w, h) = art::decode_png_bytes(png)?;
    let (w, h) = (w as usize, h as usize);
    let mut out = Vec::with_capacity(GLANCE_W * GLANCE_H * 3);
    for row in 0..GLANCE_H {
        let (top, bottom) = (row * h / GLANCE_H, ((row + 1) * h / GLANCE_H).max(row * h / GLANCE_H + 1));
        for col in 0..GLANCE_W {
            let (left, right) = (col * w / GLANCE_W, ((col + 1) * w / GLANCE_W).max(col * w / GLANCE_W + 1));
            let mut sum = [0u32; 3];
            for y in top..bottom {
                for p in rgba[(y * w + left) * 4..(y * w + right) * 4].chunks_exact(4) {
                    for (s, c) in sum.iter_mut().zip(p) {
                        *s += u32::from(*c);
                    }
                }
            }
            let cells = ((bottom - top) * (right - left)) as u32;
            out.extend(sum.map(|s| (s / cells) as u8));
        }
    }
    Some(out)
}

pub fn looks_same(a: &[u8], b: &[u8]) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(*y) <= GLANCE_TOLERANCE)
}

pub fn untagged(png: &[u8]) -> Vec<u8> {
    let Some(list) = chunks(png) else {
        return png.to_vec();
    };
    let mut out = png[..8].to_vec();
    for (kind, at) in &list {
        if !is_tag(png, kind, at) {
            out.extend_from_slice(&png[at.clone()]);
        }
    }
    out
}

pub struct Label {
    logo: Option<(Vec<u8>, u32, u32)>,
    lw: u32,
    lh: u32,
    deep: [u8; 3],
    band: Option<Band>,
    rgba: Vec<u8>,
}

impl Label {
    pub fn from_png(
        png: &[u8],
        deep: [u8; 3],
        platform: Platform,
        band: Option<Band>,
    ) -> Option<Label> {
        let (logo, w, h) = art::decode_png_bytes(png)?;
        let (lw, lh) = size(platform);
        let rgba = compose(&logo, w, h, lw, lh, deep, pale_for(deep), band);
        Some(Label {
            logo: Some((logo, w, h)),
            lw,
            lh,
            deep,
            band,
            rgba,
        })
    }

    pub fn full(png: &[u8], platform: Platform) -> Option<Label> {
        let (art, w, h) = art::decode_png_bytes(png)?;
        let (lw, lh) = size(platform);
        let mut rgba = art::cover_rgba(&art, w, h, lw, lh)?;
        for p in rgba.chunks_exact_mut(4) {
            let a = p[3] as u32;
            for c in &mut p[..3] {
                *c = ((*c as u32 * a + 127) / 255) as u8;
            }
            p[3] = 255;
        }
        Some(Label {
            logo: None,
            lw,
            lh,
            deep: [0; 3],
            band: None,
            rgba,
        })
    }

    pub fn covered(&self, w: u32, h: u32) -> Option<Vec<u8>> {
        art::cover_rgba(&self.rgba, self.lw, self.lh, w, h)
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn size(&self) -> (u32, u32) {
        (self.lw, self.lh)
    }

    pub fn deep(&self) -> Vec<u8> {
        self.deep.to_vec()
    }

    pub fn set_deep(&mut self, deep: &[u8]) {
        let Some((logo, w, h)) = &self.logo else {
            return;
        };
        self.deep = [deep[0], deep[1], deep[2]];
        self.rgba = compose(
            logo,
            *w,
            *h,
            self.lw,
            self.lh,
            self.deep,
            pale_for(self.deep),
            self.band,
        );
    }

    pub fn png(&self) -> Vec<u8> {
        let rgb: Vec<u8> = self
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.lw, self.lh);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            if self.logo.is_some() {
                let [r, g, b] = self.deep;
                enc.add_text_chunk(TAG_KEY.to_string(), format!("logo {r:02x}{g:02x}{b:02x}"))
                    .expect("the tag is valid Latin-1");
            }
            enc.write_header()
                .expect("a PNG header writes into a Vec")
                .write_image_data(&rgb)
                .expect("PNG data writes into a Vec");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INK: [u8; 4] = [10, 200, 30, 255];

    fn solid(w: u32, h: u32) -> Vec<u8> {
        INK.repeat((w * h) as usize)
    }

    fn at(rgba: &[u8], x: u32, y: u32) -> [u8; 3] {
        let i = ((y * W + x) * 4) as usize;
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    }

    fn ground_for(hue: u16) -> ([u8; 3], [u8; 3]) {
        stops(hue, BRIGHT + 1.0)
    }

    fn logo_rows(label: &[u8], hue: u16) -> (u32, u32) {
        let (deep, pale) = ground_for(hue);
        let ground = gradient(W, H, deep, pale);
        let rows: Vec<u32> = (0..H)
            .filter(|y| at(label, 640, *y) != at(&ground, 640, *y))
            .collect();
        (rows[0], *rows.last().expect("a logo row"))
    }

    fn near(a: [u8; 3], b: [u8; 3], by: i32) -> bool {
        (0..3).all(|c| (a[c] as i32 - b[c] as i32).abs() <= by)
    }

    #[test]
    fn a_yellow_ground_going_deep_turns_to_amber() {
        let bright = BRIGHT + 1.0;
        assert_eq!(stops(55, bright), stops(AMBER as u16, bright));
        assert_eq!(stops(OLIVE_LO, bright), stops(AMBER as u16, bright));
        assert_eq!(stops(OLIVE_HI, bright), stops(AMBER as u16, bright));

        for hue in [OLIVE_LO - 1, OLIVE_HI + 1, 35, 105, 235] {
            assert_eq!(
                stops(hue, bright).0,
                hue::hsl_to_rgb(hue as f32, 0.65, 0.24),
                "hue {hue} was turned"
            );
        }

        let dark = BRIGHT - 1.0;
        assert_eq!(stops(55, dark).0, hue::hsl_to_rgb(55.0, 0.55, 0.62));
    }

    #[test]
    fn the_corners_are_the_stops() {
        let (deep, pale) = ground_for(200);
        let g = gradient(W, H, deep, pale);
        assert_eq!(at(&g, 0, 0), deep);
        assert!(near(at(&g, W - 1, H - 1), pale, 1));
    }

    #[test]
    fn a_bright_logo_gets_a_deep_ground_and_a_dark_one_gets_a_pale_ground() {
        assert_eq!(
            stops(0, BRIGHT + 1.0),
            (
                hue::hsl_to_rgb(0.0, 0.65, 0.24),
                hue::hsl_to_rgb(0.0, 0.60, 0.46)
            )
        );
        assert_eq!(
            stops(0, BRIGHT - 1.0),
            (
                hue::hsl_to_rgb(0.0, 0.55, 0.62),
                hue::hsl_to_rgb(0.0, 0.50, 0.84)
            )
        );
    }

    #[test]
    fn a_picked_colour_keeps_its_hue_and_lifts_toward_the_light() {
        let deep = hue::hsl_to_rgb(210.0, 0.65, 0.24);
        let pale = pale_for(deep);
        let (dh, ds, dl) = hue::rgb_to_hsl(deep);
        let (ph, ps, pl) = hue::rgb_to_hsl(pale);
        assert!((ph - dh).abs() <= 1.0, "hue moved from {dh} to {ph}");
        assert!(pl > dl, "pale {pl} is not lighter than deep {dl}");
        assert!((pl - dl - 0.22).abs() <= 0.01, "lift was {}", pl - dl);
        assert!(ps < ds, "pale {ps} is not eased off {ds}");
    }

    #[test]
    fn a_tall_logo_fills_the_box_from_its_top_row() {
        let (deep, pale) = ground_for(30);
        let label = compose(&solid(100, 100), 100, 100, W, H, deep, pale, None);
        let (first, last) = logo_rows(&label, 30);
        let row = |r: f32| (r * H as f32 / 640.0).round() as u32;
        assert!(
            (row(70.0) - 1..=row(70.0) + 1).contains(&first),
            "first logo row {first}"
        );
        assert!(
            (row(493.0) - 2..=row(493.0)).contains(&last),
            "last logo row {last}"
        );
        assert!(near(at(&label, 640, 280), [INK[0], INK[1], INK[2]], 2));
    }

    #[test]
    fn a_wide_logo_centres_on_the_middle_row() {
        let (deep, pale) = ground_for(30);
        let label = compose(&solid(400, 40), 400, 40, W, H, deep, pale, None);
        let (first, last) = logo_rows(&label, 30);
        let mid = (first + last) as f32 / 2.0;
        assert!(
            (mid - H as f32 / 2.0).abs() <= 1.0,
            "logo centred on row {mid}"
        );
        assert!((90..=93).contains(&(last - first)), "rows {first}..{last}");
    }

    #[test]
    fn transparent_margins_and_a_faint_fringe_do_not_count() {
        let mut logo = vec![0u8; 200 * 200 * 4];
        for y in 50..150 {
            for x in 50..150 {
                let i = (y * 200 + x) * 4;
                logo[i..i + 4].copy_from_slice(&INK);
            }
        }
        logo[3] = 15;
        assert_eq!(trim(&logo, 200, 200), Some((50, 50, 150, 150)));
        let (deep, pale) = ground_for(30);
        assert!(
            compose(&logo, 200, 200, W, H, deep, pale, None)
                == compose(&solid(100, 100), 100, 100, W, H, deep, pale, None)
        );
    }

    #[test]
    fn an_invisible_logo_leaves_the_bare_gradient() {
        let (deep, pale) = ground_for(90);
        assert!(
            compose(&[0u8; 16 * 16 * 4], 16, 16, W, H, deep, pale, None)
                == gradient(W, H, deep, pale)
        );
    }

    #[test]
    fn the_png_is_rgb_at_the_gba_size() {
        let mut logo = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut logo, 8, 4);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&solid(8, 4))
                .unwrap();
        }
        let deep = hue::hsl_to_rgb(45.0, 0.65, 0.24);
        let label = Label::from_png(&logo, deep, Platform::Gba, None).expect("logo decodes");
        assert_eq!(label.deep(), deep.to_vec());
        let out = label.png();
        let reader = png::Decoder::new(std::io::Cursor::new(out))
            .read_info()
            .unwrap();
        let info = reader.info();
        assert_eq!(
            (info.width, info.height, info.color_type, info.bit_depth),
            (W, H, png::ColorType::Rgb, png::BitDepth::Eight)
        );
    }

    #[test]
    fn bytes_that_are_not_a_png_make_no_label() {
        assert!(Label::from_png(b"not a png", [101, 68, 21], Platform::Gba, None).is_none());
    }

    #[test]
    fn a_game_boy_label_is_the_shape_of_its_own_well() {
        assert_eq!(size(Platform::Gba), (W, H));
        assert_eq!(size(Platform::Gb), (GB_W, GB_H));
        assert_eq!(size(Platform::Gbc), (GB_W, GB_H));

        let want = crate::cart::GB_LABEL_W as f32 / crate::cart::GB_LABEL_H as f32;
        let got = GB_W as f32 / GB_H as f32;
        assert!((got - want).abs() < 0.01, "{got} is not the well's {want}");

        let mut logo = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut logo, 8, 4);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&solid(8, 4))
                .unwrap();
        }
        let deep = hue::hsl_to_rgb(200.0, 0.65, 0.24);
        for (platform, want) in [(Platform::Gba, (W, H)), (Platform::Gbc, (GB_W, GB_H))] {
            let label = Label::from_png(&logo, deep, platform, None).expect("logo decodes");
            let reader = png::Decoder::new(std::io::Cursor::new(label.png()))
                .read_info()
                .unwrap();
            let info = reader.info();
            assert_eq!((info.width, info.height), want, "{platform:?}");
        }
    }
    #[test]
    fn a_band_is_drawn_along_its_edge_and_the_logo_stays_off_it() {
        let (deep, pale) = ground_for(30);
        let band = Band {
            edge: Edge::Top,
            size: 0.2,
            colour: [200, 10, 20],
        };
        let label = compose(&solid(100, 100), 100, 100, W, H, deep, pale, Some(band));
        let strip = (H as f32 * 0.2).round() as u32;
        for y in 0..strip {
            for x in (0..W).step_by(37) {
                assert_eq!(at(&label, x, y), [200, 10, 20], "band at {x},{y}");
            }
        }
        let ink = [INK[0], INK[1], INK[2]];
        let first_ink = (0..H)
            .find(|y| (0..W).step_by(4).any(|x| near(at(&label, x, *y), ink, 2)))
            .expect("a logo row");
        assert!(
            first_ink >= strip,
            "logo starts at row {first_ink}, inside the band"
        );
    }

    #[test]
    fn set_deep_keeps_the_band() {
        let mut logo = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut logo, 8, 4);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header()
                .unwrap()
                .write_image_data(&solid(8, 4))
                .unwrap();
        }
        let band = Band {
            edge: Edge::Left,
            size: 0.1,
            colour: [5, 150, 60],
        };
        let mut label =
            Label::from_png(&logo, [60, 20, 90], Platform::Gba, Some(band)).expect("decodes");
        label.set_deep(&[20, 90, 60]);
        let mut reader = png::Decoder::new(std::io::Cursor::new(label.png()))
            .read_info()
            .unwrap();
        let mut rgb = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut rgb).unwrap();
        assert_eq!(
            &rgb[..3],
            &[5, 150, 60],
            "the band's corner after Label Color moved"
        );
    }

    fn png_of(rgba: &[u8], w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(rgba).unwrap();
        }
        out
    }

    fn rgb_of(png: &[u8]) -> (Vec<u8>, u32, u32) {
        let mut reader = png::Decoder::new(std::io::Cursor::new(png))
            .read_info()
            .unwrap();
        let mut rgb = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut rgb).unwrap();
        (rgb, info.width, info.height)
    }

    #[test]
    fn a_full_label_fills_the_platforms_label_edge_to_edge() {
        let ink = [INK[0], INK[1], INK[2]];
        for (platform, want) in [(Platform::Gba, (W, H)), (Platform::Gbc, (GB_W, GB_H))] {
            let label =
                Label::full(&png_of(&solid(600, 290), 600, 290), platform).expect("decodes");
            let (rgb, w, h) = rgb_of(&label.png());
            assert_eq!((w, h), want, "{platform:?}");
            for (x, y) in [
                (0, 0),
                (w - 1, 0),
                (0, h - 1),
                (w - 1, h - 1),
                (w / 2, h / 2),
            ] {
                let i = ((y * w + x) * 3) as usize;
                assert_eq!(&rgb[i..i + 3], &ink, "{platform:?} at {x},{y}");
            }
        }
    }

    #[test]
    fn a_full_label_is_cropped_to_fill_not_shrunk() {
        let mut art = solid(400, 400);
        for y in (0..90).chain(310..400) {
            for x in 0..400 {
                let i = ((y * 400 + x) * 4) as usize;
                art[i..i + 4].copy_from_slice(&[250, 0, 0, 255]);
            }
        }
        let label = Label::full(&png_of(&art, 400, 400), Platform::Gba).expect("decodes");
        let (rgb, _, _) = rgb_of(&label.png());
        assert!(
            rgb.chunks_exact(3).all(|p| p == [INK[0], INK[1], INK[2]]),
            "the cropped rows showed"
        );
    }

    #[test]
    fn a_ground_colour_leaves_a_full_label_alone() {
        let mut label =
            Label::full(&png_of(&solid(64, 32), 64, 32), Platform::Gba).expect("decodes");
        let before = label.png();
        label.set_deep(&[200, 30, 90]);
        assert_eq!(label.png(), before);
    }

    #[test]
    fn bytes_that_are_not_a_png_make_no_full_label() {
        assert!(Label::full(b"not a png", Platform::Gba).is_none());
    }

    #[test]
    fn a_band_parses_only_from_a_known_edge_and_a_real_size() {
        assert_eq!(
            Band::parse("top", 0.2, &[1, 2, 3]),
            Some(Band {
                edge: Edge::Top,
                size: 0.2,
                colour: [1, 2, 3]
            })
        );
        assert_eq!(Band::parse("", 0.2, &[1, 2, 3]), None);
        assert_eq!(Band::parse("middle", 0.2, &[1, 2, 3]), None);
        assert_eq!(Band::parse("top", 0.0, &[1, 2, 3]), None);
        assert_eq!(Band::parse("top", 0.2, &[1, 2]), None);
    }
}
