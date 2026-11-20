mod common;

use common::{app_playing_in, app_playing_with, tmp_root_with_carts, CoreSnapshot};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform, StateRing};

#[test]
fn undoing_a_save_removes_it_and_restores_the_evicted_entry() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    for i in 0..10 {
        r.push(&[i as u8; 64], b"png", &format!("2026-08-09_00-00-{i:02}"))
            .unwrap();
    }
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    assert_eq!(r.list().unwrap().len(), 10);
    assert!(r
        .list()
        .unwrap()
        .iter()
        .all(|e| e.stamp != "2026-08-09_00-00-00"));

    a.undo(2_000);
    let l = r.list().unwrap();
    assert_eq!(l.len(), 10);
    assert!(
        l.iter().any(|e| e.stamp == "2026-08-09_00-00-00"),
        "the evicted oldest entry was not restored"
    );
}

#[test]
fn a_save_behind_the_newest_stamp_survives_a_full_ring() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    for i in 0..10 {
        r.push(&[i as u8; 64], b"png", &format!("2099-01-01_00-00-{i:02}"))
            .unwrap();
    }
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    let l = r.list().unwrap();
    assert_eq!(l.len(), 10);
    assert_eq!(
        l[0].stamp, "2099-01-01_00-00-10",
        "the new save was not stamped after the newest one"
    );
    assert!(
        l.iter().all(|e| e.stamp != "2099-01-01_00-00-00"),
        "the oldest entry was kept instead of making room"
    );
}

#[test]
fn undo_expires_after_thirty_seconds() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    assert!(a.undo_available(30_999));
    assert!(!a.undo_available(31_001));
}

#[test]
fn a_second_save_replaces_the_undo_rather_than_stacking() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::SaveState, 2_000);
    a.undo(3_000);
    assert_eq!(
        r.list().unwrap().len(),
        1,
        "only the most recent save should be undone"
    );
}

#[test]
fn undoing_a_load_returns_the_emulator_to_the_prior_state() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    core.run_frame();
    a.apply_at(Action::SaveState, 500);
    for _ in 0..120 {
        core.run_frame();
    }
    let before = core.bytes();
    a.apply_at(Action::LoadState, 1_000);
    assert_ne!(core.bytes(), before, "the load never moved the core");
    a.undo(2_000);
    assert_eq!(core.bytes(), before);
}

#[test]
fn a_reset_starts_the_game_over_and_undoing_it_puts_the_game_back() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    let start = core.bytes();
    for _ in 0..120 {
        core.run_frame();
    }
    let before = core.bytes();
    a.apply_at(Action::Reset, 1_000);
    assert_eq!(core.bytes(), start);
    assert_eq!(a.undo_label(), Some("undo reset"));
    a.undo(2_000);
    assert_eq!(core.bytes(), before);
}

#[test]
fn a_reset_is_refused_while_linked() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    core.run_frame();
    let before = core.bytes();
    a.begin_link(0);
    a.apply_at(Action::Reset, 1_000);
    assert_eq!(core.bytes(), before);
    assert!(!a.undo_available(1_000));
}

#[test]
fn undo_is_not_offered_when_nothing_is_undoable() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let a = app_playing_in(d.path(), "Emerald");
    assert!(!a.undo_available(0));
}

#[test]
fn the_label_names_what_will_be_undone() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    assert_eq!(a.undo_label(), Some("undo save"));
    a.apply_at(Action::LoadState, 2_000);
    assert_eq!(a.undo_label(), Some("undo load"));
}

#[test]
fn x_undoes_from_the_switcher_and_never_from_the_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::GbaDown(Btn::X), 1_100);
    assert_eq!(r.list().unwrap().len(), 1, "X undid from the game");

    a.apply_at(Action::Polaroids, 1_200);
    a.apply_at(Action::GbaDown(Btn::X), 1_300);
    assert!(r.list().unwrap().is_empty(), "X did not undo the save");
}

#[test]
fn undoing_twice_does_not_put_the_save_back() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.undo(2_000);
    a.undo(3_000);
    assert!(r.list().unwrap().is_empty());
    assert!(!a.undo_available(3_000));
}

#[test]
fn ejecting_the_cart_takes_its_undo_with_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.apply_at(Action::Eject, 1_100);
    assert!(!a.undo_available(1_200));
}

#[test]
fn the_plate_is_drawn_only_over_the_switcher_and_only_while_the_offer_stands() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply_at(Action::SaveState, 1_000);
    a.tick_ms(4_000);
    let mut playing = Vec::new();
    a.draw(&mut playing);
    assert!(
        a.undo_available(4_000),
        "the offer expired before it was tested"
    );
    assert!(playing.is_empty(), "the offer reached the game");

    a.apply_at(Action::Polaroids, 4_000);
    let mut open = Vec::new();
    a.draw(&mut open);
    a.tick_ms(40_000);
    let mut expired = Vec::new();
    a.draw(&mut expired);
    assert_eq!(
        open.len(),
        expired.len() + 1,
        "the plate outlived the offer"
    );
}

#[test]
fn undo_load_in_the_menu_returns_the_emulator_to_the_prior_state() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let core = CoreSnapshot::new();
    let mut a = app_playing_with(d.path(), "Emerald", core.boxed());
    core.run_frame();
    a.apply_at(Action::SaveState, 500);
    for _ in 0..120 {
        core.run_frame();
    }
    let before = core.bytes();
    a.apply_at(Action::LoadState, 1_000);
    a.apply_at(Action::QuickMenu, 1_100);
    let mut at = 1_200;
    common::reach(
        &mut a,
        slot_ui::QuickRow::UndoLoad,
        |a| a.in_game_menu(),
        |a| {
            a.apply_at(Action::GbaDown(Btn::Down), at);
            a.apply_at(Action::GbaUp(Btn::Down), at + 50);
            at += 100;
        },
    );
    a.apply_at(Action::GbaDown(Btn::A), at);
    assert_eq!(core.bytes(), before);
}
