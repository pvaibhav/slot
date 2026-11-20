use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use slot::art::index::ArtIndex;
use slot::art::jobs::{
    box_art_paths, fetch_box_art_with, fetch_missing_with, load_index, ArtEvent, IndexFetch,
};
use slot::art::make::Kind;
use slot::update::UpdateFail;

const ART: &str = "https://art.slot-cfw.fyi/";

fn encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(rgba).unwrap();
    }
    out
}

fn scan_png() -> Vec<u8> {
    encode(600, 310, &[90, 60, 200, 255].repeat(600 * 310))
}

fn size_of(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    let info = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    (info.info().width, info.info().height)
}

fn card(roms: &[(&str, &[u8])]) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    for (name, bytes) in roms {
        std::fs::write(d.path().join("Games/GBA").join(name), bytes).unwrap();
    }
    d
}

fn rom(seed: u8) -> Vec<u8> {
    let mut r = vec![seed; 0x200];
    r[0] = seed;
    r
}

fn index_for(entries: &[(u32, &str)]) -> ArtIndex {
    let body: Vec<String> = entries
        .iter()
        .map(|(crc, fields)| format!("\"{crc:08X}\":{{{fields}}}"))
        .collect();
    ArtIndex::parse(&format!("{{{}}}", body.join(","))).unwrap()
}

fn run(
    root: &Path,
    kind: Kind,
    index: &ArtIndex,
    served: HashMap<String, Vec<u8>>,
    cancel_after: Option<usize>,
) -> Vec<ArtEvent> {
    let mut events = Vec::new();
    let seen = AtomicUsize::new(0);
    let fetch = |url: &str| served.get(url).cloned().ok_or(UpdateFail::Download);
    let cancelled = || cancel_after.is_some_and(|n| seen.load(Ordering::SeqCst) >= n);
    fetch_missing_with(root, kind, index, &fetch, &cancelled, &mut |e| {
        if matches!(e, ArtEvent::Landed { .. }) {
            seen.fetch_add(1, Ordering::SeqCst);
        }
        events.push(e);
    });
    events
}

#[test]
fn real_label_lands_from_the_index() {
    let a = rom(1);
    let d = card(&[("A.gba", &a)]);
    let crc = crc32fast::hash(&a);
    let index = index_for(&[(crc, r#""support-texture":"GBA/support-texture/a.png""#)]);
    let served = HashMap::from([(format!("{ART}GBA/support-texture/a.png"), scan_png())]);
    let cart = slot::art::rom::missing_art(d.path()).remove(0);
    let events = run(d.path(), Kind::Real, &index, served, None);
    assert!(
        slot::label_cache::cache_path(d.path(), &cart).is_file(),
        "the shelf would decode it cold"
    );
    assert!(matches!(events[0], ArtEvent::Counted(1)), "{events:?}");
    assert!(
        matches!(&events[1], ArtEvent::Landed { stem, .. } if stem == "A"),
        "{events:?}"
    );
    assert!(
        matches!(&events[2], ArtEvent::Done { landed: 1, missed } if missed.is_empty()),
        "{events:?}"
    );
    assert_eq!(size_of(&d.path().join("Labels/GBA/A.png")), (1280, 661));
}

#[test]
fn logo_falls_back_to_real_when_no_wheel() {
    let a = rom(2);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"GBA/support-texture/a.png""#,
    )]);
    let served = HashMap::from([(format!("{ART}GBA/support-texture/a.png"), scan_png())]);
    let events = run(d.path(), Kind::Logo, &index, served, None);
    assert!(
        events.iter().any(|e| matches!(e, ArtEvent::Landed { .. })),
        "{events:?}"
    );
}

#[test]
fn unknown_rom_is_missed() {
    let a = rom(3);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(0xDEADBEEF, r#""wheel":"x.png""#)]);
    let events = run(d.path(), Kind::Real, &index, HashMap::new(), None);
    assert!(
        matches!(&events[1], ArtEvent::Missed { stem } if stem == "A"),
        "{events:?}"
    );
    assert!(
        matches!(&events[2], ArtEvent::Done { landed: 0, missed } if missed == &vec!["A".to_string()]),
        "{events:?}"
    );
    assert!(!d.path().join("Labels/GBA/A.png").exists());
}

#[test]
fn html_from_the_server_is_missed() {
    let a = rom(4);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"GBA/support-texture/a.png""#,
    )]);
    let served = HashMap::from([(
        format!("{ART}GBA/support-texture/a.png"),
        b"<html>oops</html>".to_vec(),
    )]);
    let events = run(d.path(), Kind::Real, &index, served, None);
    assert!(matches!(&events[1], ArtEvent::Missed { .. }), "{events:?}");
    assert!(!d.path().join("Labels/GBA/A.png").exists());
}

#[test]
fn a_label_that_appears_is_kept() {
    let a = rom(5);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"GBA/support-texture/a.png""#,
    )]);
    let served = HashMap::from([(format!("{ART}GBA/support-texture/a.png"), scan_png())]);
    let mut events = Vec::new();
    let fetch = |url: &str| {
        std::fs::create_dir_all(d.path().join("Labels/GBA")).unwrap();
        std::fs::write(d.path().join("Labels/GBA/A.png"), b"mine").unwrap();
        served.get(url).cloned().ok_or(UpdateFail::Download)
    };
    fetch_missing_with(d.path(), Kind::Real, &index, &fetch, &|| false, &mut |e| {
        events.push(e)
    });
    assert_eq!(
        std::fs::read(d.path().join("Labels/GBA/A.png")).unwrap(),
        b"mine"
    );
    assert!(
        !events.iter().any(|e| matches!(e, ArtEvent::Landed { .. })),
        "{events:?}"
    );
}

#[test]
fn cancel_stops_between_carts() {
    let roms: Vec<Vec<u8>> = (20..30).map(rom).collect();
    let names: Vec<String> = (0..10).map(|i| format!("C{i}.gba")).collect();
    let pairs: Vec<(&str, &[u8])> = names
        .iter()
        .map(String::as_str)
        .zip(roms.iter().map(Vec::as_slice))
        .collect();
    let d = card(&pairs);
    let entries: Vec<(u32, &str)> = roms
        .iter()
        .map(|r| {
            (
                crc32fast::hash(r),
                r#""support-texture":"GBA/support-texture/a.png""#,
            )
        })
        .collect();
    let index = index_for(&entries);
    let served = HashMap::from([(format!("{ART}GBA/support-texture/a.png"), scan_png())]);
    let events = run(d.path(), Kind::Real, &index, served, Some(1));
    let landed = events
        .iter()
        .filter(|e| matches!(e, ArtEvent::Landed { .. }))
        .count();
    assert!((1..10).contains(&landed), "{events:?}");
    assert!(
        matches!(events.last(), Some(ArtEvent::Done { .. })),
        "{events:?}"
    );
}

const INDEX: &str = r#"{"0000ABCD":{"wheel":"w.png"}}"#;

#[test]
fn a_fresh_index_is_cached_with_its_etag() {
    let d = tempfile::tempdir().unwrap();
    let mut asked = Vec::new();
    let index = load_index(d.path(), &mut |etag| {
        asked.push(etag.map(str::to_string));
        Ok(IndexFetch::Fresh {
            body: INDEX.as_bytes().to_vec(),
            etag: Some("\"v1\"".into()),
        })
    })
    .unwrap();
    assert!(index.get(0xABCD).is_some());
    assert_eq!(asked, vec![None]);
    let mut asked = Vec::new();
    let again = load_index(d.path(), &mut |etag| {
        asked.push(etag.map(str::to_string));
        Ok(IndexFetch::NotModified)
    })
    .unwrap();
    assert!(again.get(0xABCD).is_some());
    assert_eq!(asked, vec![Some("\"v1\"".to_string())]);
}

#[test]
fn an_offline_run_uses_the_cache() {
    let d = tempfile::tempdir().unwrap();
    load_index(d.path(), &mut |_| {
        Ok(IndexFetch::Fresh {
            body: INDEX.as_bytes().to_vec(),
            etag: None,
        })
    })
    .unwrap();
    let index = load_index(d.path(), &mut |_| Err(UpdateFail::Offline)).unwrap();
    assert!(index.get(0xABCD).is_some());
}

#[test]
fn no_cache_and_no_network_fails() {
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(
        load_index(d.path(), &mut |_| Err(UpdateFail::Offline)),
        Err(UpdateFail::Offline)
    ));
}

#[test]
fn losing_the_network_ends_the_run() {
    let (a, b) = (rom(8), rom(9));
    let d = card(&[("A.gba", &a), ("B.gba", &b)]);
    let index = index_for(&[
        (
            crc32fast::hash(&a),
            r#""support-texture":"GBA/support-texture/a.png""#,
        ),
        (
            crc32fast::hash(&b),
            r#""support-texture":"GBA/support-texture/a.png""#,
        ),
    ]);
    let mut events = Vec::new();
    let fetch = |_: &str| Err(UpdateFail::Offline);
    fetch_missing_with(d.path(), Kind::Real, &index, &fetch, &|| false, &mut |e| {
        events.push(e)
    });
    assert!(
        matches!(events.last(), Some(ArtEvent::Failed(UpdateFail::Offline))),
        "{events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, ArtEvent::Missed { .. })),
        "{events:?}"
    );
}

#[test]
fn the_cached_index_survives_a_boot() {
    let d = tempfile::tempdir().unwrap();
    let mut get = |_: Option<&str>| {
        Ok(IndexFetch::Fresh {
            body: b"{}".to_vec(),
            etag: Some("\"v1\"".into()),
        })
    };
    load_index(d.path(), &mut get).unwrap();
    slot::root::ensure(d.path());
    let mut asked = None;
    let mut again = |etag: Option<&str>| {
        asked = etag.map(str::to_string);
        Ok(IndexFetch::NotModified)
    };
    load_index(d.path(), &mut again).unwrap();
    assert_eq!(asked.as_deref(), Some("\"v1\""));
}

#[test]
fn a_broken_cache_sends_no_etag() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("Cache/art");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.json"), b"").unwrap();
    std::fs::write(dir.join("index.etag"), b"\"v1\"").unwrap();
    let mut asked = Vec::new();
    let index = load_index(d.path(), &mut |etag| {
        asked.push(etag.map(str::to_string));
        Ok(IndexFetch::Fresh {
            body: INDEX.as_bytes().to_vec(),
            etag: None,
        })
    })
    .unwrap();
    assert_eq!(asked, vec![None]);
    assert!(index.get(0xABCD).is_some());
}

#[test]
fn a_new_run_does_not_undo_a_stop() {
    let mut runs = slot::art::jobs::Runs::default();
    let first = runs.start();
    runs.cancel();
    let second = runs.start();
    assert!(first.load(std::sync::atomic::Ordering::SeqCst));
    assert!(!second.load(std::sync::atomic::Ordering::SeqCst));
}

#[test]
fn carts_run_at_the_same_time() {
    let roms: Vec<Vec<u8>> = (40..46).map(rom).collect();
    let names: Vec<String> = (0..6).map(|i| format!("P{i}.gba")).collect();
    let pairs: Vec<(&str, &[u8])> = names
        .iter()
        .map(String::as_str)
        .zip(roms.iter().map(Vec::as_slice))
        .collect();
    let d = card(&pairs);
    let entries: Vec<(u32, &str)> = roms
        .iter()
        .map(|r| (crc32fast::hash(r), r#""support-texture":"a.png""#))
        .collect();
    let index = index_for(&entries);
    let (now, peak) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let scan = scan_png();
    let fetch = |_: &str| {
        let n = now.fetch_add(1, Ordering::SeqCst) + 1;
        peak.fetch_max(n, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(60));
        now.fetch_sub(1, Ordering::SeqCst);
        Ok(scan.clone())
    };
    let mut landed = 0;
    fetch_missing_with(d.path(), Kind::Real, &index, &fetch, &|| false, &mut |e| {
        landed += matches!(e, ArtEvent::Landed { .. }) as usize;
    });
    assert_eq!(landed, 6);
    assert_eq!(peak.load(Ordering::SeqCst), 3);
}

mod one_cart {
    use super::*;
    use slot::art::jobs::studio_art_with;
    use slot::art::make::{apply_label, keep_existing, kept_label, label_path, STUDIO_CACHE};

    fn wheel() -> Vec<u8> {
        encode(200, 80, &[255, 255, 255, 255].repeat(200 * 80))
    }

    fn setup() -> (tempfile::TempDir, slot_store::Cart, Vec<u8>) {
        let a = rom(10);
        let d = card(&[("A.gba", &a)]);
        let cart = slot::art::rom::missing_art(d.path()).remove(0);
        (d, cart, a)
    }

    #[test]
    fn both_kinds_come_back() {
        let (d, cart, a) = setup();
        let index = index_for(&[(
            crc32fast::hash(&a),
            r#""support-texture":"r.png","wheel":"w.png""#,
        )]);
        let served = HashMap::from([
            (format!("{ART}r.png"), scan_png()),
            (format!("{ART}w.png"), wheel()),
        ]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            served.get(u).cloned().ok_or(UpdateFail::Download)
        });
        assert_eq!(size_of(art.real.as_ref().unwrap()), (1280, 661));
        assert_eq!(size_of(art.logo.as_ref().unwrap()), (1280, 661));
        assert!(art.wheel.as_ref().unwrap().is_file());
        assert!(art.failed.is_none());
    }

    #[test]
    fn a_webp_wheel_is_kept_as_a_png() {
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""wheel":"w.png""#)]);
        let served = HashMap::from([(format!("{ART}w.webp"), webp_wheel())]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            served.get(u).cloned().ok_or(UpdateFail::Download)
        });
        assert_eq!(size_of(art.wheel.as_ref().unwrap()), (400, 200));
    }

    #[test]
    fn a_label_the_cart_wears_is_not_offered() {
        let (d, cart, a) = setup();
        let index = index_for(&[(
            crc32fast::hash(&a),
            r#""support-texture":"r.png","wheel":"w.png""#,
        )]);
        let served = HashMap::from([
            (format!("{ART}r.png"), scan_png()),
            (format!("{ART}w.png"), wheel()),
        ]);
        let mut fetch = |u: &str| served.get(u).cloned().ok_or(UpdateFail::Download);
        let first = studio_art_with(d.path(), &cart, &index, &mut fetch);
        apply_label(d.path(), &cart, first.real.as_deref()).unwrap();
        let art = studio_art_with(d.path(), &cart, &index, &mut fetch);
        assert!(art.real.is_none());
        assert!(art.logo.is_some());
        assert!(art.duplicates);
    }

    fn grained(rgb: [u8; 3], w: u32, h: u32) -> Vec<u8> {
        let rgba: Vec<u8> = (0..w * h)
            .flat_map(|i| {
                let n = (i % 7) as u8;
                [rgb[0] + n, rgb[1] + n, rgb[2] + n, 255]
            })
            .collect();
        encode(w, h, &rgba)
    }

    #[test]
    fn a_reencoded_copy_of_the_worn_label_is_not_offered() {
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""support-texture":"r.png""#)]);
        let first = HashMap::from([(
            format!("{ART}r.png"),
            encode(600, 310, &[90, 60, 200, 255].repeat(600 * 310)),
        )]);
        let got = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            first.get(u).cloned().ok_or(UpdateFail::Download)
        });
        apply_label(d.path(), &cart, got.real.as_deref()).unwrap();
        let again = HashMap::from([(format!("{ART}r.webp"), grained([87, 57, 197], 600, 310))]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            again.get(u).cloned().ok_or(UpdateFail::Download)
        });
        assert!(art.real.is_none());
        assert!(art.duplicates);
    }

    #[test]
    fn a_different_label_is_still_offered() {
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""support-texture":"r.png""#)]);
        let first = HashMap::from([(format!("{ART}r.png"), grained([90, 60, 200], 600, 310))]);
        let got = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            first.get(u).cloned().ok_or(UpdateFail::Download)
        });
        apply_label(d.path(), &cart, got.real.as_deref()).unwrap();
        let again = HashMap::from([(format!("{ART}r.png"), grained([120, 60, 200], 600, 310))]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            again.get(u).cloned().ok_or(UpdateFail::Download)
        });
        assert!(art.real.is_some());
        assert!(!art.duplicates);
    }

    #[test]
    fn an_old_logo_label_is_recognized_and_tagged() {
        use slot_ui::house_label::{logo_tag, untagged};
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""wheel":"w.png""#)]);
        let served = HashMap::from([(format!("{ART}w.png"), wheel())]);
        let mut fetch = |u: &str| served.get(u).cloned().ok_or(UpdateFail::Download);
        let first = studio_art_with(d.path(), &cart, &index, &mut fetch);
        let made = std::fs::read(first.logo.as_ref().unwrap()).unwrap();
        let label = label_path(d.path(), &cart);
        std::fs::create_dir_all(label.parent().unwrap()).unwrap();
        std::fs::write(&label, untagged(&made)).unwrap();
        let art = studio_art_with(d.path(), &cart, &index, &mut fetch);
        assert!(art.logo.is_none(), "offered the label it already wears");
        assert!(art.wheel.is_some());
        assert_eq!(art.logo_worn, logo_tag(&made));
        assert_eq!(logo_tag(&std::fs::read(&label).unwrap()), logo_tag(&made));
    }

    #[test]
    fn one_missing_kind_leaves_the_other() {
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""support-texture":"r.png""#)]);
        let served = HashMap::from([(format!("{ART}r.png"), scan_png())]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |u: &str| {
            served.get(u).cloned().ok_or(UpdateFail::Download)
        });
        assert!(art.real.is_some());
        assert!(art.logo.is_none());
    }

    #[test]
    fn offline_reports_failed() {
        let (d, cart, a) = setup();
        let index = index_for(&[(crc32fast::hash(&a), r#""support-texture":"r.png""#)]);
        let art = studio_art_with(d.path(), &cart, &index, &mut |_: &str| {
            Err(UpdateFail::Offline)
        });
        assert_eq!(art.failed, Some(UpdateFail::Offline));
    }

    #[test]
    fn apply_then_existing_restores_the_bytes() {
        let (d, cart, _a) = setup();
        let path = label_path(d.path(), &cart);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"mine").unwrap();
        let new = d.path().join("new.png");
        std::fs::write(&new, b"theirs").unwrap();
        keep_existing(d.path(), &cart).unwrap();
        apply_label(d.path(), &cart, Some(&new)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"theirs");
        let kept = d.path().join(STUDIO_CACHE).join("existing.png");
        apply_label(d.path(), &cart, Some(&kept)).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"mine");
    }

    #[test]
    fn a_recolor_keeps_the_band_and_automatic_keeps_the_served_ground() {
        use slot::art::index::Look;
        use slot_ui::house_label::{logo_tag, Band, Edge};
        let (d, cart, _a) = setup();
        let dir = d.path().join(STUDIO_CACHE);
        let label = label_path(d.path(), &cart);
        std::fs::create_dir_all(label.parent().unwrap()).unwrap();
        let band = Band {
            edge: Edge::Left,
            size: 0.1,
            colour: [5, 150, 60],
        };
        let served = Look {
            ground: [120, 30, 30],
            band: Some(band),
        };
        let save = |ground| {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("wheel.png"), wheel()).unwrap();
            slot::art::jobs::studio_save(d.path(), &cart, ground, Some(&served));
            std::fs::read(&label).unwrap()
        };
        let plain = slot::art::make::logo_label(&wheel(), None, &cart.stem, cart.platform).unwrap();
        let recolored = save(Some([20, 90, 60]));
        let want = Look {
            ground: [20, 90, 60],
            band: Some(band),
        };
        let want =
            slot::art::make::logo_label(&wheel(), Some(&want), &cart.stem, cart.platform).unwrap();
        assert_eq!(recolored, want);
        let automatic = save(None);
        assert_ne!(automatic, plain);
        assert_eq!(logo_tag(&automatic), Some([120, 30, 30]));
    }

    #[test]
    fn a_same_second_same_size_rewrite_is_not_served_from_the_cache() {
        let (d, mut cart, _a) = setup();
        let path = label_path(d.path(), &cart);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let (red, blue) = (d.path().join("red.png"), d.path().join("blue.png"));
        std::fs::write(&red, encode(4, 4, &[200, 10, 10, 255].repeat(16))).unwrap();
        std::fs::write(&blue, encode(4, 4, &[10, 10, 200, 255].repeat(16))).unwrap();
        assert_eq!(
            std::fs::metadata(&red).unwrap().len(),
            std::fs::metadata(&blue).unwrap().len()
        );
        cart.label = Some(path.clone());
        apply_label(d.path(), &cart, Some(&red)).unwrap();
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        let old = slot::label_cache::label_art(d.path(), &cart).unwrap();
        apply_label(d.path(), &cart, Some(&blue)).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(first).unwrap();
        drop(file);
        let new = slot::label_cache::label_art(d.path(), &cart).unwrap();
        assert_ne!(old, new, "the cache served the old art");
    }

    #[test]
    fn title_removes_the_label() {
        let (d, cart, _a) = setup();
        let path = label_path(d.path(), &cart);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"mine").unwrap();
        apply_label(d.path(), &cart, None).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn putting_back_needs_the_kept_label_when_the_cart_had_one() {
        let (d, mut cart, _a) = setup();
        assert_eq!(kept_label(d.path(), &cart).unwrap(), None);
        let path = label_path(d.path(), &cart);
        cart.label = Some(path.clone());
        assert!(kept_label(d.path(), &cart).is_err());
        let kept = d.path().join(STUDIO_CACHE).join("existing.png");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, b"mine").unwrap();
        assert_eq!(kept_label(d.path(), &cart).unwrap(), Some(kept));
    }
}

#[test]
fn a_fetch_cancelled_before_it_starts_still_ends() {
    use slot::art::jobs::{art_jobs, ArtEvent, ArtJob};
    use slot::art::make::Kind;
    let d = card(&[]);
    let mut jobs = art_jobs(d.path().to_path_buf());
    jobs.ask(ArtJob::FetchMissing(Kind::Real));
    jobs.ask(ArtJob::Cancel);
    for _ in 0..500 {
        if matches!(
            jobs.take(),
            Some(ArtEvent::Done { .. } | ArtEvent::Failed(_))
        ) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    panic!("the fetch never reported an end");
}

fn webp_of(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    image_webp::WebPEncoder::new(&mut out)
        .encode(rgba, w, h, image_webp::ColorType::Rgba8)
        .unwrap();
    out
}

fn webp_wheel() -> Vec<u8> {
    let mut rgba = vec![0u8; 400 * 200 * 4];
    for px in rgba.chunks_exact_mut(4).skip(400 * 80).take(400 * 40) {
        px.copy_from_slice(&[240, 240, 240, 255]);
    }
    webp_of(400, 200, &rgba)
}

#[test]
fn the_webp_is_asked_for_first_and_the_png_is_left_alone() {
    let a = rom(1);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"GBA/support-texture/a.png""#,
    )]);
    let webp = webp_of(600, 310, &[90, 60, 200, 255].repeat(600 * 310));
    let asked = std::sync::Mutex::new(Vec::new());
    let fetch = |url: &str| {
        asked.lock().unwrap().push(url.to_string());
        match url.ends_with(".webp") {
            true => Ok(webp.clone()),
            false => Err(UpdateFail::Download),
        }
    };
    fetch_missing_with(d.path(), Kind::Real, &index, &fetch, &|| false, &mut |_| {});
    assert_eq!(size_of(&d.path().join("Labels/GBA/A.png")), (1280, 661));
    assert_eq!(
        asked.into_inner().unwrap(),
        vec![format!("{ART}GBA/support-texture/a.webp")]
    );
}

#[test]
fn a_logo_label_lands_from_a_webp_wheel() {
    let a = rom(1);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(crc32fast::hash(&a), r#""wheel":"GBA/wheel/a.png""#)]);
    let served = HashMap::from([(format!("{ART}GBA/wheel/a.webp"), webp_wheel())]);
    run(d.path(), Kind::Logo, &index, served, None);
    assert!(d.path().join("Labels/GBA/A.png").exists());
}

#[test]
fn a_lost_connection_on_the_webp_does_not_try_the_png() {
    let a = rom(1);
    let d = card(&[("A.gba", &a)]);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"GBA/support-texture/a.png""#,
    )]);
    let asked = AtomicUsize::new(0);
    let fetch = |_: &str| {
        asked.fetch_add(1, Ordering::SeqCst);
        Err(UpdateFail::Offline)
    };
    fetch_missing_with(d.path(), Kind::Real, &index, &fetch, &|| false, &mut |_| {});
    assert_eq!(asked.into_inner(), 1);
}

#[cfg(feature = "nds")]
#[test]
fn fetch_missing_art_labels_a_ds_game() {
    let a = rom(9);
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), &a).unwrap();
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""support-texture":"NDS/support-texture/k.png""#,
    )]);
    let scan = encode(541, 600, &[200, 40, 40, 255].repeat(541 * 600));
    let served = HashMap::from([(format!("{ART}NDS/support-texture/k.png"), scan)]);
    let events = run(d.path(), Kind::Real, &index, served, None);
    assert!(
        matches!(&events[1], ArtEvent::Landed { stem, .. } if stem == "Kart"),
        "{events:?}"
    );
    let (w, h) = size_of(&d.path().join("Labels/NDS/Kart.png"));
    assert!(
        (0.85..1.0).contains(&(w as f32 / h as f32)),
        "a DS label came out {w}x{h}, not nearly square"
    );
}

fn boxes(
    root: &Path,
    index: &ArtIndex,
    served: &HashMap<String, Vec<u8>>,
    cancel: bool,
) -> (Vec<ArtEvent>, usize) {
    let mut events = Vec::new();
    let asked = AtomicUsize::new(0);
    let fetch = |url: &str| {
        asked.fetch_add(1, Ordering::SeqCst);
        served.get(url).cloned().ok_or(UpdateFail::Download)
    };
    fetch_box_art_with(root, index, &fetch, &|| cancel, &mut |e| events.push(e));
    (events, asked.load(Ordering::SeqCst))
}

fn labelled_card(seed: u8) -> (tempfile::TempDir, Vec<u8>) {
    let a = rom(seed);
    let d = card(&[("A.gba", &a)]);
    std::fs::create_dir_all(d.path().join("Labels/GBA")).unwrap();
    std::fs::write(d.path().join("Labels/GBA/A.png"), scan_png()).unwrap();
    (d, a)
}

fn box_served() -> HashMap<String, Vec<u8>> {
    HashMap::from([
        (
            format!("{ART}GBA/box-3D/a.png"),
            encode(40, 50, &[200, 0, 0, 255].repeat(40 * 50)),
        ),
        (
            format!("{ART}GBA/wheel/a.png"),
            encode(60, 30, &[255, 255, 255, 255].repeat(60 * 30)),
        ),
    ])
}

#[test]
fn fetch_box_art_saves_box_and_logo_for_a_labelled_cart() {
    let (d, a) = labelled_card(11);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""box-3D":"GBA/box-3D/a.png","wheel":"GBA/wheel/a.png""#,
    )]);
    let (events, _) = boxes(d.path(), &index, &box_served(), false);
    let cart = slot_store::scan(d.path()).unwrap().remove(0);
    let (boxed, logo) = box_art_paths(d.path(), &cart);
    assert_eq!(size_of(&boxed), (40, 50));
    assert_eq!(size_of(&logo), (60, 30));
    assert!(matches!(events[0], ArtEvent::Counted(1)), "{events:?}");
    assert!(
        matches!(&events[1], ArtEvent::BoxLanded { stem, .. } if stem == "A"),
        "{events:?}"
    );
    assert!(
        matches!(&events[2], ArtEvent::Done { landed: 1, missed } if missed.is_empty()),
        "{events:?}"
    );
}

#[test]
fn fetch_box_art_skips_a_cart_it_already_has() {
    let (d, a) = labelled_card(12);
    let index = index_for(&[(
        crc32fast::hash(&a),
        r#""box-3D":"GBA/box-3D/a.png","wheel":"GBA/wheel/a.png""#,
    )]);
    boxes(d.path(), &index, &box_served(), false);
    let (events, asked) = boxes(d.path(), &index, &box_served(), false);
    assert_eq!(asked, 0, "the second run fetched again");
    assert!(matches!(events[0], ArtEvent::Counted(0)), "{events:?}");
}

#[test]
fn fetch_box_art_misses_a_cart_the_server_does_not_have() {
    let (d, _) = labelled_card(13);
    let index = index_for(&[]);
    let (events, _) = boxes(d.path(), &index, &box_served(), false);
    assert!(
        matches!(&events.last().unwrap(), ArtEvent::Done { landed: 0, missed } if missed == &vec!["A".to_string()]),
        "{events:?}"
    );
}

#[test]
fn fetch_box_art_stops_when_cancelled() {
    let (d, a) = labelled_card(14);
    let index = index_for(&[(crc32fast::hash(&a), r#""box-3D":"GBA/box-3D/a.png""#)]);
    let (events, asked) = boxes(d.path(), &index, &box_served(), true);
    assert_eq!(asked, 0);
    assert!(
        matches!(&events.last().unwrap(), ArtEvent::Done { landed: 0, .. }),
        "{events:?}"
    );
}
