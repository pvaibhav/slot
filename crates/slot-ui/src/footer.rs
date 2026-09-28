use slot_gfx::{Draw, TexId, OUT_H, OUT_W};
use slot_power::Battery;

use crate::battery::{draw_gauge, GAUGE_H};
use crate::plate::HINT_H;
use crate::slot_chrome::MOUTH_H;
use crate::CartFace;

pub const SYNC_PX: u32 = 18;
const SYNC_GAP: f32 = 14.0;

/// Two small circular arrows, rasterised once by the achievement UI worker.
pub fn sync_icon_face() -> CartFace {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
      <g fill="none" stroke="#f5f2ef" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M5 10a7.2 7.2 0 0 1 12.2-3.2L20 10M20 5v5h-5"/>
        <path d="M19 14a7.2 7.2 0 0 1-12.2 3.2L4 14M4 19v-5h5"/>
      </g>
    </svg>"##;
    CartFace {
        rgba: crate::art::render_svg(svg, SYNC_PX, SYNC_PX)
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
}

/// Shares the clock's baseline and preserves a gap even at the widest clock label.
pub fn draw_footer_sync(clock: Printed, icon: SyncIndicator, out: &mut Vec<Draw>) {
    let x = OUT_W as f32 - FOOTER_MARGIN - clock.w as f32 - SYNC_GAP - SYNC_PX as f32;
    let y = FOOTER_Y + (HINT_H as f32 - SYNC_PX as f32) / 2.0;
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

const FOOTER_Y: f32 = OUT_H as f32 - MOUTH_H + (MOUTH_H - HINT_H as f32) / 2.0;
const FOOTER_MARGIN: f32 = 24.0;

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
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    clock: Printed,
    out: &mut Vec<Draw>,
) {
    let y = FOOTER_Y + (HINT_H as f32 - GAUGE_H) / 2.0;
    draw_gauge(FOOTER_MARGIN, y, battery, percent, bolt, out);
    printed(OUT_W as f32 - FOOTER_MARGIN - clock.w as f32, clock, out);
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

fn printed(x: f32, p: Printed, out: &mut Vec<Draw>) {
    draw_printed(x, FOOTER_Y, p, out);
}

/// Home connection mark beside the left battery cluster, never a gameplay overlay.
pub fn draw_home_wifi(
    battery: Option<Battery>,
    percent: Printed,
    icon: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    let Some(tex) = icon else {
        return;
    };
    let x = FOOTER_MARGIN
        + if battery.is_some() {
            crate::battery::cluster_width(percent) + 12.0
        } else {
            0.0
        };
    out.push(Draw::Tex {
        x,
        y: FOOTER_Y + (HINT_H as f32 - 18.0) / 2.0,
        w: 18.0,
        h: 18.0,
        tex,
        alpha: 1.0,
    });
}
