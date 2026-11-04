mod common;

use common::tmp_root;
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};

fn favorite(platform: Platform, stem: &str) -> Recent {
    Recent {
        platform,
        stem: stem.into(),
    }
}

fn clocked() -> SlotState {
    SlotState {
        clock_set: true,
        ..SlotState::default()
    }
}

#[test]
fn nothing_is_a_favorite_until_it_is_hearted() {
    let s = SlotState::default();
    assert!(s.favorites.is_empty());
    assert!(!s.is_favorite(Platform::Gba, "Emerald"));
}

#[test]
fn hearting_twice_takes_the_heart_back_off() {
    let mut s = clocked();
    assert!(s.toggle_favorite(Platform::Gba, "Emerald"));
    assert!(s.is_favorite(Platform::Gba, "Emerald"));
    assert!(!s.toggle_favorite(Platform::Gba, "Emerald"));
    assert!(!s.is_favorite(Platform::Gba, "Emerald"));
}

#[test]
fn the_same_name_on_two_systems_is_two_favorites() {
    let mut s = clocked();
    s.toggle_favorite(Platform::Gb, "Tetris");
    assert!(!s.is_favorite(Platform::Gbc, "Tetris"));
    s.toggle_favorite(Platform::Gbc, "Tetris");
    assert_eq!(s.favorites.len(), 2);
}

#[test]
fn favorites_survive_a_restart() {
    let d = tmp_root();
    let mut s = clocked();
    s.toggle_favorite(Platform::Gba, "Pokemon - Emerald Version (USA, Europe)");
    s.toggle_favorite(Platform::Gbc, "Tetris DX");
    write_slot_state(d.path(), &s).unwrap();
    assert_eq!(read_slot_state(d.path()), s);
}

#[test]
fn a_card_without_a_favorites_file_reads_with_none() {
    let d = tmp_root();
    let mut s = clocked();
    s.toggle_favorite(Platform::Gba, "Emerald");
    write_slot_state(d.path(), &s).unwrap();
    std::fs::remove_file(d.path().join("Config/favorites.txt")).unwrap();
    let s = read_slot_state(d.path());
    assert!(s.clock_set, "the old settings did not read back");
    assert!(s.favorites.is_empty());
}

#[test]
fn garbled_or_repeated_favorite_lines_are_skipped() {
    let d = tmp_root();
    let mut s = clocked();
    s.toggle_favorite(Platform::Gba, "Emerald");
    write_slot_state(d.path(), &s).unwrap();
    let path = d.path().join("Config/favorites.txt");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("n64/Mario\ngba/\nnothing\ngba/Emerald\n");
    std::fs::write(&path, text).unwrap();
    assert_eq!(
        read_slot_state(d.path()).favorites,
        [favorite(Platform::Gba, "Emerald")]
    );
}

#[test]
fn recents_and_favorites_live_in_their_own_files_one_cart_a_line() {
    let d = tmp_root();
    let mut s = clocked();
    s.played(Platform::Gba, "Emerald");
    s.played(Platform::Gb, "Tetris");
    s.toggle_favorite(Platform::Gbc, "Tetris DX");
    write_slot_state(d.path(), &s).unwrap();
    let config = d.path().join("Config");
    assert_eq!(
        std::fs::read_to_string(config.join("recents.txt")).unwrap(),
        "gb/Tetris\ngba/Emerald\n"
    );
    assert_eq!(
        std::fs::read_to_string(config.join("favorites.txt")).unwrap(),
        "gbc/Tetris DX\n"
    );
    let state = std::fs::read_to_string(config.join("slot.state")).unwrap();
    assert!(
        !state
            .lines()
            .any(|l| l.starts_with("recent=") || l.starts_with("favorite=")),
        "{state}"
    );
}

#[test]
fn nothing_hearted_writes_no_favorites_file() {
    let d = tmp_root();
    write_slot_state(d.path(), &clocked()).unwrap();
    assert!(!d.path().join("Config/favorites.txt").exists());
}
