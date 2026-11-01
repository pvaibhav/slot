#![cfg(target_os = "macos")]

use slot::print_scene::{
    print_art, print_draws, stage_art, PaperJob, PrintArtState, PrintArtWorker, PrintFrame, PrintScene,
    PrintTextures,
};
use slot::server::qr_modules;
use slot_gfx::{paper_bounds, Canvas, Compositor, Draw, HeadlessSurface, OUT_H, OUT_W};
use slot_ui::{receipt_face, ReceiptText};

const PRINT_AT: u64 = 1160;
const RIP_AT: u64 = PRINT_AT + 3200 + 350;
const CARRY_AT: u64 = RIP_AT + 960;
const GAP_AT: u64 = CARRY_AT + 340;
const REST_AT: u64 = GAP_AT + 140 + 560;
const URL: &str = "http://192.168.1.42/?pin=4821";

struct Rig {
    c: Compositor,
    tex: PrintTextures,
    canvas: Canvas,
    _surface: HeadlessSurface,
}

fn job() -> PaperJob {
    PaperJob::Code {
        modules: qr_modules(URL).unwrap(),
        text: ReceiptText {
            host: Some("RG35XXPLUS.LOCAL:2003".into()),
            ip: "192.168.100.200:2003".into(),
            pin: "4821".into(),
        },
    }
}

fn rig(window: (u32, u32)) -> Option<Rig> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    c.fit(window);
    let tex = PrintTextures::upload(&mut c, &stage_art().unwrap()).unwrap();
    tex.set_paper(&mut c, &print_art(job()).unwrap());
    Some(Rig {
        c,
        tex,
        canvas: Canvas::of(window),
        _surface: surface,
    })
}

impl Rig {
    fn shot(&mut self, f: &PrintFrame) -> Vec<u8> {
        let mut d = Vec::new();
        print_draws(f, &self.tex, self.canvas, &mut d);
        self.c.begin_frame();
        self.c.draw_list(&d);
        self.c.read_frame()
    }

    fn sheet_only(&mut self, f: &PrintFrame) -> Vec<u8> {
        let mut d = Vec::new();
        print_draws(f, &self.tex, self.canvas, &mut d);
        d.retain(|i| !matches!(i, Draw::Paper { flap: true, .. }));
        self.c.begin_frame();
        self.c.draw_list(&d);
        self.c.read_frame()
    }

    fn at(&mut self, t: u64) -> Vec<u8> {
        let f = PrintScene::new(0).frame(t);
        self.shot(&f)
    }

    fn bare(&mut self, t: u64) -> Vec<u8> {
        let f = PrintFrame {
            paper: None,
            ..PrintScene::new(0).frame(t)
        };
        self.shot(&f)
    }
}

fn at(px: &[u8], w: usize, x: usize, y: usize) -> [u8; 3] {
    let o = (y * w + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn near(a: [u8; 3], b: [u8; 3], by: i32) -> bool {
    (0..3).all(|i| (a[i] as i32 - b[i] as i32).abs() <= by)
}

fn is_paper(c: [u8; 3]) -> bool {
    c.iter().all(|v| *v > 170)
}

const W: usize = OUT_W as usize;

#[test]
fn resting_paper_is_right_of_printer() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let px = r.at(REST_AT);
    let empty = r.bare(REST_AT);
    assert!(is_paper(at(&px, W, 555, 430)), "{:?}", at(&px, W, 555, 430));
    assert!(near(at(&px, W, 300, 240), at(&empty, W, 300, 240), 0));
    let changed: Vec<usize> = (0..W)
        .filter(|&x| {
            (0..OUT_H as usize)
                .any(|y| is_paper(at(&px, W, x, y)) && at(&px, W, x, y) != at(&empty, W, x, y))
        })
        .collect();
    assert!(*changed.first().unwrap() >= 435, "{changed:?}");
    assert!(*changed.last().unwrap() <= 675, "{changed:?}");
}

#[test]
fn blades_cover_paper_during_print() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let px = r.at(RIP_AT - 10);
    let empty = r.bare(RIP_AT - 10);
    for i in 0..39 {
        let x = 105 + 6 * i;
        let c = at(&px, W, x, 285);
        assert!(near(c, [163, 165, 171], 8), "tooth at {x}: {c:?}");
        assert!(c[2] as i32 - c[0] as i32 >= 5, "tooth at {x}: {c:?}");
        assert!(near(c, at(&empty, W, x, 285), 6), "tooth at {x}: {c:?}");
    }
    assert!(is_paper(at(&px, W, 218, 275)), "{:?}", at(&px, W, 218, 275));
}

#[test]
fn fold_shows_blank_back() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let t = RIP_AT + 620;
    let f = PrintScene::new(0).frame(t);
    let p = f.paper.unwrap();
    let px = r.shot(&f);
    let (x0, y0) = (p.pivot.0 - 226.0, p.pivot.1 - 425.0);
    let mut flap = 0;
    for y in 0..OUT_H as usize {
        for x in 0..W {
            let (qx, qy) = (x as f32 - x0, y as f32 - y0);
            let along = 0.62 * qx + 0.78 * qy - p.fold;
            let across = 0.78 * qx - 0.62 * qy;
            if (20.0..45.0).contains(&along) && (-40.0..100.0).contains(&across) {
                let c = at(&px, W, x, y);
                assert!(c[1] > 150, "ink at {x},{y}: {c:?}");
                flap += 1;
            }
        }
    }
    assert!(flap > 500, "{flap}");
}

#[test]
fn no_sliver_after_the_rip() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let t = CARRY_AT + 10;
    let f = PrintScene::new(0).frame(t);
    let px = r.sheet_only(&f);
    let empty = r.bare(t);
    for y in 262..312 {
        for x in 95..345 {
            assert_eq!(at(&px, W, x, y), at(&empty, W, x, y), "at {x},{y}");
        }
    }
}

#[test]
fn fold_line_matches_the_mockup_offset() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let tex = r.tex.paper();
    let d = [
        Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: 720.0,
            h: 480.0,
            colour: [1.0; 4],
        },
        Draw::Paper {
            tex,
            pivot: [400.0, 430.0],
            scale: 1.0,
            lift: 0.0,
            fold: 300.0,
            tear: -1.0,
            bow: false,
            flap: false,
        },
    ];
    r.c.begin_frame();
    r.c.draw_list(&d);
    let px = r.c.read_frame();
    let first = (174..400).find(|&x| at(&px, W, x, 305)[1] < 250).unwrap();
    assert!((first as i32 - 238).abs() <= 1, "{first}");
}

#[test]
fn no_stub_is_left_in_the_slot_when_the_rip_ends() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let f = PrintScene::new(0).frame(CARRY_AT - 1);
    let px = r.sheet_only(&f);
    let empty = r.shot(&PrintFrame { paper: None, ..f });
    for y in 262..312 {
        for x in 95..345 {
            assert!(near(at(&px, W, x, y), at(&empty, W, x, y), 2), "at {x},{y}");
        }
    }
}

#[test]
fn stop_is_printed_on_the_foot_of_the_sheet() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let px = r.at(REST_AT);
    let empty = r.bare(REST_AT);
    let dark = |x: usize, y: usize| {
        let [a, b, c] = at(&px, W, x, y);
        a as u32 + b as u32 + c as u32 <= 300
    };
    let foot = (405..429).flat_map(|y| (442..668).map(move |x| (x, y)));
    assert!(foot.filter(|(x, y)| dark(*x, *y)).count() > 150);
    for y in 445..480 {
        for x in 442..668 {
            assert!(near(at(&px, W, x, y), at(&empty, W, x, y), 8), "at {x},{y}");
        }
    }
}

fn settled(worker: &mut PrintArtWorker) -> PrintArtState {
    for _ in 0..500 {
        match worker.take() {
            PrintArtState::Pending => std::thread::sleep(std::time::Duration::from_millis(10)),
            state => return state,
        }
    }
    PrintArtState::Pending
}

#[test]
fn worker_builds_the_art() {
    let mut worker = PrintArtWorker::spawn(job());
    assert!(matches!(settled(&mut worker), PrintArtState::Ready(_)));
    assert!(matches!(worker.take(), PrintArtState::Pending));
}

#[test]
fn worker_reports_a_failed_build() {
    let mut worker = PrintArtWorker::spawn_with(|| None);
    assert!(matches!(settled(&mut worker), PrintArtState::Failed));
}

#[test]
fn worker_reports_a_build_that_panics() {
    let mut worker = PrintArtWorker::spawn_with(|| panic!("no art"));
    assert!(matches!(settled(&mut worker), PrintArtState::Failed));
}

#[test]
fn fits_640_panel() {
    let Some(mut r) = rig((640, 480)) else {
        return;
    };
    let px = r.at(REST_AT);
    assert_eq!(r.c.frame_size(), (640, 480));
    let empty = r.bare(REST_AT);
    for t in [REST_AT - 200, REST_AT - 60] {
        let f = PrintScene::new(0).frame(t);
        let moving = r.shot(&f);
        let still = r.shot(&PrintFrame { paper: None, ..f });
        for y in 0..480 {
            assert_eq!(
                at(&moving, 640, 639, y),
                at(&still, 640, 639, y),
                "{t} row {y}"
            );
        }
    }
    let Some(mut wide) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let full = wide.at(REST_AT);
    for y in 0..480 {
        for x in 0..640 {
            assert!(
                near(at(&px, 640, x, y), at(&full, W, x + 40, y), 2),
                "at {x},{y}"
            );
        }
    }
    for y in 0..480 {
        assert_eq!(at(&px, 640, 639, y), at(&empty, 640, 639, y), "row {y}");
        assert_eq!(at(&px, 640, 0, y), at(&empty, 640, 0, y), "row {y}");
    }
}

#[test]
fn resting_qr_decodes() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let px = r.at(REST_AT);
    let grey = |x: usize, y: usize| {
        let c = at(&px, W, x, y);
        ((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as u8
    };
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(W, OUT_H as usize, grey);
    let grids = img.detect_grids();
    assert_eq!(grids.len(), 1);
    let (_, content) = grids[0].decode().unwrap();
    assert_eq!(content, URL);
}

#[test]
fn serrated_edge_has_no_dark_fringe() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let face = receipt_face(
        &qr_modules(URL).unwrap(),
        &ReceiptText {
            host: None,
            ip: "1.2.3.4".into(),
            pin: "1".into(),
        },
    );
    let tex = r.c.create_texture(face.w, face.h, &face.rgba);
    let d = [
        Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: 720.0,
            h: 480.0,
            colour: [1.0; 4],
        },
        Draw::Paper {
            tex,
            pivot: [400.0, 460.0],
            scale: 1.04,
            lift: 0.0,
            fold: -60.0,
            tear: -1.0,
            bow: false,
            flap: false,
        },
    ];
    r.c.begin_frame();
    r.c.draw_list(&d);
    let px = r.c.read_frame();
    let mut touched = 0;
    for y in 12..30 {
        for x in 165..400 {
            let c = at(&px, W, x, y);
            assert!(c[1] > 165, "fringe at {x},{y}: {c:?}");
            touched += (c[1] < 250) as u32;
        }
    }
    assert!(touched > 100, "{touched}");
}

#[test]
fn flap_never_reaches_the_edge_of_its_bounds() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let white = Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: 720.0,
        h: 480.0,
        colour: [1.0; 4],
    };
    let tex = r.tex.paper();
    for step in 0..12 {
        let fold = 40.0 + step as f32 * 42.0;
        let (pivot, scale, lift) = ([331.0, 302.0], 1.0, 8.0);
        let d = [
            white,
            Draw::Paper {
                tex,
                pivot,
                scale,
                lift,
                fold,
                tear: 400.0,
                bow: false,
                flap: true,
            },
        ];
        r.c.begin_frame();
        r.c.draw_list(&d);
        let px = r.c.read_frame();
        let [x, y, w, h] = paper_bounds(pivot, scale, lift, fold, true);
        let left = x.max(0.0) as usize;
        let right = ((x + w) as usize).min(W) - 1;
        let top = y.max(0.0) as usize;
        let bottom = ((y + h) as usize).min(OUT_H as usize) - 1;
        for py in top..=bottom {
            if x > 0.0 {
                assert_eq!(at(&px, W, left + 1, py), [255; 3], "fold {fold} left {py}");
            }
            if x + w < W as f32 {
                assert_eq!(
                    at(&px, W, right - 1, py),
                    [255; 3],
                    "fold {fold} right {py}"
                );
            }
        }
        for px_x in left..=right {
            if y > 0.0 {
                assert_eq!(
                    at(&px, W, px_x, top + 1),
                    [255; 3],
                    "fold {fold} top {px_x}"
                );
            }
            if y + h < OUT_H as f32 {
                assert_eq!(
                    at(&px, W, px_x, bottom - 1),
                    [255; 3],
                    "fold {fold} bottom {px_x}"
                );
            }
        }
    }
}

#[test]
#[ignore]
fn contact_sheet() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let times = [
        PRINT_AT + 1800,
        RIP_AT + 500,
        RIP_AT + 800,
        CARRY_AT + 130,
        REST_AT,
    ];
    let mut shots: Vec<Vec<u8>> = times.iter().map(|t| r.at(*t)).collect();
    let mut s = PrintScene::new(0);
    s.stop(REST_AT + 100);
    let f = s.frame(REST_AT + 100 + 220);
    shots.push(r.shot(&f));
    let (cols, rows) = (3, 2);
    let (sw, sh) = (W * cols, OUT_H as usize * rows);
    let mut sheet = vec![0u8; sw * sh * 3];
    for (i, shot) in shots.iter().enumerate() {
        let (ox, oy) = (i % cols * W, i / cols * OUT_H as usize);
        for y in 0..OUT_H as usize {
            for x in 0..W {
                let c = at(shot, W, x, y);
                let o = ((oy + y) * sw + ox + x) * 3;
                sheet[o..o + 3].copy_from_slice(&c);
            }
        }
    }
    let path = std::env::var("RENDER9_OUT").unwrap();
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut enc = png::Encoder::new(file, sw as u32, sh as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&sheet)
        .unwrap();
}

#[test]
fn the_slide_carries_the_whole_scene_up() {
    let Some(mut r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let mut s = PrintScene::new(0);
    s.stop(REST_AT);
    let still = s.frame(REST_AT + 920);
    assert!(still.slide == 0.0 && !still.lamp_on && still.paper.is_none());
    let half = PrintFrame {
        slide: 0.5,
        ..still
    };
    let whole = r.shot(&still);
    let slid = r.shot(&half);
    let row = |px: &[u8], y: usize| px[y * OUT_W as usize * 4..(y + 1) * OUT_W as usize * 4].to_vec();
    assert_eq!(row(&slid, 0), row(&whole, OUT_H as usize / 2));
    assert_ne!(row(&slid, OUT_H as usize - 1), row(&whole, OUT_H as usize - 1));
}

#[test]
fn the_printer_does_not_ride_up_with_the_slide() {
    let Some(r) = rig((OUT_W, OUT_H)) else {
        return;
    };
    let mut s = PrintScene::new(0);
    s.stop(REST_AT);
    let mut d = Vec::new();
    print_draws(&s.frame(REST_AT + 1040), &r.tex, r.canvas, &mut d);
    assert_eq!(d.len(), 1, "{d:?}");
}
