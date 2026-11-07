mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{repo_root, session_with_platform, tmp_root_with_gb_carts};
use slot::app::Phase;
use slot::session::Session;
use slot_input::{Btn, Millis, RawEvent};
use slot_store::{write_selected_core, write_slot_state, Core, Platform, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

const FRAME_MS: Millis = 16;
const DT: f32 = 1.0 / 60.0;

fn step(s: &mut Session, now: &mut Millis) {
    *now += FRAME_MS;
    s.feed([], *now);
    s.update(DT);
}

fn event(s: &mut Session, ev: RawEvent, now: &mut Millis) {
    *now += FRAME_MS;
    s.feed([ev], *now);
    s.update(DT);
}

fn play(s: &mut Session, now: &mut Millis) {
    event(s, RawEvent::Down(Btn::A), now);
    event(s, RawEvent::Up(Btn::A), now);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(s, now);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn catrap(root: &Path) -> Option<()> {
    let real = repo_root().join("sdcard/Games/GB/Catrap (USA).gb");
    let rom = std::fs::read(&real).ok()?;
    std::fs::write(root.join("Games/GB/Catrap.gb"), rom).expect("write rom");
    Some(())
}

fn root_with_catrap() -> Option<TempDir> {
    let d = tmp_root_with_gb_carts(&["Catrap"]);
    catrap(d.path())?;
    Some(d)
}

fn mean_saturation(frame: &[u8]) -> f64 {
    let sum: f64 = frame
        .chunks_exact(4)
        .map(|p| {
            let (hi, lo) = (p[..3].iter().max(), p[..3].iter().min());
            f64::from(hi.copied().unwrap_or(0) - lo.copied().unwrap_or(0))
        })
        .sum();
    sum / (frame.len() / 4) as f64
}

fn until(s: &mut Session, now: &mut Millis, what: &str, cond: impl Fn(&[u8]) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if s.frame().is_some_and(|f| cond(&f)) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        step(s, now);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_chord_recolours_the_running_game() {
    let Some(dylib) = common::vendored_core() else {
        eprintln!("no mgba dylib, skipping");
        return;
    };
    std::env::set_var("SLOT_CORE", dylib);
    let _g = common::core_lock();
    let Some(d) = root_with_catrap() else {
        eprintln!("no Catrap on this machine's card, skipping");
        return;
    };
    write_selected_core(d.path(), Platform::Gb, "Catrap", Core::Mgba).expect("write core");
    write_slot_state(
        d.path(),
        &SlotState {
            gb_palettes: false,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let (mut s, _motor) = session_with_platform(d.path());
    let mut now = 0;
    play(&mut s, &mut now);
    until(&mut s, &mut now, "a title screen in Auto's colours", |f| {
        mean_saturation(f) > 6.0
    });

    for ev in [
        RawEvent::Down(Btn::Select),
        RawEvent::Down(Btn::R2),
        RawEvent::Up(Btn::R2),
        RawEvent::Up(Btn::Select),
    ] {
        event(&mut s, ev, &mut now);
    }
    assert_eq!(Some(s.app().gb_colors().core_name()), Some("Grayscale"));
    until(&mut s, &mut now, "the picture to turn to Grayscale", |f| {
        mean_saturation(f) < 4.0
    });
}

#[test]
fn the_chord_recolours_the_running_game_on_gambatte() {
    let dylib = repo_root().join(format!(
        "vendor/gambatte_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    if !dylib.exists() {
        eprintln!("no gambatte dylib, skipping");
        return;
    }
    std::env::set_var("SLOT_CORE", dylib);
    let _g = common::core_lock();
    let Some(d) = root_with_catrap() else {
        eprintln!("no Catrap on this machine's card, skipping");
        return;
    };
    write_selected_core(d.path(), Platform::Gb, "Catrap", Core::Gambatte).expect("write core");
    write_slot_state(
        d.path(),
        &SlotState {
            gb_palettes: false,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let (mut s, _motor) = session_with_platform(d.path());
    let mut now = 0;
    play(&mut s, &mut now);
    assert_eq!(s.app().core(), Core::Gambatte);
    until(&mut s, &mut now, "a title screen in Auto's colours", |f| {
        mean_saturation(f) > 6.0
    });

    for ev in [
        RawEvent::Down(Btn::Select),
        RawEvent::Down(Btn::R2),
        RawEvent::Up(Btn::R2),
        RawEvent::Up(Btn::Select),
    ] {
        event(&mut s, ev, &mut now);
    }
    assert_eq!(Some(s.app().gb_colors().core_name()), Some("Grayscale"));
    until(&mut s, &mut now, "the picture to turn to Grayscale", |f| {
        mean_saturation(f) < 4.0
    });
}

fn tap(s: &mut Session, now: &mut Millis, btn: Btn) {
    event(s, RawEvent::Down(btn), now);
    event(s, RawEvent::Up(btn), now);
}

#[test]
fn a_palette_picked_behind_the_menu_outlasts_the_menu() {
    let Some(dylib) = common::vendored_core() else {
        eprintln!("no mgba dylib, skipping");
        return;
    };
    std::env::set_var("SLOT_CORE", dylib);
    let _g = common::core_lock();
    let Some(d) = root_with_catrap() else {
        eprintln!("no Catrap on this machine's card, skipping");
        return;
    };
    write_selected_core(d.path(), Platform::Gb, "Catrap", Core::Mgba).expect("write core");
    write_slot_state(
        d.path(),
        &SlotState {
            gb_palettes: false,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let (mut s, _motor) = session_with_platform(d.path());
    let mut now = 0;
    play(&mut s, &mut now);
    until(&mut s, &mut now, "a title screen in Auto's colours", |f| {
        mean_saturation(f) > 6.0
    });

    tap(&mut s, &mut now, Btn::Menu);
    for btn in [Btn::Down, Btn::A, Btn::A] {
        tap(&mut s, &mut now, btn);
    }
    for _ in 0..QuickRow::Palette.position(&s.app().in_game_hidden()) {
        tap(&mut s, &mut now, Btn::Down);
    }
    assert_eq!(s.app().in_game_menu(), Some(QuickRow::Palette));
    tap(&mut s, &mut now, Btn::Right);
    assert_eq!(Some(s.app().gb_colors().core_name()), Some("Grayscale"));
    while s.app().in_game_menu().is_some() {
        tap(&mut s, &mut now, Btn::B);
    }
    for _ in 0..120 {
        step(&mut s, &mut now);
        std::thread::sleep(Duration::from_millis(1));
    }
    let f = s.frame().expect("no frame");
    assert!(
        mean_saturation(&f) < 4.0,
        "the game went back to Auto once the menu closed, saturation {}",
        mean_saturation(&f)
    );
}
