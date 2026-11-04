#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;

use common::tmp_root_with_carts;
use slot::frontend::Frontend;
use slot::wifi::{Fake, Wifi};
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Action, Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::{write_slot_state, SlotState};
use slot_ui::{ClockRow, QuickRow, MOUTH_H, QUICK_LABEL_X, QUICK_PITCH};

const LEGEND_Y_FOR_TEST: f32 = 427.0;

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

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/clock-{name}.png");
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

fn bright(px: &[u8], x: usize, y: usize) -> bool {
    let at = (y * OUT_W as usize + x) * 4;
    px[at..at + 3].iter().all(|&c| c > 0xd0)
}

fn inked_rows(px: &[u8], xs: std::ops::Range<usize>) -> Vec<usize> {
    (0..(OUT_H as f32 - MOUTH_H) as usize)
        .filter(|&y| xs.clone().any(|x| bright(px, x, y)))
        .collect()
}

fn bands(rows: &[usize]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for &y in rows {
        match out.last_mut() {
            Some((_, end)) if *end + 1 == y => *end = y,
            _ => out.push((y, y)),
        }
    }
    out
}

#[test]
fn the_clock_is_a_list_of_four_rows_with_the_open_part_underlined() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);

    let px = composed(&mut f, &mut c, "closed");
    let x = QUICK_LABEL_X as usize;
    let label_bands = bands(&inked_rows(&px, x..x + 120));
    assert_eq!(
        label_bands.len(),
        1,
        "only the selected label is in full ink: {label_bands:?}"
    );

    tap(&mut f, &mut input, Btn::A);
    let open = composed(&mut f, &mut c, "open");
    let right = OUT_W as usize - 40;
    let value_bands = bands(&inked_rows(&open, right - 200..right));
    let underline = value_bands
        .iter()
        .find(|(top, end)| end - top <= 4 && *top > label_bands[0].1)
        .unwrap_or_else(|| panic!("no underline under the date: {value_bands:?}"));
    assert!(
        (underline.0 as f32 - label_bands[0].0 as f32) < QUICK_PITCH,
        "the underline is not on the date's row: {underline:?} vs {:?}",
        label_bands[0]
    );

    tap(&mut f, &mut input, Btn::A);
    for _ in 0..2 {
        tap(&mut f, &mut input, Btn::Down);
    }
    assert_eq!(f.app().picker().map(|p| p.row()), Some(ClockRow::Zone));
    composed(&mut f, &mut c, "city");
    tap(&mut f, &mut input, Btn::A);
    assert!(
        f.app().picker().and_then(|p| p.highlighted()).is_some(),
        "A on the city did not open the list"
    );
    let cities = composed(&mut f, &mut c, "cities");
    let rows = bands(&inked_rows(&cities, x..x + 200));
    assert_eq!(
        rows.len(),
        1,
        "only the highlighted city is in full ink: {rows:?}"
    );
}

#[test]
fn network_time_on_draws_three_rows_with_the_pointer_on_the_first() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            network_time: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    f.advance(&mut input);
    f.app_mut().set_wifi(Wifi::inline(Box::new(Fake::joined())));
    f.app_mut().apply(Action::QuickMenu);
    for _ in 0..QuickRow::General.position(&[]) {
        f.app_mut().apply(Action::GbaDown(Btn::Down));
    }
    f.app_mut().apply(Action::GbaDown(Btn::A));
    for _ in 0..QuickRow::DateTime.position(&f.app().hidden_rows()) {
        f.app_mut().apply(Action::GbaDown(Btn::Down));
    }
    f.app_mut().apply(Action::GbaDown(Btn::A));
    f.advance(&mut input);

    assert_eq!(
        f.app().picker().map(|p| p.rows().len()),
        Some(3),
        "{:?}",
        f.app().phase()
    );
    let px = composed(&mut f, &mut c, "network");
    let x = QUICK_LABEL_X as usize;
    let lit = bands(&inked_rows(&px, x..x + 120));
    let top = (LEGEND_Y_FOR_TEST - QUICK_PITCH * 3.0) / 2.0;
    assert_eq!(lit.len(), 1, "{lit:?}");
    assert!(
        (lit[0].0 as f32 - top).abs() < QUICK_PITCH,
        "the first row is not where three rows sit: {lit:?} vs {top}"
    );
}
