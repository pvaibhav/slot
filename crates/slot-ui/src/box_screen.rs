use slot_gfx::{Canvas, Draw, TexId};

use crate::slot_chrome::MOUTH_H;

const BOX_H: f32 = 0.82;
const MARGIN: f32 = 0.06;
const LOGO_W: f32 = 0.42;
const LOGO_H: f32 = 0.32;
const GAME_DIM: f32 = 0.8;

pub struct BoxScreen {
    pub canvas: Canvas,
    pub boxed: Option<(TexId, u32, u32)>,
    pub logo: Option<(TexId, u32, u32)>,
    pub over_game: bool,
}

impl BoxScreen {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        if self.over_game {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: self.canvas.w,
                h: self.canvas.h,
                colour: [0.0, 0.0, 0.0, GAME_DIM],
            });
        }
        let area = self.canvas.h - MOUTH_H;
        let margin = self.canvas.w * MARGIN;
        let box_h = area * BOX_H;
        let bottom = (area + box_h) / 2.0;
        let column_x = match self.boxed {
            Some((tex, w, h)) if w > 0 && h > 0 => {
                let bw = box_h * w as f32 / h as f32;
                out.push(Draw::Tex {
                    x: margin,
                    y: bottom - box_h,
                    w: bw,
                    h: box_h,
                    tex,
                    alpha: 1.0,
                });
                margin + bw
            }
            _ => 0.0,
        };
        let centre = (column_x + self.canvas.w) / 2.0;
        if let Some((tex, w, h)) = self.logo.filter(|&(_, w, h)| w > 0 && h > 0) {
            let fit = (self.canvas.w * LOGO_W / w as f32).min(area * LOGO_H / h as f32);
            let (lw, lh) = (w as f32 * fit, h as f32 * fit);
            out.push(Draw::Tex {
                x: centre - lw / 2.0,
                y: bottom - (box_h + lh) / 2.0,
                w: lw,
                h: lh,
                tex,
                alpha: 1.0,
            });
        }
    }
}
