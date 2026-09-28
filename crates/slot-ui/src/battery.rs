use slot_gfx::{Draw, TexId};
use slot_power::{Battery, Charge};

use crate::footer::Printed;
use crate::hud::HUD_INK;
use crate::plate::HINT_H;

pub const BOLT_PX: f32 = 18.0;

pub const GAUGE_W: f32 = 22.0;
pub const GAUGE_H: f32 = 11.0;
const _: () = assert!(GAUGE_H < GAUGE_W);
const NUB_W: f32 = 2.5;
const NUB_H: f32 = 4.0;
pub const WALL: f32 = 1.5;
const GAP: f32 = 7.0;

const BOLT_W: f32 = 14.0;
const BOLT_H: f32 = 14.0;
const BOLT_GAP: f32 = 5.0;
const _: () = assert!(BOLT_GAP > 0.0);

const INK: [f32; 4] = [
    HUD_INK[0] as f32 / 255.0,
    HUD_INK[1] as f32 / 255.0,
    HUD_INK[2] as f32 / 255.0,
    1.0,
];

pub fn draw_gauge(
    x: f32,
    y: f32,
    battery: Option<Battery>,
    percent: Printed,
    bolt: Option<TexId>,
    out: &mut Vec<Draw>,
) {
    let Some(b) = battery else {
        return;
    };

    let rect = |x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Draw>| {
        out.push(Draw::Rect {
            x,
            y,
            w,
            h,
            colour: INK,
        });
    };

    let cx = x + BOLT_W + BOLT_GAP;

    rect(cx, y, GAUGE_W, WALL, out);
    rect(cx, y + GAUGE_H - WALL, GAUGE_W, WALL, out);
    rect(cx, y, WALL, GAUGE_H, out);
    rect(cx + GAUGE_W - WALL, y, WALL, GAUGE_H, out);
    rect(cx + GAUGE_W, y + (GAUGE_H - NUB_H) / 2.0, NUB_W, NUB_H, out);

    let inner = GAUGE_W - 4.0 * WALL;
    let fill = inner * f32::from(b.percent.min(100)) / 100.0;
    if fill > 0.0 {
        rect(
            cx + 2.0 * WALL,
            y + 2.0 * WALL,
            fill,
            GAUGE_H - 4.0 * WALL,
            out,
        );
    }

    if let (Charge::Charging, Some(tex)) = (b.charge, bolt) {
        out.push(Draw::Tex {
            x,
            y: y + (GAUGE_H - BOLT_H) / 2.0,
            w: BOLT_W,
            h: BOLT_H,
            tex,
            alpha: 1.0,
        });
    }

    if percent.w > 0 {
        let px = cx + GAUGE_W + NUB_W + GAP;
        let py = y + (GAUGE_H - HINT_H as f32) / 2.0;
        out.push(match percent.face {
            Some(tex) => Draw::Tex {
                x: px,
                y: py,
                w: percent.w as f32,
                h: HINT_H as f32,
                tex,
                alpha: 1.0,
            },
            None => Draw::Rect {
                x: px,
                y: py,
                w: percent.w as f32,
                h: HINT_H as f32,
                colour: [1.0, 1.0, 1.0, 0.08],
            },
        });
    }
}

/// Width reserved by the complete battery cluster, including charging and percent.
pub(crate) fn cluster_width(percent: Printed) -> f32 {
    BOLT_W + BOLT_GAP + GAUGE_W + NUB_W + GAP + percent.w as f32
}
