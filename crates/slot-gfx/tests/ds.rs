#![cfg(target_os = "macos")]

use slot_gfx::{ds_rect, Compositor, DsOverlay, DsPass, DsScreen, HeadlessSurface};
use std::sync::{Mutex, MutexGuard, PoisonError};

static GL: Mutex<()> = Mutex::new(());

const RED: [u8; 3] = [255, 0, 0];
const BLUE: [u8; 3] = [0, 0, 255];
const GREEN: [u8; 3] = [0, 255, 0];

fn frame() -> Vec<u8> {
    let mut out = Vec::with_capacity(256 * 384 * 4);
    for y in 0..384usize {
        for x in 0..256usize {
            let [r, g, b] = match y < 192 {
                true if (x + y) % 2 == 0 => RED,
                true => BLUE,
                false => GREEN,
            };
            out.extend_from_slice(&[b, g, r, 0]);
        }
    }
    out
}

struct Target {
    fbo: u32,
    tex: u32,
    size: (u32, u32),
}

impl Target {
    fn new(size: (u32, u32)) -> Target {
        unsafe {
            let mut tex = 0;
            gl::GenTextures(1, &mut tex);
            gl::BindTexture(gl::TEXTURE_2D, tex);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA8 as i32,
                size.0 as i32,
                size.1 as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                std::ptr::null(),
            );
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
            gl::ClearColor(1.0, 0.0, 1.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            Target { fbo, tex, size }
        }
    }

    fn read(&self) -> Vec<u8> {
        let (w, h) = self.size;
        let mut px = vec![0u8; (w * h * 4) as usize];
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.fbo);
            gl::ReadPixels(
                0,
                0,
                w as i32,
                h as i32,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                px.as_mut_ptr() as *mut _,
            );
        }
        px
    }

    fn at(&self, px: &[u8], x: u32, y: u32) -> [u8; 3] {
        let (w, h) = self.size;
        let row = h - 1 - y;
        let o = ((row * w + x) * 4) as usize;
        [px[o], px[o + 1], px[o + 2]]
    }
}

impl Drop for Target {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteFramebuffers(1, &self.fbo);
            gl::DeleteTextures(1, &self.tex);
        }
    }
}

fn setup() -> (MutexGuard<'static, ()>, HeadlessSurface, Compositor, DsPass) {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().expect("headless gl");
    let compositor = Compositor::new(&surface).expect("compositor");
    let mut ds = DsPass::new().expect("ds pass");
    ds.upload(&frame());
    (guard, surface, compositor, ds)
}

#[test]
fn the_top_screen_fills_1024_by_768_at_4x() {
    let (_g, _s, _c, ds) = setup();
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Top, (1024, 768));
    let px = t.read();
    assert_eq!(t.at(&px, 0, 0), RED);
    assert_eq!(t.at(&px, 3, 3), RED);
    assert_eq!(t.at(&px, 4, 0), BLUE);
    assert_eq!(t.at(&px, 0, 4), BLUE);
    assert_eq!(t.at(&px, 1023, 767), RED);
    assert_eq!(t.at(&px, 1019, 767), BLUE);
    for y in (0..768).step_by(7) {
        for x in (0..1024).step_by(13) {
            assert_ne!(t.at(&px, x, y), GREEN, "bottom screen leaked at {x},{y}");
        }
    }
}

#[test]
fn the_bottom_screen_draws_only_the_bottom_half() {
    let (_g, _s, _c, ds) = setup();
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Bottom, (1024, 768));
    let px = t.read();
    for y in (0..768).step_by(5) {
        for x in (0..1024).step_by(9) {
            assert_eq!(t.at(&px, x, y), GREEN, "at {x},{y}");
        }
    }
}

#[test]
fn a_smaller_panel_centres_the_largest_whole_scale() {
    assert_eq!(ds_rect((1024, 768)), [0, 0, 1024, 768]);
    assert_eq!(ds_rect((640, 480)), [64, 48, 512, 384]);
    assert_eq!(ds_rect((200, 100)), [-28, -46, 256, 192]);
    let (_g, _s, _c, ds) = setup();
    let t = Target::new((640, 480));
    ds.draw_screen(DsScreen::Bottom, (640, 480));
    let px = t.read();
    assert_eq!(t.at(&px, 63, 240), [0, 0, 0]);
    assert_eq!(t.at(&px, 64, 240), GREEN);
    assert_eq!(t.at(&px, 320, 47), [0, 0, 0]);
    assert_eq!(t.at(&px, 320, 48), GREEN);
    assert_eq!(t.at(&px, 575, 431), GREEN);
    assert_eq!(t.at(&px, 576, 431), [0, 0, 0]);
}

#[test]
fn clearing_a_panel_makes_it_black() {
    let (_g, _s, _c, ds) = setup();
    let t = Target::new((1024, 768));
    ds.clear((1024, 768));
    let px = t.read();
    assert_eq!(t.at(&px, 512, 384), [0, 0, 0]);
}

#[test]
fn a_one_panel_surface_has_one_panel_and_ignores_selection() {
    use slot_gfx::Surface;
    let (_g, mut s, _c, _ds) = setup();
    assert_eq!(s.panel_count(), 1);
    assert!(s.select_panel(0).is_ok());
}

#[test]
fn fps_text_has_one_decimal() {
    assert_eq!(slot_gfx::fps_text(59.8261), "59.8");
    assert_eq!(slot_gfx::fps_text(60.04), "60.0");
    assert_eq!(slot_gfx::fps_text(7.0), "7.0");
}

#[test]
fn the_fps_readout_draws_in_the_top_left_corner_only() {
    let (_g, _s, _c, mut ds) = setup();
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Bottom, (1024, 768));
    ds.draw_fps(59.8, (1024, 768));
    let px = t.read();
    let white = (0..40)
        .flat_map(|y| (0..120).map(move |x| (x, y)))
        .filter(|&(x, y)| t.at(&px, x, y) == [255, 255, 255])
        .count();
    assert!(white > 60, "only {white} white pixels in the corner");
    for y in (60..768).step_by(11) {
        for x in (200..1024).step_by(17) {
            assert_eq!(t.at(&px, x, y), GREEN, "the readout drew at {x},{y}");
        }
    }
}

#[test]
fn both_ds_screens_take_the_compositors_blue_light() {
    let (_g, _s, mut c, mut ds) = setup();
    c.set_blue_light(slot_gfx::BLUE_LIGHT_MAX);
    ds.set_grade(c.grade());
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Top, (1024, 768));
    let px = t.read();
    let blue = t.at(&px, 4, 0);
    assert!(
        blue[2] < 250,
        "the top screen's blue stayed {blue:?} under full blue light"
    );
    ds.draw_screen(DsScreen::Bottom, (1024, 768));
    let px = t.read();
    let green = t.at(&px, 512, 384);
    assert!(
        green[1] < 230,
        "the bottom screen is untouched by blue light: {green:?}"
    );
}

fn column_black_tile(ds: &mut DsPass, n: u32, column: u32) -> (u32, u32, u32) {
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        rgba[((y * n + column) * 4 + 3) as usize] = 255;
    }
    (ds.create_overlay_texture(n, n, &rgba), n, n)
}

fn solid(ds: &mut DsPass, w: u32, h: u32, px: [u8; 4]) -> u32 {
    let rgba: Vec<u8> = (0..w * h).flat_map(|_| px).collect();
    ds.create_overlay_texture(w, h, &rgba)
}

#[test]
fn a_ds_overlay_tile_lines_up_with_every_ds_pixel_at_4x() {
    let (_g, _s, _c, mut ds) = setup();
    let tile = column_black_tile(&mut ds, 4, 3);
    ds.set_overlay(Some(DsOverlay {
        tile: Some(tile),
        top: None,
        bottom: None,
    }));
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Top, (1024, 768));
    let px = t.read();
    assert_eq!(t.at(&px, 3, 0), [0, 0, 0]);
    assert_eq!(t.at(&px, 7, 100), [0, 0, 0]);
    assert_eq!(t.at(&px, 2, 0), RED);
    assert_eq!(t.at(&px, 4, 0), BLUE);
}

#[test]
fn a_ds_overlay_tile_starts_at_the_screen_corner_on_a_smaller_panel() {
    let (_g, _s, _c, mut ds) = setup();
    let tile = column_black_tile(&mut ds, 3, 2);
    ds.set_overlay(Some(DsOverlay {
        tile: Some(tile),
        top: None,
        bottom: None,
    }));
    let t = Target::new((640, 480));
    ds.draw_screen(DsScreen::Bottom, (640, 480));
    let px = t.read();
    assert_eq!(t.at(&px, 66, 240), [0, 0, 0]);
    assert_eq!(t.at(&px, 65, 240), GREEN);
    assert_eq!(t.at(&px, 63, 240), [0, 0, 0]);
    assert_eq!(t.at(&px, 62, 240), [0, 0, 0]);
}

#[test]
fn a_top_only_ds_overlay_leaves_the_bottom_bare() {
    let (_g, _s, _c, mut ds) = setup();
    let top = solid(&mut ds, 1024, 768, [255, 0, 255, 255]);
    ds.set_overlay(Some(DsOverlay {
        tile: None,
        top: Some(top),
        bottom: None,
    }));
    let t = Target::new((1024, 768));
    ds.draw_screen(DsScreen::Top, (1024, 768));
    assert_eq!(t.at(&t.read(), 512, 384), [255, 0, 255]);
    ds.draw_screen(DsScreen::Bottom, (1024, 768));
    assert_eq!(t.at(&t.read(), 512, 384), GREEN);
}

#[test]
fn no_ds_overlay_while_the_screen_powers_on() {
    let (_g, _s, _c, mut ds) = setup();
    let top = solid(&mut ds, 1024, 768, [255, 0, 255, 255]);
    ds.set_overlay(Some(DsOverlay {
        tile: None,
        top: Some(top),
        bottom: Some(top),
    }));
    let t = Target::new((1024, 768));
    ds.draw_screen_powered(DsScreen::Bottom, (1024, 768), 0.5);
    let px = t.read();
    for y in (0..768).step_by(11) {
        for x in (0..1024).step_by(13) {
            assert_ne!(
                t.at(&px, x, y),
                [255, 0, 255],
                "overlay drawn mid power-on at {x},{y}"
            );
        }
    }
}
