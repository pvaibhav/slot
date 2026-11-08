mod common;

use common::{clocked, tmp_root_with_gb_carts};
use slot::frontend::Frontend;
use slot::studio::{Row, Screen};
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Action, Btn};
use slot_power::SimPlatform;
use tempfile::TempDir;

fn setup() -> Option<(TempDir, Frontend, Compositor, HeadlessSurface)> {
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
    clocked(d.path());
    let f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    Some((d, f, compositor, surface))
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [frame[o], frame[o + 1], frame[o + 2]]
}

fn press(f: &mut Frontend, btn: Btn) {
    f.app_mut().apply(Action::GbaDown(btn));
    f.app_mut().apply(Action::GbaUp(btn));
}

fn frame(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    let settled = f.app().now() + 300;
    f.app_mut().tick_ms(settled);
    let mut settled = false;
    for _ in 0..1000 {
        let now = f.app().now();
        f.app_mut().tick_ms(now + 5);
        f.compose(c);
        if f.app().studio_tex().is_some() && f.app().studio_wanted_text().is_empty() {
            settled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(settled, "the studio never finished drawing");
    std::thread::sleep(std::time::Duration::from_millis(250));
    f.compose(c);
    f.compose(c);
    let out = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let file = std::fs::File::create(format!("{dir}/studio-{name}.png")).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&out).unwrap();
    }
    out
}

fn lit(frame: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    let mut n = 0;
    for y in ys {
        for x in xs.clone() {
            let p = px(frame, x, y);
            if p[0] as u32 + p[1] as u32 + p[2] as u32 > 540 {
                n += 1;
            }
        }
    }
    n
}

fn to_row(f: &mut Frontend, row: Row) {
    while f.app().studio().and_then(|s| s.row()) != Some(row) {
        press(f, Btn::Down);
    }
}

#[test]
fn list_renders() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().apply(Action::SelectTap);
    let img = frame(&mut f, &mut c, "list");
    assert!(lit(&img, 390..700, 50..380) > 300, "no rows");
    let [r, _, _] = px(&img, 160, 220);
    assert!(r > 80, "no cart: {r}");
}

#[test]
fn the_full_menu_renders_with_wifi() {
    let Some((d, mut f, mut c, _s)) = setup() else {
        return;
    };
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    common::connect(f.app_mut());
    f.set_art_jobs(slot::art::jobs::no_art_jobs());
    f.app_mut().apply(Action::SelectTap);
    let img = frame(&mut f, &mut c, "list-wifi");
    assert_eq!(f.app().studio().unwrap().rows()[0], Row::Label);
    assert!(lit(&img, 390..700, 30..390) > 300);
}

#[test]
fn the_ground_is_a_green_cutting_mat() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().apply(Action::SelectTap);
    let img = frame(&mut f, &mut c, "mat");
    let [r, g, b] = px(&img, 6, 6);
    assert!(g > r + 20 && g > b + 10, "{r} {g} {b}");
}

#[test]
fn the_mat_slides_in_over_black_with_wallpaper_none() {
    let Some(surface) = HeadlessSurface::new().ok() else {
        return;
    };
    let Some(mut c) = Compositor::new(&surface).ok() else {
        return;
    };
    let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
    let mut s = slot_store::read_slot_state(d.path());
    s.clock_set = true;
    s.wallpaper = slot_store::WallpaperPick::None;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.compose(&mut c);
    f.app_mut().apply(Action::SelectTap);
    f.compose(&mut c);
    let img = c.read_frame();
    let [r, g, b] = px(&img, 6, 6);
    assert!(r < 20 && g < 20 && b < 20, "mat drawn before it slid in: {r} {g} {b}");
}

#[test]
fn chips_render() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().apply(Action::SelectTap);
    to_row(&mut f, Row::Color);
    press(&mut f, Btn::A);
    let img = frame(&mut f, &mut c, "chips");
    assert!(lit(&img, 370..700, 10..60) > 100, "no heading");
    assert!(lit(&img, 430..700, 130..180) > 500, "no chips");
}

#[test]
fn custom_and_hex_render_on_the_closed_bar() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().apply(Action::SelectTap);
    to_row(&mut f, Row::Color);
    press(&mut f, Btn::A);
    press(&mut f, Btn::Right);
    press(&mut f, Btn::Right);
    press(&mut f, Btn::A);
    let img = frame(&mut f, &mut c, "custom");
    assert!(lit(&img, 200..520, 430..470) > 40, "legends should spread across the bar");
    press(&mut f, Btn::Y);
    assert!(matches!(f.app().studio().unwrap().screen, Screen::Hex { .. }));
    let img = frame(&mut f, &mut c, "hex");
    assert!(lit(&img, 380..700, 150..300) > 100, "no digits");
}

#[test]
fn a_held_slider_recolors_the_cart_before_release() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().apply(Action::SelectTap);
    to_row(&mut f, Row::Color);
    press(&mut f, Btn::A);
    press(&mut f, Btn::Right);
    press(&mut f, Btn::Right);
    press(&mut f, Btn::A);
    let [before, _, _] = px(&frame(&mut f, &mut c, "slider-before"), 190, 75);
    for _ in 0..12 {
        f.app_mut().apply(Action::GbaDown(Btn::Right));
        let now = f.app().now();
        f.app_mut().tick_ms(now + 5);
        f.compose(&mut c);
        if px(&c.read_frame(), 190, 75)[0] > before {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    panic!("the cart never changed while the button was held");
}

#[test]
fn the_printer_waits_with_blank_paper() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    std::fs::write(_d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    common::connect(f.app_mut());
    f.set_art_jobs(slot::art::jobs::no_art_jobs());
    f.app_mut().apply(Action::SelectTap);
    assert!(f.app().studio().unwrap().row().is_some());
    press(&mut f, Btn::A);
    let img = frame(&mut f, &mut c, "sheet");
    assert!(lit(&img, 370..700, 0..24) > 1000, "no blank paper");
}

mod apply {
    use super::*;
    use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs, StudioArt};
    use slot::studio::{LabelKind, APPLY_MS};
    use std::collections::VecDeque;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<VecDeque<ArtEvent>>>);

    impl ArtJobs for Fake {
        fn ask(&mut self, _job: ArtJob) {}

        fn take(&mut self) -> Option<ArtEvent> {
            self.0.lock().unwrap().pop_front()
        }
    }

    fn label(path: &PathBuf, rgb: [u8; 3]) {
        let (w, h) = (1024u32, 901u32);
        let file = std::fs::File::create(path).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgb.repeat((w * h) as usize)).unwrap();
    }

    #[test]
    fn apply_plays_and_lands_without_a_pop() {
        apply_run(false);
    }

    #[test]
    fn peeling_the_bottom_sticker_stays_above_its_neighbor() {
        apply_run(true);
    }

    #[test]
    fn a_label_that_cannot_be_read_stays_on_the_sheet() {
        let Some((d, mut f, mut c, _s)) = setup() else {
            return;
        };
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        let (real, logo) = (d.path().join("real.png"), d.path().join("logo.png"));
        label(&real, [200, 40, 40]);
        label(&logo, [40, 60, 200]);
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        art.0.lock().unwrap().push_back(ArtEvent::StudioArt(StudioArt {
            stem,
            real: Some(real.clone()),
            logo: Some(logo),
            wheel: None,
            duplicates: false,
            logo_worn: None,
            look: None,
            failed: None,
        }));
        press(&mut f, Btn::A);
        for _ in 0..200 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            if f.app().studio_tex().is_some_and(|t| t.faces.len() >= 3) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for _ in 0..60 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
        }
        std::fs::remove_file(&real).unwrap();
        press(&mut f, Btn::A);
        assert_eq!(f.app().studio().unwrap().wearing, LabelKind::Real, "the peel waited on the card");
        for _ in 0..100 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let studio = f.app().studio().unwrap();
        assert_eq!(studio.wearing, LabelKind::Existing);
        assert!(matches!(studio.screen, Screen::Sheet { .. }));
    }

    #[test]
    fn a_cart_wearing_its_only_art_prints_the_text_sticker() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
        label(&d.path().join("Labels/GB/Kirby.png"), [40, 60, 200]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        assert!(f.app().studio().unwrap().cart.label.is_some());
        art.0.lock().unwrap().push_back(ArtEvent::StudioArt(StudioArt {
            stem,
            duplicates: true,
            ..StudioArt::default()
        }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            if f.app().studio_slip_count() == 1 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(f.app().studio().unwrap().sheet, vec![LabelKind::Title]);
        for _ in 0..80 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
        }
        let img = frame(&mut f, &mut c, "text-sticker");
        assert_eq!(f.app().studio_slip_count(), 1);
        assert!(lit(&img, 370..700, 24..200) > 1000, "no sticker came out of the printer");
    }

    fn apply_run(bottom: bool) {
        let Some((d, mut f, mut c, _s)) = setup() else {
            return;
        };
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        let (real, logo) = (d.path().join("real.png"), d.path().join("logo.png"));
        label(&real, [200, 40, 40]);
        label(&logo, [40, 60, 200]);
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        art.0.lock().unwrap().push_back(ArtEvent::StudioArt(StudioArt {
            stem,
            real: Some(real),
            logo: Some(logo),
            wheel: None,
            duplicates: false,
            logo_worn: None,
            look: None,
            failed: None,
        }));
        press(&mut f, Btn::A);
        for _ in 0..200 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            let done = f.app().studio_tex().is_some_and(|t| t.faces.len() >= 3);
            if done && f.app().studio_wanted_text().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        for _ in 0..60 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
        }
        let sheet = frame(&mut f, &mut c, "sheet-ready");
        assert!(lit(&sheet, 370..700, 0..24) > 1000, "no blank paper");
        let before = px(&sheet, 160, 220);
        press(&mut f, Btn::Right);
        let moved = frame(&mut f, &mut c, "sheet-moved");
        assert_eq!(px(&moved, 160, 220), before, "moving the highlight changed the cart");
        if !bottom {
            press(&mut f, Btn::Left);
        }
        let now = f.app().now();
        f.app_mut().tick_ms(now + 300);
        f.compose(&mut c);
        press(&mut f, Btn::A);
        let want = if bottom { LabelKind::Logo } else { LabelKind::Real };
        assert_eq!(f.app().studio().unwrap().wearing, want);
        let now = f.app().now();
        f.app_mut().tick_ms(now + 60);
        assert_eq!(f.app().studio_apart(), 0.0, "applying shuffled the stickers");
        let start = now;
        let mut shots = Vec::new();
        for (name, at) in [("peel", 120), ("toss", 320), ("lift", 500), ("tocart", 680), ("roll", 900)] {
            f.app_mut().tick_ms(start + at);
            f.compose(&mut c);
            let out = c.read_frame();
            if name == "peel" {
                let [r, _, b] = px(&out, 560, 240);
                let on_top = if bottom { b > r } else { r > b };
                assert!(on_top, "the sticker being peeled fell under its neighbor: {r} {b}");
                let other = px(&out, 560, if bottom { 120 } else { 340 });
                assert!(other.iter().all(|c| *c < 140), "the other sticker lit up: {other:?}");
                if bottom {
                    return;
                }
            }
            if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
                let file = std::fs::File::create(format!("{dir}/apply-{name}.png")).unwrap();
                let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header().unwrap().write_image_data(&out).unwrap();
            }
            shots.push(out);
        }
        let other = px(&shots[2], 560, 340);
        assert!(other[2] > other[0] + 30, "the other sticker lost its art: {other:?}");
        let on_slip = px(&shots[3], 522, 120);
        assert!(on_slip[1] > 150, "the chosen sticker is still on its slip: {on_slip:?}");
        f.app_mut().tick_ms(start + APPLY_MS as u64 + 50);
        f.compose(&mut c);
        let end = c.read_frame();
        let cart = px(&end, 160, 220);
        assert!(cart[0] > 150 && cart[2] < 90, "the real label is not on the cart: {cart:?}");
        assert!(f.app().studio().is_some());
    }
}
