mod common;

use std::path::Path;

use common::{tmp_root_with_carts, write_gb_cart};
use slot::app::{App, Phase};
use slot_gfx::{Draw, TexId};
use slot_input::{Action, Btn, FAVORITE_HOLD_MS};
use slot_store::{read_slot_state, write_slot_state, Platform, Recent, SlotState};
use slot_ui::{QuickRow, QuickValue};
use tempfile::TempDir;

const TITLE: usize = 900;
const RACK: usize = 901;

fn card() -> TempDir {
    let d = tmp_root_with_carts(&["Emerald (USA, Europe)", "Fusion"]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    d
}

fn boot(root: &Path, show_titles: bool) -> App {
    write_slot_state(
        root,
        &SlotState {
            clock_set: true,
            show_titles,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let mut a = App::boot(root);
    a.set_shelf_title_face(TexId::from_raw(TITLE), 120);
    a.set_shelf_platform_face(TexId::from_raw(RACK), 80);
    a
}

fn tick(a: &mut App, secs: f32) {
    let steps = (secs * 60.0).round() as usize;
    for _ in 0..steps {
        a.update(1.0 / 60.0);
    }
}

fn press(a: &mut App, btn: Btn) {
    common::through(a, btn);
}

fn title_alpha(a: &App) -> Option<f32> {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter().find_map(|d| match *d {
        Draw::Tex { tex, alpha, .. } if tex == TexId::from_raw(TITLE) => Some(alpha),
        _ => None,
    })
}

#[test]
fn the_title_is_the_selected_carts_cleaned_file_name() {
    let d = card();
    let mut a = boot(d.path(), true);
    tick(&mut a, 3.0);
    assert_eq!(a.title_text().as_deref(), Some("Emerald"));
    press(&mut a, Btn::Right);
    assert_eq!(a.title_text().as_deref(), Some("Fusion"));
}

#[test]
fn with_titles_off_there_is_no_title() {
    let d = card();
    let mut a = boot(d.path(), false);
    tick(&mut a, 3.0);
    assert_eq!(a.title_text(), None);
    assert_eq!(title_alpha(&a), None, "a title was drawn with titles off");
}

#[test]
fn show_titles_is_flipped_from_the_shelf_page_and_saved() {
    let d = card();
    let mut a = boot(d.path(), false);
    tick(&mut a, 3.0);
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::General.position(&[]) {
        press(&mut a, Btn::Down);
    }
    press(&mut a, Btn::A);
    for _ in 0..QuickRow::ShowTitles.position(&a.hidden_rows()) {
        press(&mut a, Btn::Down);
    }
    assert_eq!(a.quick_menu(), Some(QuickRow::ShowTitles));
    assert_eq!(a.quick_value(QuickRow::ShowTitles), Some(QuickValue::Hide));
    press(&mut a, Btn::Right);
    assert_eq!(a.quick_value(QuickRow::ShowTitles), Some(QuickValue::Show));
    assert!(
        read_slot_state(d.path()).show_titles,
        "the choice was not saved"
    );
}

#[test]
fn the_rack_name_fades_into_the_title_on_a_rack_change() {
    let d = card();
    let mut a = boot(d.path(), true);
    tick(&mut a, 4.0);
    assert_eq!(
        title_alpha(&a),
        Some(1.0),
        "the title is not up on a settled shelf"
    );

    press(&mut a, Btn::R1);
    assert!(
        title_alpha(&a).unwrap_or(0.0) == 0.0,
        "the title stayed up while the rack name was on its way"
    );
    a.set_shelf_platform_face(TexId::from_raw(RACK), 80);
    tick(&mut a, 0.1);
    assert_eq!(
        title_alpha(&a).unwrap_or(0.0),
        0.0,
        "the title showed under the rack name"
    );
    tick(&mut a, 1.2);
    assert_eq!(
        title_alpha(&a).unwrap_or(0.0),
        0.0,
        "the title showed while the rack name held"
    );
    tick(&mut a, 0.4);
    let mid = title_alpha(&a).unwrap_or(0.0);
    assert!(
        mid > 0.0 && mid < 1.0,
        "the title did not fade in as the name faded out: {mid}"
    );
    tick(&mut a, 1.0);
    assert_eq!(
        title_alpha(&a),
        Some(1.0),
        "the title did not settle after the rack name"
    );
}

#[test]
fn the_title_hides_while_a_cart_goes_in() {
    let d = card();
    let mut a = boot(d.path(), true);
    tick(&mut a, 4.0);
    a.apply(Action::Insert);
    assert!(matches!(a.phase(), Phase::Inserting { .. }));
    assert_eq!(
        title_alpha(&a),
        None,
        "the title showed over a cart going in"
    );
}

fn collection_card() -> (TempDir, App) {
    let d = card();
    let dir = d.path().join("Collections");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Puzzlers.txt"), "gba/Fusion\n").unwrap();
    std::fs::write(dir.join("order.txt"), "Puzzlers\n").unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            show_titles: true,
            favorites: vec![Recent {
                platform: Platform::Gba,
                stem: "Emerald (USA, Europe)".into(),
            }],
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let mut a = App::boot(d.path());
    a.set_shelf_title_face(TexId::from_raw(TITLE), 120);
    a.set_shelf_platform_face(TexId::from_raw(RACK), 80);
    (d, a)
}

fn go_to(a: &mut App, shelf: &str) {
    tick(a, 1.0);
    for _ in 0..12 {
        if a.shelf_platform_name() == Some(shelf) {
            tick(a, 4.0);
            return;
        }
        press(a, Btn::R1);
        a.set_shelf_platform_face(TexId::from_raw(RACK), 80);
        tick(a, 0.5);
    }
    panic!(
        "never reached the {shelf} shelf: {:?}",
        a.shelf_platform_name()
    );
}

fn hidden(alpha: Option<f32>) -> bool {
    alpha.unwrap_or(0.0) < 0.01
}

#[test]
fn the_title_fades_out_behind_the_add_to_collection_screen() {
    let (_d, mut a) = collection_card();
    go_to(&mut a, "Game Boy Advance");
    assert!(!hidden(title_alpha(&a)), "no title to fade on the shelf");
    common::hold(&mut a, Btn::Y, FAVORITE_HOLD_MS);
    assert!(matches!(
        a.edit(),
        Some(slot::editing::Edit::Choosing { .. })
    ));
    tick(&mut a, 2.0);
    assert!(
        hidden(title_alpha(&a)),
        "the title showed over the add screen at {:?}",
        title_alpha(&a)
    );
}

#[test]
fn the_title_fades_out_behind_the_remove_from_collection_screen() {
    let (_d, mut a) = collection_card();
    go_to(&mut a, "Favorites");
    press(&mut a, Btn::R2);
    assert_eq!(a.shelf_platform_name(), Some("Puzzlers"));
    a.set_shelf_platform_face(TexId::from_raw(RACK), 80);
    tick(&mut a, 4.0);
    assert!(
        !hidden(title_alpha(&a)),
        "no title to fade on the collection"
    );
    common::hold(&mut a, Btn::Y, FAVORITE_HOLD_MS);
    assert!(matches!(
        a.edit(),
        Some(slot::editing::Edit::Removing { .. })
    ));
    tick(&mut a, 2.0);
    assert!(
        hidden(title_alpha(&a)),
        "the title showed over the remove screen at {:?}",
        title_alpha(&a)
    );
}
