mod common;

use common::{tmp_root_with_gbc_carts, write_gb_cart};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn tetris_twice() -> tempfile::TempDir {
    let d = tmp_root_with_gbc_carts(&["Tetris"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn on_favorites(d: &tempfile::TempDir) -> App {
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            favorites: vec![
                Recent {
                    platform: Platform::Gb,
                    stem: "Tetris".into(),
                },
                Recent {
                    platform: Platform::Gbc,
                    stem: "Tetris".into(),
                },
            ],
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    for _ in 0..12 {
        if a.shelf_platform_name() == Some("Favorites") {
            break;
        }
        common::through(&mut a, Btn::R1);
    }
    assert_eq!(a.shelf_platform_name(), Some("Favorites"));
    assert_eq!(a.shelf_stems(), ["Tetris", "Tetris"]);
    a
}

fn play_the(a: &mut App, platform: Platform) {
    for _ in 0..3 {
        if a.selected_cart().map(|c| c.platform) == Some(platform) {
            break;
        }
        press(a, Btn::Right);
        for _ in 0..30 {
            a.update(1.0 / 60.0);
        }
    }
    assert_eq!(a.selected_cart().map(|c| c.platform), Some(platform));
    press(a, Btn::A);
    a.set_snapshot(common::StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn the_second_of_two_same_name_carts_plays_itself() {
    for platform in [Platform::Gb, Platform::Gbc] {
        let d = tetris_twice();
        let mut a = on_favorites(&d);
        play_the(&mut a, platform);
        assert_eq!(a.seated_cart().map(|c| c.platform), Some(platform));
        assert_eq!(a.playing_platform(), platform);
        a.flush_state();
        assert_eq!(read_slot_state(d.path()).cart_platform, Some(platform));
    }
}

#[test]
fn a_resume_seats_the_platform_it_was_saved_with() {
    let d = tetris_twice();
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            cart: Some("Tetris".into()),
            cart_platform: Some(Platform::Gbc),
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_snapshot(common::StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(a.seated_cart().map(|c| c.platform), Some(Platform::Gbc));
}
