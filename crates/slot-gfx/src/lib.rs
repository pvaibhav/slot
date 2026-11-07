macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod bezel;
mod draw;
mod ds;
mod fbdev;
mod fbo;
mod gl;
mod grade;
#[cfg(target_os = "macos")]
mod headless;
#[cfg(feature = "host")]
mod host;
#[cfg(unix)]
mod kms;
#[cfg(unix)]
mod kms_atomic;
mod layout;
mod lcd3x;
mod overlay;
mod pipeline;
mod placement;
mod power;
mod quad;
mod shaders;
mod surface;

pub use bezel::{bezel_column, framed, game_rect, hole_on_top, lifted_rect, screen_rect};
pub use ds::{
    ds_rect, ds_rect_powered, fps_text, DsOverlay, DsPass, DsScreen, DS_SCREEN_H, DS_SCREEN_W,
};
pub use draw::{paper_bounds, paper_flap_shows, Draw, TexId};
pub use fbdev::{egl_error, panel_mode, panel_size, FbdevSurface};
pub use fbo::{canvas_size, Compositor, BACKDROP};
pub use grade::{
    blue_light_gain, contrast_boost, ColorGrade, dim_gain, saturation_boost, BLUE_LIGHT_MAX, CONTRAST_MAX,
    SATURATION_MAX,
};
#[cfg(target_os = "macos")]
pub use headless::HeadlessSurface;
#[cfg(feature = "host")]
pub use host::HostSurface;
#[cfg(unix)]
pub use kms_atomic::{atomic_plan, primary_plane_for, AtomicIds, PlaneInfo, PLANE_TYPE_PRIMARY};
#[cfg(unix)]
pub use kms::{
    free_crtc, preferred_mode, retry_until, KmsSurface, Panel, CONNECTOR_DSI, MODE_TYPE_PREFERRED,
};
pub use layout::{canvas_scale, fills, panel_pixels, Canvas};
pub use lcd3x::{lcd3x_factors, lcd3x_mask, ScreenEffect};
pub use overlay::{Overlay, OverlayPass};
pub use pipeline::{SCALE, SRC_H, SRC_W, WHOLE_TEXTURE};
pub use placement::{game_layout, table_placement, GameLayout, Sizing};
pub use power::{screen_brightness, screen_rect_above, screen_scale, screen_width};
pub use surface::{
    blit_rect, blit_rect_fit, fit_rect, fit_scale, mode_period, GfxError, Surface, OUT_H, OUT_W,
};
