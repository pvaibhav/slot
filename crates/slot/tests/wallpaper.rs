mod common;

use slot::app::App;
use slot::wallpaper::{pick, WallpaperLoader};
use slot_store::{write_slot_state, Platform, Recent, SlotState, WallpaperPick};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn write_png(dir: &Path, name: &str) {
    let f = std::fs::File::create(dir.join(name)).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), 2, 2);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&[0u8; 12])
        .unwrap();
}

fn papers(names: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    let dir = d.path().join("Wallpapers");
    std::fs::create_dir_all(&dir).unwrap();
    for n in names {
        write_png(&dir, n);
    }
    (d, dir)
}

fn name(p: PathBuf) -> String {
    p.file_name().unwrap().to_string_lossy().into_owned()
}

fn on_shelf(d: &tempfile::TempDir, shelf: &str) -> App {
    let (platform, stem) = match shelf {
        "gb" => (Platform::Gb, "Tetris"),
        "gbc" => (Platform::Gbc, "Crystal"),
        "gbav" => (Platform::Gbav, "Shrek"),
        _ => (Platform::Gba, "Emerald"),
    };
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            wallpaper: WallpaperPick::Shelf,
            shelf: Some(shelf.into()),
            shelf_cart: Some(Recent {
                platform,
                stem: stem.into(),
            }),
            ..SlotState::default()
        },
    )
    .unwrap();
    App::boot(d.path())
}

fn all_shelves_card(names: &[&str]) -> tempfile::TempDir {
    let (d, _) = papers(names);
    common::write_gb_cart(&d, "Tetris", "TETRIS");
    let gbc = d.path().join("Games").join(Platform::Gbc.dir_name());
    std::fs::create_dir_all(&gbc).unwrap();
    std::fs::write(gbc.join("Crystal.gbc"), vec![0u8; 0x150]).unwrap();
    let gbav = d.path().join("Games").join(Platform::Gbav.dir_name());
    std::fs::create_dir_all(&gbav).unwrap();
    std::fs::write(gbav.join("Shrek.gba"), vec![0u8; 0x100]).unwrap();
    d
}

#[test]
fn shelf_wallpaper_follows_the_platform() {
    let d = all_shelves_card(&["GB.png", "GBC.png", "GBA.png", "GBAV.png"]);
    for (shelf, file) in [
        ("gb", "GB.png"),
        ("gbc", "GBC.png"),
        ("gba", "GBA.png"),
        ("gbav", "GBAV.png"),
    ] {
        assert_eq!(
            on_shelf(&d, shelf).shown_wallpaper().as_deref(),
            Some(file),
            "on the {shelf} shelf"
        );
    }
}

#[test]
fn shelf_wallpaper_on_gbav_falls_back_to_gba() {
    let d = all_shelves_card(&["GB.png", "GBA.png"]);
    assert_eq!(
        on_shelf(&d, "gbav").shown_wallpaper().as_deref(),
        Some("GBA.png")
    );
}

#[test]
fn shelf_wallpaper_ignores_case_and_shows_nothing_without_a_file() {
    let d = all_shelves_card(&["gbc.PNG"]);
    assert_eq!(
        on_shelf(&d, "gbc").shown_wallpaper().as_deref(),
        Some("gbc.PNG")
    );
    assert_eq!(on_shelf(&d, "gb").shown_wallpaper(), None);
}

#[test]
fn shelf_wallpaper_changes_when_the_shelf_does() {
    let d = all_shelves_card(&["GB.png", "GBC.png", "GBA.png", "GBAV.png"]);
    let mut a = on_shelf(&d, "gba");
    let first = a.shown_wallpaper();
    a.apply(slot_input::Action::NextShelf);
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    let next = a.shown_wallpaper();
    assert!(next.is_some());
    assert_ne!(first, next);
}

#[test]
fn every_shelf_wallpaper_is_listed_once_for_decoding() {
    let d = all_shelves_card(&["GB.png", "GBA.png", "Recents.png", "other.png"]);
    assert_eq!(
        on_shelf(&d, "gba").shelf_wallpapers(),
        ["GB.png", "GBA.png", "Recents.png"]
    );
}

#[test]
fn the_loader_decodes_a_wallpaper_on_its_own_thread() {
    let (d, _) = papers(&["GBA.png", "broken.png"]);
    std::fs::write(d.path().join("Wallpapers/broken.png"), b"not a png").unwrap();
    let loader = WallpaperLoader::spawn();
    loader.request(d.path(), "broken.png", (4, 4));
    loader.request(d.path(), "GBA.png", (4, 4));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let done = loop {
        if let Some(done) = loader.take() {
            break done;
        }
        assert!(std::time::Instant::now() < until, "nothing came back");
        std::thread::yield_now();
    };
    assert_eq!(done.name, "GBA.png");
    assert_eq!(done.size, (4, 4));
    assert_eq!(done.rgba.len(), 4 * 4 * 4);
}

#[test]
fn a_card_with_no_wallpapers_picks_nothing() {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    assert!(pick(d.path(), 0).is_none());
    std::fs::create_dir_all(d.path().join("Wallpapers")).unwrap();
    assert!(pick(d.path(), 7).is_none(), "an empty folder picked a file");
}

#[test]
fn every_wallpaper_shows_once_before_any_repeats() {
    let (d, _) = papers(&["a.png", "b.png", "c.png", "d.png", "e.png"]);
    for round in 0..4 {
        let shown: Vec<_> = (0..5)
            .map(|i| name(pick(d.path(), (round * 5 + i) * 60).unwrap()))
            .collect();
        assert_eq!(
            shown.iter().collect::<HashSet<_>>().len(),
            5,
            "round {round} showed {shown:?}"
        );
    }
}

#[test]
fn a_new_round_never_opens_on_the_last_wallpaper() {
    let (d, _) = papers(&["a.png", "b.png", "c.png"]);
    let mut last = name(pick(d.path(), 0).unwrap());
    for seed in 1..60 {
        let next = name(pick(d.path(), seed * 60).unwrap());
        assert_ne!(next, last, "seed {seed} repeated {last}");
        last = next;
    }
}

#[test]
fn two_wallpapers_alternate() {
    let (d, _) = papers(&["psyduck.png", "aussyduck.png"]);
    let shown: Vec<_> = (0..6)
        .map(|s| name(pick(d.path(), s * 60).unwrap()))
        .collect();
    for pair in shown.windows(2) {
        assert_ne!(pair[0], pair[1], "two wallpapers gave {shown:?}");
    }
}

#[test]
fn an_added_wallpaper_joins_the_current_round() {
    let (d, dir) = papers(&["a.png", "b.png", "c.png", "d.png"]);
    let first = name(pick(d.path(), 0).unwrap());
    write_png(&dir, "new.png");
    let rest: Vec<_> = (1..5)
        .map(|s| name(pick(d.path(), s * 60).unwrap()))
        .collect();
    assert!(
        rest.contains(&"new.png".to_string()),
        "the added wallpaper missed the round: {first} then {rest:?}"
    );
    let round: HashSet<_> = std::iter::once(&first).chain(&rest).collect();
    assert_eq!(round.len(), 5, "the round repeated: {first} then {rest:?}");
}

#[test]
fn a_removed_wallpaper_is_never_picked() {
    let (d, dir) = papers(&["a.png", "b.png", "c.png"]);
    let first = name(pick(d.path(), 0).unwrap());
    let gone = ["a.png", "b.png", "c.png"]
        .into_iter()
        .find(|n| *n != first)
        .unwrap();
    std::fs::remove_file(dir.join(gone)).unwrap();
    for seed in 1..20 {
        let p = pick(d.path(), seed * 60).unwrap();
        assert!(p.exists(), "seed {seed} picked {p:?}");
        assert_ne!(name(p), gone, "seed {seed} picked the removed file");
    }
}

#[test]
fn the_shown_wallpaper_being_removed_moves_on() {
    let (d, dir) = papers(&["a.png", "b.png"]);
    let first = name(pick(d.path(), 0).unwrap());
    std::fs::remove_file(dir.join(&first)).unwrap();
    let next = name(pick(d.path(), 60).unwrap());
    assert_ne!(next, first);
    assert_eq!(name(pick(d.path(), 120).unwrap()), next, "one file left");
}

#[test]
fn a_damaged_deck_starts_a_fresh_one() {
    let (d, _) = papers(&["a.png", "b.png"]);
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(d.path().join("Config/wallpaper.deck"), b"\xff\xfe junk").unwrap();
    assert!(pick(d.path(), 0).is_some());
}

#[test]
fn a_sidecar_is_never_the_wallpaper() {
    let (d, dir) = papers(&["psyduck.png"]);
    write_png(&dir, "._psyduck.png");
    for seed in 0..8 {
        let picked = pick(d.path(), seed).expect("a wallpaper was present");
        assert_eq!(
            picked.file_name().unwrap(),
            "psyduck.png",
            "seed {seed} picked the sidecar"
        );
    }
}

#[test]
fn only_pngs_are_picked() {
    let (d, dir) = papers(&[]);
    std::fs::write(dir.join("notes.txt"), b"hello").unwrap();
    std::fs::write(dir.join("art.jpg"), b"not a png either").unwrap();
    assert!(pick(d.path(), 0).is_none(), "a non png was picked");
    write_png(&dir, "real.PNG");
    assert!(
        pick(d.path(), 0).is_some(),
        "an upper case extension was skipped"
    );
}
