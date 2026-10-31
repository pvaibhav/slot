use crate::quad::Quad;
use crate::shaders::{PAPER_FRAG, PAPER_VERT, SPRITE_FRAG, SPRITE_VERT};
use crate::surface::{GfxError, OUT_H, OUT_W};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct TexId(usize);

#[cfg(feature = "test-support")]
impl TexId {
    pub const fn from_raw(id: usize) -> Self {
        TexId(id)
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Draw {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        colour: [f32; 4],
    },
    Tex {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
    },
    Part {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        uv: [f32; 4],
    },
    Turned {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex: TexId,
        alpha: f32,
        turn: f32,
    },
    Quad {
        corners: [[f32; 2]; 4],
        tex: TexId,
        uv: [f32; 4],
        alpha: f32,
    },
    Paper {
        tex: TexId,
        pivot: [f32; 2],
        scale: f32,
        lift: f32,
        fold: f32,
        tear: f32,
        bow: bool,
        flap: bool,
    },
    Game,
    Shot {
        tex: TexId,
    },
    Capture,
}

const SHEET: [f32; 2] = [226.0, 425.0];
const FOLD_DIR: [f32; 2] = [-0.62, -0.78];
const FOLD_R: f32 = 24.0;
const FLAP_SQUASH: f32 = 1.5;
const BOW_SPREAD: f32 = 6.0;

pub fn paper_flap_shows(fold: f32) -> bool {
    fold >= FOLD_R * std::f32::consts::FRAC_PI_2
}

fn flap_point(q: [f32; 2], fold: f32) -> [f32; 2] {
    let s = q[0] * FOLD_DIR[0] + q[1] * FOLD_DIR[1] + fold;
    let round = FOLD_R * std::f32::consts::PI;
    let d = match s > round {
        true => (round - s) / FLAP_SQUASH,
        false => FOLD_R,
    };
    [q[0] + FOLD_DIR[0] * (d - s), q[1] + FOLD_DIR[1] * (d - s)]
}

pub fn paper_bounds(pivot: [f32; 2], scale: f32, lift: f32, fold: f32, flap: bool) -> [f32; 4] {
    let (mut lo, mut hi) = ([0.0f32, 0.0], SHEET);
    if flap {
        for q in [[0.0, 0.0], [SHEET[0], 0.0], [0.0, SHEET[1]], SHEET] {
            let p = flap_point(q, fold);
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
    }
    let pad = 1.2 * lift + BOW_SPREAD + FOLD_R;
    let x = pivot[0] + (lo[0] - SHEET[0]) * scale - pad;
    let y = pivot[1] + (lo[1] - SHEET[1]) * scale - pad;
    let w = (hi[0] - lo[0]) * scale + 2.0 * pad;
    let h = (hi[1] - lo[1]) * scale + 2.0 * pad;
    [x, y, w, h]
}

struct PaperPass {
    prog: gl::types::GLuint,
    u_rect: gl::types::GLint,
    u_target: gl::types::GLint,
    u_pv: gl::types::GLint,
    u_sc: gl::types::GLint,
    u_lift: gl::types::GLint,
    u_tear: gl::types::GLint,
    u_fold: gl::types::GLint,
    u_mode: gl::types::GLint,
    u_bow: gl::types::GLint,
}

impl PaperPass {
    fn new() -> Result<PaperPass, GfxError> {
        let prog = crate::shaders::program(PAPER_VERT, PAPER_FRAG)?;
        let at = |name: &str| crate::gl::uniform_location(prog, name);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(at("u_tex"), 0);
        }
        Ok(PaperPass {
            prog,
            u_rect: at("u_rect"),
            u_target: at("u_target"),
            u_pv: at("pv"),
            u_sc: at("sc"),
            u_lift: at("lift"),
            u_tear: at("tear"),
            u_fold: at("fold"),
            u_mode: at("mode"),
            u_bow: at("bow"),
        })
    }
}

impl Drop for PaperPass {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.prog) };
    }
}

pub struct Sprites {
    prog: gl::types::GLuint,
    white: gl::types::GLuint,
    textures: Vec<gl::types::GLuint>,
    u_rect: gl::types::GLint,
    u_colour: gl::types::GLint,
    u_turn: gl::types::GLint,
    u_target: gl::types::GLint,
    u_src: gl::types::GLint,
    u_quad: gl::types::GLint,
    u_c01: gl::types::GLint,
    u_c23: gl::types::GLint,
    target: crate::Canvas,
    paper: Option<PaperPass>,
}

impl Sprites {
    pub fn new() -> Result<Self, GfxError> {
        let prog = crate::shaders::program(SPRITE_VERT, SPRITE_FRAG)?;
        let white = crate::gl::texture(
            1,
            1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            Some(&[255u8; 4]),
        );
        let (u_rect, u_colour, u_turn, u_target, u_src, u_quad, u_c01, u_c23);
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            u_target = crate::gl::uniform_location(prog, "u_target");
            gl::Uniform2f(u_target, OUT_W as f32, OUT_H as f32);
            u_src = crate::gl::uniform_location(prog, "u_src");
            u_rect = crate::gl::uniform_location(prog, "u_rect");
            u_colour = crate::gl::uniform_location(prog, "u_colour");
            u_turn = crate::gl::uniform_location(prog, "u_turn");
            u_quad = crate::gl::uniform_location(prog, "u_quad");
            u_c01 = crate::gl::uniform_location(prog, "u_c01");
            u_c23 = crate::gl::uniform_location(prog, "u_c23");
        }
        Ok(Sprites {
            prog,
            white,
            textures: Vec::new(),
            u_rect,
            u_colour,
            u_turn,
            u_target,
            u_src,
            u_quad,
            u_c01,
            u_c23,
            target: crate::Canvas::FRAMED,
            paper: None,
        })
    }

    pub fn prepare_paper(&mut self) -> Result<(), GfxError> {
        if self.paper.is_none() {
            self.paper = Some(PaperPass::new()?);
        }
        Ok(())
    }

    pub fn set_target(&mut self, canvas: crate::Canvas) {
        self.target = canvas;
        unsafe {
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_target, canvas.w, canvas.h);
        }
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::LINEAR)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.push(w, h, rgba, gl::NEAREST)
    }

    fn push(&mut self, w: u32, h: u32, rgba: &[u8], filter: gl::types::GLenum) -> TexId {
        let tex = crate::gl::texture(w, h, filter, gl::CLAMP_TO_EDGE, gl::RGBA, Some(rgba));
        self.textures.push(tex);
        TexId(self.textures.len() - 1)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        let Some(tex) = self.textures.get(id.0) else {
            return;
        };
        if rgba.len() < (w * h * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                crate::gl::internal_format(gl::RGBA),
                w as i32,
                h as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    pub fn source(&self, id: TexId) -> Option<gl::types::GLuint> {
        self.textures.get(id.0).copied()
    }

    pub fn draw(&self, items: &[Draw], quad: &Quad) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
        }
        for item in items {
            let whole = [0.0, 0.0, 1.0, 1.0];
            let mut corners = None;
            let (x, y, w, h, tex, colour, turn, uv) = match *item {
                Draw::Rect { x, y, w, h, colour } => (x, y, w, h, self.white, colour, 0.0, whole),
                Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], 0.0, whole),
                    None => continue,
                },
                Draw::Part {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    uv,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], 0.0, uv),
                    None => continue,
                },
                Draw::Turned {
                    x,
                    y,
                    w,
                    h,
                    tex,
                    alpha,
                    turn,
                } => match self.textures.get(tex.0) {
                    Some(t) => (x, y, w, h, *t, [1.0, 1.0, 1.0, alpha], turn, whole),
                    None => continue,
                },
                Draw::Quad {
                    corners: c,
                    tex,
                    uv,
                    alpha,
                } => match self.textures.get(tex.0) {
                    Some(t) => {
                        corners = Some(c);
                        (0.0, 0.0, 1.0, 1.0, *t, [1.0, 1.0, 1.0, alpha], 0.0, uv)
                    }
                    None => continue,
                },
                Draw::Paper { .. } => {
                    self.draw_paper(item, quad);
                    continue;
                }
                Draw::Game | Draw::Shot { .. } | Draw::Capture => continue,
            };
            let (cos, sin) = if turn == 0.0 {
                (1.0, 0.0)
            } else {
                (turn.cos(), turn.sin())
            };
            unsafe {
                gl::BindTexture(gl::TEXTURE_2D, tex);
                gl::Uniform4f(self.u_rect, x, y, w, h);
                gl::Uniform2f(self.u_turn, cos, sin);
                gl::Uniform4f(self.u_src, uv[0], uv[1], uv[2], uv[3]);
                gl::Uniform4f(self.u_colour, colour[0], colour[1], colour[2], colour[3]);
                match corners {
                    Some(c) => {
                        gl::Uniform1f(self.u_quad, 1.0);
                        gl::Uniform4f(self.u_c01, c[0][0], c[0][1], c[1][0], c[1][1]);
                        gl::Uniform4f(self.u_c23, c[2][0], c[2][1], c[3][0], c[3][1]);
                    }
                    None => gl::Uniform1f(self.u_quad, 0.0),
                }
            }
            quad.draw();
        }
        unsafe { gl::Disable(gl::BLEND) };
    }
}

impl Sprites {
    fn draw_paper(&self, item: &Draw, quad: &Quad) {
        let Draw::Paper {
            tex,
            pivot,
            scale,
            lift,
            fold,
            tear,
            bow,
            flap,
        } = *item
        else {
            return;
        };
        let (Some(pass), Some(tex)) = (&self.paper, self.textures.get(tex.0)) else {
            return;
        };
        let [x, y, w, h] = paper_bounds(pivot, scale, lift, fold, flap);
        let (left, top) = (x.max(0.0), y.max(0.0));
        let (right, bottom) = ((x + w).min(self.target.w), (y + h).min(self.target.h));
        if right <= left || bottom <= top {
            return;
        }
        unsafe {
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(pass.prog);
            gl::BindTexture(gl::TEXTURE_2D, *tex);
            gl::Uniform4f(pass.u_rect, left, top, right - left, bottom - top);
            gl::Uniform2f(pass.u_target, self.target.w, self.target.h);
            gl::Uniform2f(pass.u_pv, pivot[0], pivot[1]);
            gl::Uniform1f(pass.u_sc, scale);
            gl::Uniform1f(pass.u_lift, lift);
            gl::Uniform1f(pass.u_tear, tear);
            gl::Uniform1f(pass.u_fold, fold);
            gl::Uniform1f(pass.u_mode, if flap { 1.0 } else { 0.0 });
            gl::Uniform1f(pass.u_bow, if bow { 1.0 } else { 0.0 });
        }
        quad.draw();
        unsafe {
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
        }
    }
}

impl Drop for Sprites {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
            gl::DeleteTextures(1, &self.white);
            gl::DeleteProgram(self.prog);
        }
    }
}
