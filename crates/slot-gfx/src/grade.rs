const WARMEST: [f32; 3] = [1.0, 0.82, 0.62];

pub const BLUE_LIGHT_MAX: u8 = 9;

/// On the device this precedes the fixed RGSP hardware LUT: the panel receives
/// LUT(colour * gain). Step zero keeps panel calibration, with no extra warming.
pub fn blue_light_gain(step: u8) -> [f32; 3] {
    let t = step.min(BLUE_LIGHT_MAX) as f32 / BLUE_LIGHT_MAX as f32;
    let mut gain = [1.0f32; 3];
    for (g, warm) in gain.iter_mut().zip(WARMEST) {
        *g = 1.0 + (warm - 1.0) * t;
    }
    gain
}
