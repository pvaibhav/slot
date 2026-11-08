use std::path::Path;
use std::sync::OnceLock;

use slot_gfx::{Canvas, Draw, TexId, OUT_H, OUT_W};

use crate::art;

const DEFAULT_PNG: &[u8] = include_bytes!("../../../assets/background_default.png");
const BEZEL_GB_PNG: &[u8] = include_bytes!("../../../assets/bezel_gb.png");
const BEZEL_GBC_PNG: &[u8] = include_bytes!("../../../assets/bezel_gbc.png");
const BEZEL_GB_FIT_3X2_PNG: &[u8] = include_bytes!("../../../assets/bezel_gb_fit_3x2.png");
const BEZEL_GBC_FIT_3X2_PNG: &[u8] = include_bytes!("../../../assets/bezel_gbc_fit_3x2.png");
const BEZEL_GB_INTEGER_4X3_PNG: &[u8] = include_bytes!("../../../assets/bezel_gb_integer_4x3.png");
const BEZEL_GBC_INTEGER_4X3_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_gbc_integer_4x3.png");
const BEZEL_GB_FIT_4X3_PNG: &[u8] = include_bytes!("../../../assets/bezel_gb_fit_4x3.png");
const BEZEL_GBC_FIT_4X3_PNG: &[u8] = include_bytes!("../../../assets/bezel_gbc_fit_4x3.png");
// Harlequin's dmg-3x and gb-pocket-3x border art (GPL-3.0), cut to Authentic's window: its
// lens, the window's lip and shadow, the glass and the power light.
const BEZEL_AUTHENTIC_DMG_3X2_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_authentic_dmg_3x2.png");
const BEZEL_AUTHENTIC_DMG_4X3_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_authentic_dmg_4x3.png");
const BEZEL_AUTHENTIC_POCKET_3X2_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_authentic_pocket_3x2.png");
const BEZEL_AUTHENTIC_POCKET_4X3_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_authentic_pocket_4x3.png");
const BEZEL_AUTHENTIC_GBA_4X3_PNG: &[u8] =
    include_bytes!("../../../assets/bezel_authentic_gba_4x3.png");
const BLUR_PNG: &[u8] = include_bytes!("../../../assets/background_default_blur.png");

const SCRIM: f32 = 0.62;

pub fn wallpaper_face(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    art::cover(path, w, h)
}

pub fn bezel_face(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    art::cover(path, w, h)
}

pub fn game_bezel_face(path: &Path, w: u32, h: u32) -> Option<Vec<u8>> {
    let (src, sw, sh) = art::decode(path)?;
    panel_bezel(&src, (sw, sh), (w, h), false)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BezelArt {
    Default,
    Fit3x2,
    Integer4x3,
    Fit4x3,
    /// Authentic's own bezel around a whole-scale Game Boy or GBC picture: the Game Boy's grey
    /// lens, or the Pocket's black one. It carries the glass and the window's shadow.
    AuthenticGb {
        pocket: bool,
    },
    /// Authentic's own bezel around a whole-scale GBA picture on a 4:3 panel, with the lit
    /// panel in its window.
    AuthenticGba,
}

impl BezelArt {
    /// Whether the art carries Authentic's glass and window shadow itself.
    pub fn authentic(self) -> bool {
        matches!(self, BezelArt::AuthenticGb { .. } | BezelArt::AuthenticGba)
    }
}

pub fn default_game_bezel_face(colour: bool, kind: BezelArt, w: u32, h: u32) -> Option<Vec<u8>> {
    let png = match (kind, colour) {
        (BezelArt::Default, false) => BEZEL_GB_PNG,
        (BezelArt::Default, true) => BEZEL_GBC_PNG,
        (BezelArt::Fit3x2, false) => BEZEL_GB_FIT_3X2_PNG,
        (BezelArt::Fit3x2, true) => BEZEL_GBC_FIT_3X2_PNG,
        (BezelArt::Integer4x3, false) => BEZEL_GB_INTEGER_4X3_PNG,
        (BezelArt::Integer4x3, true) => BEZEL_GBC_INTEGER_4X3_PNG,
        (BezelArt::Fit4x3, false) => BEZEL_GB_FIT_4X3_PNG,
        (BezelArt::Fit4x3, true) => BEZEL_GBC_FIT_4X3_PNG,
        (BezelArt::AuthenticGb { pocket }, _) => match (pocket, w * 3 == h * 4) {
            (false, false) => BEZEL_AUTHENTIC_DMG_3X2_PNG,
            (false, true) => BEZEL_AUTHENTIC_DMG_4X3_PNG,
            (true, false) => BEZEL_AUTHENTIC_POCKET_3X2_PNG,
            (true, true) => BEZEL_AUTHENTIC_POCKET_4X3_PNG,
        },
        (BezelArt::AuthenticGba, _) => BEZEL_AUTHENTIC_GBA_4X3_PNG,
    };
    let (src, sw, sh) = art::decode_png_bytes(png)?;
    match kind {
        BezelArt::Default => panel_bezel(&src, (sw, sh), (w, h), true),
        _ => art::cover_rgba(&src, sw, sh, w, h),
    }
}

/// Takes the glass's reflection out of Authentic's own bezel art and keeps its shade. Over
/// the screen the art is the white sheen laid on the black shade, so a pixel lets through
/// `(1 - shade)(1 - sheen)` of the screen and adds `sheen` of white: the sheen is the white
/// it adds, and the shade what is left of the light it holds back. The lens's lip, just
/// inside the solid bezel, is left as drawn.
pub fn clear_glass_reflection(rgba: &mut [u8], w: u32, h: u32) {
    let (w, h) = (w as usize, h as usize);
    if rgba.len() < w * h * 4 {
        return;
    }
    let solid: Vec<bool> = rgba.chunks_exact(4).map(|px| px[3] == 255).collect();
    let near_solid = |x: usize, y: usize| {
        (y.saturating_sub(2)..(y + 3).min(h))
            .any(|y| (x.saturating_sub(2)..(x + 3).min(w)).any(|x| solid[y * w + x]))
    };
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        if px[3] == 0 || solid[i] || near_solid(i % w, i / w) {
            continue;
        }
        let a = px[3] as f32 / 255.0;
        let sheen = px[..3].iter().map(|&c| c as f32).sum::<f32>() / (3.0 * 255.0) * a;
        let shade = (1.0 - (1.0 - a) / (1.0 - sheen).max(1e-3)).max(0.0);
        px.copy_from_slice(&[0, 0, 0, (shade * 255.0).round() as u8]);
    }
}

/// How dark the bezel's shadow falls on the screen under it, where it is deepest.
const BEZEL_SHADOW: f32 = 0.6;

/// Lays the bezel's shadow on the screen in its window, as dmg-3x's border casts it in room
/// light from the top right: the bezel's own outline moved down and to the left by a game
/// pixel and a half and softened, so it shades the window's top and right edges.
pub fn cast_screen_shadow(rgba: &mut [u8], w: u32, h: u32) {
    let (w, h) = (w as usize, h as usize);
    if w == 0 || h == 0 || rgba.len() < w * h * 4 {
        return;
    }
    let reach = (h as f32 * 1.5 / 144.0).round().max(1.0) as usize;
    // Past the image's edges the bezel carries on.
    let mut shade: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w + reach, (i / w).checked_sub(reach));
            match y {
                Some(y) if x < w => rgba[(y * w + x) * 4 + 3] as f32 / 255.0,
                _ => 1.0,
            }
        })
        .collect();
    for _ in 0..3 {
        soften(&mut shade, w, h, (reach / 2).max(1));
    }
    for (px, s) in rgba.chunks_exact_mut(4).zip(shade) {
        let a = px[3] as f32 / 255.0;
        let s = BEZEL_SHADOW * s * (1.0 - a);
        let out = a + s;
        if out > 0.0 {
            for c in &mut px[..3] {
                *c = (*c as f32 * a / out).round() as u8;
            }
        }
        px[3] = (out * 255.0).round() as u8;
    }
}

/// A box blur of radius `r` across and then down, holding the edges.
fn soften(v: &mut [f32], w: usize, h: usize, r: usize) {
    let mut line = Vec::new();
    for (len, step, count, stride) in [(w, 1, h, w), (h, w, w, 1)] {
        for n in 0..count {
            let at = |i: usize| n * stride + i * step;
            line.clear();
            line.extend((0..len).map(|i| v[at(i)]));
            let mut sum: f32 =
                (0..=r).map(|i| line[i.min(len - 1)]).sum::<f32>() + line[0] * r as f32;
            for i in 0..len {
                v[at(i)] = sum / (2 * r + 1) as f32;
                sum += line[(i + r + 1).min(len - 1)] - line[i.saturating_sub(r)];
            }
        }
    }
}

const BEZEL_GRAIN_ROWS: usize = 10;

fn panel_bezel(
    src: &[u8],
    (sw, sh): (u32, u32),
    (w, h): (u32, u32),
    extend: bool,
) -> Option<Vec<u8>> {
    if !slot_gfx::framed((w, h)) || sw * 3 == sh * 4 {
        return art::cover_rgba(src, sw, sh, w, h);
    }
    let game_h = slot_gfx::screen_rect((w, h)).3 as usize;
    let inner = art::cover_rgba(src, sw, sh, w, game_h as u32)?;
    let (h, row) = (h as usize, w as usize * 4);
    let gap = h - game_h;
    let top = gap - gap / 2;
    let mut out = vec![0u8; row * h];
    out[top * row..(top + game_h) * row].copy_from_slice(&inner);
    if !extend {
        return Some(out);
    }
    let ping = |d: usize| {
        let p = d % (2 * BEZEL_GRAIN_ROWS);
        p.min(2 * BEZEL_GRAIN_ROWS - 1 - p)
    };
    for d in 0..top {
        let from = ping(d) * row;
        out[(top - 1 - d) * row..][..row].copy_from_slice(&inner[from..from + row]);
    }
    for d in 0..h - top - game_h {
        let from = (game_h - 1 - ping(d)) * row;
        out[(top + game_h + d) * row..][..row].copy_from_slice(&inner[from..from + row]);
    }
    Some(out)
}

/// The carousel ground, scaled to cover the whole screen, including behind the footer.
pub fn default_background() -> Option<&'static [u8]> {
    static P: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    P.get_or_init(|| art::cover_bytes(DEFAULT_PNG, OUT_W, OUT_H))
        .as_deref()
}

/// The blurred carousel ground, scaled to cover the whole screen the same way as the default.
pub fn blur_background() -> Option<&'static [u8]> {
    static P: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    P.get_or_init(|| art::cover_bytes(BLUR_PNG, OUT_W, OUT_H))
        .as_deref()
}

pub fn reference_ground_covers((w, h): (u32, u32)) -> bool {
    h == OUT_H && w <= OUT_W
}

pub fn default_background_at(w: u32, h: u32) -> Option<Vec<u8>> {
    art::cover_bytes(DEFAULT_PNG, w, h)
}

pub fn blur_background_at(w: u32, h: u32) -> Option<Vec<u8>> {
    art::cover_bytes(BLUR_PNG, w, h)
}

pub fn draw_ground(canvas: Canvas, face: Option<(TexId, u32)>, out: &mut Vec<Draw>) {
    let Some(face) = face else {
        return;
    };
    out.push(ground(canvas, face));
}

pub fn draw_backdrop(canvas: Canvas, face: Option<(TexId, u32)>, out: &mut Vec<Draw>) {
    let Some(face) = face else {
        return;
    };
    out.push(ground(canvas, face));
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: canvas.w,
        h: canvas.h,
        colour: [0.0, 0.0, 0.0, SCRIM],
    });
}

fn ground(canvas: Canvas, (tex, tex_w): (TexId, u32)) -> Draw {
    Draw::Part {
        x: 0.0,
        y: 0.0,
        w: canvas.w,
        h: canvas.h,
        tex,
        alpha: 1.0,
        uv: canvas.cover(tex_w as f32),
    }
}
