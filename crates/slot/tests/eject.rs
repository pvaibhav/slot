mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{app_playing_in, boot, tmp_root_with_carts, StubSnapshot};
use slot::app::Phase;
use slot::emu::Speed;
use slot::persist::eject;
use slot_input::Action;
use slot_store::{read_slot_state, write_slot_state, Core, Platform, SlotState, StateRing};
use slot_ui::{Draw, CART_W, OUT_H, OUT_W};

fn seated(cart: &str) -> SlotState {
    SlotState {
        cart: Some(cart.into()),
        clock_set: true,
        ..Default::default()
    }
}

fn set_mode(dir: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).expect("chmod");
}

#[test]
fn eject_clears_the_slot_only_after_the_state_is_durable() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            ..Default::default()
        },
    )
    .unwrap();
    eject(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[9u8; 1024]),
        Some(b"savdata"),
        &read_slot_state(d.path()),
    )
    .unwrap();
    assert_eq!(read_slot_state(d.path()).cart, None);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert_eq!(r.read_resume().unwrap().unwrap().len(), 1024);
    assert_eq!(
        std::fs::read(d.path().join("Saves/GBA/Emerald.sav")).unwrap(),
        b"savdata"
    );
    assert!(
        r.list().unwrap().is_empty(),
        "eject must not create a polaroid"
    );
}

#[test]
fn a_resume_that_cannot_be_written_leaves_the_cart_in_the_slot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(d.path(), &seated("Emerald")).unwrap();
    std::fs::create_dir_all(d.path().join("States/GBA")).unwrap();
    std::fs::write(d.path().join("States/GBA/mgba"), b"in the way").unwrap();
    assert!(eject(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[9u8; 1024]),
        None,
        &read_slot_state(d.path()),
    )
    .is_err());
    assert_eq!(read_slot_state(d.path()).cart, Some("Emerald".into()));
}

#[test]
fn an_unchanged_battery_save_is_not_rewritten() {
    let d = tmp_root_with_carts(&["Emerald"]);
    std::fs::create_dir_all(d.path().join("Saves/GBA")).unwrap();
    std::fs::write(d.path().join("Saves/GBA/Emerald.sav"), b"savdata").unwrap();
    let saves = d.path().join("Saves/GBA");
    set_mode(&saves, 0o555);
    let unchanged = eject(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[0u8; 8]),
        Some(b"savdata"),
        &read_slot_state(d.path()),
    );
    let changed = eject(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[0u8; 8]),
        Some(b"changed"),
        &read_slot_state(d.path()),
    );
    set_mode(&saves, 0o755);
    unchanged.expect("identical bytes must not touch the card");
    assert!(
        changed.is_err(),
        "the directory stayed writable, so the first half proved nothing"
    );
}

#[test]
fn eject_preserves_the_levels() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            brightness: 2,
            blue_light: 7,
            volume: 35,
            muted: true,
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    eject(
        d.path(),
        Platform::Gba,
        Core::Mgba,
        "Emerald",
        Some(&[0u8; 8]),
        None,
        &read_slot_state(d.path()),
    )
    .unwrap();
    let s = read_slot_state(d.path());
    assert_eq!((s.brightness, s.blue_light, s.volume), (2, 7, 35));
    assert!(s.muted, "the cart came out and the sound came back");
}

#[test]
fn ejecting_a_playing_cart_is_durable_before_the_cart_is_out() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));
    for _ in 0..600 {
        if !matches!(a.phase(), Phase::Ejecting { .. }) {
            break;
        }
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    a.settle_saves();
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert_eq!(
        r.read_resume().unwrap().expect("nothing was flushed").len(),
        1024
    );
    assert_eq!(read_slot_state(d.path()).cart, None);
}

#[test]
fn a_refused_cart_writes_no_resume() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(d.path(), &seated("Emerald")).unwrap();
    let mut a = boot(d.path());
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_failed();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none()
    );
    assert_eq!(read_slot_state(d.path()).cart, None);
}

#[test]
fn a_cart_can_be_ejected_before_its_core_is_ready() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::Insert);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    a.apply(Action::Eject);
    assert!(
        matches!(a.phase(), Phase::Ejecting { .. }),
        "a cart with no core behind it cannot be got out"
    );
}

#[test]
fn the_row_closes_back_up_as_the_cart_comes_out() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Metroid"]);
    let mut a = boot(d.path());
    a.apply(Action::Insert);
    for _ in 0..90 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::Eject);
    assert!(matches!(a.phase(), Phase::Ejecting { .. }));

    let carts = |a: &slot::app::App| {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.iter()
            .filter(|d| matches!(d, Draw::Rect { w, .. } | Draw::Tex { w, .. } if *w < CART_W as f32 + 1.0 && *w > 1.0))
            .count()
    };
    let early = carts(&a);
    for _ in 0..50 {
        a.update(1.0 / 60.0);
    }
    let late = carts(&a);
    assert!(
        late > early,
        "the row went from {early} carts to {late}: it is not coming back"
    );
}

#[test]
fn the_eject_is_the_insert_run_backwards() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Metroid"]);
    let mut a = boot(d.path());
    let dt = 1.0 / 60.0;

    let steps = |a: &mut slot::app::App, done: fn(f32) -> bool| {
        let mut out = Vec::new();
        let mut last = a.seat();
        while !done(a.seat()) {
            a.update(dt);
            out.push((a.seat() - last).abs());
            last = a.seat();
        }
        out
    };

    a.apply(Action::Insert);
    let going_in = steps(&mut a, |s| s >= 1.0);
    for _ in 0..120 {
        a.update(dt);
    }

    a.apply(Action::Eject);
    while a.seat() >= 1.0 {
        a.update(dt);
    }
    let coming_out = steps(&mut a, |s| s <= 0.0);

    assert!(
        going_in.len() > 10,
        "the insert took {} frames",
        going_in.len()
    );
    assert!(
        going_in.len().abs_diff(coming_out.len()) <= 1,
        "in over {} frames, out over {}",
        going_in.len(),
        coming_out.len()
    );
    let n = going_in.len().min(coming_out.len()) - 1;
    for i in 1..n {
        let (a_in, a_out) = (going_in[i], coming_out[coming_out.len() - 1 - i]);
        assert!(
            (a_in - a_out).abs() < 0.01,
            "frame {i}: moved {a_in} going in against {a_out} coming out"
        );
    }
}

#[test]
fn the_veil_never_falls_again_on_the_way_out() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion", "Metroid"]);
    let mut a = boot(d.path());
    let dt = 1.0 / 60.0;
    a.apply(Action::Insert);
    for _ in 0..120 {
        a.update(dt);
    }
    a.apply(Action::Eject);

    let veil = |a: &slot::app::App| {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.iter()
            .find_map(|d| match *d {
                Draw::Rect { w, h, colour, .. }
                    if w >= OUT_W as f32 && h >= OUT_H as f32 && colour[..3] == [0.0; 3] =>
                {
                    Some(colour[3])
                }
                _ => None,
            })
            .unwrap_or(0.0)
    };
    while a.seat() >= 1.0 {
        a.update(dt);
    }
    let first = veil(&a);
    for _ in 0..12 {
        a.update(dt);
    }
    let later = veil(&a);
    assert!(
        later <= first,
        "the veil went from {first} to {later}: it is darkening on the way out"
    );
}

#[test]
fn the_core_stops_before_the_cart_moves() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    common::clocked(d.path());
    let mut s = slot::session::Session::boot(d.path().to_path_buf());
    s.app_mut().apply(Action::Insert);
    for _ in 0..90 {
        s.update(1.0 / 60.0);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while s.emu().is_none() {
        assert!(Instant::now() < deadline, "no core to stop");
        std::thread::sleep(Duration::from_millis(1));
        s.update(1.0 / 60.0);
    }

    s.app_mut().apply(Action::Eject);
    s.update(1.0 / 60.0);

    let deadline = Instant::now() + Duration::from_secs(2);
    while s.observed_speed() != Some(Speed::Paused) {
        assert!(
            Instant::now() < deadline,
            "the worker never reported pausing"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        s.app().seat() >= 1.0,
        "the eject ran out of travel before the core stopped, so the check below is vacuous"
    );
    let published = s.frames_published();
    while s.app().seat() >= 1.0 {
        s.update(1.0 / 60.0);
    }
    assert_eq!(
        s.frames_published(),
        published,
        "the core kept running while the picture was out"
    );
}

#[test]
fn an_eject_the_card_refuses_puts_up_a_banner() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let states = d.path().join("States");
    let _ = std::fs::remove_dir_all(&states);
    std::fs::write(&states, b"").unwrap();
    a.apply(Action::Eject);
    a.settle_saves();
    a.tick_ms(10);
    assert!(a.card_alarm(), "a refused eject raised no alarm");
}

#[test]
fn an_eject_is_on_the_card_once_saves_settle() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Eject);
    a.settle_saves();
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert_eq!(r.read_resume().unwrap().expect("the eject wrote no resume").len(), 1024);
    assert_eq!(read_slot_state(d.path()).cart, None);
}
