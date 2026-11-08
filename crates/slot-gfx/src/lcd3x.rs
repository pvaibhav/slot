use std::f32::consts::PI;

const BRIGHTEN_SCANLINES: f32 = 16.0;
const BRIGHTEN_LCD: f32 = 4.0;
const OFFSETS: [f32; 3] = [PI / 2.0, PI * (0.5 - 2.0 / 3.0), PI * (0.5 - 4.0 / 3.0)];

pub fn lcd3x_factors(x: f32, y: f32) -> [f32; 3] {
    let yfactor = (BRIGHTEN_SCANLINES + (2.0 * PI * y).sin()) / (BRIGHTEN_SCANLINES + 1.0);
    OFFSETS.map(|o| yfactor * (BRIGHTEN_LCD + (2.0 * PI * x + o).sin()) / (BRIGHTEN_LCD + 1.0))
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ScreenEffect {
    None,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
    Shimmerless,
    Authentic,
    LcdGrid,
}

impl ScreenEffect {
    pub fn mode(self) -> f32 {
        self as u8 as f32
    }

    pub fn lcd_for(panel: (u32, u32)) -> ScreenEffect {
        let (w, h) = panel;
        match w > 0 && w * 2 == h * 3 {
            true => ScreenEffect::Lcd3x,
            false => ScreenEffect::LcdGrid,
        }
    }
}

pub fn lcd3x_mask() -> [[[f32; 3]; 3]; 3] {
    let mut mask = [[[0.0f32; 3]; 3]; 3];
    for (oy, row) in mask.iter_mut().enumerate() {
        for (ox, cell) in row.iter_mut().enumerate() {
            *cell = lcd3x_factors((ox as f32 + 0.5) / 3.0, (oy as f32 + 0.5) / 3.0);
        }
    }
    mask
}
