use super::*;
use image::{ImageFormat, Rgb, RgbImage};
use std::io::Cursor;

fn cart(stem: &str) -> Cart {
    Cart {
        platform: slot_store::Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        title: "".into(),
        code: "".into(),
        shell: None,
        sealed: false,
    }
}

fn image_bytes(w: u32, h: u32) -> Vec<u8> {
    // The top and bottom have different colours: catches upside-down/top-centering errors.
    let im = RgbImage::from_fn(w, h, |_, y| {
        if y < h / 2 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    im.write_to(&mut bytes, ImageFormat::Png).unwrap();
    bytes.into_inner()
}

#[test]
fn no_intro_names_match_database_punctuation() {
    for (stem, db) in [
        (
            "Legend of Zelda, The - A Link to the Past & Four Swords (USA)",
            "The Legend of Zelda: A Link to the Past and Four Swords",
        ),
        (
            "WarioWare, Inc. - Mega Microgame$! (USA)",
            "WarioWare, Inc.: Mega Microgame$!",
        ),
        ("Aladdin (USA) (En,Fr,De,Es)", "Disney's Aladdin"),
        (
            "Pokémon - Emerald Version (USA) [!]\u{200B}",
            "Pokemon: Emerald Version",
        ),
    ] {
        assert_eq!(source::title(stem), source::normalize(db));
    }
}

#[test]
fn crop_produces_exact_rgb_png_and_rejects_bad_input() {
    for (platform, sizes) in [
        (
            Platform::Gba,
            &[(1000, 574), (600, 355), (473, 283), (800, 465)][..],
        ),
        (Platform::Gb, &[(796, 906), (674, 759)][..]),
        (Platform::Gbc, &[(800, 916)][..]),
    ] {
        let (lw, lh) = label_size(platform);
        for (w, h) in sizes {
            let png = artwork::prepare(&image_bytes(*w, *h), platform).unwrap();
            assert_eq!(image::guess_format(&png).unwrap(), ImageFormat::Png);
            let im = image::load_from_memory(&png).unwrap().to_rgb8();
            assert_eq!(im.dimensions(), (lw, lh));
            assert_eq!(im.get_pixel(lw / 2, 0), &Rgb([255, 0, 0]));
            assert_eq!(im.get_pixel(lw / 2, lh - 1), &Rgb([0, 0, 255]));
        }
    }
    assert!(artwork::prepare(b"<html>Error</html>", Platform::Gba).is_err());
    assert!(artwork::prepare(&image_bytes(100, 100), Platform::Gba).is_err());
    assert!(artwork::prepare(&image_bytes(5000, 2), Platform::Gba).is_err());
    // A cart scan of the other shape is not this platform's cart.
    assert!(artwork::prepare(&image_bytes(1000, 574), Platform::Gb).is_err());
    assert!(artwork::prepare(&image_bytes(796, 906), Platform::Gba).is_err());
}

#[test]
fn a_custom_label_wins_a_publish_race_and_no_partial_files_remain() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("label.png");
    std::fs::write(&file, b"custom").unwrap();
    publish(&file, b"download").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"custom");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn legacy_card_publish_is_complete_and_keeps_existing_labels() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("label.png");
    let make = |bytes: &[u8]| {
        let mut tmp = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
        tmp.write_all(bytes).unwrap();
        tmp.as_file().sync_all().unwrap();
        tmp
    };
    publish_legacy(make(b"first complete image"), &target).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"first complete image");
    publish_legacy(make(b"replacement"), &target).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), b"first complete image");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn cart_fronts_rank_own_region_photos_then_renders_then_other_regions() {
    let images = [
        (source::RENDER, "Japan", "jp-render"),
        (source::PHOTO, "Japan", "jp-photo"),
        ("Box - Front", "North America", "box"),
        (source::RENDER, "North America", "us-render"),
        (source::PHOTO, "North America", "us-photo"),
    ];
    assert_eq!(
        source::rank("Advance Wars (USA)", "", &images),
        ["us-photo", "us-render", "jp-photo"]
    );
    assert_eq!(
        source::rank("Advance Wars", "AWRJ", &images),
        ["jp-photo", "jp-render", "us-photo"]
    );
}

/// A Game Boy ROM, on the Game Boy Color shelf, whose CRC32 is the standard check value CBF43926.
fn rom_on_gbc_shelf(dir: &Path) -> Cart {
    let rom = dir.join("Games/GBC/Anything.gb");
    std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
    std::fs::write(&rom, b"123456789").unwrap();
    Cart {
        platform: Platform::Gbc,
        stem: "Anything".into(),
        rom,
        ..cart("unused")
    }
}

fn write_pack(root: &Path, entries: Vec<(u32, Vec<u8>)>) {
    let dir = root.join("Labels");
    std::fs::create_dir_all(&dir).unwrap();
    let mut out = Vec::new();
    build::write_pack(&mut out, entries).unwrap();
    std::fs::write(dir.join(build::PACK), out).unwrap();
}

#[test]
fn a_rom_in_the_artwork_pack_is_labelled_for_the_shelf_it_sits_on() {
    let dir = tempfile::tempdir().unwrap();
    let jpeg = build::cart_label_jpeg(&image_bytes(438, 503), Platform::Gb).unwrap();
    assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
    write_pack(dir.path(), vec![(0xCBF4_3926, jpeg)]);
    let file = from_pack(dir.path(), &rom_on_gbc_shelf(dir.path()))
        .unwrap()
        .unwrap();
    assert_eq!(file, dir.path().join("Labels/GBC/Anything.png"));
    // Made for the Game Boy label, fitted to the Game Boy Color shelf the cart sits on.
    let label = image::open(file).unwrap();
    assert_eq!((label.width(), label.height()), label_size(Platform::Gbc));
}

#[test]
fn gba_video_uses_gba_artwork_and_writes_to_its_own_shelf() {
    let dir = tempfile::tempdir().unwrap();
    let rom = dir.path().join("Games/GBAV/Video.gba");
    std::fs::create_dir_all(rom.parent().unwrap()).unwrap();
    std::fs::write(&rom, b"123456789").unwrap();
    let jpeg = build::cart_label_jpeg(&image_bytes(1000, 574), Platform::Gbav).unwrap();
    write_pack(dir.path(), vec![(0xCBF4_3926, jpeg)]);
    let cart = Cart {
        platform: Platform::Gbav,
        rom,
        ..cart("Video")
    };
    let label = from_pack(dir.path(), &cart).unwrap().unwrap();
    assert_eq!(label, dir.path().join("Labels/GBAV/Video.png"));
    let image = image::open(label).unwrap();
    assert_eq!((image.width(), image.height()), label_size(Platform::Gba));
}

#[test]
fn an_existing_label_is_kept_and_the_pack_is_not_read() {
    let dir = tempfile::tempdir().unwrap();
    let cart = rom_on_gbc_shelf(dir.path());
    let custom = dir.path().join("Labels/GBC/Anything.png");
    std::fs::create_dir_all(custom.parent().unwrap()).unwrap();
    std::fs::write(&custom, b"custom").unwrap();
    std::fs::write(dir.path().join("Labels").join(build::PACK), b"SLOTART1junk").unwrap();
    assert_eq!(from_pack(dir.path(), &cart).unwrap(), Some(custom.clone()));
    assert_eq!(std::fs::read(custom).unwrap(), b"custom");
}

#[test]
fn no_pack_a_pack_without_the_rom_or_a_damaged_pack_leaves_the_cart_unlabelled() {
    for case in ["none", "other", "damaged"] {
        let dir = tempfile::tempdir().unwrap();
        match case {
            "other" => write_pack(dir.path(), vec![(0x1234, b"other".to_vec())]),
            "damaged" => {
                std::fs::create_dir_all(dir.path().join("Labels")).unwrap();
                std::fs::write(dir.path().join("Labels").join(build::PACK), b"SLOTART1junk")
                    .unwrap();
            }
            _ => {}
        }
        assert_eq!(
            from_pack(dir.path(), &rom_on_gbc_shelf(dir.path())).unwrap(),
            None,
            "{case}"
        );
        assert!(!dir.path().join("Labels/GBC").exists(), "{case}");
    }
}
