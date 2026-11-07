use crate::layout::Canvas;
use crate::surface::{OUT_H, OUT_W};

const LINE_PX: f32 = 2.0;

const DOT_T: f32 = 0.12;

const OVERSHOOT: f32 = 0.6;

pub fn screen_scale(t: f32) -> f32 {
    screen_scale_over(OUT_H as f32, t)
}

fn screen_scale_over(h: f32, t: f32) -> f32 {
    let left = 1.0 - t.clamp(0.0, 1.0);
    1.0 - (1.0 - LINE_PX / h) * left * left
}

pub fn screen_width(t: f32) -> f32 {
    screen_width_over(OUT_W as f32, t)
}

fn screen_width_over(w: f32, t: f32) -> f32 {
    let left = 1.0 - (t.clamp(0.0, 1.0) / DOT_T).min(1.0);
    1.0 - (1.0 - LINE_PX / w) * left
}

pub fn screen_squash(w: f32, h: f32, t: f32) -> (f32, f32) {
    (screen_width_over(w, t), screen_scale_over(h, t))
}

pub fn screen_brightness(t: f32) -> f32 {
    let left = 1.0 - t.clamp(0.0, 1.0);
    1.0 + OVERSHOOT * left * left
}

/// The picture's place while a footer of `footer` px stays put along the bottom.
/// The scale is centred in the space above that strip, so the strip is not part of it.
pub fn screen_rect_above(canvas: Canvas, t: f32, footer: f32) -> (f32, f32, f32, f32) {
    let view = canvas.h - footer.max(0.0);
    let w = canvas.w * screen_width_over(canvas.w, t);
    let h = view * screen_scale_over(canvas.h, t);
    ((canvas.w - w) / 2.0, (view - h) / 2.0, w, h)
}
