mod common;

use common::tmp_root_with_carts;
use slot::app::{App, Phase};
use slot::loading_mark::LoadingMark;
use slot::persist;
use slot_store::{read_slot_state, write_slot_state, Core, Platform, SlotState, StateRing};

fn seated_in(root: &std::path::Path, stem: &str) {
    write_slot_state(
        root,
        &SlotState {
            cart: Some(stem.to_string()),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn a_cart_that_took_slot_down_while_loading_comes_back_on_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    seated_in(d.path(), "Emerald");
    persist::flush(
        d.path(),
        Platform::Gba,
        Core::Armgba,
        "Emerald",
        Some(&[5u8; 64]),
        None,
    )
    .unwrap();
    let run = tempfile::tempdir().unwrap();
    let mark = LoadingMark::in_dir(run.path());
    mark.set("Emerald");

    let a = App::boot_marked(d.path(), Some(mark.clone()));

    assert!(
        matches!(a.phase(), Phase::Shelf),
        "the crashing cart was seated again"
    );
    assert_eq!(mark.left(), None, "the mark outlived the boot it guarded");
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Armgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none(),
        "the state it crashed on would load again"
    );
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).cart, None);
}

#[test]
fn a_seated_cart_is_marked_while_it_loads() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    seated_in(d.path(), "Emerald");
    let run = tempfile::tempdir().unwrap();
    let mark = LoadingMark::in_dir(run.path());

    let a = App::boot_marked(d.path(), Some(mark.clone()));

    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    assert_eq!(mark.left().as_deref(), Some("Emerald"));
}

#[test]
fn ten_seconds_of_play_clears_the_mark() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    seated_in(d.path(), "Emerald");
    let run = tempfile::tempdir().unwrap();
    let mark = LoadingMark::in_dir(run.path());
    let mut a = App::boot_marked(d.path(), Some(mark.clone()));
    a.set_snapshot(common::StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    let now = a.now();

    a.tick_ms(now + 7_000);
    assert!(mark.left().is_some(), "cleared before the game had settled");
    a.tick_ms(now + 10_000);
    assert_eq!(mark.left(), None);
}

#[test]
fn without_a_mark_a_seated_cart_comes_back() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    seated_in(d.path(), "Emerald");
    let run = tempfile::tempdir().unwrap();

    let a = App::boot_marked(d.path(), Some(LoadingMark::in_dir(run.path())));

    assert!(matches!(a.phase(), Phase::Inserting { .. }));
}
