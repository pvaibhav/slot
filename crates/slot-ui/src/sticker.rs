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

pub const CREDITS: [&str; 12] = [
    "THE EXQUISITE SKEUOMORPHIC",
    "ARTWORK MADE BY MARKUS.",
    "EMULATION POWERED BY MGBA,",
    "GPSP AND GAMBATTE.",
    "UNDERLYING OS IS",
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
        self.print_layout(x, y, &layout, c)
    }

    fn cased(s: &str, px: f32) -> text::Layout {
        text::Layout {
            lines: vec![s.to_string()],
            px,
            tracking: DS_CASED_TRACKING,
        }
    }

    fn print_cased(&mut self, x: f32, y: f32, s: &str, px: f32, c: [u8; 3]) -> f32 {
        self.print_layout(x, y, &Canvas::cased(s, px), c)
    }

    fn measure_cased(&self, s: &str, px: f32) -> f32 {
        text::label_font().map_or(0.0, |font| text::line_width(font, s, px, DS_CASED_TRACKING))
    }

    fn print_layout(&mut self, x: f32, y: f32, layout: &text::Layout, c: [u8; 3]) -> f32 {
        let Some(font) = text::label_font() else {
            return y;
        };
        let px = layout.px;
        let measured = layout
            .lines
            .first()
            .map(|l| text::line_width(font, l, px, layout.tracking))
            .unwrap_or(0.0);
        let (bw, bh) = ((measured.ceil() as u32).max(1), (px * 1.6).ceil() as u32);
        let cov = text::coverage(bw, bh, layout);
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

impl Canvas {
    fn barcode(&mut self, panel_x: u32, panel_w: u32, bars_y: u32, bars_h: u32, payload: &str) {
        let Some(run) = code39(&format!("*{payload}*")) else {
            return;
        };
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
                    self.rect(x, bars_y, ew, bars_h, BLACK);
                }
                x += ew;
            }
            x += narrow;
        }
    }

    fn serial(&mut self, panel_x: u32, panel_w: u32, sy: f32, serial: &str) {
        let (body, check) = split_check(serial);
        let hash_w = self.print_measure(body, SERIAL_PX);
        let box_w = SERIAL_PX * 0.9;
        let total = hash_w + 10.0 + box_w;
        let sx = panel_x as f32 + (panel_w as f32 - total) / 2.0;
        self.print(sx, sy, body, SERIAL_PX, BLACK);
        let bx = sx + hash_w + 10.0;
        self.outline(bx, sy + 1.0, box_w, SERIAL_PX * 1.25, 2.0, BLACK);
        self.print(bx + box_w * 0.28, sy + 2.0, check, SERIAL_PX * 0.85, BLACK);
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
    c.barcode(
        panel_x,
        panel_w,
        bars_y,
        bars_h,
        &format!("SLOT-{}", f.serial),
    );
    c.serial(panel_x, panel_w, (bars_y + bars_h + 4) as f32, f.serial);

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

pub const DS_STICKER_W: u32 = 600;
pub const DS_STICKER_H: u32 = 368;

const DS_MODEL: &str = "RG DS+";
const DS_MARGIN: f32 = 20.0;
const DS_RADIUS: f32 = 14.0;
const DS_HEAD_PX: f32 = 13.0;
const DS_BODY_PX: f32 = 11.5;
const DS_PANEL_H: u32 = 122;
const DS_CASED_TRACKING: f32 = 0.3;

pub const DS_CREDITS: &str = "The exquisite skeuomorphic artwork made by Markus. \
Emulation powered by DSperate, mGBA, gpSP and Gambatte. Type is Open Sans and Nerd \
Fonts Symbols by Ryan L McIntyre. The panel mask is derived from Gigaherz's LCD3x. \
The cart sounds are my childhood Game\u{a0}Boy.";

pub const DS_MODEL_NO: &str = "MODEL NO. NTR-002";

pub const DS_SERVICE: &str = "FOR CONSUMER SERVICE: SLOT-CFW.FYI";

pub const DS_BADGE: &str = "SLOT";

pub const DS_SEAL: [&str; 2] = ["RUSS", "APPROVED"];

const DS_GAP: &str = "   ";

pub fn ds_head_rows(f: &StickerFields) -> [String; 3] {
    let battery = match f.battery {
        Some(p) => format!("BATTERY: LI-ION ({p}%)"),
        None => "BATTERY: LI-ION".to_string(),
    };
    let mut version = format!("VERSION {}", f.version);
    if let Some(os) = f.baseos {
        version.push_str(&format!("{DS_GAP}DSOS {os}"));
    }
    [
        COPYRIGHT.to_string(),
        format!("RATING: 5V{DC}1.5A{DS_GAP}{battery}"),
        version,
    ]
}

pub fn ds_sticker_lines(f: &StickerFields) -> Vec<String> {
    let mut out = vec![DS_MODEL_NO.to_string()];
    out.extend(ds_head_rows(f));
    out.push(DS_BADGE.to_string());
    out.push(DS_SEAL.join(" "));
    out.push(DS_CREDITS.to_string());
    out.push(DS_SERVICE.to_string());
    let (body, check) = split_check(f.serial);
    out.push(format!("{body} {check}"));
    out.extend(ORIGIN.iter().rev().map(|s| s.to_string()));
    out
}

fn rounded_cover(px: f32, py: f32, x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let dx = (px - (x + w / 2.0)).abs() - (w / 2.0 - r);
    let dy = (py - (y + h / 2.0)).abs() - (h / 2.0 - r);
    let out = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - r;
    (0.5 - out).clamp(0.0, 1.0)
}

fn mirror(i: u32, n: u32) -> u32 {
    let k = i % (2 * n);
    if k < n {
        k
    } else {
        2 * n - 1 - k
    }
}

impl Canvas {
    fn blank(w: u32, h: u32) -> Canvas {
        Canvas {
            px: vec![0; (w * h * 4) as usize],
            w,
            h,
        }
    }

    fn cover(&mut self, x: u32, y: u32, c: [u8; 3], a: f32) {
        if a <= 0.0 || x >= self.w || y >= self.h {
            return;
        }
        let at = ((y * self.w + x) * 4) as usize;
        let under = self.px[at + 3] as f32 / 255.0;
        let out = a + under * (1.0 - a);
        for (k, ink) in c.iter().enumerate() {
            let below = self.px[at + k] as f32 * under * (1.0 - a);
            self.px[at + k] = ((*ink as f32 * a + below) / out).round() as u8;
        }
        self.px[at + 3] = (out * 255.0).round() as u8;
    }

    fn rounded_fill(
        &mut self,
        rect: (f32, f32, f32, f32, f32),
        sample: impl Fn(u32, u32) -> [u8; 3],
    ) {
        let (x, y, w, h, r) = rect;
        for yy in y.floor() as u32..(y + h).ceil() as u32 {
            for xx in x.floor() as u32..(x + w).ceil() as u32 {
                let a = rounded_cover(xx as f32 + 0.5, yy as f32 + 0.5, x, y, w, h, r);
                self.cover(xx, yy, sample(xx - x as u32, yy - y as u32), a);
            }
        }
    }

    fn rounded_ring(&mut self, rect: (f32, f32, f32, f32, f32), t: f32, c: [u8; 3]) {
        let (x, y, w, h, r) = rect;
        for yy in y.floor() as u32..(y + h).ceil() as u32 {
            for xx in x.floor() as u32..(x + w).ceil() as u32 {
                let (px, py) = (xx as f32 + 0.5, yy as f32 + 0.5);
                let outer = rounded_cover(px, py, x, y, w, h, r);
                let inner = rounded_cover(
                    px,
                    py,
                    x + t,
                    y + t,
                    w - 2.0 * t,
                    h - 2.0 * t,
                    (r - t).max(0.0),
                );
                self.cover(xx, yy, c, outer - inner);
            }
        }
    }

    fn print_spaced(&mut self, x: f32, y: f32, line: &str, px: f32, c: [u8; 3]) -> f32 {
        let mut x = x;
        let mut next = y;
        for part in line.split(DS_GAP) {
            next = self.print_dc_row(x, y, part, px, c);
            x += self.measure_dc_row(part, px) + px * 1.1;
        }
        next
    }

    fn measure_dc_row(&self, line: &str, px: f32) -> f32 {
        match line.split_once(DC) {
            Some((before, after)) => {
                self.print_measure(before, px) + px * 0.78 + 8.0 + self.print_measure(after, px)
            }
            None => self.print_measure(line, px),
        }
    }

    fn print_dc_row(&mut self, x: f32, y: f32, line: &str, px: f32, c: [u8; 3]) -> f32 {
        let Some((before, after)) = line.split_once(DC) else {
            return self.print(x, y, line, px, c);
        };
        let bw = self.print_measure(before, px);
        self.print(x, y, before, px, c);
        let dw = self.dc(x + bw + 4.0, y, px, c);
        self.print(x + bw + dw + 8.0, y, after, px, c)
    }

    fn wrap(&self, text: &str, px: f32, max_w: f32) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in text.split(' ').filter(|w| !w.is_empty()) {
            let next = match line.is_empty() {
                true => word.to_string(),
                false => format!("{line} {word}"),
            };
            if !line.is_empty() && self.measure_cased(&next, px) > max_w {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = next;
            }
        }
        lines.extend((!line.is_empty()).then_some(line));
        lines
    }
}

pub fn ds_sticker_face(f: &StickerFields) -> UndoFace {
    let (w, h) = (DS_STICKER_W, DS_STICKER_H);
    let mut c = Canvas::blank(w, h);
    let paper = bitmaps::sticker();
    let at = |x: u32, y: u32| -> [u8; 3] {
        let i = ((y * STICKER_W + x) * 4) as usize;
        [paper[i], paper[i + 1], paper[i + 2]]
    };
    let (gx, gy, gw, gh) = (12, 12, 170, 150);
    c.rounded_fill((0.0, 0.0, w as f32, h as f32, DS_RADIUS), |x, y| {
        at(gx + mirror(x, gw), gy + mirror(y, gh))
    });

    let m = DS_MARGIN;
    let right = w as f32 - m;
    let mut y = m;
    let mut head_bottom = y + DS_HEAD_PX * 1.6;
    let mut after_mark = m;
    if let Some((rgba, (mw, mh))) = wordmark(DS_MODEL) {
        let lift = WORDMARK_PX_PER_UNIT * LOCKUP_PAD;
        c.blit(
            (m - lift).max(0.0) as u32,
            (y - lift).max(0.0) as u32,
            &rgba,
            mw,
            mh,
        );
        after_mark = m - lift + mw as f32 + 6.0;
        head_bottom = y - lift + mh as f32 - lift;
    }
    let model_y = head_bottom - DS_HEAD_PX * 1.3;
    c.print(after_mark, model_y, DS_MODEL_NO, DS_HEAD_PX, WHITE);

    let pill_h = 24.0;
    let pill_px = 15.0;
    let pill_w = c.print_measure(DS_BADGE, pill_px) + 22.0;
    let pill_y = head_bottom - pill_h;
    c.rounded_ring(
        (right - pill_w, pill_y, pill_w, pill_h, pill_h / 2.0),
        2.0,
        WHITE,
    );
    c.print(
        right - pill_w + 11.0,
        pill_y + 2.0,
        DS_BADGE,
        pill_px,
        WHITE,
    );

    let rule_y = (head_bottom + 5.0).round();
    c.rect(m as u32, rule_y as u32, (right - m) as u32, 2, WHITE);
    y = rule_y + 6.0;
    let rows = ds_head_rows(f);
    let box_top = y + DS_HEAD_PX * 1.35;
    for row in &rows {
        y = c.print_spaced(m, y, row, DS_HEAD_PX, WHITE);
    }

    let mid = box_top + (y - box_top) / 2.0;
    let seal_px: f32 = 18.0;
    let seal_h = (seal_px * 1.45).round();
    let seal_y = (mid - seal_h / 2.0).round();
    let word_w = c.print_measure(DS_SEAL[1], seal_px);
    c.print(right - word_w, seal_y + 1.0, DS_SEAL[1], seal_px, WHITE);
    let box_w = (c.print_measure(DS_SEAL[0], seal_px) + 10.0).round();
    let box_x = (right - word_w - 6.0 - box_w).round();
    c.rect(
        box_x as u32,
        seal_y as u32,
        box_w as u32,
        seal_h as u32,
        WHITE,
    );
    c.print(box_x + 5.0, seal_y + 1.0, DS_SEAL[0], seal_px, BLACK);

    y += 1.0;
    for line in c.wrap(DS_CREDITS, DS_BODY_PX, right - m) {
        y = c.print_cased(m, y, &line, DS_BODY_PX, WHITE);
    }
    let dot = DS_HEAD_PX * 0.3;
    c.rounded_fill(
        (m + 1.0, y + DS_HEAD_PX * 0.62, dot, dot, dot / 2.0),
        |_, _| WHITE,
    );
    y = c.print(m + dot + 6.0, y, DS_SERVICE, DS_HEAD_PX, WHITE);

    let panel_x = m as u32;
    let panel_w = (right - m) as u32;
    let panel_y = (y + 6.0) as u32;
    let (px0, py0) = (
        (PANEL_FX * STICKER_W as f32) as u32 + 10,
        (PANEL_FY * STICKER_H as f32) as u32 + 10,
    );
    let (pw, ph) = (
        (PANEL_FW * STICKER_W as f32) as u32 - 20,
        (PANEL_FH * STICKER_H as f32) as u32 - 20,
    );
    c.rounded_fill(
        (
            panel_x as f32,
            panel_y as f32,
            panel_w as f32,
            DS_PANEL_H as f32,
            8.0,
        ),
        |x, y| at(px0 + x * pw / panel_w, py0 + y * ph / DS_PANEL_H),
    );
    let bars_y = panel_y + 14;
    let bars_h = 62;
    c.barcode(
        panel_x,
        panel_w,
        bars_y,
        bars_h,
        &format!("SLOT-{}", f.serial),
    );
    c.serial(panel_x, panel_w, (bars_y + bars_h + 4) as f32, f.serial);

    let foot_y = (panel_y + DS_PANEL_H) as f32 + 8.0;
    c.print(m, foot_y, ORIGIN[1], DS_HEAD_PX, WHITE);
    let origin_w = c.print_measure(ORIGIN[0], DS_HEAD_PX);
    c.print(right - origin_w, foot_y, ORIGIN[0], DS_HEAD_PX, WHITE);

    UndoFace { rgba: c.px, w, h }
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
    sticker_rect_sized(canvas, STICKER_W, STICKER_H)
}

pub fn sticker_rect_sized(canvas: slot_gfx::Canvas, w: u32, h: u32) -> (f32, f32, f32, f32) {
    let (w, h) = (w as f32, h as f32);
    let room = canvas.w - 2.0 * STICKER_MARGIN;
    let scale = (room / w).min(1.0);
    let (w, h) = ((w * scale).round(), (h * scale).round());
    (
        canvas.centred(w).round(),
        ((canvas.h - h) / 2.0).round(),
        w,
        h,
    )
}

pub fn draw_sticker(
    canvas: slot_gfx::Canvas,
    face: Option<(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {
    let Some((tex, fw, fh)) = face else {
        return;
    };
    let (x, y, w, h) = sticker_rect_sized(canvas, fw, fh);
    out.push(Draw::Tex {
        x,
        y,
        w,
        h,
        tex,
        alpha: 1.0,
    });
}
