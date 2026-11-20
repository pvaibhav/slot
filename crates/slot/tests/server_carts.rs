use slot::server::{render_once, Server};
use slot_store::{scan, Cart, Platform, ShellChoice};
use slot_ui::{cart_backing, cart_face, cart_interior, gb_shell_of, shows_interior};
use std::io::{Cursor, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant, SystemTime};

const AUTH: &str = "c2xvdDoxMjM0";
const CGB_FLAG: usize = 0x143;

static NEVER: AtomicBool = AtomicBool::new(false);

fn start() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(dir.path().to_path_buf(), 0, "1234".to_string()).unwrap();
    (dir, server)
}

fn get_with(port: u16, target: &str, auth: bool) -> (u16, String, Vec<u8>) {
    let line = if auth {
        format!("Authorization: Basic {AUTH}\r\n")
    } else {
        String::new()
    };
    let raw = format!("GET {target} HTTP/1.1\r\nHost: x\r\n{line}\r\n");
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..split]).into_owned();
    let status = head
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    (status, head, out[split + 4..].to_vec())
}

fn get(port: u16, target: &str) -> (u16, String, Vec<u8>) {
    get_with(port, target, true)
}

fn put(root: &Path, rel: &str, data: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, data).unwrap();
}

fn label_png(seed: u8) -> Vec<u8> {
    let (w, h) = (300u32, 200u32);
    let mut rgba = Vec::new();
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, seed, 255]);
        }
    }
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    out
}

fn gb_rom(flag: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[CGB_FLAG] = flag;
    rom
}

fn row(port: u16, tag: &str, id: usize) -> serde_json::Value {
    let (status, _, body) = get(port, &format!("/api/games/{tag}"));
    assert_eq!(status, 200);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    v["games"][id].clone()
}

fn cart_url(port: u16, tag: &str, id: usize) -> String {
    row(port, tag, id)["cartUrl"].as_str().unwrap().to_string()
}

fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let mut d = image_webp::WebPDecoder::new(Cursor::new(bytes)).unwrap();
    let (w, h) = d.dimensions();
    assert!(d.has_alpha());
    let mut rgba = vec![0u8; d.output_buffer_size().unwrap()];
    d.read_image(&mut rgba).unwrap();
    (w, h, rgba)
}

fn scanned(root: &Path, platform: Platform, id: usize) -> Cart {
    let mut cart = scan(root)
        .unwrap()
        .into_iter()
        .filter(|c| c.platform == platform)
        .nth(id)
        .unwrap();
    cart.read_header();
    cart
}

fn over(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let sa = u32::from(s[3]);
        let da = u32::from(d[3]) * (255 - sa) / 255;
        let a = sa + da;
        if a == 0 {
            continue;
        }
        for c in 0..3 {
            d[c] = ((u32::from(s[c]) * sa + u32::from(d[c]) * da + a / 2) / a) as u8;
        }
        d[3] = a as u8;
    }
}

fn drawn(cart: &Cart) -> Vec<u8> {
    let face = cart_face(cart).rgba;
    if !shows_interior(cart) {
        return face;
    }
    let mut out = cart_interior(cart).rgba;
    over(&mut out, &cart_backing(gb_shell_of(cart)).rgba);
    over(&mut out, &face);
    out
}

fn set_mtime(path: &Path, ago_secs: u64) {
    let when = SystemTime::now() - Duration::from_secs(ago_secs);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

fn cached_files(root: &Path, tag: &str) -> Vec<PathBuf> {
    let dir = root.join("Cache/server-carts").join(tag);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries.map(|e| e.unwrap().path()).collect()
}

#[test]
fn cart_is_served_as_an_image() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    put(dir.path(), "Labels/GBA/Alpha.png", &label_png(1));
    let url = cart_url(server.port(), "GBA", 0);
    assert!(url.starts_with("/api/games/GBA/0/cart?v="), "{url}");
    let (status, _, _) = get_with(server.port(), &url, false);
    assert_eq!(status, 401);
    let (status, head, body) = get(server.port(), &url);
    assert_eq!(status, 200, "{head}");
    assert!(head.contains("image/webp"), "{head}");
    assert!(head.contains("immutable"), "{head}");
    let (w, h, _) = decode(&body);
    assert_eq!((w, h), (360, 212));
    server.stop();
}

#[test]
fn an_old_key_is_not_cached_forever() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let (status, head, _) = get(server.port(), "/api/games/GBA/0/cart?v=0");
    assert_eq!(status, 200, "{head}");
    assert!(!head.contains("immutable"), "{head}");
    server.stop();
}

#[test]
fn cart_refuses_a_write() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let raw = format!(
        "DELETE /api/games/GBA/0/cart HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\n\r\n"
    );
    let mut s = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    assert_eq!(out.split_whitespace().nth(1), Some("405"), "{out}");
    server.stop();
}

#[test]
fn cart_matches_cart_face() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    put(dir.path(), "Labels/GBA/Alpha.png", &label_png(2));
    let (_, _, body) = get(server.port(), &cart_url(server.port(), "GBA", 0));
    let face = cart_face(&scanned(dir.path(), Platform::Gba, 0));
    let (w, h, rgba) = decode(&body);
    assert_eq!((w, h), (face.w, face.h));
    assert!(rgba == face.rgba, "served pixels differ from cart_face");
    server.stop();
}

#[test]
fn clear_shells_layer_interior_backing_and_face() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBC/Alpha.gbc", &gb_rom(0xc0));
    let choice = ShellChoice::parse("rounded 3366aa clear").unwrap();
    put(
        dir.path(),
        slot_store::LABELS_SHELL_FILE,
        format!("Alpha={}\n", choice.to_value()).as_bytes(),
    );
    let cart = scanned(dir.path(), Platform::Gbc, 0);
    assert!(shows_interior(&cart));
    let (_, _, body) = get(server.port(), &cart_url(server.port(), "GBC", 0));
    let (_, _, rgba) = decode(&body);
    assert!(
        rgba == drawn(&cart),
        "served pixels differ from the layered cart"
    );
    server.stop();
}

#[test]
fn changed_label_changes_the_key() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    put(dir.path(), "Labels/GBA/Alpha.png", &label_png(3));
    set_mtime(&dir.path().join("Labels/GBA/Alpha.png"), 100);
    let port = server.port();
    let before = cart_url(port, "GBA", 0);
    let (_, _, old) = get(port, &before);
    assert_eq!(cached_files(dir.path(), "GBA").len(), 1);
    put(dir.path(), "Labels/GBA/Alpha.png", &label_png(200));
    let after = cart_url(port, "GBA", 0);
    assert_ne!(before, after);
    let (_, _, new) = get(port, &after);
    assert_ne!(decode(&old).2, decode(&new).2);
    assert_eq!(
        cached_files(dir.path(), "GBA").len(),
        1,
        "the stale render stays behind"
    );
    server.stop();
}

#[test]
fn changed_shell_changes_the_key() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    let before = cart_url(port, "GBA", 0);
    let (_, _, old) = get(port, &before);
    let choice = ShellChoice::parse("notched cc2211 solid").unwrap();
    put(
        dir.path(),
        slot_store::LABELS_SHELL_FILE,
        format!("Alpha={}\n", choice.to_value()).as_bytes(),
    );
    let after = cart_url(port, "GBA", 0);
    assert_ne!(before, after);
    let (_, _, new) = get(port, &after);
    assert_ne!(decode(&old).2, decode(&new).2);
    server.stop();
}

#[test]
fn second_request_uses_the_cache() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    let url = cart_url(port, "GBA", 0);
    get(port, &url);
    let files = cached_files(dir.path(), "GBA");
    assert_eq!(files.len(), 1);
    std::fs::write(&files[0], b"from the cache").unwrap();
    let (status, _, body) = get(port, &url);
    assert_eq!(status, 200);
    assert_eq!(body, b"from the cache");
    server.stop();
}

#[test]
fn no_label_gets_a_generated_cart() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let row = row(server.port(), "GBA", 0);
    assert_eq!(row["hasArt"], false);
    let (status, _, body) = get(server.port(), row["cartUrl"].as_str().unwrap());
    assert_eq!(status, 200);
    let face = cart_face(&scanned(dir.path(), Platform::Gba, 0));
    assert!(
        decode(&body).2 == face.rgba,
        "served pixels differ from the generated cart"
    );
    server.stop();
}

#[test]
fn concurrent_requests_render_once() {
    let dir = tempfile::tempdir().unwrap();
    let dest = Arc::new(dir.path().join("one.webp"));
    let renders = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let (dest, renders, gate) = (dest.clone(), renders.clone(), gate.clone());
            std::thread::spawn(move || {
                gate.wait();
                render_once(&dest, &NEVER, || {
                    renders.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(50));
                    Some(b"cart".to_vec())
                })
            })
        })
        .collect();
    for t in threads {
        assert!(t.join().unwrap());
    }
    assert_eq!(renders.load(Ordering::SeqCst), 1);
}

#[test]
fn concurrent_requests_over_http_agree() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    let url = Arc::new(cart_url(port, "GBA", 0));
    let threads: Vec<_> = (0..6)
        .map(|_| {
            let url = url.clone();
            std::thread::spawn(move || get(port, &url))
        })
        .collect();
    let replies: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert!(replies.iter().all(|r| r.0 == 200 && r.2 == replies[0].2));
    assert_eq!(cached_files(dir.path(), "GBA").len(), 1);
    server.stop();
}

#[test]
fn at_most_two_render_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let running = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let threads: Vec<_> = (0..6)
        .map(|i| {
            let dest = dir.path().join(format!("{i}.webp"));
            let (running, peak) = (running.clone(), peak.clone());
            std::thread::spawn(move || {
                render_once(&dest, &NEVER, || {
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(30));
                    running.fetch_sub(1, Ordering::SeqCst);
                    Some(b"cart".to_vec())
                })
            })
        })
        .collect();
    for t in threads {
        assert!(t.join().unwrap());
    }
    assert_eq!(peak.load(Ordering::SeqCst), 2);
}

#[test]
fn stop_returns_promptly_with_renders_queued() {
    let dir = tempfile::tempdir().unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let started = Arc::new(Barrier::new(3));
    let release = Arc::new(AtomicBool::new(false));
    let holders: Vec<_> = (0..2)
        .map(|i| {
            let dest = dir.path().join(format!("held{i}.webp"));
            let (started, release) = (started.clone(), release.clone());
            std::thread::spawn(move || {
                render_once(&dest, &NEVER, || {
                    started.wait();
                    while !release.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Some(b"cart".to_vec())
                })
            })
        })
        .collect();
    started.wait();
    let rendered = Arc::new(AtomicUsize::new(0));
    let queued: Vec<_> = (0..4)
        .map(|i| {
            let dest = dir.path().join(format!("queued{i}.webp"));
            let (stop, rendered) = (stop.clone(), rendered.clone());
            std::thread::spawn(move || {
                render_once(&dest, &stop, || {
                    rendered.fetch_add(1, Ordering::SeqCst);
                    Some(b"cart".to_vec())
                })
            })
        })
        .collect();
    std::thread::sleep(Duration::from_millis(100));
    let begun = Instant::now();
    stop.store(true, Ordering::SeqCst);
    for t in queued {
        assert!(!t.join().unwrap());
    }
    assert!(
        begun.elapsed() < Duration::from_millis(500),
        "{:?}",
        begun.elapsed()
    );
    assert_eq!(rendered.load(Ordering::SeqCst), 0);
    release.store(true, Ordering::SeqCst);
    for t in holders {
        assert!(t.join().unwrap());
    }
}

#[test]
fn a_cached_cart_is_served_without_a_scan() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    let url = cart_url(port, "GBA", 0);
    let (_, _, first) = get(port, &url);
    std::fs::remove_file(dir.path().join("Games/GBA/Alpha.gba")).unwrap();
    let (status, head, body) = get(port, &url);
    assert_eq!(status, 200, "{head}");
    assert!(head.contains("immutable"), "{head}");
    assert_eq!(body, first);
    server.stop();
}

#[test]
fn v_cannot_reach_outside_the_cache() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    put(dir.path(), "Secret.webp", b"secret");
    let (status, _, body) = get(
        server.port(),
        "/api/games/GBA/0/cart?v=..%2F..%2F..%2FSecret",
    );
    assert_eq!(status, 200);
    assert_ne!(body, b"secret");
    server.stop();
}

fn post(port: u16, target: &str) -> u16 {
    let raw = format!("POST {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Length: 0\r\n\r\n");
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out.split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

#[test]
fn rename_removes_the_cached_cart() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    get(port, &cart_url(port, "GBA", 0));
    assert_eq!(cached_files(dir.path(), "GBA").len(), 1);
    assert_eq!(post(port, "/api/games/GBA/0/rename?name=Omega.gba"), 200);
    assert!(cached_files(dir.path(), "GBA").is_empty());
    server.stop();
}

#[test]
fn purge_removes_the_cached_cart() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBA/Alpha.gba", b"rom");
    let port = server.port();
    get(port, &cart_url(port, "GBA", 0));
    assert_eq!(cached_files(dir.path(), "GBA").len(), 1);
    let raw = format!("DELETE /api/games/GBA/0?purge=true&rom=Alpha.gba HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\n\r\n");
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    assert_eq!(out.split_whitespace().nth(1), Some("200"), "{out}");
    assert!(cached_files(dir.path(), "GBA").is_empty());
    server.stop();
}

#[test]
fn a_failed_render_frees_its_turn() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("cart.webp");
    assert!(!render_once(&dest, &NEVER, || None));
    assert!(render_once(&dest, &NEVER, || Some(b"cart".to_vec())));
}

#[test]
fn gb_mould_follows_header() {
    let (dir, server) = start();
    put(dir.path(), "Games/GBC/Dual.gbc", &gb_rom(0x80));
    put(dir.path(), "Games/GBC/Only.gbc", &gb_rom(0xc0));
    let port = server.port();
    let dual = decode(&get(port, &cart_url(port, "GBC", 0)).2).2;
    let only = decode(&get(port, &cart_url(port, "GBC", 1)).2).2;
    let cart_dual = scanned(dir.path(), Platform::Gbc, 0);
    let cart_only = scanned(dir.path(), Platform::Gbc, 1);
    assert_eq!(gb_shell_of(&cart_dual), Some(slot_ui::GbShell::Notched));
    assert_eq!(gb_shell_of(&cart_only), Some(slot_ui::GbShell::Rounded));
    assert!(
        dual == drawn(&cart_dual),
        "dual mode pak drew the wrong mould"
    );
    assert!(
        only == drawn(&cart_only),
        "color only pak drew the wrong mould"
    );
    server.stop();
}
