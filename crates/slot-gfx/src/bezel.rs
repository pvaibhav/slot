use crate::surface::{OUT_H, OUT_W};

pub fn framed(panel: (u32, u32)) -> bool {
    let c = crate::layout::Canvas::of(panel);
    crate::layout::fills(panel) && c.w == 640.0 && c.h == 480.0
}

pub fn screen_rect(panel: (u32, u32)) -> (i32, i32, i32, i32) {
    let w = panel.0;
    let h = (w * OUT_H + OUT_W / 2) / OUT_W;
    ((panel.0 - w) as i32 / 2, 0, w as i32, h as i32)
}

pub fn game_rect(panel: (u32, u32), sp_frame: bool) -> (i32, i32, i32, i32) {
    let (x, _, w, h) = screen_rect(panel);
    let gap = panel.1 as i32 - h;
    let y = match sp_frame {
        true => 0,
        false => gap - gap / 2,
    };
    (x, y, w, h)
}

pub fn bezel_column(rgba: &[u8], panel: (u32, u32)) -> Vec<u8> {
    let (w, h) = (panel.0 as usize, panel.1 as usize);
    if !framed(panel) || rgba.len() < w * h * 4 {
        return Vec::new();
    }
    (0..h).map(|y| rgba[(y * w + w / 2) * 4 + 3]).collect()
}

pub fn hole_on_top(column: &[u8], panel: (u32, u32), picture: (f32, f32)) -> bool {
    let h = panel.1 as usize;
    if !framed(panel) || column.len() != h {
        return false;
    }
    let game_h = screen_rect(panel).3 as f32;
    let lift = (h - game_h as usize) - (h - game_h as usize) / 2;
    let bottom = lift + ((picture.0 + picture.1) * game_h) as usize;
    (bottom - lift + 1..bottom)
        .chain([h - 1])
        .all(|y| column[y] >= 128)
}

pub fn lifted_rect(panel: (u32, u32), lift: f32) -> (i32, i32, i32, i32) {
    let (x, _, w, h) = screen_rect(panel);
    let t = lift.clamp(0.0, 1.0);
    let eased = t * t * (3.0 - 2.0 * t);
    let drop = ((panel.1 as i32 - h) as f32 * (1.0 - eased)).round() as i32;
    (x, drop, w, h)
}
