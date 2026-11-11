#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts, tmp_root_with_gb_carts, tmp_root_with_gbc_carts};
use slot::app::App;
use slot::frontend::Frontend;
use slot::video_mode::{write_video_mode, VideoMode};
use slot_gfx::{Compositor, HeadlessSurface};
use slot_gfx::{Draw, SRC_H, SRC_W};
use slot_input::Action;
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::{read_slot_state, write_slot_state, BezelPick, Scale, Stretch};

const DT: f32 = 1.0 / 60.0;
const STOPPED: i64 = 1_793_000_000;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

struct Shown {
    w: usize,
    px: Vec<u8>,
}

impl Shown {
    fn at(&self, x: usize, y: usize) -> [u8; 4] {
        let o = (y * self.w + x) * 4;
        [self.px[o], self.px[o + 1], self.px[o + 2], self.px[o + 3]]
    }

    fn row(&self, y: usize, xs: std::ops::Range<usize>) -> Vec<u8> {
        let o = y * self.w * 4;
        self.px[o + xs.start * 4..o + xs.end * 4].to_vec()
    }

    fn dark(&self, x0: usize, y0: usize) -> bool {
        (y0..y0 + 8).all(|y| (x0..x0 + 8).all(|x| self.at(x, y)[..3].iter().all(|c| *c < 8)))
    }
}

fn write_png(name: &str, w: u32, h: u32, rgba: &[u8]) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let file = std::fs::File::create(format!("{dir}/{name}.png")).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
}

fn shelf(c: &mut Compositor, panel: (u32, u32), taps: &[Btn]) -> Shown {
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(
        SimPlatform::at(d.path().to_path_buf()).stopped_at(STOPPED),
    ));
    c.fit(panel);
    f.upload_faces(c);
    f.upload_bezel(c, panel);
    let mut input = Script(VecDeque::new());
    let mut now = 0;
    let mut step = |f: &mut Frontend, input: &mut Script| {
        now += 16;
        f.advance_at(input, now, DT);
    };
    for _ in 0..300 {
        step(&mut f, &mut input);
    }
    for btn in taps {
        input.0.push_back(vec![RawEvent::Down(*btn)]);
        step(&mut f, &mut input);
        input.0.push_back(vec![RawEvent::Up(*btn)]);
        for _ in 0..30 {
            step(&mut f, &mut input);
        }
    }
    for _ in 0..60 {
        step(&mut f, &mut input);
    }
    f.compose(c);
    let px = c.read_window(panel);
    let name = format!("fill-{}x{}-{}", panel.0, panel.1, taps.len());
    write_png(&name, panel.0, panel.1, &px);
    Shown {
        w: panel.0 as usize,
        px,
    }
}

fn compositor() -> Option<(HeadlessSurface, Compositor)> {
    let surface = HeadlessSurface::new().ok()?;
    let c = Compositor::new(&surface).ok()?;
    Some((surface, c))
}

#[test]
fn the_footer_reaches_both_edges_at_1024() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let wide = shelf(&mut c, (1024, 768), &[]);
    for x in [1, 1022] {
        let p = wide.at(x, 760);
        assert!(
            p[..3].iter().all(|c| *c < 0x40),
            "the footer does not reach x {x}: {p:?}"
        );
    }
}

#[test]
fn the_footer_sits_on_the_bottom_at_720_square() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let reference = shelf(&mut c, (720, 480), &[]);
    let square = shelf(&mut c, (720, 720), &[]);
    for (low, high) in [(424, 664), (460, 700), (472, 712)] {
        assert_eq!(
            square.row(high, 0..720),
            reference.row(low, 0..720),
            "footer row {low} is not at {high}"
        );
    }
}

#[test]
fn the_wallpaper_covers_every_corner() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    for panel in [(720, 720), (1024, 768)] {
        let shown = shelf(&mut c, panel, &[]);
        let (w, h) = (panel.0 as usize, panel.1 as usize - 58);
        for (x, y) in [(0, 0), (w - 8, 0), (0, h - 8), (w - 8, h - 8)] {
            assert!(
                !shown.dark(x, y),
                "{panel:?}: the corner at ({x}, {y}) is bare"
            );
        }
    }
}

fn label_crop(shown: &Shown, cx: usize, cy: usize) -> Vec<u8> {
    (cy - 50..cy + 50)
        .flat_map(|y| shown.row(y, cx - 110..cx + 110))
        .collect()
}

#[test]
fn a_cart_face_is_drawn_texel_for_texel() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let reference = shelf(&mut c, (720, 480), &[]);
    let at = label_crop(&reference, 360, 252);
    {
        let panel = (720, 720);
        let shown = shelf(&mut c, panel, &[]);
        let (dx, dy) = ((panel.0 as usize - 720) / 2, (panel.1 as usize - 480) / 2);
        assert!(
            label_crop(&shown, 360 + dx, 252 + dy) == at,
            "{panel:?}: the selected cart's label is not the 720x480 one, moved to the middle"
        );
    }
}

#[test]
fn scenes_reach_the_bottom_on_tall_panels() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    for taps in [&[Btn::Menu][..], &[Btn::Menu, Btn::A][..]] {
        let reference = shelf(&mut c, (720, 480), taps);
        let square = shelf(&mut c, (720, 720), taps);
        for (low, high) in [(440, 680), (452, 692)] {
            assert_eq!(
                square.row(high, 0..720),
                reference.row(low, 0..720),
                "{taps:?}: footer row {low} is not at {high}"
            );
        }
    }
}

fn lit_rows(shown: &Shown, xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> Vec<usize> {
    ys.filter(|y| {
        xs.clone().any(|x| {
            let p = shown.at(x, *y);
            let (lo, hi) = (p[..3].iter().min().unwrap(), p[..3].iter().max().unwrap());
            *lo > 0x70 && hi - lo < 12
        })
    })
    .collect()
}

fn text_lines(rows: &[usize]) -> Vec<(usize, usize)> {
    let mut lines: Vec<(usize, usize)> = Vec::new();
    for &y in rows {
        match lines.last_mut() {
            Some((_, end)) if *end + 1 == y => *end = y,
            _ => lines.push((y, y)),
        }
    }
    lines
}

fn ink(shown: &Shown, x0: usize, y0: usize) -> Vec<bool> {
    (y0..y0 + 30)
        .flat_map(|y| (x0..x0 + 260).map(move |x| (x, y)))
        .map(|(x, y)| shown.at(x, y)[..3].iter().all(|c| *c > 0xe8))
        .collect()
}

fn bounds(mask: &[bool]) -> (usize, usize, usize, usize) {
    let on: Vec<(usize, usize)> = (0..mask.len())
        .filter(|i| mask[*i])
        .map(|i| (i % 260, i / 260))
        .collect();
    let xs = on.iter().map(|p| p.0);
    let ys = on.iter().map(|p| p.1);
    (
        xs.clone().min().unwrap_or(0),
        ys.clone().min().unwrap_or(0),
        xs.max().unwrap_or(0),
        ys.max().unwrap_or(0),
    )
}

#[test]
fn menu_text_is_the_same_size_on_every_panel() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let taps = [Btn::Menu, Btn::Down, Btn::Down, Btn::A, Btn::Down, Btn::Down, Btn::A];
    let reference = shelf(&mut c, (720, 480), &taps);
    let square = shelf(&mut c, (720, 720), &taps);
    let low = text_lines(&lit_rows(&reference, 40..300, 50..422));
    let high = text_lines(&lit_rows(&square, 120..300, 50..662));
    assert!(
        high.len() > low.len(),
        "{} rows at 720x480, {} at 720x720",
        low.len(),
        high.len()
    );
    assert_eq!(
        bounds(&ink(&square, 40, high[0].0 - 3)),
        bounds(&ink(&reference, 40, low[0].0 - 3)),
        "the first row's text changes size on the taller panel"
    );
}

const LIME: [u8; 4] = [50, 205, 50, 255];

fn gba(c: &mut Compositor, panel: (u32, u32), scale: Scale, card_bezel: bool) -> Shown {
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    clocked(d.path());
    let mut s = read_slot_state(d.path());
    s.scale_gba = scale;
    if card_bezel {
        let dir = d.path().join(slot::bezel::GAME_BEZELS).join("GBA");
        std::fs::create_dir_all(&dir).expect("bezel dir");
        let blue: Vec<u8> = [0u8, 0, 255, 255].repeat(720 * 480);
        let file = std::fs::File::create(dir.join("blue.png")).expect("bezel png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), 720, 480);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&blue)
            .expect("png data");
        s.bezel_gba = BezelPick::File("blue.png".into());
    }
    write_slot_state(d.path(), &s).expect("write slot.state");
    let name = format!("game-{}x{}-{:?}-{card_bezel}", panel.0, panel.1, scale);
    played(c, panel, d.path(), &name)
}

fn played(c: &mut Compositor, panel: (u32, u32), root: &std::path::Path, name: &str) -> Shown {
    let mut a = App::boot(root);
    a.set_panel(panel);
    for _ in 0..300 {
        a.update(DT);
    }
    a.apply(Action::Insert);
    a.set_video_mode(slot::video_mode::video_mode_for(root, "Tetris"));
    a.on_core_ready();
    a.set_game_ready(true);
    for _ in 0..240 {
        a.update(DT);
    }
    c.fit(panel);
    let (w, h) = a.game_bezel_size();
    if let BezelPick::File(file) = a.resolved_bezel(a.playing_platform()) {
        let path = root.join(slot::bezel::GAME_BEZELS).join("GBA").join(file);
        let tex = slot_ui::game_bezel_face(&path, w, h).map(|rgba| c.create_texture(w, h, &rgba));
        a.set_game_bezel(tex);
    }
    c.set_picture(a.picture_rect());
    c.set_game_source_rect(a.source_rect());
    c.set_game_layout(a.game_layout());
    c.set_placeholder_bezel(a.placeholder_bezel());
    c.set_screen_power(a.screen_power());
    c.set_screen_footer(a.screen_footer());
    c.set_game_bezel(a.shown_game_bezel());
    c.begin_frame();
    c.upload_game(&[0xff, 0xff, 0xff, 0xff].repeat((SRC_W * SRC_H) as usize));
    let mut draws: Vec<Draw> = Vec::new();
    a.draw(&mut draws);
    c.draw_list(&draws);
    let px = c.read_window(panel);
    write_png(name, panel.0, panel.1, &px);
    Shown {
        w: panel.0 as usize,
        px,
    }
}

fn picture_rows(shown: &Shown, x: usize, h: usize) -> (usize, usize) {
    let rows: Vec<usize> = (0..h).filter(|y| shown.at(x, *y) != LIME).collect();
    (rows[0], rows[rows.len() - 1])
}

#[test]
fn gba_on_the_square_panel_has_lime_bars() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let shown = gba(&mut c, (720, 720), Scale::Integer, false);
    assert_eq!(shown.at(360, 60), LIME);
    assert_eq!(shown.at(360, 660), LIME);
    assert_eq!(picture_rows(&shown, 360, 720), (120, 599));
}

#[test]
fn gba_on_the_ds_plus_is_4x() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let shown = gba(&mut c, (1024, 768), Scale::Fit, false);
    assert_eq!(shown.at(10, 10), LIME);
    let cols: Vec<usize> = (0..1024).filter(|x| shown.at(*x, 384) != LIME).collect();
    assert_eq!((cols[0], cols[cols.len() - 1]), (32, 991));
}

#[test]
fn a_card_bezel_is_not_used_on_the_square_panel() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let shown = gba(&mut c, (720, 720), Scale::Integer, true);
    assert_eq!(shown.at(360, 60), LIME);
}

#[test]
fn contact_sheet() {
    if std::env::var_os("SCRATCH_PNG_DIR").is_none() {
        return;
    }
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    for panel in [(720, 720), (1024, 768)] {
        shelf(&mut c, panel, &[]);
        shelf(&mut c, panel, &[Btn::Menu]);
        shelf(&mut c, panel, &[Btn::Menu, Btn::A]);
        for scale in [
            Scale::SpFrame,
            Scale::Integer,
            Scale::Fit,
            Scale::FullScreen,
        ] {
            gba(&mut c, panel, scale, false);
        }
        for colour in [false, true] {
            for (mode, scale, stretch) in [
                (VideoMode::Actual, Scale::SpFrame, Stretch::Classic),
                (VideoMode::Actual, Scale::Integer, Stretch::Classic),
                (VideoMode::Actual, Scale::Fit, Stretch::Classic),
                (VideoMode::Stretch, Scale::SpFrame, Stretch::Classic),
                (VideoMode::Stretch, Scale::SpFrame, Stretch::FullScreen),
            ] {
                let d = match colour {
                    true => tmp_root_with_gbc_carts(&["Tetris", "Zzz"]),
                    false => tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
                };
                clocked(d.path());
                let mut s = read_slot_state(d.path());
                s.scale_gb = scale;
                s.scale_gbc = scale;
                s.stretch_gb = stretch;
                s.stretch_gbc = stretch;
                write_slot_state(d.path(), &s).expect("write slot.state");
                write_video_mode(d.path(), "Tetris", mode).expect("write video mode");
                let system = if colour { "gbc" } else { "gb" };
                let name = format!(
                    "{system}-{}x{}-{mode:?}-{scale:?}-{stretch:?}",
                    panel.0, panel.1
                );
                played(&mut c, panel, d.path(), &name);
            }
        }
    }
}

#[test]
fn no_cart_shows_as_a_sliver_on_a_wide_panel() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let panel = (1024, 768);
    let d = tmp_root_with_gb_carts(&["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(
        SimPlatform::at(d.path().to_path_buf()).stopped_at(STOPPED),
    ));
    c.fit(panel);
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, panel);
    let mut input = Script(VecDeque::new());
    for k in 0..300 {
        f.advance_at(&mut input, 16 * (k + 1), DT);
    }
    let mut draws = Vec::new();
    f.app().draw(&mut draws);
    let slivers: Vec<(f32, f32)> = draws
        .iter()
        .filter_map(|d| match d {
            Draw::Tex { x, w, h, alpha, .. } if *h > 200.0 && *w > 100.0 && *alpha > 0.0 => {
                let shown = (x + w).min(1024.0) - x.max(0.0);
                (shown < w * 0.25).then_some((*x, shown))
            }
            _ => None,
        })
        .collect();
    assert!(slivers.is_empty(), "carts showing as slivers: {slivers:?}");
}

fn label_run(shown: &Shown, y: usize) -> usize {
    let label = shown.at(shown.w / 2, y);
    let near = |p: [u8; 4]| p[..3].iter().zip(&label[..3]).all(|(a, b)| a.abs_diff(*b) < 12);
    let left = (0..shown.w / 2).rev().take_while(|x| near(shown.at(*x, y))).count();
    let right = (shown.w / 2..shown.w).take_while(|x| near(shown.at(*x, y))).count();
    left + right
}

#[test]
fn the_ds_plus_draws_carts_at_1_28x() {
    let Some((_s, mut c)) = compositor() else {
        return;
    };
    let reference = shelf(&mut c, (720, 480), &[]);
    let ds = shelf(&mut c, (1024, 768), &[]);
    let (low, high) = (label_run(&reference, 200), label_run(&ds, 333));
    let ratio = high as f32 / low as f32;
    assert!(
        (ratio - 1.28).abs() < 0.03,
        "label {low} px at 720x480, {high} px on the RG DS+: {ratio}"
    );
}
