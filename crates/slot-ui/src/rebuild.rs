use slot_gfx::{Canvas, Draw, TexId, OUT_H, OUT_W};

use crate::hud::{FILL, HUD_INK, TRACK};
use crate::plate::UndoFace;
use crate::text;

const BAR_W: f32 = 440.0;
const BAR_H: f32 = 10.0;
const TITLE_PX: f32 = 40.0;
const TITLE_H: u32 = 52;
const HEADLINE_PX: f32 = 35.0;
const HEADLINE_H: u32 = 46;
const COUNT_PX: f32 = 26.0;
const COUNT_H: u32 = 36;
const TITLE_GAP: f32 = 30.0;
const COUNT_GAP: f32 = 24.0;

pub fn rebuild_title_face(text: &str) -> UndoFace {
    line_face(text, TITLE_PX, TITLE_H)
}

pub fn headline_face(text: &str) -> UndoFace {
    line_face(text, HEADLINE_PX, HEADLINE_H)
}

pub fn rebuild_count_face(text: &str) -> UndoFace {
    line_face(text, COUNT_PX, COUNT_H)
}

fn line_face(label: &str, px: f32, h: u32) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let w = OUT_W;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, px, px);
    text::draw_centred(&mut rgba, w, h, &layout, HUD_INK);
    trim(&rgba, w, h)
}

fn trim(rgba: &[u8], w: u32, h: u32) -> UndoFace {
    let inked = |x: u32, y: u32| rgba[((y * w + x) * 4 + 3) as usize] > 0;
    let rows: Vec<u32> = (0..h).filter(|&y| (0..w).any(|x| inked(x, y))).collect();
    let cols: Vec<u32> = (0..w).filter(|&x| (0..h).any(|y| inked(x, y))).collect();
    let (Some(&top), Some(&bottom), Some(&left), Some(&right)) =
        (rows.first(), rows.last(), cols.first(), cols.last())
    else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let (tw, th) = (right - left + 1, bottom - top + 1);
    let mut out = Vec::with_capacity((tw * th * 4) as usize);
    for y in top..=bottom {
        let at = ((y * w + left) * 4) as usize;
        out.extend_from_slice(&rgba[at..at + (tw * 4) as usize]);
    }
    UndoFace {
        rgba: out,
        w: tw,
        h: th,
    }
}

pub struct RebuildScreen {
    pub title: Option<(TexId, u32, u32)>,
    pub count: Option<(TexId, u32, u32)>,
    pub fraction: f32,
}

impl RebuildScreen {
    pub fn draw(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: OUT_H as f32,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let height = |line: Option<(TexId, u32, u32)>, gap: f32| {
            line.map_or(0.0, |(_, _, h)| h as f32 + gap)
        };
        let stack = height(self.title, TITLE_GAP) + BAR_H + height(self.count, COUNT_GAP);
        let x = canvas.centred(BAR_W);
        let y = OUT_H as f32 / 2.0 - stack / 2.0 + height(self.title, TITLE_GAP);
        let centred = |(tex, w, h): (TexId, u32, u32), top: f32| Draw::Tex {
            x: canvas.centred(w as f32),
            y: top,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        };
        if let Some(title) = self.title {
            out.push(centred(title, y - TITLE_GAP - title.2 as f32));
        }
        out.push(Draw::Rect {
            x,
            y,
            w: BAR_W,
            h: BAR_H,
            colour: TRACK,
        });
        out.push(Draw::Rect {
            x,
            y,
            w: BAR_W * self.fraction.clamp(0.0, 1.0),
            h: BAR_H,
            colour: FILL,
        });
        if let Some(count) = self.count {
            out.push(centred(count, y + BAR_H + COUNT_GAP));
        }
    }
}
