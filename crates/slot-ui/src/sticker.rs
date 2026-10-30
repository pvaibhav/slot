use slot_gfx::{Draw, TexId};

use crate::art::render_svg;
use crate::barcode::{code39, CODE39_NARROW, CODE39_WIDE};
use crate::bitmaps;
use crate::plate::UndoFace;
use crate::text;

const ANBERNIC_PATHS: &str = include_str!("../assets/anbernic.txt");
const MODEL_GLYPHS: &str = include_str!("../assets/model_glyphs.txt");

const DEFAULT_MODEL: &str = "RG SP";

const LOCKUP_PAD: f32 = 22.5;
const LOCKUP_H: f32 = 145.0;
const MODEL_X: f32 = 778.0955;
const WORDMARK_PX_PER_UNIT: f32 = 250.0 / 1283.0;

pub const STICKER_W: u32 = 660;
pub const STICKER_H: u32 = 228;

const BLACK: [u8; 3] = [0x23, 0x1f, 0x20];
const WHITE: [u8; 3] = [0xff, 0xff, 0xff];

const PANEL_FX: f32 = 62.133 / 205.762;
const PANEL_FY: f32 = 3.81 / 71.116;
const PANEL_FW: f32 = 140.315 / 205.762;
const PANEL_FH: f32 = 35.433 / 71.116;

pub const DC: char = '\u{2393}';

const MARGIN: f32 = 11.0;
const HEAD_PX: f32 = 11.0;
const BODY_PX: f32 = 8.5;
const SERIAL_PX: f32 = 26.0;
const SMALL_PX: f32 = 9.0;

pub struct StickerFields<'a> {
    pub battery: Option<u8>,
    pub serial: &'a str,
    pub version: &'a str,
    pub baseos: Option<&'a str>,
    pub model: &'a str,
}

pub const CREDITS: [&str; 11] = [
    "THE EXQUISITE SKEUOMORPHIC",
    "ARTWORK MADE BY MARKUS.",
    "EMULATION POWERED BY MGBA",
    "AND GPSP. UNDERLYING OS IS",
    "BASEOS BY PVAIBHAV. TYPE IS",
    "OPEN SANS AND NERD FONTS",
    "SYMBOLS BY RYAN L MCINTYRE.",
    "THE PANEL MASK IS DERIVED",
    "FROM GIGAHERZ'S LCD3X. THE",
    "CART SOUNDS ARE MY CHILDHOOD",
    "GAMEBOY.",
];

pub const ORIGIN: [&str; 2] = ["S/LOT-USA", "MADE IN ITHACA"];

pub const COPYRIGHT: &str = "\u{a9} 2026 BRANDON T. KOWALSKI";

pub const HOME: &str = "SEE README.";

pub fn head_rows(f: &StickerFields) -> [String; 3] {
    [
        "MODEL NO. AGS-102".into(),
        format!("INPUT : 5V{DC}1.5A"),
        match f.battery {
            Some(p) => format!("BATTERY : LI-ION ({p}%)"),
            None => "BATTERY : LI-ION".to_string(),
        },
    ]
}

fn split_check(serial: &str) -> (&str, &str) {
    let at = serial.char_indices().last().map_or(0, |(i, _)| i);
    serial.split_at(at)
}

fn version_lines(f: &StickerFields) -> Vec<String> {
    let mut out = vec![format!("VERSION {}", f.version)];
    out.extend(f.baseos.map(|v| format!("BASEOS {v}")));
    out
}

pub fn sticker_lines(f: &StickerFields) -> Vec<String> {
    let mut out: Vec<String> = head_rows(f).into();
    out.extend(CREDITS.iter().map(|s| s.to_string()));
    out.extend(ORIGIN.iter().map(|s| s.to_string()));
    let (body, check) = split_check(f.serial);
    out.push(format!("{body} {check}"));
    out.push(COPYRIGHT.to_string());
    out.extend(version_lines(f));
    out.push(HOME.to_string());
    out
}

struct Canvas {
    px: Vec<u8>,
    w: u32,
    h: u32,
}

impl Canvas {
    fn shape(w: u32, h: u32) -> Canvas {
        Canvas {
            px: bitmaps::sticker().to_vec(),
            w,
            h,
        }
    }

    fn set(&mut self, x: u32, y: u32, c: [u8; 3], a: u8) {
        if x >= self.w || y >= self.h {
            return;
        }
        let at = ((y * self.w + x) * 4) as usize;
        self.px[at..at + 3].copy_from_slice(&c);
        self.px[at + 3] = a;
    }

    fn rect(&mut self, x: u32, y: u32, w: u32, h: u32, c: [u8; 3]) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                self.set(xx, yy, c, 255);
            }
        }
    }

    fn print_measure(&self, s: &str, px: f32) -> f32 {
        let Some(font) = text::label_font() else {
            return 0.0;
        };
        let layout = text::fit(font, s, f32::MAX, 1, px, px);
        layout
            .lines
            .first()
            .map(|l| text::line_width(font, l, px, layout.tracking))
            .unwrap_or(0.0)
    }

    fn dc(&mut self, x: f32, y: f32, px: f32, c: [u8; 3]) -> f32 {
        let bar_w = px * 0.78;
        let t = (px / 8.0).round().max(1.0);
        let gap = (t * 2.0).max(3.0);
        let bar_y = (y + px * 0.685 - (t * 2.0 + gap) / 2.0).round();
        self.rect(x as u32, bar_y as u32, bar_w as u32, t as u32, c);
        let dash = bar_w / 5.0;
        for n in 0..3 {
            let dx = x + n as f32 * dash * 2.0;
            self.rect(
                dx as u32,
                (bar_y + t + gap) as u32,
                dash.ceil() as u32,
                t as u32,
                c,
            );
        }
        bar_w
    }

    fn blit(&mut self, x: u32, y: u32, src: &[u8], sw: u32, sh: u32) {
        for row in 0..sh {
            for col in 0..sw {
                let s = ((row * sw + col) * 4) as usize;
                let a = src[s + 3] as u32;
                if a == 0 {
                    continue;
                }
                let (dx, dy) = (x + col, y + row);
                if dx >= self.w || dy >= self.h {
                    continue;
                }
                let d = ((dy * self.w + dx) * 4) as usize;
                for k in 0..3 {
                    let under = self.px[d + k] as u32;
                    self.px[d + k] = ((src[s + k] as u32 * a + under * (255 - a)) / 255) as u8;
                }
                self.px[d + 3] = 255;
            }
        }
    }

    fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, t: f32, c: [u8; 3]) {
        let (x, y, w, h, t) = (x as u32, y as u32, w as u32, h as u32, t as u32);
        self.rect(x, y, w, t, c);
        self.rect(x, y + h - t, w, t, c);
        self.rect(x, y, t, h, c);
        self.rect(x + w - t, y, t, h, c);
    }

    fn print(&mut self, x: f32, y: f32, s: &str, px: f32, c: [u8; 3]) -> f32 {
        let Some(font) = text::label_font() else {
            return y;
        };
        let layout = text::fit(font, s, f32::MAX, 1, px, px);
        let measured = layout
            .lines
            .first()
            .map(|l| text::line_width(font, l, px, layout.tracking))
            .unwrap_or(0.0);
        let (bw, bh) = ((measured.ceil() as u32).max(1), (px * 1.6).ceil() as u32);
        let cov = text::coverage(bw, bh, &layout);
        for row in 0..bh {
            for col in 0..bw {
                let a = cov[(row * bw + col) as usize];
                if a == 0 {
                    continue;
                }
                let (dx, dy) = (x as u32 + col, y as u32 + row);
                if dx < self.w && dy < self.h {
                    let at = ((dy * self.w + dx) * 4) as usize;
                    let inv = 255 - a as u32;
                    for (k, ink) in c.iter().enumerate() {
                        let under = self.px[at + k] as u32;
                        self.px[at + k] = ((*ink as u32 * a as u32 + under * inv) / 255) as u8;
                    }
                    self.px[at + 3] = 255;
                }
            }
        }
        y + px * 1.35
    }
}

pub fn sticker_face(f: &StickerFields) -> UndoFace {
    let mut c = Canvas::shape(STICKER_W, STICKER_H);
    let panel_x = (PANEL_FX * STICKER_W as f32).round() as u32;
    let panel_y = (PANEL_FY * STICKER_H as f32).round() as u32;
    let panel_w = (PANEL_FW * STICKER_W as f32).round() as u32;
    let panel_h = (PANEL_FH * STICKER_H as f32).round() as u32;

    let left = MARGIN;
    let mut y = MARGIN;
    for (n, line) in head_rows(f).iter().enumerate() {
        if n == 1 {
            if let Some((before, after)) = line.split_once(DC) {
                let bw = c.print_measure(before, HEAD_PX);
                c.print(left, y, before, HEAD_PX, WHITE);
                let dw = c.dc(left + bw + 4.0, y, HEAD_PX, WHITE);
                y = c.print(left + bw + dw + 8.0, y, after, HEAD_PX, WHITE);
                continue;
            }
        }
        y = c.print(left, y, line, HEAD_PX, WHITE);
    }
    y += 3.0;
    for line in CREDITS {
        y = c.print(left, y, line, BODY_PX, WHITE);
    }
    y += 3.0;
    let col_right = panel_x as f32 - MARGIN;
    c.print(left, y, ORIGIN[0], BODY_PX, WHITE);
    let maker_w = c.print_measure(ORIGIN[1], BODY_PX);
    c.print(col_right - maker_w, y, ORIGIN[1], BODY_PX, WHITE);

    let bars_y = panel_y + 14;
    let bars_h = 62;
    let payload = format!("SLOT-{}", f.serial);
    if let Some(run) = code39(&format!("*{payload}*")) {
        let syms = run.len() as f32 / 9.0;
        let per_sym = 3.0 * CODE39_WIDE + 7.0 * CODE39_NARROW;
        let scale = ((panel_w as f32 - 12.0) / (syms * per_sym + 20.0 * CODE39_NARROW)).min(2.0);
        let total_for = |narrow: u32, wide: u32| -> u32 {
            run.iter()
                .map(|w| if *w { wide } else { narrow })
                .sum::<u32>()
                + (run.len() as u32 / 9) * narrow
        };
        let top = ((CODE39_NARROW * scale).round() as u32).max(1);
        let (narrow, wide, total) = (1..=top)
            .rev()
            .flat_map(|n| {
                let w = ((CODE39_WIDE * n as f32).round() as u32).max(n * 2);
                (n * 2..=w).rev().map(move |w| (n, w))
            })
            .map(|(n, w)| (n, w, total_for(n, w)))
            .find(|&(n, _, t)| t + 20 * n <= panel_w)
            .unwrap_or((1, 2, total_for(1, 2)));
        let mut x = panel_x + panel_w.saturating_sub(total) / 2;
        for chunk in run.chunks_exact(9) {
            for (n, is_wide) in chunk.iter().enumerate() {
                let ew = if *is_wide { wide } else { narrow };
                if n.is_multiple_of(2) {
                    c.rect(x, bars_y, ew, bars_h, BLACK);
                }
                x += ew;
            }
            x += narrow;
        }
    }

    let sy = (bars_y + bars_h + 4) as f32;
    let (body, check) = split_check(f.serial);
    let hash_w = c.print_measure(body, SERIAL_PX);
    let box_w = SERIAL_PX * 0.9;
    let total = hash_w + 10.0 + box_w;
    let sx = panel_x as f32 + (panel_w as f32 - total) / 2.0;
    c.print(sx, sy, body, SERIAL_PX, BLACK);
    let bx = sx + hash_w + 10.0;
    c.outline(bx, sy + 1.0, box_w, SERIAL_PX * 1.25, 2.0, BLACK);
    c.print(bx + box_w * 0.28, sy + 2.0, check, SERIAL_PX * 0.85, BLACK);

    let mut ry = panel_y as f32 + panel_h as f32 + 10.0;
    let right_edge = (panel_x + panel_w) as f32;
    if let Some(mark) = wordmark(f.model) {
        let (mw, mh) = mark.1;
        c.blit(
            (right_edge - mw as f32 - 10.0) as u32,
            ry as u32,
            &mark.0,
            mw,
            mh,
        );
        ry += mh as f32 + 4.0;
    }
    let lines = [COPYRIGHT.to_string()]
        .into_iter()
        .chain(version_lines(f))
        .chain([HOME.to_string()]);
    for line in lines {
        let line = line.as_str();
        let lw = c.print_measure(line, SMALL_PX);
        ry = c.print(right_edge - lw - 10.0, ry, line, SMALL_PX, WHITE);
    }

    UndoFace {
        rgba: c.px,
        w: STICKER_W,
        h: STICKER_H,
    }
}

fn model_line(model: &str) -> Option<(Vec<(f32, &'static str)>, f32)> {
    let mut x = 0.0;
    let mut glyphs = Vec::new();
    let first = model.split('/').next().unwrap_or_default();
    for ch in first.trim().to_uppercase().chars() {
        let (advance, path) = MODEL_GLYPHS.lines().find_map(|line| {
            let mut cols = line.split('\t');
            (cols.next()?.chars().eq([ch])).then_some(())?;
            Some((cols.next()?.parse::<f32>().ok()?, cols.next()?))
        })?;
        if !path.is_empty() {
            glyphs.push((x, path));
        }
        x += advance;
    }
    (!glyphs.is_empty()).then_some((glyphs, x))
}

fn wordmark_svg(model: &str) -> String {
    let (glyphs, advance) = model_line(model)
        .or_else(|| model_line(DEFAULT_MODEL))
        .unwrap_or_default();
    let w = 2.0 * LOCKUP_PAD + MODEL_X + advance;
    let paths: String = ANBERNIC_PATHS
        .lines()
        .map(|d| format!("<path d=\"{d}\"/>"))
        .chain(glyphs.iter().map(|(x, d)| {
            format!(
                "<path transform=\"translate({:.4} 0)\" d=\"{d}\"/>",
                MODEL_X + x
            )
        }))
        .collect();
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.4} {LOCKUP_H}\">\
         <g transform=\"translate({LOCKUP_PAD} {}) scale(1 -1)\">\
         <g fill=\"none\" stroke=\"#ffffff\" stroke-width=\"17\" stroke-linejoin=\"round\" stroke-linecap=\"round\">{paths}</g>\
         <g fill=\"#231f20\">{paths}</g></g></svg>",
        LOCKUP_H - LOCKUP_PAD
    )
}

fn wordmark(model: &str) -> Option<(Vec<u8>, (u32, u32))> {
    let svg = wordmark_svg(model);
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let w = (size.width() * WORDMARK_PX_PER_UNIT).round() as u32;
    let h = (size.height() * WORDMARK_PX_PER_UNIT).round() as u32;
    Some((render_svg(&svg, w, h)?, (w, h)))
}

const STICKER_MARGIN: f32 = 16.0;

pub fn sticker_rect(canvas: slot_gfx::Canvas) -> (f32, f32, f32, f32) {
    let (w, h) = (STICKER_W as f32, STICKER_H as f32);
    let room = canvas.w - 2.0 * STICKER_MARGIN;
    let scale = (room / w).min(1.0);
    let (w, h) = ((w * scale).round(), (h * scale).round());
    (
        canvas.centred(w).round(),
        ((slot_gfx::OUT_H as f32 - h) / 2.0).round(),
        w,
        h,
    )
}

pub fn draw_sticker(canvas: slot_gfx::Canvas, face: Option<TexId>, out: &mut Vec<Draw>) {
    let Some(tex) = face else {
        return;
    };
    let (x, y, w, h) = sticker_rect(canvas);
    out.push(Draw::Tex {
        x,
        y,
        w,
        h,
        tex,
        alpha: 1.0,
    });
}
