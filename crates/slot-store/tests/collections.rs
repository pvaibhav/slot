mod common;

use common::tmp_root;
use slot_store::{
    name_ok, read_collections, write_collection, Collection, Platform, Recent, COLLECTIONS_DIR,
    ORDER_FILE,
};
use std::path::Path;

fn game(platform: Platform, stem: &str) -> Recent {
    Recent {
        platform,
        stem: stem.into(),
    }
}

fn put(root: &Path, name: &str, body: &str) {
    let dir = root.join(COLLECTIONS_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), body).unwrap();
}

fn order(root: &Path) -> String {
    std::fs::read_to_string(root.join(COLLECTIONS_DIR).join(ORDER_FILE)).unwrap()
}

fn names(root: &Path) -> Vec<String> {
    read_collections(root).into_iter().map(|c| c.name).collect()
}

#[test]
fn a_card_without_a_collections_folder_has_none() {
    let d = tmp_root();
    assert_eq!(read_collections(d.path()), vec![]);
}

#[test]
fn collections_come_back_in_order_txt_order() {
    let d = tmp_root();
    put(d.path(), "Puzzlers.txt", "");
    put(d.path(), "Dragon Ball.txt", "");
    put(d.path(), ORDER_FILE, "Dragon Ball\nPuzzlers\n");
    assert_eq!(names(d.path()), ["Dragon Ball", "Puzzlers"]);
}

#[test]
fn a_file_missing_from_order_txt_joins_the_end_and_is_written_in() {
    let d = tmp_root();
    put(d.path(), "Puzzlers.txt", "");
    put(d.path(), "Zelda.txt", "");
    put(d.path(), "Arcade.txt", "");
    put(d.path(), ORDER_FILE, "Puzzlers\n");
    assert_eq!(names(d.path()), ["Puzzlers", "Arcade", "Zelda"]);
    assert_eq!(order(d.path()), "Puzzlers\nArcade\nZelda\n");
}

#[test]
fn an_order_line_without_a_file_is_dropped() {
    let d = tmp_root();
    put(d.path(), "Puzzlers.txt", "");
    put(d.path(), ORDER_FILE, "Gone\nPuzzlers\n");
    assert_eq!(names(d.path()), ["Puzzlers"]);
    assert_eq!(order(d.path()), "Puzzlers\n");
}

#[test]
fn order_matches_files_without_regard_to_case() {
    let d = tmp_root();
    put(d.path(), "puzzlers.txt", "");
    put(d.path(), ORDER_FILE, "Puzzlers\n");
    assert_eq!(names(d.path()), ["puzzlers"]);
}

#[test]
fn lines_keep_their_file_order_across_platforms() {
    let d = tmp_root();
    put(
        d.path(),
        "Mix.txt",
        "gb/Catrap (USA)\ngba/Apotris\ngbc/Tetris DX\n",
    );
    let c = read_collections(d.path());
    assert_eq!(
        c[0].games,
        [
            game(Platform::Gb, "Catrap (USA)"),
            game(Platform::Gba, "Apotris"),
            game(Platform::Gbc, "Tetris DX"),
        ]
    );
}

#[test]
fn hand_edited_files_load_cleanly() {
    let d = tmp_root();
    put(
        d.path(),
        "Mix.txt",
        "gba/Apotris\r\n\r\n gb/Catrap \ngba/Apotris\nnonsense\n",
    );
    let c = read_collections(d.path());
    assert_eq!(
        c[0].games,
        [game(Platform::Gba, "Apotris"), game(Platform::Gb, "Catrap")]
    );
}

#[test]
fn writing_a_collection_round_trips() {
    let d = tmp_root();
    let c = Collection {
        name: "Dragon Ball".into(),
        games: vec![
            game(Platform::Gba, "Advanced Adventure"),
            game(Platform::Gb, "Z"),
        ],
    };
    write_collection(d.path(), &c).unwrap();
    assert_eq!(read_collections(d.path()), vec![c]);
}

#[test]
fn adding_skips_games_already_there() {
    let mut c = Collection {
        name: "P".into(),
        games: vec![game(Platform::Gba, "A")],
    };
    assert_eq!(
        c.add(&[game(Platform::Gba, "A"), game(Platform::Gb, "B")]),
        1
    );
    assert_eq!(c.games, [game(Platform::Gba, "A"), game(Platform::Gb, "B")]);
}

#[test]
fn swap_moves_one_place_and_stops_at_the_ends() {
    let (a, b, c) = (
        game(Platform::Gba, "a"),
        game(Platform::Gba, "b"),
        game(Platform::Gba, "c"),
    );
    let mut col = Collection {
        name: "P".into(),
        games: vec![a.clone(), b.clone(), c.clone()],
    };
    assert_eq!(col.swap(0, 1), Some(1));
    assert_eq!(col.games, [b.clone(), a.clone(), c.clone()]);
    assert_eq!(col.swap(0, -1), None);
    assert_eq!(col.swap(2, 1), None);
    assert_eq!(col.games, [b, a, c]);
}

#[test]
fn removing_takes_the_game_out() {
    let mut c = Collection {
        name: "P".into(),
        games: vec![game(Platform::Gba, "A"), game(Platform::Gb, "B")],
    };
    assert!(c.remove(&game(Platform::Gba, "A")));
    assert!(!c.remove(&game(Platform::Gba, "A")));
    assert_eq!(c.games, [game(Platform::Gb, "B")]);
}

#[test]
fn names_that_cannot_be_files_are_refused() {
    let existing = vec![Collection {
        name: "Puzzlers".into(),
        games: vec![],
    }];
    for bad in [
        "",
        "order",
        "ORDER",
        "a/b",
        "a:b",
        "a\\b",
        "a*",
        "a?",
        "a\"",
        "a<",
        "a>",
        "a|",
        "name.",
        "name ",
        "puzzlers",
        ".hack",
        "x".repeat(252).as_str(),
    ] {
        assert!(!name_ok(bad, &existing), "{bad:?} was allowed");
    }
    assert!(name_ok("Dragon Ball", &existing));
    assert!(name_ok(&"x".repeat(251), &existing));
}

#[test]
fn a_byte_order_mark_does_not_eat_the_first_game() {
    let d = tmp_root();
    put(d.path(), "Mix.txt", "\u{feff}gba/Apotris\ngb/Catrap\n");
    let c = read_collections(d.path());
    assert_eq!(
        c[0].games,
        [game(Platform::Gba, "Apotris"), game(Platform::Gb, "Catrap")]
    );
}

#[test]
fn a_latin_1_file_still_loads_every_line() {
    let d = tmp_root();
    let dir = d.path().join(COLLECTIONS_DIR);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Mix.txt"), b"gba/Apotris\ngba/Pok\xe9mon\n").unwrap();
    let c = read_collections(d.path());
    assert_eq!(
        c[0].games,
        [
            game(Platform::Gba, "Apotris"),
            game(Platform::Gba, "Pokémon")
        ]
    );
}
