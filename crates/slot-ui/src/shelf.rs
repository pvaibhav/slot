use slot_gfx::{Canvas, Draw, TexId, OUT_H, OUT_W};
use slot_store::{Cart, Platform};

use crate::board::{board_in, Placed};
use crate::cart::{cart_box, label_colour, label_rect, label_text, shelf_shape, CART_H, CART_W};
use crate::hud::Millis;
use crate::silhouette::GbShell;
use crate::slot_chrome::{draw_empty_slot, MOUTH_H};

const SIDE_SCALE: f32 = 0.78;
const HIDDEN_BELOW: f32 = 0.25;
const SHOWN_FROM: f32 = 0.5;

fn edge_fade(canvas: Canvas, x: f32, w: f32) -> f32 {
    if canvas.w <= OUT_W as f32 || w <= 0.0 {
        return 1.0;
    }
    let shown = ((x + w).min(canvas.w) - x.max(0.0)).max(0.0) / w;
    ((shown - HIDDEN_BELOW) / (SHOWN_FROM - HIDDEN_BELOW)).clamp(0.0, 1.0)
}
const SIDE_ALPHA: f32 = 1.0;

pub fn rest_y(canvas: Canvas, h: f32) -> f32 {
    let y = canvas.inset_y()
        + if h > CART_H as f32 {
            (OUT_H as f32 - MOUTH_H - h) / 2.0
        } else {
            (OUT_H + CART_H) as f32 / 2.0 - h
        };
    // A Game Boy pak is 337px in a 422px gap, which centres at 42.5. Snap to a
    // whole pixel so the 312×337 bitmap hits the RG SP panel 1:1.
    y.round()
}

pub fn foot_y(canvas: Canvas, h: f32) -> f32 {
    rest_y(canvas, h) + h
}

pub fn box_size(platform: Platform) -> (u32, u32) {
    match platform {
        Platform::Gba | Platform::Gbav => (318, 282),
        #[cfg(feature = "nds")]
        Platform::Nds => (318, 282),
        Platform::Gb | Platform::Gbc => (312, 277),
    }
}

const OMEGA: f32 = 16.0;
const PART: f32 = 130.0;

const SLOTS: i32 = 3;

pub const REPEAT_DELAY_MS: Millis = 400;

pub const HEART_SCALE: u32 = 2;
pub const HEART_INK: [u8; 3] = [0xe8, 0x3a, 0x3f];
const HEART_INSET: f32 = 5.0;
const REPEAT_MS: [Millis; 4] = [110, 85, 65, 50];

pub fn repeat_gap(fired: usize) -> Millis {
    REPEAT_MS[fired.min(REPEAT_MS.len() - 1)]
}

pub struct Shelf {
    pub carts: Vec<Cart>,
    pub index: usize,
    pub scroll: f32,
    faces: Vec<Option<TexId>>,
    wrapped: Vec<Option<TexId>>,
    interiors: Vec<Option<TexId>>,
    shadow: Option<TexId>,
    gb_shadow: Option<TexId>,
    gbc_shadow: Option<TexId>,
    backing: Option<TexId>,
    gb_backing: Option<TexId>,
    gbc_backing: Option<TexId>,
    hearted: Vec<bool>,
    heart: Option<(TexId, f32, f32)>,
    ride: f32,
    vel: f32,
    held: Option<(i32, Millis, usize)>,
    lead: f32,
}

impl Shelf {
    pub fn new(carts: Vec<Cart>) -> Self {
        Shelf {
            faces: vec![None; carts.len()],
            wrapped: vec![None; carts.len()],
            interiors: vec![None; carts.len()],
            hearted: vec![false; carts.len()],
            heart: None,
            carts,
            index: 0,
            scroll: 0.0,
            shadow: None,
            gb_shadow: None,
            gbc_shadow: None,
            backing: None,
            gb_backing: None,
            gbc_backing: None,
            ride: 0.0,
            vel: 0.0,
            held: None,
            lead: 0.0,
        }
    }

    fn shell(&self, i: usize) -> Option<GbShell> {
        self.carts.get(i).and_then(|c| shelf_shape(c).0)
    }

    fn clear(&self, i: usize) -> bool {
        self.carts.get(i).is_some_and(|c| shelf_shape(c).1)
    }

    pub fn restock(&mut self, carts: Vec<Cart>) -> Vec<TexId> {
        let mut faces = vec![None; carts.len()];
        let mut wrapped = vec![None; carts.len()];
        let mut interiors = vec![None; carts.len()];
        let mut kept = vec![false; self.carts.len()];
        for (to, cart) in carts.iter().enumerate() {
            let from = self.carts.iter().enumerate().position(|(k, c)| {
                !kept[k] && c.platform == cart.platform && c.stem == cart.stem
            });
            if let Some(from) = from {
                kept[from] = true;
                faces[to] = self.faces[from].take();
                wrapped[to] = self.wrapped[from].take();
                interiors[to] = self.interiors[from].take();
            }
        }
        let old: Vec<TexId> = self
            .faces
            .drain(..)
            .chain(self.interiors.drain(..))
            .flatten()
            .collect();
        self.faces = faces;
        self.wrapped = wrapped;
        self.interiors = interiors;
        self.hearted = vec![false; carts.len()];
        self.carts = carts;
        self.held = None;
        self.select(0);
        old
    }

    pub fn empty_like(&self) -> Shelf {
        Shelf {
            shadow: self.shadow,
            gb_shadow: self.gb_shadow,
            gbc_shadow: self.gbc_shadow,
            backing: self.backing,
            gb_backing: self.gb_backing,
            gbc_backing: self.gbc_backing,
            heart: self.heart,
            ..Shelf::new(Vec::new())
        }
    }

    pub fn carry(&mut self, by: i32) -> Option<(usize, usize)> {
        let n = self.carts.len();
        if n < 2 {
            return None;
        }
        let at = self.index;
        let to = (at as i32 + by).rem_euclid(n as i32) as usize;
        self.carts.swap(at, to);
        self.faces.swap(at, to);
        self.wrapped.swap(at, to);
        self.interiors.swap(at, to);
        self.hearted.swap(at, to);
        self.index = to;
        self.ride += by as f32;
        Some((at, to))
    }

    pub fn swap_carts(&mut self, at: usize, to: usize) {
        if at >= self.carts.len() || to >= self.carts.len() {
            return;
        }
        self.carts.swap(at, to);
        self.faces.swap(at, to);
        self.wrapped.swap(at, to);
        self.interiors.swap(at, to);
        self.hearted.swap(at, to);
        if self.index == at {
            self.ride += to as f32 - at as f32;
            self.index = to;
        }
    }

    pub fn select(&mut self, i: usize) {
        self.index = i;
        self.scroll = i as f32;
        self.ride = i as f32;
        self.vel = 0.0;
    }

    pub fn set_heart(&mut self, face: TexId, w: u32, h: u32) {
        self.heart = Some((face, w as f32, h as f32));
    }

    pub fn mark_hearted(&mut self, hearted: impl Fn(&Cart) -> bool) {
        self.hearted = self.carts.iter().map(hearted).collect();
    }

    pub fn set_shadow(&mut self, face: TexId) {
        self.shadow = Some(face);
    }

    pub fn set_gb_shadow(&mut self, shell: GbShell, face: TexId) {
        match shell {
            GbShell::Notched => self.gb_shadow = Some(face),
            GbShell::Rounded => self.gbc_shadow = Some(face),
        }
    }

    pub fn set_backing(&mut self, face: TexId) {
        self.backing = Some(face);
    }

    pub fn set_gb_backing(&mut self, shell: GbShell, face: TexId) {
        match shell {
            GbShell::Notched => self.gb_backing = Some(face),
            GbShell::Rounded => self.gbc_backing = Some(face),
        }
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        let mut faces = faces.into_iter();
        for face in &mut self.faces {
            *face = faces.next();
        }
    }

    pub fn face(&self, i: usize) -> Option<TexId> {
        self.faces.get(i).copied().flatten()
    }

    pub fn set_wrapped(&mut self, i: usize, tex: Option<TexId>) {
        if let Some(slot) = self.wrapped.get_mut(i) {
            *slot = tex;
        }
    }

    pub fn wrapped(&self, i: usize) -> Option<TexId> {
        self.wrapped.get(i).copied().flatten()
    }

    pub fn set_face(&mut self, i: usize, tex: TexId) -> Option<TexId> {
        self.faces.get_mut(i).and_then(|f| f.replace(tex))
    }

    pub fn set_interior(&mut self, i: usize, tex: TexId) -> Option<TexId> {
        self.interiors.get_mut(i).and_then(|f| f.replace(tex))
    }

    pub fn take_interior(&mut self, i: usize) -> Option<TexId> {
        self.interiors.get_mut(i).and_then(Option::take)
    }

    pub fn take_face(&mut self, i: usize) -> Option<TexId> {
        self.faces.get_mut(i).and_then(Option::take)
    }

    pub fn on_screen(&self) -> Vec<usize> {
        self.around(SLOTS)
    }

    pub fn around(&self, radius: i32) -> Vec<usize> {
        let mut out = Vec::new();
        for step in 0..=radius {
            for off in [step, -step] {
                if let Some(i) = self.cart_at_offset(off) {
                    if !out.contains(&i) {
                        out.push(i);
                    }
                }
            }
        }
        out
    }

    /// What a see-through cart shows behind its front shell, back plastic first and then the board.
    /// The shelf draws these under the face. The slot has to as well, or the board vanishes
    /// as the cart goes in or comes out.
    pub fn behind(&self, stem: &str) -> [Option<TexId>; 2] {
        let Some(i) = self.carts.iter().position(|c| c.stem == stem) else {
            return [None, None];
        };
        if !self.clear(i) || self.face(i).is_none() {
            return [None, None];
        }
        let board = match self.shell(i) {
            None => self.backing,
            Some(GbShell::Notched) => self.gb_backing,
            Some(GbShell::Rounded) => self.gbc_backing,
        };
        [self.interiors.get(i).copied().flatten(), board]
    }

    pub fn find(&self, stem: &str) -> Option<(&Cart, Option<TexId>)> {
        let i = self.carts.iter().position(|c| c.stem == stem)?;
        Some((&self.carts[i], self.face(i)))
    }

    pub fn left(&mut self) {
        self.step(-1);
    }

    pub fn right(&mut self) {
        self.step(1);
    }

    pub fn jump_next_letter(&mut self) {
        self.jump(1);
    }

    pub fn jump_prev_letter(&mut self) {
        self.jump(-1);
    }

    fn start_of_letter(&self, from: usize) -> usize {
        let n = self.carts.len();
        let letter = slot_store::initial(&self.carts[from].stem);
        let mut at = from;
        for _ in 0..n {
            let before = (at as i32 - 1).rem_euclid(n as i32) as usize;
            if slot_store::initial(&self.carts[before].stem) != letter {
                break;
            }
            at = before;
        }
        at
    }

    fn jump(&mut self, dir: i32) {
        let n = self.carts.len();
        if n < 2 {
            return;
        }
        let wrap = |i: i32| i.rem_euclid(n as i32) as usize;
        let here = slot_store::initial(&self.carts[self.index].stem);
        if self
            .carts
            .iter()
            .all(|c| slot_store::initial(&c.stem) == here)
        {
            return;
        }
        let target = match dir > 0 {
            true => {
                let mut at = self.index;
                for _ in 0..n {
                    at = wrap(at as i32 + 1);
                    if slot_store::initial(&self.carts[at].stem) != here {
                        break;
                    }
                }
                at
            }
            false => {
                let start = self.start_of_letter(self.index);
                match start == self.index {
                    true => self.start_of_letter(wrap(start as i32 - 1)),
                    false => start,
                }
            }
        };
        let ahead = (target as i32 - self.index as i32).rem_euclid(n as i32);
        let delta = match dir > 0 {
            true => ahead,
            false => ahead - n as i32,
        };
        self.index = target;
        self.ride += delta as f32;
        let to = self.scroll_target();
        self.scroll = to - (to - self.scroll).clamp(-1.0, 1.0);
    }

    pub fn hold_left(&mut self, now: Millis) {
        self.hold(-1, now);
    }

    pub fn hold_right(&mut self, now: Millis) {
        self.hold(1, now);
    }

    fn hold(&mut self, by: i32, now: Millis) {
        self.step(by);
        self.held = Some((by, now + REPEAT_DELAY_MS, 0));
    }

    pub fn release_left(&mut self) {
        self.release(-1);
    }

    pub fn release_right(&mut self) {
        self.release(1);
    }

    fn release(&mut self, by: i32) {
        if matches!(self.held, Some((held, _, _)) if held == by) {
            self.held = None;
        }
    }

    pub fn release_hold(&mut self) {
        self.held = None;
    }

    pub fn tick(&mut self, now: Millis) {
        let Some((by, due, fired)) = self.held else {
            return;
        };
        if now < due {
            return;
        }
        self.step(by);
        self.held = Some((by, now + repeat_gap(fired), fired + 1));
    }

    fn step(&mut self, by: i32) {
        let n = self.carts.len();
        if n < 2 {
            return;
        }
        self.index = (self.index as i32 + by).rem_euclid(n as i32) as usize;
        self.ride += by as f32;
    }

    pub fn scroll_target(&self) -> f32 {
        let n = self.carts.len();
        if n == 0 {
            return 0.0;
        }
        let from = self.ride;
        let n = n as f32;
        from + (self.index as f32 - from + n / 2.0).rem_euclid(n) - n / 2.0 - self.lead
    }

    fn placed(&self) -> f32 {
        self.scroll_target() + self.lead
    }

    pub fn set_lead(&mut self, lead: f32) {
        self.lead = lead;
    }

    pub fn cart_at_offset(&self, off: i32) -> Option<usize> {
        let n = self.carts.len() as i32;
        if n == 0 {
            return None;
        }
        if n == 1 {
            return (off == 0).then_some(self.index);
        }
        Some((self.index as i32 + off).rem_euclid(n) as usize)
    }

    pub fn selected_shell(&self) -> Option<GbShell> {
        self.shell(self.index)
    }

    pub fn selected_at(&self, canvas: Canvas) -> (f32, f32) {
        let (w, _) = self.slot_width(0);
        let scale = shrink(self.placed() - self.scroll);
        (self.centre(canvas, 0) - w * scale / 2.0, scale)
    }

    fn slot_width(&self, slot: i32) -> (f32, bool) {
        match self.cart_at_offset(slot).map(|i| self.carts[i].platform) {
            Some(Platform::Gba | Platform::Gbav) | None => (CART_W as f32, true),
            Some(p) => (cart_box(p).0 as f32, false),
        }
    }

    fn pitch(&self, a: i32, u: f32) -> f32 {
        let ((wa, gba_a), (wb, gba_b)) = (self.slot_width(a), self.slot_width(a + 1));
        let (sa, sb) = (shrink(a as f32 - u), shrink((a + 1) as f32 - u));
        let gap_of = if gba_a || gba_b { CART_W as f32 } else { wa };
        wa * sa / 2.0 + gap_of * (1.0 - (sa + sb) / 2.0) + wb * sb / 2.0
    }

    fn centre(&self, canvas: Canvas, slot: i32) -> f32 {
        let u = self.scroll - self.placed();
        let a = u.floor() as i32;
        let mut c = canvas.w / 2.0 - (u - a as f32) * self.pitch(a, u);
        if slot >= a {
            for s in a..slot {
                c += self.pitch(s, u);
            }
        } else {
            for s in (slot..a).rev() {
                c -= self.pitch(s, u);
            }
        }
        c
    }

    pub fn update(&mut self, dt: f32) {
        let target = self.scroll_target();
        let off = self.scroll - target;
        let pull = self.vel + OMEGA * off;
        let decay = (-OMEGA * dt).exp();
        self.scroll = target + (off + pull * dt) * decay;
        self.vel = (self.vel - OMEGA * pull * dt) * decay;
    }

    pub fn draw(
        &self,
        canvas: Canvas,
        shake: f32,
        slot: Option<(TexId, TexId)>,
        out: &mut Vec<Draw>,
    ) {
        self.draw_row(canvas, None, shake, 0.0, 1.0, out);
        draw_empty_slot(canvas, slot, out);
    }

    fn layers(&self, i: usize, rect: [f32; 4], alpha: f32, out: &mut Vec<Draw>) {
        let [x, y, w, h] = rect;
        if self.clear(i) && self.face(i).is_some() {
            let board = match self.shell(i) {
                None => self.backing,
                Some(GbShell::Notched) => self.gb_backing,
                Some(GbShell::Rounded) => self.gbc_backing,
            };
            let gba = self.shell(i).is_none();
            // A GBA cart's board and back shell fill the cart's box, exactly where the
            // picker lifts them from; a Game Boy pak's backing fills its box.
            let Placed {
                x: bx,
                y: by,
                w: bw,
                h: bh,
            } = if gba {
                board_in(Placed { x, y, w, h })
            } else {
                Placed { x, y, w, h }
            };
            // Back plastic, then the board, then the front shell.
            if let Some(tex) = self.interiors.get(i).copied().flatten() {
                out.push(Draw::Tex {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                    tex,
                    alpha,
                });
            }
            if let Some(tex) = board {
                out.push(Draw::Tex {
                    x: bx,
                    y: by,
                    w: bw,
                    h: bh,
                    tex,
                    alpha,
                });
            }
        }
        out.push(match self.face(i) {
            Some(tex) => Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha,
            },
            None => {
                let c = label_colour(&label_text(&self.carts[i]));
                Draw::Rect {
                    x,
                    y,
                    w,
                    h,
                    colour: [
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0,
                        alpha,
                    ],
                }
            }
        });
    }

    pub fn draw_cart_at(&self, i: usize, rect: [f32; 4], out: &mut Vec<Draw>) {
        self.layers(i, rect, 1.0, out);
    }

    pub fn draw_cart_faded(&self, i: usize, rect: [f32; 4], alpha: f32, out: &mut Vec<Draw>) {
        self.layers(i, rect, alpha, out);
    }

    pub fn draw_row_turned(
        &self,
        canvas: Canvas,
        hidden: Option<&str>,
        dim: f32,
        turn: &dyn Fn(usize) -> f32,
        shift: &dyn Fn(usize) -> (f32, f32, f32),
        out: &mut Vec<Draw>,
    ) {
        self.row(canvas, hidden, 0.0, 0.0, dim, turn, shift, out);
    }

    pub fn draw_row(
        &self,
        canvas: Canvas,
        hidden: Option<&str>,
        shake: f32,
        recede: f32,
        dim: f32,
        out: &mut Vec<Draw>,
    ) {
        self.row(canvas, hidden, shake, recede, dim, &|_| 0.0, &|_| (0.0, 0.0, 1.0), out);
    }

    #[allow(clippy::too_many_arguments)]
    fn row(
        &self,
        canvas: Canvas,
        hidden: Option<&str>,
        shake: f32,
        recede: f32,
        dim: f32,
        turn: &dyn Fn(usize) -> f32,
        shift: &dyn Fn(usize) -> (f32, f32, f32),
        out: &mut Vec<Draw>,
    ) {
        let recede = recede.clamp(0.0, 1.0);
        let dim = dim.clamp(0.0, 1.0);
        let target = self.placed();
        let front = shift(self.index) != (0.0, 0.0, 1.0);
        let order = (-SLOTS..=SLOTS)
            .filter(|&s| !front || s != 0)
            .chain(front.then_some(0));
        for slot in order {
            let Some(i) = self.cart_at_offset(slot) else {
                continue;
            };
            let cart = &self.carts[i];
            if hidden == Some(cart.stem.as_str()) {
                continue;
            }
            let (dx, dy, grow) = shift(i);
            let offset = target + slot as f32 + dx - self.scroll;
            let t = offset.abs().min(1.0);
            let scale = shrink(offset) * grow;
            let mut alpha = (1.0 + (SIDE_ALPHA - 1.0) * t) * (1.0 - recede);
            let (cw, ch) = cart_box(cart.platform);
            let (w, h) = (cw as f32 * scale, ch as f32 * scale);
            let away = offset.signum() * (1.0 + offset.abs());
            let u = self.scroll - self.placed();
            let step = match dx < 0.0 {
                true => self.pitch(slot - 1, u),
                false => self.pitch(slot, u),
            };
            let x = self.centre(canvas, slot) + dx * step - w / 2.0 + away * PART * recede;
            alpha *= edge_fade(canvas, x, w);
            if x + w <= 0.0 || x >= canvas.w || alpha <= 0.0 {
                continue;
            }
            let x = x + shake;
            let y = foot_y(canvas, ch as f32) - h + dy;
            let start = out.len();
            let deg = turn(i);
            let centre = [x + w / 2.0, y + h / 2.0];
            if let Some(tex) = self.wrapped(i) {
                let (bw, bh) = box_size(cart.platform);
                let (bw, bh) = (bw as f32 * scale, bh as f32 * scale);
                out.push(Draw::Tex {
                    x: x + (w - bw) / 2.0,
                    y: y + h - bh,
                    w: bw,
                    h: bh,
                    tex,
                    alpha: alpha * dim,
                });
                turn_about(&mut out[start..], centre, deg);
                continue;
            }
            if alpha < 1.0 && !is_ds(cart.platform) {
                let backing = match self.shell(i) {
                    None => self.shadow,
                    Some(GbShell::Notched) => self.gb_shadow,
                    Some(GbShell::Rounded) => self.gbc_shadow,
                };
                if let Some(tex) = backing {
                    out.push(Draw::Tex {
                        x,
                        y,
                        w,
                        h,
                        tex,
                        alpha: recede_alpha(alpha),
                    });
                }
            }
            self.layers(i, [x, y, w, h], alpha * dim, out);
            if let (Some(true), Some((tex, sw, sh))) = (self.hearted.get(i), self.heart) {
                let (lx, ly, lw, _) = label_rect(self.shell(i));
                out.push(Draw::Tex {
                    x: (x + (lx + lw) * w - sw - HEART_INSET).round(),
                    y: (y + ly * h + HEART_INSET).round(),
                    w: sw,
                    h: sh,
                    tex,
                    alpha: alpha * dim,
                });
            }
            turn_about(&mut out[start..], centre, deg);
        }
    }
}

pub fn turned(rect: [f32; 4], centre: [f32; 2], deg: f32) -> [[f32; 2]; 4] {
    let [x, y, w, h] = rect;
    let (sin, cos) = deg.to_radians().sin_cos();
    [[x, y], [x + w, y], [x + w, y + h], [x, y + h]].map(|[px, py]| {
        let (dx, dy) = (px - centre[0], py - centre[1]);
        [centre[0] + dx * cos - dy * sin, centre[1] + dx * sin + dy * cos]
    })
}

pub fn turn_about(draws: &mut [Draw], centre: [f32; 2], deg: f32) {
    if deg == 0.0 {
        return;
    }
    for draw in draws {
        *draw = match *draw {
            Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha,
            } => Draw::Quad {
                corners: turned([x, y, w, h], centre, deg),
                tex,
                uv: [0.0, 0.0, 1.0, 1.0],
                alpha,
            },
            Draw::Part {
                x,
                y,
                w,
                h,
                tex,
                alpha,
                uv,
            } => Draw::Quad {
                corners: turned([x, y, w, h], centre, deg),
                tex,
                uv,
                alpha,
            },
            other => other,
        };
    }
}

fn shrink(offset: f32) -> f32 {
    1.0 + (SIDE_SCALE - 1.0) * offset.abs().min(1.0)
}

fn recede_alpha(face_alpha: f32) -> f32 {
    (face_alpha / SIDE_ALPHA).clamp(0.0, 1.0)
}

fn is_ds(platform: slot_store::Platform) -> bool {
    #[cfg(feature = "nds")]
    {
        platform == slot_store::Platform::Nds
    }
    #[cfg(not(feature = "nds"))]
    {
        let _ = platform;
        false
    }
}
