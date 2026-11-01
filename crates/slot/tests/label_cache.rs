use std::path::Path;

use slot::label_cache::{cache_path, fresh, label_art, stale, Rebuild, SCREEN_FROM};
use slot_store::{Cart, Platform};
use slot_ui::label_size;

fn label_png(path: &Path, w: u32, h: u32, rgb: [u8; 3]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(file, w, h);
    enc.set_color(png::ColorType::Rgb);
    let mut writer = enc.write_header().unwrap();
    let px: Vec<u8> = (0..w * h).flat_map(|_| rgb).collect();
    writer.write_image_data(&px).unwrap();
}

fn cart_with_label(root: &Path, rgb: [u8; 3]) -> Cart {
    let label = root.join("Labels/GBA/Emerald.png");
    label_png(&label, 640, 330, rgb);
    Cart {
        platform: Platform::Gba,
        stem: "Emerald".into(),
        rom: root.join("Games/GBA/Emerald.gba"),
        label: Some(label),
        title: String::new(),
        code: String::new(),
        shell: None,
    }
}

#[test]
fn a_scaled_label_is_cached_outside_the_studio_folder_and_read_back() {
    let d = tempfile::tempdir().unwrap();
    let cart = cart_with_label(d.path(), [200, 10, 10]);
    let first = label_art(d.path(), &cart).expect("no art from a good label");
    let cached = cache_path(d.path(), &cart);
    assert!(cached.starts_with(d.path().join("Cache/Labels")));
    assert!(cached.is_file(), "the scaled label was not cached");
    let (w, h) = label_size(&cart);
    assert_eq!(first.len(), (w * h * 4) as usize);
    assert_eq!(label_art(d.path(), &cart), Some(first));
}

#[test]
fn a_redrawn_label_replaces_its_cached_copy() {
    let d = tempfile::tempdir().unwrap();
    let cart = cart_with_label(d.path(), [200, 10, 10]);
    let red = label_art(d.path(), &cart).unwrap();
    label_png(cart.label.as_ref().unwrap(), 600, 330, [10, 10, 200]);
    let blue = label_art(d.path(), &cart).unwrap();
    assert_ne!(red, blue, "the old label came back from the cache");
    assert_eq!(&blue[..3], &[10, 10, 200]);
}

#[test]
fn a_truncated_cache_file_is_rebuilt() {
    let d = tempfile::tempdir().unwrap();
    let cart = cart_with_label(d.path(), [200, 10, 10]);
    let good = label_art(d.path(), &cart).unwrap();
    let cached = cache_path(d.path(), &cart);
    let bytes = std::fs::read(&cached).unwrap();
    std::fs::write(&cached, &bytes[..bytes.len() / 2]).unwrap();
    assert!(
        !fresh(d.path(), &cart),
        "a cut-off cache file counted as fresh"
    );
    assert_eq!(label_art(d.path(), &cart), Some(good));
    assert_eq!(std::fs::read(&cached).unwrap(), bytes);
}

#[test]
fn a_label_that_will_not_decode_is_remembered_so_it_is_not_rebuilt_every_boot() {
    let d = tempfile::tempdir().unwrap();
    let mut cart = cart_with_label(d.path(), [200, 10, 10]);
    let bad = d.path().join("Labels/GBA/Broken.png");
    std::fs::write(&bad, b"not a png").unwrap();
    cart.label = Some(bad);
    assert!(!fresh(d.path(), &cart));
    assert_eq!(label_art(d.path(), &cart), None);
    assert!(
        fresh(d.path(), &cart),
        "a bad label stays stale and rebuilds forever"
    );
    assert_eq!(label_art(d.path(), &cart), None);
}

#[test]
fn a_rebuild_caches_every_stale_label_and_counts_to_the_end() {
    let d = tempfile::tempdir().unwrap();
    let carts: Vec<Cart> = (0..6)
        .map(|i| {
            let mut c = cart_with_label(d.path(), [i * 40, 10, 10]);
            c.stem = format!("Cart {i}");
            let label = d.path().join(format!("Labels/GBA/{}.png", c.stem));
            std::fs::rename(c.label.as_ref().unwrap(), &label).unwrap();
            c.label = Some(label);
            c
        })
        .collect();
    let todo = stale(d.path(), carts.iter());
    assert_eq!(todo.len(), 6);
    let rebuild = Rebuild::start(d.path(), todo);
    let began = std::time::Instant::now();
    while !rebuild.finished() {
        assert!(began.elapsed().as_secs() < 30, "the rebuild never finished");
        std::thread::yield_now();
    }
    assert_eq!((rebuild.done(), rebuild.total()), (6, 6));
    assert!(stale(d.path(), carts.iter()).is_empty());
}

#[test]
fn a_rebuild_that_stops_moving_lets_the_shelf_come_up() {
    let d = tempfile::tempdir().unwrap();
    let mut cart = cart_with_label(d.path(), [10, 10, 10]);
    let stuck = d.path().join("Labels/GBA/Stuck.png");
    let made = std::process::Command::new("mkfifo")
        .arg(&stuck)
        .status()
        .unwrap();
    assert!(made.success());
    cart.stem = "Stuck".into();
    cart.label = Some(stuck);
    let mut rebuild =
        Rebuild::start_with_stall(d.path(), vec![cart], std::time::Duration::from_millis(300));
    let began = std::time::Instant::now();
    while !rebuild.stalled() {
        assert!(
            began.elapsed().as_secs() < 5,
            "a label read that never returned held the screen"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!rebuild.finished());
}

#[test]
fn a_rebuild_that_keeps_moving_is_not_stalled() {
    let d = tempfile::tempdir().unwrap();
    let cart = cart_with_label(d.path(), [10, 10, 10]);
    let mut rebuild =
        Rebuild::start_with_stall(d.path(), vec![cart], std::time::Duration::from_secs(5));
    let began = std::time::Instant::now();
    while !rebuild.finished() {
        assert!(began.elapsed().as_secs() < 30, "the rebuild never finished");
        assert!(!rebuild.stalled());
        std::thread::yield_now();
    }
}

#[test]
fn ensure_moves_an_existing_system_cache_to_the_root() {
    let d = tempfile::tempdir().unwrap();
    let old = d.path().join("System/Cache/Labels/GBA/x.bin");
    std::fs::create_dir_all(old.parent().unwrap()).unwrap();
    std::fs::write(&old, b"art").unwrap();
    slot::root::ensure(d.path());
    assert_eq!(
        std::fs::read(d.path().join("Cache/Labels/GBA/x.bin")).unwrap(),
        b"art"
    );
    assert!(!d.path().join("System/Cache").exists());
}

#[test]
fn ensure_drops_the_old_cache_when_the_new_one_exists() {
    let d = tempfile::tempdir().unwrap();
    let old = d.path().join("System/Cache/Labels/GBA/old.bin");
    let new = d.path().join("Cache/Labels/GBA/new.bin");
    for p in [&old, &new] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"art").unwrap();
    }
    slot::root::ensure(d.path());
    assert!(new.is_file());
    assert!(!d.path().join("Cache/Labels/GBA/old.bin").exists());
    assert!(!d.path().join("System/Cache").exists());
}

#[test]
fn only_a_big_rebuild_takes_the_screen() {
    let d = tempfile::tempdir().unwrap();
    let cart = cart_with_label(d.path(), [10, 10, 10]);
    let batch = |n: usize| Rebuild::start(d.path(), vec![cart.clone(); n]);
    assert!(!batch(SCREEN_FROM - 1).on_screen());
    assert!(batch(SCREEN_FROM).on_screen());
}
