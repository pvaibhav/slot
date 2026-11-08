use slot_store::Platform;

#[cfg(not(feature = "nds"))]
#[test]
fn every_platform_has_a_directory() {
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.dir_name()).collect();
    assert_eq!(names, vec!["GBA", "GBAV", "GBC", "GB"]);
}

#[cfg(not(feature = "nds"))]
#[test]
fn every_platform_is_a_shelf_of_its_own() {
    assert_eq!(
        Platform::ALL,
        [Platform::Gba, Platform::Gbav, Platform::Gbc, Platform::Gb]
    );
    for (i, p) in Platform::ALL.iter().enumerate() {
        assert!(
            !Platform::ALL[..i].contains(p),
            "{p:?} is named twice in the ring, so two shelves would hold one platform"
        );
    }
}

#[test]
fn each_platform_takes_only_its_own_extensions() {
    assert!(Platform::Gba.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(!Platform::Gba.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gb")));
    assert!(Platform::Gb.accepts(std::path::Path::new("Tetris.gbc")));
    assert!(!Platform::Gb.accepts(std::path::Path::new("Metroid Fusion.gba")));
    assert!(Platform::Gba.accepts(std::path::Path::new("Shrek.GBA")));
    assert!(Platform::Gbav.accepts(std::path::Path::new("Shrek.gba")));
    assert!(!Platform::Gbav.accepts(std::path::Path::new("Tetris.gb")));
}

#[test]
fn gba_video_runs_on_gba_hardware() {
    assert_eq!(Platform::Gbav.hardware(), Platform::Gba);
    assert_eq!(Platform::Gbav.picture(), Platform::Gba.picture());
    for p in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        assert_eq!(p.hardware(), p);
    }
}

#[cfg(feature = "nds")]
#[test]
fn nds_is_a_platform_with_its_own_folder() {
    use std::path::Path;
    assert_eq!(Platform::Nds.dir_name(), "NDS");
    assert_eq!(Platform::Nds.name(), "Nintendo DS");
    assert_eq!(Platform::Nds.picture(), (256, 192));
    assert_eq!(Platform::Nds.hardware(), Platform::Nds);
    assert!(Platform::Nds.accepts(Path::new("Mario Kart DS.NDS")));
    assert!(!Platform::Gba.accepts(Path::new("Mario Kart DS.nds")));
    assert!(!Platform::Nds.accepts(Path::new("Emerald.gba")));
    assert_eq!(
        Platform::ALL,
        [
            Platform::Nds,
            Platform::Gba,
            Platform::Gbav,
            Platform::Gbc,
            Platform::Gb
        ]
    );
}

#[cfg(not(feature = "nds"))]
#[test]
fn an_sp_build_has_no_nds_platform() {
    assert!(Platform::ALL.iter().all(|p| p.dir_name() != "NDS"));
}
