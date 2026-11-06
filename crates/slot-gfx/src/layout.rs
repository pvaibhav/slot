use crate::surface::{OUT_H, OUT_W};

const MIN_W: u32 = 640;
const MIN_H: u32 = 480;

pub fn fills(panel: (u32, u32)) -> bool {
    panel.0 >= MIN_W && panel.1 >= MIN_H
}

pub fn canvas_scale(panel: (u32, u32)) -> u32 {
    let most = (panel.0 / MIN_W).min(panel.1 / MIN_H).max(1);
    (1..=most)
        .rev()
        .find(|k| panel.0.is_multiple_of(*k) && panel.1.is_multiple_of(*k))
        .unwrap_or(1)
}

const LAYOUTS: [((u32, u32), (u32, u32)); 1] = [((1024, 768), (800, 600))];

pub fn panel_pixels(panel: (u32, u32)) -> (u32, u32) {
    let k = canvas_scale(panel);
    (panel.0 / k, panel.1 / k)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Canvas {
    pub w: f32,
    pub h: f32,
}

impl Canvas {
    pub const FRAMED: Canvas = Canvas {
        w: OUT_W as f32,
        h: OUT_H as f32,
    };

    pub fn of(panel: (u32, u32)) -> Canvas {
        if !fills(panel) {
            return Canvas::FRAMED;
        }
        let pixels = panel_pixels(panel);
        let (w, h) = LAYOUTS
            .iter()
            .find(|(p, _)| *p == pixels)
            .map_or(pixels, |(_, layout)| *layout);
        Canvas {
            w: w as f32,
            h: h as f32,
        }
    }

    pub fn inset_y(self) -> f32 {
        (self.h - OUT_H as f32) / 2.0
    }

    pub fn centred(self, w: f32) -> f32 {
        (self.w - w) / 2.0
    }

    pub fn window(self, tex_w: f32) -> [f32; 4] {
        if tex_w <= self.w {
            return [0.0, 0.0, 1.0, 1.0];
        }
        [(tex_w - self.w) / 2.0 / tex_w, 0.0, self.w / tex_w, 1.0]
    }

    pub fn cover(self, tex_w: f32) -> [f32; 4] {
        if tex_w >= self.w {
            return self.window(tex_w);
        }
        let shown = tex_w / self.w;
        [0.0, (1.0 - shown) / 2.0, 1.0, shown]
    }
}
