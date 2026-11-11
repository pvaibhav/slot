#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_ui::{quick_row_top, quick_row_top_at, quick_scroll, QuickRow, MOUTH_H, QUICK_VISIBLE};

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

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn inked(px: &[u8], xs: std::ops::Range<usize>, top: usize) -> Vec<usize> {
    xs.filter(|&x| (top + 8..top + 44).any(|y| at(px, x, y).iter().all(|c| *c > 0x80)))
        .collect()
}

struct Pointer {
    last: usize,
    mid: f32,
    label_first: usize,
    label_mid: f32,
}

fn lit(px: &[u8], x: usize, y: usize) -> bool {
    at(px, x, y).iter().all(|c| *c > 0xe0)
}

fn lit_rows(px: &[u8], xs: std::ops::Range<usize>, top: usize) -> f32 {
    let rows: Vec<usize> = (top..top + 52)
        .filter(|&y| xs.clone().any(|x| lit(px, x, y)))
        .collect();
    (rows[0] + rows[rows.len() - 1]) as f32 / 2.0
}

fn pointer(px: &[u8], top: usize) -> Option<Pointer> {
    let cols: Vec<usize> = (0..OUT_W as usize)
        .filter(|&x| (top..top + 52).any(|y| lit(px, x, y)))
        .collect();
    let split = cols.windows(2).position(|w| w[1] - w[0] >= 18)?;
    let (last, label_first) = (cols[split], cols[split + 1]);
    Some(Pointer {
        last,
        mid: lit_rows(px, last..last + 1, top),
        label_first,
        label_mid: lit_rows(px, label_first..label_first + 60, top),
    })
}

fn beside(name: &str, p: Option<Pointer>) -> Pointer {
    let p = p.unwrap_or_else(|| panic!("{name}: no pointer"));
    assert!(
        p.label_first - p.last < 30,
        "{name}: the pointer ends at {}, {} px short of its label",
        p.last,
        p.label_first - p.last
    );
    assert!(
        (p.mid - p.label_mid).abs() <= 1.0,
        "{name}: the pointer's tip is at row {} and the letters are centred on {}",
        p.mid,
        p.label_mid
    );
    p
}

fn composed(f: &mut Frontend, c: &mut Compositor, name: &str) -> Vec<u8> {
    f.compose(c);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/quick-menu-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

#[test]
fn the_quick_menu_renders_full_screen() {
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
    tap(&mut f, &mut input, Btn::Menu);

    let flat = [0x05, 0x05, 0x08];
    let mut bar_on = QuickRow::MAIN[0];
    let mut tips = Vec::new();
    for (name, selected) in [
        ("general", QuickRow::General),
        ("game", QuickRow::Game),
        ("screen", QuickRow::Screen),
        ("network", QuickRow::Network),
        ("system", QuickRow::System),
    ] {
        for _ in bar_on.position(&[])..selected.position(&[]) {
            tap(&mut f, &mut input, Btn::Down);
        }
        bar_on = selected;
        let px = composed(&mut f, &mut c, name);

        assert_ne!(
            at(&px, 2, OUT_H as usize - 2),
            flat,
            "{name}: the menu is on the flat ground, not the wallpaper"
        );

        let mut spans = Vec::new();
        for row in QuickRow::MAIN {
            let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &[]) as usize;
            let from = match row == selected {
                true => {
                    let p = beside(name, pointer(&px, top));
                    tips.push(p.last);
                    p.label_first
                }
                false => {
                    assert!(
                        pointer(&px, top).is_none(),
                        "{name}: a pointer at {row:?} as well"
                    );
                    0
                }
            };
            let ink = inked(&px, from..OUT_W as usize, top);
            spans.push((
                row,
                *ink.first().expect("a row with no label"),
                *ink.last().expect("a row with no label"),
            ));
        }
        let start = spans[0].1;
        for (row, first, _) in &spans {
            assert!(
                first.abs_diff(start) <= 3,
                "{name}: {row:?} starts at {first}, not in line with the others at {start}"
            );
        }
        let (row, first, last) = spans
            .iter()
            .max_by_key(|(_, first, last)| last - first)
            .expect("rows");
        let mid = (first + last) as f32 / 2.0;
        assert!(
            (mid - OUT_W as f32 / 2.0).abs() <= 2.0,
            "{name}: the column is centred on {mid}, by its widest row {row:?}"
        );
    }
    assert!(
        tips.iter().all(|t| t.abs_diff(tips[0]) <= 1),
        "the pointer moves from row to row: {tips:?}"
    );

    for _ in QuickRow::General.position(&[])..QuickRow::System.position(&[]) {
        tap(&mut f, &mut input, Btn::Up);
    }
    tap(&mut f, &mut input, Btn::A);
    let px = composed(&mut f, &mut c, "shelf-page");
    let hidden = &f.app().hidden_rows();
    for row in QuickRow::Theme
        .shown(hidden)
        .into_iter()
        .take(QUICK_VISIBLE)
    {
        let top = quick_row_top_at(slot_ui::Canvas::FRAMED, row, QuickRow::Theme, hidden) as usize;
        let first = *inked(&px, 40..360, top)
            .first()
            .unwrap_or_else(|| panic!("{row:?} has no label"));
        assert!(
            (53..=57).contains(&first),
            "{row:?}'s label starts at x {first}"
        );
    }
    for _ in 0..QuickRow::DateTime.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    let px = composed(&mut f, &mut c, "clock");
    let strip = (OUT_H as f32 - MOUTH_H) as usize..OUT_H as usize;
    assert!(
        strip
            .flat_map(|y| (0..130).map(move |x| (x, y)))
            .any(|(x, y)| at(&px, x, y).iter().all(|&c| c > 0xd0)),
        "the clock from the menu does not offer B BACK in the footer"
    );
}

#[test]
fn every_fast_forward_speed_sits_on_the_rows_right_edge_and_clears_the_label() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Game.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    tap(&mut f, &mut input, Btn::Left);
    tap(&mut f, &mut input, Btn::Left);

    let top = quick_row_top(
        slot_ui::Canvas::FRAMED,
        QuickRow::FastForward,
        &f.app().hidden_rows(),
    ) as usize;
    for name in ["2x", "3x", "4x", "6x", "8x"] {
        let px = composed(&mut f, &mut c, name);
        let value = inked(&px, 360..OUT_W as usize, top);
        let last = *value
            .last()
            .unwrap_or_else(|| panic!("{name}: the Fast Forward row has no value"));
        assert!(
            (679..=688).contains(&last),
            "{name} ends at x {last}, off the edge every other value keeps"
        );
        assert!(
            inked(&px, 350..370, top).is_empty(),
            "{name} reaches the middle of the row"
        );
        let label = inked(&px, 40..350, top);
        let first = *label
            .first()
            .unwrap_or_else(|| panic!("{name}: the Fast Forward row has no label"));
        assert!(
            (53..=57).contains(&first),
            "the label moved to x {first} to make room for {name}"
        );
        tap(&mut f, &mut input, Btn::Right);
    }
}

#[test]
fn the_game_boy_page_shows_the_palette_by_name() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Screen.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    for _ in 0..QuickRow::ScreenGb.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    let hidden = [
        QuickRow::GbModel,
        QuickRow::GbaOverlay,
        QuickRow::GbOverlay,
        QuickRow::GbcOverlay,
        QuickRow::GbReflection,
    ];
    for _ in 0..QuickRow::GbPalette.position(&hidden) {
        tap(&mut f, &mut input, Btn::Down);
    }
    let px = composed(&mut f, &mut c, "screen-page");

    let top = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::GbPalette, &hidden) as usize;
    beside("Palette", pointer(&px, top));
    for row in QuickRow::SCREEN_GB
        .into_iter()
        .filter(|r| !hidden.contains(r))
    {
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &hidden) as usize;
        if row != QuickRow::GbPalette {
            assert!(pointer(&px, top).is_none(), "a pointer at {row:?} as well");
        }
        let label = inked(&px, 40..360, top);
        let first = *label.first().expect("a row with no label");
        assert!(
            (53..=57).contains(&first),
            "{row:?}'s label starts at x {first}"
        );
        if row.opens() {
            continue;
        }
        let value = inked(&px, 360..OUT_W as usize, top);
        let last = *value.last().expect("a row with no value");
        assert!(
            (679..=688).contains(&last),
            "{row:?}'s value ends at x {last}"
        );
    }
}

#[test]
fn the_about_label_is_on_the_tinted_wallpaper() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::System.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    for _ in 0..QuickRow::About.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    let menu = composed(&mut f, &mut c, "about-row");
    tap(&mut f, &mut input, Btn::A);
    let about = composed(&mut f, &mut c, "about-label");
    for (x, y) in [(4, 4), (OUT_W as usize - 4, 4), (4, OUT_H as usize - 4)] {
        assert_eq!(
            at(&about, x, y),
            at(&menu, x, y),
            "the About label's ground at {x},{y} is not the menu's tinted ground"
        );
    }
}

#[test]
fn the_general_page_renders() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::General.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    composed(&mut f, &mut c, "main");
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Theme));
    composed(&mut f, &mut c, "general");
    for _ in 0..QuickRow::KidMode.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    assert_eq!(f.app().quick_menu(), Some(QuickRow::KidMode));
    composed(&mut f, &mut c, "general-bottom");
}

#[test]
fn the_general_screen_page_renders() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Screen.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ScreenGeneral));
    composed(&mut f, &mut c, "screen-pages");
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Brightness));
    let px = composed(&mut f, &mut c, "screen-general");
    beside(
        "Brightness",
        pointer(
            &px,
            quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::Brightness, &[]) as usize,
        ),
    );
    for row in QuickRow::SCREEN_GENERAL {
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &[]) as usize;
        let label = inked(&px, 40..360, top);
        let first = *label.first().expect("a row with no label");
        assert!(
            (53..=57).contains(&first),
            "{row:?}'s label starts at x {first}"
        );
        let value = inked(&px, 360..OUT_W as usize, top);
        let last = *value.last().expect("a row with no value");
        assert!(
            (679..=688).contains(&last),
            "{row:?}'s value ends at x {last}"
        );
    }
}

#[test]
fn the_controls_pages_render() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ControlsShelf));
    composed(&mut f, &mut c, "controls");
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ShelfPrev));
    composed(&mut f, &mut c, "controls-shelf");
    tap(&mut f, &mut input, Btn::B);
    tap(&mut f, &mut input, Btn::Up);
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::GameUp));
    composed(&mut f, &mut c, "controls-in-game");
    tap(&mut f, &mut input, Btn::Y);
    assert!(!f.app().reset_prompt());
    tap(&mut f, &mut input, Btn::X);
    composed(&mut f, &mut c, "controls-in-game-changed");
    tap(&mut f, &mut input, Btn::Y);
    assert!(f.app().reset_prompt());
    composed(&mut f, &mut c, "controls-reset");
    tap(&mut f, &mut input, Btn::B);
    tap(&mut f, &mut input, Btn::B);
    tap(&mut f, &mut input, Btn::Up);
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ScSave));
    composed(&mut f, &mut c, "controls-shortcuts");
    tap(&mut f, &mut input, Btn::X);
    tap(&mut f, &mut input, Btn::Y);
    composed(&mut f, &mut c, "controls-shortcuts-reset");
    tap(&mut f, &mut input, Btn::B);
}

#[test]
fn the_shortcuts_page_renders() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ScSave));
    let px = composed(&mut f, &mut c, "shortcuts");
    let bar = quick_scroll(
        slot_ui::Canvas::FRAMED,
        QuickRow::ScSave,
        &f.app().hidden_rows(),
    )
    .expect("the page scrolls");
    let x = OUT_W as usize - 14;
    let lum = |y: f32| {
        at(&px, x, y as usize)
            .iter()
            .map(|&c| u32::from(c))
            .sum::<u32>()
    };
    let thumb = lum(bar.thumb_y + bar.thumb_h / 2.0);
    let track = lum(bar.thumb_y + bar.thumb_h + 20.0);
    let clear = lum(bar.y - 20.0);
    assert!(
        thumb > track && track > clear,
        "no scroll bar at the right edge: thumb {thumb}, track {track}, above it {clear}"
    );
}

#[test]
fn the_left_stick_row_keeps_its_label_clear_of_its_value() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.set_left_stick(true);
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Game.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    let hidden = f.app().hidden_rows();
    for _ in 0..QuickRow::LeftStick.position(&hidden) {
        tap(&mut f, &mut input, Btn::Down);
    }
    assert_eq!(f.app().quick_menu(), Some(QuickRow::LeftStick));
    let top = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::LeftStick, &hidden) as usize;
    let px = composed(&mut f, &mut c, "left-stick");
    let ink = inked(&px, 40..OUT_W as usize, top);
    assert!(!ink.is_empty(), "the Left Stick row drew nothing");
    let gap = ink.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    assert!(
        gap >= 24,
        "the label runs into the value, widest gap {gap} px"
    );
}

#[test]
fn resume_on_power_on_stays_clear_of_its_value() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Game.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    let hidden = f.app().hidden_rows();
    for _ in 0..QuickRow::ResumePowerOn.position(&hidden) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::Right);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::ResumePowerOn));
    assert_eq!(
        f.app().quick_value(QuickRow::ResumePowerOn),
        Some(slot_ui::QuickValue::Off)
    );
    let top = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::ResumePowerOn, &hidden) as usize;
    let px = composed(&mut f, &mut c, "power-on");
    let ink = inked(&px, 40..OUT_W as usize, top);
    assert!(!ink.is_empty(), "the Resume on Power On row drew nothing");
    let gap = ink.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
    assert!(
        gap >= 24,
        "the label runs into the value, widest gap {gap} px"
    );
}

#[test]
fn a_duplicate_binding_shows_a_warning_naming_the_other_row() {
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
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Controls.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    for _ in 0..QuickRow::Shortcuts.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    tap(&mut f, &mut input, Btn::A);
    assert!(f.app().capturing());
    tap(&mut f, &mut input, Btn::A);
    assert!(!f.app().capturing());
    assert_eq!(
        f.app().toast(),
        Some(slot_ui::Toast::Duplicate(QuickRow::GameA))
    );
    let px = composed(&mut f, &mut c, "duplicate-toast");
    let (x, y, w, h) =
        slot_ui::toast_rect(f.app().canvas(), slot_ui::Toast::Duplicate(QuickRow::GameA));
    let ink: Vec<usize> = (x as usize..(x + w) as usize)
        .filter(|&px_x| {
            (y as usize..(y + h) as usize).any(|py| at(&px, px_x, py).iter().all(|c| *c > 0xc0))
        })
        .collect();
    assert!(
        ink.len() > 100,
        "the toast has almost no ink: {}",
        ink.len()
    );
    let gaps: Vec<usize> = ink
        .windows(2)
        .filter(|p| p[1] - p[0] > 4)
        .map(|p| p[1] - p[0])
        .collect();
    assert!(
        !gaps.is_empty() && ink[0] < (x + w / 2.0) as usize - 100,
        "no icon set apart ahead of the words: first ink {} gaps {gaps:?}",
        ink[0]
    );
    let amber = |p: [u8; 3]| p[0] > 0xd0 && (0x80..0xd0).contains(&p[1]) && p[2] < 0x70;
    let amber_in = |xs: std::ops::Range<usize>, ys: std::ops::Range<usize>| {
        ys.flat_map(|py| xs.clone().map(move |px_x| (px_x, py)))
            .filter(|&(px_x, py)| amber(at(&px, px_x, py)))
            .count()
    };
    assert!(
        amber_in(x as usize..(x + w) as usize, y as usize..(y + h) as usize) > 20,
        "the warning triangle is not amber"
    );
    let width = f.app().canvas().w as usize;
    assert!(
        amber_in(width / 2..width, (y + h) as usize + 20..400) > 40,
        "no amber triangle marks the clashing rows"
    );
}

#[derive(Clone, Default)]
struct Answers(std::sync::Arc<std::sync::Mutex<VecDeque<slot::update::UpdateEvent>>>);

impl slot::update::UpdateJobs for Answers {
    fn ask(&mut self, _job: slot::update::UpdateJob) {}

    fn take(&mut self) -> Option<slot::update::UpdateEvent> {
        self.0.lock().unwrap().pop_front()
    }
    fn cancel(&mut self) -> bool {
        true
    }
}

fn open_network(f: &mut Frontend, input: &mut Script) {
    tap(f, input, Btn::Menu);
    for _ in 0..QuickRow::Network.position(&[]) {
        tap(f, input, Btn::Down);
    }
    tap(f, input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Server));
}

fn open_system(f: &mut Frontend, input: &mut Script) {
    tap(f, input, Btn::Menu);
    for _ in 0..QuickRow::System.position(&[]) {
        tap(f, input, Btn::Down);
    }
    tap(f, input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Wifi));
}

fn open_wifi(f: &mut Frontend, input: &mut Script) {
    open_system(f, input);
    for _ in 0..QuickRow::Wifi.position(&f.app().hidden_rows()) {
        tap(f, input, Btn::Down);
    }
    tap(f, input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::WifiPower));
}

fn open_check_for_updates(f: &mut Frontend, input: &mut Script) {
    open_system(f, input);
    for _ in 0..QuickRow::CheckUpdates.position(&f.app().hidden_rows()) {
        tap(f, input, Btn::Down);
    }
    tap(f, input, Btn::A);
}

#[test]
fn the_tools_page_renders_centred() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    open_network(&mut f, &mut input);
    let px = composed(&mut f, &mut c, "tools");
    let hidden = f.app().hidden_rows();
    let spans: Vec<(usize, usize)> = QuickRow::NETWORK
        .into_iter()
        .filter(|r| !hidden.contains(r) && *r != QuickRow::Romm)
        .map(|row| {
            let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &hidden) as usize;
            let from = pointer(&px, top).map_or(170, |p| p.label_first);
            let ink = inked(&px, from..OUT_W as usize - 40, top);
            (
                *ink.first().expect("a row with no label"),
                *ink.last().unwrap(),
            )
        })
        .collect();
    let left = spans.iter().map(|s| s.0).min().unwrap();
    let right = spans.iter().map(|s| s.1).max().unwrap();
    for (first, _) in &spans {
        assert!(first - left <= 3, "a label starts at x {first}, not {left}");
    }
    let off = (left + right) as f32 / 2.0 - OUT_W as f32 / 2.0;
    assert!(
        off.abs() <= 4.0,
        "the column sits {off} px off centre, spans {spans:?}, canvas {:?}",
        f.app().canvas()
    );
}

fn ink_at(px: &[u8], xs: std::ops::Range<usize>, top: usize) -> bool {
    !inked(px, xs, top).is_empty()
}

#[test]
fn the_wifi_page_renders() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    open_wifi(&mut f, &mut input);
    tap(&mut f, &mut input, Btn::Down);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::Networks));
    let px = composed(&mut f, &mut c, "network-page");
    let right = OUT_W as usize - 32;
    for row in QuickRow::WIFI {
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &[]) as usize;
        let first = *inked(&px, 40..360, top)
            .first()
            .unwrap_or_else(|| panic!("{row:?} has no label"));
        assert!(
            (53..=57).contains(&first),
            "{row:?}'s label starts at x {first}"
        );
        let valued = matches!(row, QuickRow::WifiPower | QuickRow::Networks);
        assert_eq!(
            ink_at(&px, right - 30..right, top),
            valued,
            "{row:?}: a value at the right edge is {}",
            if valued { "missing" } else { "drawn" }
        );
    }
    assert_eq!(f.app().networks_value(), "Home");
    let networks = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::Networks, &[]) as usize;
    let span = inked(&px, 360..right + 4, networks);
    assert!(
        span.last().is_some_and(|x| x.abs_diff(right) <= 3),
        "Home does not end at the right edge: {span:?}"
    );
    assert!(
        span.first().is_some_and(|x| *x < right - 60),
        "Home is too short to be the word: {span:?}"
    );
    tap(&mut f, &mut input, Btn::Up);
    tap(&mut f, &mut input, Btn::A);
    assert!(f.app().wifi_is_off());
    let off = composed(&mut f, &mut c, "network-page-off");
    let hidden = f.app().hidden_rows();
    assert_eq!(QuickRow::WifiPower.shown(&hidden), [QuickRow::WifiPower]);
    let wifi = quick_row_top(slot_ui::Canvas::FRAMED, QuickRow::WifiPower, &[]) as usize;
    assert!(
        !inked(&off, 40..360, wifi).is_empty(),
        "the Wi-Fi row moved when Wi-Fi went off"
    );
    {
        let row = QuickRow::Networks;
        let top = quick_row_top(slot_ui::Canvas::FRAMED, row, &[]) as usize;
        assert!(
            inked(&off, 40..right + 4, top).is_empty(),
            "{row:?} is still drawn with Wi-Fi off"
        );
    }
}

#[test]
fn about_shows_x_restart() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::System.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    for _ in 0..QuickRow::About.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    let px = composed(&mut f, &mut c, "about-restart");
    let w = f.app().canvas().w as usize;
    let footer = (OUT_H as f32 - MOUTH_H) as usize..OUT_H as usize;
    let bright = |xs: std::ops::Range<usize>| {
        footer
            .clone()
            .flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| at(&px, x, y).iter().all(|&c| c > 0xd0))
            .count()
    };
    assert!(bright(0..130) > 40, "no B BACK on the left");
    assert!(bright(w - 170..w) > 40, "no RESTART X on the right");
    tap(&mut f, &mut input, Btn::X);
    assert!(f.app().restarting(), "X on About did not restart");
}

fn wait(f: &mut Frontend, input: &mut Script, ms: u64) {
    let mut now = f.app().now();
    for _ in 0..ms / 50 {
        now += 50;
        f.advance_at(input, now, 0.05);
    }
}

fn slot_release() -> slot::update::Release {
    slot::update::Release {
        kind: slot::update::Kind::Slot,
        running: "1.12.0".into(),
        version: "1.13.0".into(),
        url: String::new(),
        sha256: [0; 32],
        size: 1000,
    }
}

fn baseos_release() -> slot::update::Release {
    slot::update::Release {
        kind: slot::update::Kind::BaseOs,
        running: "1.3.0".into(),
        version: "1.4.0".into(),
        url: String::new(),
        sha256: [0; 32],
        size: 1000,
    }
}

#[test]
fn the_baseos_update_scene_renders_beside_slot() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    let answers = Answers::default();
    f.set_update_jobs(Box::new(answers.clone()));
    let say = |e| answers.0.lock().unwrap().push_back(e);
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    open_check_for_updates(&mut f, &mut input);
    say(slot::update::UpdateEvent::Available(vec![
        slot_release(),
        baseos_release(),
    ]));
    wait(&mut f, &mut input, 800);
    composed(&mut f, &mut c, "update-both-slot");
    tap(&mut f, &mut input, Btn::Right);
    wait(&mut f, &mut input, 700);
    composed(&mut f, &mut c, "update-both-baseos");
    tap(&mut f, &mut input, Btn::A);
    say(slot::update::UpdateEvent::Progress(470));
    wait(&mut f, &mut input, 1900);
    assert_eq!(f.app().update_progress(), Some(0.47));
    let downloading = composed(&mut f, &mut c, "update-baseos-downloading");
    let [r, g, b] = at(&downloading, 196, 250);
    assert!(
        r < 60 && g < 60 && b < 60,
        "the open cart's back is not black: {r},{g},{b}"
    );
    say(slot::update::UpdateEvent::Staged);
    wait(&mut f, &mut input, 1300);
    composed(&mut f, &mut c, "update-baseos-seated");
    wait(&mut f, &mut input, 1000);
    assert!(f.app().restarting(), "a BaseOS update did not reboot");
}

#[test]
fn the_update_scene_renders_each_moment() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    let answers = Answers::default();
    f.set_update_jobs(Box::new(answers.clone()));
    let say = |e| answers.0.lock().unwrap().push_back(e);
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    open_system(&mut f, &mut input);
    for _ in 0..QuickRow::CheckUpdates.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    assert!(f.app().update_offered());
    let row = composed(&mut f, &mut c, "network-updates-row");
    tap(&mut f, &mut input, Btn::A);
    wait(&mut f, &mut input, 400);
    let checking = composed(&mut f, &mut c, "update-checking");
    assert_ne!(checking, row, "checking drew nothing new");
    say(slot::update::UpdateEvent::Available(vec![slot_release()]));
    wait(&mut f, &mut input, 800);
    composed(&mut f, &mut c, "update-available");
    tap(&mut f, &mut input, Btn::A);
    say(slot::update::UpdateEvent::Progress(470));
    wait(&mut f, &mut input, 250);
    composed(&mut f, &mut c, "update-opening-cart");
    wait(&mut f, &mut input, 1250);
    composed(&mut f, &mut c, "update-hopping");
    wait(&mut f, &mut input, 400);
    assert_eq!(f.app().update_progress(), Some(0.47));
    let downloading = composed(&mut f, &mut c, "update-downloading");
    let [r, g, b] = at(&downloading, 196, 250);
    assert!(
        b > g + 30 && r > g,
        "the open cart's back is not the shell's purple: {r},{g},{b}"
    );
    say(slot::update::UpdateEvent::Staged);
    wait(&mut f, &mut input, 150);
    composed(&mut f, &mut c, "update-closing");
    wait(&mut f, &mut input, 450);
    composed(&mut f, &mut c, "update-seating");
    wait(&mut f, &mut input, 300);
    assert!(!f.app().restarting(), "restarted before the cart seated");
    composed(&mut f, &mut c, "update-seated");
    wait(&mut f, &mut input, 250);
    composed(&mut f, &mut c, "update-opening");
    wait(&mut f, &mut input, 150);
    assert!(!f.app().relaunching(), "relaunched before the logo held");
    composed(&mut f, &mut c, "update-logo");
}

#[test]
fn a_cancelled_download_puts_the_carts_back_without_dropping_them() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    common::connect(f.app_mut());
    let answers = Answers::default();
    f.set_update_jobs(Box::new(answers.clone()));
    let say = |e| answers.0.lock().unwrap().push_back(e);
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    open_check_for_updates(&mut f, &mut input);
    say(slot::update::UpdateEvent::Available(vec![
        slot_release(),
        baseos_release(),
    ]));
    wait(&mut f, &mut input, 800);
    tap(&mut f, &mut input, Btn::Right);
    tap(&mut f, &mut input, Btn::A);
    wait(&mut f, &mut input, 1200);
    tap(&mut f, &mut input, Btn::B);
    say(slot::update::UpdateEvent::Cancelled);
    let mut lowest = f32::MAX;
    for _ in 0..30 {
        wait(&mut f, &mut input, 50);
        f.compose(&mut c);
        if f.app().update_headline() {
            let mut out = Vec::new();
            f.app().draw(&mut out);
            for d in &out {
                if let slot_gfx::Draw::Turned { y, .. } = d {
                    lowest = lowest.min(*y);
                }
            }
        }
    }
    composed(&mut f, &mut c, "update-cancelled");
    assert!(f.app().update_headline(), "the carts never came back");
    assert!(
        lowest < f32::MAX && lowest > 100.0,
        "a cart rose to y {lowest} over the header"
    );
}

#[test]
fn the_tools_page_opens_offline_and_its_rows_ask_for_wifi() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::Menu);
    for _ in 0..QuickRow::Network.position(&[]) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert!(!f.app().wifi_up());
    assert!(f
        .app()
        .quick_menu()
        .is_some_and(|r| r.page() == QuickRow::NETWORK));
    assert!(!f.app().update_shown());
    for _ in 0..QuickRow::FetchArt.position(&f.app().hidden_rows()) {
        tap(&mut f, &mut input, Btn::Down);
    }
    tap(&mut f, &mut input, Btn::A);
    assert_eq!(f.app().quick_menu(), Some(QuickRow::FetchArt));
    assert!(f.app().update_shown());
    composed(&mut f, &mut c, "tools-offline");
}

fn footer_cols(px: &[u8]) -> Vec<usize> {
    let top = slot_ui::FOOTER_Y as usize;
    (0..OUT_W as usize / 2)
        .filter(|&x| {
            (top..top + slot_ui::HINT_H as usize).any(|y| at(px, x, y).iter().all(|c| *c > 0xc0))
        })
        .collect()
}

#[test]
fn the_shelf_footer_shows_what_battery_display_asks_for() {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let mut cols = Vec::new();
    for display in slot_store::BatteryDisplay::ALL {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        let mut s = slot_store::read_slot_state(d.path());
        s.battery_display = display;
        slot_store::write_slot_state(d.path(), &s).expect("write slot.state");
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(&mut c);
        let mut input = Script(VecDeque::new());
        f.advance(&mut input);
        let px = composed(&mut f, &mut c, &format!("battery-{}", display.key()));
        cols.push(footer_cols(&px));
    }
    let [both, icon, percent] = [&cols[0], &cols[1], &cols[2]];
    let gauge_x = 40;
    let bolt: Vec<usize> = both.iter().copied().filter(|&x| x < gauge_x).collect();
    assert!(
        !bolt.is_empty(),
        "the sim battery is not charging, so the bolt test is moot"
    );
    let capsule: Vec<usize> = icon.iter().copied().filter(|&x| x >= gauge_x).collect();
    let number: Vec<usize> = both.iter().copied().filter(|x| !icon.contains(x)).collect();
    assert!(!capsule.is_empty(), "Icon drew no capsule");
    assert!(!number.is_empty(), "Both drew no number");
    assert!(
        icon.iter().all(|x| both.contains(x)),
        "Icon drew something Both does not"
    );
    assert!(
        capsule.last() < number.first(),
        "the number does not sit after the capsule: {capsule:?} {number:?}"
    );
    let text: Vec<usize> = percent.iter().copied().filter(|&x| x >= gauge_x).collect();
    assert_eq!(
        percent
            .iter()
            .copied()
            .filter(|&x| x < gauge_x)
            .collect::<Vec<_>>(),
        bolt,
        "the bolt moved"
    );
    assert!(
        text[0].abs_diff(capsule[0]) <= 1,
        "Percentage starts at {} and the capsule at {}",
        text[0],
        capsule[0]
    );
    let span = |c: &[usize]| c[c.len() - 1] - c[0];
    assert!(
        span(&text).abs_diff(span(&number)) <= 1,
        "Percentage prints {text:?}, Both printed {number:?}"
    );
}
