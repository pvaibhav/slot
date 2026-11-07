#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Canvas, Compositor, Draw, HeadlessSurface};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_ui::{quick_row_top, QuickRow};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn write_png(name: &str, w: u32, h: u32, rgba: &[u8]) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let path = format!("{dir}/{name}.png");
    let file = std::fs::File::create(&path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

#[test]
fn the_shelf_on_a_4_by_3_panel_fills_it_from_the_top_row() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    let mut input = Script(VecDeque::new());
    for _ in 0..10 {
        f.advance(&mut input);
    }
    c.fit((640, 480));
    f.compose(&mut c);
    assert_eq!(
        c.frame_size(),
        (640, 480),
        "the shelf was not drawn at the panel size"
    );
    let frame = c.read_frame();
    write_png("native-shelf", 640, 480, &frame);
    let lit = frame[..640 * 4]
        .chunks(4)
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
        .count();
    assert!(lit > 600, "the top row is black: {lit} of 640 pixels lit");
}

#[test]
fn a_host_window_resized_to_4_by_3_lays_the_shelf_out_for_it() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (720, 480));
    f.render(&mut c, (720, 480));
    assert_eq!(f.app().canvas(), Canvas::FRAMED);
    f.render(&mut c, (640, 480));
    assert_eq!(
        f.app().canvas(),
        Canvas { w: 640.0, h: 480.0 },
        "the shelf kept its 720 layout in a 4:3 window"
    );
    assert_eq!(c.frame_size(), (640, 480));
    f.render(&mut c, (1080, 720));
    assert_eq!(
        f.app().canvas(),
        Canvas {
            w: 1080.0,
            h: 720.0
        },
        "a 3:2 window kept the 4:3 layout"
    );
}

#[test]
fn a_title_shows_under_the_cart_when_titles_are_on() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Pokemon - Emerald Version (USA, Europe)", "Zzz"]);
    clocked(d.path());
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            show_titles: true,
            ..state
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    let mut input = Script(VecDeque::new());
    for _ in 0..10 {
        f.advance(&mut input);
    }
    assert_eq!(
        f.app().title_text().as_deref(),
        Some("Pokemon Emerald Version")
    );
    c.fit((640, 480));
    f.compose(&mut c);
    let frame = c.read_frame();
    write_png("native-shelf-title", 640, 480, &frame);
    let y = slot_ui::slot_name_y(slot_ui::Canvas::FRAMED) as usize + 8;
    let lit = (100..540)
        .filter(|&x| {
            let o = (y * 640 + x) * 4;
            frame[o] > 200 && frame[o + 1] > 200 && frame[o + 2] > 200
        })
        .count();
    assert!(lit > 20, "no title ink under the cart: {lit} white pixels");
}

#[test]
fn a_panel_sized_wallpaper_lands_pixel_for_pixel_on_a_4_by_3_panel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    clocked(d.path());
    let (w, h) = (640usize, 480usize);
    let mut paper = vec![0u8; w * h * 4];
    for (i, px) in paper.chunks_mut(4).enumerate() {
        let colour = match i % w {
            0..=1 => [255, 0, 0, 255],
            638..=639 => [0, 0, 255, 255],
            _ => [128, 128, 128, 255],
        };
        px.copy_from_slice(&colour);
    }
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    let file = std::fs::File::create(dir.join("edges.png")).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&paper).unwrap();
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            wallpaper: slot_store::WallpaperPick::File("edges.png".into()),
            ..state
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    let mut input = Script(VecDeque::new());
    for _ in 0..10 {
        f.advance(&mut input);
    }
    c.fit((640, 480));
    f.compose(&mut c);
    f.compose(&mut c);
    let frame = c.read_frame();
    write_png("native-wallpaper-edges", 640, 480, &frame);
    let at = |x: usize, y: usize| {
        let o = (y * w + x) * 4;
        [frame[o], frame[o + 1], frame[o + 2]]
    };
    for y in [20, 380] {
        assert_eq!(at(0, y), [255, 0, 0], "left edge at y {y}");
        assert_eq!(at(1, y), [255, 0, 0], "second column at y {y}");
        assert_eq!(at(2, y), [128, 128, 128], "third column at y {y}");
        assert_eq!(at(639, y), [0, 0, 255], "right edge at y {y}");
        assert_eq!(
            at(637, y),
            [128, 128, 128],
            "inside the right edge at y {y}"
        );
    }
}

type Paint = fn(usize, usize, usize, usize) -> [u8; 4];

fn edges(x: usize, y: usize, w: usize, h: usize) -> [u8; 4] {
    match (x, y) {
        (0..=1, _) => [255, 0, 0, 255],
        (x, _) if x >= w - 2 => [0, 0, 255, 255],
        (_, 0..=1) => [0, 255, 0, 255],
        (_, y) if y >= h - 2 => [255, 255, 0, 255],
        _ => [0, 0, 0, 0],
    }
}

fn bezel_on_a_4_by_3_panel(
    platform: slot_store::Platform,
    name: &str,
    pick: slot_store::BezelPick,
    file: Option<(usize, usize, Paint)>,
) -> Option<Vec<u8>> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let d = match platform {
        slot_store::Platform::Gba => common::tmp_root_with_carts(&["Emerald"]),
        _ => common::tmp_root_with_gb_carts(&["Tetris"]),
    };
    clocked(d.path());
    if let Some((w, h, paint)) = file {
        let mut bezel = vec![0u8; w * h * 4];
        for (i, px) in bezel.chunks_mut(4).enumerate() {
            px.copy_from_slice(&paint(i % w, i / w, w, h));
        }
        let dir = d.path().join("Bezels").join(platform.dir_name());
        let file = std::fs::File::create(dir.join("edges.png")).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&bezel).unwrap();
    }
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &match platform {
            slot_store::Platform::Gba => slot_store::SlotState {
                bezel_gba: pick,
                ..state
            },
            _ => slot_store::SlotState {
                bezel_gb: pick,
                ..state
            },
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    c.fit((640, 480));
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(slot_input::Btn::A)]);
    input.0.push_back(vec![RawEvent::Up(slot_input::Btn::A)]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while std::time::Instant::now() < until {
        f.advance(&mut input);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    f.compose(&mut c);
    f.compose(&mut c);
    let panel = c.read_window((640, 480));
    write_png(name, 640, 480, &panel);
    Some(panel)
}

fn panel_at(panel: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * 640 + x) * 4;
    [panel[o], panel[o + 1], panel[o + 2]]
}

#[test]
fn a_panel_sized_bezel_lands_pixel_for_pixel_on_a_4_by_3_panel() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gb,
        "native-bezel-panel",
        slot_store::BezelPick::File("edges.png".into()),
        Some((640, 480, edges)),
    ) else {
        return;
    };
    let at = |x, y| panel_at(&panel, x, y);
    for y in [100, 300] {
        assert_eq!(at(0, y), [255, 0, 0], "left edge at y {y}");
        assert_eq!(at(1, y), [255, 0, 0], "second column at y {y}");
        assert_ne!(at(2, y), [255, 0, 0], "third column at y {y}");
        assert_eq!(at(639, y), [0, 0, 255], "right edge at y {y}");
        assert_ne!(at(637, y), [0, 0, 255], "inside the right edge at y {y}");
    }
    assert_eq!(at(320, 0), [0, 255, 0], "top edge");
    assert_eq!(at(320, 1), [0, 255, 0], "second row");
    assert_ne!(at(320, 2), [0, 255, 0], "third row");
    assert_eq!(at(320, 479), [255, 255, 0], "bottom edge");
    assert_eq!(at(320, 478), [255, 255, 0], "second row from the bottom");
    assert_ne!(at(320, 477), [255, 255, 0], "third row from the bottom");
}

#[test]
fn a_bezel_the_size_of_the_game_area_sits_on_it_on_a_4_by_3_panel() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gb,
        "native-bezel-edges",
        slot_store::BezelPick::File("edges.png".into()),
        Some((640, 427, edges)),
    ) else {
        return;
    };
    let at = |x, y| panel_at(&panel, x, y);
    assert_eq!(at(0, 200), [255, 0, 0], "left edge");
    assert_eq!(at(639, 200), [0, 0, 255], "right edge");
    assert_eq!(at(320, 26), [0, 0, 0], "above the game area");
    assert_eq!(at(320, 27), [0, 255, 0], "top edge");
    assert_eq!(at(320, 28), [0, 255, 0], "second row");
    assert_eq!(at(320, 453), [255, 255, 0], "bottom edge");
    assert_eq!(at(320, 454), [0, 0, 0], "below the game area");
}

#[test]
fn the_default_bezel_reaches_the_top_and_bottom_of_a_4_by_3_panel() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gb,
        "native-bezel-default",
        slot_store::BezelPick::Default,
        None,
    ) else {
        return;
    };
    for y in [0, 13, 26, 453, 466, 479] {
        for x in [5, 320, 634] {
            let px = panel_at(&panel, x, y);
            assert!(
                px.iter().all(|c| (0x20..0x80).contains(c)),
                "the bezel's grain is missing at {x}, {y}: {px:?}"
            );
        }
    }
}

#[test]
fn every_shortcut_row_keeps_its_label_clear_of_its_binding_on_a_4_by_3_panel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    c.fit((640, 480));
    let mut input = Script(VecDeque::new());
    let mut tap = |f: &mut Frontend, b: Btn| {
        input.0.push_back(vec![RawEvent::Down(b)]);
        input.0.push_back(vec![RawEvent::Up(b)]);
        f.advance(&mut input);
        f.advance(&mut input);
    };
    tap(&mut f, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut f, Btn::Down);
    }
    tap(&mut f, Btn::A);
    tap(&mut f, Btn::Down);
    tap(&mut f, Btn::A);
    for row in QuickRow::SHORTCUTS {
        assert_eq!(f.app().quick_menu(), Some(row));
        f.compose(&mut c);
        let px = c.read_frame();
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &[]) as usize;
        let ink: Vec<usize> = (40..640)
            .filter(|&x| {
                (top + 8..top + 44).any(|y| {
                    let o = (y * 640 + x) * 4;
                    px[o..o + 3].iter().all(|c| *c > 0x80)
                })
            })
            .collect();
        let gap = ink.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
        assert!(
            gap >= 24,
            "{row:?} runs into its binding, widest gap {gap} px"
        );
        tap(&mut f, Btn::Down);
    }
}

fn run_for(f: &mut Frontend, input: &mut Script, ms: u64) {
    let until = std::time::Instant::now() + std::time::Duration::from_millis(ms);
    while std::time::Instant::now() < until {
        f.advance(input);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn lit_rows(panel: &[u8], rows: std::ops::Range<usize>) -> usize {
    rows.filter(|y| (0..640).any(|x| panel_at(panel, x, *y).iter().all(|c| *c > 0xc0)))
        .count()
}

fn link_screen_on(panel: (u32, u32)) -> Option<Vec<u8>> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let d = common::tmp_root_with_gb_carts(&["Tetris"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, panel);
    c.fit(panel);
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(Btn::A)]);
    input.0.push_back(vec![RawEvent::Up(Btn::A)]);
    run_for(&mut f, &mut input, 4_000);
    input.0.push_back(vec![RawEvent::Down(Btn::Select)]);
    input.0.push_back(vec![RawEvent::Down(Btn::Menu)]);
    input.0.push_back(vec![RawEvent::Up(Btn::Menu)]);
    input.0.push_back(vec![RawEvent::Up(Btn::Select)]);
    run_for(&mut f, &mut input, 1_500);
    assert!(f.app().game_menu_open(), "the link screen did not open");
    let mut out = Vec::new();
    f.app().draw(&mut out);
    let mut legends: Vec<(f32, f32)> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex { x, y, w, h, .. } if y > 400.0 && h == slot_ui::MOUTH_H => {
                Some((x, x + w - slot_ui::HINT_EDGE as f32))
            }
            _ => None,
        })
        .collect();
    legends.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(!legends.is_empty(), "no link legends");
    for pair in legends.windows(2) {
        assert!(
            pair[0].1 <= pair[1].0,
            "link legends overlap at {panel:?}: {legends:?}"
        );
    }
    assert!(legends.last().unwrap().1 <= panel.0 as f32 * 480.0 / panel.1 as f32 + 1.0);
    f.compose(&mut c);
    let px = c.read_window(panel);
    write_png(&format!("native-link-{}", panel.0), panel.0, panel.1, &px);
    Some(px)
}

#[test]
fn the_link_screen_uses_the_whole_4_by_3_panel() {
    let Some(link) = link_screen_on((640, 480)) else {
        return;
    };
    assert!(
        lit_rows(&link, 448..480) > 0,
        "the link legends are not at the bottom of the panel"
    );
    link_screen_on((720, 480));
}

#[test]
fn a_gba_bezel_from_the_card_frames_the_game_on_a_4_by_3_panel() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gba,
        "native-bezel-gba",
        slot_store::BezelPick::File("edges.png".into()),
        Some((640, 480, edges)),
    ) else {
        return;
    };
    let at = |x, y| panel_at(&panel, x, y);
    assert_eq!(at(0, 200), [255, 0, 0], "left edge");
    assert_eq!(at(639, 200), [0, 0, 255], "right edge");
    assert_eq!(at(320, 0), [0, 255, 0], "top edge");
    assert_eq!(at(320, 479), [255, 255, 0], "bottom edge");
    assert_eq!(at(320, 10), [0, 0, 0], "above the game area");
    assert_eq!(at(320, 470), [0, 0, 0], "below the game area");
}

fn hole_on_top(_: usize, y: usize, _: usize, _: usize) -> [u8; 4] {
    match y {
        0..=426 => [0, 0, 0, 0],
        _ => [255, 0, 255, 255],
    }
}

#[test]
fn a_gba_bezel_with_its_hole_at_the_top_puts_the_game_at_the_top() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gba,
        "native-bezel-gba-top",
        slot_store::BezelPick::File("edges.png".into()),
        Some((640, 480, hole_on_top)),
    ) else {
        return;
    };
    let at = |x, y| panel_at(&panel, x, y);
    for y in [0, 13, 26, 426] {
        assert_ne!(at(320, y), [0, 0, 0], "no game at row {y}");
        assert_ne!(at(320, y), [255, 0, 255], "bezel at row {y}");
    }
    assert_eq!(at(320, 427), [255, 0, 255], "the bezel under the game");
    assert_eq!(at(320, 479), [255, 0, 255], "the bezel's bottom row");
}

fn gb_chin(_: usize, y: usize, _: usize, _: usize) -> [u8; 4] {
    match y {
        0..=404 => [0, 0, 0, 0],
        _ => [255, 0, 255, 255],
    }
}

#[test]
fn a_game_boy_bezel_with_its_hole_at_the_top_lifts_the_game() {
    let Some(panel) = bezel_on_a_4_by_3_panel(
        slot_store::Platform::Gb,
        "native-bezel-gb-top",
        slot_store::BezelPick::File("edges.png".into()),
        Some((640, 480, gb_chin)),
    ) else {
        return;
    };
    let at = |x, y| panel_at(&panel, x, y);
    for y in [21, 30, 404] {
        assert_ne!(at(320, y), [0, 0, 0], "no game at row {y}");
        assert_ne!(at(320, y), [255, 0, 255], "bezel at row {y}");
    }
    assert_eq!(at(320, 405), [255, 0, 255], "the bezel under the game");
}

fn game_boy_page_on(panel: (u32, u32)) -> Option<(Vec<u8>, Vec<QuickRow>)> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, panel);
    c.fit(panel);
    let mut input = Script(VecDeque::new());
    let mut tap = |f: &mut Frontend, b: Btn| {
        input.0.push_back(vec![RawEvent::Down(b)]);
        input.0.push_back(vec![RawEvent::Up(b)]);
        f.advance(&mut input);
        f.advance(&mut input);
    };
    f.compose(&mut c);
    tap(&mut f, Btn::Menu);
    for _ in 0..QuickRow::Screen.position(&f.app().hidden_rows()) {
        tap(&mut f, Btn::Down);
    }
    tap(&mut f, Btn::A);
    for _ in 0..QuickRow::ScreenGb.position(&f.app().hidden_rows()) {
        tap(&mut f, Btn::Down);
    }
    tap(&mut f, Btn::A);
    let hidden = f.app().hidden_rows();
    let last = *QuickRow::ScreenGb.child()?.shown(&hidden).last()?;
    while f.app().quick_menu() != Some(last) {
        tap(&mut f, Btn::Down);
    }
    f.compose(&mut c);
    f.compose(&mut c);
    let px = c.read_window(panel);
    write_png(
        &format!("gb_page_{}x{}", panel.0, panel.1),
        panel.0,
        panel.1,
        &px,
    );
    Some((px, hidden))
}

#[test]
fn the_game_boy_page_lists_scale_and_stretch_on_both_panels() {
    for (panel, rows) in [
        ((720, 480), &[QuickRow::GbScale, QuickRow::GbStretch][..]),
        ((640, 480), &[QuickRow::GbScale, QuickRow::GbStretch][..]),
    ] {
        let Some((px, hidden)) = game_boy_page_on(panel) else {
            return;
        };
        for &row in rows {
            assert!(!hidden.contains(&row), "{row:?} hidden on {panel:?}");
            let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &hidden) as usize;
            let w = panel.0 as usize;
            let lit = (top + 8..top + 40)
                .any(|y| (0..w).any(|x| px[(y * w + x) * 4..][..3].iter().all(|c| *c > 0x80)));
            assert!(lit, "{row:?} drew nothing on {panel:?}");
        }
    }
}

fn gb_game_on(name: &str, panel: (u32, u32), scale: slot_store::Scale) -> Option<Vec<u8>> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let d = common::tmp_root_with_gb_carts(&["Tetris"]);
    clocked(d.path());
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            bezel_gb: slot_store::BezelPick::Default,
            scale_gb: scale,
            ..state
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, panel);
    c.fit(panel);
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(Btn::A)]);
    input.0.push_back(vec![RawEvent::Up(Btn::A)]);
    run_for(&mut f, &mut input, 4000);
    f.render(&mut c, panel);
    f.render(&mut c, panel);
    let px = c.read_window(panel);
    write_png(name, panel.0, panel.1, &px);
    Some(px)
}

fn rgb(px: &[u8], w: u32, x: usize, y: usize) -> [u8; 3] {
    let o = (y * w as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn plastic(p: [u8; 3]) -> bool {
    p.iter().all(|c| (0x14..0x80).contains(c))
}

#[test]
fn the_fit_bezel_frames_the_fit_picture_on_both_panels() {
    for (panel, left, right) in [((720, 480), 92, 626), ((640, 480), 52, 586)] {
        let name = format!("fit_bezel_{}x{}", panel.0, panel.1);
        let Some(px) = gb_game_on(&name, panel, slot_store::Scale::Fit) else {
            return;
        };
        for y in [10, 240, 470] {
            assert!(
                plastic(rgb(&px, panel.0, left, y)),
                "{panel:?} left of the picture at y {y}: {:?}",
                rgb(&px, panel.0, left, y)
            );
            assert!(
                plastic(rgb(&px, panel.0, right, y)),
                "{panel:?} right of the picture at y {y}: {:?}",
                rgb(&px, panel.0, right, y)
            );
        }
        let red = (0..left).any(|x| {
            (140..200).any(|y| {
                let [r, g, _] = rgb(&px, panel.0, x, y);
                r > 0xc0 && g < 0x60
            })
        });
        assert!(red, "{panel:?} lost its light");
    }
}

#[test]
fn the_integer_bezel_frames_the_integer_picture_on_4_by_3() {
    let Some(px) = gb_game_on(
        "integer_bezel_640x480",
        (640, 480),
        slot_store::Scale::Integer,
    ) else {
        return;
    };
    for x in [72, 76] {
        assert!(
            rgb(&px, 640, x, 240).iter().all(|c| *c < 0x20),
            "no black frame at x {x}: {:?}",
            rgb(&px, 640, x, 240)
        );
    }
    for (x, y) in [(30, 400), (320, 6), (320, 470), (620, 240)] {
        assert!(
            plastic(rgb(&px, 640, x, y)),
            "no plastic at {x}, {y}: {:?}",
            rgb(&px, 640, x, y)
        );
    }
}

#[test]
fn the_change_legend_clears_the_cart_slot_on_a_4_by_3_panel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    c.fit((640, 480));
    let mut input = Script(VecDeque::new());
    let mut tap = |f: &mut Frontend, b: Btn| {
        input.0.push_back(vec![RawEvent::Down(b)]);
        input.0.push_back(vec![RawEvent::Up(b)]);
        f.advance(&mut input);
        f.advance(&mut input);
    };
    tap(&mut f, Btn::Menu);
    for _ in 0..QuickRow::General.position(&[]) {
        tap(&mut f, Btn::Down);
    }
    tap(&mut f, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Theme));
    f.compose(&mut c);
    let px = c.read_frame();
    write_png("native-general", 640, 480, &px);
    let bay_right = 320 + (slot_ui::MOUTH_W as usize + 18) / 2;
    let word = (320..640)
        .find(|&x| {
            (436..466).any(|y| {
                let o = (y * 640 + x) * 4;
                px[o..o + 3].iter().all(|c| *c > 0xc0)
            })
        })
        .expect("no legend ink right of centre");
    assert!(
        word >= bay_right + 12,
        "Change starts at {word}, the slot ends at {bay_right}"
    );
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

fn row_gap(frame: &[u8], row: QuickRow, hidden: &[QuickRow]) -> (usize, usize) {
    let top = slot_ui::quick_row_top_at(slot_ui::Canvas::FRAMED, row, row, hidden) as usize;
    let at = |x: usize, y: usize| {
        let o = (y * 640 + x) * 4;
        frame[o..o + 3].iter().all(|c| *c > 0x80)
    };
    let ink: Vec<usize> = (40..640)
        .filter(|&x| (top + 8..top + 44).any(|y| at(x, y)))
        .collect();
    let gap = ink.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    (gap, *ink.last().unwrap_or(&0))
}

fn down_to(f: &mut Frontend, input: &mut Script, row: QuickRow) {
    for _ in 0..120 {
        if f.app().quick_menu() == Some(row) {
            return;
        }
        tap(f, input, Btn::Down);
    }
    panic!("never reached {row:?}, stuck on {:?}", f.app().quick_menu());
}

fn check(f: &mut Frontend, c: &mut Compositor, input: &mut Script, row: QuickRow) {
    let hidden = f.app().hidden_rows();
    down_to(f, input, row);
    f.render(c, (640, 480));
    let frame = c.read_frame();
    write_png(&format!("native-{row:?}"), 640, 480, &frame);
    let (gap, last) = row_gap(&frame, row, &hidden);
    assert!(gap >= 22, "{row:?}: label and value {gap} px apart");
    assert!(last >= 600, "{row:?}: the value ends at x {last}");
}

#[test]
fn long_menu_values_keep_clear_of_their_labels_on_a_4_by_3_panel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut s = slot_store::read_slot_state(d.path());
    s.blending_gba = slot_store::Blending::Ghosting;
    slot_store::write_slot_state(d.path(), &s).unwrap();
    std::fs::write(
        d.path().join("Config/shortcuts.txt"),
        "fast_forward_toggle=start+right\ncolor_correction=start+right\n",
    )
    .unwrap();
    let open = |c: &mut Compositor| {
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(c);
        f.upload_bezel(c, (640, 480));
        f.render(c, (640, 480));
        f
    };
    let mut input = Script(VecDeque::new());
    let mut f = open(&mut c);
    tap(&mut f, &mut input, Btn::Menu);
    down_to(&mut f, &mut input, QuickRow::Screen);
    tap(&mut f, &mut input, Btn::A);
    tap(&mut f, &mut input, Btn::Down);
    tap(&mut f, &mut input, Btn::A);
    check(&mut f, &mut c, &mut input, QuickRow::GbaBlending);
    let mut f = open(&mut c);
    tap(&mut f, &mut input, Btn::Menu);
    down_to(&mut f, &mut input, QuickRow::Controls);
    tap(&mut f, &mut input, Btn::A);
    tap(&mut f, &mut input, Btn::Down);
    tap(&mut f, &mut input, Btn::A);
    check(&mut f, &mut c, &mut input, QuickRow::ScColour);
    check(&mut f, &mut c, &mut input, QuickRow::ScFastForwardToggle);
}

fn gb_with_bezel_files(
    name: &str,
    scale: slot_store::Scale,
    picked: &str,
    files: &[(&str, [u8; 4])],
) -> Option<Vec<u8>> {
    let surface = HeadlessSurface::new().ok()?;
    let mut c = Compositor::new(&surface).ok()?;
    let d = common::tmp_root_with_gb_carts(&["Tetris"]);
    clocked(d.path());
    let dir = d.path().join("Bezels").join("GB");
    for (file, colour) in files {
        let mut rgba = vec![0u8; 640 * 480 * 4];
        for (i, px) in rgba.chunks_mut(4).enumerate() {
            if i % 640 < 20 {
                px.copy_from_slice(colour);
            }
        }
        let out = std::fs::File::create(dir.join(file)).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(out), 640, 480);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&rgba).unwrap();
    }
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            bezel_gb: slot_store::BezelPick::File(picked.into()),
            scale_gb: scale,
            ..state
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    c.fit((640, 480));
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(Btn::A)]);
    input.0.push_back(vec![RawEvent::Up(Btn::A)]);
    run_for(&mut f, &mut input, 3000);
    f.render(&mut c, (640, 480));
    f.render(&mut c, (640, 480));
    let px = c.read_window((640, 480));
    write_png(name, 640, 480, &px);
    Some(px)
}

const WOOD: [u8; 4] = [255, 0, 0, 255];
const SHELL_FIT: [u8; 4] = [0, 0, 255, 255];
const SHELL_INTEGER: [u8; 4] = [0, 255, 0, 255];
const BEZELS: [(&str, [u8; 4]); 3] = [
    ("Wood.png", WOOD),
    ("Shell_fit.png", SHELL_FIT),
    ("Shell_integer.png", SHELL_INTEGER),
];

#[test]
fn a_plain_bezel_draws_in_every_scale() {
    for scale in [
        slot_store::Scale::SpFrame,
        slot_store::Scale::Integer,
        slot_store::Scale::Fit,
    ] {
        let Some(px) = gb_with_bezel_files(
            &format!("native-bezel-plain-{scale:?}"),
            scale,
            "Wood.png",
            &BEZELS,
        ) else {
            return;
        };
        assert_eq!(panel_at(&px, 5, 240), [255, 0, 0], "{scale:?}");
    }
}

#[test]
fn a_suffixed_bezel_draws_its_file_for_the_scale() {
    let Some(px) = gb_with_bezel_files(
        "native-bezel-suffix-integer",
        slot_store::Scale::Integer,
        "Shell_fit.png",
        &BEZELS,
    ) else {
        return;
    };
    assert_eq!(panel_at(&px, 5, 240), [0, 255, 0]);
}

#[test]
fn a_suffixed_bezel_with_no_file_for_the_scale_draws_no_bezel() {
    let Some(px) = gb_with_bezel_files(
        "native-bezel-suffix-missing",
        slot_store::Scale::SpFrame,
        "Shell_fit.png",
        &BEZELS,
    ) else {
        return;
    };
    let edge = panel_at(&px, 5, 240);
    for colour in [WOOD, SHELL_FIT, SHELL_INTEGER] {
        assert_ne!(edge, [colour[0], colour[1], colour[2]]);
    }
}

fn card_png(path: &std::path::Path, w: usize, h: usize, paint: impl Fn(usize, usize) -> [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut px = vec![0u8; w * h * 4];
    for (i, p) in px.chunks_mut(4).enumerate() {
        p.copy_from_slice(&paint(i % w, i / w));
    }
    let file = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(&px).unwrap();
}

#[test]
fn a_card_overlay_draws_over_the_game_and_under_the_bezel() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = common::tmp_root_with_carts(&["Emerald"]);
    clocked(d.path());
    card_png(&d.path().join("Bezels/GBA/edges.png"), 640, 480, |x, y| {
        hole_on_top(x, y, 640, 480)
    });
    card_png(&d.path().join("Overlays/GBA/grid.png"), 640, 480, |_, _| [255, 0, 0, 255]);
    card_png(&d.path().join("Overlays/GBA/grid_tile.png"), 2, 1, |x, _| match x {
        1 => [0, 255, 0, 255],
        _ => [0, 0, 0, 0],
    });
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            bezel_gba: slot_store::BezelPick::File("edges.png".into()),
            overlay_gba: Some("grid".into()),
            ..state
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, (640, 480));
    c.fit((640, 480));
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(slot_input::Btn::A)]);
    input.0.push_back(vec![RawEvent::Up(slot_input::Btn::A)]);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(4);
    while std::time::Instant::now() < until {
        f.advance(&mut input);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let mut panel = Vec::new();
    for _ in 0..100 {
        f.compose(&mut c);
        panel = c.read_window((640, 480));
        if panel_at(&panel, 101, 200) == [0, 255, 0] {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    write_png("native-overlay-gba", 640, 480, &panel);
    let at = |x, y| panel_at(&panel, x, y);
    for x in [101, 103, 321, 639] {
        assert_eq!(at(x, 200), [0, 255, 0], "tile column at x {x}");
    }
    for x in [100, 102, 320, 638] {
        assert_eq!(at(x, 200), [255, 0, 0], "full image at x {x}");
    }
    assert_eq!(at(320, 479), [255, 0, 255], "the bezel covers the overlay");
}
