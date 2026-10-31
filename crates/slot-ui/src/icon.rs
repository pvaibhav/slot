use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use fontdue::{Font, FontSettings};

use crate::CartFace;

const SYMBOLS_TTF: &[u8] = include_bytes!("../assets/SymbolsNerdFontMono-Regular.ttf");

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    Volume,
    VolumeZero,
    VolumeMuted,
    Brightness,
    BlueLight,
    FastForward,
    FastForwardLatched,
    Rewind,
    Alert,
    Charging,
    Headphones,
    HeadphonesMuted,
    Camera,
    Sunlight,
    Backspace,
    Return,
    Shift,
    Space,
}

impl Icon {
    pub const ALL: [Icon; 18] = [
        Icon::Volume,
        Icon::VolumeZero,
        Icon::VolumeMuted,
        Icon::Brightness,
        Icon::BlueLight,
        Icon::FastForward,
        Icon::FastForwardLatched,
        Icon::Rewind,
        Icon::Alert,
        Icon::Charging,
        Icon::Headphones,
        Icon::HeadphonesMuted,
        Icon::Camera,
        Icon::Sunlight,
        Icon::Backspace,
        Icon::Return,
        Icon::Shift,
        Icon::Space,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn glyph(self) -> char {
        match self {
            Icon::Volume => '\u{f028}',
            Icon::VolumeZero => '\u{f026}',
            Icon::VolumeMuted => '\u{f075f}',
            Icon::Brightness => '\u{f185}',
            Icon::BlueLight => '\u{f186}',
            Icon::FastForward => '\u{f06d2}',
            Icon::FastForwardLatched => '\u{f0211}',
            Icon::Rewind => '\u{f04a}',
            Icon::Alert => '\u{f0026}',
            Icon::Charging => '\u{f0e7}',
            Icon::Headphones => '\u{f025}',
            Icon::HeadphonesMuted => '\u{f07ce}',
            Icon::Camera => '\u{f030}',
            Icon::Sunlight => '\u{f0a72}',
            Icon::Backspace => '\u{f0b5c}',
            Icon::Return => '\u{f0311}',
            Icon::Shift => '\u{f0636}',
            Icon::Space => '\u{f1050}',
        }
    }
}

const HALO: [u8; 3] = [0x08, 0x08, 0x0a];
pub const HALO_PX: u32 = 1;

pub const WARNING_INK: [u8; 3] = [0xff, 0xb8, 0x2e];

pub fn haloed(cov: &[u8], cw: u32, ch: u32, colour: [u8; 3]) -> CartFace {
    haloed_with(cov, cw, ch, |_, _| colour)
}

pub fn haloed_with(
    cov: &[u8],
    cw: u32,
    ch: u32,
    colour_at: impl Fn(isize, isize) -> [u8; 3],
) -> CartFace {
    let pad = HALO_PX as usize;
    let (cw, ch) = (cw as usize, ch as usize);
    let (w, h) = (cw + 2 * pad, ch + 2 * pad);
    let at = |x: isize, y: isize| -> u8 {
        if x < 0 || y < 0 || x >= cw as isize || y >= ch as isize {
            0
        } else {
            cov[y as usize * cw + x as usize]
        }
    };

    let mut rgba = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (gx, gy) = (x as isize - pad as isize, y as isize - pad as isize);
            let ink = at(gx, gy);
            let mut halo = 0u8;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    halo = halo.max(at(gx + dx, gy + dy));
                }
            }
            if ink > 0 {
                let colour = colour_at(gx, gy);
                let a = ink as u32;
                let inv = 255 - a;
                let mix = |c: u8, s: u8| ((c as u32 * a + s as u32 * inv) / 255) as u8;
                rgba.extend_from_slice(&[
                    mix(colour[0], HALO[0]),
                    mix(colour[1], HALO[1]),
                    mix(colour[2], HALO[2]),
                    ink.max(halo),
                ]);
            } else {
                rgba.extend_from_slice(&[HALO[0], HALO[1], HALO[2], halo]);
            }
        }
    }
    CartFace {
        rgba,
        w: w as u32,
        h: h as u32,
    }
}

pub fn icon_face(icon: Icon, px: f32, colour: [u8; 3]) -> CartFace {
    let Some(r) = raster(icon, px) else {
        return CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    haloed(&r.cov, r.w, r.h, colour)
}

pub fn icon_box(px: f32) -> (u32, u32) {
    match raster(Icon::Volume, px) {
        Some(r) => (r.w + 2 * HALO_PX, r.h + 2 * HALO_PX),
        None => (0, 0),
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Badge {
    Link,
    LinkBroken,
}

impl Badge {
    pub fn glyph(self) -> char {
        match self {
            Badge::Link => '\u{f0339}',
            Badge::LinkBroken => '\u{f033a}',
        }
    }
}

const HEART: [&str; 8] = [
    ".##...##.",
    "####.####",
    "#########",
    "#########",
    ".#######.",
    "..#####..",
    "...###...",
    "....#....",
];

pub fn heart_face(scale: u32, colour: [u8; 3]) -> CartFace {
    let (gw, gh) = (HEART[0].len() as i32, HEART.len() as i32);
    let ink = |x: i32, y: i32| {
        (0..gw).contains(&x)
            && (0..gh).contains(&y)
            && HEART[y as usize].as_bytes()[x as usize] == b'#'
    };
    let (w, h) = ((gw + 2) as u32 * scale, (gh + 2) as u32 * scale);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for py in 0..h {
        for px in 0..w {
            let (x, y) = ((px / scale) as i32 - 1, (py / scale) as i32 - 1);
            let rim = [(0, -1), (0, 1), (-1, 0), (1, 0)]
                .iter()
                .any(|(dx, dy)| ink(x + dx, y + dy));
            let c = match (ink(x, y), rim) {
                (true, _) => [colour[0], colour[1], colour[2], 255],
                (false, true) => [HALO[0], HALO[1], HALO[2], 255],
                _ => continue,
            };
            let o = ((py * w + px) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&c);
        }
    }
    CartFace { rgba, w, h }
}

pub fn badge_face(badge: Badge, px: f32, colour: [u8; 3]) -> CartFace {
    let Some(font) = symbols_font() else {
        return CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let f = frame(font, px);
    let (m, cov) = font.rasterize(badge.glyph(), px);
    let x0 = m.xmin - f.left;
    let y0 = f.top - (m.ymin + m.height as i32);
    let mut out = vec![0u8; (f.w * f.h) as usize];
    for gy in 0..m.height {
        for gx in 0..m.width {
            let (dx, dy) = (x0 + gx as i32, y0 + gy as i32);
            if dx < 0 || dy < 0 || dx >= f.w as i32 || dy >= f.h as i32 {
                continue;
            }
            out[(dy as u32 * f.w + dx as u32) as usize] = cov[gy * m.width + gx];
        }
    }
    haloed(&out, f.w, f.h, colour)
}

struct Raster {
    cov: Vec<u8>,
    w: u32,
    h: u32,
}

type Cache = Mutex<HashMap<(Icon, u32), Arc<Raster>>>;

pub(crate) fn icon_coverage(icon: Icon, px: f32) -> Option<(Vec<u8>, u32, u32)> {
    raster(icon, px).map(|r| (r.cov.clone(), r.w, r.h))
}

fn raster(icon: Icon, px: f32) -> Option<Arc<Raster>> {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(Mutex::default);
    let key = (icon, px.to_bits());
    let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(r) = cache.get(&key) {
        return Some(r.clone());
    }
    let r = Arc::new(rasterise(icon, px)?);
    cache.insert(key, r.clone());
    Some(r)
}

fn rasterise(icon: Icon, px: f32) -> Option<Raster> {
    let font = symbols_font()?;
    let f = frame(font, px);
    let (m, cov) = font.rasterize(icon.glyph(), px);
    let x0 = m.xmin - f.left;
    let y0 = f.top - (m.ymin + m.height as i32);

    let mut out = vec![0u8; (f.w * f.h) as usize];
    for gy in 0..m.height {
        for gx in 0..m.width {
            let (dx, dy) = ((x0 + gx as i32) as u32, (y0 + gy as i32) as u32);
            out[(dy * f.w + dx) as usize] = cov[gy * m.width + gx];
        }
    }
    Some(Raster {
        cov: out,
        w: f.w,
        h: f.h,
    })
}

struct Frame {
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

fn frame(font: &Font, px: f32) -> Frame {
    let (mut left, mut right, mut top, mut bottom) = (i32::MAX, i32::MIN, i32::MIN, i32::MAX);
    for icon in Icon::ALL {
        let m = font.metrics(icon.glyph(), px);
        left = left.min(m.xmin);
        right = right.max((m.xmin + m.width as i32).max(m.advance_width.ceil() as i32));
        top = top.max(m.ymin + m.height as i32);
        bottom = bottom.min(m.ymin);
    }
    Frame {
        w: (right - left).max(1) as u32,
        h: (top - bottom).max(1) as u32,
        left,
        top,
    }
}

pub fn warm_fonts() {
    let _ = symbols_font();
    let _ = crate::text::label_font();
}

pub(crate) fn symbols_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(SYMBOLS_TTF, FontSettings::default()).ok())
        .as_ref()
}
