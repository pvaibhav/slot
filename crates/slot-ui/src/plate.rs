use crate::draw::{Draw, TexId};
use crate::text;

pub const HINT_H: u32 = 24;
pub const CAP: u32 = 20;
const CAP_PAD: u32 = 4;
const CAP_MAX_W: f32 = 64.0;
pub const HINT_GAP: f32 = 14.0;

pub const CAP_GAP: u32 = 5;
const EDGE: u32 = 2;
pub const HINT_EDGE: u32 = EDGE;
const LABEL_MAX_W: f32 = 140.0;
const WORD_MAX_W: f32 = 320.0;

pub const TITLE_W: u32 = 360;
pub const TITLE_H: u32 = 24;

const INK: [u8; 3] = [0xf6, 0xf4, 0xef];
const CAP_INK: [u8; 3] = [0x1a, 0x19, 0x17];
const KEY_PX: f32 = 14.0;
const LABEL_PX: f32 = 16.0;
const LABEL_MIN_PX: f32 = 10.0;
const TITLE_PX: f32 = 20.0;
const TITLE_MIN_PX: f32 = 12.0;

pub const TITLE_LINE_H: u32 = 32;
const TITLE_LINE_PX: f32 = 24.0;
const TITLE_LINE_MIN_PX: f32 = 14.0;
const TITLE_LINE_MAX_W: f32 = 560.0;

pub struct UndoFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Hint {
    pub key: &'static str,
    pub label: String,
}

pub fn hint_row(legend: &[(&'static str, &'static str)]) -> Vec<Hint> {
    legend
        .iter()
        .map(|(key, label)| Hint {
            key,
            label: label.to_string(),
        })
        .collect()
}

pub fn cap_width(key: &str) -> u32 {
    let Some(font) = text::label_font() else {
        return CAP;
    };
    let layout = text::fit(font, key, CAP_MAX_W, 1, KEY_PX, KEY_PX);
    let ink = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    CAP.max(ink.ceil() as u32 + 2 * CAP_PAD)
}

pub fn hint_width(key: &str, label: &str) -> u32 {
    cap_width(key) + CAP_GAP + band_width(label) + EDGE
}

pub fn hint_face(key: &str, label: &str) -> UndoFace {
    hint_coloured(key, label, INK)
}

pub fn hint_coloured(key: &str, label: &str, ink: [u8; 3]) -> UndoFace {
    let w = hint_width(key, label);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];

    let cap_w = cap_width(key);
    let mut cap = Vec::with_capacity((cap_w * CAP * 4) as usize);
    for _ in 0..cap_w * CAP {
        cap.extend_from_slice(&[INK[0], INK[1], INK[2], 255]);
    }
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, key, cap_w as f32, 1, KEY_PX, KEY_PX);
        text::draw_centred(&mut cap, cap_w, CAP, &layout, CAP_INK);
    }
    blit(&mut rgba, w, &cap, cap_w, CAP, 0, (HINT_H - CAP) / 2);

    let text_w = band_width(label);
    let mut band = vec![0u8; (text_w * HINT_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, label, text_w as f32, 1, LABEL_PX, LABEL_MIN_PX);
        text::draw_centred(&mut band, text_w, HINT_H, &layout, ink);
    }
    blit(&mut rgba, w, &band, text_w, HINT_H, cap_w + CAP_GAP, 0);

    UndoFace { rgba, w, h: HINT_H }
}

pub fn word_width(text: &str) -> u32 {
    capped_width(text, WORD_MAX_W)
}

pub fn word_face(text: &str) -> UndoFace {
    word_ink(text, INK)
}

pub fn word_ink(text: &str, ink: [u8; 3]) -> UndoFace {
    let w = word_width(text);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, text, w as f32, 1, LABEL_PX, LABEL_MIN_PX);
        text::draw_centred(&mut rgba, w, HINT_H, &layout, ink);
    }
    UndoFace { rgba, w, h: HINT_H }
}

pub fn hint_quad(x: f32, y: f32, w: f32, face: Option<TexId>) -> Draw {
    let h = HINT_H as f32;
    match face {
        Some(tex) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [1.0, 1.0, 1.0, 0.12],
        },
    }
}

fn band_width(label: &str) -> u32 {
    capped_width(label, LABEL_MAX_W)
}

fn capped_width(label: &str, max_w: f32) -> u32 {
    let Some(font) = text::label_font() else {
        return max_w as u32;
    };
    let layout = text::fit(font, label, max_w, 1, LABEL_PX, LABEL_MIN_PX);
    let ink = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    (ink.ceil() as u32).clamp(1, max_w as u32)
}

pub fn title_line_face(text: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(
        font,
        text,
        TITLE_LINE_MAX_W,
        1,
        TITLE_LINE_PX,
        TITLE_LINE_MIN_PX,
    );
    let ink = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = (ink.ceil() as u32).clamp(1, TITLE_LINE_MAX_W as u32);
    let mut rgba = vec![0u8; (w * TITLE_LINE_H * 4) as usize];
    text::draw_centred(&mut rgba, w, TITLE_LINE_H, &layout, [0xff, 0xff, 0xff]);
    UndoFace {
        rgba,
        w,
        h: TITLE_LINE_H,
    }
}

pub fn title_face(text: &str) -> UndoFace {
    let mut rgba = vec![0u8; (TITLE_W * TITLE_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, text, TITLE_W as f32, 1, TITLE_PX, TITLE_MIN_PX);
        text::draw_centred(&mut rgba, TITLE_W, TITLE_H, &layout, INK);
    }
    UndoFace {
        rgba,
        w: TITLE_W,
        h: TITLE_H,
    }
}

pub(crate) fn blit(dst: &mut [u8], dst_w: u32, src: &[u8], src_w: u32, src_h: u32, x: u32, y: u32) {
    for row in 0..src_h {
        let from = ((row * src_w) * 4) as usize;
        let to = (((y + row) * dst_w + x) * 4) as usize;
        dst[to..to + (src_w * 4) as usize].copy_from_slice(&src[from..from + (src_w * 4) as usize]);
    }
}

pub const ARROW_GAP: u32 = 3;

const LEFT_CARET: char = '\u{f0d9}';
const RIGHT_CARET: char = '\u{f0da}';

pub fn arrows_hint_width(label: &str) -> u32 {
    2 * CAP + ARROW_GAP + CAP_GAP + band_width(label) + EDGE
}

pub fn arrows_hint_face(label: &str) -> UndoFace {
    arrows_hint_coloured(label, INK)
}

pub fn arrows_hint_coloured(label: &str, ink: [u8; 3]) -> UndoFace {
    let w = arrows_hint_width(label);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];
    for (i, glyph) in [LEFT_CARET, RIGHT_CARET].into_iter().enumerate() {
        blit(
            &mut rgba,
            w,
            &glyph_cap(glyph),
            CAP,
            CAP,
            i as u32 * (CAP + ARROW_GAP),
            (HINT_H - CAP) / 2,
        );
    }
    let text_w = band_width(label);
    let mut band = vec![0u8; (text_w * HINT_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, label, text_w as f32, 1, LABEL_PX, LABEL_MIN_PX);
        text::draw_centred(&mut band, text_w, HINT_H, &layout, ink);
    }
    blit(
        &mut rgba,
        w,
        &band,
        text_w,
        HINT_H,
        2 * CAP + ARROW_GAP + CAP_GAP,
        0,
    );
    UndoFace { rgba, w, h: HINT_H }
}

fn glyph_cap(glyph: char) -> Vec<u8> {
    let mut cap = Vec::with_capacity((CAP * CAP * 4) as usize);
    for _ in 0..CAP * CAP {
        cap.extend_from_slice(&[INK[0], INK[1], INK[2], 255]);
    }
    let Some(font) = crate::icon::symbols_font() else {
        return cap;
    };
    let (m, cov) = font.rasterize(glyph, KEY_PX);
    let x0 = (CAP as i32 - m.width as i32) / 2;
    let y0 = (CAP as i32 - m.height as i32) / 2;
    for gy in 0..m.height {
        for gx in 0..m.width {
            let (dx, dy) = (x0 + gx as i32, y0 + gy as i32);
            if dx < 0 || dy < 0 || dx >= CAP as i32 || dy >= CAP as i32 {
                continue;
            }
            let a = cov[gy * m.width + gx] as u32;
            let at = ((dy as u32 * CAP + dx as u32) * 4) as usize;
            for k in 0..3 {
                cap[at + k] =
                    ((CAP_INK[k] as u32 * a + cap[at + k] as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
    cap
}

const PAD_GLYPH: char = '\u{f047}';
const TICK_GLYPH: char = '\u{f00c}';

pub fn pad_hint_width(label: &str) -> u32 {
    CAP + CAP_GAP + band_width(label) + EDGE
}

pub fn pad_hint_face(label: &str) -> UndoFace {
    let w = pad_hint_width(label);
    let mut rgba = vec![0u8; (w * HINT_H * 4) as usize];
    blit(
        &mut rgba,
        w,
        &glyph_cap(PAD_GLYPH),
        CAP,
        CAP,
        0,
        (HINT_H - CAP) / 2,
    );
    let text_w = band_width(label);
    let mut band = vec![0u8; (text_w * HINT_H * 4) as usize];
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, label, text_w as f32, 1, LABEL_PX, LABEL_MIN_PX);
        text::draw_centred(&mut band, text_w, HINT_H, &layout, INK);
    }
    blit(&mut rgba, w, &band, text_w, HINT_H, CAP + CAP_GAP, 0);
    UndoFace { rgba, w, h: HINT_H }
}

pub fn tick_cap_face() -> UndoFace {
    UndoFace {
        rgba: glyph_cap(TICK_GLYPH),
        w: CAP,
        h: CAP,
    }
}
