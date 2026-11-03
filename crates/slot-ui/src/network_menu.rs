use std::sync::OnceLock;

use crate::draw::{Canvas, Draw, TexId, OUT_H};
use crate::hud::HUD_INK;
use crate::icon::symbols_font;
use crate::plate::UndoFace;
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD};
use crate::slot_chrome::MOUTH_H;
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NetIcons {
    pub bars: [[NetFace; 2]; 4],
    pub lock: [NetFace; 2],
}

pub struct NetworkList<'a> {
    pub lines: &'a [NetLine],
    pub icons: Option<NetIcons>,
    pub selected: usize,
    pub message: Option<NetFace>,
    pub pointer: Option<NetFace>,
    pub ground: Option<(TexId, u32)>,
    pub canvas: Canvas,
}

pub const GLYPH_GAP: f32 = 12.0;
pub const NET_ICON_PX: f32 = 30.0;
pub const WIFI_ICON_PX: f32 = 14.0;

const BAR_GLYPHS: [char; 4] = ['\u{f092f}', '\u{f091f}', '\u{f0925}', '\u{f0928}'];
const LOCK: char = '\u{f033e}';
const NET_GLYPHS: [char; 5] = [
    BAR_GLYPHS[0],
    BAR_GLYPHS[1],
    BAR_GLYPHS[2],
    BAR_GLYPHS[3],
    LOCK,
];

#[derive(Clone, Copy)]
struct Frame {
    w: u32,
    h: u32,
    left: i32,
    top: i32,
}

fn frame_at(px: f32, glyphs: &[char]) -> Option<Frame> {
    let font = symbols_font()?;
    let (mut left, mut right, mut top, mut bottom) = (i32::MAX, i32::MIN, i32::MIN, i32::MAX);
    for &ch in glyphs {
        let m = font.metrics(ch, px);
        left = left.min(m.xmin);
        right = right.max(m.xmin + m.width as i32);
        top = top.max(m.ymin + m.height as i32);
        bottom = bottom.min(m.ymin);
    }
    Some(Frame {
        w: (right - left).max(1) as u32,
        h: (top - bottom).max(1) as u32,
        left,
        top,
    })
}

fn frame() -> Option<Frame> {
    static FRAME: OnceLock<Option<Frame>> = OnceLock::new();
    *FRAME.get_or_init(|| frame_at(NET_ICON_PX, &NET_GLYPHS))
}

fn glyph_face(ch: char, px: f32, glyphs: &[char], ink: [u8; 3]) -> UndoFace {
    let (Some(font), Some(f)) = (symbols_font(), frame_at(px, glyphs)) else {
        return UndoFace {
            rgba: vec![0; 4],
            w: 1,
            h: 1,
        };
    };
    let (m, cov) = font.rasterize(ch, px);
    let x0 = m.xmin - f.left;
    let y0 = f.top - (m.ymin + m.height as i32);
    let mut rgba = vec![0u8; (f.w * f.h * 4) as usize];
    for gy in 0..m.height {
        for gx in 0..m.width {
            let (dx, dy) = (x0 + gx as i32, y0 + gy as i32);
            if dx < 0 || dy < 0 || dx >= f.w as i32 || dy >= f.h as i32 {
                continue;
            }
            let at = ((dy as u32 * f.w + dx as u32) * 4) as usize;
            rgba[at..at + 3].copy_from_slice(&ink);
            rgba[at + 3] = cov[gy * m.width + gx];
        }
    }
    UndoFace {
        rgba,
        w: f.w,
        h: f.h,
    }
}

pub fn net_bars_face(bars: u8, lit: bool) -> UndoFace {
    let ink = if lit { MENU_INK } else { DIM_INK };
    glyph_face(
        BAR_GLYPHS[bars.min(3) as usize],
        NET_ICON_PX,
        &NET_GLYPHS,
        ink,
    )
}

pub fn wifi_face(bars: u8, ink: [u8; 3]) -> UndoFace {
    glyph_face(
        BAR_GLYPHS[bars.min(3) as usize],
        WIFI_ICON_PX,
        &BAR_GLYPHS,
        ink,
    )
}

pub fn net_lock_face(lit: bool) -> UndoFace {
    let ink = if lit { MENU_INK } else { DIM_INK };
    glyph_face(LOCK, NET_ICON_PX, &NET_GLYPHS, ink)
}

fn icon_size() -> (f32, f32) {
    frame().map_or((0.0, 0.0), |f| (f.w as f32, f.h as f32))
}

pub fn bars_rect(canvas: Canvas, row_top: f32) -> [f32; 4] {
    let right = canvas.w - QUICK_EDGE;
    let (w, h) = icon_size();
    [right - w, glyph_top(row_top, h), w, h]
}

pub fn lock_rect(canvas: Canvas, row_top: f32) -> [f32; 4] {
    let [bx, ..] = bars_rect(canvas, row_top);
    let (w, h) = icon_size();
    [bx - GLYPH_GAP - w, glyph_top(row_top, h), w, h]
}

fn glyph_top(row_top: f32, h: f32) -> f32 {
    (row_top + TYPE_DROP + (MENU_H as f32 - h) / 2.0).round()
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
            let y = top + TYPE_DROP;
            let mut end = right;
            if let (Some(bars), Some(icons)) = (line.bars, self.icons) {
                let [x, gy, _, _] = bars_rect(self.canvas, top);
                push(out, icons.bars[bars.min(3) as usize][lit as usize], x, gy);
                end = x;
            }
            if let (true, Some(icons)) = (line.lock, self.icons) {
                let [x, gy, _, _] = lock_rect(self.canvas, top);
                push(out, icons.lock[lit as usize], x, gy);
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

pub const PAIR_X: f32 = 40.0;
pub const PAIR_TOP: f32 = 50.0;
pub const PAIR_QR: u32 = 300;
pub const PAIR_GAP: f32 = 32.0;
pub const PAIR_EDGE: f32 = 20.0;
pub const PAIR_LABEL_PX: f32 = 24.0;
pub const PAIR_CODE_PX: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PairSlot {
    Scan,
    Phone,
    Enter,
    Code,
    Status,
}

impl PairSlot {
    fn y(self) -> f32 {
        match self {
            PairSlot::Scan => 58.0,
            PairSlot::Phone => 92.0,
            PairSlot::Enter => 152.0,
            PairSlot::Code => 186.0,
            PairSlot::Status => 316.0,
        }
    }

    pub fn px(self) -> f32 {
        match self {
            PairSlot::Code => PAIR_CODE_PX,
            _ => PAIR_LABEL_PX,
        }
    }

    pub fn ink(self) -> [u8; 3] {
        match self {
            PairSlot::Code => MENU_INK,
            _ => DIM_INK,
        }
    }
}

pub fn pair_text_width(canvas: Canvas) -> f32 {
    canvas.w - PAIR_X - PAIR_QR as f32 - PAIR_GAP - PAIR_EDGE
}

pub fn pair_face(text: &str, slot: PairSlot, max_w: f32) -> UndoFace {
    let Some(font) = crate::text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let px = slot.px();
    let layout = crate::text::fit(font, text, max_w, 1, px, 12.0);
    let ink = crate::text::line_width(font, &layout.lines.join(""), layout.px, layout.tracking)
        .ceil() as u32;
    let (w, h) = (ink.max(1), (px * 1.4).ceil() as u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    crate::text::draw_centred(&mut rgba, w, h, &layout, slot.ink());
    UndoFace { rgba, w, h }
}

pub struct PairScreen<'a> {
    pub qr: Option<NetFace>,
    pub lines: &'a [(PairSlot, NetFace)],
    pub ground: Option<(TexId, u32)>,
    pub canvas: Canvas,
}

impl PairScreen<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        draw_menu_ground(self.canvas, self.ground, out);
        if let Some(face) = self.qr {
            let inset = (PAIR_QR.saturating_sub(face.1) / 2) as f32;
            push(out, face, PAIR_X + inset, PAIR_TOP + inset);
        }
        let Some(first) = self.lines.first() else {
            return;
        };
        if self.qr.is_some() {
            let x = PAIR_X + PAIR_QR as f32 + PAIR_GAP;
            for (slot, face) in self.lines {
                push(out, *face, x, slot.y());
            }
            return;
        }
        let top = first.0.y();
        let bottom = self
            .lines
            .iter()
            .map(|(slot, face)| slot.y() + face.2 as f32)
            .fold(top, f32::max);
        let shift = (OUT_H as f32 - MOUTH_H - (bottom - top)) / 2.0 - top;
        for (slot, face) in self.lines {
            push(out, *face, self.canvas.centred(face.1 as f32), slot.y() + shift);
        }
    }
}
