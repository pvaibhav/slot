use slot_store::{initial, sort_key};

#[test]
fn titles_file_digits_first_then_a_to_z_whatever_their_case() {
    let mut names = vec![
        "Zebra",
        "apple",
        "3D Pinball",
        "Metroid",
        "1943",
        "banana",
        "Apotris",
    ];
    names.sort_by_key(|n| sort_key(n));
    assert_eq!(
        names,
        vec![
            "1943",
            "3D Pinball",
            "Apotris",
            "apple",
            "banana",
            "Metroid",
            "Zebra"
        ]
    );
}

#[test]
fn a_title_led_by_punctuation_files_last() {
    let mut names = vec!["[BIOS] Test", "Apotris", "1943"];
    names.sort_by_key(|n| sort_key(n));
    assert_eq!(names, vec!["1943", "Apotris", "[BIOS] Test"]);
}

#[test]
fn digits_and_punctuation_share_one_stop() {
    assert_eq!(initial("1943"), '#');
    assert_eq!(initial("3D Pinball"), '#');
    assert_eq!(initial("[BIOS] Test"), '#');
    assert_eq!(initial("apple"), 'A');
    assert_eq!(initial("Apotris"), 'A');
    assert_eq!(initial("  Metroid"), 'M', "leading space hid the letter");
}

use slot_store::{Cart, Platform};
use slot_ui::Shelf;

fn shelf_of(names: &[&str]) -> Shelf {
    Shelf::new(
        names
            .iter()
            .map(|n| Cart {
                platform: Platform::Gba,
                stem: (*n).to_string(),
                rom: format!("Games/GBA/{n}.gba").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: n.to_uppercase(),
                sealed: false,
            })
            .collect(),
    )
}

const ROW: [&str; 7] = [
    "1943",
    "Apotris",
    "Advance Wars",
    "Metroid",
    "Mario Kart",
    "Zelda",
    "Zzz",
];

#[test]
fn down_crosses_to_the_next_letter() {
    let mut s = shelf_of(&ROW);
    assert_eq!(s.index, 0);
    s.jump_next_letter();
    assert_eq!(s.carts[s.index].stem, "Apotris");
    s.jump_next_letter();
    assert_eq!(s.carts[s.index].stem, "Metroid");
    s.jump_next_letter();
    assert_eq!(s.carts[s.index].stem, "Zelda");
}

#[test]
fn up_lands_on_the_start_of_the_letter_before_leaving_it() {
    let mut s = shelf_of(&ROW);
    s.select(4);
    s.jump_prev_letter();
    assert_eq!(s.carts[s.index].stem, "Metroid", "it left the Ms too early");
    s.jump_prev_letter();
    assert_eq!(s.carts[s.index].stem, "Apotris");
}

#[test]
fn the_letters_wrap_at_both_ends() {
    let mut s = shelf_of(&ROW);
    s.select(5);
    s.jump_next_letter();
    assert_eq!(
        s.carts[s.index].stem, "1943",
        "the end did not loop forward"
    );

    s.select(0);
    s.jump_prev_letter();
    assert_eq!(
        s.carts[s.index].stem, "Zelda",
        "the start did not loop back"
    );
}

#[test]
fn a_wrap_travels_the_way_the_press_asked() {
    let mut s = shelf_of(&ROW);
    s.select(5);
    let before = s.scroll_target();
    s.jump_next_letter();
    assert!(
        s.scroll_target() > before,
        "Down at the end turned round instead of carrying on forwards"
    );

    s.select(0);
    let before = s.scroll_target();
    s.jump_prev_letter();
    assert!(
        s.scroll_target() < before,
        "Up at the start turned round instead of carrying on backwards"
    );
}

#[test]
fn a_row_of_one_letter_stays_put() {
    let mut s = shelf_of(&["Metroid", "Mario Kart"]);
    s.jump_next_letter();
    assert_eq!(
        s.carts[s.index].stem, "Metroid",
        "it moved within one letter"
    );
    s.jump_prev_letter();
    assert_eq!(s.carts[s.index].stem, "Metroid");
}

