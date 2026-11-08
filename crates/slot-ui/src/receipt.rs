use crate::plate::UndoFace;
use crate::quick_menu::print_stop_legend;
use crate::text;

const PW: usize = 226;
const PH: usize = 425;
const CUT: usize = 407;
const TOP_PAD: usize = 40;
const MODULE: usize = 7;
const FIT_W: f32 = 204.0;
const MIN_PX: f32 = 8.0;
const STOP_Y: usize = 368;
const STOP_LIGHT: f32 = 0.62;
const STOP_DARK: f32 = 0.5;
const TOOTH_W: usize = 6;
const TOOTH_H: f32 = 5.0;
const PAPER: [f32; 3] = [0.945, 0.955, 0.945];
const INK: [f32; 3] = [0.10, 0.145, 0.125];

pub struct ReceiptText {
    pub host: Option<String>,
    pub ip: String,
    pub pin: String,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 + 0.5) / 16_777_216.0
    }

    fn gauss(&mut self) -> f32 {
        let a = self.next();
        let b = self.next();
        (-2.0 * a.ln()).sqrt() * (std::f32::consts::TAU * b).cos()
    }

    fn field(&mut self, n: usize) -> Vec<f32> {
        (0..n).map(|_| self.gauss()).collect()
    }
}

fn kernel(sigma: f32) -> Vec<f32> {
    let r = (sigma * 3.0).ceil() as i32;
    let mut k: Vec<f32> = (-r..=r)
        .map(|i| (-(i * i) as f32 / (2.0 * sigma * sigma)).exp())
        .collect();
    let sum: f32 = k.iter().sum();
    k.iter_mut().for_each(|v| *v /= sum);
    k
}

fn blur_line(src: &[f32], dst: &mut [f32], stride: usize, n: usize, k: &[f32]) {
    let r = (k.len() / 2) as i32;
    for i in 0..n as i32 {
        let mut acc = 0.0;
        for (j, w) in k.iter().enumerate() {
            let at = (i + j as i32 - r).clamp(0, n as i32 - 1) as usize;
            acc += src[at * stride] * w;
        }
        dst[i as usize * stride] = acc;
    }
}

fn blur(buf: &[f32], w: usize, h: usize, sigma: f32) -> Vec<f32> {
    let k = kernel(sigma);
    let mut mid = vec![0.0; w * h];
    for y in 0..h {
        blur_line(&buf[y * w..], &mut mid[y * w..], 1, w, &k);
    }
    let mut out = vec![0.0; w * h];
    for x in 0..w {
        blur_line(&mid[x..], &mut out[x..], w, h, &k);
    }
    out
}

struct Line {
    text: String,
    px: f32,
    gap: usize,
}

fn fitted(font: &fontdue::Font, text: &str, px: f32, fit_w: f32, min_px: f32) -> f32 {
    let mut px = px;
    while px > min_px && text::line_width(font, text, px, 0.0) > fit_w {
        px -= 0.5;
    }
    px
}

fn fits(line: &str) -> bool {
    text::label_font().is_none_or(|font| text::line_width(font, &line.to_uppercase(), MIN_PX, 0.0) <= FIT_W)
}

fn raster(lines: &[Line], top: usize, w: usize, h: usize, fit_w: f32, min_px: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    let Some(font) = text::label_font() else {
        return out;
    };
    let mut y = top as f32;
    for line in lines {
        let px = fitted(font, &line.text, line.px, fit_w, min_px);
        let Some(vm) = font.horizontal_line_metrics(px) else {
            continue;
        };
        let base = y + vm.ascent;
        let mut pen = (w as f32 - text::line_width(font, &line.text, px, 0.0)) / 2.0;
        for ch in line.text.chars() {
            let (m, cov) = font.rasterize(ch, px);
            let gx = (pen + m.xmin as f32).round() as i32;
            let gy = (base - (m.height as f32 + m.ymin as f32)).round() as i32;
            for r in 0..m.height {
                for c in 0..m.width {
                    let (dx, dy) = (gx + c as i32, gy + r as i32);
                    if dx < 0 || dy < 0 || dx >= w as i32 || dy >= h as i32 {
                        continue;
                    }
                    let at = dy as usize * w + dx as usize;
                    out[at] = out[at].max(cov[r * m.width + c] as f32 / 255.0);
                }
            }
            pen += m.advance_width;
        }
        y += line.gap as f32;
    }
    out
}

fn type_ink(lines: &[Line], top: usize) -> Vec<f32> {
    let smooth = raster(lines, top, PW, CUT, FIT_W, MIN_PX);
    let half: Vec<Line> = lines
        .iter()
        .map(|l| Line {
            text: l.text.clone(),
            px: l.px / 2.0,
            gap: l.gap / 2,
        })
        .collect();
    let low = raster(&half, top / 2, PW / 2, CUT / 2 + 1, FIT_W / 2.0, MIN_PX / 2.0);
    let lw = PW / 2;
    let mut out = vec![0.0f32; PW * PH];
    for (i, v) in smooth.iter().enumerate() {
        out[i] = *v * 0.5;
    }
    for (i, v) in low.iter().enumerate() {
        if *v > 90.0 / 255.0 {
            let (lx, ly) = (i % lw, i / lw);
            for (oy, ox) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                let at = (ly * 2 + oy) * PW + lx * 2 + ox;
                if at < PW * CUT {
                    out[at] += 0.5;
                }
            }
        }
    }
    out
}

fn stamp_stop(typed: &mut [f32]) {
    let legend = print_stop_legend([0, 0, 0]);
    let (w, h) = (legend.w as usize, legend.h as usize);
    let ox = PW.saturating_sub(w) / 2;
    for y in 0..h {
        for x in 0..w.min(PW) {
            let px = &legend.rgba[(y * w + x) * 4..][..4];
            let lum = (px[0] as f32 + px[1] as f32 + px[2] as f32) / 765.0;
            let at = (STOP_Y + y) * PW + ox + x;
            let dark = ((STOP_LIGHT - lum) / (STOP_LIGHT - STOP_DARK)).clamp(0.0, 1.0);
            typed[at] = typed[at].max(dark * px[3] as f32 / 255.0);
        }
    }
}

fn tooth_cut(x: usize, y: usize) -> bool {
    let t = (x % TOOTH_W) as f32 + 0.5;
    let depth = TOOTH_H * (1.0 - (t - 3.0).abs() / 3.0);
    let from_top = y as f32 + 0.5;
    let from_bottom = PH as f32 - from_top;
    from_top < depth || from_bottom < depth
}

fn print(ink: &[f32], type_ink: &[f32]) -> UndoFace {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ 3);
    let head_noise = {
        let row = rng.field(PW);
        let mut out = vec![0.0; PW];
        blur_line(&row, &mut out, 1, PW, &kernel(1.2));
        out
    };
    let phase = rng.next() * 3.0;
    let mottle = blur(&rng.field(PW * PH), PW, PH, 0.8);
    let grain = blur(&rng.field(PW * PH), PW, PH, 1.1);
    let mut rgba = vec![0u8; PW * PH * 4];
    for y in 0..PH {
        let v = y as f32 / (PH - 1) as f32;
        let curl = 1.0 - 0.09 * (-(v / 0.055).powi(2)).exp() - 0.05 * (-((1.0 - v) / 0.045).powi(2)).exp();
        let band = 0.97 + 0.03 * (y as f32 / 3.7 + phase).sin().signum();
        for (x, head_n) in head_noise.iter().enumerate() {
            let at = y * PW + x;
            if tooth_cut(x, y) {
                continue;
            }
            let head = 0.95 + 0.05 * head_n * 2.2;
            let grid = if y % 2 == 0 || x % 2 == 0 { 1.0 } else { 0.0 };
            let mot = 1.0 + mottle[at] * 0.16;
            let press = (head * band * mot * (0.90 + 0.10 * grid)).clamp(0.0, 1.0);
            let u = (x as f32 / (PW - 1) as f32 - 0.5) * 2.0;
            let roll = 0.90 + 0.10 * (u * 1.35).cos();
            let sheen = 0.075 * (-((u + 0.34) / 0.20).powi(2)).exp();
            let press_t = 1.0 - (1.0 - press) * 2.0 / 3.0;
            let a = (ink[at] * press).max(type_ink[at] * press_t);
            for c in 0..3 {
                let paper = PAPER[c] * (1.0 + grain[at] * 0.012);
                let col = paper * (1.0 - a) + INK[c] * a;
                let col = col * roll * curl + sheen;
                rgba[at * 4 + c] = (col.clamp(0.0, 1.0) * 255.0) as u8;
            }
            rgba[at * 4 + 3] = 255;
        }
    }
    UndoFace {
        rgba,
        w: PW as u32,
        h: PH as u32,
    }
}

fn stamp_code(ink: &mut [f32], modules: &[Vec<bool>]) -> usize {
    let n = modules.len();
    let ox = PW.saturating_sub(n * MODULE) / 2;
    for (r, row) in modules.iter().enumerate() {
        for (c, on) in row.iter().enumerate() {
            if !*on {
                continue;
            }
            for dy in 0..MODULE {
                for dx in 0..MODULE {
                    let (x, y) = (ox + c * MODULE + dx, TOP_PAD + r * MODULE + dy);
                    if x < PW && y < CUT {
                        ink[y * PW + x] = 1.0;
                    }
                }
            }
        }
    }
    n * MODULE
}

pub fn receipt_face(modules: &[Vec<bool>], text: &ReceiptText) -> UndoFace {
    let mut ink = vec![0.0; PW * PH];
    let side = stamp_code(&mut ink, modules);
    let mut lines = vec![Line {
        text: "SCAN OR GO TO".into(),
        px: 18.0,
        gap: 24,
    }];
    if let Some(host) = text.host.as_ref().filter(|h| fits(h)) {
        lines.push(Line {
            text: host.to_uppercase(),
            px: 26.0,
            gap: 28,
        });
    }
    lines.push(Line {
        text: text.ip.to_uppercase(),
        px: 22.0,
        gap: 28,
    });
    lines.push(Line {
        text: format!("PIN  {}", text.pin),
        px: 24.0,
        gap: 0,
    });
    let mut typed = type_ink(&lines, (TOP_PAD + side + 12) / 2 * 2);
    stamp_stop(&mut typed);
    print(&ink, &typed)
}

pub fn receipt_blank(lines: &[&str]) -> UndoFace {
    let rows: Vec<Line> = lines
        .iter()
        .map(|l| Line {
            text: l.to_uppercase(),
            px: 24.0,
            gap: 32,
        })
        .collect();
    let mut typed = type_ink(&rows, TOP_PAD);
    stamp_stop(&mut typed);
    print(&vec![0.0; PW * PH], &typed)
}
