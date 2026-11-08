use slot_gfx::{Canvas, Draw, TexId};

use crate::footer::Printed;
use crate::plate::TITLE_LINE_H;
use crate::slot_chrome::MOUTH_H;

const BOX_H: f32 = 0.82;
const MARGIN: f32 = 0.06;
const LOGO_W: f32 = 0.42;
const LOGO_H: f32 = 0.32;
const GAP: f32 = 16.0;
const SYSTEM_GAP: f32 = 4.0;
const SYSTEM_ALPHA: f32 = 0.6;

pub struct BoxScreen {
    pub canvas: Canvas,
    pub boxed: Option<(TexId, u32, u32)>,
    pub logo: Option<(TexId, u32, u32)>,
    pub title: Printed,
    pub system: Printed,
}

impl BoxScreen {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let area = self.canvas.h - MOUTH_H;
        let margin = self.canvas.w * MARGIN;
        let column_x = match self.boxed {
            Some((tex, w, h)) if w > 0 && h > 0 => {
                let bh = area * BOX_H;
                let bw = bh * w as f32 / h as f32;
                out.push(Draw::Tex {
                    x: margin,
                    y: (area - bh) / 2.0,
                    w: bw,
                    h: bh,
                    tex,
                    alpha: 1.0,
                });
                margin + bw
            }
            _ => 0.0,
        };
        let centre = (column_x + self.canvas.w) / 2.0;
        let logo = self
            .logo
            .filter(|&(_, w, h)| w > 0 && h > 0)
            .map(|(tex, w, h)| {
                let fit = (self.canvas.w * LOGO_W / w as f32).min(area * LOGO_H / h as f32);
                (tex, w as f32 * fit, h as f32 * fit)
            });
        let line = TITLE_LINE_H as f32;
        let logo_h = logo.map_or(0.0, |(_, _, h)| h + GAP);
        let system_h = self.system.face.map_or(0.0, |_| SYSTEM_GAP + line);
        let mut y = (area - (logo_h + line + system_h)) / 2.0;
        if let Some((tex, w, h)) = logo {
            out.push(Draw::Tex {
                x: centre - w / 2.0,
                y,
                w,
                h,
                tex,
                alpha: 1.0,
            });
            y += h + GAP;
        }
        for (printed, alpha) in [(self.title, 1.0), (self.system, SYSTEM_ALPHA)] {
            let Some(tex) = printed.face else {
                continue;
            };
            let w = printed.w as f32;
            out.push(Draw::Tex {
                x: centre - w / 2.0,
                y,
                w,
                h: line,
                tex,
                alpha,
            });
            y += line + SYSTEM_GAP;
        }
    }
}
