const WARMEST: [f32; 3] = [1.0, 0.82, 0.62];

pub const BLUE_LIGHT_MAX: u8 = 9;
pub const SATURATION_MAX: u8 = 9;
pub const CONTRAST_MAX: u8 = 9;

const DIM_GAINS: [f32; 7] = [1.0, 0.8, 0.65, 0.52, 0.42, 0.33, 0.25];

const SATURATION_TOP: f32 = 0.6;
const CONTRAST_TOP: f32 = 0.2;

pub fn saturation_boost(step: u8) -> f32 {
    SATURATION_TOP * step.min(SATURATION_MAX) as f32 / SATURATION_MAX as f32
}

pub fn contrast_boost(step: u8) -> f32 {
    CONTRAST_TOP * step.min(CONTRAST_MAX) as f32 / CONTRAST_MAX as f32
}

pub fn dim_gain(step: u8) -> f32 {
    DIM_GAINS[usize::from(step).min(DIM_GAINS.len() - 1)]
}

pub fn blue_light_gain(step: u8) -> [f32; 3] {
    let t = step.min(BLUE_LIGHT_MAX) as f32 / BLUE_LIGHT_MAX as f32;
    let mut gain = [1.0f32; 3];
    for (g, warm) in gain.iter_mut().zip(WARMEST) {
        *g = 1.0 + (warm - 1.0) * t;
    }
    gain
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorGrade {
    pub gain: [f32; 3],
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for ColorGrade {
    fn default() -> Self {
        ColorGrade {
            gain: [1.0; 3],
            saturation: 0.0,
            contrast: 0.0,
        }
    }
}
