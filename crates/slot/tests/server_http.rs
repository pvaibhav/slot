use slot::server::{parse_target, qr_modules, qr_url, shown_address, Reply, Server};
use std::net::Ipv4Addr;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

fn start(pin: &str) -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(dir.path().to_path_buf(), 0, pin.to_string()).unwrap();
    (dir, server)
}

fn request(port: u16, raw: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

fn b64(input: &str) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.as_bytes().chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |a, (i, b)| a | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn auth_status_is_open() {
    let (_d, server) = start("1234");
    let r = request(server.port(), "GET /api/auth HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.ends_with("{\"required\":true}"), "{r}");
    server.stop();
}

#[test]
fn api_needs_the_pin() {
    let (_d, server) = start("1234");
    let p = server.port();
    let r = request(p, "GET /api/anything HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 401"), "{r}");
    let bad = format!("GET /api/anything HTTP/1.1\r\nAuthorization: Basic {}\r\n\r\n", b64("slot:0000"));
    assert!(request(p, &bad).starts_with("HTTP/1.1 401"));
    let wrong_user = format!("GET /api/anything HTTP/1.1\r\nAuthorization: Basic {}\r\n\r\n", b64("nonna:1234"));
    assert!(request(p, &wrong_user).starts_with("HTTP/1.1 401"));
    let good = format!("GET /api/anything HTTP/1.1\r\nAuthorization: Basic {}\r\n\r\n", b64("slot:1234"));
    assert!(!request(p, &good).starts_with("HTTP/1.1 401"));
    server.stop();
}

#[test]
fn percent_encoded_path_decodes() {
    let (path, query) = parse_target("/api/roms/Game%20Boy/a%2Bb%25.gba?x=1%202&y=a+b&z");
    assert_eq!(path, vec!["api", "roms", "Game Boy", "a+b%.gba"]);
    assert_eq!(query["x"], "1 2");
    assert_eq!(query["y"], "a b");
    assert_eq!(query["z"], "");
}

#[test]
fn stop_frees_the_port() {
    let (d, server) = start("1234");
    let port = server.port();
    assert_ne!(port, 0);
    let _ = request(port, "GET /api/auth HTTP/1.1\r\n\r\n");
    server.stop();
    let again = Server::start(d.path().to_path_buf(), port, "1234".into()).unwrap();
    assert_eq!(again.port(), port);
    again.stop();
}

#[test]
fn stop_during_a_stalled_upload_returns() {
    let dir = tempfile::tempdir().unwrap();
    let handler: slot::server::Handler = std::sync::Arc::new(|root, req| {
        match req.body.save_to(&root.join("up.bin")) {
            Ok(()) => Reply::json(serde_json::json!({})),
            Err(_) => Reply::error(500, "failed"),
        }
    });
    let server = Server::start_with(dir.path().to_path_buf(), 0, "1234".into(), handler).unwrap();
    let auth = b64("slot:1234");
    let mut s = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    write!(
        s,
        "POST /api/upload/x HTTP/1.1\r\nAuthorization: Basic {auth}\r\nContent-Length: 1000\r\n\r\nhalf"
    )
    .unwrap();
    let part = dir.path().join("up.bin.part");
    let t = Instant::now();
    while !part.exists() && t.elapsed() < Duration::from_secs(3) {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(part.exists());
    let t = Instant::now();
    server.stop();
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    assert!(!part.exists());
    assert!(!dir.path().join("up.bin").exists());
}

#[test]
fn stop_during_a_stalled_download_returns() {
    let dir = tempfile::tempdir().unwrap();
    let big = dir.path().join("big.bin");
    std::fs::write(&big, vec![7u8; 64 * 1024 * 1024]).unwrap();
    let target = big.clone();
    let handler: slot::server::Handler = std::sync::Arc::new(move |_, _| Reply::file(target.clone()));
    let server = Server::start_with(dir.path().to_path_buf(), 0, "1234".into(), handler).unwrap();
    let auth = b64("slot:1234");
    let mut s = TcpStream::connect(("127.0.0.1", server.port())).unwrap();
    write!(s, "GET /api/file HTTP/1.1\r\nAuthorization: Basic {auth}\r\n\r\n").unwrap();
    let mut first = [0u8; 16];
    s.read_exact(&mut first).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let t = Instant::now();
    server.stop();
    assert!(t.elapsed() < Duration::from_secs(3), "{:?}", t.elapsed());
    drop(s);
}

#[test]
fn malformed_content_length_is_400() {
    let (_d, server) = start("1234");
    let auth = b64("slot:1234");
    let raw = format!("POST /api/x HTTP/1.1\r\nAuthorization: Basic {auth}\r\nContent-Length: abc\r\n\r\n");
    assert!(request(server.port(), &raw).starts_with("HTTP/1.1 400"));
    server.stop();
}

#[test]
fn generated_pin_is_four_digits() {
    let pin = slot::server::generate_pin();
    assert_eq!(pin.len(), 4);
    assert!(pin.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn body_save_is_atomic_and_cleans_up() {
    use slot::server::Body;
    let dir = tempfile::tempdir().unwrap();
    let ok = dir.path().join("a.bin");
    Body::from_bytes(b"hello".to_vec()).save_to(&ok).unwrap();
    assert_eq!(std::fs::read(&ok).unwrap(), b"hello");
    assert!(!dir.path().join("a.bin.part").exists());

    let bad = dir.path().join("b.bin");
    let short = Body::new(Box::new(std::io::Cursor::new(b"abc".to_vec())), 10);
    assert!(short.save_to(&bad).is_err());
    assert!(!bad.exists());
    assert!(!dir.path().join("b.bin.part").exists());
}

fn with_web() -> (tempfile::TempDir, Server) {
    let (d, server) = start("1234");
    let web = d.path().join("System").join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<html>slot</html>").unwrap();
    std::fs::write(web.join("assets").join("app.js"), "let a = 1;").unwrap();
    std::fs::write(d.path().join("System").join("secret.txt"), "hidden").unwrap();
    (d, server)
}

#[test]
fn root_serves_index() {
    let (_d, server) = with_web();
    let r = request(server.port(), "GET / HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.contains("text/html"), "{r}");
    assert!(r.ends_with("<html>slot</html>"), "{r}");
    server.stop();
}

#[test]
fn router_paths_fall_back_to_index() {
    let (_d, server) = with_web();
    let r = request(server.port(), "GET /platform/GBA HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(r.starts_with("HTTP/1.1 200"), "{r}");
    assert!(r.ends_with("<html>slot</html>"), "{r}");
    let guide = request(
        server.port(),
        "GET /platform/GBA/game/3/guides/Walkthrough.txt HTTP/1.1\r\nHost: x\r\n\r\n",
    );
    assert!(guide.ends_with("<html>slot</html>"), "{guide}");
    let missing = request(server.port(), "GET /assets/none.js HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");
    server.stop();
}

#[test]
fn web_folder_is_served_though_system_is_refused_by_api() {
    let (_d, server) = with_web();
    let p = server.port();
    let js = request(p, "GET /assets/app.js HTTP/1.1\r\nHost: x\r\n\r\n");
    assert!(js.starts_with("HTTP/1.1 200"), "{js}");
    assert!(js.contains("text/javascript"), "{js}");
    assert!(js.ends_with("let a = 1;"), "{js}");
    let auth = b64("slot:1234");
    let api = format!("GET /api/fs/card/System/web/index.html HTTP/1.1\r\nAuthorization: Basic {auth}\r\n\r\n");
    assert!(request(p, &api).starts_with("HTTP/1.1 403"));
    server.stop();
}

#[test]
fn static_paths_cannot_leave_the_web_folder() {
    let (_d, server) = with_web();
    let p = server.port();
    for target in ["/..%2Fsecret.txt", "/../secret.txt", "/assets/%2e%2e/%2e%2e/secret.txt", "/assets/..%5C..%5Csecret.txt", "/..%20/secret.txt", "/.../secret.txt", "/.. /secret.txt"] {
        let r = request(p, &format!("GET {target} HTTP/1.1\r\nHost: x\r\n\r\n"));
        assert!(!r.contains("hidden"), "{target}: {r}");
        assert!(!r.starts_with("HTTP/1.1 200"), "{target}: {r}");
    }
    server.stop();
}

#[test]
fn qr_url_has_pin() {
    assert_eq!(qr_url(Ipv4Addr::new(192, 168, 1, 42), 80, "4821"), "http://192.168.1.42/?pin=4821");
}

#[test]
fn fallback_url_has_port() {
    assert_eq!(qr_url(Ipv4Addr::new(192, 168, 1, 42), 2003, "4821"), "http://192.168.1.42:2003/?pin=4821");
}

#[test]
fn shown_address_omits_80() {
    assert_eq!(shown_address("192.168.1.42", 80), "192.168.1.42");
    assert_eq!(shown_address("192.168.1.42", 2003), "192.168.1.42:2003");
}

#[test]
fn longest_address_fits_version_3() {
    let url = qr_url(Ipv4Addr::new(192, 168, 100, 200), 2003, "4821");
    let m = qr_modules(&url).unwrap();
    assert_eq!(m.len(), 29);
    assert!(m.iter().all(|row| row.len() == 29));
}

#[test]
fn a_full_server_reads_the_head_before_refusing() {
    let dir = tempfile::tempdir().unwrap();
    let release = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hold = release.clone();
    let handler: slot::server::Handler = std::sync::Arc::new(move |_, _| {
        while !hold.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(5));
        }
        Reply::Empty(204)
    });
    let server = Server::start_with(dir.path().to_path_buf(), 0, "1234".to_string(), handler).unwrap();
    let port = server.port();
    let raw = "GET /page HTTP/1.1\r\nHost: x\r\n\r\n";
    let held: Vec<_> = (0..8)
        .map(|_| {
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            s.write_all(raw.as_bytes()).unwrap();
            s
        })
        .collect();
    std::thread::sleep(Duration::from_millis(100));
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    std::thread::sleep(Duration::from_millis(50));
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    assert!(out.starts_with("HTTP/1.1 503"), "{out}");
    release.store(true, std::sync::atomic::Ordering::SeqCst);
    drop(held);
    server.stop();
}

#[test]
fn oversized_url_has_no_qr() {
    assert!(qr_modules(&"a".repeat(200)).is_none());
}
