use crate::draw::TexId;
use crate::quad::Quad;
use crate::shaders::RECT_VERT;
use crate::surface::GfxError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Overlay {
    pub full: Option<(TexId, u32, u32)>,
    pub tile: Option<(TexId, u32, u32)>,
}

const OVERLAY_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
uniform vec2 u_tile;
uniform vec2 u_corner;
varying vec2 v_uv;
void main() {
    vec2 p = floor(vec2(gl_FragCoord.x - u_corner.x, u_corner.y - gl_FragCoord.y));
    vec2 cell = mod(p, u_tile);
    FRAG_COLOR = texture2D(u_tex, (cell + 0.5) / u_tile);
}
"#;

pub struct OverlayPass {
    prog: gl::types::GLuint,
    rect: gl::types::GLint,
    target: gl::types::GLint,
    tile: gl::types::GLint,
    corner: gl::types::GLint,
}

impl OverlayPass {
    pub fn new() -> Result<OverlayPass, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, OVERLAY_FRAG)?;
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
        }
        Ok(OverlayPass {
            rect: crate::gl::uniform_location(prog, "u_rect"),
            target: crate::gl::uniform_location(prog, "u_target"),
            tile: crate::gl::uniform_location(prog, "u_tile"),
            corner: crate::gl::uniform_location(prog, "u_corner"),
            prog,
        })
    }

    pub fn draw_tile(
        &self,
        tex: gl::types::GLuint,
        tile: (u32, u32),
        picture_px: [f32; 4],
        target: (u32, u32),
        quad: &Quad,
    ) {
        let [x, y, w, h] = picture_px;
        self.draw(
            tex,
            tile,
            [x, y, w, h],
            (x, target.1 as f32 - y),
            target,
            quad,
        );
    }

    pub fn draw_full(&self, tex: gl::types::GLuint, target: (u32, u32), quad: &Quad) {
        let (w, h) = (target.0 as f32, target.1 as f32);
        self.draw(tex, target, [0.0, 0.0, w, h], (0.0, h), target, quad);
    }

    fn draw(
        &self,
        tex: gl::types::GLuint,
        tile: (u32, u32),
        rect: [f32; 4],
        corner: (f32, f32),
        target: (u32, u32),
        quad: &Quad,
    ) {
        unsafe {
            let mut viewport = [0i32; 4];
            gl::GetIntegerv(gl::VIEWPORT, viewport.as_mut_ptr());
            let blending = gl::IsEnabled(gl::BLEND) == gl::TRUE;
            let (mut src, mut dst) = (0i32, 0i32);
            gl::GetIntegerv(gl::BLEND_SRC_RGB, &mut src);
            gl::GetIntegerv(gl::BLEND_DST_RGB, &mut dst);
            gl::Viewport(0, 0, target.0 as i32, target.1 as i32);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::Uniform4f(self.rect, rect[0], rect[1], rect[2], rect[3]);
            gl::Uniform2f(self.target, target.0 as f32, target.1 as f32);
            gl::Uniform2f(self.tile, tile.0 as f32, tile.1 as f32);
            gl::Uniform2f(self.corner, corner.0, corner.1);
            quad.draw();
            gl::BlendFunc(src as u32, dst as u32);
            if !blending {
                gl::Disable(gl::BLEND);
            }
            gl::Viewport(viewport[0], viewport[1], viewport[2], viewport[3]);
        }
    }
}

impl Drop for OverlayPass {
    fn drop(&mut self) {
        unsafe { gl::DeleteProgram(self.prog) };
    }
}
