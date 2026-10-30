use slot_gfx::{Canvas, Draw, TexId, OUT_H};
use slot_power::Battery;

use crate::battery::{draw_gauge, GAUGE_H, GAUGE_ROW_W};
use crate::hud::{HUD_INK, PLATE_H};
use crate::plate::HINT_H;
use crate::slot_chrome::MOUTH_H;

pub const FOOTER_Y: f32 = OUT_H as f32 - MOUTH_H + (MOUTH_H - HINT_H as f32) / 2.0;
const FOOTER_MARGIN: f32 = 24.0;
const CORNER_MARGIN: f32 = 16.0;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Printed {
    pub face: Option<TexId>,
    pub w: u32,
}

impl Printed {
    pub fn new(face: TexId, w: u32) -> Self {
        Printed {
            face: Some(face),
            w,
        }
    }
}

pub fn draw_footer(
    canvas: Canvas,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    ink: [u8; 3],
    out: &mut Vec<Draw>,
) {
    let y = FOOTER_Y + (HINT_H as f32 - GAUGE_H) / 2.0;
    draw_gauge(FOOTER_MARGIN, y, battery, percent, bolt, ink, out);
    printed(canvas.w - FOOTER_MARGIN - clock.w as f32, clock, out);
}

const WIFI_GLYPH: [&str; 10] = [
    "....3333333....",
    "..333.....333..",
    ".33.........33.",
    "33...22222...33",
    "...222...222...",
    "..22.......22..",
    "......111......",
    ".....11.11.....",
    "......111......",
    ".......1.......",
];
const WIFI_DIM: f32 = 0.3;
const WIFI_GAP: f32 = 12.0;

pub fn draw_wifi_icon(percent: Printed, ink: [u8; 3], bars: u8, out: &mut Vec<Draw>) {
    let x = FOOTER_MARGIN + GAUGE_ROW_W + percent.w as f32 + WIFI_GAP;
    let top = FOOTER_Y + (HINT_H as f32 - WIFI_GLYPH.len() as f32) / 2.0;
    let colour = ink.map(|c| c as f32 / 255.0);
    let [r, g, b] = colour;
    for (row, line) in WIFI_GLYPH.iter().enumerate() {
        for (col, cell) in line.bytes().enumerate() {
            if cell != b'.' {
                let alpha = if cell - b'0' <= bars { 1.0 } else { WIFI_DIM };
                out.push(Draw::Rect {
                    x: x + col as f32,
                    y: top + row as f32,
                    w: 1.0,
                    h: 1.0,
                    colour: [r, g, b, alpha],
                });
            }
        }
    }
}

pub fn draw_corners(
    canvas: Canvas,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    out: &mut Vec<Draw>,
) {
    let y = (PLATE_H - GAUGE_H) / 2.0;
    draw_gauge(CORNER_MARGIN, y, battery, percent, bolt, HUD_INK, out);
    let x = canvas.w - CORNER_MARGIN - clock.w as f32;
    draw_printed(x, (PLATE_H - HINT_H as f32) / 2.0, clock, out);
}

pub(crate) fn draw_printed(x: f32, y: f32, p: Printed, out: &mut Vec<Draw>) {
    if p.w == 0 {
        return;
    }
    let (w, h) = (p.w as f32, HINT_H as f32);
    out.push(match p.face {
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
            colour: [1.0, 1.0, 1.0, 0.08],
        },
    });
}

fn printed(x: f32, p: Printed, out: &mut Vec<Draw>) {
    draw_printed(x, FOOTER_Y, p, out);
}
