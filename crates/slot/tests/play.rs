mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{boot, tmp_root_with_carts};
use slot::app::Phase;
use slot::persist;
use slot::session::Session;
use slot_input::{Action, Btn, RawEvent};
use slot_store::{write_slot_state, AutoLoad, Core, Platform, SlotState, StateRing};

#[test]
fn a_tap_resumes_and_a_hold_starts_clean() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
        .write_resume(&[7u8; 64])
        .unwrap();

    let mut tap = boot(d.path());
    tap.apply_at(Action::GbaDown(Btn::A), 0);
    tap.apply_at(Action::GbaUp(Btn::A), 120);
    assert!(
        matches!(tap.phase(), Phase::Inserting { .. }),
        "a tap did not put the cart in: {:?}",
        tap.phase()
    );
    assert!(!tap.starting_clean(), "a tap threw the state away");

    let mut hold = boot(d.path());
    hold.apply_at(Action::GbaDown(Btn::A), 0);
    hold.tick_ms(500);
    assert!(hold.starting_clean(), "holding A still resumed");
}

#[test]
fn auto_load_alone_decides_whether_a_tap_resumes() {
    for (auto_load, eject_save) in [
        (AutoLoad::Never, true),
        (AutoLoad::PowerOn, true),
        (AutoLoad::Always, false),
    ] {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .write_resume(&[7u8; 64])
            .unwrap();
        let s = SlotState {
            clock_set: true,
            auto_load,
            eject_save,
            ..SlotState::default()
        };
        write_slot_state(d.path(), &s).unwrap();
        let mut a = boot(d.path());
        a.apply_at(Action::GbaDown(Btn::A), 0);
        a.apply_at(Action::GbaUp(Btn::A), 120);
        assert_eq!(
            a.starting_clean(),
            auto_load != AutoLoad::Always,
            "{auto_load:?} eject_save {eject_save}"
        );
    }
}

#[test]
fn a_clean_start_leaves_the_state_on_disk() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    r.write_resume(&[7u8; 64]).unwrap();
    let mut a = boot(d.path());
    a.apply_at(Action::GbaDown(Btn::A), 0);
    a.tick_ms(500);
    assert_eq!(
        r.read_resume().unwrap(),
        Some(vec![7u8; 64]),
        "the state was deleted"
    );
}

#[test]
fn the_clean_start_fires_on_the_threshold_not_the_release() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply_at(Action::GbaDown(Btn::A), 0);
    a.tick_ms(499);
    assert!(matches!(a.phase(), Phase::Shelf), "it seated early");
    a.tick_ms(500);
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "nothing happened on the hold: {:?}",
        a.phase()
    );
}

#[test]
fn the_release_after_a_hold_is_not_another_press() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply_at(Action::GbaDown(Btn::A), 0);
    a.tick_ms(500);
    a.apply_at(Action::GbaUp(Btn::A), 700);
    assert!(a.starting_clean(), "the release resumed the cart after all");
}

fn counter_after(root: &Path, hold: bool) -> u64 {
    common::clocked(root);
    let mut s = Session::boot(root.to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    if !hold {
        s.feed([RawEvent::Up(Btn::A)], 32);
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    s.app_mut().tick_ms(300_000);
    s.app_mut().settle_saves();
    let state = persist::read_resume(root, Platform::Gba, Core::Mgba, "Emerald")
        .unwrap()
        .expect("nothing was flushed");
    u64::from_le_bytes(state.try_into().expect("the mock's state is 8 bytes"))
}

#[test]
fn a_hold_hands_the_core_no_state_and_a_tap_hands_it_the_resume() {
    let tapped = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let held = tmp_root_with_carts(&["Emerald", "Fusion"]);
    for d in [&tapped, &held] {
        persist::flush(
            d.path(),
            Platform::Gba,
            Core::Mgba,
            "Emerald",
            Some(&500_000u64.to_le_bytes()),
            None,
        )
        .unwrap();
    }
    assert!(
        counter_after(tapped.path(), false) >= 500_000,
        "a tap started the cart cold"
    );
    assert!(
        counter_after(held.path(), true) < 500_000,
        "a hold resumed the state it was meant to skip"
    );
}

#[test]
fn a_game_left_running_starts_loading_before_the_first_frame() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            cart: Some("Emerald".into()),
            cart_platform: Some(Platform::Gba),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();

    let s = Session::boot(d.path().to_path_buf());
    assert!(
        matches!(s.app().phase(), Phase::Inserting { resumed: true, .. }),
        "the game was not put back in the slot: {:?}",
        s.app().phase()
    );
    assert!(
        s.has_core(),
        "the core waited for the first update, so a cold boot sat idle on the slot"
    );
}

#[test]
fn an_empty_slot_loads_no_core_at_boot() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    common::clocked(d.path());
    let s = Session::boot(d.path().to_path_buf());
    assert!(matches!(s.app().phase(), Phase::Shelf));
    assert!(!s.has_core(), "a core opened with nothing in the slot");
}

#[cfg(unix)]
#[test]
fn a_save_the_card_cannot_read_is_never_replaced_with_a_blank_one() {
    use std::os::unix::fs::PermissionsExt;
    let d = tmp_root_with_carts(&["Emerald"]);
    let root = d.path();
    common::clocked(root);
    let sav = root.join("Saves/GBA/Emerald.sav");
    std::fs::create_dir_all(sav.parent().unwrap()).unwrap();
    std::fs::write(&sav, b"the player's real save").unwrap();
    std::fs::set_permissions(&sav, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&sav).is_ok() {
        return;
    }

    let mut s = Session::boot(root.to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    s.app_mut().tick_ms(60_000);
    s.app_mut().settle_saves();

    std::fs::set_permissions(&sav, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        std::fs::read(&sav).unwrap(),
        b"the player's real save",
        "a save that failed to read was replaced by the blank one the game booted with"
    );
}
