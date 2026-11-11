mod common;

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use common::{clocked, tmp_root_with_carts};
use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs};
use slot::fetch_art::{FetchArt, Stage};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_W};
use slot_input::{Action, Btn};
use slot_power::SimPlatform;
use slot_store::Platform;
use tempfile::TempDir;

#[derive(Default)]
struct Log {
    asked: Vec<ArtJob>,
    events: VecDeque<ArtEvent>,
}

#[derive(Clone, Default)]
struct FakeArt(Arc<Mutex<Log>>);

impl ArtJobs for FakeArt {
    fn ask(&mut self, job: ArtJob) {
        self.0.lock().unwrap().asked.push(job);
    }

    fn take(&mut self) -> Option<ArtEvent> {
        self.0.lock().unwrap().events.pop_front()
    }
}

impl FakeArt {
    fn push(&self, event: ArtEvent) {
        self.0.lock().unwrap().events.push_back(event);
    }
}

const BLUE: [u8; 3] = [40, 60, 200];

fn label_png() -> Vec<u8> {
    let (w, h) = (1280u32, 661u32);
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&BLUE.repeat((w * h) as usize))
            .unwrap();
    }
    out
}

fn setup() -> Option<(TempDir, Frontend, Compositor, HeadlessSurface, FakeArt)> {
    setup_with_shell(None)
}

fn setup_with_shell(
    shell: Option<&str>,
) -> Option<(TempDir, Frontend, Compositor, HeadlessSurface, FakeArt)> {
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut state = slot_store::read_slot_state(d.path());
    state.wallpaper = slot_store::WallpaperPick::None;
    slot_store::write_slot_state(d.path(), &state).unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    if let Some(shell) = shell {
        std::fs::write(d.path().join("Config/cart_shell.txt"), shell).unwrap();
    }
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    let art = FakeArt::default();
    f.set_art_jobs(Box::new(art.clone()));
    Some((d, f, compositor, surface, art))
}

fn dump(frame: &[u8], name: &str) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let file = std::fs::File::create(format!("{dir}/fetch-{name}.png")).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, slot_gfx::OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(frame).unwrap();
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [frame[o], frame[o + 1], frame[o + 2]]
}

fn near(a: [u8; 3], b: [u8; 3], by: i32) -> bool {
    (0..3).all(|k| (a[k] as i32 - b[k] as i32).abs() <= by)
}

fn luma(p: [u8; 3]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn compose_until(f: &mut Frontend, c: &mut Compositor, done: impl Fn(&Frontend) -> bool) {
    for _ in 0..2000 {
        let now = f.app().now();
        f.app_mut().tick_ms(now + 5);
        f.compose(c);
        if done(f) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("never got there");
}

fn press(f: &mut Frontend, btn: Btn) {
    f.app_mut().apply(Action::GbaDown(btn));
    f.app_mut().apply(Action::GbaUp(btn));
}

fn first_edge(frame: &[u8], x: usize) -> usize {
    let ground = px(frame, x, 100);
    (100..400)
        .find(|&y| !near(px(frame, x, y), ground, 30))
        .unwrap_or(400)
}

#[test]
fn choice_screen_renders() {
    let Some((_d, mut f, mut c, _s, _art)) = setup() else {
        return;
    };
    compose_until(&mut f, &mut c, |f| f.app().fetch_examples_ready());
    f.app_mut().open_fetch_art();
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "choice");
    let (left, right) = (first_edge(&frame, 200), first_edge(&frame, 520));
    assert!((8..=12).contains(&(right as i32 - left as i32)), "left {left} right {right}");
    let inked = |xs: std::ops::Range<usize>| {
        xs.flat_map(|x| (430..470).map(move |y| (x, y)))
            .filter(|&(x, y)| luma(px(&frame, x, y)) > 180.0)
            .count()
    };
    assert!(inked(0..200) > 30, "no CANCEL legend on the bar");
    assert!(inked(520..720) > 30, "no PICK legend on the bar");
}

fn running_with_label(f: &mut Frontend, c: &mut Compositor, d: &TempDir, art: &FakeArt) -> f64 {
    let path = d.path().join("Labels/GBA/Emerald.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, label_png()).unwrap();
    f.app_mut().open_fetch_art();
    press(f, Btn::A);
    art.push(ArtEvent::Counted(1));
    art.push(ArtEvent::Landed {
        stem: "Emerald".into(),
        platform: Platform::Gba,
        path,
    });
    compose_until(f, c, |f| match f.app().fetch_art() {
        Some(FetchArt::Running(run)) => run.scene.is_some(),
        _ => false,
    });
    match f.app().fetch_art() {
        Some(FetchArt::Running(run)) => run.scene.as_ref().unwrap().started_ms,
        _ => unreachable!(),
    }
}

#[test]
fn mid_peel_shows_paper_and_well() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut().tick_ms((start + slot::fetch_art::stage_start(Stage::Peel) + 200.0) as u64);
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "peel");
    let paper = (228..500)
        .flat_map(|x| (110..300).map(move |y| (x, y)))
        .any(|(x, y)| near(px(&frame, x, y), [0xf6, 0xf4, 0xef], 10));
    assert!(paper, "no paper back showing mid-peel");
    assert!(!near(px(&frame, 486, 222), BLUE, 40), "the new label showed before it was applied");
}

#[test]
fn landed_face_is_identical_at_rest() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut()
        .tick_ms((start + slot::fetch_art::stage_start(Stage::Rest) + 200.0) as u64);
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "rest");
    assert!(near(px(&frame, 360, 222), BLUE, 3), "{:?}", px(&frame, 360, 222));
    f.app_mut()
        .tick_ms((start + slot::fetch_art::stage_start(Stage::Rest) + 260.0) as u64);
    f.compose(&mut c);
    let later = c.read_frame();
    for y in 120..320 {
        for x in 240..480 {
            assert_eq!(px(&frame, x, y), px(&later, x, y), "the landed face moved at {x},{y}");
        }
    }
}

#[test]
fn a_clear_shell_shows_its_board_under_the_bare_well() {
    let Some((d, mut f, mut c, _s, art)) = setup_with_shell(Some("Emerald = auto ff2020 clear\n")) else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut().tick_ms((start + slot::fetch_art::stage_start(Stage::Peel) + 200.0) as u64);
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "clear-peel");
    let p = px(&frame, 486, 222);
    assert!(p[1] > 30, "the bare well shows plain plastic, no board: {p:?}");
}

fn example_up(f: &mut Frontend, c: &mut Compositor) {
    compose_until(f, c, |f| f.app().fetch_examples_ready());
    f.app_mut().open_fetch_art();
    press(f, Btn::A);
    compose_until(f, c, |f| !f.app().fetch_art_lines().is_empty());
    f.compose(c);
}

#[test]
fn connecting_is_centered_text_alone() {
    let Some((_d, mut f, mut c, _s, _art)) = setup() else {
        return;
    };
    example_up(&mut f, &mut c);
    let frame = c.read_frame();
    dump(&frame, "connecting");
    let lit: Vec<usize> = (0..422)
        .filter(|&y| (0..720).any(|x| luma(px(&frame, x, y)) > 180.0))
        .collect();
    let (top, bottom) = (lit[0], lit[lit.len() - 1]);
    assert!(((top + bottom) as i32 - 422).abs() <= 8, "text spans {top}..{bottom}");
    assert!(bottom - top < 60, "more than a line of text: {top}..{bottom}");
}

#[test]
fn summary_with_nothing_landed_has_no_cart() {
    let Some((_d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    example_up(&mut f, &mut c);
    art.push(ArtEvent::Counted(1));
    art.push(ArtEvent::Missed { stem: "Fusion".into() });
    art.push(ArtEvent::Done { landed: 0, missed: vec!["Fusion".into()] });
    compose_until(&mut f, &mut c, |f| matches!(f.app().fetch_art(), Some(FetchArt::Summary { .. })));
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "summary-empty");
    let ink: Vec<usize> = (0..720)
        .filter(|&x| (0..422).any(|y| luma(px(&frame, x, y)) > 180.0))
        .collect();
    assert!(ink[0] > 150 && *ink.last().unwrap() < 570, "{}..{}", ink[0], ink.last().unwrap());
}

#[test]
fn summary_shows_a_cart_that_landed() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    art.push(ArtEvent::Done { landed: 1, missed: vec![] });
    f.app_mut().tick_ms((start + slot::fetch_art::SCENE_MS + 10.0) as u64);
    compose_until(&mut f, &mut c, |f| {
        matches!(f.app().fetch_art(), Some(FetchArt::Summary { .. })) && f.app().fetch_featured_ready()
    });
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "summary");
    assert!(near(px(&frame, 188, 211), BLUE, 6), "{:?}", px(&frame, 188, 211));
}

#[test]
fn the_cart_itself_is_centered_above_the_footer() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut()
        .tick_ms((start + slot::fetch_art::stage_start(Stage::Rest) + 200.0) as u64);
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "centered");
    let blue: Vec<usize> = (0..422).filter(|&y| near(px(&frame, 360, y), BLUE, 6)).collect();
    let well = (blue[0] + blue[blue.len() - 1]) as i32;
    let want = 2 * (106 + 118);
    assert!((well - want).abs() <= 4, "well spans {}..{}", blue[0], blue[blue.len() - 1]);
}

#[test]
fn the_count_sits_under_the_name() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    let start = running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut()
        .tick_ms((start + slot::fetch_art::stage_start(Stage::Rest) + 200.0) as u64);
    f.compose(&mut c);
    let frame = c.read_frame();
    dump(&frame, "applying");
    let inked = |ys: std::ops::Range<usize>| {
        ys.flat_map(|y| (200..520).map(move |x| (x, y)))
            .filter(|&(x, y)| luma(px(&frame, x, y)) > 180.0)
            .count()
    };
    assert!(inked(392..416) > 30, "no count on the bottom line");
    assert!(inked(362..388) > 30, "no name above the count");
    assert!(inked(68..100) == 0, "text between the header and the cart");
    assert!(inked(26..64) > 30, "no header");
}

#[test]
fn skip_shows_on_the_bar() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    running_with_label(&mut f, &mut c, &d, &art);
    f.compose(&mut c);
    let frame = c.read_frame();
    let inked = (520..720)
        .flat_map(|x| (430..470).map(move |y| (x, y)))
        .filter(|&(x, y)| luma(px(&frame, x, y)) > 180.0)
        .count();
    assert!(inked > 30, "no SKIP legend");
}

#[test]
fn skipping_draws_a_centered_bar_and_no_cart() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut().apply(Action::GbaDown(Btn::X));
    f.app_mut().apply(Action::GbaUp(Btn::X));
    for _ in 0..40 {
        let now = f.app().now();
        f.app_mut().tick_ms(now + 5);
        f.compose(&mut c);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let frame = c.read_frame();
    dump(&frame, "skip-progress");
    assert!(!near(px(&frame, 360, 120), BLUE, 40), "a cart is still showing");
    let bright = (150..570).filter(|&x| luma(px(&frame, x, 210)) > 200.0).count();
    assert!(bright > 300, "no filled bar: {bright}");
}

#[test]
fn the_summary_cart_is_there_on_its_first_frame() {
    let Some((d, mut f, mut c, _s, art)) = setup() else {
        return;
    };
    running_with_label(&mut f, &mut c, &d, &art);
    f.app_mut().apply(Action::GbaDown(Btn::X));
    f.app_mut().apply(Action::GbaUp(Btn::X));
    art.push(ArtEvent::Done { landed: 1, missed: Vec::new() });
    compose_until(&mut f, &mut c, |f| matches!(f.app().fetch_art(), Some(FetchArt::Summary { .. })));
    assert!(f.app().fetch_featured_ready(), "the cart popped in after the summary");
}
