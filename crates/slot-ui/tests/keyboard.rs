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
    assert_eq!(selected(&kb), "Shift");
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
fn menu_shows_help_and_any_button_closes_it() {
    let mut kb = general("a");
    assert_eq!(kb.press(Btn::Menu), Typed::Help);
    assert_eq!(kb.help_top(), Some(0));
    assert_eq!(kb.press(Btn::A), Typed::Changed);
    assert_eq!(kb.help_top(), None);
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
