#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::path::Path;

use common::{clocked, tmp_root_with_carts, tmp_root_with_gb_carts};
use slot::app::App;
use slot::frontend::Frontend;
use slot::video_mode::{write_video_mode, VideoMode};
use slot_gfx::{Compositor, Draw, HeadlessSurface, SRC_H, SRC_W};
use slot_input::{Action, Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::{read_slot_state, write_slot_state, BezelPick, Platform, Scale, Stretch};
use slot_ui::default_game_bezel_face;

const FIXTURE: &str = "tests/fixtures/layout_golden.txt";
const PANELS: [(u32, u32); 2] = [(720, 480), (640, 480)];
const DT: f32 = 1.0 / 60.0;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

struct Clock {
    now: Millis,
}

impl Clock {
    fn step(&mut self, f: &mut Frontend, input: &mut Script) {
        self.now += 16;
        f.advance_at(input, self.now, DT);
    }

    fn settle(&mut self, f: &mut Frontend, input: &mut Script, frames: usize) {
        for _ in 0..frames {
            self.step(f, input);
        }
    }

    fn tap(&mut self, f: &mut Frontend, input: &mut Script, btn: Btn) {
        input.0.push_back(vec![RawEvent::Down(btn)]);
        self.step(f, input);
        input.0.push_back(vec![RawEvent::Up(btn)]);
        self.settle(f, input, 30);
    }
}

#[derive(Clone, Copy)]
enum Card {
    Gba(Scale),
    Gb(VideoMode, Scale, Stretch),
}

enum Scene {
    Ui(&'static str, &'static [Btn]),
    Game(&'static str, Card),
}

impl Scene {
    fn name(&self) -> &'static str {
        match self {
            Scene::Ui(name, _) | Scene::Game(name, _) => name,
        }
    }
}

fn scenes() -> Vec<Scene> {
    use Scene::{Game, Ui};
    vec![
        Ui("shelf", &[]),
        Ui("quick-menu", &[Btn::Menu, Btn::Down]),
        Ui("settings-long-list", &[Btn::Menu, Btn::A]),
        Game("gba-sp-frame", Card::Gba(Scale::SpFrame)),
        Game("gba-integer", Card::Gba(Scale::Integer)),
        Game("gba-fit", Card::Gba(Scale::Fit)),
        Game("gba-fullscreen", Card::Gba(Scale::FullScreen)),
        Game(
            "gb-actual-sp-frame",
            Card::Gb(VideoMode::Actual, Scale::SpFrame, Stretch::Classic),
        ),
        Game(
            "gb-actual-integer",
            Card::Gb(VideoMode::Actual, Scale::Integer, Stretch::Classic),
        ),
        Game(
            "gb-actual-fit",
            Card::Gb(VideoMode::Actual, Scale::Fit, Stretch::Classic),
        ),
        Game(
            "gb-stretch-classic",
            Card::Gb(VideoMode::Stretch, Scale::SpFrame, Stretch::Classic),
        ),
        Game(
            "gb-stretch-fullscreen",
            Card::Gb(VideoMode::Stretch, Scale::SpFrame, Stretch::FullScreen),
        ),
    ]
}

fn prepare(root: &Path, card: Card) {
    clocked(root);
    let mut s = read_slot_state(root);
    s.wallpaper = slot_store::WallpaperPick::None;
    s.bezel_gba = BezelPick::Default;
    s.bezel_gb = BezelPick::Default;
    match card {
        Card::Gba(scale) => s.scale_gba = scale,
        Card::Gb(mode, scale, stretch) => {
            s.scale_gb = scale;
            s.stretch_gb = stretch;
            write_video_mode(root, "Tetris", mode).expect("write video mode");
        }
    }
    write_slot_state(root, &s).expect("write slot.state");
}

fn render_ui(c: &mut Compositor, taps: &[Btn], panel: (u32, u32)) -> Vec<u8> {
    let d = tmp_root_with_carts(&["Emerald", "Zzz"]);
    prepare(d.path(), Card::Gba(Scale::SpFrame));
    let mut f = Frontend::boot(Box::new(
        SimPlatform::at(d.path().to_path_buf()).stopped_at(1_793_000_000),
    ));
    c.fit(panel);
    f.upload_faces(c);
    f.upload_bezel(c, panel);
    let mut input = Script(VecDeque::new());
    let mut clock = Clock { now: 0 };
    clock.settle(&mut f, &mut input, 300);
    for btn in taps {
        clock.tap(&mut f, &mut input, *btn);
    }
    clock.settle(&mut f, &mut input, 60);
    f.compose(c);
    c.read_window(panel)
}

fn render_game(c: &mut Compositor, card: Card, panel: (u32, u32)) -> Vec<u8> {
    let d = match card {
        Card::Gba(_) => tmp_root_with_carts(&["Emerald", "Zzz"]),
        Card::Gb(..) => tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
    };
    prepare(d.path(), card);
    let mut a = App::boot(d.path());
    a.set_panel(panel);
    for _ in 0..300 {
        a.update(DT);
    }
    a.apply(Action::Insert);
    a.set_video_mode(slot::video_mode::video_mode_for(d.path(), "Tetris"));
    a.on_core_ready();
    a.set_game_ready(true);
    for _ in 0..240 {
        a.update(DT);
    }
    c.fit(panel);
    let platform = a.playing_platform();
    let (w, h) = a.game_bezel_size();
    let bezel = a
        .bezel_art()
        .and_then(|kind| default_game_bezel_face(platform == Platform::Gbc, kind, w, h))
        .map(|rgba| c.create_texture(w, h, &rgba));
    a.set_game_bezel(bezel);
    c.set_picture(a.picture_rect());
    c.set_game_source_rect(a.source_rect());
    c.set_game_layout(a.game_layout());
    c.set_placeholder_bezel(a.placeholder_bezel());
    c.set_screen_power(a.screen_power());
    c.set_screen_footer(a.screen_footer());
    c.set_game_bezel(a.shown_game_bezel());
    c.begin_frame();
    c.upload_game(&pattern());
    let mut draws: Vec<Draw> = Vec::new();
    a.draw(&mut draws);
    c.draw_list(&draws);
    c.read_window(panel)
}

fn pattern() -> Vec<u8> {
    (0..SRC_H)
        .flat_map(|y| {
            (0..SRC_W).flat_map(move |x| {
                let edge = x == 0 || y == 0 || x == SRC_W - 1 || y == SRC_H - 1;
                match edge {
                    true => [0xff, 0xff, 0xff, 0xff],
                    false => [(x * 255 / SRC_W) as u8, (y * 255 / SRC_H) as u8, 0x80, 0xff],
                }
            })
        })
        .collect()
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3)
    })
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

#[test]
fn frames_match_the_recorded_hashes() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut lines = Vec::new();
    for scene in scenes() {
        for panel in PANELS {
            let frame = match &scene {
                Scene::Ui(_, taps) => render_ui(&mut c, taps, panel),
                Scene::Game(_, card) => render_game(&mut c, *card, panel),
            };
            let name = format!("{}-{}x{}", scene.name(), panel.0, panel.1);
            write_png(&name, panel.0, panel.1, &frame);
            lines.push(format!(
                "{} {}x{} {:016x}",
                scene.name(),
                panel.0,
                panel.1,
                fnv(&frame)
            ));
        }
    }
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    if std::env::var_os("SLOT_RECORD_GOLDEN").is_some() {
        std::fs::write(&path, lines.join("\n") + "\n").expect("record golden hashes");
        return;
    }
    let recorded = std::fs::read_to_string(&path).expect("read golden hashes");
    let recorded: Vec<&str> = recorded.lines().collect();
    let moved: Vec<String> = lines
        .iter()
        .filter(|l| !recorded.contains(&l.as_str()))
        .cloned()
        .collect();
    assert!(moved.is_empty(), "frames changed: {moved:#?}");
    assert_eq!(recorded.len(), lines.len(), "scene list changed");
}
