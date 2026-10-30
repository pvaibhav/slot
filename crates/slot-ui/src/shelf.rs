use slot_gfx::{Canvas, Draw, TexId, OUT_H};
use slot_store::{Cart, Platform};
use std::sync::OnceLock;

use crate::board::{board_in, Placed};
use crate::cart::{
    cart_box, gb_shell_of, is_clear, label_colour, label_rect, label_text, CART_H, CART_W,
};
use crate::hud::Millis;
use crate::silhouette::GbShell;
use crate::slot_chrome::{draw_empty_slot, MOUTH_H};

const SIDE_SCALE: f32 = 0.78;
const SIDE_ALPHA: f32 = 1.0;

pub fn rest_y(h: f32) -> f32 {
    let y = if h > CART_H as f32 {
        (OUT_H as f32 - MOUTH_H - h) / 2.0
    } else {
        (OUT_H + CART_H) as f32 / 2.0 - h
    };
    // A Game Boy pak is 337px in a 422px gap, which centres at 42.5. Snap to a
    // whole pixel so the 312×337 bitmap hits the RG SP panel 1:1.
    y.round()
}

pub fn foot_y(h: f32) -> f32 {
    rest_y(h) + h
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
    interiors: Vec<Option<TexId>>,
    shadow: Option<TexId>,
    gb_shadow: Option<TexId>,
    gbc_shadow: Option<TexId>,
    backing: Option<TexId>,
    gb_backing: Option<TexId>,
    gbc_backing: Option<TexId>,
    shells: Vec<OnceLock<Option<GbShell>>>,
    clears: Vec<OnceLock<bool>>,
    hearted: Vec<bool>,
    heart: Option<(TexId, f32, f32)>,
    ride: f32,
    vel: f32,
    held: Option<(i32, Millis, usize)>,
}

impl Shelf {
    pub fn new(carts: Vec<Cart>) -> Self {
        Shelf {
            shells: carts.iter().map(|_| OnceLock::new()).collect(),
            clears: carts.iter().map(|_| OnceLock::new()).collect(),
            faces: vec![None; carts.len()],
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
        }
    }

    fn shell(&self, i: usize) -> Option<GbShell> {
        let (Some(cell), Some(cart)) = (self.shells.get(i), self.carts.get(i)) else {
            return None;
        };
        *cell.get_or_init(|| gb_shell_of(cart))
    }

    fn clear(&self, i: usize) -> bool {
        let (Some(cell), Some(cart)) = (self.clears.get(i), self.carts.get(i)) else {
            return false;
        };
        *cell.get_or_init(|| is_clear(cart))
    }

    pub fn restock(&mut self, carts: Vec<Cart>) -> Vec<TexId> {
        let old: Vec<TexId> = self
            .faces
            .drain(..)
            .chain(self.interiors.drain(..))
            .flatten()
            .collect();
        self.shells = carts.iter().map(|_| OnceLock::new()).collect();
        self.clears = carts.iter().map(|_| OnceLock::new()).collect();
        self.faces = vec![None; carts.len()];
        self.interiors = vec![None; carts.len()];
        self.hearted = vec![false; carts.len()];
        self.carts = carts;
        self.held = None;
        self.select(0);
        old
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
        from + (self.index as f32 - from + n / 2.0).rem_euclid(n) - n / 2.0
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

    pub fn selected_at(&self, canvas: Canvas) -> (f32, f32) {
        let (w, _) = self.slot_width(0);
        let scale = shrink(self.scroll_target() - self.scroll);
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
        let u = self.scroll - self.scroll_target();
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
        let accel = -2.0 * OMEGA * self.vel - OMEGA * OMEGA * (self.scroll - self.scroll_target());
        self.vel += accel * dt;
        self.scroll += self.vel * dt;
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

    pub fn draw_row(
        &self,
        canvas: Canvas,
        hidden: Option<&str>,
        shake: f32,
        recede: f32,
        dim: f32,
        out: &mut Vec<Draw>,
    ) {
        let recede = recede.clamp(0.0, 1.0);
        let dim = dim.clamp(0.0, 1.0);
        let target = self.scroll_target();
        for slot in -SLOTS..=SLOTS {
            let Some(i) = self.cart_at_offset(slot) else {
                continue;
            };
            let cart = &self.carts[i];
            if hidden == Some(cart.stem.as_str()) {
                continue;
            }
            let offset = target + slot as f32 - self.scroll;
            let t = offset.abs().min(1.0);
            let scale = shrink(offset);
            let alpha = (1.0 + (SIDE_ALPHA - 1.0) * t) * (1.0 - recede);
            let (cw, ch) = cart_box(cart.platform);
            let (w, h) = (cw as f32 * scale, ch as f32 * scale);
            let away = offset.signum() * (1.0 + offset.abs());
            let x = self.centre(canvas, slot) - w / 2.0 + away * PART * recede;
            if x + w <= 0.0 || x >= canvas.w || alpha <= 0.0 {
                continue;
            }
            let x = x + shake;
            let y = foot_y(ch as f32) - h;
            if alpha < 1.0 {
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
                        alpha: alpha * dim,
                    });
                }
                if let Some(tex) = board {
                    out.push(Draw::Tex {
                        x: bx,
                        y: by,
                        w: bw,
                        h: bh,
                        tex,
                        alpha: alpha * dim,
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
                    alpha: alpha * dim,
                },
                None => {
                    let c = label_colour(&label_text(cart));
                    Draw::Rect {
                        x,
                        y,
                        w,
                        h,
                        colour: [
                            c[0] as f32 / 255.0,
                            c[1] as f32 / 255.0,
                            c[2] as f32 / 255.0,
                            alpha * dim,
                        ],
                    }
                }
            });
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
        }
    }
}

fn shrink(offset: f32) -> f32 {
    1.0 + (SIDE_SCALE - 1.0) * offset.abs().min(1.0)
}

fn recede_alpha(face_alpha: f32) -> f32 {
    (face_alpha / SIDE_ALPHA).clamp(0.0, 1.0)
}
