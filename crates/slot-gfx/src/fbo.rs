use crate::draw::{Draw, Sprites, TexId};
use crate::grade::{blue_light_gain, contrast_boost, saturation_boost};
use crate::pipeline::GamePass;
use crate::quad::Quad;
use crate::shaders::{BLIT_FRAG, BLIT_VERT, TURN_VERT};
use crate::surface::{blit_rect, GfxError, Surface, OUT_H, OUT_W};
use crate::Canvas;

pub const BACKDROP: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

struct Turn {
    panel: (u32, u32),
    prog: gl::types::GLuint,
    fbo: gl::types::GLuint,
    tex: gl::types::GLuint,
    size: (u32, u32),
}

pub struct Compositor {
    fbo: gl::types::GLuint,
    tex: gl::types::GLuint,
    size: (u32, u32),
    blit: gl::types::GLuint,
    u_gain: gl::types::GLint,
    u_saturation: gl::types::GLint,
    u_contrast: gl::types::GLint,
    u_blit_uv: gl::types::GLint,
    gain: [f32; 3],
    saturation: f32,
    contrast: f32,
    shake: f32,
    window: (u32, u32),
    screen: f32,
    quad: Quad,
    game: GamePass,
    sprites: Sprites,
    game_bezel: Option<TexId>,
    sp_frame: bool,
    game_panel: bool,
    turn: Option<Turn>,
    captured: Option<(u32, u32, Vec<u8>)>,
}

impl Compositor {
    pub fn new(surface: &dyn Surface) -> Result<Self, GfxError> {
        crate::gl::load(surface);
        let blit = crate::shaders::program(BLIT_VERT, BLIT_FRAG)?;
        let tex = crate::gl::texture(OUT_W, OUT_H, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None);
        unsafe {
            let mut fbo = 0;
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            if status != gl::FRAMEBUFFER_COMPLETE {
                gl::DeleteFramebuffers(1, &fbo);
                gl::DeleteTextures(1, &tex);
                gl::DeleteProgram(blit);
                return Err(GfxError::Framebuffer(status));
            }
            gl::UseProgram(blit);
            gl::Uniform1i(crate::gl::uniform_location(blit, "u_tex"), 0);
            let u_gain = crate::gl::uniform_location(blit, "u_gain");
            let u_saturation = crate::gl::uniform_location(blit, "u_saturation");
            let u_contrast = crate::gl::uniform_location(blit, "u_contrast");
            let u_blit_uv = crate::gl::uniform_location(blit, "u_uv");
            gl::Uniform4f(u_blit_uv, 0.0, 0.0, 1.0, 1.0);

            Ok(Compositor {
                fbo,
                tex,
                size: (OUT_W, OUT_H),
                blit,
                u_gain,
                u_saturation,
                u_contrast,
                u_blit_uv,
                gain: blue_light_gain(0),
                saturation: saturation_boost(0),
                contrast: contrast_boost(0),
                shake: 0.0,
                window: (OUT_W, OUT_H),
                screen: 0.0,
                quad: Quad::new(),
                game: GamePass::new()?,
                sprites: Sprites::new()?,
                game_bezel: None,
                sp_frame: true,
                game_panel: false,
                turn: None,
                captured: None,
            })
        }
    }

    pub fn set_panel_turn(&mut self, panel: (u32, u32), ccw: bool) -> Result<(), GfxError> {
        let prog = crate::shaders::program(TURN_VERT, BLIT_FRAG)?;
        unsafe {
            gl::UseProgram(prog);
            gl::Uniform1i(crate::gl::uniform_location(prog, "u_tex"), 0);
            gl::Uniform3f(crate::gl::uniform_location(prog, "u_gain"), 1.0, 1.0, 1.0);
            gl::Uniform1f(
                crate::gl::uniform_location(prog, "u_ccw"),
                if ccw { 1.0 } else { 0.0 },
            );
        }
        self.turn = Some(Turn {
            panel,
            prog,
            fbo: 0,
            tex: 0,
            size: (0, 0),
        });
        Ok(())
    }

    pub fn upload_game(&mut self, xrgb8888: &[u8]) {
        self.game.upload(xrgb8888, &self.quad);
    }

    pub fn draw_game(&mut self) {
        self.in_game_rect(|c| c.game.draw(&c.quad));
    }

    pub fn draw_list(&mut self, items: &[Draw]) {
        let mut from = 0;
        for (i, item) in items.iter().enumerate() {
            match *item {
                Draw::Game => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    self.in_game_rect(|c| {
                        c.game.draw(&c.quad);
                        if c.panel_bezel().is_none() {
                            c.draw_game_bezel();
                        }
                    });
                    if let Some(bezel) = self.panel_bezel() {
                        self.sprites.draw(&[bezel], &self.quad);
                    }
                }
                Draw::Shot { tex } => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    if let Some(tex) = self.sprites.source(tex) {
                        self.in_game_rect(|c| c.game.draw_still(tex, &c.quad));
                    }
                }
                Draw::Capture => {
                    self.sprites.draw(&items[from..i], &self.quad);
                    let (w, h) = self.size;
                    self.captured = Some((w, h, self.read_frame()));
                }
                _ => continue,
            }
            from = i + 1;
        }
        self.sprites.draw(&items[from..], &self.quad);
    }

    pub fn create_texture(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture(w, h, rgba)
    }

    pub fn create_texture_nearest(&mut self, w: u32, h: u32, rgba: &[u8]) -> TexId {
        self.sprites.create_texture_nearest(w, h, rgba)
    }

    pub fn update_texture(&mut self, id: TexId, w: u32, h: u32, rgba: &[u8]) {
        self.sprites.update_texture(id, w, h, rgba);
    }

    pub fn set_blue_light(&mut self, step: u8) {
        self.gain = blue_light_gain(step);
    }

    pub fn set_saturation(&mut self, step: u8) {
        self.saturation = saturation_boost(step);
    }

    pub fn set_contrast(&mut self, step: u8) {
        self.contrast = contrast_boost(step);
    }

    pub fn set_screen_effect(&mut self, effect: crate::ScreenEffect) {
        self.game.set_effect(effect);
    }

    pub fn set_authentic_screen(&mut self, screen: crate::Screen) {
        self.game.set_screen(screen);
    }

    /// Whether Authentic's shader lays the glass's sheen and shade over the screen; off when
    /// the bezel carries the glass.
    pub fn set_authentic_glass(&mut self, on: bool) {
        self.game.set_glass(on);
    }

    pub fn set_picture(&mut self, rect: [f32; 4]) {
        self.game.set_picture(rect);
    }

    pub fn set_game_bezel(&mut self, tex: Option<TexId>) {
        self.game_bezel = tex;
    }

    pub fn set_sp_frame(&mut self, on: bool) {
        self.sp_frame = on;
    }

    pub fn set_game_layout(&mut self, layout: crate::GameLayout) {
        self.game_panel = layout.panel;
        self.game.set_layout(layout.rect);
    }

    fn draw_game_bezel(&mut self) {
        let Some(tex) = self.game_bezel else {
            return;
        };
        let (x, y, w, h) = self.game.frame();
        self.sprites.set_target(Canvas::FRAMED);
        self.sprites.draw(
            &[Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha: 1.0,
            }],
            &self.quad,
        );
        self.sprites.set_target(Canvas::of(self.window));
    }

    pub fn set_paper(&mut self, size: u32, gray: &[u8]) {
        self.game.set_paper(size, gray);
    }

    pub fn fit(&mut self, window: (u32, u32)) {
        self.window = window;
        self.resize(canvas_size(window));
    }

    fn native(&self) -> bool {
        crate::bezel::framed(self.window)
    }

    fn resize(&mut self, size: (u32, u32)) {
        if size == self.size {
            return;
        }
        let tex = crate::gl::texture(
            size.0,
            size.1,
            gl::NEAREST,
            gl::CLAMP_TO_EDGE,
            gl::RGBA,
            None,
        );
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteTextures(1, &self.tex);
        }
        self.tex = tex;
        self.size = size;
    }

    pub fn set_screen_power(&mut self, t: f32) {
        self.screen = t;
        self.game.set_power(t);
    }

    pub fn set_screen_footer(&mut self, px: f32) {
        self.game.set_bottom(px);
    }

    pub fn set_game_fills_frame(&mut self, on: bool) {
        self.game.set_fills_frame(on);
    }

    pub fn set_game_source_rect(&mut self, rect: [f32; 4]) {
        self.game.set_source_rect(rect);
    }

    pub fn set_shake(&mut self, dx: f32) {
        self.shake = dx;
    }

    pub fn take_capture(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.captured.take()
    }

    pub fn frame_size(&self) -> (u32, u32) {
        self.size
    }

    pub fn read_frame(&self) -> Vec<u8> {
        let (w, h) = self.size;
        let stride = w as usize * 4;
        let mut buf = vec![0u8; stride * h as usize];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                w as i32,
                h as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
        }
        let mut top_down = Vec::with_capacity(buf.len());
        for row in buf.chunks_exact(stride).rev() {
            top_down.extend_from_slice(row);
        }
        top_down
    }

    pub fn read_window(&mut self, window: (u32, u32)) -> Vec<u8> {
        let (w, h) = window;
        let tex = crate::gl::texture(w, h, gl::NEAREST, gl::CLAMP_TO_EDGE, gl::RGBA, None);
        let mut fbo = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut fbo);
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::COLOR_ATTACHMENT0,
                gl::TEXTURE_2D,
                tex,
                0,
            );
        }
        self.output(window, fbo);
        let stride = w as usize * 4;
        let mut buf = vec![0u8; stride * h as usize];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                w as i32,
                h as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                buf.as_mut_ptr() as *mut std::ffi::c_void,
            );
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::DeleteFramebuffers(1, &fbo);
            gl::DeleteTextures(1, &tex);
        }
        buf.chunks_exact(stride).rev().flatten().copied().collect()
    }

    pub fn begin_frame(&mut self) {
        let size = if self.native() {
            self.window
        } else {
            canvas_size(self.window)
        };
        self.resize(size);
        self.sprites.set_target(Canvas::of(self.window));
        self.game.set_fbo_scale(
            self.size.0 as f32 / OUT_W as f32,
            self.size.1 as f32 / OUT_H as f32,
        );
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::Viewport(0, 0, self.size.0 as i32, self.size.1 as i32);
            gl::ClearColor(BACKDROP[0], BACKDROP[1], BACKDROP[2], BACKDROP[3]);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
    }

    pub fn end_frame(&mut self, window: (u32, u32)) {
        let Some(mut turn) = self.turn.take() else {
            self.output(window, 0);
            return;
        };
        if turn.size != window {
            unsafe {
                if turn.fbo != 0 {
                    gl::DeleteFramebuffers(1, &turn.fbo);
                    gl::DeleteTextures(1, &turn.tex);
                }
                turn.tex = crate::gl::texture(
                    window.0,
                    window.1,
                    gl::NEAREST,
                    gl::CLAMP_TO_EDGE,
                    gl::RGBA,
                    None,
                );
                gl::GenFramebuffers(1, &mut turn.fbo);
                gl::BindFramebuffer(gl::FRAMEBUFFER, turn.fbo);
                gl::FramebufferTexture2D(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0,
                    gl::TEXTURE_2D,
                    turn.tex,
                    0,
                );
            }
            turn.size = window;
        }
        self.output(window, turn.fbo);
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, 0);
            gl::Viewport(0, 0, turn.panel.0 as i32, turn.panel.1 as i32);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(turn.prog);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, turn.tex);
        }
        self.quad.draw();
        self.turn = Some(turn);
    }

    fn output(&mut self, window: (u32, u32), target: gl::types::GLuint) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, target);
            gl::Viewport(0, 0, window.0 as i32, window.1 as i32);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            gl::UseProgram(self.blit);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::Uniform3f(self.u_gain, self.gain[0], self.gain[1], self.gain[2]);
            gl::Uniform1f(self.u_saturation, self.saturation);
            gl::Uniform1f(self.u_contrast, self.contrast);
            gl::BindTexture(gl::TEXTURE_2D, self.tex);
        }
        if self.native() {
            let canvas = Canvas::of(window);
            let dx = (self.shake * window.0 as f32 / canvas.w).round() as i32;
            self.blit_part(
                dx,
                0,
                window.0 as i32,
                window.1 as i32,
                [0.0, 0.0, 1.0, 1.0],
            );
            return;
        }
        let (x, y, w, h) = blit_rect(window, self.shake);
        self.blit_part(x, y, w, h, [0.0, 0.0, 1.0, 1.0]);
    }

    fn panel_bezel(&self) -> Option<Draw> {
        let tex = self
            .game_bezel
            .filter(|_| crate::bezel::framed(self.window))?;
        let canvas = Canvas::of(self.window);
        Some(Draw::Tex {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            tex,
            alpha: 1.0,
        })
    }
}

impl Compositor {
    fn in_game_rect(&mut self, draw: impl FnOnce(&mut Self)) {
        if !self.native() {
            return draw(self);
        }
        let (x, top, w, h) = match self.game_panel {
            true => (0, 0, self.window.0 as i32, self.window.1 as i32),
            false => crate::bezel::game_rect(self.window, self.sp_frame),
        };
        self.game
            .set_fbo_scale(w as f32 / OUT_W as f32, h as f32 / OUT_H as f32);
        let y = self.size.1 as i32 - top - h;
        self.game.set_origin(x as f32, y as f32);
        unsafe { gl::Viewport(x, y, w, h) };
        draw(self);
        self.game.set_origin(0.0, 0.0);
        self.game.set_fbo_scale(
            self.size.0 as f32 / OUT_W as f32,
            self.size.1 as f32 / OUT_H as f32,
        );
        unsafe { gl::Viewport(0, 0, self.size.0 as i32, self.size.1 as i32) };
    }

    fn blit_part(&self, x: i32, y: i32, w: i32, h: i32, uv: [f32; 4]) {
        unsafe {
            gl::Viewport(x, y, w, h);
            gl::Uniform4f(self.u_blit_uv, uv[0], uv[1], uv[2], uv[3]);
        }
        self.quad.draw();
    }
}

pub fn canvas_size(window: (u32, u32)) -> (u32, u32) {
    let (_, _, w, h) = blit_rect(window, 0.0);
    let (w, h) = (w.max(1) as u32, h.max(1) as u32);
    if w % OUT_W == 0 && h % OUT_H == 0 && w / OUT_W == h / OUT_H {
        (OUT_W, OUT_H)
    } else {
        (w, h)
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.tex);
            gl::DeleteProgram(self.blit);
            if let Some(turn) = &self.turn {
                gl::DeleteProgram(turn.prog);
                if turn.fbo != 0 {
                    gl::DeleteFramebuffers(1, &turn.fbo);
                    gl::DeleteTextures(1, &turn.tex);
                }
            }
        }
    }
}
