#![cfg(target_os = "macos")]

use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use slot::thumb;
use slot_gfx::{lcd3x_mask, Compositor, Draw, HeadlessSurface, OUT_W, SRC_H, SRC_W};
use slot_retro::{ButtonMask, MockCore, RetroCore};
use slot_store::{Core, Platform, StateRing};
use slot_ui::{photo_face, Polaroids, Printed};

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

fn switcher_photo(p: &Polaroids) -> Vec<Draw> {
    let mut out = Vec::new();
    p.draw(slot_ui::Gauge::default(), None, Printed::default(), &mut out);
    let photo = *out.first().expect("the switcher drew nothing");
    assert!(
        matches!(photo, Draw::Shot { .. }),
        "the screenshot is not drawn on the panel"
    );
    vec![photo]
}

fn mock_frame(frames: u32) -> Vec<u8> {
    let mut core = MockCore::new();
    core.load(Path::new("mock")).expect("mock load");
    for _ in 0..frames {
        core.run_frame(ButtonMask::default());
    }
    core.video_xrgb8888().to_vec()
}

#[test]
fn a_mock_frame_keeps_its_colours_through_the_game_pass() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let src = mock_frame(7);

    c.begin_frame();
    c.upload_game(&src);
    c.draw_game();
    let frame = c.read_frame();

    let mask = lcd3x_mask();
    let mut worst = 0i32;
    for (sx, sy) in [(0, 0), (5, 1), (113, 37), (120, 80), (239, 159)] {
        let o = (sy * SRC_W as usize + sx) * 4;
        let want_rgb = [src[o + 2], src[o + 1], src[o]];
        for (oy, row) in mask.iter().enumerate() {
            for (ox, cell) in row.iter().enumerate() {
                let got = px(&frame, sx * 3 + ox, sy * 3 + oy);
                for (ch, gain) in cell.iter().enumerate() {
                    let want = (want_rgb[ch] as f32 * gain).round() as i32;
                    worst = worst.max((want - got[ch] as i32).abs());
                }
            }
        }
    }
    assert!(
        worst <= 1,
        "max channel deviation {worst} from the mock frame"
    );
}

#[test]
fn a_saved_frame_arrives_intact_on_its_screenshot() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let src = mock_frame(13);
    let d = tempfile::tempdir().expect("tempdir");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Mock");
    let stamp = "2026-08-09_14-32-05";
    ring.push(b"state", &thumb::png(&src).expect("encode"), stamp)
        .expect("push");
    let entries = ring.list().expect("list");

    let face = photo_face(&entries[0]);
    let tex = c.create_texture(face.w, face.h, &face.rgba);
    c.begin_frame();
    c.draw_list(&[Draw::Tex {
        x: 0.0,
        y: 0.0,
        w: face.w as f32,
        h: face.h as f32,
        tex,
        alpha: 1.0,
    }]);
    let frame = c.read_frame();

    for (sx, sy) in [(0, 0), (113, 37), (239, 159)] {
        let o = (sy * SRC_W as usize + sx) * 4;
        let want = [src[o + 2], src[o + 1], src[o]];
        let got = px(&frame, sx, sy);
        assert_eq!(got, want, "screenshot pixel {sx},{sy}");
    }
}

#[test]
fn the_switcher_magnifies_its_screenshot_without_resampling_it() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let src = mock_frame(21);
    let d = tempfile::tempdir().expect("tempdir");
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Mock");
    ring.push(
        b"state",
        &thumb::png(&src).expect("encode"),
        "2026-08-09_14-32-05",
    )
    .expect("push");
    let entries = ring.list().expect("list");

    let face = photo_face(&entries[0]);
    let tex = c.create_texture_nearest(face.w, face.h, &face.rgba);
    let mut switcher = Polaroids::new(entries);
    switcher.set_faces(vec![tex]);
    c.set_screen_power(1.0);
    c.begin_frame();
    c.draw_list(&switcher_photo(&switcher));
    let frame = c.read_frame();

    let mask = lcd3x_mask();
    for (sx, sy) in [
        (0, 0),
        (1, 0),
        (113, 37),
        (SRC_W as usize - 1, SRC_H as usize - 1),
    ] {
        let o = (sy * SRC_W as usize + sx) * 4;
        let want_rgb = [src[o + 2], src[o + 1], src[o]];
        for (oy, row) in mask.iter().enumerate() {
            for (ox, cell) in row.iter().enumerate() {
                let got = px(&frame, sx * 3 + ox, sy * 3 + oy);
                for (ch, gain) in cell.iter().enumerate() {
                    let want = (want_rgb[ch] as f32 * gain).round() as i32;
                    assert!(
                        (want - got[ch] as i32).abs() <= 1,
                        "source pixel {sx},{sy} bled into its neighbours"
                    );
                }
            }
        }
    }
}
