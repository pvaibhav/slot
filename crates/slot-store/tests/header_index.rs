use std::path::{Path, PathBuf};

use slot_store::header_index::{dirty, gb_header, gba_code, load, peek_gb_header, save, verify};

fn gb_rom(root: &Path, name: &str, cgb: u8, len: usize) -> PathBuf {
    let path = root.join("Games/GB").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rom = vec![0u8; len];
    rom[0x134..0x13a].copy_from_slice(b"TETRIS");
    rom[0x143] = cgb;
    rom[0x14a] = 1;
    std::fs::write(&path, rom).unwrap();
    path
}

fn gba_rom(root: &Path, name: &str, code: &[u8; 4]) -> PathBuf {
    let path = root.join("Games/GBA").join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rom = vec![0u8; 0x200];
    rom[0xac..0xb0].copy_from_slice(code);
    std::fs::write(&path, rom).unwrap();
    path
}

#[test]
fn a_saved_index_answers_without_the_roms() {
    let d = tempfile::tempdir().unwrap();
    let gb = gb_rom(d.path(), "Tetris.gb", 0xc0, 0x150);
    let gba = gba_rom(d.path(), "Emerald.gba", b"BPEE");
    let header = gb_header(&gb).expect("header");
    assert_eq!(gba_code(&gba), "BPEE");
    save(d.path()).unwrap();
    std::fs::remove_file(&gb).unwrap();
    std::fs::remove_file(&gba).unwrap();
    load(d.path());
    assert_eq!(peek_gb_header(&gb), Some(Some(header)));
    assert_eq!(gba_code(&gba), "BPEE");
}

#[test]
fn a_replaced_rom_is_read_again() {
    let d = tempfile::tempdir().unwrap();
    let gb = gb_rom(d.path(), "Pak.gb", 0x00, 0x150);
    assert_eq!(gb_header(&gb).unwrap().cgb, 0x00);
    save(d.path()).unwrap();
    load(d.path());
    gb_rom(d.path(), "Pak.gb", 0xc0, 0x200);
    verify(&gb);
    assert_eq!(gb_header(&gb).unwrap().cgb, 0xc0);
}

#[test]
fn an_unchanged_rom_keeps_its_entry() {
    let d = tempfile::tempdir().unwrap();
    let gb = gb_rom(d.path(), "Pak.gb", 0x80, 0x150);
    let header = gb_header(&gb);
    verify(&gb);
    assert_eq!(peek_gb_header(&gb), Some(header));
}

#[test]
fn a_corrupt_index_loads_as_empty() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Cache")).unwrap();
    std::fs::write(
        d.path().join("Cache/headers"),
        b"G\tnot\tenough\n\xff\xfe junk\n",
    )
    .unwrap();
    load(d.path());
    assert_eq!(peek_gb_header(&d.path().join("Games/GB/not")), None);
}

#[test]
fn reading_a_new_rom_leaves_the_index_unsaved_until_it_is_saved() {
    let d = tempfile::tempdir().unwrap();
    load(d.path());
    assert!(!dirty());
    gba_code(&gba_rom(d.path(), "Zero.gba", b"AZEE"));
    assert!(dirty());
    save(d.path()).unwrap();
    assert!(!dirty());
}

#[test]
fn entries_for_roms_no_longer_on_the_card_are_dropped() {
    let d = tempfile::tempdir().unwrap();
    let kept = gb_rom(d.path(), "Kept.gb", 0x00, 0x150);
    let gone = gb_rom(d.path(), "Gone.gb", 0x80, 0x150);
    gb_header(&kept);
    gb_header(&gone);
    save(d.path()).unwrap();
    load(d.path());
    slot_store::header_index::retain(std::slice::from_ref(&kept));
    assert!(
        dirty(),
        "dropping an entry did not mark the index for saving"
    );
    assert!(peek_gb_header(&kept).is_some());
    assert_eq!(peek_gb_header(&gone), None);
    save(d.path()).unwrap();
    load(d.path());
    assert_eq!(
        peek_gb_header(&gone),
        None,
        "the dropped entry came back from the file"
    );
}

#[test]
fn keeping_every_entry_leaves_the_index_saved() {
    let d = tempfile::tempdir().unwrap();
    let rom = gb_rom(d.path(), "Kept.gb", 0x00, 0x150);
    gb_header(&rom);
    save(d.path()).unwrap();
    slot_store::header_index::retain(&[rom]);
    assert!(!dirty());
}
