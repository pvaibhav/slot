use crate::authentic::{Ghost, Screen};
use crate::lcd3x::ScreenEffect;
use crate::power::{screen_brightness, screen_rect_above};
use crate::quad::Quad;
use crate::shaders::{GAME_FRAG, RECT_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

pub const SCALE: u32 = 3;
pub const SRC_W: u32 = OUT_W / SCALE;
pub const SRC_H: u32 = OUT_H / SCALE;

pub const WHOLE_TEXTURE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// How each new frame meets the ones before it, at the game's own size before any scaling.
/// Authentic always draws its LCD's response, whatever this is.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum FrameBlend {
    #[default]
    Off,
    /// Each frame half and half with the one before it.
    Mix,
    /// Authentic's LCD response for the screen in play: each pixel slow to change, slower
    /// while it turns lighter.
    Ghosting,
}

const FULL: [f32; 4] = [0.0, 0.0, OUT_W as f32, OUT_H as f32];

pub struct GamePass {
    prog: gl::types::GLuint,
    /// The game's last two frames, the newest at `game[at]`.
    game: [gl::types::GLuint; 2],
    at: usize,
    /// How many of `game` hold frames of the picture now showing, up to both.
    held: usize,
    blend: FrameBlend,
    u_rect: gl::types::GLint,
    u_bright: gl::types::GLint,
    u_uv: gl::types::GLint,
    u_mode: gl::types::GLint,
    u_paper_size: gl::types::GLint,
    u_fbo: gl::types::GLint,
    fbo_scale: [f32; 2],
    u_origin: gl::types::GLint,
    origin: [f32; 2],
    u_pic: gl::types::GLint,
    pic: [f32; 4],
    u_light: gl::types::GLint,
    u_dark: gl::types::GLint,
    u_response: gl::types::GLint,
    u_gamma: gl::types::GLint,
    u_least: gl::types::GLint,
    u_grain: gl::types::GLint,
    u_lcd: gl::types::GLint,
    u_ink: gl::types::GLint,
    u_glass: gl::types::GLint,
    u_sheen: gl::types::GLint,
    /// How much of the glass's sheen and of its shade the shader lays over Authentic: none
    /// when the bezel carries the glass itself, and no sheen when the reflection is off.
    sheen: [f32; 2],
    ramp: gl::types::GLuint,
    screen: Screen,
    ghost: Ghost,
    paper: gl::types::GLuint,
    paper_size: f32,
    effect: ScreenEffect,
    power: f32,
    fills: bool,
    /// Pixels kept for the carousel footer. The picture scales in the space above them.
    bottom: f32,
    src: [f32; 4],
    layout: [f32; 4],
}

impl GamePass {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, GAME_FRAG)?;
        let game = [0, 1].map(|_| {
            crate::gl::texture(SRC_W, SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::BGRA, None)
        });
        let paper = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::REPEAT,
            crate::gl::gray_format(),
            Some(&[255]),
        );
        let ramp = crate::gl::texture(
            32,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&crate::authentic::ramp_texture()),
        );
        let (u_rect, u_bright, u_uv, u_mode, u_paper_size, u_pic, u_fbo, u_origin);
        let (u_light, u_dark, u_response, u_gamma, u_least, u_grain, u_lcd, u_ink, u_glass);
        let u_sheen;
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_game"), 0);
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_src"),
                SRC_W as f32,
                SRC_H as f32,
            );
            gl::Uniform2f(
                crate::gl::uniform_location(prog, "u_target"),
                OUT_W as f32,
                OUT_H as f32,
            );
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_bright = crate::gl::uniform_location(prog, "u_bright");
            u_uv = crate::gl::uniform_location(prog, "u_uv");
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_paper"), 1);
            u_mode = crate::gl::uniform_location(prog, "u_mode");
            u_paper_size = crate::gl::uniform_location(prog, "u_paper_size");
            u_fbo = crate::gl::uniform_location(prog, "u_fbo");
            u_pic = crate::gl::uniform_location(prog, "u_pic");
            u_origin = crate::gl::uniform_location(prog, "u_origin");
            u_light = crate::gl::uniform_location(prog, "u_light");
            u_dark = crate::gl::uniform_location(prog, "u_dark");
            u_response = crate::gl::uniform_location(prog, "u_response");
            u_gamma = crate::gl::uniform_location(prog, "u_gamma");
            u_least = crate::gl::uniform_location(prog, "u_least");
            u_grain = crate::gl::uniform_location(prog, "u_grain");
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_ramp"), 2);
            u_lcd = crate::gl::uniform_location(prog, "u_lcd");
            u_ink = crate::gl::uniform_location(prog, "u_ink");
            u_glass = crate::gl::uniform_location(prog, "u_glass");
            u_sheen = crate::gl::uniform_location(prog, "u_sheen");
        }
        let mut ghost = Ghost::new()?;
        ghost.set_filter(gl::NEAREST);
        Ok(GamePass {
            prog,
            game,
            at: 0,
            held: 0,
            blend: FrameBlend::Off,
            u_rect,
            u_bright,
            u_uv,
            u_mode,
            u_paper_size,
            u_fbo,
            fbo_scale: [1.0, 1.0],
            u_origin,
            origin: [0.0, 0.0],
            u_pic,
            pic: WHOLE_TEXTURE,
            u_light,
            u_dark,
            u_response,
            u_gamma,
            u_least,
            u_grain,
            u_lcd,
            u_ink,
            u_glass,
            u_sheen,
            sheen: [1.0; 2],
            ramp,
            screen: Screen::Gba,
            ghost,
            paper,
            paper_size: 1.0,
            effect: ScreenEffect::Lcd3x,
            power: 1.0,
            fills: false,
            bottom: 0.0,
            src: WHOLE_TEXTURE,
            layout: FULL,
        })
    }

    pub fn set_power(&mut self, t: f32) {
        self.power = t.clamp(0.0, 1.0);
    }

    pub fn set_bottom(&mut self, px: f32) {
        self.bottom = px.max(0.0);
    }

    pub fn frame(&self) -> (f32, f32, f32, f32) {
        let foot = self.bottom;
        match self.fills {
            true => (0.0, 0.0, OUT_W as f32, OUT_H as f32 - foot),
            false => screen_rect_above(crate::Canvas::FRAMED, self.power, foot),
        }
    }

    pub fn set_layout(&mut self, rect: [f32; 4]) {
        self.layout = rect;
    }

    pub fn set_fills_frame(&mut self, on: bool) {
        self.fills = on;
    }

    pub fn set_picture(&mut self, rect: [f32; 4]) {
        self.pic = rect;
    }

    pub fn fbo_scale(&self) -> [f32; 2] {
        self.fbo_scale
    }

    pub fn origin(&self) -> [f32; 2] {
        self.origin
    }

    pub fn picture_out(&self) -> [f32; 4] {
        let (rx, ry, rw, rh) = placed_in(self.frame(), self.layout);
        let [sx, sy, sw, sh] = self.src;
        let [p0, q0, p1, q1] = self.pic;
        let (u0, v0) = (p0.max(sx), q0.max(sy));
        let (u1, v1) = (p1.min(sx + sw), q1.min(sy + sh));
        let x = rx + (u0 - sx) / sw * rw;
        let y = ry + (v0 - sy) / sh * rh;
        [x, y, (u1 - u0) / sw * rw, (v1 - v0) / sh * rh]
    }

    pub fn set_fbo_scale(&mut self, x: f32, y: f32) {
        self.fbo_scale = [x, y];
    }

    pub fn set_origin(&mut self, x: f32, y: f32) {
        self.origin = [x, y];
    }

    pub fn set_effect(&mut self, effect: ScreenEffect, quad: &Quad) {
        if effect != self.effect {
            self.effect = effect;
            self.ghost.set_filter(match effect {
                ScreenEffect::Authentic => gl::LINEAR,
                _ => gl::NEAREST,
            });
            self.reblend(quad);
        }
    }

    pub fn set_blend(&mut self, blend: FrameBlend, quad: &Quad) {
        if blend != self.blend {
            self.blend = blend;
            self.reblend(quad);
        }
    }

    /// Blend from the next frame on with nothing before it: the picture jumped.
    pub fn cut(&mut self) {
        self.held = 0;
        self.ghost.reset();
    }

    /// Blend the last two frames again the new way, so a paused game shows the change.
    /// Ghosting starts from the frame before; its longer history is gone.
    fn reblend(&mut self, quad: &Quad) {
        self.ghost.reset();
        let (game, last) = (self.game[self.at], self.game[1 - self.at]);
        match (self.blend(), self.held) {
            (_, 0) | (FrameBlend::Off, _) => {}
            // One frame since the last cut: it is all there is to blend the next one with.
            (_, 1) => self.ghost.respond(game, [0.0; 2], quad),
            (FrameBlend::Mix, _) => {
                self.ghost.respond(last, [0.0; 2], quad);
                self.ghost.mix(game, last, quad);
            }
            (FrameBlend::Ghosting, _) => {
                self.ghost.respond(last, [0.0; 2], quad);
                self.ghost.respond(game, self.screen.look().ghost, quad);
            }
        }
    }

    fn blend(&self) -> FrameBlend {
        match self.effect {
            ScreenEffect::Authentic => FrameBlend::Ghosting,
            _ => self.blend,
        }
    }

    pub fn set_glass(&mut self, sheen: bool, shade: bool) {
        self.sheen = [sheen, shade].map(|on| if on { 1.0 } else { 0.0 });
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }

    pub fn set_paper(&mut self, size: u32, gray: &[u8]) {
        let tex = crate::gl::texture(
            size,
            size,
            gl::NEAREST,
            gl::REPEAT,
            crate::gl::gray_format(),
            Some(gray),
        );
        unsafe { gl::DeleteTextures(1, &self.paper) };
        self.paper = tex;
        self.paper_size = size as f32;
    }

    pub fn set_source_rect(&mut self, rect: [f32; 4]) {
        self.src = rect;
    }

    pub fn upload(&mut self, xrgb8888: &[u8], quad: &Quad) {
        if xrgb8888.len() < (SRC_W * SRC_H * 4) as usize {
            return;
        }
        let last = self.game[self.at];
        self.at = 1 - self.at;
        let game = self.game[self.at];
        self.held = (self.held + 1).min(2);
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, game);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                SRC_W as i32,
                SRC_H as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
        }
        match self.blend() {
            FrameBlend::Off => {}
            FrameBlend::Mix => self.ghost.mix(game, last, quad),
            FrameBlend::Ghosting => self.ghost.respond(game, self.screen.look().ghost, quad),
        }
    }

    pub fn draw(&self, quad: &Quad) {
        let src = match self.effect {
            ScreenEffect::Grid if self.layout == FULL => {
                whole_scale_source(self.src, self.fbo_scale)
            }
            _ => self.src,
        };
        let game = self.game[self.at];
        let shown = match self.blend() {
            FrameBlend::Off => game,
            FrameBlend::Mix | FrameBlend::Ghosting => self.ghost.texture().unwrap_or(game),
        };
        self.draw_source(shown, quad, src, self.layout);
    }

    pub fn draw_still(&self, tex: gl::types::GLuint, quad: &Quad) {
        self.draw_source(tex, quad, self.src, self.layout);
    }

    fn draw_source(&self, tex: gl::types::GLuint, quad: &Quad, src: [f32; 4], layout: [f32; 4]) {
        let (mut x, mut y, mut w, mut h) = placed_in(self.frame(), layout);
        let bright = if self.fills {
            1.0
        } else {
            screen_brightness(self.power)
        };
        // The footer covers the bottom of a full-frame picture. Keep the part that sat above
        // it, so opening the scale does not squash the picture into the shorter rect.
        let mut src = src;
        if self.bottom > 0.0 {
            src[3] *= (OUT_H as f32 - self.bottom) / OUT_H as f32;
        }
        if self.effect == ScreenEffect::Authentic && self.pic != WHOLE_TEXTURE {
            // The lit panel fills the screen around a Game Boy or GBC picture, with the same
            // glass over it; a bezel, when one is chosen, is drawn on top.
            let (fx, fy, fw, fh) = placed_in(self.frame(), FULL);
            src = [
                src[0] + (fx - x) / w * src[2],
                src[1] + (fy - y) / h * src[3],
                src[2] * fw / w,
                src[3] * fh / h,
            ];
            (x, y, w, h) = (fx, fy, fw, fh);
        }
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform4f(self.u_rect, x, y, w, h);
            gl::Uniform4f(self.u_uv, src[0], src[1], src[2], src[3]);
            gl::Uniform1f(self.u_bright, bright);
            gl::Uniform1f(self.u_mode, self.effect.mode());
            gl::Uniform1f(self.u_paper_size, self.paper_size);
            gl::Uniform2f(self.u_fbo, self.fbo_scale[0], self.fbo_scale[1]);
            gl::Uniform2f(self.u_origin, self.origin[0], self.origin[1]);
            gl::Uniform4f(
                self.u_pic,
                self.pic[0],
                self.pic[1],
                self.pic[2],
                self.pic[3],
            );
            if self.effect == ScreenEffect::Authentic {
                let look = self.screen.look();
                let [r, g, b] = look.light;
                gl::Uniform3f(self.u_light, r, g, b);
                let [r, g, b] = look.dark;
                gl::Uniform3f(self.u_dark, r, g, b);
                gl::UniformMatrix3fv(self.u_response, 1, gl::FALSE, look.response.as_ptr());
                let [r, g, b] = look.gamma;
                gl::Uniform3f(self.u_gamma, r, g, b);
                let [r, g, b] = look.least;
                gl::Uniform3f(self.u_least, r, g, b);
                gl::Uniform1f(self.u_grain, look.grain);
                let [x, y, z, w] = look.lcd;
                gl::Uniform4f(self.u_lcd, x, y, z, w);
                let [x, y, z, w] = look.ink;
                gl::Uniform4f(self.u_ink, x, y, z, w);
                let [x, y, z] = look.glass;
                gl::Uniform3f(self.u_glass, x, y, z);
                gl::Uniform2f(self.u_sheen, self.sheen[0], self.sheen[1]);
                gl::ActiveTexture(gl::TEXTURE2);
                gl::BindTexture(gl::TEXTURE_2D, self.ramp);
            }
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, self.paper);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
        }
        quad.draw();
    }
}

fn placed_in(frame: (f32, f32, f32, f32), layout: [f32; 4]) -> (f32, f32, f32, f32) {
    let (x, y, w, h) = frame;
    let [lx, ly, lw, lh] = layout;
    let (sx, sy) = (lw / OUT_W as f32, lh / OUT_H as f32);
    (lx + x * sx, ly + y * sy, w * sx, h * sy)
}

fn whole_scale_source(src: [f32; 4], fbo: [f32; 2]) -> [f32; 4] {
    let (x, w) = whole_scale(src[0], src[2], SRC_W as f32, OUT_W as f32 * fbo[0]);
    let (y, h) = whole_scale(src[1], src[3], SRC_H as f32, OUT_H as f32 * fbo[1]);
    [x, y, w, h]
}

fn whole_scale(at: f32, len: f32, texels: f32, out: f32) -> (f32, f32) {
    let shown = len * texels;
    let scale = (out / shown).ceil();
    let trim = scale * shown - out;
    if trim >= 0.1 * shown {
        return (at, len);
    }
    let top = (trim / 2.0).floor();
    (at + top / scale / texels, out / scale / texels)
}

impl Drop for GamePass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(2, self.game.as_ptr());
            gl::DeleteTextures(1, &self.paper);
            gl::DeleteTextures(1, &self.ramp);
            gl::DeleteProgram(self.prog);
        }
    }
}
