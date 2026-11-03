#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{app_booting_at, clocked, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::frontend::Frontend;
use slot::romm::{RommEvent, RommJob, RommJobs, GIFTS};
use slot::unwrap_scene::{
    box_face, cart_rect, footer_top, rim_y, unwrap_art, unwrap_draws, UnwrapFrame, UnwrapScene, UnwrapTextures,
};
use slot_gfx::{Canvas, Compositor, Draw, HeadlessSurface, TexId, OUT_H, OUT_W};
use slot::wifi::{Fake, Wifi};
use slot_input::{Action, Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::Platform;
use slot_ui::{box_size, cart_box, default_background, foot_y};

const STEM: &str = "Emerald";
const CART: [u8; 3] = [0xff, 0x00, 0xff];
const GROUND: [u8; 3] = [0x20, 0x40, 0x20];

struct Rig {
    c: Compositor,
    tex: UnwrapTextures,
    ground: TexId,
    wood: TexId,
    cart: TexId,
    face: [TexId; 2],
    _surface: HeadlessSurface,
}

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn rig() -> Option<Rig> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let art = unwrap_art().unwrap();
    let tex = UnwrapTextures::upload(&mut c, &art);
    let ground = c.create_texture(1, 1, &[GROUND[0], GROUND[1], GROUND[2], 255]);
    let wood = c.create_texture(OUT_W, OUT_H, default_background().unwrap());
    let cart = c.create_texture(1, 1, &[CART[0], CART[1], CART[2], 255]);
    let mut face = |p: Platform| {
        let (w, h) = box_size(p);
        c.create_texture(w, h, &box_face(&art, p, STEM))
    };
    let face = [face(Platform::Gba), face(Platform::Gb)];
    Some(Rig {
        c,
        tex,
        ground,
        wood,
        cart,
        face,
        _surface: surface,
    })
}

fn at(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 3] {
    let o = (y * w + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn near(a: [u8; 3], b: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= by)
}

fn purple(c: [u8; 3]) -> bool {
    c[2] as i32 > c[1] as i32 + 15 && c[0] as i32 > c[1] as i32 && c[2] > 0x40
}

fn save(name: &str, w: u32, px: &[u8]) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let opaque: Vec<u8> = px
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2], 255])
        .collect();
    e.write_header().unwrap().write_image_data(&opaque).unwrap();
}

impl Rig {
    fn shot(&mut self, f: &UnwrapFrame, p: Platform, window: u32, under: TexId) -> Vec<u8> {
        let canvas = Canvas::of((window, OUT_H));
        self.c.fit((window, OUT_H));
        let cart = self.cart;
        let mut d = vec![Draw::Tex {
            x: 0.0,
            y: 0.0,
            w: OUT_W as f32,
            h: OUT_H as f32,
            tex: under,
            alpha: 1.0,
        }];
        unwrap_draws(
            f,
            &self.tex,
            (p, STEM),
            |[x, y, w, h], out| {
                out.push(Draw::Tex {
                    x,
                    y,
                    w,
                    h,
                    tex: cart,
                    alpha: 1.0,
                })
            },
            canvas,
            &mut d,
        );
        self.c.begin_frame();
        self.c.draw_list(&d);
        self.c.read_frame()
    }

    fn at_ms(&mut self, ms: u64, p: Platform, window: u32, name: &str) -> Vec<u8> {
        let f = UnwrapScene::new(0).frame(ms);
        let (wood, ground) = (self.wood, self.ground);
        let px = self.shot(&f, p, window, wood);
        save(&format!("{name}-{ms}"), window, &px);
        self.shot(&f, p, window, ground)
    }

    fn face_shot(&mut self, i: usize, p: Platform) -> Vec<u8> {
        let canvas = Canvas::FRAMED;
        self.c.fit((OUT_W, OUT_H));
        let (w, h) = box_size(p);
        let d = [
            Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                tex: self.ground,
                alpha: 1.0,
            },
            Draw::Tex {
                x: canvas.centred(w as f32),
                y: foot_y(cart_box(p).1 as f32) - h as f32,
                w: w as f32,
                h: h as f32,
                tex: self.face[i],
                alpha: 1.0,
            },
        ];
        self.c.begin_frame();
        self.c.draw_list(&d);
        self.c.read_frame()
    }
}

#[test]
fn box_sits_in_the_cart_box() {
    let Some(mut r) = rig() else {
        return;
    };
    let w = OUT_W as usize;
    for (i, p) in [Platform::Gba, Platform::Gb].into_iter().enumerate() {
        let (bw, bh) = box_size(p);
        let (cw, ch) = cart_box(p);
        assert!(bw <= cw, "{p:?}");
        let foot = foot_y(ch as f32) as usize;
        let px = r.face_shot(i, p);
        save(&format!("face-{p:?}"), OUT_W, &px);
        let mid = w / 2;
        assert!(purple(at(&px, w, mid, foot - 4)), "{:?}", at(&px, w, mid, foot - 4));
        assert_eq!(at(&px, w, mid, foot + 2), GROUND);
        let body = (146.0 * bw as f32 / 318.0) as usize;
        assert!(purple(at(&px, w, mid - body + 4, foot - 30)));
        assert_eq!(at(&px, w, mid - cw as usize / 2 - 2, foot - 30), GROUND);
        assert_eq!(at(&px, w, mid, foot - bh as usize - 2), GROUND);
    }
}

#[test]
fn the_scene_opens_on_exactly_the_shelf_face() {
    let Some(mut r) = rig() else {
        return;
    };
    let w = OUT_W as usize;
    for (i, p) in [Platform::Gba, Platform::Gb].into_iter().enumerate() {
        let still = UnwrapFrame {
            shake: 0.0,
            ..UnwrapScene::new(0).frame(0)
        };
        let ground = r.ground;
        let scene = r.shot(&still, p, OUT_W, ground);
        let shelf = r.face_shot(i, p);
        let mut worst = 0;
        let (mut off, mut total) = (0, 0);
        for y in 0..OUT_H as usize {
            for x in 0..w {
                let (a, b) = (at(&scene, w, x, y), at(&shelf, w, x, y));
                if a == GROUND && b == GROUND {
                    continue;
                }
                total += 1;
                let d = (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).max().unwrap();
                worst = worst.max(d);
                off += (d > 24) as usize;
            }
        }
        assert!(off * 100 < total, "{p:?}: {off} of {total} differ, worst {worst}");
    }
}

#[test]
fn cart_stays_behind_the_front_wall() {
    let Some(mut r) = rig() else {
        return;
    };
    for window in [OUT_W, 640] {
        let w = window as usize;
        for p in [Platform::Gba, Platform::Gb] {
            let px = r.at_ms(900, p, window, &format!("wall-{p:?}-{window}"));
            let rim = rim_y(p) as usize;
            let mid = w / 2;
            assert!(purple(at(&px, w, mid, rim + 1)), "{:?}", at(&px, w, mid, rim + 1));
            for y in rim..rim + 140 {
                assert!(!near(at(&px, w, mid, y), CART, 40), "{p:?} cart on the wall at {y}");
            }
            let frame = UnwrapScene::new(0).frame(900);
            let [_, top, _, _] = cart_rect(frame.cart.unwrap(), p, Canvas::of((window, OUT_H)));
            let above = (top + 3.0) as usize;
            if above + 1 < rim {
                assert!(near(at(&px, w, mid, above), CART, 40), "{:?}", at(&px, w, mid, above));
            }
        }
    }
}

#[test]
fn nothing_draws_over_the_footer() {
    let Some(mut r) = rig() else {
        return;
    };
    let w = OUT_W as usize;
    let top = footer_top() as usize;
    for p in [Platform::Gba, Platform::Gb] {
        for ms in (0..=2220).step_by(20) {
            let ground = r.ground;
            let px = r.shot(&UnwrapScene::new(0).frame(ms), p, OUT_W, ground);
            for y in top..OUT_H as usize {
                for x in (0..w).step_by(7) {
                    assert_eq!(at(&px, w, x, y), GROUND, "{p:?} {ms} ms at {x},{y}");
                }
            }
        }
    }
}

#[test]
fn glow_is_gone_before_the_cart_lands() {
    let Some(mut r) = rig() else {
        return;
    };
    let w = OUT_W as usize;
    let lit = r.at_ms(800, Platform::Gba, OUT_W, "glow");
    assert_ne!(at(&lit, w, w / 2 - 120, 150), GROUND);
    let px = r.at_ms(1800, Platform::Gba, OUT_W, "glow");
    let [x, y, cw, ch] = cart_rect(UnwrapScene::new(0).frame(1800).cart.unwrap(), Platform::Gba, Canvas::FRAMED);
    for py in 0..footer_top() as usize {
        for px_x in (0..w).step_by(5) {
            let inside = (px_x as f32) > x - 2.0
                && (px_x as f32) < x + cw + 2.0
                && (py as f32) > y - 2.0
                && (py as f32) < y + ch + 2.0;
            if !inside {
                assert_eq!(at(&px, w, px_x, py), GROUND, "{px_x},{py}");
            }
        }
    }
}

#[test]
fn every_keyframe_renders() {
    let Some(mut r) = rig() else {
        return;
    };
    for ms in [0, 130, 700, 1000, 1380, 1500, 2000, 2220] {
        r.at_ms(ms, Platform::Gba, 640, "gba-640");
        r.at_ms(ms, Platform::Gba, OUT_W, "gba-720");
        r.at_ms(ms, Platform::Gb, 640, "gb-640");
    }
}

#[test]
fn a_gift_unwraps_on_the_real_shelf() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Alpha", STEM, "Fusion"]);
    clocked(d.path());
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(d.path().join(GIFTS), format!("GBA/{STEM}\n")).unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    let step = 1.0 / 60.0;
    let mut now: Millis = 0;
    let mut tick = |f: &mut Frontend, input: &mut Script, events: Vec<RawEvent>| {
        now += 16;
        input.0.push_back(events);
        f.advance_at(input, now, step);
    };
    while !f
        .app()
        .browsed_rom()
        .is_some_and(|r| r.to_string_lossy().contains(STEM))
    {
        tick(&mut f, &mut input, vec![RawEvent::Down(Btn::Right)]);
        tick(&mut f, &mut input, vec![RawEvent::Up(Btn::Right)]);
    }
    let w = OUT_W as usize;
    let foot = foot_y(cart_box(Platform::Gba).1 as f32) as usize;
    let mut built = false;
    for _ in 0..400 {
        tick(&mut f, &mut input, Vec::new());
        f.compose(&mut c);
        if purple(at(&c.read_frame(), w, w / 2, foot - 12)) {
            built = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(built, "no box on the shelf: {:?}", f.app().phase());
    for _ in 0..60 {
        tick(&mut f, &mut input, Vec::new());
    }
    f.compose(&mut c);
    save("real-box", OUT_W, &c.read_frame());
    tick(&mut f, &mut input, vec![RawEvent::Down(Btn::A)]);
    tick(&mut f, &mut input, vec![RawEvent::Up(Btn::A)]);
    assert!(matches!(f.app().phase(), Phase::Unwrap { .. }), "{:?}", f.app().phase());
    let start = f.app().now();
    let mut shots = vec![130u64, 700, 1000, 1380, 1500, 2000];
    while matches!(f.app().phase(), Phase::Unwrap { .. }) {
        let ms = f.app().now() - start;
        assert!(ms < 10_000, "scene never ended");
        tick(&mut f, &mut input, vec![RawEvent::Down(Btn::Right)]);
        tick(&mut f, &mut input, vec![RawEvent::Up(Btn::Right)]);
        if shots.first().is_some_and(|s| ms >= *s) {
            let at_ms = shots.remove(0);
            f.compose(&mut c);
            let px = c.read_frame();
            save(&format!("real-{at_ms}"), OUT_W, &px);
            if at_ms == 2000 {
                assert!(!purple(at(&px, w, w / 2, foot + 20)));
            }
        }
    }
    assert!(shots.is_empty(), "{shots:?}");
    assert!(matches!(f.app().phase(), Phase::Shelf));
    assert!(!f.app().gift_held(Platform::Gba, STEM));
    assert!(f.app().browsed_rom().is_some_and(|r| r.to_string_lossy().contains(STEM)));
    f.compose(&mut c);
    save("real-2220", OUT_W, &c.read_frame());
}

#[derive(Clone, Default)]
struct Events(Arc<Mutex<Vec<RommEvent>>>);

impl RommJobs for Events {
    fn ask(&mut self, _: RommJob) {}

    fn take(&mut self) -> Option<RommEvent> {
        let mut events = self.0.lock().unwrap();
        match events.is_empty() {
            true => None,
            false => Some(events.remove(0)),
        }
    }
}

fn opened(r: &Rig) -> (tempfile::TempDir, App) {
    let d = tmp_root_with_carts(&[STEM, "Fusion"]);
    clocked(d.path());
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(d.path().join(GIFTS), format!("GBA/{STEM}\n")).unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    common::connect(&mut a);
    a.set_unwrap_tex(r.tex);
    (d, a)
}

fn run(a: &mut App, ms: u64) {
    for _ in 0..ms / 16 {
        a.update(0.016);
    }
}

fn press_a(a: &mut App) {
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaUp(Btn::A));
}

#[test]
fn the_radio_stays_held_through_the_scene() {
    let Some(r) = rig() else {
        return;
    };
    let (_d, mut a) = opened(&r);
    a.set_wifi(Wifi::inline(Box::new(Fake::joined())));
    for _ in 0..4 {
        let t = a.now();
        a.tick_ms(t + 1_000);
    }
    assert!(a.wifi().held);
    press_a(&mut a);
    assert!(matches!(a.phase(), Phase::Unwrap { .. }), "{:?}", a.phase());
    let start = a.now();
    while matches!(a.phase(), Phase::Unwrap { .. }) {
        run(&mut a, 16);
        assert!(a.wifi().held, "{} ms", a.now() - start);
    }
    assert!(a.now() - start >= 2_200);
}

#[test]
fn a_rescan_waits_for_the_scene_to_end() {
    let Some(r) = rig() else {
        return;
    };
    let (d, mut a) = opened(&r);
    let events = Events::default();
    a.set_romm_jobs(Box::new(events.clone()));
    let wants = a.gift_wants();
    assert_eq!(wants.len(), 1, "{wants:?}");
    a.set_gift_box(wants[0].0, r.face[0]);
    press_a(&mut a);
    run(&mut a, 800);
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    events.0.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    run(&mut a, 800);
    assert!(matches!(a.phase(), Phase::Unwrap { .. }), "{:?}", a.phase());
    assert!(a.gift_wants().is_empty(), "{:?}", a.gift_wants());
    run(&mut a, 800);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    let stems: Vec<String> = a.gift_wants().into_iter().map(|(_, (_, stem))| stem).collect();
    assert_eq!(stems, ["Sent"]);
}
