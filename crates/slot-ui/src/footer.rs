use slot_gfx::{Canvas, Draw, TexId, OUT_H};
use slot_power::Battery;

use crate::battery::{draw_gauge, GAUGE_H, GAUGE_ROW_W};
use crate::hud::{HUD_INK, PLATE_H};
use crate::plate::HINT_H;
use crate::slot_chrome::MOUTH_H;
use crate::CartFace;

pub const SYNC_PX: u32 = 18;
const SYNC_GAP: f32 = 14.0;

/// Two small circular arrows, rasterised once by the achievement UI worker.
pub fn sync_icon_face(light: bool) -> CartFace {
    let [r, g, b] = crate::footer_ink(light);
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
      <g fill="none" stroke="#{r:02x}{g:02x}{b:02x}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 10a7.2 7.2 0 0 1 12.2-3.2L20 10M20 5v5h-5"/>
        <path d="M19 14a7.2 7.2 0 0 1-12.2 3.2L4 14M4 19v-5h5"/>
      </g>
    </svg>"##
    );
    CartFace {
        rgba: crate::art::render_svg(&svg, SYNC_PX, SYNC_PX)
            .unwrap_or_else(|| vec![0; (SYNC_PX * SYNC_PX * 4) as usize]),
        w: SYNC_PX,
        h: SYNC_PX,
    }
}

#[derive(Clone, Copy)]
pub struct SyncIndicator {
    pub face: TexId,
    pub turn: f32,
    pub alpha: f32,
    pub attention: bool,
    pub progress: Option<TexId>,
}

/// Shares the clock's baseline and preserves a gap even at the widest clock label.
pub fn draw_footer_sync(canvas: Canvas, clock: Printed, icon: SyncIndicator, out: &mut Vec<Draw>) {
    let right = canvas.w - FOOTER_MARGIN - clock.w as f32 - SYNC_GAP;
    let x = right - SYNC_PX as f32 - if icon.progress.is_some() { 42.0 } else { 0.0 };
    if let Some(tex) = icon.progress {
        out.push(Draw::Tex {
            x: right - 36.0,
            y: footer_y(canvas),
            w: 36.0,
            h: HINT_H as f32,
            tex,
            alpha: 0.75,
        });
    }
    let y = footer_y(canvas) + (HINT_H as f32 - SYNC_PX as f32) / 2.0;
    out.push(Draw::Turned {
        x,
        y,
        w: SYNC_PX as f32,
        h: SYNC_PX as f32,
        tex: icon.face,
        alpha: icon.alpha,
        turn: icon.turn,
    });
    if icon.attention {
        out.push(Draw::Rect {
            x: x + SYNC_PX as f32 - 2.0,
            y: y + SYNC_PX as f32 - 2.0,
            w: 3.0,
            h: 3.0,
            colour: [
                0xf0 as f32 / 255.0,
                0xb4 as f32 / 255.0,
                0x3c as f32 / 255.0,
                1.0,
            ],
        });
    }
}

pub const FOOTER_Y: f32 = OUT_H as f32 - MOUTH_H + (MOUTH_H - HINT_H as f32) / 2.0;
const FOOTER_MARGIN: f32 = 24.0;

pub fn footer_y(canvas: Canvas) -> f32 {
    FOOTER_Y + canvas.h - OUT_H as f32
}
const CORNER_MARGIN: f32 = 16.0;

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug)]
pub struct Printed {
    pub face: Option<TexId>,
    pub w: u32,
}

impl Printed {
    pub fn new(face: TexId, w: u32) -> Self {
        Printed {
            face: Some(face),
            w,
        }
    }
}

pub fn draw_footer(
    canvas: Canvas,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    ink: [u8; 3],
    out: &mut Vec<Draw>,
) {
    let y = footer_y(canvas) + (HINT_H as f32 - GAUGE_H) / 2.0;
    draw_gauge(FOOTER_MARGIN, y, battery, percent, bolt, ink, out);
    let x = canvas.w - FOOTER_MARGIN - clock.w as f32;
    draw_printed(x, footer_y(canvas), clock, out);
}

const WIFI_GAP: f32 = 12.0;

pub fn draw_wifi_icon(
    canvas: Canvas,
    percent: Printed,
    face: Option<(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {
    let Some((tex, w, h)) = face else {
        return;
    };
    let x = FOOTER_MARGIN + GAUGE_ROW_W + percent.w as f32 + WIFI_GAP;
    let y = footer_y(canvas) + (HINT_H as f32 - h as f32) / 2.0;
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

pub const KID_ICON_PX: f32 = 20.0;

pub fn draw_kid_icon(
    canvas: Canvas,
    percent: Printed,
    wifi: Option<(TexId, u32, u32)>,
    face: Option<(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {
    let Some((tex, w, h)) = face else {
        return;
    };
    let after_wifi = wifi.map_or(0.0, |(_, ww, _)| ww as f32 + WIFI_GAP);
    let x = FOOTER_MARGIN + GAUGE_ROW_W + percent.w as f32 + WIFI_GAP + after_wifi;
    let y = footer_y(canvas) + (HINT_H as f32 - h as f32) / 2.0;
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

pub fn draw_corners(
    canvas: Canvas,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    out: &mut Vec<Draw>,
) {
    let y = (PLATE_H - GAUGE_H) / 2.0;
    draw_gauge(CORNER_MARGIN, y, battery, percent, bolt, HUD_INK, out);
    let x = canvas.w - CORNER_MARGIN - clock.w as f32;
    draw_printed(x, (PLATE_H - HINT_H as f32) / 2.0, clock, out);
}

pub(crate) fn draw_printed(x: f32, y: f32, p: Printed, out: &mut Vec<Draw>) {
    if p.w == 0 {
        return;
    }
    let (w, h) = (p.w as f32, HINT_H as f32);
    out.push(match p.face {
        Some(tex) => Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
        },
        None => Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [1.0, 1.0, 1.0, 0.08],
        },
    });
}
