mod common;

use std::path::Path;

use common::{app_booting_at, tmp_root_with_carts, write_gb_cart, Clock, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{read_slot_state, write_slot_state, KidMode, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

fn put(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_gb_cart(&d, "Alleyway", "ALLEYWAY");
    put(d.path(), "Collections/Puzzlers.txt", "gba/Fusion\n");
    put(d.path(), "Collections/order.txt", "Puzzlers\n");
    d
}

fn boot_on(d: &TempDir, state: SlotState) -> (App, Clock) {
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..state
        },
    )
    .expect("write slot.state");
    app_booting_at(d.path(), CLOCK_IS_SET)
}

fn kid(mode: KidMode) -> SlotState {
    SlotState {
        kid_mode: mode,
        ..SlotState::default()
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open_at(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    let mut path = vec![row];
    while let Some(parent) = path.last().and_then(|r| r.parent()) {
        path.push(parent);
    }
    for (depth, step) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(a, Btn::A);
        }
        for _ in 0..step.position(&a.hidden_rows()) {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.quick_menu(), Some(row), "the bar never reached {row:?}");
}

fn saved(d: &TempDir) -> SlotState {
    read_slot_state(d.path())
}

#[test]
fn turning_kid_mode_on_asks_first() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Right);
    assert_eq!(a.kid_prompt(), Some(KidMode::OneShelf));
    assert!(!a.kid_locked());
    assert_eq!(saved(&d).kid_mode, KidMode::Off);
}

#[test]
fn confirming_locks_and_lands_on_the_shelf() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert!(a.kid_locked());
    assert_eq!(a.kid_prompt(), None);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(saved(&d).kid_mode, KidMode::OneShelf);
}

#[test]
fn left_from_off_asks_for_shelves() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Left);
    assert_eq!(a.kid_prompt(), Some(KidMode::Shelves));
}

#[test]
fn b_on_the_prompt_leaves_kid_mode_off() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::B);
    assert_eq!(a.kid_prompt(), None);
    assert!(!a.kid_locked());
    assert_eq!(a.quick_menu(), Some(QuickRow::KidMode));
    assert_eq!(saved(&d).kid_mode, KidMode::Off);
}

#[test]
fn a_kid_mode_card_boots_locked() {
    let d = card();
    let (a, _clock) = boot_on(&d, kid(KidMode::Shelves));
    assert!(a.kid_locked());
}

#[test]
fn unlocked_steps_between_modes_without_asking() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::OneShelf));
    a.unlock_kid_mode();
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Right);
    assert_eq!(a.kid_prompt(), None);
    assert_eq!(saved(&d).kid_mode, KidMode::Shelves);
    press(&mut a, Btn::Right);
    assert_eq!(saved(&d).kid_mode, KidMode::Off);
    assert!(!a.kid_locked());
}

#[test]
fn kid_shelf_shows_only_for_one_shelf() {
    for (mode, hidden) in [
        (KidMode::Off, true),
        (KidMode::OneShelf, false),
        (KidMode::Shelves, true),
    ] {
        let d = card();
        let (a, _clock) = boot_on(&d, kid(mode));
        assert_eq!(
            a.hidden_rows().contains(&QuickRow::KidShelf),
            hidden,
            "{mode:?}"
        );
    }
}

#[test]
fn kid_shelf_steps_through_all_games_then_each_shelf() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::OneShelf));
    a.unlock_kid_mode();
    let choices = a.kid_shelf_choices();
    assert_eq!(choices[0], "all");
    for key in ["collection/Puzzlers", "gba", "gb"] {
        assert!(
            choices.iter().any(|c| c == key),
            "{key} missing: {choices:?}"
        );
    }
    assert_eq!(a.kid_shelf_text(), "All Games");
    open_at(&mut a, QuickRow::KidShelf);
    press(&mut a, Btn::Right);
    assert_eq!(saved(&d).kid_shelf, choices[1]);
    press(&mut a, Btn::Left);
    press(&mut a, Btn::Left);
    assert_eq!(saved(&d).kid_shelf, *choices.last().unwrap());
}

#[test]
fn kid_shelf_names_a_collection_by_its_name() {
    let d = card();
    let (mut a, _clock) = boot_on(
        &d,
        SlotState {
            kid_mode: KidMode::OneShelf,
            kid_shelf: "collection/Puzzlers".into(),
            ..SlotState::default()
        },
    );
    a.unlock_kid_mode();
    assert_eq!(a.kid_shelf_text(), "Puzzlers");
}

fn favorites(d: &TempDir) -> usize {
    saved(d).favorites.len()
}

fn locked_on_shelf(mode: KidMode) -> (TempDir, App, Clock) {
    let d = card();
    let (a, clock) = boot_on(&d, kid(mode));
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    (d, a, clock)
}

fn playing(d: &TempDir, stem: &str, mode: KidMode) -> App {
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some(stem.to_string()),
            clock_set: true,
            kid_mode: mode,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let mut a = App::boot(d.path());
    a.set_snapshot(common::StubSnapshot::boxed());
    a.on_core_ready();
    settle(&mut a);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    a
}

fn settle(a: &mut App) {
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
}

#[test]
fn a_locked_shelf_ignores_every_editing_button() {
    let (d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    let before = favorites(&d);
    common::through(&mut a, Btn::Y);
    common::hold(&mut a, Btn::Y, 600);
    common::through(&mut a, Btn::X);
    common::through(&mut a, Btn::Select);
    common::hold(&mut a, Btn::Select, 800);
    common::through(&mut a, Btn::Start);
    a.apply(Action::QuickMenu);
    assert_eq!(favorites(&d), before);
    assert!(a.edit().is_none(), "{:?}", a.edit());
    assert!(a.core_picker().is_none());
    assert!(a.studio().is_none());
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn a_locked_shelf_still_scrolls_and_plays() {
    let (_d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    let first = a.selected_stem().map(str::to_string);
    press(&mut a, Btn::Right);
    settle(&mut a);
    assert_ne!(a.selected_stem().map(str::to_string), first);
    press(&mut a, Btn::A);
    assert!(!matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn shelves_mode_still_switches_shelves() {
    let (_d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    let first = a.shelf_platform_name().map(str::to_string);
    common::through(&mut a, Btn::R1);
    assert_ne!(a.shelf_platform_name().map(str::to_string), first);
}

#[test]
fn menu_in_a_locked_game_ejects_to_the_shelf() {
    let d = card();
    let mut a = playing(&d, "Emerald", KidMode::Shelves);
    a.apply(Action::QuickMenu);
    assert!(a.in_game_menu().is_none());
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
}

#[test]
fn a_locked_game_drops_every_shortcut() {
    let d = card();
    let mut a = playing(&d, "Emerald", KidMode::Shelves);
    let lit = a.brightness();
    common::chord(&mut a, Btn::Select, Btn::R1);
    common::chord(&mut a, Btn::Select, Btn::Up);
    common::chord(&mut a, Btn::Select, Btn::Down);
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::GameMenu);
    a.apply(Action::Eject);
    settle(&mut a);
    assert_eq!(a.brightness(), lit);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    let states = d.path().join("States");
    let saved_any = std::fs::read_dir(&states)
        .map(|mut r| r.next().is_some())
        .unwrap_or(false)
        && walk(&states).iter().any(|p| p.is_file());
    assert!(!saved_any, "a state was saved");
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn l2_and_r2_step_brightness_in_a_locked_game() {
    let d = card();
    let mut a = playing(&d, "Emerald", KidMode::Shelves);
    let lit = a.brightness();
    common::through(&mut a, Btn::L2);
    assert_eq!(a.brightness(), lit - 1);
    common::through(&mut a, Btn::R2);
    assert_eq!(a.brightness(), lit);
    common::hold(&mut a, Btn::L2, 600);
    assert_eq!(a.brightness(), lit - 1);
}

#[test]
fn a_game_bound_to_l2_keeps_it() {
    let d = card();
    let mut controls = slot_store::Controls::default();
    controls.set_game(
        slot_store::GameButton::TurboA,
        slot_store::Binding::plain(slot_store::Device::L2),
    );
    slot_store::write_controls(d.path(), &controls).unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            turbo: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = {
        let mut s = read_slot_state(d.path());
        s.cart = Some("Emerald".into());
        s.clock_set = true;
        s.kid_mode = KidMode::Shelves;
        write_slot_state(d.path(), &s).unwrap();
        let mut a = App::boot(d.path());
        a.set_snapshot(common::StubSnapshot::boxed());
        a.on_core_ready();
        settle(&mut a);
        a
    };
    assert!(a
        .binding_table()
        .entries()
        .iter()
        .all(|e| e.action != Action::BrightnessDown));
    let lit = a.brightness();
    common::through(&mut a, Btn::L2);
    assert_eq!(a.brightness(), lit);
}

#[test]
fn single_cart_menu_does_nothing_and_makes_no_sound() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = playing(&d, "Emerald", KidMode::Shelves);
    assert!(a.single_cart());
    a.apply(Action::QuickMenu);
    assert!(a.in_game_menu().is_none(), "the in-game menu opened");
    assert!(!a.refusal_active(a.now()), "the menu refused out loud");
    settle(&mut a);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
}

#[test]
fn the_allow_list_keeps_only_what_the_spec_keeps() {
    use slot::kid::{allowed, KidWhere::*};
    assert_eq!(
        allowed(Playing, KidMode::Shelves, Action::QuickMenu),
        Some(Action::Eject)
    );
    assert_eq!(allowed(Playing, KidMode::Shelves, Action::Eject), None);
    assert_eq!(
        allowed(Playing, KidMode::Shelves, Action::RewindStart),
        None
    );
    assert_eq!(allowed(Playing, KidMode::Shelves, Action::FfStart), None);
    assert_eq!(
        allowed(Playing, KidMode::Shelves, Action::BrightnessUp),
        Some(Action::BrightnessUp)
    );
    assert_eq!(allowed(Shelf, KidMode::OneShelf, Action::NextShelf), None);
    assert_eq!(
        allowed(Shelf, KidMode::Shelves, Action::NextShelf),
        Some(Action::NextShelf)
    );
    assert_eq!(
        allowed(Shelf, KidMode::Shelves, Action::GbaDown(Btn::Start)),
        None
    );
    assert_eq!(allowed(Shelf, KidMode::Shelves, Action::QuickMenu), None);
    assert_eq!(allowed(Shelf, KidMode::Shelves, Action::BrightnessUp), None);
    assert_eq!(
        allowed(Shelf, KidMode::Shelves, Action::VolumeUp),
        Some(Action::VolumeUp)
    );
    assert_eq!(
        allowed(Other, KidMode::Shelves, Action::GbaDown(Btn::A)),
        Some(Action::GbaDown(Btn::A))
    );
    assert_eq!(allowed(Other, KidMode::Shelves, Action::SelectTap), None);
}

fn one_shelf(d: &TempDir, shelf: &str) -> App {
    let (a, _clock) = boot_on(
        d,
        SlotState {
            kid_mode: KidMode::OneShelf,
            kid_shelf: shelf.to_string(),
            ..SlotState::default()
        },
    );
    a
}

#[test]
fn all_games_puts_every_platform_on_one_shelf_a_to_z() {
    let d = card();
    write_gb_cart(&d, "Zelda", "ZELDA");
    let a = one_shelf(&d, "all");
    assert_eq!(a.shelf_key(), "all");
    assert_eq!(a.shelf_stems(), ["Alleyway", "Emerald", "Fusion", "Zelda"]);
}

#[test]
fn one_shelf_ignores_the_shoulders() {
    let d = card();
    let mut a = one_shelf(&d, "all");
    for btn in [Btn::L1, Btn::R1, Btn::L2, Btn::R2] {
        common::through(&mut a, btn);
        assert_eq!(a.shelf_key(), "all", "{btn:?}");
    }
}

#[test]
fn one_shelf_can_be_a_collection() {
    let d = card();
    let a = one_shelf(&d, "collection/Puzzlers");
    assert_eq!(a.shelf_key(), "collection/Puzzlers");
    assert_eq!(a.shelf_stems(), ["Fusion"]);
}

#[test]
fn a_missing_kid_shelf_falls_back_to_all_games_and_is_kept() {
    let d = card();
    let a = one_shelf(&d, "collection/Gone");
    assert_eq!(a.shelf_key(), "all");
    assert_eq!(saved(&d).kid_shelf, "collection/Gone");
}

fn cycle_keys(a: &mut App) -> Vec<String> {
    let mut seen = Vec::new();
    for _ in 0..10 {
        seen.push(a.shelf_key());
        common::through(a, Btn::R1);
    }
    seen
}

#[test]
fn shelves_mode_never_shows_all_games() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::Shelves));
    assert!(!cycle_keys(&mut a).iter().any(|k| k == "all"));
}

#[test]
fn unlocked_never_shows_all_games() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::OneShelf));
    a.unlock_kid_mode();
    let keys = cycle_keys(&mut a);
    assert!(!keys.iter().any(|k| k == "all"), "{keys:?}");
    assert!(keys.iter().any(|k| k == "gba"), "{keys:?}");
}

#[test]
fn confirming_one_shelf_lands_on_the_kid_shelf() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    open_at(&mut a, QuickRow::KidMode);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert_eq!(a.shelf_key(), "all");
}

#[test]
fn ejecting_a_locked_resume_lands_on_the_kid_shelf() {
    let d = card();
    let mut a = playing(&d, "Emerald", KidMode::OneShelf);
    a.apply(Action::QuickMenu);
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_key(), "all");
    assert_eq!(a.selected_stem(), Some("Emerald"));
}

#[test]
fn the_chord_needs_all_four() {
    use slot::kid::unlock_chord;
    assert!(unlock_chord(&[Btn::Start, Btn::Select, Btn::L2, Btn::R2]));
    assert!(unlock_chord(&[
        Btn::R2,
        Btn::A,
        Btn::L2,
        Btn::Select,
        Btn::Start
    ]));
    assert!(!unlock_chord(&[Btn::Start, Btn::Select, Btn::L2]));
    assert!(!unlock_chord(&[Btn::Start, Btn::Select, Btn::X, Btn::Y]));
    assert!(!unlock_chord(&[]));
}

#[test]
fn unlocking_opens_settings_until_restart() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::Shelves));
    a.unlock_kid_mode();
    assert_eq!(a.toast(), Some(slot_ui::Toast::KidUnlocked));
    assert!(!a.kid_locked());
    a.apply(Action::QuickMenu);
    assert!(a.quick_menu().is_some());
    drop(a);
    let (a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(a.kid_locked());
    assert_eq!(saved(&d).kid_mode, KidMode::Shelves);
}

#[test]
fn unlocking_does_nothing_when_kid_mode_is_off() {
    let d = card();
    let (mut a, _clock) = boot_on(&d, SlotState::default());
    a.unlock_kid_mode();
    assert_eq!(a.toast(), None);
}

#[test]
fn an_unlocked_resume_keeps_the_game_on_its_own_shelf() {
    let d = card();
    let mut a = playing(&d, "Alleyway", KidMode::OneShelf);
    assert_eq!(a.shelf_key(), "all");
    a.unlock_kid_mode();
    assert_eq!(a.playing_platform(), slot_store::Platform::Gb);
    assert_eq!(a.shelf_key(), "gb");
    a.apply(Action::Eject);
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_key(), "gb");
    assert_eq!(a.selected_stem(), Some("Alleyway"));
}

#[test]
fn ejecting_a_cart_off_the_kid_collection_lands_on_the_kid_shelf() {
    let d = card();
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            kid_mode: KidMode::OneShelf,
            kid_shelf: "collection/Puzzlers".into(),
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    a.set_snapshot(common::StubSnapshot::boxed());
    a.on_core_ready();
    settle(&mut a);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    a.apply(Action::QuickMenu);
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert_eq!(a.shelf_key(), "collection/Puzzlers");
    assert_eq!(a.shelf_stems(), ["Fusion"]);
}

#[test]
fn a_locked_clock_screen_only_confirms() {
    let d = card();
    write_slot_state(
        d.path(),
        &SlotState {
            kid_mode: KidMode::Shelves,
            ..SlotState::default()
        },
    )
    .unwrap();
    let before = saved(&d);
    let mut a = App::boot(d.path());
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "{:?}",
        a.phase()
    );
    for btn in [Btn::Down, Btn::Down, Btn::Right, Btn::Down, Btn::Right] {
        press(&mut a, btn);
    }
    press(&mut a, Btn::B);
    let after = saved(&d);
    assert_eq!(after.zone, before.zone);
    assert_eq!(after.clock_24h, before.clock_24h);
}

#[test]
fn holding_a_in_kid_mode_still_resumes() {
    let d = card();
    slot_store::StateRing::new(
        d.path(),
        slot_store::Platform::Gba,
        slot_store::Core::Mgba,
        "Emerald",
    )
    .write_resume(&[7u8; 64])
    .unwrap();
    let (mut a, _clock) = boot_on(&d, kid(KidMode::Shelves));
    for _ in 0..4 {
        if a.selected_stem() == Some("Emerald") {
            break;
        }
        press(&mut a, Btn::Right);
        settle(&mut a);
    }
    assert_eq!(a.selected_stem(), Some("Emerald"));
    a.apply_at(Action::GbaDown(Btn::A), 0);
    a.tick_ms(500);
    assert!(!a.starting_clean(), "holding A threw the kid's game away");
}

fn with_all_wallpaper(mode: KidMode) -> (TempDir, App) {
    let d = card();
    put(d.path(), "Wallpapers/ALL.png", "png");
    put(d.path(), "Wallpapers/GBA.png", "png");
    let (a, _clock) = boot_on(
        &d,
        SlotState {
            kid_mode: mode,
            wallpaper: slot_store::WallpaperPick::Shelf,
            ..SlotState::default()
        },
    );
    (d, a)
}

#[test]
fn all_png_is_the_all_games_wallpaper() {
    let (_d, a) = with_all_wallpaper(KidMode::OneShelf);
    assert!(
        a.shelf_wallpapers().iter().any(|w| w == "ALL.png"),
        "{:?}",
        a.shelf_wallpapers()
    );
}

#[test]
fn all_png_is_not_loaded_when_all_games_cannot_show() {
    let (_d, a) = with_all_wallpaper(KidMode::Off);
    let walls = a.shelf_wallpapers();
    assert!(!walls.iter().any(|w| w == "ALL.png"), "{walls:?}");
    assert!(walls.iter().any(|w| w == "GBA.png"), "{walls:?}");
}

#[test]
fn one_shelf_l2_and_r2_step_brightness_on_the_shelf() {
    let d = card();
    let mut a = one_shelf(&d, "all");
    let lit = a.brightness();
    common::through(&mut a, Btn::L2);
    assert_eq!(a.brightness(), lit - 1);
    common::through(&mut a, Btn::R2);
    assert_eq!(a.brightness(), lit);
    assert_eq!(a.shelf_key(), "all");
}

#[test]
fn shelves_mode_keeps_l2_and_r2_for_collections() {
    let (_d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    let lit = a.brightness();
    common::through(&mut a, Btn::L2);
    common::through(&mut a, Btn::R2);
    assert_eq!(a.brightness(), lit);
}

const KID_TEX: slot_ui::TexId = slot_ui::TexId::from_raw(9_001);

fn shows_the_kid_icon(a: &App) -> bool {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter()
        .any(|d| matches!(d, slot_ui::Draw::Tex { tex, .. } if *tex == KID_TEX))
}

#[test]
fn a_locked_shelf_shows_the_kid_icon() {
    let (_d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    a.set_kid_icon_faces([(KID_TEX, 20, 20); 2]);
    assert!(shows_the_kid_icon(&a));
}

#[test]
fn an_unlocked_shelf_has_no_kid_icon() {
    let (_d, mut a, _clock) = locked_on_shelf(KidMode::Shelves);
    a.set_kid_icon_faces([(KID_TEX, 20, 20); 2]);
    a.unlock_kid_mode();
    assert!(!shows_the_kid_icon(&a));
}
