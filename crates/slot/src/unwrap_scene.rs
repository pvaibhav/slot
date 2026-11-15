use std::f32::consts::PI;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

use crate::app::App;
use slot_gfx::{Canvas, Compositor, Draw, TexId};
use slot_input::Millis;
use slot_store::Platform;
use slot_ui::{box_size, cart_box, foot_y, rest_y, CART_H, MOUTH_H};

const SHAKE_END: Millis = 520;
const LID_AT: Millis = 520;
const LID_END: Millis = 1050;
const GLOW_AT: Millis = 520;
const GLOW_FULL: Millis = 760;
const RISE_AT: Millis = 820;
const RISE_END: Millis = 1380;
const DROP_AT: Millis = 1380;
const DROP_END: Millis = 1740;
const FADE_AT: Millis = 1620;
const GLOW_OUT_END: Millis = 1760;
const SETTLE_AT: Millis = 1640;
const SETTLE_END: Millis = 2220;

const SHAKE_TURNS: f32 = 6.0;
const SHAKE_REACH: f32 = 0.05;
const LID_DX: f32 = 150.0;
const LID_DRIFT: f32 = 40.0;
const LID_FLING: f32 = 430.0;
const LID_TURN: f32 = 0.9;
const DROP_BY: f32 = 210.0;
const OVERSHOOT: f32 = 1.5;

const CART_SMALL: f32 = 0.62;
const START_H: f32 = CART_H as f32 * CART_SMALL;
const CART_IN: f32 = 0.548;
const CART_UP: f32 = -0.7;

const LID_W: f32 = 318.0;
const BODY_W: f32 = 292.0;
const BODY_H: f32 = 150.0;
const LID_FRONT: f32 = 56.0;
const RIM_H: f32 = 4.0;
const RIM: [f32; 4] = [
    0x6b as f32 / 255.0,
    0x52 as f32 / 255.0,
    0xb4 as f32 / 255.0,
    1.0,
];
const GLOW_ABOVE: f32 = 16.0;
const BODY_RECT: [f32; 4] = [-148.0, -152.0, 296.0, 154.0];
const INSIDE_RECT: [f32; 4] = [-148.0, -184.0, 296.0, 36.0];
const LID_RECT: [f32; 4] = [-161.0, -106.0, 322.0, 108.0];
const GLOW_SIZE: (f32, f32) = (440.0, 272.0);

const BODY_PNG: &[u8] = include_bytes!("../../../assets/romm/box_body.png");
const INSIDE_PNG: &[u8] = include_bytes!("../../../assets/romm/box_inside.png");
const LID_PNG: &[u8] = include_bytes!("../../../assets/romm/lid.png");
const GLOW_PNG: &[u8] = include_bytes!("../../../assets/romm/glow.png");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnwrapScene {
    start: Millis,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnwrapFrame {
    pub shake: f32,
    pub lid: Option<(f32, f32, f32)>,
    pub glow: f32,
    pub cart: Option<(f32, f32)>,
    pub box_drop: f32,
    pub box_alpha: f32,
    pub open: bool,
}

fn seg(t: Millis, a: Millis, b: Millis) -> f32 {
    (t.saturating_sub(a) as f32 / (b - a) as f32).min(1.0)
}

fn mix(a: f32, b: f32, k: f32) -> f32 {
    a * (1.0 - k) + b * k
}

fn out_back(x: f32) -> f32 {
    1.0 + (OVERSHOOT + 1.0) * (x - 1.0).powi(3) + OVERSHOOT * (x - 1.0).powi(2)
}

fn in_out(x: f32) -> f32 {
    match x < 0.5 {
        true => 2.0 * x * x,
        false => 1.0 - (-2.0 * x + 2.0).powi(2) / 2.0,
    }
}

fn shake_at(t: Millis) -> f32 {
    if t >= SHAKE_END {
        return 0.0;
    }
    let k = seg(t, 0, SHAKE_END);
    (k * PI * SHAKE_TURNS).sin() * (1.0 - k) * SHAKE_REACH
}

fn lid_at(t: Millis) -> Option<(f32, f32, f32)> {
    if t >= LID_END {
        return None;
    }
    let k = seg(t, LID_AT, LID_END);
    Some((k * LID_DX, -k * LID_DRIFT - LID_FLING * k * k, k * LID_TURN))
}

fn cart_at(t: Millis) -> Option<(f32, f32)> {
    if t < RISE_AT {
        return None;
    }
    let up = out_back(seg(t, RISE_AT, RISE_END));
    let settle = in_out(seg(t, SETTLE_AT, SETTLE_END));
    Some((
        mix(CART_SMALL, 1.0, settle),
        mix(CART_IN, CART_UP, up) * (1.0 - settle),
    ))
}

impl UnwrapScene {
    pub fn new(now: Millis) -> UnwrapScene {
        UnwrapScene { start: now }
    }

    pub fn frame(&self, now: Millis) -> UnwrapFrame {
        let t = now.saturating_sub(self.start);
        let drop = seg(t, DROP_AT, DROP_END);
        UnwrapFrame {
            shake: shake_at(t),
            lid: lid_at(t),
            glow: seg(t, GLOW_AT, GLOW_FULL) * (1.0 - seg(t, DROP_AT, GLOW_OUT_END)),
            cart: cart_at(t),
            box_drop: drop * drop * drop * DROP_BY,
            box_alpha: 1.0 - seg(t, FADE_AT, DROP_END),
            open: t >= LID_AT,
        }
    }

    pub fn done(&self, now: Millis) -> bool {
        now.saturating_sub(self.start) >= SETTLE_END
    }
}

fn box_scale(platform: Platform) -> f32 {
    box_size(platform).0 as f32 / LID_W
}

pub fn body_width(platform: Platform) -> f32 {
    BODY_W * box_scale(platform)
}

pub fn rim_y(canvas: Canvas, platform: Platform) -> f32 {
    foot_y(canvas, cart_box(platform).1 as f32) - BODY_H * box_scale(platform)
}

pub fn footer_top(canvas: Canvas) -> f32 {
    canvas.h - MOUTH_H
}

pub fn cart_rect(cart: (f32, f32), platform: Platform, canvas: Canvas) -> [f32; 4] {
    let (scale, off) = cart;
    let (cw, ch) = cart_box(platform);
    let (cw, ch) = (cw as f32, ch as f32);
    let small = (1.0 - scale) / (1.0 - CART_SMALL);
    let size = mix(1.0, START_H / ch, small);
    let (w, h) = (cw * size, ch * size);
    let rest = rest_y(canvas, ch) + ch / 2.0;
    let mid = mix(rest, rim_y(canvas, platform), small) + off * START_H;
    [canvas.centred(w), mid - h / 2.0, w, h]
}

#[derive(Clone, Copy)]
struct Aff([f32; 6]);

impl Aff {
    fn shift(x: f32, y: f32) -> Aff {
        Aff([1.0, 0.0, 0.0, 1.0, x, y])
    }

    fn scale(x: f32, y: f32) -> Aff {
        Aff([x, 0.0, 0.0, y, 0.0, 0.0])
    }

    fn turn(r: f32) -> Aff {
        let (s, c) = r.sin_cos();
        Aff([c, s, -s, c, 0.0, 0.0])
    }

    fn then(self, o: Aff) -> Aff {
        let [a, b, c, d, e, f] = self.0;
        let [p, q, r, s, t, u] = o.0;
        Aff([
            a * p + c * q,
            b * p + d * q,
            a * r + c * s,
            b * r + d * s,
            a * t + c * u + e,
            b * t + d * u + f,
        ])
    }

    fn at(self, x: f32, y: f32) -> [f32; 2] {
        let [a, b, c, d, e, f] = self.0;
        [a * x + c * y + e, b * x + d * y + f]
    }

    fn undo(self) -> Aff {
        let [a, b, c, d, e, f] = self.0;
        let det = a * d - b * c;
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        Aff([ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)])
    }
}

fn lid_pose(base: Aff, lid: (f32, f32, f32)) -> Aff {
    let (dx, dy, turn) = lid;
    base.then(Aff::shift(dx, dy - BODY_H - LID_FRONT / 2.0))
        .then(Aff::turn(turn))
        .then(Aff::shift(0.0, LID_FRONT / 2.0))
}

fn piece(m: Aff, rect: [f32; 4], limit: f32, tex: TexId, alpha: f32) -> Option<Draw> {
    let [x, y, w, h] = rect;
    let shown = h.min(limit - y);
    if shown <= 0.0 {
        return None;
    }
    let bottom = y + shown;
    Some(Draw::Quad {
        corners: [
            m.at(x, y),
            m.at(x + w, y),
            m.at(x + w, bottom),
            m.at(x, bottom),
        ],
        tex,
        uv: [0.0, 0.0, 1.0, shown / h],
        alpha,
    })
}

pub fn unwrap_draws(
    frame: &UnwrapFrame,
    tex: &UnwrapTextures,
    gift: (Platform, &str),
    cart: impl FnOnce([f32; 4], &mut Vec<Draw>),
    canvas: Canvas,
    out: &mut Vec<Draw>,
) {
    let (platform, _) = gift;
    let k = box_scale(platform);
    let mid = canvas.w / 2.0;
    let foot = foot_y(canvas, cart_box(platform).1 as f32);
    let floor = foot + frame.box_drop;
    let clip = footer_top(canvas);
    let limit = (clip - floor) / k;
    let base = Aff::shift(mid, floor)
        .then(Aff::turn(frame.shake))
        .then(Aff::scale(k, k));
    if frame.glow > 0.0 {
        let (w, h) = (GLOW_SIZE.0 * k, GLOW_SIZE.1 * k);
        let y = rim_y(canvas, platform) - GLOW_ABOVE * k + frame.box_drop;
        let rect = [mid - w / 2.0, y - h / 2.0, w, h];
        out.extend(piece(
            Aff::shift(0.0, 0.0),
            rect,
            clip,
            tex.glow,
            frame.glow,
        ));
    }
    let shown = frame.box_alpha > 0.0;
    if shown && frame.open {
        out.extend(piece(base, INSIDE_RECT, limit, tex.inside, frame.box_alpha));
    }
    if let Some(c) = frame.cart {
        cart(cart_rect(c, platform, canvas), out);
    }
    if shown {
        out.extend(piece(base, BODY_RECT, limit, tex.body, frame.box_alpha));
        let (rim, h) = (floor - BODY_H * k, RIM_H * k);
        if frame.open && rim + h <= clip {
            let [r, g, b, _] = RIM;
            out.push(Draw::Rect {
                x: mid - BODY_W * k / 2.0,
                y: rim,
                w: BODY_W * k,
                h,
                colour: [r, g, b, frame.box_alpha],
            });
        }
    }
    if let Some(lid) = frame.lid {
        let pose = lid_pose(base, lid);
        out.extend(piece(pose, LID_RECT, f32::MAX, tex.lid, 1.0));
    }
}

pub struct Layer {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
}

fn layer(png: &[u8]) -> Option<Layer> {
    let (rgba, w, h) = slot_ui::decode_png_bytes(png)?;
    Some(Layer { rgba, w, h })
}

pub struct UnwrapArt {
    body: Layer,
    inside: Layer,
    lid: Layer,
    glow: Layer,
}

pub fn unwrap_art() -> Option<UnwrapArt> {
    Some(UnwrapArt {
        body: layer(BODY_PNG)?,
        inside: layer(INSIDE_PNG)?,
        lid: layer(LID_PNG)?,
        glow: layer(GLOW_PNG)?,
    })
}

fn sample(l: &Layer, u: f32, v: f32) -> [f32; 4] {
    let (x, y) = (u - 0.5, v - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let texel = |tx: i32, ty: i32| -> [f32; 4] {
        if tx < 0 || ty < 0 || tx >= l.w as i32 || ty >= l.h as i32 {
            return [0.0; 4];
        }
        let o = (ty as usize * l.w as usize + tx as usize) * 4;
        let p = &l.rgba[o..o + 4];
        let a = p[3] as f32 / 255.0;
        [
            p[0] as f32 / 255.0 * a,
            p[1] as f32 / 255.0 * a,
            p[2] as f32 / 255.0 * a,
            a,
        ]
    };
    let (ix, iy) = (x0 as i32, y0 as i32);
    let (a, b, c, d) = (
        texel(ix, iy),
        texel(ix + 1, iy),
        texel(ix, iy + 1),
        texel(ix + 1, iy + 1),
    );
    std::array::from_fn(|i| mix(mix(a[i], b[i], fx), mix(c[i], d[i], fx), fy))
}

struct Sheet {
    px: Vec<f32>,
    w: u32,
    h: u32,
}

impl Sheet {
    fn stamp(&mut self, l: &Layer, m: Aff, rect: [f32; 4]) {
        let [rx, ry, rw, rh] = rect;
        let corners = [
            m.at(rx, ry),
            m.at(rx + rw, ry),
            m.at(rx + rw, ry + rh),
            m.at(rx, ry + rh),
        ];
        let least = |i: usize| corners.iter().fold(f32::MAX, |a, c| a.min(c[i]));
        let most = |i: usize| corners.iter().fold(f32::MIN, |a, c| a.max(c[i]));
        let x0 = least(0).floor().max(0.0) as u32;
        let y0 = least(1).floor().max(0.0) as u32;
        let x1 = (most(0).ceil() as u32).min(self.w);
        let y1 = (most(1).ceil() as u32).min(self.h);
        let back = m.undo();
        for y in y0..y1 {
            for x in x0..x1 {
                let [lx, ly] = back.at(x as f32 + 0.5, y as f32 + 0.5);
                let u = (lx - rx) / rw * l.w as f32;
                let v = (ly - ry) / rh * l.h as f32;
                let s = sample(l, u, v);
                let o = (y as usize * self.w as usize + x as usize) * 4;
                for i in 0..4 {
                    self.px[o + i] = s[i] + self.px[o + i] * (1.0 - s[3]);
                }
            }
        }
    }

    fn bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.px.len());
        for p in self.px.chunks_exact(4) {
            let a = p[3].max(f32::EPSILON);
            for c in &p[..3] {
                out.push((c / a * 255.0).round().min(255.0) as u8);
            }
            out.push((p[3] * 255.0).round() as u8);
        }
        out
    }
}

pub fn box_face(art: &UnwrapArt, platform: Platform, _stem: &str) -> Vec<u8> {
    let (w, h) = box_size(platform);
    let k = box_scale(platform);
    let mut sheet = Sheet {
        px: vec![0.0; (w * h * 4) as usize],
        w,
        h,
    };
    let base = Aff::shift(w as f32 / 2.0, h as f32).then(Aff::scale(k, k));
    let lid = lid_pose(base, (0.0, 0.0, 0.0));
    sheet.stamp(&art.body, base, BODY_RECT);
    sheet.stamp(&art.lid, lid, LID_RECT);
    sheet.bytes()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnwrapTextures {
    body: TexId,
    inside: TexId,
    lid: TexId,
    glow: TexId,
}

impl UnwrapTextures {
    pub fn upload(c: &mut Compositor, art: &UnwrapArt) -> UnwrapTextures {
        let mut up = |l: &Layer| c.create_texture(l.w, l.h, &l.rgba);
        UnwrapTextures {
            body: up(&art.body),
            inside: up(&art.inside),
            lid: up(&art.lid),
            glow: up(&art.glow),
        }
    }
}

type GiftKey = (Platform, String);

enum Built {
    Art(Arc<UnwrapArt>),
    Face(GiftKey, Vec<u8>),
}

struct GiftWorker {
    jobs: Sender<GiftKey>,
    built: Receiver<Built>,
}

impl GiftWorker {
    fn spawn() -> GiftWorker {
        let (jobs, inbox) = mpsc::channel::<GiftKey>();
        let (outbox, built) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-gift-art".into())
            .spawn(move || {
                let Some(art) = unwrap_art().map(Arc::new) else {
                    eprintln!("slot: gift art did not decode");
                    return;
                };
                let _ = outbox.send(Built::Art(art.clone()));
                for key in inbox {
                    let face = box_face(&art, key.0, &key.1);
                    let _ = outbox.send(Built::Face(key, face));
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: gift art: worker thread failed to start: {e}");
        }
        GiftWorker { jobs, built }
    }
}

#[derive(Default)]
pub struct GiftBoxes {
    worker: Option<GiftWorker>,
    faces: Vec<(GiftKey, TexId)>,
    asked: Vec<GiftKey>,
    free: Vec<TexId>,
}

impl GiftBoxes {
    fn place(&mut self, app: &mut App, c: &mut Compositor, built: Built) {
        match built {
            Built::Art(art) => app.set_unwrap_tex(UnwrapTextures::upload(c, &art)),
            Built::Face(key, rgba) => {
                let (w, h) = box_size(key.0);
                let tex = match self.free.pop() {
                    Some(tex) => {
                        c.update_texture(tex, w, h, &rgba);
                        tex
                    }
                    None => c.create_texture(w, h, &rgba),
                };
                self.asked.retain(|k| *k != key);
                self.faces.push((key, tex));
            }
        }
    }

    fn shed(&mut self, app: &App) {
        let free = &mut self.free;
        self.faces.retain(|((platform, stem), tex)| {
            let held = app.gift_held(*platform, stem);
            if !held {
                free.push(*tex);
            }
            held
        });
    }

    pub fn sync(&mut self, app: &mut App, c: &mut Compositor) {
        if !app.has_gifts() && self.faces.is_empty() {
            return;
        }
        let worker = self.worker.get_or_insert_with(GiftWorker::spawn);
        let built: Vec<Built> = worker.built.try_iter().collect();
        for b in built {
            self.place(app, c, b);
        }
        self.shed(app);
        for (i, key) in app.gift_wants() {
            match self.faces.iter().find(|(k, _)| *k == key) {
                Some((_, tex)) => app.set_gift_box(i, *tex),
                None if self.asked.contains(&key) => {}
                None => {
                    if let Some(worker) = &self.worker {
                        let _ = worker.jobs.send(key.clone());
                    }
                    self.asked.push(key);
                }
            }
        }
    }
}
