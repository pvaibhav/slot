use slot::studio::{Effect, LabelKind, Legend, Row, Screen, Studio, Target};
use slot_input::Btn;
use slot_store::{Cart, Outline, Platform, ShellChoice, ShellFinish};

fn cart(platform: Platform, art: bool) -> Cart {
    Cart {
        platform,
        stem: "Probe".into(),
        rom: "Games/GBA/Probe.gba".into(),
        label: art.then(|| "Labels/GBA/Probe.png".into()),
        title: "PROBE".into(),
        code: String::new(),
        shell: None,
    }
}

fn gba() -> Studio {
    Studio::new(cart(Platform::Gba, true), true)
}

fn to_row(s: &mut Studio, row: Row) {
    while s.row() != Some(row) {
        s.press(Btn::Down);
    }
}

#[test]
fn rows_for_a_gba_cart() {
    assert_eq!(gba().rows(), vec![Row::Label, Row::Color, Row::Finish]);
}

#[test]
fn shape_only_on_game_boy() {
    let s = Studio::new(cart(Platform::Gb, true), true);
    assert_eq!(s.rows(), vec![Row::Label, Row::Color, Row::Finish, Row::Shape]);
}

#[test]
fn no_label_row_without_wifi() {
    let s = Studio::new(cart(Platform::Gba, true), false);
    assert_eq!(s.rows(), vec![Row::Color, Row::Finish]);
}

#[test]
fn artless_sheet_is_two() {
    let s = Studio::new(cart(Platform::Gba, false), true);
    assert_eq!(s.sheet, vec![LabelKind::Real, LabelKind::Logo]);
}

#[test]
fn a_sheet_always_has_two() {
    let mut s = gba();
    assert_eq!(s.sheet, vec![LabelKind::Real, LabelKind::Logo]);
    s.press(Btn::A);
    s.art_arrived(true, true);
    s.press(Btn::A);
    s.applied(LabelKind::Real);
    assert_eq!(s.sheet, vec![LabelKind::Existing, LabelKind::Logo]);
    s.press(Btn::Right);
    s.press(Btn::A);
    s.applied(LabelKind::Logo);
    assert_eq!(s.sheet, vec![LabelKind::Real, LabelKind::Existing]);
}

#[test]
fn a_opens_the_sheet_and_asks_for_art() {
    let mut s = gba();
    assert_eq!(s.press(Btn::A), Some(Effect::Fetch));
    assert!(matches!(s.screen, Screen::Sheet { .. }));
}

#[test]
fn apply_swaps_the_label_back() {
    let mut s = Studio::new(cart(Platform::Gba, false), true);
    s.press(Btn::A);
    s.art_arrived(true, true);
    s.press(Btn::Right);
    assert_eq!(s.press(Btn::A), Some(Effect::Apply(LabelKind::Logo)));
    s.applied(LabelKind::Logo);
    assert_eq!(s.wearing, LabelKind::Logo);
    assert_eq!(s.sheet, vec![LabelKind::Existing, LabelKind::Real]);
    assert_eq!(s.press(Btn::A), Some(Effect::Apply(LabelKind::Existing)));
    s.applied(LabelKind::Existing);
    assert_eq!(s.sheet, vec![LabelKind::Logo, LabelKind::Real]);
}

#[test]
fn label_color_only_on_logo() {
    let mut s = gba();
    assert!(!s.rows().contains(&Row::LabelColor));
    s.press(Btn::A);
    s.art_arrived(true, true);
    s.press(Btn::Right);
    s.press(Btn::A);
    s.applied(LabelKind::Logo);
    assert!(s.rows().contains(&Row::LabelColor));
}

#[test]
fn a_preset_sets_the_finish() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    assert!(matches!(s.screen, Screen::Chips { target: Target::Shell, .. }));
    let clear = slot_ui::palette::chip_grid(0.0, 0.0)
        .iter()
        .position(|p| match p.chip {
            slot_ui::palette::Chip::Preset(i) => {
                slot_ui::shell_presets()[i].0 == "Kirby pink"
            }
            _ => false,
        })
        .unwrap();
    s.chips_jump(clear);
    assert_eq!(s.shell.unwrap().finish, ShellFinish::Clear);
    assert_eq!(s.shell.unwrap().colour, [0xec, 0x94, 0xb4]);
}

#[test]
fn b_on_chips_keeps_the_highlight() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(5);
    let held = s.shell;
    s.press(Btn::B);
    assert!(matches!(s.screen, Screen::List { .. }));
    assert_eq!(s.shell, held);
    assert!(held.is_some());
}

fn custom(s: &mut Studio) {
    to_row(s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(2);
    s.press(Btn::A);
    assert!(matches!(s.screen, Screen::Custom { .. }));
}

#[test]
fn custom_keeps_the_finish() {
    let mut s = gba();
    to_row(&mut s, Row::Finish);
    s.press(Btn::Right);
    let finish = s.shell.unwrap().finish;
    custom(&mut s);
    s.press(Btn::Right);
    assert_eq!(s.shell.unwrap().finish, finish);
}

#[test]
fn reset_returns_to_start() {
    let mut s = gba();
    custom(&mut s);
    let Screen::Custom { start, .. } = s.screen else { panic!() };
    s.press(Btn::Right);
    s.press(Btn::Right);
    s.press(Btn::X);
    let Screen::Custom { rgb, .. } = s.screen else { panic!() };
    assert_eq!(rgb, start);
}

#[test]
fn hex_rolls_and_wraps() {
    let mut s = gba();
    custom(&mut s);
    s.press(Btn::Y);
    assert!(matches!(s.screen, Screen::Hex { .. }));
    let Screen::Hex { digits, .. } = s.screen else { panic!() };
    for _ in 0..16 {
        s.press(Btn::Up);
    }
    let Screen::Hex { digits: after, .. } = s.screen else { panic!() };
    assert_eq!(digits, after);
    s.press(Btn::Right);
    s.press(Btn::Down);
    let Screen::Hex { digits: down, pos, .. } = s.screen else { panic!() };
    assert_eq!(pos, 1);
    assert_eq!(down[1], (digits[1] + 15) % 16);
}

#[test]
fn hex_back_sets_the_sliders() {
    let mut s = gba();
    custom(&mut s);
    s.press(Btn::Y);
    s.press(Btn::Up);
    let Screen::Hex { digits, .. } = s.screen else { panic!() };
    s.press(Btn::B);
    let Screen::Custom { rgb, .. } = s.screen else { panic!() };
    assert_eq!(rgb[0], digits[0] * 16 + digits[1]);
}

#[test]
fn shape_cycles_on_game_boy() {
    let mut s = Studio::new(cart(Platform::Gb, true), true);
    to_row(&mut s, Row::Shape);
    s.press(Btn::Right);
    assert_eq!(s.shell.unwrap().outline, Outline::Notched);
    s.press(Btn::Right);
    assert_eq!(s.shell.unwrap().outline, Outline::Rounded);
}

#[test]
fn auto_clears_the_shell() {
    let mut s = Studio::new(
        Cart {
            shell: Some(ShellChoice {
                outline: Outline::Auto,
                colour: [1, 2, 3],
                finish: ShellFinish::Solid,
            }),
            ..cart(Platform::Gba, true)
        },
        true,
    );
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(1);
    assert_eq!(s.shell, None);
    s.chips_jump(0);
    assert_eq!(s.shell.unwrap().colour, [1, 2, 3]);
}

#[test]
fn legends_follow_the_screen() {
    let mut s = gba();
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Open]);
    to_row(&mut s, Row::Finish);
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Change]);
    to_row(&mut s, Row::Color);
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Open]);
    s.press(Btn::A);
    assert_eq!(s.legends(), vec![Legend::Back]);
    let custom = slot_ui::palette::chip_grid(0.0, 0.0)
        .iter()
        .position(|p| p.chip == slot_ui::palette::Chip::Custom)
        .unwrap();
    s.screen = Screen::Chips { target: Target::Shell, at: custom };
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Open]);
    s.chips_jump(2);
    s.press(Btn::A);
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Change, Legend::Reset, Legend::Hex]);
    s.press(Btn::Y);
    assert_eq!(s.legends(), vec![Legend::Back, Legend::Move, Legend::ChangeVertical, Legend::Reset]);
}

#[test]
fn b_on_the_list_closes() {
    assert_eq!(gba().press(Btn::B), Some(Effect::Close));
}

#[test]
fn fetched_labels_wait_for_the_art() {
    let mut s = gba();
    s.press(Btn::A);
    assert_eq!(s.press(Btn::A), None);
    s.art_arrived(true, false);
    assert_eq!(s.press(Btn::A), Some(Effect::Apply(LabelKind::Real)));
    s.applied(LabelKind::Real);
}

#[test]
fn one_option_on_a_cart_with_art_adds_the_text_label() {
    let mut s = gba();
    s.art_arrived(false, true);
    assert_eq!(s.sheet, vec![LabelKind::Logo, LabelKind::Title]);
}

#[test]
fn a_cart_wearing_its_only_art_keeps_the_text_label_on_offer() {
    let mut s = gba();
    s.duplicates = true;
    s.art_arrived(false, false);
    assert_eq!(s.sheet, vec![LabelKind::Title]);
    s.press(Btn::A);
    assert_eq!(s.press(Btn::A), Some(Effect::Apply(LabelKind::Title)));
}

#[test]
fn no_art_at_all_still_offers_the_text_label() {
    let mut s = gba();
    s.art_arrived(false, false);
    assert_eq!(s.sheet, vec![LabelKind::Title]);
}

#[test]
fn the_text_label_is_not_offered_while_it_is_worn() {
    let mut s = gba();
    s.adopt_text([7, 8, 9]);
    s.art_arrived(false, false);
    assert!(s.sheet.is_empty());
}

#[test]
fn an_artless_cart_never_gets_the_text_label() {
    let mut s = Studio::new(cart(Platform::Gba, false), true);
    s.art_arrived(true, false);
    assert_eq!(s.sheet, vec![LabelKind::Real]);
}

#[test]
fn two_options_stay_two() {
    let mut s = gba();
    s.art_arrived(true, true);
    assert_eq!(s.sheet, vec![LabelKind::Real, LabelKind::Logo]);
}

mod tray {
    use slot::studio::{tray_step, LabelKind, Slip};

    const H: f32 = 183.0;
    const BOTTOM: f32 = 189.0;

    fn settle(slips: &mut Vec<Slip>, sheet: &[LabelKind], ready: &dyn Fn(LabelKind) -> bool) -> usize {
        let mut now = 0.0;
        let mut prints = 0;
        for _ in 0..200 {
            let before = slips.len();
            tray_step(slips, sheet, ready, now, H, BOTTOM);
            if slips.len() > before {
                prints += 1;
            }
            now += 50.0;
        }
        prints
    }

    fn at(slips: &[Slip], kind: LabelKind) -> f32 {
        slips.iter().find(|s| s.kind == kind).unwrap().y1
    }

    #[test]
    fn the_bottom_sticker_prints_first_and_drops() {
        let mut slips = Vec::new();
        let sheet = [LabelKind::Real, LabelKind::Logo];
        tray_step(&mut slips, &sheet, &|_| true, 0.0, H, BOTTOM);
        assert_eq!(slips[0].kind, LabelKind::Logo);
        assert_eq!(settle(&mut slips, &sheet, &|_| true), 1);
        assert_eq!(at(&slips, LabelKind::Logo), BOTTOM);
        assert_eq!(at(&slips, LabelKind::Real), 0.0);
    }

    #[test]
    fn a_swap_prints_one_slip() {
        let mut slips = Vec::new();
        settle(&mut slips, &[LabelKind::Real, LabelKind::Logo], &|_| true);
        let sheet = [LabelKind::Existing, LabelKind::Logo];
        assert_eq!(settle(&mut slips, &sheet, &|_| true), 1);
        assert_eq!(slips.len(), 2);
        assert_eq!(at(&slips, LabelKind::Logo), BOTTOM);
        assert_eq!(at(&slips, LabelKind::Existing), 0.0);
    }

    #[test]
    fn using_the_bottom_slip_drops_the_top_one() {
        let mut slips = Vec::new();
        settle(&mut slips, &[LabelKind::Real, LabelKind::Logo], &|_| true);
        let sheet = [LabelKind::Existing, LabelKind::Real];
        assert_eq!(settle(&mut slips, &sheet, &|_| true), 1);
        assert_eq!(at(&slips, LabelKind::Real), BOTTOM);
    }

    #[test]
    fn nothing_prints_before_its_art() {
        let mut slips = Vec::new();
        let sheet = [LabelKind::Real, LabelKind::Logo];
        assert_eq!(settle(&mut slips, &sheet, &|_| false), 0);
    }
}

#[test]
fn the_light_blinks_while_waiting() {
    use slot::studio::{printer_light, Light};
    assert_eq!(printer_light(true, false, 0.0), Light::Printing);
    assert_eq!(printer_light(false, false, 0.0), Light::Off);
    assert_eq!(printer_light(false, true, 100.0), Light::Waiting);
    assert_eq!(printer_light(false, true, 500.0), Light::Off);
    assert_eq!(printer_light(false, true, 900.0), Light::Waiting);
}

#[test]
fn the_picked_sticker_zooms_in_quickly() {
    use slot::studio::{slip_zoom, SLIP_ZOOM};
    assert_eq!(slip_zoom(0.0), 1.0);
    assert!(slip_zoom(60.0) > 1.0 && slip_zoom(60.0) < SLIP_ZOOM);
    assert_eq!(slip_zoom(120.0), SLIP_ZOOM);
    assert_eq!(slip_zoom(5000.0), SLIP_ZOOM);
}

#[test]
fn the_used_backing_leaves_to_the_right() {
    use slot::studio::backing_shift;
    assert_eq!(backing_shift(0.0, 300.0), 0.0);
    assert!(backing_shift(0.5, 300.0) < 150.0);
    assert_eq!(backing_shift(1.0, 300.0), 300.0);
}

#[test]
fn a_shuffle_pulls_apart_and_returns() {
    use slot::studio::slip_shuffle;
    assert_eq!(slip_shuffle(0.0), 0.0);
    assert!((slip_shuffle(110.0) - 1.0).abs() < 0.001);
    assert!(slip_shuffle(220.0).abs() < 0.001);
    assert_eq!(slip_shuffle(5000.0), 0.0);
}

mod worn_logo {
    use super::*;

    #[test]
    fn label_color_waits_for_the_logo_image() {
        let mut s = gba();
        s.adopt_logo([1, 2, 3]);
        assert!(!s.rows().contains(&Row::LabelColor));
        s.wheel_arrived();
        assert!(s.rows().contains(&Row::LabelColor));
        assert_eq!(s.ground, Some([1, 2, 3]));
    }

    #[test]
    fn the_row_arriving_keeps_the_selection() {
        let mut s = gba();
        s.adopt_logo([1, 2, 3]);
        s.press(Btn::Down);
        let was = s.row();
        s.wheel_arrived();
        assert_eq!(s.row(), was);
    }

    #[test]
    fn an_unchanged_logo_has_nothing_to_save() {
        let mut s = gba();
        s.adopt_logo([1, 2, 3]);
        s.wheel_arrived();
        assert!(!s.recolored());
        s.ground = Some([4, 5, 6]);
        assert!(s.recolored());
    }

    #[test]
    fn a_real_label_is_not_recolored() {
        let mut s = gba();
        s.wheel_arrived();
        assert!(!s.rows().contains(&Row::LabelColor));
        s.ground = Some([4, 5, 6]);
        assert!(!s.recolored());
    }

    #[test]
    fn applying_another_label_drops_the_row() {
        let mut s = gba();
        s.adopt_logo([1, 2, 3]);
        s.wheel_arrived();
        s.art_arrived(true, false);
        s.applied(LabelKind::Real);
        assert!(!s.rows().contains(&Row::LabelColor));
        assert!(!s.recolored());
    }

    #[test]
    fn applying_a_fresh_logo_starts_from_its_own_color() {
        let mut s = gba();
        s.adopt_logo([1, 2, 3]);
        s.applied(LabelKind::Logo);
        assert_eq!(s.ground, None);
        assert!(s.rows().contains(&Row::LabelColor));
    }
}

mod text_label {
    use super::*;

    fn bare() -> Studio {
        Studio::new(cart(Platform::Gba, false), true)
    }

    #[test]
    fn an_artless_cart_gets_label_color_with_no_art() {
        assert!(bare().rows().contains(&Row::LabelColor));
    }

    #[test]
    fn it_needs_no_wifi() {
        let s = Studio::new(cart(Platform::Gba, false), false);
        assert!(s.rows().contains(&Row::LabelColor));
    }

    #[test]
    fn it_starts_on_the_automatic_color_with_nothing_to_save() {
        let s = bare();
        assert!(s.ground.is_some());
        assert!(!s.recolored());
    }

    #[test]
    fn changing_it_shows_the_text_face_and_saves() {
        let mut s = bare();
        s.ground = Some([1, 2, 3]);
        assert!(s.recolored());
        assert!(s.wearing_text());
        assert_eq!(s.shown_label(), LabelKind::Title);
    }

    #[test]
    fn a_saved_text_label_opens_on_its_color_and_is_not_offered_again() {
        let mut s = gba();
        s.adopt_text([7, 8, 9]);
        assert!(s.rows().contains(&Row::LabelColor));
        assert_eq!(s.ground, Some([7, 8, 9]));
        s.art_arrived(true, false);
        assert_eq!(s.sheet, vec![LabelKind::Real]);
    }

    #[test]
    fn applying_the_text_sticker_gets_the_row() {
        let mut s = gba();
        s.applied(LabelKind::Title);
        assert!(s.rows().contains(&Row::LabelColor));
        assert!(s.wearing_text());
    }

    #[test]
    fn a_real_label_is_not_text() {
        assert!(!gba().wearing_text());
    }
}

#[test]
fn the_mac_build_can_pretend_to_have_wifi() {
    let d = tempfile::tempdir().unwrap();
    assert!(!slot::update::wifi_conf(d.path()));
    unsafe { std::env::set_var("SLOT_WIFI", "1") };
    assert!(slot::update::wifi_conf(d.path()));
    unsafe { std::env::remove_var("SLOT_WIFI") };
}

#[test]
fn signal_reads_the_driver_and_becomes_bars() {
    use slot::update::{parse_rssi, signal_bars};
    assert_eq!(parse_rssi("rssi:-55\nsignal_strength:45\nsignal_qual:100\n"), Some(-55));
    assert_eq!(parse_rssi("nothing here"), None);
    assert_eq!(signal_bars(-55), 3);
    assert_eq!(signal_bars(-60), 3);
    assert_eq!(signal_bars(-61), 2);
    assert_eq!(signal_bars(-72), 2);
    assert_eq!(signal_bars(-73), 1);
    assert_eq!(signal_bars(-95), 1);
}

#[test]
fn only_a_tinted_label_passes_its_color_to_the_previews() {
    let mut s = Studio::new(cart(Platform::Gba, false), true);
    assert!(s.preview_ground().is_some());
    s.applied(LabelKind::Real);
    assert_eq!(s.preview_ground(), None);
}

#[test]
fn the_cursor_stays_on_the_sheet_when_art_arrives() {
    let mut s = Studio::new(cart(Platform::Gba, false), true);
    s.press(Btn::A);
    s.press(Btn::Right);
    assert_eq!(s.screen, Screen::Sheet { at: 1 });
    s.art_arrived(true, false);
    assert_eq!(s.screen, Screen::Sheet { at: 0 });
}

#[test]
fn browsing_past_auto_keeps_the_shape() {
    let mut c = cart(Platform::Gb, true);
    c.shell = Some(ShellChoice {
        outline: Outline::Rounded,
        colour: [0xec, 0x94, 0xb4],
        finish: ShellFinish::Glitter,
    });
    let mut s = Studio::new(c, true);
    s.screen = Screen::Chips { target: Target::Shell, at: 0 };
    for _ in 0..3 {
        s.press(Btn::Right);
        assert_eq!(s.current_shell().outline, Outline::Rounded);
    }
    s.press(Btn::Left);
    s.press(Btn::Left);
    s.press(Btn::Left);
    assert_eq!(s.shell, s.cart.shell);
}

#[test]
fn the_print_light_outlasts_the_print() {
    use slot::studio::{Slip, PRINT_LIGHT_HOLD_MS};
    let slip = Slip { kind: LabelKind::Real, y0: -100.0, y1: 0.0, t0: 1000.0, ms: 300.0 };
    assert!(slip.lit(1100.0));
    assert!(!slip.printing(1350.0));
    assert!(slip.lit(1350.0));
    assert!(slip.lit(1300.0 + PRINT_LIGHT_HOLD_MS - 1.0));
    assert!(!slip.lit(1300.0 + PRINT_LIGHT_HOLD_MS));
    let settled = Slip { y0: 40.0, y1: 0.0, ..slip };
    assert!(!settled.lit(1100.0));
}

#[test]
fn reset_undoes_custom_without_saving_a_color() {
    let mut s = Studio::new(cart(Platform::Gb, true), true);
    s.screen = Screen::Chips { target: Target::Shell, at: 1 };
    s.chips_jump(1);
    assert_eq!(s.shell, None);
    s.chips_jump(2);
    s.press(Btn::A);
    s.nudge(5);
    assert!(s.shell.is_some());
    s.press(Btn::X);
    assert_eq!(s.shell, None);
    s.press(Btn::Y);
    s.press(Btn::Up);
    assert!(s.shell.is_some());
    s.press(Btn::X);
    assert_eq!(s.shell, None);
}

#[test]
fn reset_puts_an_automatic_label_color_back() {
    let mut s = gba();
    s.screen = Screen::Chips { target: Target::Label, at: 2 };
    s.press(Btn::A);
    s.nudge(5);
    assert!(s.ground.is_some());
    s.press(Btn::X);
    assert_eq!(s.ground, None);
}

fn chip_at(chip: slot_ui::palette::Chip) -> usize {
    slot_ui::palette::chip_grid(0.0, 0.0)
        .iter()
        .position(|p| p.chip == chip)
        .unwrap()
}

fn worn(index: usize) -> Studio {
    let preset = slot_ui::shell_presets()[index].1;
    let mut c = cart(Platform::Gba, true);
    c.shell = Some(ShellChoice {
        outline: Outline::Auto,
        colour: preset.colour,
        finish: ShellFinish::Solid,
    });
    Studio::new(c, true)
}

#[test]
fn the_shell_picker_starts_on_the_preset_the_cart_wears() {
    use slot_ui::palette::Chip;
    let mut s = worn(7);
    let before = s.shell;
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    assert_eq!(
        s.screen,
        Screen::Chips { target: Target::Shell, at: chip_at(Chip::Preset(7)) }
    );
    assert_eq!(s.shell, before);
}

#[test]
fn the_shell_picker_starts_on_current_for_a_cart_on_auto_or_a_custom_color() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    assert_eq!(s.screen, Screen::Chips { target: Target::Shell, at: 0 });

    let mut c = cart(Platform::Gba, true);
    c.shell = Some(ShellChoice {
        outline: Outline::Auto,
        colour: [1, 2, 3],
        finish: ShellFinish::Solid,
    });
    let mut s = Studio::new(c, true);
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    assert_eq!(s.screen, Screen::Chips { target: Target::Shell, at: 0 });
}

#[test]
fn the_shell_picker_reopens_on_what_was_just_picked() {
    use slot_ui::palette::Chip;
    let mut s = worn(7);
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(chip_at(Chip::Preset(3)));
    s.press(Btn::B);
    s.press(Btn::A);
    assert_eq!(
        s.screen,
        Screen::Chips { target: Target::Shell, at: chip_at(Chip::Preset(3)) }
    );
    s.chips_jump(chip_at(Chip::Auto));
    s.press(Btn::B);
    s.press(Btn::A);
    assert_eq!(
        s.screen,
        Screen::Chips { target: Target::Shell, at: chip_at(Chip::Auto) }
    );
}

#[test]
fn the_label_picker_starts_on_the_preset_in_use() {
    use slot_ui::palette::Chip;
    let mut s = Studio::new(cart(Platform::Gba, false), true);
    s.screen = Screen::Chips { target: Target::Label, at: 0 };
    s.chips_jump(chip_at(Chip::Preset(3)));
    s.press(Btn::B);
    s.press(Btn::A);
    assert_eq!(
        s.screen,
        Screen::Chips { target: Target::Label, at: chip_at(Chip::Preset(3)) }
    );
}
