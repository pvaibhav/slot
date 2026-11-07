mod common;

use common::tmp_root;
use slot_store::{scan, Platform};
use tempfile::TempDir;

fn write_rom(d: &TempDir, rel: &str, title: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(d.path().join("Games").join(rel), rom).expect("write rom");
}

fn write_png(d: &TempDir, rel: &str) {
    let path = d.path().join("Labels").join(rel);
    std::fs::create_dir_all(path.parent().expect("label has a parent")).expect("create dir");
    std::fs::write(path, b"\x89PNG\r\n\x1a\n").expect("write png");
}

#[test]
fn a_png_in_labels_is_paired_to_its_rom_by_stem() {
    let d = tmp_root();
    write_rom(&d, "GBA/Pokemon Emerald.gba", "POKEMON EMER");
    write_png(&d, "GBA/Pokemon Emerald.png");
    write_rom(&d, "GBA/Advance Wars.gba", "ADVANCEWARS");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 2);
    assert_eq!(carts[0].stem, "Advance Wars");
    assert!(carts[0].label.is_none());
    assert!(
        carts[1].label.is_some(),
        "a label in Labels/ was not picked up"
    );
    let mut emerald = carts[1].clone();
    emerald.read_header();
    assert_eq!(emerald.title, "POKEMON EMER");
}

#[test]
fn a_label_pairs_with_its_rom_whatever_the_case_of_its_name() {
    let d = tmp_root();
    write_rom(&d, "GBA/Pokemon Emerald.gba", "POKEMON EMER");
    write_png(&d, "GBA/pokemon emerald.PNG");
    let carts = scan(d.path()).unwrap();
    assert_eq!(
        carts[0].label.as_deref(),
        Some(d.path().join("Labels/GBA/pokemon emerald.PNG").as_path())
    );
}

#[test]
fn a_scan_reads_names_and_leaves_the_rom_headers_for_later() {
    let d = tmp_root();
    write_rom(&d, "GBA/Advance Wars.gba", "ADVANCEWARS");
    let mut cart = scan(d.path()).unwrap().remove(0);
    assert!(cart.title.is_empty());
    cart.read_header();
    assert_eq!(cart.title, "ADVANCEWARS");
}

#[test]
fn scan_ignores_non_gba_files() {
    let d = tmp_root();
    write_rom(&d, "GBA/Real.gba", "REAL");
    std::fs::write(d.path().join("Games/GBA/notes.txt"), "hi").unwrap();
    assert_eq!(scan(d.path()).unwrap().len(), 1);
}

#[test]
fn an_appledouble_sidecar_is_not_shelved_as_a_cart() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    write_rom(&d, "GBA/._Metroid Fusion.gba", "METROID");
    let carts = scan(d.path()).unwrap();
    assert_eq!(carts.len(), 1, "an AppleDouble sidecar reached the shelf");
    assert_eq!(carts[0].stem, "Metroid Fusion");
}

#[test]
fn header_title_of_a_truncated_rom_is_none_not_a_panic() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/GBA/Tiny.gba"), [0u8; 8]).unwrap();
    let mut cart = scan(d.path()).unwrap().remove(0);
    cart.read_header();
    assert!(cart.title.is_empty());
}

#[test]
fn a_header_title_that_is_not_text_is_dropped_rather_than_mangled() {
    let d = tmp_root();
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xac].copy_from_slice(&[0xffu8; 12]);
    std::fs::write(d.path().join("Games/GBA/Garbage.gba"), rom).unwrap();
    let mut cart = scan(d.path()).unwrap().remove(0);
    cart.read_header();
    assert!(cart.title.is_empty());
}

#[test]
fn a_root_with_no_games_directory_scans_as_empty() {
    let d = tempfile::tempdir().unwrap();
    assert!(scan(d.path()).unwrap().is_empty());
}

#[test]
fn a_cart_takes_the_platform_of_the_folder_it_is_in() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::write(d.path().join("Games/GBC/Chromatic.gbc"), vec![0u8; 0x150]).unwrap();

    let carts = scan(d.path()).unwrap();

    let by_stem = |s: &str| carts.iter().find(|c| c.stem == s).unwrap().platform;
    assert_eq!(by_stem("Metroid Fusion"), Platform::Gba);
    assert_eq!(by_stem("Tetris"), Platform::Gb);
    assert_eq!(by_stem("Chromatic"), Platform::Gbc);
}

#[test]
fn a_gba_rom_in_the_game_boy_folder_does_not_appear() {
    let d = tmp_root();
    std::fs::write(d.path().join("Games/GB/Wrong.gba"), vec![0u8; 0x150]).unwrap();
    assert!(scan(d.path()).unwrap().is_empty());
}

#[test]
fn an_unreadable_platform_folder_does_not_take_the_rest_of_the_library_with_it() {
    let d = tmp_root();
    write_rom(&d, "GBA/Metroid Fusion.gba", "METROID");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::remove_dir(d.path().join("Games/GBC")).unwrap();
    std::fs::write(d.path().join("Games/GBC"), b"not a directory").unwrap();

    let carts = scan(d.path()).expect("one bad folder must not fail the whole scan");

    assert_eq!(carts.len(), 2, "the readable shelves were lost too");
    assert!(carts.iter().any(|c| c.stem == "Metroid Fusion"));
    assert!(carts.iter().any(|c| c.stem == "Tetris"));
}

#[test]
fn the_same_stem_on_two_platforms_is_two_carts_with_two_labels() {
    let d = tmp_root();
    write_rom(&d, "GBA/Tetris.gba", "TETRIS");
    std::fs::write(d.path().join("Games/GB/Tetris.gb"), vec![0u8; 0x150]).unwrap();
    std::fs::create_dir_all(d.path().join("Labels/GB")).unwrap();
    std::fs::write(d.path().join("Labels/GB/Tetris.png"), b"\x89PNG\r\n\x1a\n").unwrap();

    let carts = scan(d.path()).unwrap();

    assert_eq!(carts.len(), 2);
    let gb = carts.iter().find(|c| c.platform == Platform::Gb).unwrap();
    let gba = carts.iter().find(|c| c.platform == Platform::Gba).unwrap();
    assert!(gb.label.is_some(), "the Game Boy label was not found");
    assert!(
        gba.label.is_none(),
        "the GBA cart borrowed the Game Boy label"
    );
}

#[test]
fn nds_roms_are_scanned_only_with_the_feature() {
    let d = tmp_root();
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Mario Kart DS.nds"), vec![0u8; 0x200]).unwrap();
    write_rom(&d, "GBA/Emerald.gba", "POKEMON EMER");
    let carts = scan(d.path()).unwrap();
    let stems: Vec<&str> = carts.iter().map(|c| c.stem.as_str()).collect();
    if cfg!(feature = "nds") {
        assert_eq!(carts.len(), 2, "{stems:?}");
        assert!(carts
            .iter()
            .any(|c| c.stem == "Mario Kart DS" && c.platform.dir_name() == "NDS"));
    } else {
        assert_eq!(stems, vec!["Emerald"]);
    }
}
