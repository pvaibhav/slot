mod common;

use common::{app_playing_in, app_playing_with_volume, boot, tmp_root_with_carts};
use slot::app::Phase;
use slot_input::Action;
use slot_store::read_slot_state;
use slot_ui::Icon;

#[test]
fn levels_clamp_and_persist() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    for _ in 0..20 {
        a.apply(Action::BrightnessUp);
    }
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).brightness, 9);
    for _ in 0..20 {
        a.apply(Action::BrightnessDown);
    }
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).brightness, 0);
}

#[test]
fn brightness_down_past_the_bottom_dims_and_up_undims_first() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    for _ in 0..20 {
        a.apply(Action::BrightnessDown);
    }
    a.flush_state();
    let s = read_slot_state(d.path());
    assert_eq!((s.brightness, s.dim), (0, 6));
    assert_eq!(a.brightness_level(), 0);
    a.apply(Action::BrightnessUp);
    assert_eq!((a.brightness(), a.dim()), (0, 5));
    assert_eq!(a.brightness_level(), 1);
    for _ in 0..5 {
        a.apply(Action::BrightnessUp);
    }
    assert_eq!((a.brightness(), a.dim()), (0, 0));
    a.apply(Action::BrightnessUp);
    assert_eq!((a.brightness(), a.dim()), (1, 0));
    assert_eq!(a.brightness_level(), 7);
}

#[test]
fn volume_moves_five_per_press() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    a.apply(Action::VolumeUp);
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).volume, 65);
    for _ in 0..20 {
        a.apply(Action::VolumeDown);
    }
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).volume, 0);
}

#[test]
fn levels_work_on_the_shelf_not_only_in_game() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let mut a = boot(d.path());
    assert!(matches!(a.phase(), Phase::Shelf));
    a.apply(Action::BlueLightUp);
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).blue_light, 1);
}

#[test]
fn adjusting_a_level_in_game_leaves_the_game_running() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::VolumeDown);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    a.flush_state();
    assert_eq!(read_slot_state(d.path()).volume, 55);
}

#[test]
fn muting_leaves_the_level_alone_even_at_the_ends() {
    for start in [0u8, 5, 60, 100] {
        let d = tmp_root_with_carts(&["Emerald"]);
        let mut a = app_playing_with_volume(d.path(), start);
        a.apply_at(Action::VolumeUp, 0);
        a.apply_at(Action::VolumeDown, 40);
        a.apply_at(Action::MuteToggle, 60);
        assert!(a.muted(), "not muted from {start}");
        a.apply_at(Action::MuteToggle, 2_000);
        assert_eq!(a.volume(), start, "unmuted to the wrong level from {start}");
    }
}

#[test]
fn muted_is_silent_but_remembers() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_volume(d.path(), 70);
    a.apply_at(Action::MuteToggle, 0);
    assert_eq!(a.output_volume(), 0, "muted but still audible");
    assert_eq!(a.volume(), 70, "muting forgot the level");
    assert_eq!(a.hud_icon(), Icon::VolumeMuted);
}

#[test]
fn adjusting_the_volume_unmutes() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_volume(d.path(), 70);
    a.apply_at(Action::MuteToggle, 0);
    a.apply_at(Action::VolumeUp, 1_000);
    assert!(!a.muted(), "still muted after turning it up");
    assert_eq!(a.volume(), 75);
}

#[test]
fn unmuting_with_the_chord_does_not_mute_again() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_volume(d.path(), 70);
    a.apply_at(Action::MuteToggle, 0);
    a.apply_at(Action::VolumeDown, 1_000);
    a.apply_at(Action::VolumeUp, 1_060);
    a.apply_at(Action::MuteToggle, 1_060);
    assert!(!a.muted(), "the unmute chord muted it again");
    assert_eq!(a.volume(), 70);
}

#[test]
fn the_mute_survives_a_reboot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_volume(d.path(), 70);
    a.apply_at(Action::MuteToggle, 0);
    drop(a);
    let b = boot(d.path());
    assert!(b.muted());
    assert_eq!(b.volume(), 70);
}

#[test]
fn turning_the_volume_up_saves_the_unmute_with_the_level() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_volume(d.path(), 70);
    a.apply_at(Action::MuteToggle, 0);
    a.flush_state();
    a.apply_at(Action::VolumeUp, 1_000);
    a.flush_state();
    let saved = read_slot_state(d.path());
    assert_eq!(saved.volume, 75);
    assert!(
        !saved.muted,
        "the card still says muted after the sound came back"
    );
}
