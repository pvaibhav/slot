use crate::plate::{word_ink, UndoFace};

const PAPER: [u8; 3] = [0xf4, 0xf0, 0xe6];
const INK: [u8; 3] = [120, 108, 88];
const TILE_W: f32 = 46.0;
const TILE_H: f32 = 24.0;
const SHRINK: f32 = 0.6;

pub fn liner_face(w: u32, h: u32) -> UndoFace {
    let word = word_ink("slot.", INK);
    let (sin, cos) = (-0.35f32).sin_cos();
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let (tx, ty) = (
                (x as f32).rem_euclid(TILE_W) - TILE_W / 2.0,
                (y as f32).rem_euclid(TILE_H) - TILE_H / 2.0,
            );
            let (rx, ry) = (tx * cos + ty * sin, -tx * sin + ty * cos);
            let (sx, sy) = (
                rx / SHRINK + word.w as f32 / 2.0,
                ry / SHRINK + word.h as f32 / 2.0,
            );
            let inside = sx >= 0.0 && sy >= 0.0 && sx < word.w as f32 && sy < word.h as f32;
            let cover = match inside {
                true => {
                    let i = (sy as u32 * word.w + sx as u32) as usize * 4 + 3;
                    word.rgba[i] as f32 / 255.0 * 0.22
                }
                false => 0.0,
            };
            for c in 0..3 {
                rgba.push((PAPER[c] as f32 + (INK[c] as f32 - PAPER[c] as f32) * cover) as u8);
            }
            rgba.push(255);
        }
    }
    UndoFace { rgba, w, h }
}
