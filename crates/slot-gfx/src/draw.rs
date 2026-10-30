use crate::quad::Quad;
use crate::shaders::{SPRITE_FRAG, SPRITE_VERT};
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
    Game,
    Shot {
        tex: TexId,
    },
    Capture,
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
        })
    }

    pub fn set_target(&mut self, canvas: crate::Canvas) {
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

impl Drop for Sprites {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
            gl::DeleteTextures(1, &self.white);
            gl::DeleteProgram(self.prog);
        }
    }
}
