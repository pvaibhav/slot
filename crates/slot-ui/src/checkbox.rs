use crate::cart::CartFace;
use crate::text;

const LIGHT: [u8; 4] = [246, 244, 239, 255];
const BADGE: u32 = 30;

pub fn count_badge_face(n: usize) -> CartFace {
    let mut rgba = vec![0u8; (BADGE * BADGE * 4) as usize];
    let r = BADGE as f32 / 2.0;
    for py in 0..BADGE {
        for px in 0..BADGE {
            let (dx, dy) = (px as f32 + 0.5 - r, py as f32 + 0.5 - r);
            let edge = (r - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            if edge <= 0.0 {
                continue;
            }
            let o = ((py * BADGE + px) * 4) as usize;
            rgba[o..o + 3].copy_from_slice(&LIGHT[..3]);
            rgba[o + 3] = (edge * 255.0) as u8;
        }
    }
    if let Some(font) = text::label_font() {
        let layout = text::fit(font, &n.to_string(), BADGE as f32 - 6.0, 1, 16.0, 10.0);
        text::draw_centred(&mut rgba, BADGE, BADGE, &layout, [26, 25, 23]);
    }
    CartFace {
        rgba,
        w: BADGE,
        h: BADGE,
    }
}
