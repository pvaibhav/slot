use slot_gfx::{Compositor, Draw, HeadlessSurface, OUT_H, OUT_W};
use std::sync::{Mutex, MutexGuard, PoisonError};

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [frame[o], frame[o + 1], frame[o + 2]]
}

fn ground() -> Draw {
    Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: OUT_W as f32,
        h: OUT_H as f32,
        colour: [0.0, 0.0, 0.0, 1.0],
    }
}

#[test]
fn quad_fills_its_trapezoid() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let red = c.create_texture(4, 4, &[255, 0, 0, 255].repeat(16));
    c.begin_frame();
    c.draw_list(&[
        ground(),
        Draw::Quad {
            corners: [[100.0, 100.0], [140.0, 110.0], [140.0, 190.0], [100.0, 200.0]],
            tex: red,
            uv: [0.0, 0.0, 1.0, 1.0],
            alpha: 1.0,
        },
    ]);
    let frame = c.read_frame();
    assert_eq!(px(&frame, 110, 150), [255, 0, 0]);
    assert_eq!(px(&frame, 139, 103), [0, 0, 0]);
    assert_eq!(px(&frame, 139, 197), [0, 0, 0]);
    assert_eq!(px(&frame, 95, 150), [0, 0, 0]);
}

#[test]
fn quad_samples_its_uv() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let pair = c.create_texture_nearest(2, 1, &[0, 255, 0, 255, 0, 0, 255, 255]);
    c.begin_frame();
    c.draw_list(&[
        ground(),
        Draw::Quad {
            corners: [[200.0, 200.0], [260.0, 200.0], [260.0, 260.0], [200.0, 260.0]],
            tex: pair,
            uv: [0.5, 0.0, 0.5, 1.0],
            alpha: 1.0,
        },
    ]);
    let frame = c.read_frame();
    assert_eq!(px(&frame, 230, 230), [0, 0, 255]);
}

#[test]
fn quad_corners_run_around_the_edge() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let red = c.create_texture(4, 4, &[255, 0, 0, 255].repeat(16));
    c.begin_frame();
    c.draw_list(&[
        ground(),
        Draw::Quad {
            corners: [[100.0, 100.0], [200.0, 100.0], [200.0, 200.0], [100.0, 200.0]],
            tex: red,
            uv: [0.0, 0.0, 1.0, 1.0],
            alpha: 1.0,
        },
    ]);
    let frame = c.read_frame();
    for (x, y) in [(110, 150), (190, 150), (150, 110), (150, 190)] {
        assert_eq!(px(&frame, x, y), [255, 0, 0], "{x},{y}");
    }
}

#[test]
fn quad_v_runs_down_the_screen() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let pair = c.create_texture_nearest(1, 2, &[0, 255, 0, 255, 0, 0, 255, 255]);
    c.begin_frame();
    c.draw_list(&[
        ground(),
        Draw::Quad {
            corners: [[100.0, 100.0], [200.0, 100.0], [200.0, 200.0], [100.0, 200.0]],
            tex: pair,
            uv: [0.0, 0.0, 1.0, 1.0],
            alpha: 1.0,
        },
    ]);
    let frame = c.read_frame();
    assert_eq!(px(&frame, 150, 120), [0, 255, 0]);
    assert_eq!(px(&frame, 150, 180), [0, 0, 255]);
}

#[test]
fn quad_alpha_blends_and_a_rect_after_it_still_lands_flat() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let white = c.create_texture(4, 4, &[255, 255, 255, 255].repeat(16));
    c.begin_frame();
    c.draw_list(&[
        ground(),
        Draw::Quad {
            corners: [[100.0, 100.0], [200.0, 100.0], [200.0, 200.0], [100.0, 200.0]],
            tex: white,
            uv: [0.0, 0.0, 1.0, 1.0],
            alpha: 0.5,
        },
        Draw::Rect {
            x: 300.0,
            y: 300.0,
            w: 40.0,
            h: 40.0,
            colour: [0.0, 1.0, 0.0, 1.0],
        },
    ]);
    let frame = c.read_frame();
    let [r, _, _] = px(&frame, 150, 150);
    assert!((120..=136).contains(&r), "{r}");
    assert_eq!(px(&frame, 320, 320), [0, 255, 0]);
}
