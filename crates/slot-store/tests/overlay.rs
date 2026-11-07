use slot_store::overlay::{overlay_entries, overlay_part, OverlayPart};
use slot_store::{read_slot_state, write_slot_state, SlotState};

#[test]
fn overlay_part_reads_suffixes_case_insensitively() {
    assert_eq!(
        overlay_part("grid_tile.png"),
        Some(("grid".into(), OverlayPart::Tile))
    );
    assert_eq!(
        overlay_part("GRID_TILE.PNG"),
        Some(("GRID".into(), OverlayPart::Tile))
    );
    assert_eq!(
        overlay_part("glass.png"),
        Some(("glass".into(), OverlayPart::Both))
    );
    assert_eq!(
        overlay_part("glass_top.png"),
        Some(("glass".into(), OverlayPart::Top))
    );
    assert_eq!(
        overlay_part("glass_bottom.png"),
        Some(("glass".into(), OverlayPart::Bottom))
    );
    assert_eq!(overlay_part("._glass.png"), None);
    assert_eq!(overlay_part(".png"), None);
    assert_eq!(overlay_part("_tile.png"), None);
    assert_eq!(overlay_part("notes.txt"), None);
}

#[test]
fn overlay_entries_group_parts_under_one_name() {
    let files = [
        "glass_top.png",
        "Grid_tile.png",
        "glass.png",
        "glass_bottom.png",
    ]
    .map(String::from)
    .to_vec();
    let e = overlay_entries(&files);
    assert_eq!(
        e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
        ["glass", "Grid"]
    );
    assert_eq!(e[0].top.as_deref(), Some("glass_top.png"));
    assert_eq!(e[0].both.as_deref(), Some("glass.png"));
    assert_eq!(e[0].bottom.as_deref(), Some("glass_bottom.png"));
    assert_eq!(e[0].tile, None);
    assert_eq!(e[1].tile.as_deref(), Some("Grid_tile.png"));
}

#[test]
fn a_tile_and_a_full_image_with_one_stem_are_one_entry() {
    let e = overlay_entries(&["lcd.png".to_string(), "lcd_tile.png".to_string()]);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].both.as_deref(), Some("lcd.png"));
    assert_eq!(e[0].tile.as_deref(), Some("lcd_tile.png"));
}

#[test]
fn a_top_only_set_is_still_an_entry() {
    let e = overlay_entries(&["glass_top.png".to_string()]);
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].top.as_deref(), Some("glass_top.png"));
    assert_eq!(e[0].both, None);
    assert_eq!(e[0].bottom, None);
}

#[test]
fn overlay_picks_survive_a_save_and_load() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    let s = SlotState {
        overlay_gba: Some("grid".into()),
        overlay_gb: None,
        overlay_gbc: Some("lcd".into()),
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let back = read_slot_state(d.path());
    assert_eq!(back.overlay_gba.as_deref(), Some("grid"));
    assert_eq!(back.overlay_gb, None);
    assert_eq!(back.overlay_gbc.as_deref(), Some("lcd"));
}

#[test]
fn overlays_default_to_off() {
    let s = SlotState::default();
    assert_eq!(s.overlay_gba, None);
    assert_eq!(s.overlay_gb, None);
    assert_eq!(s.overlay_gbc, None);
}

#[test]
fn top_and_bottom_are_plain_names_on_one_screen() {
    let files = ["desert_top.png".to_string(), "grid_tile.png".to_string()];
    let e = slot_store::overlay::overlay_entries_one_screen(&files);
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].name, "desert_top");
    assert_eq!(e[0].both.as_deref(), Some("desert_top.png"));
    assert_eq!(e[1].tile.as_deref(), Some("grid_tile.png"));
}
