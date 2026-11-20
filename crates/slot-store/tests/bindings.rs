use slot_store::{
    read_carousel, read_controls, read_shortcuts, write_carousel, write_controls, write_shortcuts,
    Binding, CarouselAction, CarouselKeys, Controls, Device, GameButton, Modifier, Platform,
    Shortcut, ShortcutKeys,
};

fn card() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    d
}

fn plain(button: Device) -> Option<Binding> {
    Some(Binding {
        modifier: None,
        button,
    })
}

fn with(modifier: Modifier, button: Device) -> Option<Binding> {
    Some(Binding {
        modifier: Some(modifier),
        button,
    })
}

#[test]
fn a_gba_has_twelve_buttons_and_a_game_boy_ten() {
    assert_eq!(GameButton::of(Platform::Gba).len(), 12);
    assert_eq!(GameButton::of(Platform::Gb).len(), 10);
    assert_eq!(GameButton::of(Platform::Gbc).len(), 10);
    assert!(!GameButton::of(Platform::Gb).contains(&GameButton::L));
    assert!(GameButton::of(Platform::Gb).contains(&GameButton::TurboA));
    assert!(GameButton::of(Platform::Gbc).contains(&GameButton::TurboB));
}

#[test]
fn every_console_button_starts_on_the_device_button_of_its_name() {
    let c = Controls::default();
    assert_eq!(c.game(GameButton::A), plain(Device::A));
    assert_eq!(c.game(GameButton::L), plain(Device::L1));
    assert_eq!(c.game(GameButton::R), plain(Device::R1));
    assert_eq!(c.game(GameButton::Select), plain(Device::Select));
    assert_eq!(c.game(GameButton::TurboA), plain(Device::X));
    assert_eq!(c.game(GameButton::TurboB), plain(Device::Y));
}

#[test]
fn the_shortcuts_start_where_they_are_today() {
    let c = ShortcutKeys::default();
    assert_eq!(
        c.get(Shortcut::SaveState),
        with(Modifier::Select, Device::R1)
    );
    assert_eq!(
        c.get(Shortcut::LoadState),
        with(Modifier::Select, Device::L1)
    );
    assert_eq!(
        c.get(Shortcut::BlueLightDown),
        with(Modifier::Select, Device::Left)
    );
    assert_eq!(c.get(Shortcut::FastForward), plain(Device::R2));
    assert_eq!(c.get(Shortcut::Rewind), plain(Device::L2));
}

#[test]
fn a_binding_reads_short_enough_to_fit_a_4_by_3_row() {
    assert_eq!(Binding::text(plain(Device::A)), "A");
    assert_eq!(Binding::text(with(Modifier::Menu, Device::X)), "MENU+X");
    assert_eq!(Binding::text(with(Modifier::Select, Device::R1)), "SEL+R1");
    assert_eq!(Binding::text(None), "NONE");
}

#[test]
fn the_buttons_are_one_set_written_and_read_back() {
    let d = card();
    assert_eq!(read_controls(d.path()), Controls::default());
    let mut c = Controls::default();
    c.set_game(GameButton::A, plain(Device::B));
    c.set_game(GameButton::Start, None);
    c.set_game(GameButton::L, with(Modifier::Menu, Device::X));
    write_controls(d.path(), &c).unwrap();
    assert_eq!(read_controls(d.path()), c);
    assert!(d.path().join("Config/controls.txt").is_file());
}

#[test]
fn a_hand_written_file_reads_spaces_any_case_and_skips_what_it_cannot() {
    let d = card();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(
        d.path().join("Config/controls.txt"),
        "A = Menu+X\nstart=none\nb=nonsense\nnot_a_key=a\n",
    )
    .unwrap();
    let c = read_controls(d.path());
    assert_eq!(c.game(GameButton::A), with(Modifier::Menu, Device::X));
    assert_eq!(c.game(GameButton::Start), None);
    assert_eq!(c.game(GameButton::B), plain(Device::B));
}

#[test]
fn shortcuts_are_one_set_for_every_console_and_read_back() {
    let d = card();
    assert_eq!(read_shortcuts(d.path()), ShortcutKeys::default());
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::Rewind, None);
    keys.set(Shortcut::SaveState, with(Modifier::Menu, Device::R1));
    write_shortcuts(d.path(), &keys).unwrap();
    assert_eq!(read_shortcuts(d.path()), keys);
    assert!(d.path().join("Config/shortcuts.txt").is_file());
}

#[test]
fn the_carousel_starts_on_the_shoulders_y_and_x_and_reads_back() {
    let d = card();
    let keys = CarouselKeys::default();
    assert_eq!(keys.get(CarouselAction::PreviousShelf), plain(Device::L1));
    assert_eq!(keys.get(CarouselAction::NextShelf), plain(Device::R1));
    assert_eq!(keys.get(CarouselAction::Favorite), plain(Device::Y));
    assert_eq!(keys.get(CarouselAction::ForgetRecent), plain(Device::X));
    assert_eq!(read_carousel(d.path()), keys);
    let mut moved = keys;
    moved.set(CarouselAction::PreviousShelf, plain(Device::Y));
    moved.set(CarouselAction::Favorite, with(Modifier::Menu, Device::Y));
    write_carousel(d.path(), &moved).unwrap();
    assert_eq!(read_carousel(d.path()), moved);
}

#[test]
fn a_pair_is_the_same_binding_in_either_order() {
    assert_eq!(
        Binding::pair(Device::R1, Device::L1),
        Binding::pair(Device::L1, Device::R1)
    );
    assert_eq!(
        Binding::pair(Device::L1, Device::R1),
        with(Modifier::Button(Device::L1), Device::R1)
    );
    assert_eq!(
        Binding::text(Binding::pair(Device::R1, Device::L1)),
        "L1+R1"
    );
}

#[test]
fn a_pair_with_select_is_a_select_chord() {
    assert_eq!(
        Binding::pair(Device::A, Device::Select),
        with(Modifier::Select, Device::A)
    );
    assert_eq!(
        Binding::pair(Device::Select, Device::A),
        with(Modifier::Select, Device::A)
    );
}

#[test]
fn start_leads_any_pair_it_is_in() {
    assert_eq!(
        Binding::pair(Device::A, Device::Start),
        with(Modifier::Button(Device::Start), Device::A)
    );
    assert_eq!(
        Binding::pair(Device::Start, Device::A),
        with(Modifier::Button(Device::Start), Device::A)
    );
    assert_eq!(
        Binding::text(Binding::pair(Device::A, Device::Start)),
        "START+A"
    );
    assert!(Device::Start.leads() && Device::Select.leads() && !Device::A.leads());
}

#[test]
fn a_button_paired_with_itself_is_that_button() {
    assert_eq!(Binding::pair(Device::B, Device::B), plain(Device::B));
}

#[test]
fn pairs_are_written_and_read_back_in_either_order() {
    let d = card();
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::SaveState, Binding::pair(Device::L1, Device::R1));
    keys.set(Shortcut::LoadState, Binding::pair(Device::Start, Device::A));
    write_shortcuts(d.path(), &keys).unwrap();
    assert_eq!(read_shortcuts(d.path()), keys);
    std::fs::write(
        d.path().join("Config/shortcuts.txt"),
        "save_state=r1+l1\nload_state=A + Select\n",
    )
    .unwrap();
    let read = read_shortcuts(d.path());
    assert_eq!(
        read.get(Shortcut::SaveState),
        Binding::pair(Device::L1, Device::R1)
    );
    assert_eq!(
        read.get(Shortcut::LoadState),
        with(Modifier::Select, Device::A)
    );
}

#[test]
fn screenshot_defaults_to_select_x() {
    let c = ShortcutKeys::default();
    assert_eq!(
        c.get(Shortcut::Screenshot),
        with(Modifier::Select, Device::X)
    );
    assert_eq!(Shortcut::Screenshot.label(), "Screenshot");
    for other in Shortcut::ALL
        .into_iter()
        .filter(|s| *s != Shortcut::Screenshot)
    {
        assert_ne!(
            c.get(other),
            c.get(Shortcut::Screenshot),
            "{other:?} starts on the screenshot chord"
        );
    }
}

#[test]
fn a_screenshot_binding_round_trips_through_the_file() {
    let d = card();
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::Screenshot, with(Modifier::Select, Device::L2));
    write_shortcuts(d.path(), &keys).unwrap();
    assert_eq!(
        read_shortcuts(d.path()).get(Shortcut::Screenshot),
        with(Modifier::Select, Device::L2)
    );
    let text = std::fs::read_to_string(d.path().join("Config/shortcuts.txt")).unwrap();
    assert!(text.contains("screenshot"), "{text}");
}

#[test]
fn fast_forward_toggle_starts_unbound_and_round_trips_through_the_file() {
    assert_eq!(
        ShortcutKeys::default().get(Shortcut::FastForwardToggle),
        None
    );
    assert_eq!(Shortcut::FastForwardToggle.label(), "FF Toggle");
    let d = card();
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::FastForwardToggle, plain(Device::R1));
    write_shortcuts(d.path(), &keys).unwrap();
    assert_eq!(
        read_shortcuts(d.path()).get(Shortcut::FastForwardToggle),
        plain(Device::R1)
    );
    let text = std::fs::read_to_string(d.path().join("Config/shortcuts.txt")).unwrap();
    assert!(text.contains("fast_forward_toggle=r1"), "{text}");
}

#[test]
fn the_two_fast_forwards_never_share_a_binding() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::FastForwardToggle, plain(Device::R2));
    assert_eq!(keys.get(Shortcut::FastForwardToggle), plain(Device::R2));
    assert_eq!(keys.get(Shortcut::FastForward), None);
    keys.set(Shortcut::FastForward, plain(Device::R2));
    assert_eq!(keys.get(Shortcut::FastForward), plain(Device::R2));
    assert_eq!(keys.get(Shortcut::FastForwardToggle), None);
}

#[test]
fn other_shortcuts_may_still_share_a_binding() {
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::Rewind, plain(Device::R2));
    assert_eq!(keys.get(Shortcut::Rewind), plain(Device::R2));
    assert_eq!(keys.get(Shortcut::FastForward), plain(Device::R2));
    keys.set(Shortcut::FastForwardToggle, plain(Device::L1));
    keys.set(Shortcut::FastForward, plain(Device::L1));
    keys.set(Shortcut::FastForwardToggle, None);
    assert_eq!(keys.get(Shortcut::FastForward), plain(Device::L1));
}

#[test]
fn the_sunlight_shortcuts_start_unbound_and_round_trip_through_the_file() {
    for (shortcut, label, key) in [
        (Shortcut::SunlightUp, "Sunlight Up", "sunlight_up=r1"),
        (Shortcut::SunlightDown, "Sunlight Down", "sunlight_down=r1"),
    ] {
        assert_eq!(ShortcutKeys::default().get(shortcut), None);
        assert_eq!(shortcut.label(), label);
        let d = card();
        let mut keys = ShortcutKeys::default();
        keys.set(shortcut, plain(Device::R1));
        write_shortcuts(d.path(), &keys).unwrap();
        assert_eq!(read_shortcuts(d.path()).get(shortcut), plain(Device::R1));
        let text = std::fs::read_to_string(d.path().join("Config/shortcuts.txt")).unwrap();
        assert!(text.contains(key), "{text}");
    }
}

#[test]
fn reset_starts_unbound_and_round_trips_through_the_file() {
    assert_eq!(ShortcutKeys::default().get(Shortcut::Reset), None);
    assert_eq!(Shortcut::Reset.label(), "Reset Game");
    let d = card();
    let mut keys = ShortcutKeys::default();
    keys.set(Shortcut::Reset, plain(Device::R1));
    write_shortcuts(d.path(), &keys).unwrap();
    assert_eq!(
        read_shortcuts(d.path()).get(Shortcut::Reset),
        plain(Device::R1)
    );
    let text = std::fs::read_to_string(d.path().join("Config/shortcuts.txt")).unwrap();
    assert!(text.contains("reset=r1"), "{text}");
}

#[test]
fn collection_rows_default_to_the_triggers() {
    let keys = CarouselKeys::default();
    assert_eq!(
        keys.get(CarouselAction::PreviousCollection),
        plain(Device::L2)
    );
    assert_eq!(keys.get(CarouselAction::NextCollection), plain(Device::R2));
    assert_eq!(CarouselAction::ALL.len(), 6);
}

#[test]
fn an_old_carousel_file_gets_the_collection_defaults() {
    let d = card();
    std::fs::write(
        d.path().join("Config/carousel.txt"),
        "previous_shelf=l1\nnext_shelf=r1\nfavorite=y\nremove_recent=x\n",
    )
    .unwrap();
    assert_eq!(read_carousel(d.path()), CarouselKeys::default());
}

#[test]
fn collection_rows_round_trip() {
    let d = card();
    let mut keys = CarouselKeys::default();
    keys.set(
        CarouselAction::NextCollection,
        with(Modifier::Select, Device::Left),
    );
    write_carousel(d.path(), &keys).unwrap();
    let back = read_carousel(d.path());
    assert_eq!(
        back.get(CarouselAction::NextCollection),
        with(Modifier::Select, Device::Left)
    );
    assert_eq!(back, keys);
}
