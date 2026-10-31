use crate::draw::{Canvas, Draw, TexId};
use crate::footer::{WIFI_DIM, WIFI_GLYPH};
use crate::hud::HUD_INK;
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD};
use crate::quick_menu::{
    draw_menu_ground, DIM_INK, LEGEND_Y, POINTER_BLEED, POINTER_GAP, QUICK_EDGE, QUICK_LABEL_X,
    QUICK_PITCH, QUICK_VALUE_GAP, QUICK_VISIBLE, SCROLL_INSET, SCROLL_W, TYPE_DROP,
    VALUE_MIN_SCALE,
};

pub type NetFace = (TexId, u32, u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetLine {
    pub label: [NetFace; 2],
    pub value: Option<[NetFace; 2]>,
    pub lock: bool,
    pub bars: Option<u8>,
}

pub struct NetworkList<'a> {
    pub lines: &'a [NetLine],
    pub selected: usize,
    pub message: Option<NetFace>,
    pub pointer: Option<NetFace>,
    pub ground: Option<(TexId, u32)>,
    pub canvas: Canvas,
}

pub const GLYPH_SCALE: f32 = 2.0;
pub const GLYPH_GAP: f32 = 12.0;

const LOCK_GLYPH: [&str; 10] = [
    "..XXXXX..",
    ".XX...XX.",
    ".X.....X.",
    ".X.....X.",
    "XXXXXXXXX",
    "XXXXXXXXX",
    "XXXX.XXXX",
    "XXXX.XXXX",
    "XXXXXXXXX",
    "XXXXXXXXX",
];

fn glyph_w(glyph: &[&str]) -> f32 {
    glyph[0].len() as f32 * GLYPH_SCALE
}

fn glyph_h(glyph: &[&str]) -> f32 {
    glyph.len() as f32 * GLYPH_SCALE
}

pub fn bars_rect(canvas: Canvas, row_top: f32) -> [f32; 4] {
    let right = canvas.w - QUICK_EDGE;
    let (w, h) = (glyph_w(&WIFI_GLYPH), glyph_h(&WIFI_GLYPH));
    [right - w, glyph_top(row_top, h), w, h]
}

pub fn lock_rect(canvas: Canvas, row_top: f32) -> [f32; 4] {
    let [bx, ..] = bars_rect(canvas, row_top);
    let (w, h) = (glyph_w(&LOCK_GLYPH), glyph_h(&LOCK_GLYPH));
    [bx - GLYPH_GAP - w, glyph_top(row_top, h), w, h]
}

fn glyph_top(row_top: f32, h: f32) -> f32 {
    (row_top + TYPE_DROP + (MENU_H as f32 - h) / 2.0).round()
}

fn draw_glyph(
    glyph: &[&str],
    x: f32,
    y: f32,
    ink: [u8; 3],
    lit: impl Fn(u8) -> bool,
    out: &mut Vec<Draw>,
) {
    let [r, g, b] = ink.map(|c| c as f32 / 255.0);
    for (row, line) in glyph.iter().enumerate() {
        let cells = line.as_bytes();
        let mut col = 0;
        while col < cells.len() {
            let cell = cells[col];
            if cell == b'.' {
                col += 1;
                continue;
            }
            let start = col;
            while col < cells.len() && cells[col] == cell {
                col += 1;
            }
            let alpha = if lit(cell) { 1.0 } else { WIFI_DIM };
            out.push(Draw::Rect {
                x: x + start as f32 * GLYPH_SCALE,
                y: y + row as f32 * GLYPH_SCALE,
                w: (col - start) as f32 * GLYPH_SCALE,
                h: GLYPH_SCALE,
                colour: [r, g, b, alpha],
            });
        }
    }
}

pub fn net_window(count: usize, selected: usize) -> (usize, usize) {
    if count <= QUICK_VISIBLE {
        return (0, count);
    }
    let start = selected
        .saturating_sub(QUICK_VISIBLE / 2)
        .min(count - QUICK_VISIBLE);
    (start, QUICK_VISIBLE)
}

pub fn net_row_top(count: usize, selected: usize, at: usize) -> f32 {
    let (start, len) = net_window(count, selected);
    let rows = QUICK_PITCH * len as f32;
    (LEGEND_Y - rows) / 2.0 + QUICK_PITCH * (at as f32 - start as f32)
}

fn push(out: &mut Vec<Draw>, (tex, w, h): NetFace, x: f32, y: f32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

impl NetworkList<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        draw_menu_ground(self.canvas, self.ground, out);
        let pad = MENU_PAD as f32;
        let right = self.canvas.w - QUICK_EDGE;
        let count = self.lines.len() + self.message.is_some() as usize;
        let (start, len) = net_window(count, self.selected);
        for at in start..start + len {
            let top = net_row_top(count, self.selected, at);
            let Some(line) = self.lines.get(at) else {
                if let Some(message) = self.message {
                    let y = top + TYPE_DROP + (MENU_H as f32 - message.2 as f32) / 2.0;
                    push(out, message, self.canvas.centred(message.1 as f32), y);
                }
                continue;
            };
            let lit = at == self.selected;
            let ink = if lit { MENU_INK } else { DIM_INK };
            let y = top + TYPE_DROP;
            let mut end = right;
            if let Some(bars) = line.bars {
                let [x, gy, _, _] = bars_rect(self.canvas, top);
                draw_glyph(&WIFI_GLYPH, x, gy, ink, |c| c - b'0' <= bars, out);
                end = x;
            }
            if line.lock {
                let [x, gy, _, _] = lock_rect(self.canvas, top);
                draw_glyph(&LOCK_GLYPH, x, gy, ink, |_| true, out);
                end = x;
            }
            if let Some(value) = line.value {
                let (tex, w, h) = value[lit as usize];
                let x = right - (w as f32 - pad);
                push(out, (tex, w, h), x, y);
                end = x + pad;
            }
            let (tex, w, h) = line.label[lit as usize];
            let x = QUICK_LABEL_X - pad;
            let ink_w = w as f32 - 2.0 * pad;
            let room = match end < right {
                true => end - QUICK_VALUE_GAP - QUICK_LABEL_X,
                false => right - QUICK_LABEL_X,
            };
            let s = (room / ink_w.max(1.0)).clamp(VALUE_MIN_SCALE, 1.0);
            let (sw, sh) = (w as f32 * s, h as f32 * s);
            let ly = (y + (h as f32 - sh) / 2.0).round();
            let lx = QUICK_LABEL_X - pad * s;
            let shown = (room + pad * s).min(sw);
            match shown < sw {
                true => out.push(Draw::Part {
                    x: lx.round(),
                    y: ly,
                    w: shown,
                    h: sh,
                    tex,
                    alpha: 1.0,
                    uv: [0.0, 0.0, shown / sw, 1.0],
                }),
                false => out.push(Draw::Tex {
                    x: lx.round(),
                    y: ly,
                    w: sw,
                    h: sh,
                    tex,
                    alpha: 1.0,
                }),
            }
            if lit {
                if let Some(pointer) = self.pointer {
                    let px = x + pad - POINTER_GAP - pointer.1 as f32;
                    push(out, pointer, px, y - POINTER_BLEED as f32);
                }
            }
        }
        if len < count {
            let x = self.canvas.w - SCROLL_INSET - SCROLL_W;
            let y = net_row_top(count, self.selected, start);
            let h = QUICK_PITCH * len as f32;
            let thumb_h = h / count as f32;
            let along = self.selected as f32 / (count - 1) as f32;
            let ink = DIM_INK.map(|c| f32::from(c) / 255.0);
            out.push(Draw::Rect {
                x,
                y,
                w: SCROLL_W,
                h,
                colour: [ink[0], ink[1], ink[2], 0.3],
            });
            let lit = HUD_INK.map(|c| f32::from(c) / 255.0);
            out.push(Draw::Rect {
                x,
                y: y + (h - thumb_h) * along,
                w: SCROLL_W,
                h: thumb_h,
                colour: [lit[0], lit[1], lit[2], 1.0],
            });
        }
    }
}
