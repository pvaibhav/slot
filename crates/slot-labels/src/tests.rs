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
fn title_resolution_requires_unique_exact_gba_match() {
    let item = |id, name, platform| {
        format!("<a href='/games/details/{id}-game'><h3>{name}</h3><p>{platform}</p></a>")
    };
    let correct = item(1, "Mario Kart: Super Circuit", "Nintendo Game Boy Advance");
    let page = format!(
        "{}{}{}",
        correct,
        item(2, "Mario Kart: Super Circuit", "Nintendo DS"),
        item(
            3,
            "Mario Kart: Super Circuit 2",
            "Nintendo Game Boy Advance"
        )
    );
    assert_eq!(
        source::game_id(&page, "mario kart super circuit").unwrap(),
        1
    );
    assert!(source::game_id(
        &(correct.clone() + &item(4, "Mario Kart: Super Circuit", "Nintendo Game Boy Advance")),
        "mario kart super circuit"
    )
    .is_err());
    assert_eq!(
        source::game_id(&(correct.clone() + &correct), "mario kart super circuit").unwrap(),
        1
    );
    assert!(source::game_id("<html>changed layout</html>", "mario kart super circuit").is_err());
}

#[test]
fn artwork_is_cartridge_front_for_the_requested_region() {
    let page = r#"
        <a href="https://images.launchbox-app.com/box.png" data-title="Test - Box - Front Image (North America)"></a>
        <a href="https://images.launchbox-app.com/eu.png" data-title="Test - Cart - Front Image (Europe)"></a>
        <a href="https://evil.invalid/us.png" data-title="Test - Cart - Front Image (North America)"></a>
        <a href="https://images.launchbox-app.com/us.png" data-title="Test - Cart - Front Image (North America)"></a>
    "#;
    assert!(source::image_url(page, &cart("Test (USA)"))
        .unwrap()
        .ends_with("/us.png"));
    assert!(source::image_url(page, &cart("Test (Europe)"))
        .unwrap()
        .ends_with("/eu.png"));
    assert!(source::image_url(page, &cart("Test (Japan)")).is_err());
    for url in [
        "http://images.launchbox-app.com/x",
        "https://images.launchbox-app.com.evil/x",
        "file:///tmp/x",
    ] {
        assert!(!source::allowed_url(url));
    }
}

#[test]
fn crop_produces_exact_rgb_png_and_rejects_bad_input() {
    for (w, h) in [(1000, 574), (600, 355), (473, 283), (800, 465)] {
        let png = artwork::prepare(&image_bytes(w, h)).unwrap();
        assert_eq!(image::guess_format(&png).unwrap(), ImageFormat::Png);
        let im = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(im.dimensions(), (196, 86));
        assert_eq!(im.get_pixel(98, 0), &Rgb([255, 0, 0]));
        assert_eq!(im.get_pixel(98, 85), &Rgb([0, 0, 255]));
    }
    assert!(artwork::prepare(b"<html>Error</html>").is_err());
    assert!(artwork::prepare(&image_bytes(100, 100)).is_err());
    assert!(artwork::prepare(&image_bytes(5000, 2)).is_err());
}

struct Fake {
    calls: Vec<String>,
    image: Vec<u8>,
}
impl Transport for Fake {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, Error> {
        self.calls.push(url.into());
        if url.contains("/games/images/") {
            Ok(br#"<a href="https://images.launchbox-app.com/cart.png" data-title="Advance Wars - Cart - Front Image (North America)"></a>"#.to_vec())
        } else {
            Ok(self.image.clone())
        }
    }
}

#[test]
fn end_to_end_writes_exact_filename_and_skips_existing_without_network() {
    let dir = tempfile::tempdir().unwrap();
    let cart = cart("Advance Wars (USA) (Rev 1)");
    let mut http = Fake {
        calls: vec![],
        image: image_bytes(600, 355),
    };
    let mut cache = HashMap::new();
    let file = prepare(&mut http, &mut cache, dir.path(), &cart).unwrap();
    assert_eq!(
        file,
        dir.path().join("Labels/GBA/Advance Wars (USA) (Rev 1).png")
    );
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(image::load_from_memory(&bytes).unwrap().width(), 196);
    assert_eq!(http.calls.len(), 2);
    prepare(&mut http, &mut cache, dir.path(), &cart).unwrap();
    assert_eq!(http.calls.len(), 2);
    assert_eq!(std::fs::read(file).unwrap(), bytes);
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
fn corrupt_download_is_not_published_and_can_be_retried() {
    let dir = tempfile::tempdir().unwrap();
    let cart = cart("Advance Wars (USA)");
    let mut http = Fake {
        calls: vec![],
        image: b"truncated".to_vec(),
    };
    let mut cache = HashMap::new();
    assert!(prepare(&mut http, &mut cache, dir.path(), &cart).is_err());
    assert!(!dir.path().join("Labels").exists());
    http.image = image_bytes(600, 355);
    assert!(prepare(&mut http, &mut cache, dir.path(), &cart).is_ok());
    assert_eq!(
        http.calls.len(),
        3,
        "retry should reuse resolved source URL"
    );
}

#[test]
#[ignore = "live public artwork service; run manually"]
fn live_download() {
    let dir = tempfile::tempdir().unwrap();
    let mut downloader = Downloader::default();
    for name in ["Advance Wars (USA)", "Mario Kart - Super Circuit (USA)"] {
        let path = downloader.prepare(dir.path(), &cart(name)).unwrap();
        let image = image::open(path).unwrap();
        assert_eq!((image.width(), image.height()), (196, 86));
    }
}
