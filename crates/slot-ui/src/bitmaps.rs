use std::sync::OnceLock;

use slot_gfx::OUT_W;

use crate::art::decode_png_bytes;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Mould {
    Gba,
    Notched,
    Rounded,
    #[cfg(feature = "nds")]
    Ds,
}

pub const GBA_CART_W: u32 = 360;
pub const GBA_CART_H: u32 = 212;
pub const GB_CART_W: u32 = 312;
pub const GB_CART_H: u32 = 337;
pub const BOARD_W: u32 = GBA_CART_W;
pub const BOARD_H: u32 = GBA_CART_H;
pub const SOCKET_W: u32 = 64;
pub const SOCKET_H: u32 = 46;
pub const CHIP_W: u32 = 59;
pub const CHIP_H: u32 = 41;
pub const GB_CHIP_W: u32 = 59;
pub const GB_CHIP_H: u32 = 92;
pub const SLOT_W: u32 = OUT_W;
pub const SLOT_H: u32 = 58;
pub const PORT_W: u32 = OUT_W;
pub const PORT_H: u32 = SLOT_H;
pub const PLUG_W: u32 = 92;
pub const PLUG_H: u32 = 244;
pub const ADAPTER_W: u32 = 290;
pub const ADAPTER_H: u32 = 180;
pub const STICKER_W: u32 = 660;
pub const STICKER_H: u32 = 228;
pub const SEAL_W: u32 = 36;
pub const SEAL_H: u32 = 36;

const GBA_CART_PNG: &[u8] = include_bytes!("../../../assets/gba_cart.png");
const GB_CART_PNG: &[u8] = include_bytes!("../../../assets/gb_cart.png");
const GBC_CART_PNG: &[u8] = include_bytes!("../../../assets/gbc_cart.png");
const GBA_INTERIOR_PNG: &[u8] = include_bytes!("../../../assets/gba_interior.png");
const GB_INTERIOR_PNG: &[u8] = include_bytes!("../../../assets/gb_interior.png");
const GBC_INTERIOR_PNG: &[u8] = include_bytes!("../../../assets/gbc_interior.png");
const GB_BOARD_PNG: &[u8] = include_bytes!("../../../assets/gb_board.png");
const GB_BOARD_EMPTY_PNG: &[u8] = include_bytes!("../../../assets/gb_board_empty.png");
const GB_CHIP_PNG: &[u8] = include_bytes!("../../../assets/gb_chip.png");
const BOARD_PNG: &[u8] = include_bytes!("../../../assets/board.png");
const UPDATE_CART_PNG: &[u8] = include_bytes!("../../../assets/update_cart.png");
const BASEOS_CART_PNG: &[u8] = include_bytes!("../../../assets/baseos_cart.png");
const SOCKET_PNG: &[u8] = include_bytes!("../../../assets/socket.png");
const CHIP_PNG: &[u8] = include_bytes!("../../../assets/chip.png");
const SLOT_BACK_DARK_PNG: &[u8] = include_bytes!("../../../assets/slot_back_dark.png");
const SLOT_BACK_LIGHT_PNG: &[u8] = include_bytes!("../../../assets/slot_back_light.png");
const SLOT_FRONT_DARK_PNG: &[u8] = include_bytes!("../../../assets/slot_front_dark.png");
const SLOT_FRONT_LIGHT_PNG: &[u8] = include_bytes!("../../../assets/slot_front_light.png");
const SLOT_PLUGGED_DARK_PNG: &[u8] = include_bytes!("../../../assets/slot_plugged_dark.png");
const SLOT_PLUGGED_LIGHT_PNG: &[u8] = include_bytes!("../../../assets/slot_plugged_light.png");
const PORT_DARK_PNG: &[u8] = include_bytes!("../../../assets/link_port_dark.png");
const PORT_LIGHT_PNG: &[u8] = include_bytes!("../../../assets/link_port_light.png");
const PLUG_HOST_PNG: &[u8] = include_bytes!("../../../assets/link_plug_host.png");
const PLUG_JOIN_PNG: &[u8] = include_bytes!("../../../assets/link_plug_join.png");
const ADAPTER_PNG: &[u8] = include_bytes!("../../../assets/link_adapter.png");
const STICKER_PNG: &[u8] = include_bytes!("../../../assets/sticker.png");
const SEAL_PNG: &[u8] = include_bytes!("../../../assets/seal.png");

fn load(bytes: &'static [u8], w: u32, h: u32, slot: &'static OnceLock<Vec<u8>>) -> &'static [u8] {
    slot.get_or_init(|| {
        let (mut rgba, sw, sh) = decode_png_bytes(bytes).expect("a template png decodes");
        if sh == h && sw > w {
            rgba = crop_centre(&rgba, sw, sh, w);
        } else if (sw, sh) != (w, h) {
            rgba = scale(&rgba, sw, sh, w, h);
        }
        // Transparent pixels are often stored as white. The cart is sampled with a linear
        // filter, which would pull that white in along every edge.
        bleed(&mut rgba, w, h);
        rgba
    })
}

/// Copy an opaque neighbour's colour into each transparent pixel, leaving its alpha at 0.
fn bleed(rgba: &mut [u8], w: u32, h: u32) {
    let src = rgba.to_vec();
    let (w, h) = (w as i32, h as i32);
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            if src[i + 3] != 0 {
                continue;
            }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h {
                    continue;
                }
                let n = ((ny * w + nx) * 4) as usize;
                if src[n + 3] == 0 {
                    continue;
                }
                rgba[i..i + 3].copy_from_slice(&src[n..n + 3]);
                break;
            }
        }
    }
}

fn crop_centre(src: &[u8], sw: u32, sh: u32, w: u32) -> Vec<u8> {
    let left = ((sw - w) / 2) as usize;
    let mut out = Vec::with_capacity((w * sh * 4) as usize);
    for row in src.chunks_exact(sw as usize * 4) {
        out.extend_from_slice(&row[left * 4..(left + w as usize) * 4]);
    }
    out
}

fn scale(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let sy = y * sh / h;
        for x in 0..w {
            let sx = x * sw / w;
            let from = ((sy * sw + sx) * 4) as usize;
            let to = ((y * w + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
    out
}

/// The RetroAchievements seal from `assets/seal.png`, its shadow included.
pub fn seal() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SEAL_PNG, SEAL_W, SEAL_H, &P)
}

pub fn gba_cart() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GBA_CART_PNG, GBA_CART_W, GBA_CART_H, &P)
}

pub fn gb_cart() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GB_CART_PNG, GB_CART_W, GB_CART_H, &P)
}

pub fn gbc_cart() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GBC_CART_PNG, GB_CART_W, GB_CART_H, &P)
}

/// The inside of a GBA cart, from `assets/gba_interior.png`. Same size as the cart and
/// the board, so the carousel can stack the three 1:1. Its colour is set by the game, with the
/// same overlay blend the shells use.
pub fn gba_interior() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GBA_INTERIOR_PNG, BOARD_W, BOARD_H, &P)
}

/// The back of a Game Boy pak, from `assets/gb_interior.png`.
pub fn gb_interior() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GB_INTERIOR_PNG, GB_CART_W, GB_CART_H, &P)
}

/// The back of a Game Boy Color pak, from `assets/gbc_interior.png`.
pub fn gbc_interior() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GBC_INTERIOR_PNG, GB_CART_W, GB_CART_H, &P)
}

/// The Game Boy PCB, from `assets/gb_board.png`, drawn between the pak's back and front plastic.
pub fn gb_board() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GB_BOARD_PNG, GB_CART_W, GB_CART_H, &P)
}

pub fn gb_board_empty() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GB_BOARD_EMPTY_PNG, GB_CART_W, GB_CART_H, &P)
}

pub fn gb_chip() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(GB_CHIP_PNG, GB_CHIP_W, GB_CHIP_H, &P)
}

pub fn update_cart() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(UPDATE_CART_PNG, BOARD_W, BOARD_H, &P)
}

pub fn baseos_cart() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(BASEOS_CART_PNG, BOARD_W, BOARD_H, &P)
}

pub fn board() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(BOARD_PNG, BOARD_W, BOARD_H, &P)
}

pub fn socket() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SOCKET_PNG, SOCKET_W, SOCKET_H, &P)
}

pub fn chip() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(CHIP_PNG, CHIP_W, CHIP_H, &P)
}

#[cfg(feature = "nds")]
const MOUTH_SPAN: (usize, usize) = (232, 488);
#[cfg(feature = "nds")]
const MOUTH_CUTS: [(usize, usize); 2] = [(276, 329), (391, 445)];
#[cfg(feature = "nds")]
pub const DS_MOUTH_W: u32 = 149;

#[cfg(feature = "nds")]
fn narrowed(src: &[u8]) -> Vec<u8> {
    let w = SLOT_W as usize;
    let (left, right) = MOUTH_SPAN;
    let kept: Vec<usize> = (left..right)
        .filter(|x| !MOUTH_CUTS.iter().any(|(a, b)| (*a..*b).contains(x)))
        .collect();
    let start = (left + right - kept.len()) / 2;
    let gap = start - left;
    let mut out = src.to_vec();
    for y in 0..SLOT_H as usize {
        let px = |x: usize| (y * w + x) * 4;
        for x in left..start {
            let from = px(x - gap);
            out[px(x)..px(x) + 4].copy_from_slice(&src[from..from + 4]);
        }
        for (i, &sx) in kept.iter().enumerate() {
            let from = px(sx);
            out[px(start + i)..px(start + i) + 4].copy_from_slice(&src[from..from + 4]);
        }
        for x in start + kept.len()..right {
            let from = px(x + gap);
            out[px(x)..px(x) + 4].copy_from_slice(&src[from..from + 4]);
        }
    }
    out
}

#[cfg(feature = "nds")]
pub fn ds_slot_back() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    P.get_or_init(|| narrowed(slot_back()))
}

#[cfg(feature = "nds")]
pub fn ds_slot_front() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    P.get_or_init(|| narrowed(slot_front()))
}

#[cfg(feature = "nds")]
pub fn ds_slot_back_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    P.get_or_init(|| narrowed(slot_back_light()))
}

#[cfg(feature = "nds")]
pub fn ds_slot_front_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    P.get_or_init(|| narrowed(slot_front_light()))
}

pub fn slot_back() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_BACK_DARK_PNG, SLOT_W, SLOT_H, &P)
}

pub fn slot_back_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_BACK_LIGHT_PNG, SLOT_W, SLOT_H, &P)
}

pub fn slot_front() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_FRONT_DARK_PNG, SLOT_W, SLOT_H, &P)
}

pub fn slot_front_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_FRONT_LIGHT_PNG, SLOT_W, SLOT_H, &P)
}

pub fn slot_plugged() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_PLUGGED_DARK_PNG, SLOT_W, SLOT_H, &P)
}

pub fn slot_plugged_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(SLOT_PLUGGED_LIGHT_PNG, SLOT_W, SLOT_H, &P)
}

pub fn link_port() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(PORT_DARK_PNG, PORT_W, PORT_H, &P)
}

pub fn link_port_light() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(PORT_LIGHT_PNG, PORT_W, PORT_H, &P)
}

pub fn link_plug_host() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(PLUG_HOST_PNG, PLUG_W, PLUG_H, &P)
}

pub fn link_plug_join() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(PLUG_JOIN_PNG, PLUG_W, PLUG_H, &P)
}

pub fn link_adapter() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(ADAPTER_PNG, ADAPTER_W, ADAPTER_H, &P)
}

pub fn sticker() -> &'static [u8] {
    static P: OnceLock<Vec<u8>> = OnceLock::new();
    load(STICKER_PNG, STICKER_W, STICKER_H, &P)
}

// Shells are hand drawn. There is no Game Boy clear file yet, so a clear Game Boy pak uses
// the neutral shell at 80%.
const GBA_SHELL_PNG: &[u8] = include_bytes!("../../../assets/gba_neutral.png");
const GB_SHELL_PNG: &[u8] = include_bytes!("../../../assets/gb_neutral.png");
const GBC_SHELL_PNG: &[u8] = include_bytes!("../../../assets/gbc_neutral.png");
const GBA_CLEAR_PNG: &[u8] = include_bytes!("../../../assets/gba_clear.png");
const GB_CLEAR_PNG: &[u8] = include_bytes!("../../../assets/gb_neutral.png");
const GBC_CLEAR_PNG: &[u8] = include_bytes!("../../../assets/gbc_clear.png");

/// The neutral shell asset for a mould, opaque or the separate clear one. The game's colour is
/// blended onto it by the caller.
#[cfg(feature = "nds")]
pub const DS_CART_W: u32 = 200;
#[cfg(feature = "nds")]
pub const DS_CART_H: u32 = 212;
#[cfg(feature = "nds")]
const DS_CORNER: f32 = 12.0;
#[cfg(feature = "nds")]
const DS_RIDGE_TOP: u32 = 5;
#[cfg(feature = "nds")]
const DS_RIDGE_BOTTOM: u32 = 19;

#[cfg(feature = "nds")]
pub fn ds_cart_mask() -> &'static [u8] {
    static MASK: OnceLock<Vec<u8>> = OnceLock::new();
    MASK.get_or_init(|| {
        let (w, h) = (DS_CART_W as f32, DS_CART_H as f32);
        let mut out = Vec::with_capacity((DS_CART_W * DS_CART_H) as usize);
        for y in 0..DS_CART_H {
            for x in 0..DS_CART_W {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let cx = px.clamp(DS_CORNER, w - DS_CORNER);
                let cy = py.clamp(DS_CORNER, h - DS_CORNER);
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
                let cover = (DS_CORNER + 0.5 - d).clamp(0.0, 1.0);
                out.push((cover * 255.0).round() as u8);
            }
        }
        out
    })
}

#[cfg(feature = "nds")]
pub fn ds_cart() -> &'static [u8] {
    static PAINT: OnceLock<Vec<u8>> = OnceLock::new();
    PAINT.get_or_init(|| {
        let mask = ds_cart_mask();
        let mut out = Vec::with_capacity(mask.len() * 4);
        for (i, cover) in mask.iter().enumerate() {
            let (x, y) = (i as u32 % DS_CART_W, i as u32 / DS_CART_W);
            let ridge = (DS_RIDGE_TOP..DS_RIDGE_BOTTOM).contains(&y)
                && (16..DS_CART_W - 16).contains(&x)
                && (y - DS_RIDGE_TOP) % 4 < 2;
            let grey = if ridge { 104 } else { 128 };
            out.extend_from_slice(&[grey, grey, grey, *cover]);
        }
        out
    })
}

pub fn shell_asset(mould: Mould, clear: bool) -> &'static [u8] {
    static GBA: OnceLock<Vec<u8>> = OnceLock::new();
    static GB: OnceLock<Vec<u8>> = OnceLock::new();
    static GBC: OnceLock<Vec<u8>> = OnceLock::new();
    static GBA_CLEAR: OnceLock<Vec<u8>> = OnceLock::new();
    static GB_CLEAR: OnceLock<Vec<u8>> = OnceLock::new();
    static GBC_CLEAR: OnceLock<Vec<u8>> = OnceLock::new();
    let (bytes, w, h, slot) = match (mould, clear) {
        (Mould::Gba, false) => (GBA_SHELL_PNG, GBA_CART_W, GBA_CART_H, &GBA),
        (Mould::Notched, false) => (GB_SHELL_PNG, GB_CART_W, GB_CART_H, &GB),
        (Mould::Rounded, false) => (GBC_SHELL_PNG, GB_CART_W, GB_CART_H, &GBC),
        (Mould::Gba, true) => (GBA_CLEAR_PNG, GBA_CART_W, GBA_CART_H, &GBA_CLEAR),
        (Mould::Notched, true) => (GB_CLEAR_PNG, GB_CART_W, GB_CART_H, &GB_CLEAR),
        (Mould::Rounded, true) => (GBC_CLEAR_PNG, GB_CART_W, GB_CART_H, &GBC_CLEAR),
        #[cfg(feature = "nds")]
        (Mould::Ds, _) => return ds_cart(),
    };
    load(bytes, w, h, slot)
}

/// Footer buttons. Each file is already the footer height.
#[derive(Copy, Clone)]
pub(crate) enum ButtonLegend {
    A,
    B,
    Left,
    Right,
    Select,
    X,
    Y,
    L1,
    R1,
    Start,
    Menu,
}

const BUTTON_LEGEND_PNG: [&[u8]; 11] = [
    include_bytes!("../../../assets/buttonlegend_round_a.png"),
    include_bytes!("../../../assets/buttonlegend_round_b.png"),
    include_bytes!("../../../assets/buttonlegend_dpad_left.png"),
    include_bytes!("../../../assets/buttonlegend_dpad_right.png"),
    include_bytes!("../../../assets/buttonlegend_long_select.png"),
    include_bytes!("../../../assets/buttonlegend_round_x.png"),
    include_bytes!("../../../assets/buttonlegend_round_y.png"),
    include_bytes!("../../../assets/buttonlegend_square_l1.png"),
    include_bytes!("../../../assets/buttonlegend_square_r1.png"),
    include_bytes!("../../../assets/buttonlegend_long_start.png"),
    include_bytes!("../../../assets/buttonlegend_round_menu.png"),
];

pub(crate) fn button_legend(which: ButtonLegend) -> (&'static [u8], u32, u32) {
    static ALL: OnceLock<Vec<(Vec<u8>, u32, u32)>> = OnceLock::new();
    let all = ALL.get_or_init(|| {
        BUTTON_LEGEND_PNG
            .iter()
            .map(|bytes| {
                let (mut rgba, w, h) = decode_png_bytes(bytes).expect("a button legend decodes");
                bleed(&mut rgba, w, h);
                (rgba, w, h)
            })
            .collect()
    });
    let (rgba, w, h) = &all[which as usize];
    (rgba, *w, *h)
}
