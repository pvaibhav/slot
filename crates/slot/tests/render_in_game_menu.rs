#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_ui::{quick_row_top, QuickRow, MOUTH_H};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

fn run_for(f: &mut Frontend, input: &mut Script, secs: f32) {
    let until = Instant::now() + Duration::from_secs_f32(secs);
    while Instant::now() < until {
        f.advance(input);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/in-game-menu-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
    }
    px
}

fn bright(p: [u8; 3]) -> u32 {
    p.iter().map(|&c| u32::from(c)).sum()
}

#[test]
fn the_in_game_menu_darkens_the_game_and_screen_leaves_it_bare() {
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
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    tap(&mut f, &mut input, Btn::Menu);
    run_for(&mut f, &mut input, 0.1);
    let held = composed(&mut f, &mut c, "held");
    run_for(&mut f, &mut input, 0.4);
    assert_eq!(f.app().in_game_menu(), Some(QuickRow::Resume));
    let top = composed(&mut f, &mut c, "top");
    for btn in [Btn::Down, Btn::A, Btn::A] {
        tap(&mut f, &mut input, btn);
    }
    run_for(&mut f, &mut input, 0.3);
    assert_eq!(f.app().in_game_menu(), Some(QuickRow::Shader));
    let screen = composed(&mut f, &mut c, "screen");

    let panel = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::Shader, &f.app().in_game_hidden()) as usize - 16;
    let (mut probed, mut lit) = (0, 0);
    for y in (48..panel).step_by(17) {
        for x in (0..150).chain(570..OUT_W as usize).step_by(13) {
            let (bare, dark) = (at(&screen, x, y), at(&top, x, y));
            assert_eq!(
                at(&held, x, y),
                bare,
                "({x}, {y}): the menu drew inside the double tap window"
            );
            for ch in 0..3 {
                let want = f32::from(bare[ch]) * 0.2;
                assert!(
                    (f32::from(dark[ch]) - want).abs() <= 3.0,
                    "({x}, {y}) channel {ch}: bare {} drew {} under the scrim, want about {want}",
                    bare[ch],
                    dark[ch]
                );
            }
            probed += 1;
            lit += usize::from(bright(bare) > 60);
        }
    }
    assert!(
        lit * 4 > probed,
        "the game is too dark to prove anything: {lit} of {probed} probes lit"
    );

    for (side, xs) in [("battery", 8..120), ("clock", 600..712)] {
        let ink = (6..34)
            .flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| bright(at(&top, x, y)) > 600)
            .count();
        assert!(
            ink > 20,
            "no {side} in the menu's corner: {ink} bright pixels"
        );
    }

    let row = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::Shader, &f.app().in_game_hidden()) as usize;
    let ink = (row..row + 40)
        .flat_map(|y| (64..400).map(move |x| (x, y)))
        .filter(|&(x, y)| bright(at(&screen, x, y)) > 600)
        .count();
    assert!(
        ink > 100,
        "no Shader label in the panel: {ink} bright pixels"
    );
    let band = |rows: std::ops::Range<usize>| {
        let n = rows.len() as u32;
        rows.map(|y| bright(at(&screen, 700, y))).sum::<u32>() / n
    };
    let mouth = OUT_H as usize - MOUTH_H as usize;
    let below = band(mouth - 17..mouth - 2);
    let above = band(panel - 23..panel - 8);
    assert!(
        below <= above,
        "the panel is not darker than the game above it: {below} under, {above} over"
    );
}

#[test]
fn the_brightness_rows_render_on_the_screen_page() {
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
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    tap(&mut f, &mut input, Btn::Menu);
    run_for(&mut f, &mut input, 0.5);
    for btn in [Btn::Down, Btn::A, Btn::A] {
        tap(&mut f, &mut input, btn);
    }
    while f.app().in_game_menu() != Some(QuickRow::PlayBrightness) {
        tap(&mut f, &mut input, Btn::Down);
    }
    run_for(&mut f, &mut input, 0.3);
    let hidden = f.app().in_game_hidden();
    for row in [
        QuickRow::PlayBrightness,
        QuickRow::PlayBlueLight,
        QuickRow::PlaySaturation,
        QuickRow::PlayContrast,
    ] {
        if f.app().in_game_menu() != Some(row) {
            tap(&mut f, &mut input, Btn::Down);
            run_for(&mut f, &mut input, 0.2);
        }
        assert_eq!(f.app().in_game_menu(), Some(row));
        let px = composed(&mut f, &mut c, &format!("screen-{row:?}"));
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &hidden) as usize;
        let value = (top..top + 40)
            .flat_map(|y| (360..OUT_W as usize - 24).map(move |x| (x, y)))
            .filter(|&(x, y)| bright(at(&px, x, y)) > 600)
            .map(|(x, _)| x)
            .max()
            .expect("a row with no value");
        assert!(
            (679..=688).contains(&value),
            "{row:?}'s value ends at x {value}"
        );
    }
}

#[test]
fn deleting_a_state_asks_with_b_to_keep_and_x_to_delete() {
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
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    for ev in [
        RawEvent::Down(Btn::Select),
        RawEvent::Down(Btn::R1),
        RawEvent::Up(Btn::R1),
        RawEvent::Up(Btn::Select),
    ] {
        input.0.push_back(vec![ev]);
        f.advance(&mut input);
    }
    run_for(&mut f, &mut input, 0.5);
    tap(&mut f, &mut input, Btn::Menu);
    tap(&mut f, &mut input, Btn::Menu);
    run_for(&mut f, &mut input, 0.5);
    tap(&mut f, &mut input, Btn::Y);
    assert!(f.app().delete_prompt(), "Y did not ask before deleting");
    let px = composed(&mut f, &mut c, "delete-prompt");
    let lit = |ys: std::ops::Range<usize>, xs: std::ops::Range<usize>| {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| bright(at(&px, x, y)) > 600)
            .count()
    };
    assert!(
        lit(180..300, 100..620) > 300,
        "no question in the middle of the screen"
    );
    assert!(
        lit(0..150, 0..OUT_W as usize) == 0,
        "the game shows through the prompt"
    );
    assert!(lit(420..480, 0..240) > 50, "no Keep legend on the left");
    assert!(
        lit(420..480, 480..720) > 50,
        "no Delete legend on the right"
    );
}

#[test]
fn the_controls_page_renders() {
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
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    tap(&mut f, &mut input, Btn::Menu);
    run_for(&mut f, &mut input, 0.5);
    for btn in [Btn::Down, Btn::A, Btn::Down, Btn::Down, Btn::A] {
        tap(&mut f, &mut input, btn);
    }
    run_for(&mut f, &mut input, 0.3);
    assert_eq!(f.app().in_game_menu(), Some(QuickRow::GameUp));
    composed(&mut f, &mut c, "controls");
    for _ in 0..7 {
        tap(&mut f, &mut input, Btn::Down);
    }
    run_for(&mut f, &mut input, 0.2);
    assert_eq!(f.app().in_game_menu(), Some(QuickRow::GameStart));
    composed(&mut f, &mut c, "controls-scrolled");
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 0.2);
    assert!(f.app().capturing());
    composed(&mut f, &mut c, "controls-waiting");
    input.0.push_back(vec![RawEvent::Down(Btn::Menu)]);
    f.advance(&mut input);
    tap(&mut f, &mut input, Btn::X);
    input.0.push_back(vec![RawEvent::Up(Btn::Menu)]);
    f.advance(&mut input);
    run_for(&mut f, &mut input, 0.2);
    assert!(!f.app().capturing());
    tap(&mut f, &mut input, Btn::Up);
    run_for(&mut f, &mut input, 0.2);
    composed(&mut f, &mut c, "controls-bound");
}
