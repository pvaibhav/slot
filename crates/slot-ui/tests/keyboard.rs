use slot_input::Btn;
use slot_ui::{Cap, Keyboard, Layout, Special, Typed};

fn general(text: &str) -> Keyboard {
    Keyboard::new(Layout::General, "T", None, text)
}

fn selected(kb: &Keyboard) -> String {
    kb.label(kb.rows()[kb.row()][kb.col()])
}

fn press(kb: &mut Keyboard, buttons: &[Btn]) {
    for b in buttons {
        kb.press(*b);
    }
}

fn text(kb: &Keyboard) -> String {
    kb.text()
}

fn select(kb: &mut Keyboard, label: &str) {
    for r in 0..kb.rows().len() {
        while kb.row() != r {
            press(kb, &[Btn::Down]);
        }
        for _ in 0..kb.rows()[r].len() {
            if selected(kb) == label {
                return;
            }
            press(kb, &[Btn::Right]);
        }
    }
    panic!("no key {label}");
}

#[test]
fn starts_on_q_with_the_cursor_at_the_end() {
    let kb = general("ab");
    assert_eq!(selected(&kb), "q");
    assert_eq!(kb.cursor(), 2);
}

#[test]
fn numeric_starts_on_7() {
    let kb = Keyboard::new(Layout::Numeric, "T", None, "");
    assert_eq!(selected(&kb), "7");
}

#[test]
fn a_types_the_selected_key_at_the_cursor() {
    let mut kb = general("ac");
    press(&mut kb, &[Btn::L1]);
    select(&mut kb, "b");
    assert_eq!(kb.press(Btn::A), Typed::Changed);
    assert_eq!(text(&kb), "abc");
    assert_eq!(kb.cursor(), 2);
}

#[test]
fn every_direction_wraps() {
    let mut kb = general("");
    select(&mut kb, "p");
    press(&mut kb, &[Btn::Right]);
    assert_eq!(selected(&kb), "q");
    press(&mut kb, &[Btn::Left]);
    assert_eq!(selected(&kb), "p");
    select(&mut kb, "1");
    press(&mut kb, &[Btn::Up]);
    assert_eq!(selected(&kb), "Space");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "1");
}

#[test]
fn vertical_moves_clamp_onto_short_rows() {
    let mut kb = general("");
    select(&mut kb, "Delete");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "p");
    select(&mut kb, "m");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "Space");
    press(&mut kb, &[Btn::Up]);
    assert_eq!(selected(&kb), "m");
}

#[test]
fn up_and_down_stay_in_line_across_the_staggered_rows() {
    let mut kb = general("");
    select(&mut kb, "d");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "z");
    press(&mut kb, &[Btn::Up]);
    assert_eq!(selected(&kb), "d");
    select(&mut kb, "b");
    press(&mut kb, &[Btn::Up]);
    assert_eq!(selected(&kb), "j");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "b");
    select(&mut kb, "a");
    press(&mut kb, &[Btn::Down]);
    assert_eq!(selected(&kb), "Shift");
}

#[test]
fn a_trip_through_the_space_bar_comes_back_to_the_same_key() {
    let mut kb = general("");
    select(&mut kb, "x");
    press(&mut kb, &[Btn::Down, Btn::Up]);
    assert_eq!(selected(&kb), "x");
    select(&mut kb, "1");
    press(&mut kb, &[Btn::Up, Btn::Down]);
    assert_eq!(selected(&kb), "1");
}

#[test]
fn moving_sideways_starts_a_new_line() {
    let mut kb = general("");
    select(&mut kb, "x");
    press(&mut kb, &[Btn::Down, Btn::Up, Btn::Right, Btn::Up]);
    assert_eq!(selected(&kb), "g");
}

#[test]
fn shift_gives_capitals_and_digit_symbols() {
    let mut kb = general("");
    press(&mut kb, &[Btn::Select]);
    assert_eq!(selected(&kb), "Q");
    select(&mut kb, "!");
    press(&mut kb, &[Btn::A]);
    select(&mut kb, "Q");
    press(&mut kb, &[Btn::A, Btn::A]);
    assert_eq!(text(&kb), "!QQ");
    select(&mut kb, "Shift");
    press(&mut kb, &[Btn::A]);
    assert_eq!(selected(&kb), "Shift");
    select(&mut kb, "q");
}

#[test]
fn symbols_switch_every_key() {
    let mut kb = general("");
    select(&mut kb, "#+=");
    press(&mut kb, &[Btn::A]);
    let row = |kb: &Keyboard, r: usize| {
        kb.rows()[r]
            .iter()
            .map(|c| kb.label(*c))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        row(&kb, 0)[..10],
        ["!", "@", "#", "$", "%", "^", "&", "*", "(", ")"]
    );
    assert_eq!(
        row(&kb, 1),
        ["`", "~", "[", "]", "\\", "|", "{", "}", ";", ":"]
    );
    assert_eq!(
        row(&kb, 2)[..9],
        ["'", "\"", "<", ">", "?", "/", "+", "=", "_"]
    );
    assert_eq!(
        row(&kb, 3),
        ["Shift", ",", ".", "-", "\u{20ac}", "\u{a3}", "\u{a5}", "\u{a2}", "abc"]
    );
}

#[test]
fn b_backspaces_and_x_types_a_space() {
    let mut kb = general("ab");
    assert_eq!(kb.press(Btn::B), Typed::Changed);
    assert_eq!(text(&kb), "a");
    press(&mut kb, &[Btn::X]);
    assert_eq!(text(&kb), "a ");
}

#[test]
fn backspace_at_the_start_does_nothing() {
    let mut kb = general("ab");
    press(&mut kb, &[Btn::L1, Btn::L1]);
    assert_eq!(kb.press(Btn::B), Typed::Nothing);
    assert_eq!(text(&kb), "ab");
}

#[test]
fn cursor_stops_at_both_ends() {
    let mut kb = general("ab");
    assert_eq!(kb.press(Btn::R1), Typed::Nothing);
    assert_eq!(kb.cursor(), 2);
    press(&mut kb, &[Btn::L1, Btn::L1, Btn::L1]);
    assert_eq!(kb.cursor(), 0);
}

#[test]
fn multibyte_characters_edit_cleanly() {
    let mut kb = general("ab");
    press(&mut kb, &[Btn::L1]);
    select(&mut kb, "#+=");
    press(&mut kb, &[Btn::A]);
    select(&mut kb, "\u{20ac}");
    press(&mut kb, &[Btn::A]);
    assert_eq!(text(&kb), "a\u{20ac}b");
    press(&mut kb, &[Btn::B]);
    assert_eq!(text(&kb), "ab");
    assert_eq!(kb.cursor(), 1);
}

#[test]
fn start_and_done_return_the_text_untrimmed() {
    let mut kb = general(" a ");
    assert_eq!(kb.press(Btn::Start), Typed::Done(" a ".into()));
    let mut kb = general(" a ");
    select(&mut kb, "Done");
    assert_eq!(kb.press(Btn::A), Typed::Done(" a ".into()));
}

#[test]
fn y_cancels() {
    assert_eq!(general("a").press(Btn::Y), Typed::Cancelled);
}

#[test]
fn menu_opens_help_and_only_b_or_menu_closes_it() {
    let mut kb = general("a");
    assert_eq!(kb.press(Btn::Menu), Typed::Help);
    assert!(kb.help());
    for b in [Btn::A, Btn::X, Btn::Y, Btn::Start, Btn::Up, Btn::Select] {
        assert_eq!(kb.press(b), Typed::Nothing, "{b:?}");
        assert!(kb.help(), "{b:?} closed the help");
    }
    assert_eq!(kb.press(Btn::B), Typed::Changed);
    assert!(!kb.help());
    assert_eq!(text(&kb), "a");
    kb.press(Btn::Menu);
    assert_eq!(kb.press(Btn::Menu), Typed::Changed);
    assert!(!kb.help());
    assert_eq!(text(&kb), "a");
}

#[test]
fn x_does_nothing_on_the_numeric_pad() {
    let mut kb = Keyboard::new(Layout::Numeric, "T", None, "1");
    assert_eq!(kb.press(Btn::X), Typed::Nothing);
    assert_eq!(kb.press(Btn::Select), Typed::Nothing);
    assert_eq!(text(&kb), "1");
}

#[test]
fn the_numeric_pad_is_gabagools() {
    let kb = Keyboard::new(Layout::Numeric, "T", None, "");
    let rows: Vec<Vec<String>> = kb
        .rows()
        .iter()
        .map(|r| r.iter().map(|c| kb.label(*c)).collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec!["7", "8", "9", "Delete"],
            vec!["4", "5", "6", "Done"],
            vec!["1", "2", "3"],
            vec!["0"]
        ]
    );
}

#[test]
fn the_symbols_key_reads_hash_plus_equals() {
    let mut kb = general("");
    assert_eq!(kb.label(Cap::Special(Special::Symbols)), "#+=");
    select(&mut kb, "#+=");
    press(&mut kb, &[Btn::A]);
    assert_eq!(kb.label(Cap::Special(Special::Symbols)), "abc");
}

#[test]
fn the_help_fits_between_the_plates_and_inside_a_640_screen() {
    for layout in [Layout::General, Layout::Numeric] {
        let face = slot_ui::keyboard_help_face(layout);
        assert!(
            face.h as f32 <= 480.0 - 2.0 * slot_ui::PLATE_H,
            "{layout:?}: {}",
            face.h
        );
        assert!(face.w <= 600, "{layout:?}: {}", face.w);
    }
}

#[test]
fn every_key_fits_both_screens() {
    for panel in [(640, 480), (720, 480)] {
        let canvas = slot_ui::Canvas::of(panel);
        for layout in [Layout::General, Layout::Numeric] {
            let kb = Keyboard::new(layout, "T", None, "");
            for (r, row) in kb.rows().iter().enumerate() {
                let mut last = f32::MIN;
                for c in 0..row.len() {
                    let [x, _, w, _] = slot_ui::key_rect(canvas, layout, r, c);
                    assert!(
                        x >= 16.0 && x + w <= canvas.w - 16.0,
                        "{panel:?} {layout:?} key {r},{c} at {x}+{w}"
                    );
                    assert!(x > last, "{panel:?} {layout:?} key {r},{c} overlaps");
                    last = x + w;
                }
            }
        }
    }
}

#[test]
fn the_wide_screen_keeps_its_key_size() {
    let canvas = slot_ui::Canvas::of((720, 480));
    let [_, _, w, h] = slot_ui::key_rect(canvas, Layout::General, 1, 0);
    assert_eq!((w, h), (50.0, 44.0));
}

#[test]
fn the_help_lists_only_the_shortcuts_in_two_columns() {
    let labels = |layout| {
        slot_ui::keyboard_help_columns(layout)
            .map(|c| c.iter().map(|s| s.label()).collect::<Vec<_>>())
    };
    assert_eq!(
        labels(Layout::General),
        [
            vec!["Delete", "Space", "Shift"],
            vec!["Cursor", "Cancel", "Done"]
        ]
    );
    assert_eq!(
        labels(Layout::Numeric),
        [vec!["Delete", "Cursor"], vec!["Cancel", "Done"]]
    );
}

#[test]
fn host_layout_types_dots_and_dashes_without_the_symbols_page() {
    let mut kb = Keyboard::new(Layout::Host, "T", None, "");
    press(&mut kb, &[Btn::Select]);
    let first: Vec<String> = kb.rows()[0][..10].iter().map(|c| kb.label(*c)).collect();
    assert_eq!(first.concat(), ".-_:/~@+=,");
    select(&mut kb, ".");
    press(&mut kb, &[Btn::A]);
    assert_eq!(text(&kb), ".");
}

#[test]
fn numeric_keeps_starting_text() {
    let kb = Keyboard::new(Layout::Numeric, "T", None, "443");
    assert_eq!(text(&kb), "443");
    assert_eq!(kb.cursor(), 3);
}

fn alpha_columns(face: &slot_ui::UndoFace, upto: usize) -> Vec<Vec<u8>> {
    (0..upto)
        .map(|x| {
            (0..face.h as usize)
                .map(|y| face.rgba[(y * face.w as usize + x) * 4 + 3])
                .collect()
        })
        .collect()
}

#[test]
fn the_readout_keeps_earlier_letters_in_place_as_more_are_typed() {
    let words = [
        "romm",
        "romm.example",
        "romm.example.com",
        "hello world",
        "https://romm.local:8080",
    ];
    for word in words {
        let chars: Vec<char> = word.chars().collect();
        for keep in 2..chars.len() {
            let prefix: String = chars[..keep].iter().collect();
            let (short, short_stops) = slot_ui::keyboard_text_face(&general(&prefix));
            let (long, _) = slot_ui::keyboard_text_face(&general(word));
            let upto = short_stops[keep - 1].floor() as usize;
            let (a, b) = (alpha_columns(&short, upto), alpha_columns(&long, upto));
            assert!(a == b, "{prefix:?} moved inside {word:?}");
        }
    }
}

#[test]
fn the_caret_stops_follow_the_pen_from_the_left_edge() {
    let (_, stops) = slot_ui::keyboard_text_face(&general("romm"));
    assert_eq!(stops.len(), 5);
    assert!(stops[0] >= 0.0 && stops[0] < 2.0);
    assert!(stops.windows(2).all(|p| p[1] > p[0]));
}

fn brightest(face: &slot_ui::UndoFace) -> u8 {
    face.rgba
        .chunks(4)
        .filter(|p| p[3] > 200)
        .map(|p| p[0].min(p[1]).min(p[2]))
        .max()
        .unwrap_or(0)
}

#[test]
fn a_refusal_reads_brighter_and_larger_than_a_hint() {
    let hint = slot_ui::Keyboard::new(
        slot_ui::Layout::General,
        "New Collection",
        Some("Name taken"),
        "",
    );
    let alert = hint.clone().alerted();
    let canvas = slot_ui::Canvas::FRAMED;
    let a = slot_ui::keyboard_faces(&alert, canvas).subtitle.unwrap();
    let h = slot_ui::keyboard_faces(&hint, canvas).subtitle.unwrap();
    assert!(brightest(&a) > 0xe0, "{}", brightest(&a));
    assert!(brightest(&a) > brightest(&h));
    assert!(a.w > h.w);
}
