mod common;

use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::session::Session;
use slot_input::{Btn, Millis, RawEvent};
use slot_store::{write_slot_state, KidMode, SlotState};

fn step(s: &mut Session, now: &mut Millis, ev: Option<RawEvent>) {
    *now += 16;
    s.feed(ev, *now);
    s.update(1.0 / 60.0);
}

fn locked_game() -> (tempfile::TempDir, Session, Millis) {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            kid_mode: KidMode::Shelves,
            rewind: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }
    (d, s, now)
}

#[test]
fn r2_double_tap_never_latches_fast_forward_when_locked() {
    let (_d, mut s, mut now) = locked_game();
    let lit = s.app().brightness();
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::R2)));
    for _ in 0..20 {
        step(&mut s, &mut now, None);
    }
    assert!(s.app().ff_badge().is_none(), "fast forward latched");
    assert_eq!(
        s.app().brightness(),
        (lit + 2).min(slot_store::BRIGHTNESS_MAX)
    );
}

#[test]
fn l2_held_never_rewinds_when_locked() {
    let (_d, mut s, mut now) = locked_game();
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::L2)));
    for _ in 0..60 {
        step(&mut s, &mut now, None);
        assert!(s.app().ff_badge().is_none(), "a rewind badge showed");
    }
    step(&mut s, &mut now, Some(RawEvent::Up(Btn::L2)));
}

#[test]
fn releasing_the_chord_opens_nothing() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            kid_mode: KidMode::Shelves,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    for _ in 0..10 {
        step(&mut s, &mut now, None);
    }
    s.app_mut().unlock_kid_mode();
    let favorites = slot_store::read_slot_state(d.path()).favorites.len();
    for b in [Btn::Select, Btn::Start, Btn::L2, Btn::R2] {
        step(&mut s, &mut now, Some(RawEvent::Up(b)));
    }
    for _ in 0..60 {
        step(&mut s, &mut now, None);
    }
    assert!(
        matches!(s.app().phase(), Phase::Shelf),
        "{:?}",
        s.app().phase()
    );
    assert!(s.app().studio().is_none());
    assert!(s.app().core_picker().is_none());
    assert!(s.app().edit().is_none());
    assert_eq!(
        slot_store::read_slot_state(d.path()).favorites.len(),
        favorites
    );
}
