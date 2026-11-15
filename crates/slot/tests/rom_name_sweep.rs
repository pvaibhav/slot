mod common;

use std::path::Path;

use common::{tmp_root_with_gb_carts, write_gb_cart};
use slot::app::App;
use slot_store::{write_slot_state, Platform, Recent, SlotState};

fn put(root: &Path, rel: &str, body: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn stems(a: &App) -> Vec<String> {
    let mut s: Vec<String> = a.carts().map(|c| c.stem.clone()).collect();
    s.sort();
    s
}

#[test]
fn a_space_before_the_extension_is_swept_and_everything_follows() {
    let d = tmp_root_with_gb_carts(&[]);
    write_gb_cart(&d, "Super Mario Land ", "SML");
    put(d.path(), "Saves/GB/Super Mario Land .sav", b"save");
    put(d.path(), "Labels/GB/Super Mario Land .png", b"label");
    put(
        d.path(),
        "Collections/Platformers.txt",
        b"gb/Super Mario Land \n",
    );
    put(d.path(), "Collections/order.txt", b"Platformers\n");
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            favorites: vec![Recent {
                platform: Platform::Gb,
                stem: "Super Mario Land ".into(),
            }],
            ..SlotState::default()
        },
    )
    .unwrap();

    let a = App::boot(d.path());

    let root = d.path();
    assert!(root.join("Games/GB/Super Mario Land.gb").is_file());
    assert!(!root.join("Games/GB/Super Mario Land .gb").exists());
    assert_eq!(
        std::fs::read(root.join("Saves/GB/Super Mario Land.sav")).unwrap(),
        b"save"
    );
    assert!(root.join("Labels/GB/Super Mario Land.png").is_file());
    let platformers = slot_store::read_collections(root)
        .into_iter()
        .find(|c| c.name == "Platformers")
        .unwrap();
    assert_eq!(
        platformers.games,
        [Recent {
            platform: Platform::Gb,
            stem: "Super Mario Land".into()
        }]
    );
    assert_eq!(stems(&a), ["Super Mario Land"]);
    assert_eq!(
        slot_store::read_slot_state(root).favorites,
        [Recent {
            platform: Platform::Gb,
            stem: "Super Mario Land".into()
        }]
    );
}

#[test]
fn dots_and_leading_spaces_are_swept_too() {
    let d = tmp_root_with_gb_carts(&[]);
    write_gb_cart(&d, "Catrap.", "CATRAP");
    write_gb_cart(&d, " Kirby", "KIRBY");
    write_gb_cart(&d, "Tetris . ", "TETRIS");
    let a = App::boot(d.path());
    assert_eq!(stems(&a), ["Catrap", "Kirby", "Tetris"]);
    assert!(d.path().join("Games/GB/Catrap.gb").is_file());
    assert!(d.path().join("Games/GB/Kirby.gb").is_file());
    assert!(d.path().join("Games/GB/Tetris.gb").is_file());
}

#[test]
fn a_clean_name_already_on_the_card_leaves_both_alone() {
    let d = tmp_root_with_gb_carts(&[]);
    write_gb_cart(&d, "Tetris", "TETRIS");
    write_gb_cart(&d, "Tetris ", "TETRIS DX");
    let a = App::boot(d.path());
    assert!(d.path().join("Games/GB/Tetris.gb").is_file());
    assert!(d.path().join("Games/GB/Tetris .gb").is_file());
    assert_eq!(
        std::fs::read(d.path().join("Games/GB/Tetris.gb")).unwrap()[0x134..0x13a],
        *b"TETRIS"
    );
    assert_eq!(stems(&a).len(), 2);
}

#[test]
fn spaces_inside_a_name_stay() {
    let d = tmp_root_with_gb_carts(&[]);
    write_gb_cart(&d, "Super  Mario Land 2", "SML2");
    let a = App::boot(d.path());
    assert_eq!(stems(&a), ["Super  Mario Land 2"]);
}
