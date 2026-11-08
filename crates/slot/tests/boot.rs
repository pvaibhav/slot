mod common;

use common::{boot, tmp_root_with_carts};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, AutoLoad, Platform, SlotState};

fn seated(cart: &str) -> SlotState {
    SlotState {
        cart: Some(cart.into()),
        clock_set: true,
        ..Default::default()
    }
}

#[test]
fn boot_with_a_seated_cart_never_shows_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(d.path(), &seated("Emerald")).unwrap();
    let a = App::boot(d.path());
    assert!(
        matches!(a.phase(), Phase::Inserting { .. }),
        "boot must go straight to the seated cart, not the shelf"
    );
}

#[test]
fn boot_reseats_the_cart_fresh_only_when_auto_load_state_is_never() {
    for auto_load in AutoLoad::ALL {
        let d = tmp_root_with_carts(&["Emerald"]);
        let s = SlotState {
            auto_load,
            ..seated("Emerald")
        };
        write_slot_state(d.path(), &s).unwrap();
        let a = App::boot(d.path());
        assert!(matches!(a.phase(), Phase::Inserting { .. }));
        assert_eq!(
            a.starting_clean(),
            auto_load == AutoLoad::Never,
            "{auto_load:?}"
        );
    }
}

#[test]
fn an_old_on_or_off_auto_load_still_reads_from_the_card() {
    for (value, want, clean) in [
        ("1", AutoLoad::Always, false),
        ("0", AutoLoad::Never, true),
        ("power", AutoLoad::PowerOn, false),
    ] {
        let d = tmp_root_with_carts(&["Emerald"]);
        write_slot_state(d.path(), &seated("Emerald")).unwrap();
        let path = d.path().join("Config/slot.state");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("auto_load=1\n"), "{text}");
        std::fs::write(
            &path,
            text.replace("auto_load=1\n", &format!("auto_load={value}\n")),
        )
        .unwrap();
        assert_eq!(read_slot_state(d.path()).auto_load, want, "{value}");
        let a = App::boot(d.path());
        assert_eq!(a.starting_clean(), clean, "{value}");
    }
}

#[test]
fn boot_with_an_empty_slot_shows_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let a = boot(d.path());
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn boot_with_a_cart_that_no_longer_exists_falls_back_to_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(d.path(), &seated("Deleted")).unwrap();
    let a = App::boot(d.path());
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn boot_leaves_the_shelf_sitting_on_the_resumed_cart() {
    let d = tmp_root_with_carts(&["Advance Wars", "Emerald", "Fire Emblem"]);
    write_slot_state(d.path(), &seated("Emerald")).unwrap();
    let mut a = App::boot(d.path());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::Eject);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
    a.apply(Action::Insert);
    let Phase::Inserting { cart, .. } = a.phase() else {
        panic!("insert after eject did nothing: {:?}", a.phase())
    };
    assert_eq!(cart, "Emerald");
}

#[test]
fn a_seated_cart_is_recorded_so_the_next_boot_can_resume_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    a.apply(Action::Insert);
    a.on_core_ready();
    assert_eq!(
        read_slot_state(d.path()).cart,
        None,
        "a cart that has not seated yet is not in the slot"
    );
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(read_slot_state(d.path()).cart, Some("Emerald".into()));
}

#[test]
fn a_cart_that_is_gone_takes_its_shelf_out_of_the_slot_with_it() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Deleted".into()),
            cart_platform: Some(Platform::Gbc),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.apply(Action::MuteToggle);
    a.flush_state();
    let s = read_slot_state(d.path());
    assert_eq!(s.cart, None);
    assert_eq!(
        s.cart_platform, None,
        "the card names a shelf beside a line that names no cart"
    );
}

#[test]
fn a_cart_that_fails_to_load_leaves_the_slot_empty() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(d.path(), &seated("Emerald")).unwrap();
    let mut a = App::boot(d.path());
    a.on_core_failed();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
    assert_eq!(read_slot_state(d.path()).cart, None);
}

#[test]
fn seating_a_cart_preserves_the_levels_already_in_the_file() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: None,
            brightness: 2,
            blue_light: 7,
            volume: 35,
            muted: false,
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    let s = read_slot_state(d.path());
    assert_eq!((s.brightness, s.blue_light, s.volume), (2, 7, 35));
}

#[test]
fn a_resume_draws_no_shelf_but_a_chosen_insert_does() {
    let seated = || {
        let d = common::tmp_root_with_carts(&["Emerald", "Fusion", "Wars"]);
        write_slot_state(
            d.path(),
            &SlotState {
                cart: Some("Emerald".into()),
                clock_set: true,
                ..Default::default()
            },
        )
        .unwrap();
        (App::boot(d.path()), d)
    };
    let chosen = || {
        let d = common::tmp_root_with_carts(&["Emerald", "Fusion", "Wars"]);
        write_slot_state(
            d.path(),
            &SlotState {
                clock_set: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mut a = App::boot(d.path());
        a.apply(Action::Insert);
        (a, d)
    };

    let (resume, _d1) = seated();
    let (pick, _d2) = chosen();
    let (r, p) = (draw_count(&resume), draw_count(&pick));
    assert!(
        p > r,
        "a resume drew {r} and a chosen insert {p}: the shelf is on screen for both"
    );

    let (mut resume, _d3) = seated();
    for frame in 0..90 {
        assert!(
            draw_count(&resume) <= r,
            "frame {frame}: the shelf appeared partway through a resume"
        );
        resume.update(1.0 / 60.0);
    }
}

fn draw_count(a: &App) -> usize {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.len()
}

#[test]
fn a_loose_card_shows_an_empty_shelf_and_nothing_on_it_is_moved() {
    let d = tempfile::tempdir().unwrap();
    for sub in ["Games", "Saves", "Labels", "States"] {
        std::fs::create_dir_all(d.path().join(sub)).unwrap();
    }
    std::fs::write(d.path().join("Games/Emerald.gba"), vec![0u8; 0x100]).unwrap();
    std::fs::write(d.path().join("Saves/Emerald.sav"), vec![7u8; 0x10000]).unwrap();
    std::fs::write(d.path().join("Labels/Emerald.png"), b"png").unwrap();
    let old_states = d.path().join("States/mgba/Emerald");
    std::fs::create_dir_all(&old_states).unwrap();
    std::fs::write(old_states.join("resume.state"), b"resume").unwrap();

    let a = App::boot(d.path());

    assert_eq!(
        a.carts().count(),
        0,
        "a loose rom reached the shelf, so something is still reading outside Games/<platform>/"
    );
    assert_eq!(
        std::fs::read(d.path().join("Games/Emerald.gba"))
            .unwrap()
            .len(),
        0x100,
        "the loose rom was moved or disturbed"
    );
    assert_eq!(
        std::fs::read(d.path().join("Saves/Emerald.sav"))
            .unwrap()
            .len(),
        0x10000,
        "the loose battery save was moved or disturbed"
    );
    assert_eq!(
        std::fs::read(d.path().join("Labels/Emerald.png")).unwrap(),
        b"png",
        "the loose label was moved"
    );
    assert_eq!(
        std::fs::read(old_states.join("resume.state")).unwrap(),
        b"resume",
        "a pre-namespacing state directory was moved"
    );
}

#[test]
fn boot_creates_the_folders_a_person_has_to_file_into() {
    let d = tempfile::tempdir().unwrap();

    App::boot(d.path());

    for name in slot::root::DIRS {
        assert!(
            d.path().join(name).is_dir(),
            "{name} is missing, so nothing on the card says where its files go"
        );
    }
}

fn two_tetrises() -> tempfile::TempDir {
    let d = tmp_root_with_carts(&["Tetris", "Emerald"]);
    common::write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn resumed(d: &tempfile::TempDir, platform: Option<Platform>) -> App {
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Tetris".into()),
            cart_platform: platform,
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    App::boot(d.path())
}

#[test]
fn the_platform_on_the_card_decides_which_tetris_comes_back() {
    let d = two_tetrises();
    for platform in [Platform::Gb, Platform::Gba] {
        let a = resumed(&d, Some(platform));
        let cart = a.seated_cart().expect("nothing seated");
        assert_eq!(
            cart.platform, platform,
            "the card named {platform:?} and a {:?} cart came back",
            cart.platform
        );
        assert!(
            cart.rom.ends_with(format!(
                "{}/Tetris.{}",
                platform.dir_name(),
                platform.extensions()[0]
            )),
            "the rom the slot is holding is {:?}",
            cart.rom
        );
    }
}

#[test]
fn a_card_that_never_said_resumes_the_gba_cart() {
    let d = two_tetrises();
    let a = resumed(&d, None);
    assert_eq!(
        a.seated_cart().expect("nothing seated").platform,
        Platform::Gba
    );
}

#[test]
fn a_named_shelf_that_no_longer_has_the_cart_is_an_empty_slot() {
    let d = two_tetrises();
    let a = resumed(&d, Some(Platform::Gbc));
    assert!(
        matches!(a.phase(), Phase::Shelf),
        "a Colour Tetris that is not on the card seated something: {:?}",
        a.phase()
    );
}

#[test]
fn seating_a_cart_records_the_shelf_it_came_off() {
    let d = two_tetrises();
    let mut a = boot(d.path());
    common::through(&mut a, Btn::R1);
    assert_eq!(
        a.selected_stem(),
        Some("Tetris"),
        "the shoulder did not ring to the Game Boy shelf"
    );
    a.apply(Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    let s = read_slot_state(d.path());
    assert_eq!(
        (s.cart.as_deref(), s.cart_platform),
        (Some("Tetris"), Some(Platform::Gb)),
        "the card does not say which Tetris is in the slot"
    );

    let again = App::boot(d.path());
    assert_eq!(
        again.seated_cart().expect("nothing seated").platform,
        Platform::Gb
    );
}

#[test]
fn an_eject_forgets_the_platform_with_the_cart() {
    let d = two_tetrises();
    let mut a = resumed(&d, Some(Platform::Gb));
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(Action::Eject);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf));
    let s = read_slot_state(d.path());
    assert_eq!((s.cart, s.cart_platform), (None, None));
}

#[test]
fn a_resumed_cart_starts_seated() {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let a = App::boot(d.path());
    assert_eq!(a.seat(), 1.0, "the resumed cart is still sliding in");
}
