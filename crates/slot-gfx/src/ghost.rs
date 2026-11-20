use crate::pipeline::{SRC_H, SRC_W};
use crate::quad::Quad;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum FrameBlend {
    #[default]
    Off,
    Mix,
    Ghosting,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Lcd {
    Dmg,
    Gbc,
    #[default]
    Gba,
}

impl Lcd {
    pub fn response(self) -> [f32; 2] {
        match self {
            Lcd::Dmg => [keep(61.0), keep(21.0)],
            Lcd::Gbc => [keep(16.0), keep(8.0)],
            Lcd::Gba => [keep(20.0), keep(10.0)],
        }
    }
}

fn keep(ms: f32) -> f32 {
    (-1000.0 / 59.73 / ms).exp()
}

pub(crate) struct Ghost {
    prog: gl::types::GLuint,
    u_keep: gl::types::GLint,
    tex: [gl::types::GLuint; 2],
    fbo: [gl::types::GLuint; 2],
    shown: usize,
    primed: bool,
}

impl Ghost {
    pub fn new() -> Result<Self, crate::surface::GfxError> {
        let prog = crate::shaders::program(crate::shaders::BLIT_VERT, crate::shaders::GHOST_FRAG)?;
        let tex = [0, 1].map(|_| {
            crate::gl::texture(SRC_W, SRC_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None)
        });
        let mut fbo = [0; 2];
        unsafe {
            gl::GenFramebuffers(2, fbo.as_mut_ptr());
            for (f, t) in fbo.iter().zip(tex) {
                gl::BindFramebuffer(gl::FRAMEBUFFER, *f);
                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    t,
                    0,
                );
            }
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_prev"), 1);
            gl::Uniform4f(
                crate::gl::uniform_location(prog, "u_uv"),
                0.0,
                0.0,
                1.0,
                1.0,
            );
        }
        let u_keep = crate::gl::uniform_location(prog, "u_keep");
        Ok(Ghost {
            prog,
            u_keep,
            tex,
            fbo,
            shown: 0,
            primed: false,
        })
    }

    pub fn set_filter(&mut self, filter: gl::types::GLenum) {
        unsafe {
            for t in self.tex {
                gl::BindTexture(gl::TEXTURE_2D, t);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, filter as i32);
                gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, filter as i32);
            }
        }
    }

    pub fn reset(&mut self) {
        self.primed = false;
    }

    pub fn respond(&mut self, game: gl::types::GLuint, keep: [f32; 2], quad: &Quad) {
        self.draw(game, self.tex[self.shown], keep, quad);
    }

    pub fn mix(&mut self, game: gl::types::GLuint, last: gl::types::GLuint, quad: &Quad) {
        self.draw(game, last, [0.5; 2], quad);
    }

    fn draw(
        &mut self,
        game: gl::types::GLuint,
        prev: gl::types::GLuint,
        keep: [f32; 2],
        quad: &Quad,
    ) {
        let next = 1 - self.shown;
        let keep = if self.primed { keep } else { [0.0; 2] };
        unsafe {
            let mut fb = 0;
            let mut view = [0; 4];
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut fb);
            gl::GetIntegerv(gl::VIEWPORT, view.as_mut_ptr());
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo[next]);
            gl::Viewport(0, 0, SRC_W as i32, SRC_H as i32);
            gl::UseProgram(self.prog);
            gl::Uniform2f(self.u_keep, keep[0], keep[1]);
            gl::ActiveTexture(gl::TEXTURE1);
            gl::BindTexture(gl::TEXTURE_2D, prev);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, game);
            quad.draw();
            gl::BindFramebuffer(gl::FRAMEBUFFER, fb as gl::types::GLuint);
            gl::Viewport(view[0], view[1], view[2], view[3]);
        }
        self.shown = next;
        self.primed = true;
    }

    pub fn texture(&self) -> Option<gl::types::GLuint> {
        self.primed.then_some(self.tex[self.shown])
    }
}

impl Drop for Ghost {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(2, self.fbo.as_ptr());
            gl::DeleteTextures(2, self.tex.as_ptr());
            gl::DeleteProgram(self.prog);
        }
    }
}
