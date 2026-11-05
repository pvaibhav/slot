mod common;

use slot::app::App;
use slot_input::Btn;
use slot_store::{write_slot_state, Platform, Recent, SlotState};

fn card(gba: bool) -> tempfile::TempDir {
    let d = match gba {
        true => common::tmp_root_with_carts(&["Emerald"]),
        false => common::tmp_root_with_carts(&[]),
    };
    common::write_gb_cart(&d, "Tetris", "TETRIS");
    let gbav = d.path().join("Games").join(Platform::Gbav.dir_name());
    std::fs::create_dir_all(&gbav).unwrap();
    std::fs::write(gbav.join("Shrek.gba"), vec![0u8; 0x100]).unwrap();
    d
}

fn boot(d: &tempfile::TempDir, shelf: &str, platform: Platform, stem: &str) -> App {
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            shelf: Some(shelf.into()),
            shelf_cart: Some(Recent {
                platform,
                stem: stem.into(),
            }),
            ..SlotState::default()
        },
    )
    .unwrap();
    App::boot(d.path())
}

fn on_gba(d: &tempfile::TempDir) -> App {
    let a = boot(d, "gba", Platform::Gba, "Emerald");
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    a
}

fn row(a: &mut App) -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..6 {
        common::through(a, Btn::R1);
        seen.extend(a.shelf_platform_name().map(str::to_string));
    }
    seen
}

#[test]
fn video_is_not_in_the_l1_r1_row() {
    let d = card(true);
    let mut a = on_gba(&d);
    let seen = row(&mut a);
    assert!(!seen.iter().any(|n| n == "Game Boy Advance Video"), "{seen:?}");
}

#[test]
fn r2_on_gba_flips_to_video_and_back() {
    let d = card(true);
    let mut a = on_gba(&d);
    common::through(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance Video"));
    common::through(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance"));
    common::through(&mut a, Btn::L2);
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance Video"));
}

#[test]
fn l1_r1_from_video_moves_along_the_row_from_gba() {
    let d = card(true);
    let mut a = on_gba(&d);
    common::through(&mut a, Btn::R1);
    let after_gba = a.shelf_platform_name().map(str::to_string);
    let mut a = boot(&d, "gbav", Platform::Gbav, "Shrek");
    assert_eq!(a.shelf_platform_name(), Some("Game Boy Advance Video"));
    common::through(&mut a, Btn::R1);
    assert_eq!(a.shelf_platform_name().map(str::to_string), after_gba);
}

#[test]
fn with_no_gba_games_video_takes_the_gba_seat() {
    let d = card(false);
    let mut a = boot(&d, "gb", Platform::Gb, "Tetris");
    let seen = row(&mut a);
    assert!(seen.iter().any(|n| n == "Game Boy Advance Video"), "{seen:?}");
}
