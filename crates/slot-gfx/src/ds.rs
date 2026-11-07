use crate::quad::Quad;
use crate::shaders::RECT_VERT;
use crate::surface::GfxError;

pub const DS_SCREEN_W: u32 = 256;
pub const DS_SCREEN_H: u32 = 192;

const DS_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
uniform float u_half;
uniform float u_bright;
uniform vec3 u_gain;
uniform float u_saturation;
uniform float u_contrast;
varying vec2 v_uv;
void main() {
    vec2 uv = vec2(v_uv.x, (u_half + v_uv.y) * 0.5);
    vec3 c = clamp(texture2D(u_tex, uv).rgb * u_bright, 0.0, 1.0);
    c = mix(vec3(dot(c, vec3(0.2126, 0.7152, 0.0722))), c, 1.0 + u_saturation);
    c = (c - 0.5) * (1.0 + u_contrast) + 0.5;
    FRAG_COLOR = vec4(clamp(c, 0.0, 1.0) * u_gain, 1.0);
}
"#;

const FPS_FRAG: &str = r#"
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif
uniform sampler2D u_tex;
varying vec2 v_uv;
void main() {
    FRAG_COLOR = vec4(texture2D(u_tex, v_uv).rgb, 1.0);
}
"#;

const GLYPH_W: usize = 3;
const GLYPH_H: usize = 5;
const FPS_CHARS: usize = 6;
const FPS_W: usize = 1 + FPS_CHARS * (GLYPH_W + 1);
const FPS_H: usize = GLYPH_H + 2;
const FPS_SCALE: i32 = 4;
const FPS_AT: i32 = 8;

const GLYPHS: [(char, [u8; GLYPH_H]); 11] = [
    ('0', [0b111, 0b101, 0b101, 0b101, 0b111]),
    ('1', [0b010, 0b110, 0b010, 0b010, 0b111]),
    ('2', [0b111, 0b001, 0b111, 0b100, 0b111]),
    ('3', [0b111, 0b001, 0b111, 0b001, 0b111]),
    ('4', [0b101, 0b101, 0b111, 0b001, 0b001]),
    ('5', [0b111, 0b100, 0b111, 0b001, 0b111]),
    ('6', [0b111, 0b100, 0b111, 0b101, 0b111]),
    ('7', [0b111, 0b001, 0b010, 0b010, 0b010]),
    ('8', [0b111, 0b101, 0b111, 0b101, 0b111]),
    ('9', [0b111, 0b101, 0b111, 0b001, 0b111]),
    ('.', [0b000, 0b000, 0b000, 0b000, 0b010]),
];

pub fn fps_text(fps: f32) -> String {
    format!("{fps:.1}")
}

fn fps_pixels(text: &str) -> Vec<u8> {
    let chars: Vec<char> = text.chars().take(FPS_CHARS).collect();
    let mut rgba = vec![0u8; FPS_W * FPS_H * 4];
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    for (i, c) in chars.iter().enumerate() {
        let Some((_, rows)) = GLYPHS.iter().find(|(g, _)| g == c) else {
            continue;
        };
        for (y, row) in rows.iter().enumerate() {
            for x in 0..GLYPH_W {
                if row >> (GLYPH_W - 1 - x) & 1 == 1 {
                    let o = ((y + 1) * FPS_W + 1 + i * (GLYPH_W + 1) + x) * 4;
                    rgba[o..o + 3].copy_from_slice(&[255, 255, 255]);
                }
            }
        }
    }
    rgba
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DsOverlay {
    pub tile: Option<(gl::types::GLuint, u32, u32)>,
    pub top: Option<gl::types::GLuint>,
    pub bottom: Option<gl::types::GLuint>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DsScreen {
    Top,
    Bottom,
}

pub fn ds_rect(panel: (u32, u32)) -> [i32; 4] {
    let k = (panel.0 / DS_SCREEN_W).min(panel.1 / DS_SCREEN_H).max(1);
    let (w, h) = (DS_SCREEN_W * k, DS_SCREEN_H * k);
    [
        (panel.0 as i32 - w as i32) / 2,
        (panel.1 as i32 - h as i32) / 2,
        w as i32,
        h as i32,
    ]
}

pub fn ds_rect_powered(panel: (u32, u32), t: f32) -> [i32; 4] {
    let [x, y, w, h] = ds_rect(panel);
    let (sw, sh) = crate::power::screen_squash(w as f32, h as f32, t);
    let (pw, ph) = ((w as f32 * sw).round() as i32, (h as f32 * sh).round() as i32);
    [x + (w - pw) / 2, y + (h - ph) / 2, pw, ph]
}

pub struct DsPass {
    prog: gl::types::GLuint,
    tex: gl::types::GLuint,
    quad: Quad,
    rect: gl::types::GLint,
    target: gl::types::GLint,
    half: gl::types::GLint,
    bright: gl::types::GLint,
    u_gain: gl::types::GLint,
    u_saturation: gl::types::GLint,
    u_contrast: gl::types::GLint,
    grade: crate::grade::ColorGrade,
    fps_prog: gl::types::GLuint,
    fps_tex: gl::types::GLuint,
    overlay: Option<DsOverlay>,
    overlay_pass: crate::overlay::OverlayPass,
}

impl DsPass {
    pub fn new() -> Result<DsPass, GfxError> {
        let prog = crate::shaders::program(RECT_VERT, DS_FRAG)?;
        let tex = crate::gl::texture(
            DS_SCREEN_W,
            DS_SCREEN_H * 2,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::BGRA,
            None,
        );
        let fps_prog = crate::shaders::program(RECT_VERT, FPS_FRAG)?;
        let fps_tex = crate::gl::texture(
            FPS_W as u32,
            FPS_H as u32,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            None,
        );
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::UseProgram(fps_prog);
            gl::Uniform1i(crate::gl::uniform_location(fps_prog, "u_tex"), 0);
        }
        Ok(DsPass {
            overlay: None,
            overlay_pass: crate::overlay::OverlayPass::new()?,
            fps_prog,
            fps_tex,
            rect: crate::gl::uniform_location(prog, "u_rect"),
            target: crate::gl::uniform_location(prog, "u_target"),
            half: crate::gl::uniform_location(prog, "u_half"),
            bright: crate::gl::uniform_location(prog, "u_bright"),
            u_gain: crate::gl::uniform_location(prog, "u_gain"),
            u_saturation: crate::gl::uniform_location(prog, "u_saturation"),
            u_contrast: crate::gl::uniform_location(prog, "u_contrast"),
            grade: crate::grade::ColorGrade::default(),
            prog,
            tex,
            quad: Quad::new(),
        })
    }

    pub fn upload(&mut self, xrgb8888: &[u8]) {
        if xrgb8888.len() < (DS_SCREEN_W * DS_SCREEN_H * 2 * 4) as usize {
            return;
        }
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                DS_SCREEN_W as i32,
                (DS_SCREEN_H * 2) as i32,
                gl::BGRA,
                gl::UNSIGNED_BYTE,
                xrgb8888.as_ptr() as *const std::ffi::c_void,
            );
        }
    }

    pub fn clear(&self, panel: (u32, u32)) {
        unsafe {
            gl::Viewport(0, 0, panel.0 as i32, panel.1 as i32);
            gl::Disable(gl::SCISSOR_TEST);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    pub fn set_overlay(&mut self, overlay: Option<DsOverlay>) {
        self.overlay = overlay;
    }

    pub fn create_overlay_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> gl::types::GLuint {
        crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, Some(rgba))
    }

    pub fn delete_overlay_texture(&mut self, tex: gl::types::GLuint) {
        unsafe { gl::DeleteTextures(1, &tex) };
    }

    fn draw_overlay(&self, screen: DsScreen, panel: (u32, u32)) {
        let Some(overlay) = self.overlay else {
            return;
        };
        let full = match screen {
            DsScreen::Top => overlay.top,
            DsScreen::Bottom => overlay.bottom,
        };
        if let Some(tex) = full {
            self.overlay_pass.draw_full(tex, panel, &self.quad);
        }
        if let Some((tex, w, h)) = overlay.tile {
            let [x, y, rw, rh] = ds_rect(panel);
            self.overlay_pass.draw_tile(
                tex,
                (w, h),
                [x as f32, y as f32, rw as f32, rh as f32],
                panel,
                &self.quad,
            );
        }
    }

    pub fn set_grade(&mut self, grade: crate::grade::ColorGrade) {
        self.grade = grade;
    }

    pub fn draw_screen(&self, screen: DsScreen, panel: (u32, u32)) {
        self.draw_screen_powered(screen, panel, 1.0);
    }

    pub fn draw_screen_powered(&self, screen: DsScreen, panel: (u32, u32), t: f32) {
        self.clear(panel);
        if t <= 0.0 {
            return;
        }
        let [x, y, w, h] = ds_rect_powered(panel, t);
        unsafe {
            gl::Disable(gl::BLEND);
            gl::UseProgram(self.prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
            gl::Uniform4f(self.rect, x as f32, y as f32, w as f32, h as f32);
            gl::Uniform2f(self.target, panel.0 as f32, panel.1 as f32);
            gl::Uniform1f(self.bright, crate::power::screen_brightness(t));
            let [r, g, b] = self.grade.gain;
            gl::Uniform3f(self.u_gain, r, g, b);
            gl::Uniform1f(self.u_saturation, self.grade.saturation);
            gl::Uniform1f(self.u_contrast, self.grade.contrast);
            gl::Uniform1f(
                self.half,
                match screen {
                    DsScreen::Top => 0.0,
                    DsScreen::Bottom => 1.0,
                },
            );
        }
        self.quad.draw();
        if t >= 1.0 {
            self.draw_overlay(screen, panel);
        }
    }
}

impl DsPass {
    pub fn draw_fps(&mut self, fps: f32, panel: (u32, u32)) {
        let rgba = fps_pixels(&fps_text(fps));
        unsafe {
            gl::Viewport(0, 0, panel.0 as i32, panel.1 as i32);
            gl::Disable(gl::BLEND);
            gl::UseProgram(self.fps_prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.fps_tex);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                0,
                0,
                FPS_W as i32,
                FPS_H as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.as_ptr() as *const std::ffi::c_void,
            );
            gl::Uniform4f(
                crate::gl::uniform_location(self.fps_prog, "u_rect"),
                FPS_AT as f32,
                FPS_AT as f32,
                (FPS_W as i32 * FPS_SCALE) as f32,
                (FPS_H as i32 * FPS_SCALE) as f32,
            );
            gl::Uniform2f(
                crate::gl::uniform_location(self.fps_prog, "u_target"),
                panel.0 as f32,
                panel.1 as f32,
            );
        }
        self.quad.draw();
    }
}

impl Drop for DsPass {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.fps_prog);
            gl::DeleteTextures(1, &self.fps_tex);
            gl::DeleteProgram(self.prog);
            gl::DeleteTextures(1, &self.tex);
        }
    }
}
