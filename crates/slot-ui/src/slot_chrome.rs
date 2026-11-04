use slot_gfx::{Canvas, Draw, TexId, OUT_H};
use slot_store::Cart;

use crate::cart::{cart_box, label_colour, label_text, seated_box, SEATED_W};
use crate::footer::Printed;
use crate::icon::icon_box;
use crate::plate::HINT_H;
use crate::shelf::foot_y;

pub const ALERT_PX: f32 = 44.0;

pub const MOUTH_H: f32 = 58.0;
const BAND_Y: f32 = OUT_H as f32 - MOUTH_H;

pub const MOUTH_W: f32 = SEATED_W as f32 + 14.0;
const SLIT_H: f32 = 9.0;
const SLIT_Y: f32 = BAY_Y + 5.0;

pub const LIP_H: f32 = 2.0;

const BAY_W: f32 = MOUTH_W + 18.0;
const BAY_Y: f32 = BAND_Y + LIP_H;
const SCOOP_W: f32 = MOUTH_W * 0.88;
const SCOOP_D: f32 = RECESS_H - (SCOOP_Y - BAY_Y);
const SCOOP_Y: f32 = SLIT_Y + SLIT_H;

const RECESS_H: f32 = 42.0;
const RIM_W: f32 = 2.0;
const SCOOP_FLAT: f32 = 4.0;

fn mouth_x(canvas: Canvas) -> f32 {
    canvas.centred(MOUTH_W)
}

fn bay_x(canvas: Canvas) -> f32 {
    canvas.centred(BAY_W)
}

const HOUSING: [u8; 3] = [0x24, 0x24, 0x29];
const RECESS: [u8; 3] = [0x1a, 0x1a, 0x1d];
const OPENING: [u8; 3] = [0x05, 0x05, 0x08];
const EDGE: [u8; 3] = [0x4d, 0x4d, 0x57];

fn rgb(c: [u8; 3]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        1.0,
    ]
}

pub fn housing() -> [f32; 4] {
    rgb(HOUSING)
}

pub fn opening() -> [f32; 4] {
    rgb(OPENING)
}

pub fn edge() -> [f32; 4] {
    rgb(EDGE)
}

pub fn recess() -> [f32; 4] {
    rgb(RECESS)
}

const LIP_Y: f32 = BAND_Y;

const SEATED_Y: f32 = BAY_Y + 4.0;

fn catch_at(foot: f32, seated_foot: f32) -> f32 {
    (LIP_Y - foot) / (seated_foot - foot)
}

const CATCH_IN: f32 = 0.42;
const CATCH_OUT: f32 = 0.62;
const CREEP: f32 = 0.03;

pub struct SlotChrome<'a> {
    pub cart: &'a Cart,
    pub face: Option<TexId>,
    pub rest: f32,
    pub scale: f32,
    pub seat: f32,
    pub alert: Option<(TexId, f32)>,
    pub dim: f32,
    pub screen: f32,
    pub game: bool,
    /// When set, the picture opens behind a fading slot. A resumed boot, the picture opening, and the eject back to the shelf keep the cart between the slot back and front.
    pub over: bool,
    /// The back plastic and the board a see-through cart shows under its front shell.
    pub behind: [Option<TexId>; 2],
    pub slot: Option<(TexId, TexId)>,
    pub canvas: Canvas,
}

impl SlotChrome<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        let canvas = self.canvas;
        let seat = self.seat.clamp(0.0, 1.0);
        let dim = self.dim.clamp(0.0, 1.0);
        if dim > 0.0 {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: canvas.w,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, dim],
            });
        }

        let chrome = 1.0 - self.screen.clamp(0.0, 1.0);
        let opening = self.screen > 0.0;
        let held = opening && !self.over;
        let slot_alpha = if held { 1.0 } else { chrome };

        if self.game && opening && self.over {
            out.push(Draw::Game);
        }

        draw_slot_back(canvas, self.slot, slot_alpha, out);

        let (cw, ch) = cart_box(self.cart.platform);
        let (cw, ch) = (cw as f32, ch as f32);

        let scale = self.scale.clamp(0.0, 1.0);
        let (w0, h0) = (cw * scale, ch * scale);
        let (sw, sh) = seated_box(self.cart.platform);
        let (sw, sh) = (sw as f32, sh as f32);
        let (foot0, seated_foot) = (foot_y(ch), SEATED_Y + sh);
        let catch = catch_at(foot0, seated_foot);
        let travel = travel(seat, catch);
        let k = (travel / catch).min(1.0);
        let (cw, ch) = (w0 + (sw - w0) * k, h0 + (sh - h0) * k);
        let centre = self.rest + w0 / 2.0;
        let x = centre + (canvas.w / 2.0 - centre) * k - cw / 2.0;
        let y = foot0 + (seated_foot - foot0) * travel - ch;
        let cart_alpha = if seat >= 1.0 && !held { chrome } else { 1.0 };
        for tex in self.behind.into_iter().flatten() {
            out.push(Draw::Tex {
                x,
                y,
                w: cw,
                h: ch,
                tex,
                alpha: cart_alpha,
            });
        }
        out.push(match self.face {
            Some(tex) => Draw::Tex {
                x,
                y,
                w: cw,
                h: ch,
                tex,
                alpha: cart_alpha,
            },
            None => {
                let c = label_colour(&label_text(self.cart));
                Draw::Rect {
                    x,
                    y,
                    w: cw,
                    h: ch,
                    colour: [
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        cart_alpha,
                    ],
                }
            }
        });

        if let Some((tex, alpha)) = self.alert {
            let (w, h) = icon_box(ALERT_PX);
            let (w, h) = (w as f32, h as f32);
            out.push(Draw::Tex {
                x: x + (cw - w) / 2.0,
                y: y + (ch - h) / 2.0,
                w,
                h,
                tex,
                alpha,
            });
        }

        if self.over && chrome < 1.0 && self.slot.is_none() {
            front_bands(canvas, |x, y, w, h, _| {
                out.push(Draw::Rect {
                    x,
                    y,
                    w,
                    h,
                    colour: [0.0, 0.0, 0.0, 1.0],
                });
            });
        }
        draw_slot_front(
            canvas,
            self.slot,
            if self.over { chrome } else { slot_alpha },
            out,
        );

        if self.game && opening && !self.over {
            out.push(Draw::Game);
        }
    }
}

fn band(x: f32, y: f32, w: f32, h: f32, c: [f32; 4], alpha: f32) -> Draw {
    Draw::Rect {
        x,
        y,
        w,
        h,
        colour: [c[0], c[1], c[2], c[3] * alpha],
    }
}

fn strip(canvas: Canvas, tex: TexId, alpha: f32) -> Draw {
    Draw::Part {
        x: 0.0,
        y: BAND_Y,
        w: canvas.w,
        h: MOUTH_H,
        tex,
        alpha,
        uv: canvas.window(crate::bitmaps::SLOT_W as f32),
    }
}

pub fn draw_slot_back(
    canvas: Canvas,
    slot: Option<(TexId, TexId)>,
    alpha: f32,
    out: &mut Vec<Draw>,
) {
    if let Some((back, _)) = slot {
        out.push(strip(canvas, back, alpha));
        return;
    }
    let (bay_x, mouth_x) = (bay_x(canvas), mouth_x(canvas));
    out.push(band(bay_x, BAND_Y, BAY_W, LIP_H, housing(), alpha));
    out.push(band(mouth_x, BAND_Y, MOUTH_W, LIP_H, edge(), alpha));
    out.push(band(bay_x, BAY_Y, BAY_W, RECESS_H, recess(), alpha));
    out.push(band(mouth_x, SLIT_Y, MOUTH_W, SLIT_H, opening(), alpha));
    for_each_scoop_span(canvas, |x, w, depth| {
        out.push(band(x, SCOOP_Y, w, depth, opening(), alpha));
    });
}

pub fn draw_slot_front(
    canvas: Canvas,
    slot: Option<(TexId, TexId)>,
    alpha: f32,
    out: &mut Vec<Draw>,
) {
    if let Some((_, front)) = slot {
        out.push(strip(canvas, front, alpha));
        return;
    }
    front_bands(canvas, |x, y, w, h, c| out.push(band(x, y, w, h, c, alpha)));
}

fn front_bands(canvas: Canvas, mut band: impl FnMut(f32, f32, f32, f32, [f32; 4])) {
    let w = canvas.w;
    let (bay_x, cx) = (bay_x(canvas), canvas.w / 2.0);
    let right = bay_x + BAY_W;
    let floor = SCOOP_Y + SCOOP_D + RIM_W;
    band(0.0, BAND_Y, bay_x, MOUTH_H, housing());
    band(right, BAND_Y, w - right, MOUTH_H, housing());

    let near = cx - SCOOP_W / 2.0;
    band(bay_x, SCOOP_Y, near - bay_x, floor - SCOOP_Y, housing());
    let far = cx + SCOOP_W / 2.0;
    band(far, SCOOP_Y, right - far, floor - SCOOP_Y, housing());

    for_each_scoop_span(canvas, |x, w, depth| {
        let top = SCOOP_Y + depth + RIM_W;
        band(x, top, w, floor - top, housing());
    });
    band(0.0, floor, w, OUT_H as f32 - floor, housing());
    for_each_scoop_span(canvas, |x, w, depth| {
        band(x, SCOOP_Y + depth, w, RIM_W, edge());
    });
}

fn for_each_scoop_span(canvas: Canvas, mut span: impl FnMut(f32, f32, f32)) {
    let cx = canvas.w / 2.0;
    let hw = SCOOP_W / 2.0;
    let depth = |x: f32| SCOOP_D * (1.0 - (x.abs() / hw).powf(SCOOP_FLAT)).max(0.0);
    let mut x = -hw;
    while x < hw {
        let d = depth(x + 0.5).round();
        let start = x;
        while x < hw && depth(x + 0.5).round() == d {
            x += 1.0;
        }
        span(cx + start, x - start, d);
    }
}

const SLOT_NAME_GAP: f32 = 6.0;

pub fn slot_name_y() -> f32 {
    OUT_H as f32 - MOUTH_H - HINT_H as f32 - SLOT_NAME_GAP
}

pub fn draw_slot_title(canvas: Canvas, title: Printed, alpha: f32, out: &mut Vec<Draw>) {
    let (Some(tex), true) = (title.face, alpha > 0.0) else {
        return;
    };
    let (w, h) = (title.w as f32, crate::plate::TITLE_LINE_H as f32);
    out.push(Draw::Tex {
        x: canvas.centred(w),
        y: slot_name_y() + HINT_H as f32 - h,
        w,
        h,
        tex,
        alpha,
    });
}

pub fn draw_plugged_slot(canvas: Canvas, plugged: Option<TexId>, alpha: f32, out: &mut Vec<Draw>) {
    if let Some(tex) = plugged {
        out.push(strip(canvas, tex, alpha));
    }
}

pub fn draw_empty_slot(canvas: Canvas, slot: Option<(TexId, TexId)>, out: &mut Vec<Draw>) {
    draw_slot_back(canvas, slot, 1.0, out);
    draw_slot_front(canvas, slot, 1.0, out);
}

fn travel(seat: f32, catch: f32) -> f32 {
    if seat < CATCH_IN {
        catch * ease(seat / CATCH_IN)
    } else if seat < CATCH_OUT {
        catch + CREEP * (seat - CATCH_IN) / (CATCH_OUT - CATCH_IN)
    } else {
        let caught = catch + CREEP;
        caught + (1.0 - caught) * ease((seat - CATCH_OUT) / (1.0 - CATCH_OUT))
    }
}

pub fn ease(u: f32) -> f32 {
    u * u * u * (u * (u * 6.0 - 15.0) + 10.0)
}
