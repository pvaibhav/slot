mod common;

use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::emu::Speed;
use slot::session::Session;
use slot_input::{Btn, Millis, RawEvent};
use slot_store::{
    write_shortcuts, write_slot_state, Binding, Core, Device, Modifier, Shortcut, ShortcutKeys,
    SlotState,
};
use slot_ui::QuickRow;
use tempfile::TempDir;

const BAIL: Duration = Duration::from_secs(5);

fn session_playing() -> (Session, TempDir, Millis) {
    session_playing_with(&ShortcutKeys::default())
}

fn session_playing_with(keys: &ShortcutKeys) -> (Session, TempDir, Millis) {
    let d = common::tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_shortcuts(d.path(), keys).expect("write shortcuts");
    common::write_retail_header(&d, "Emerald", "POKEMON RUBY", "AXVE");
    slot_store::write_selected_core(d.path(), slot_store::Platform::Gba, "Emerald", Core::Mgba).expect("write core");
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    (s, d, now)
}

fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
    *now += 16;
    s.feed(events.iter().copied(), *now);
    s.update(1.0 / 60.0);
}

fn tap(s: &mut Session, now: &mut Millis, btn: Btn) {
    step(s, now, &[RawEvent::Down(btn)]);
    step(s, now, &[RawEvent::Up(btn)]);
}

fn runs_at(s: &mut Session, now: &mut Millis, want: Speed) -> bool {
    let deadline = Instant::now() + BAIL;
    while s.observed_speed() != Some(want) {
        if Instant::now() >= deadline {
            return false;
        }
        step(s, now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}

fn open_menu(s: &mut Session, now: &mut Millis) {
    tap(s, now, Btn::Menu);
    for _ in 0..30 {
        if s.app().in_game_menu().is_some() {
            return;
        }
        step(s, now, &[]);
    }
    panic!("a tap of MENU never opened the in-game menu");
}

#[test]
fn the_in_game_menu_pauses_the_game_and_resume_runs_it_again() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    open_menu(&mut s, &mut now);
    assert_eq!(s.app().in_game_menu(), Some(QuickRow::Resume));
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "the game ran on behind the menu"
    );
    tap(&mut s, &mut now, Btn::B);
    assert_eq!(s.app().in_game_menu(), None);
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "resume left the game paused"
    );
}

#[test]
fn colour_correction_changed_behind_the_menu_redraws_the_paused_frame() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    open_menu(&mut s, &mut now);
    assert!(runs_at(&mut s, &mut now, Speed::Paused));
    for btn in [Btn::Down, Btn::A, Btn::A, Btn::Down] {
        tap(&mut s, &mut now, btn);
    }
    assert_eq!(s.app().in_game_menu(), Some(QuickRow::ColourCorrection));
    let shown = s.emu().map(|e| e.published_count()).expect("no core");
    tap(&mut s, &mut now, Btn::Right);
    let deadline = Instant::now() + BAIL;
    while s.emu().map(|e| e.published_count()) == Some(shown) {
        assert!(
            Instant::now() < deadline,
            "the paused frame was never redrawn"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(s.observed_speed(), Some(Speed::Paused));
}

#[test]
fn frame_blending_changed_behind_the_menu_redraws_the_paused_frame() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    open_menu(&mut s, &mut now);
    assert!(runs_at(&mut s, &mut now, Speed::Paused));
    for btn in [Btn::Down, Btn::A, Btn::A, Btn::Down, Btn::Down] {
        tap(&mut s, &mut now, btn);
    }
    assert_eq!(s.app().in_game_menu(), Some(QuickRow::Blending));
    let shown = s.emu().map(|e| e.published_count()).expect("no core");
    tap(&mut s, &mut now, Btn::Right);
    let deadline = Instant::now() + BAIL;
    while s.emu().map(|e| e.published_count()) == Some(shown) {
        assert!(
            Instant::now() < deadline,
            "the paused frame was never redrawn"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(s.observed_speed(), Some(Speed::Paused));
}

#[test]
fn locked_fast_forward_pauses_behind_the_menu_and_comes_back_locked() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    tap(&mut s, &mut now, Btn::R2);
    tap(&mut s, &mut now, Btn::R2);
    assert!(
        runs_at(&mut s, &mut now, Speed::Fast),
        "a double tap of R2 never locked fast forward"
    );
    open_menu(&mut s, &mut now);
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "fast forward ran on behind the menu"
    );
    tap(&mut s, &mut now, Btn::B);
    assert!(
        runs_at(&mut s, &mut now, Speed::Fast),
        "fast forward came back unlocked"
    );
}

#[test]
fn a_double_tap_of_menu_in_game_opens_the_switcher_and_b_goes_back_to_the_game() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    s.app_mut().apply(slot_input::Action::SaveState);
    tap(&mut s, &mut now, Btn::Menu);
    tap(&mut s, &mut now, Btn::Menu);
    assert!(
        matches!(s.app().phase(), Phase::Polaroids { .. }),
        "{:?}",
        s.app().phase()
    );
    for _ in 0..30 {
        step(&mut s, &mut now, &[]);
    }
    tap(&mut s, &mut now, Btn::B);
    assert!(matches!(s.app().phase(), Phase::Playing { .. }));
    assert_eq!(s.app().in_game_menu(), None);
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game stayed paused"
    );
}

#[test]
fn fast_forward_cannot_be_locked_from_inside_the_menu() {
    let (mut s, _d, mut now) = session_playing();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    open_menu(&mut s, &mut now);
    tap(&mut s, &mut now, Btn::R2);
    tap(&mut s, &mut now, Btn::R2);
    tap(&mut s, &mut now, Btn::B);
    assert_eq!(s.app().in_game_menu(), None);
    assert!(runs_at(&mut s, &mut now, Speed::Normal));
    for _ in 0..20 {
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
        assert_eq!(
            s.observed_speed(),
            Some(Speed::Normal),
            "R2 pressed in the menu fast forwarded the game on resume"
        );
    }
}

#[test]
fn select_and_start_bound_to_reset_offers_undo_reset_in_the_menu() {
    let mut keys = ShortcutKeys::default();
    keys.set(
        Shortcut::Reset,
        Binding::with(Modifier::Select, Device::Start),
    );
    let (mut s, _d, mut now) = session_playing_with(&keys);
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Start)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Start)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    for _ in 0..30 {
        step(&mut s, &mut now, &[]);
    }
    assert_eq!(s.app().undo_label(), Some("undo reset"));
    open_menu(&mut s, &mut now);
    assert!(!s.app().in_game_hidden().contains(&QuickRow::UndoReset));
}
