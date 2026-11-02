use slot::app::App;
use slot_store::Platform;

#[test]
fn boot_takes_game_boy_shapes_from_the_saved_index() {
    let d = tempfile::tempdir().unwrap();
    let rom = d.path().join("Games/GBC/Pak.gbc");
    std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
    std::fs::write(&rom, vec![0u8; 0x150]).unwrap();
    std::fs::create_dir_all(d.path().join("Cache")).unwrap();
    std::fs::write(
        d.path().join("Cache/headers"),
        "G\tGames/GBC/Pak.gbc\t336\t0\t192\t0\t\tPAK\n",
    )
    .unwrap();
    let app = App::boot(d.path());
    let cart = app
        .carts()
        .find(|c| c.platform == Platform::Gbc)
        .expect("the pak was scanned");
    assert_eq!(
        slot_ui::shelf_shape(cart).0,
        Some(slot_ui::GbShell::Rounded)
    );
}
