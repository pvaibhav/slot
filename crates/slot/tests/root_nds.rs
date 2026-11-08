use tempfile::tempdir;

#[test]
fn ensure_creates_nds_folders_only_with_the_feature() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    for sub in ["Games/NDS", "Saves/NDS", "States/NDS", "Labels/NDS"] {
        assert_eq!(
            d.path().join(sub).is_dir(),
            cfg!(feature = "nds"),
            "{sub} with nds = {}",
            cfg!(feature = "nds")
        );
    }
    assert!(d.path().join("Games/GBA").is_dir());
}
