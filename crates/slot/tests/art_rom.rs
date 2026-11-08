use slot::art::rom::{cached_crc, missing_art, rom_crc};

#[test]
fn crc_of_known_bytes() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("check.bin");
    std::fs::write(&p, b"123456789").unwrap();
    assert_eq!(rom_crc(&p).unwrap(), 0xCBF43926);
}

#[test]
fn crc_streams_a_large_file() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("big.gba");
    let bytes = vec![0xA5u8; 3 * 1024 * 1024 + 17];
    std::fs::write(&p, &bytes).unwrap();
    assert_eq!(rom_crc(&p).unwrap(), crc32fast::hash(&bytes));
}

#[test]
fn only_carts_without_labels_are_missing() {
    let d = tempfile::tempdir().unwrap();
    for dir in ["Games/GBA", "Labels/GBA"] {
        std::fs::create_dir_all(d.path().join(dir)).unwrap();
    }
    std::fs::write(d.path().join("Games/GBA/A.gba"), vec![0u8; 0x200]).unwrap();
    std::fs::write(d.path().join("Games/GBA/B.gba"), vec![0u8; 0x200]).unwrap();
    std::fs::write(d.path().join("Labels/GBA/B.png"), b"png").unwrap();
    let stems: Vec<String> = missing_art(d.path()).into_iter().map(|c| c.stem).collect();
    assert_eq!(stems, vec!["A".to_string()]);
}

fn rom_at(root: &std::path::Path, bytes: &[u8]) -> std::path::PathBuf {
    std::fs::create_dir_all(root.join("Games/NDS")).unwrap();
    let p = root.join("Games/NDS/Kart.nds");
    std::fs::write(&p, bytes).unwrap();
    p
}

fn mtime(p: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(p).unwrap().modified().unwrap()
}

#[test]
fn a_second_lookup_does_not_read_the_rom() {
    let d = tempfile::tempdir().unwrap();
    let p = rom_at(d.path(), b"first bytes!");
    let first = cached_crc(d.path(), &p).unwrap();
    assert_eq!(first, crc32fast::hash(b"first bytes!"));
    let when = mtime(&p);
    std::fs::write(&p, b"other bytes!").unwrap();
    std::fs::File::options().write(true).open(&p).unwrap().set_modified(when).unwrap();
    assert_eq!(cached_crc(d.path(), &p).unwrap(), first, "the ROM was read again");
}

#[test]
fn a_changed_rom_is_read_again() {
    let d = tempfile::tempdir().unwrap();
    let p = rom_at(d.path(), b"first bytes!");
    cached_crc(d.path(), &p).unwrap();
    std::fs::write(&p, b"a longer set of bytes").unwrap();
    assert_eq!(cached_crc(d.path(), &p).unwrap(), crc32fast::hash(b"a longer set of bytes"));
}

#[test]
fn a_corrupt_cache_line_is_ignored() {
    let d = tempfile::tempdir().unwrap();
    let p = rom_at(d.path(), b"first bytes!");
    std::fs::create_dir_all(d.path().join("Cache/art")).unwrap();
    std::fs::write(d.path().join("Cache/art/crc.txt"), "Games/NDS/Kart.nds\tnope\n\n\t\t\t\n").unwrap();
    assert_eq!(cached_crc(d.path(), &p).unwrap(), crc32fast::hash(b"first bytes!"));
}

#[test]
fn the_cache_keeps_other_roms() {
    let d = tempfile::tempdir().unwrap();
    let a = rom_at(d.path(), b"aaaa");
    let b = d.path().join("Games/NDS/Other.nds");
    std::fs::write(&b, b"bbbb").unwrap();
    cached_crc(d.path(), &a).unwrap();
    cached_crc(d.path(), &b).unwrap();
    let text = std::fs::read_to_string(d.path().join("Cache/art/crc.txt")).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
}
