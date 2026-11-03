use qrcode::{Color, EcLevel, QrCode};

pub const QUIET: u32 = 4;

pub fn qr_rgba(url: &str, target: u32) -> Option<(Vec<u8>, u32)> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M).ok()?;
    let width = code.width() as u32;
    let total = width + 2 * QUIET;
    let scale = (target / total).max(1);
    let side = total * scale;
    let colors = code.to_colors();
    let mut rgba = vec![0xffu8; (side * side * 4) as usize];
    for y in 0..side {
        for x in 0..side {
            let (mx, my) = (x / scale, y / scale);
            if mx < QUIET || my < QUIET || mx >= QUIET + width || my >= QUIET + width {
                continue;
            }
            let dark = colors[((my - QUIET) * width + (mx - QUIET)) as usize] == Color::Dark;
            if dark {
                let at = ((y * side + x) * 4) as usize;
                rgba[at..at + 3].fill(0x00);
            }
        }
    }
    Some((rgba, side))
}
