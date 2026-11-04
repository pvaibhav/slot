use slot::art::rom::{missing_art, rom_crc};

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
