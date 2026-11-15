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
    assert!(
        r < 20 && g < 20 && b < 20,
        "mat drawn before it slid in: {r} {g} {b}"
    );
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
    assert!(
        lit(&img, 200..520, 430..470) > 40,
        "legends should spread across the bar"
    );
    press(&mut f, Btn::Y);
    assert!(matches!(
        f.app().studio().unwrap().screen,
        Screen::Hex { .. }
    ));
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
        e.write_header()
            .unwrap()
            .write_image_data(&rgb.repeat((w * h) as usize))
            .unwrap();
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
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
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
        assert_eq!(
            f.app().studio().unwrap().wearing,
            LabelKind::Real,
            "the peel waited on the card"
        );
        for _ in 0..100 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let studio = f.app().studio().unwrap();
        assert_eq!(studio.wearing, LabelKind::Existing);
        assert!(matches!(studio.screen, Screen::Sheet { .. }));
    }

    fn grey(frame: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
        let mut n = 0;
        for y in ys {
            for x in xs.clone() {
                let [r, g, b] = px(frame, x, y).map(|c| c as i32);
                if (r - g).abs() < 10 && (g - b).abs() < 10 && g > r - 4 && !(40..=120).contains(&g)
                {
                    n += 1;
                }
            }
        }
        n
    }

    #[test]
    fn the_marker_rests_on_the_mat_then_writes_and_applies() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
        let kirby = d.path().join("Labels/GB/Kirby.png");
        label(&kirby, [40, 60, 200]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let cart = f.app().studio().unwrap().cart.clone();
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem: cart.stem.clone(),
                duplicates: true,
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            if f.app().studio().unwrap().sheet == vec![LabelKind::Hand] {
                break;
            }
        }
        frame(&mut f, &mut c, "marker-sliding");
        settle(&mut f);
        let img = snap(&mut f, &mut c, "marker-rest");
        let rest = slot::studio_view::marker_rest(OUT_W as f32, &cart);
        let (mx, my) = (rest.at[0] as usize, rest.at[1] as usize);
        let (rx, ry) = if rest.deg == 0.0 { (14, 60) } else { (60, 14) };
        assert!(
            grey(&img, mx - rx..mx + rx, my - ry..my + ry) > 600,
            "no marker resting on the mat"
        );
        press(&mut f, Btn::A);
        assert!(f.app().studio().unwrap().writing());
        let start = f.app().now();
        f.app_mut().tick_ms(start + 1300);
        assert!(f.app().studio().unwrap().writing());
        f.app_mut()
            .tick_ms(start + (slot::studio::WRITE_MS + slot::studio::PEEL_MS) as u64 + 100);
        let studio = f.app().studio().unwrap();
        assert!(!studio.writing());
        assert_eq!(studio.wearing, LabelKind::Hand);
        for _ in 0..100 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!kirby.exists(), "the old label stayed on the card");
        let now = f.app().now();
        f.app_mut().tick_ms(now + 2000);
        let gone = snap(&mut f, &mut c, "marker-gone");
        assert!(
            grey(&gone, mx - rx..mx + rx, my - ry..my + ry) < 100,
            "the marker stayed after writing"
        );
    }

    #[test]
    fn the_marker_writes_while_the_printer_is_still_printing() {
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
        let kirby = d.path().join("Labels/GB/Kirby.png");
        label(&kirby, [40, 60, 200]);
        let real = d.path().join("real.png");
        label(&real, [200, 60, 40]);
        let logo = d.path().join("logo.png");
        label(&logo, [60, 200, 40]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        press(&mut f, Btn::A);
        let picked = |f: &Frontend| {
            let s = f.app().studio().unwrap();
            match s.screen {
                Screen::Sheet { at } => s.sheet.get(at).copied(),
                _ => None,
            }
        };
        press(&mut f, Btn::Left);
        assert_eq!(picked(&f), Some(LabelKind::Hand), "marker out of reach");
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem,
                real: Some(real),
                logo: Some(logo),
                ..StudioArt::default()
            }));
        for _ in 0..10 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
        }
        assert_eq!(
            picked(&f),
            Some(LabelKind::Hand),
            "the scans took the marker"
        );
        press(&mut f, Btn::A);
        assert!(f.app().studio().unwrap().writing(), "the marker waited");
    }

    fn settle(f: &mut Frontend) {
        for _ in 0..2 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 1000);
        }
    }

    fn snap(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
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

    fn blue(frame: &[u8]) -> usize {
        (0..OUT_H as usize)
            .flat_map(|y| (0..OUT_W as usize / 2).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let [r, g, b] = px(frame, x, y).map(|c| c as i32);
                b > 150 && r < 90 && g < 110
            })
            .count()
    }

    fn moved(a: &[u8], b: &[u8]) -> usize {
        (0..OUT_H as usize)
            .flat_map(|y| (0..OUT_W as usize / 2).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                let (p, q) = (px(a, x, y), px(b, x, y));
                (0..3).any(|i| (p[i] as i32 - q[i] as i32).abs() > 24)
            })
            .count()
    }

    #[test]
    fn writing_on_plastic_peels_the_label_then_writes_on_the_bare_shell() {
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
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.app_mut().apply(Action::SelectTap);
        press(&mut f, Btn::A);
        for _ in 0..100 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(f.app().studio().unwrap().sheet, vec![LabelKind::Hand]);
        frame(&mut f, &mut c, "plastic-before");
        press(&mut f, Btn::A);
        let start = f.app().now();
        let peel = slot::studio::PEEL_MS as u64;
        f.app_mut().tick_ms(start + peel + 40);
        let peeled = snap(&mut f, &mut c, "plastic-peeled");
        assert!(blue(&peeled) < 50, "the label stayed on");
        assert!(
            f.app().studio().unwrap().writing(),
            "the marker skipped writing on the bare shell"
        );
        let done = start + peel + slot::studio::WRITE_MS as u64 + 100;
        f.app_mut().tick_ms(done);
        assert!(!f.app().studio().unwrap().writing());
        let written = snap(&mut f, &mut c, "plastic-written");
        assert!(
            moved(&peeled, &written) > 200,
            "no ink went on the bare shell: {}",
            moved(&peeled, &written)
        );
    }

    fn scan_over_hand(c: &mut Compositor, name: &str) -> (TempDir, Frontend, Vec<u8>) {
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        let real = d.path().join("real.png");
        label(&real, [200, 60, 40]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let cart = f.app().studio().unwrap().cart.clone();
        assert!(cart.label.is_none());
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem: cart.stem.clone(),
                real: Some(real),
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if !matches!(f.app().studio().unwrap().screen, Screen::Sheet { .. }) {
            press(&mut f, Btn::A);
        }
        for _ in 0..4 {
            press(&mut f, Btn::Up);
        }
        let before = frame(&mut f, c, name);
        let studio = f.app().studio().unwrap();
        assert_eq!(studio.screen, Screen::Sheet { at: 0 });
        assert_eq!(studio.sheet.first(), Some(&LabelKind::Real));
        (d, f, before)
    }

    #[test]
    fn a_label_goes_on_over_the_ink_on_plastic() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let (_d, mut f, before) = scan_over_hand(&mut c, "ink-before");
        press(&mut f, Btn::A);
        let start = f.app().now();
        f.app_mut()
            .tick_ms(start + (slot::studio::PEEL_MS * 0.6) as u64);
        let peeling = snap(&mut f, &mut c, "ink-peeling");
        assert!(
            moved(&before, &peeling) < 200,
            "the ink peeled off like a sticker: {}",
            moved(&before, &peeling)
        );
    }

    #[test]
    fn swapping_one_label_for_another_shows_bare_plastic_between() {
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
        let real = d.path().join("real.png");
        label(&real, [200, 60, 40]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem,
                real: Some(real),
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let studio = f.app().studio().unwrap();
        assert_eq!(studio.screen, Screen::Sheet { at: 0 });
        assert_eq!(studio.sheet.first(), Some(&LabelKind::Real));
        let worn = snap(&mut f, &mut c, "swap-worn");
        press(&mut f, Btn::A);
        let start = f.app().now();
        f.app_mut().tick_ms(start + 505);
        let between = snap(&mut f, &mut c, "swap-between");
        let ink = |img: &[u8]| {
            (0..OUT_H as usize)
                .flat_map(|y| (0..OUT_W as usize / 2).map(move |x| (x, y)))
                .filter(|&(x, y)| px(img, x, y).iter().all(|c| *c < 50))
                .count()
        };
        assert!(
            ink(&between) <= ink(&worn) + 100,
            "handwriting showed under the swapped label: {} vs {}",
            ink(&between),
            ink(&worn)
        );
    }

    #[test]
    fn the_empty_printer_note_waits_for_the_label_to_land() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let real = d.path().join("real.png");
        label(&real, [200, 60, 40]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let stem = f.app().studio().unwrap().cart.stem.clone();
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem,
                real: Some(real),
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        if !matches!(f.app().studio().unwrap().screen, Screen::Sheet { .. }) {
            press(&mut f, Btn::A);
        }
        for _ in 0..4 {
            press(&mut f, Btn::Up);
        }
        snap(&mut f, &mut c, "note-before");
        let cart = f.app().studio().unwrap().cart.clone();
        let t = slot::studio_view::tray(OUT_W as f32, &cart);
        let note = |img: &[u8]| {
            (209..222usize)
                .flat_map(|y| {
                    (t.printer_x as usize..(t.printer_x + t.printer_w) as usize)
                        .map(move |x| (x, y))
                })
                .filter(|&(x, y)| px(img, x, y).iter().all(|c| *c > 180))
                .count()
        };
        press(&mut f, Btn::A);
        let start = f.app().now();
        f.app_mut().tick_ms(start + 100);
        let applying = snap(&mut f, &mut c, "note-applying");
        assert_eq!(f.app().studio().unwrap().sheet, vec![LabelKind::Hand]);
        f.app_mut()
            .tick_ms(start + slot::studio::APPLY_MS as u64 + 100);
        settle(&mut f);
        let landed = snap(&mut f, &mut c, "note-landed");
        assert!(
            note(&applying) < 10,
            "the note showed mid-apply: {}",
            note(&applying)
        );
        assert!(
            note(&landed) > 40,
            "no note after the label landed: {}",
            note(&landed)
        );
    }

    fn dark(frame: &[u8], at: [f32; 2], r: usize) -> usize {
        let (cx, cy) = (at[0] as usize, at[1] as usize);
        (cy - r..cy + r)
            .flat_map(|y| (cx - r..cx + r).map(move |x| (x, y)))
            .filter(|&(x, y)| px(frame, x, y).iter().all(|c| *c < 50))
            .count()
    }

    fn cap_spot(rest: &slot::studio_view::MarkerRest) -> [f32; 2] {
        let k = rest.len / slot_ui::hand::MARKER_H as f32;
        let along = ((slot_ui::hand::MARKER_CAP[0] + slot_ui::hand::MARKER_CAP[1]) / 2.0
            - slot_ui::hand::MARKER_H as f32 / 2.0)
            * k;
        let r = rest.cap_deg.to_radians();
        [rest.cap[0] - r.sin() * along, rest.cap[1] + r.cos() * along]
    }

    fn open_with_marker(f: &mut Frontend, c: &mut Compositor) -> u64 {
        f.app_mut().apply(Action::SelectTap);
        for _ in 0..2000 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(c);
            let ready = f.app().studio_tex().is_some_and(|t| t.hand.is_some());
            if ready && f.app().now() > 400 {
                press(f, Btn::A);
                let now = f.app().now();
                f.app_mut().tick_ms(now + 5);
                return now + 5;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("the marker never loaded");
    }

    fn ink_corners(shell: &str, reshape: bool) -> Option<[[f32; 2]; 4]> {
        let surface = HeadlessSurface::new().ok()?;
        let mut c = Compositor::new(&surface).ok()?;
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
        label(&d.path().join("Labels/GB/Kirby.png"), [40, 60, 200]);
        std::fs::write(
            d.path().join("Labels/cart_shell.txt"),
            format!("Kirby = {shell} 808080 solid\n"),
        )
        .unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.app_mut().apply(Action::SelectTap);
        if reshape {
            to_row(&mut f, Row::Shape);
            press(&mut f, Btn::Right);
            to_row(&mut f, Row::Label);
        }
        press(&mut f, Btn::A);
        for _ in 0..2000 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            if f.app().studio_tex().is_some_and(|t| t.hand.is_some()) && now > 1500 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        press(&mut f, Btn::A);
        assert!(
            f.app().studio().unwrap().writing(),
            "the marker never wrote"
        );
        let start = f.app().now();
        f.app_mut().tick_ms(start + 1500);
        let atlas = f.app().studio_tex().unwrap().hand.as_ref().unwrap().atlas;
        let mut out = Vec::new();
        f.app().draw(&mut out);
        out.iter().find_map(|d| match d {
            slot_gfx::Draw::Quad { corners, tex, .. } if *tex == atlas => Some(*corners),
            _ => None,
        })
    }

    #[test]
    fn the_ink_lands_in_the_well_of_a_shape_changed_in_the_studio() {
        let Some(want) = ink_corners("rounded", false) else {
            return;
        };
        let got = ink_corners("notched", true).expect("no ink drawn");
        for (g, w) in got.iter().zip(want.iter()) {
            assert!(
                (g[0] - w[0]).abs() < 0.5 && (g[1] - w[1]).abs() < 0.5,
                "ink at {got:?}, the rounded well is at {want:?}"
            );
        }
    }

    #[test]
    fn a_label_that_will_not_decode_counts_as_handwriting() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
        std::fs::write(d.path().join("Labels/GB/Kirby.png"), b"not a png").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.app_mut().apply(Action::SelectTap);
        for _ in 0..2000 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            if f.app().studio_tex().is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let studio = f.app().studio().unwrap();
        assert!(
            studio.wearing_hand(),
            "the unreadable label is not handwriting"
        );
        assert!(!studio.sheet.contains(&LabelKind::Hand));
    }

    #[test]
    fn the_marker_slides_away_after_writing() {
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
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        let start = open_with_marker(&mut f, &mut c);
        f.app_mut().tick_ms(start + 2000);
        press(&mut f, Btn::A);
        assert!(f.app().studio().unwrap().writing());
        let wrote = f.app().now();
        let done = wrote + (slot::studio::WRITE_MS + slot::studio::PEEL_MS) as u64;
        f.app_mut().tick_ms(done + 10);
        assert!(!f.app().studio().unwrap().writing());
        for _ in 0..200 {
            f.compose(&mut c);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        f.app_mut().tick_ms(done + 200);
        let body = f
            .app()
            .studio_tex()
            .and_then(|t| t.hand.as_ref())
            .map(|h| h.body)
            .expect("the marker art went with the writing");
        let mut out = Vec::new();
        f.app().draw(&mut out);
        assert!(
            out.iter()
                .any(|d| matches!(d, slot_gfx::Draw::Quad { tex, .. } if *tex == body)),
            "the marker vanished instead of sliding away"
        );
    }

    #[test]
    fn the_marker_slides_out_capped_then_uncaps() {
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
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        let start = open_with_marker(&mut f, &mut c);
        let cart = f.app().studio().unwrap().cart.clone();
        let rest = slot::studio_view::marker_rest(OUT_W as f32, &cart);
        f.app_mut().tick_ms(start + 60);
        let sliding = snap(&mut f, &mut c, "marker-sliding");
        f.app_mut().tick_ms(start + 2000);
        let out = snap(&mut f, &mut c, "marker-out");
        assert!(
            dark(&sliding, cap_spot(&rest), 8) < 20,
            "the cap was off before the marker arrived: {}",
            dark(&sliding, cap_spot(&rest), 8)
        );
        assert!(
            dark(&out, cap_spot(&rest), 8) > 100,
            "the cap never came off: {}",
            dark(&out, cap_spot(&rest), 8)
        );
    }

    #[test]
    fn the_marker_waits_for_the_backing_to_go() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let (_d, mut f, _) = scan_over_hand(&mut c, "backing-before");
        let cart = f.app().studio().unwrap().cart.clone();
        let rest = slot::studio_view::marker_rest(OUT_W as f32, &cart);
        let (mx, my) = (rest.at[0] as usize, rest.at[1] as usize);
        let (rx, ry) = if rest.deg == 0.0 { (14, 60) } else { (60, 14) };
        press(&mut f, Btn::A);
        let start = f.app().now();
        let apply = slot::studio::APPLY_MS as u64;
        for at in [apply / 2, apply - 20] {
            f.app_mut().tick_ms(start + at);
            f.app_mut().tick_ms(start + at + 1);
            let img = snap(&mut f, &mut c, "backing-going");
            assert!(
                grey(&img, mx - rx..mx + rx, my - ry..my + ry) < 100
                    && dark(&img, cap_spot(&rest), 8) < 20,
                "the marker came out before the backing left, at {at} ms"
            );
        }
        f.app_mut().tick_ms(start + apply + 5);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            if f.app().studio_tex().is_some_and(|t| t.hand.is_some()) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        settle(&mut f);
        let img = snap(&mut f, &mut c, "backing-gone");
        assert!(
            grey(&img, mx - rx..mx + rx, my - ry..my + ry) > 600,
            "no marker once the label was on"
        );
    }

    #[test]
    fn a_handwritten_cart_shows_no_marker() {
        let Some(surface) = HeadlessSurface::new().ok() else {
            return;
        };
        let Ok(mut c) = Compositor::new(&surface) else {
            return;
        };
        let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
        clocked(d.path());
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.app_mut().apply(Action::SelectTap);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            if f.app().studio_tex().is_some() && now > 400 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        press(&mut f, Btn::A);
        let start = f.app().now();
        assert!(
            f.app().studio_tex().unwrap().hand.is_none(),
            "the writing was built for a cart that cannot be written on"
        );
        let cart = f.app().studio().unwrap().cart.clone();
        let rest = slot::studio_view::marker_rest(OUT_W as f32, &cart);
        f.app_mut().tick_ms(start + 2000);
        let img = snap(&mut f, &mut c, "marker-none");
        let (mx, my) = (rest.at[0] as usize, rest.at[1] as usize);
        let (rx, ry) = if rest.deg == 0.0 { (14, 60) } else { (60, 14) };
        assert!(
            grey(&img, mx - rx..mx + rx, my - ry..my + ry) < 100,
            "a marker on a handwritten cart"
        );
        assert!(
            dark(&img, cap_spot(&rest), 8) < 20,
            "a cap on a handwritten cart"
        );
    }

    #[test]
    fn the_selected_marker_glows() {
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
        let real = d.path().join("real.png");
        label(&real, [200, 60, 40]);
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        common::connect(f.app_mut());
        let art = Fake::default();
        f.set_art_jobs(Box::new(art.clone()));
        f.app_mut().apply(Action::SelectTap);
        let cart = f.app().studio().unwrap().cart.clone();
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem: cart.stem.clone(),
                real: Some(real),
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        let rest = slot::studio_view::marker_rest(OUT_W as f32, &cart);
        let thick = rest.len * slot_ui::hand::MARKER_W as f32 / slot_ui::hand::MARKER_H as f32;
        let (x, y) = (
            (rest.at[0] + thick / 2.0 + 2.0) as usize,
            rest.at[1] as usize,
        );
        let light = |img: &[u8]| {
            (0..20)
                .map(|i| {
                    px(img, x, y - 10 + i)
                        .iter()
                        .map(|c| *c as u32)
                        .sum::<u32>()
                })
                .sum::<u32>()
        };
        frame(&mut f, &mut c, "marker-arriving");
        settle(&mut f);
        let idle = light(&snap(&mut f, &mut c, "marker-idle"));
        while f
            .app()
            .studio()
            .unwrap()
            .sheet
            .get(match f.app().studio().unwrap().screen {
                Screen::Sheet { at } => at,
                _ => 0,
            })
            != Some(&LabelKind::Hand)
        {
            press(&mut f, Btn::Left);
        }
        let held = light(&frame(&mut f, &mut c, "marker-held"));
        assert!(held > idle + 20 * 30, "no glow: {idle} -> {held}");
    }

    #[test]
    fn a_cart_wearing_its_only_art_prints_nothing_and_offers_the_marker() {
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
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
                stem,
                duplicates: true,
                ..StudioArt::default()
            }));
        press(&mut f, Btn::A);
        for _ in 0..400 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
            if f.app().studio().unwrap().sheet == vec![LabelKind::Hand] {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(f.app().studio().unwrap().sheet, vec![LabelKind::Hand]);
        for _ in 0..80 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
        }
        assert_eq!(f.app().studio_slip_count(), 0);
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
        art.0
            .lock()
            .unwrap()
            .push_back(ArtEvent::StudioArt(StudioArt {
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
        for _ in 0..1000 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 5);
            f.compose(&mut c);
            let done = f.app().studio_tex().is_some_and(|t| t.faces.len() >= 3);
            if done && f.app().studio_wanted_text().is_empty() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(
            f.app().studio_tex().is_some_and(|t| t.faces.len() >= 3)
                && f.app().studio_wanted_text().is_empty(),
            "the studio did not finish preparing the label choices"
        );
        for _ in 0..60 {
            let now = f.app().now();
            f.app_mut().tick_ms(now + 40);
            f.compose(&mut c);
        }
        let sheet = frame(&mut f, &mut c, "sheet-ready");
        assert!(lit(&sheet, 370..700, 0..24) > 1000, "no blank paper");
        let before = px(&sheet, 160, 220);
        press(&mut f, Btn::Down);
        let moved = frame(&mut f, &mut c, "sheet-moved");
        assert_eq!(
            px(&moved, 160, 220),
            before,
            "moving the highlight changed the cart"
        );
        if !bottom {
            press(&mut f, Btn::Up);
        }
        let now = f.app().now();
        f.app_mut().tick_ms(now + 300);
        f.compose(&mut c);
        press(&mut f, Btn::A);
        let want = if bottom {
            LabelKind::Logo
        } else {
            LabelKind::Real
        };
        assert_eq!(f.app().studio().unwrap().wearing, want);
        let now = f.app().now();
        f.app_mut().tick_ms(now + 60);
        assert_eq!(
            f.app().studio_apart(),
            0.0,
            "applying shuffled the stickers"
        );
        let start = now;
        let mut shots = Vec::new();
        for (name, at) in [
            ("peel", 120),
            ("toss", 320),
            ("lift", 500),
            ("tocart", 680),
            ("roll", 900),
        ] {
            f.app_mut().tick_ms(start + at);
            f.compose(&mut c);
            let out = c.read_frame();
            if name == "peel" {
                let [r, _, b] = px(&out, 560, 240);
                let on_top = if bottom { b > r } else { r > b };
                assert!(
                    on_top,
                    "the sticker being peeled fell under its neighbor: {r} {b}"
                );
                let other = px(&out, 560, if bottom { 120 } else { 340 });
                assert!(
                    other.iter().all(|c| *c < 140),
                    "the other sticker lit up: {other:?}"
                );
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
        assert!(
            other[2] > other[0] + 30,
            "the other sticker lost its art: {other:?}"
        );
        let on_slip = px(&shots[3], 522, 120);
        assert!(
            on_slip[1] > 150,
            "the chosen sticker is still on its slip: {on_slip:?}"
        );
        f.app_mut().tick_ms(start + APPLY_MS as u64 + 50);
        f.compose(&mut c);
        let end = c.read_frame();
        let cart = px(&end, 160, 220);
        assert!(
            cart[0] > 150 && cart[2] < 90,
            "the real label is not on the cart: {cart:?}"
        );
        assert!(f.app().studio().is_some());
    }
}
