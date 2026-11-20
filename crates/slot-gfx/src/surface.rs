use std::ffi::c_void;
use std::fmt;

pub const OUT_W: u32 = 720;
pub const OUT_H: u32 = 480;

#[derive(Debug)]
pub enum GfxError {
    Context(String),
    Shader(String),
    Framebuffer(u32),
}

impl fmt::Display for GfxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GfxError::Context(m) => write!(f, "gl context: {m}"),
            GfxError::Shader(m) => write!(f, "shader: {m}"),
            GfxError::Framebuffer(s) => write!(f, "framebuffer incomplete: 0x{s:x}"),
        }
    }
}

impl std::error::Error for GfxError {}

pub trait Surface {
    fn make_current(&mut self) -> Result<(), GfxError>;
    fn window_size(&self) -> (u32, u32);
    fn swap(&mut self) -> Result<(), GfxError>;
    fn proc_address(&self, name: &str) -> *const c_void;
    fn panel_count(&self) -> usize {
        1
    }
    fn refresh_period(&self) -> Option<std::time::Duration> {
        None
    }
    fn flip_times_us(&self) -> Option<[u64; 2]> {
        None
    }
    fn last_vblank(&self) -> Option<std::time::Instant> {
        None
    }

    fn select_panel(&mut self, _index: usize) -> Result<(), GfxError> {
        Ok(())
    }
}

pub fn fit_scale(win_w: u32, win_h: u32) -> u32 {
    (win_w / OUT_W).min(win_h / OUT_H).max(1)
}

pub fn fit_rect(win_w: u32, win_h: u32) -> (i32, i32, i32, i32) {
    let s = fit_scale(win_w, win_h) as i32;
    let (w, h) = (OUT_W as i32 * s, OUT_H as i32 * s);
    ((win_w as i32 - w) / 2, (win_h as i32 - h) / 2, w, h)
}

pub fn blit_rect_fit(panel: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    let scale = (panel.0 as f32 / OUT_W as f32).min(panel.1 as f32 / OUT_H as f32);
    let w = (OUT_W as f32 * scale).round() as i32;
    let h = (OUT_H as f32 * scale).round() as i32;
    let dx = (shake * scale).round() as i32;
    (
        (panel.0 as i32 - w) / 2 + dx,
        (panel.1 as i32 - h) / 2,
        w,
        h,
    )
}

pub fn blit_rect(window: (u32, u32), shake: f32) -> (i32, i32, i32, i32) {
    if crate::bezel::framed(window) {
        let (x, y, w, h) = crate::bezel::screen_rect(window);
        let dx = shake * w as f32 / OUT_W as f32;
        return (x + dx.round() as i32, window.1 as i32 - y - h, w, h);
    }
    if window.0 < OUT_W || window.1 < OUT_H {
        return blit_rect_fit(window, shake);
    }
    let (x, y, w, h) = fit_rect(window.0, window.1);
    let dx = shake * fit_scale(window.0, window.1) as f32;
    (x + dx.round() as i32, y, w, h)
}

pub fn mode_period(clock_khz: u32, htotal: u16, vtotal: u16) -> Option<std::time::Duration> {
    let dots = htotal as u64 * vtotal as u64;
    (clock_khz > 0 && dots > 0)
        .then(|| std::time::Duration::from_nanos(dots * 1_000_000 / clock_khz as u64))
}
