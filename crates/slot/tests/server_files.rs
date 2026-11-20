use slot::server::{card_name, rename_via_temp, resolve, Server};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::path::Path;
use std::time::Duration;

const AUTH: &str = "c2xvdDoxMjM0";

fn start() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(dir.path().to_path_buf(), 0, "1234".to_string()).unwrap();
    (dir, server)
}

fn send(port: u16, raw: &[u8], close_write: bool) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(raw).unwrap();
    if close_write {
        s.shutdown(Shutdown::Write).unwrap();
    }
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    (status, body)
}

fn call(port: u16, method: &str, target: &str, body: &[u8]) -> (u16, String) {
    let mut raw = format!(
        "{method} {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(body);
    send(port, &raw, false)
}

fn multipart(name: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = "----slotboundary42";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(data);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn upload(port: u16, target: &str, name: &str, data: &[u8]) -> (u16, String) {
    let (ctype, body) = multipart(name, data);
    let mut raw = format!(
        "POST {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(&body);
    send(port, &raw, false)
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn dotdot_is_refused() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    assert!(resolve(d.path(), &s(&["Games", "..", "x"])).is_err());
    assert!(resolve(d.path(), &s(&["Games", ".."])).is_err());
    assert!(resolve(d.path(), &s(&["Games", ""])).is_err());
    assert!(resolve(d.path(), &s(&["/etc", "passwd"])).is_err());
    assert!(resolve(d.path(), &s(&["Games", "a\\b"])).is_err());
    assert!(resolve(d.path(), &[]).is_err());
    let ok = resolve(d.path(), &s(&["Games", "a.gba"])).unwrap();
    assert_eq!(
        ok,
        d.path().canonicalize().unwrap().join("Games").join("a.gba")
    );
}

#[test]
fn encoded_dotdot_is_refused() {
    let (d, server) = start();
    std::fs::write(d.path().join("secret.txt"), b"x").unwrap();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    let p = server.port();
    for target in [
        "/api/roms/%2e%2e/secret.txt",
        "/api/roms/%2E%2E%2Fsecret.txt",
        "/api/roms/..%2Fsecret.txt",
        "/api/fs/card/%2e%2e/secret.txt",
    ] {
        let (status, body) = call(p, "GET", target, b"");
        assert_eq!(status, 403, "{target}");
        assert_eq!(body, "{\"error\":\"forbidden\"}", "{target}");
    }
    server.stop();
}

#[test]
fn system_is_hidden_from_fs() {
    let (d, server) = start();
    std::fs::create_dir(d.path().join("System")).unwrap();
    std::fs::write(d.path().join("System").join("slot"), b"bin").unwrap();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/fs/card", b"");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("Games"), "{body}");
    assert!(!body.contains("System"), "{body}");
    let (status, _) = call(p, "GET", "/api/fs/card/System/slot", b"");
    assert_eq!(status, 403);
    let (status, _) = call(p, "GET", "/api/fs/card/System", b"");
    assert_eq!(status, 403);
    server.stop();
}

#[test]
fn cache_is_hidden_from_fs() {
    let (d, server) = start();
    std::fs::create_dir(d.path().join("Cache")).unwrap();
    std::fs::write(d.path().join("Cache").join("index"), b"i").unwrap();
    let p = server.port();
    let (_, body) = call(p, "GET", "/api/fs/card", b"");
    assert!(!body.contains("Cache"), "{body}");
    let (status, _) = call(p, "GET", "/api/fs/card/Cache/index", b"");
    assert_eq!(status, 403);
    let (status, _) = call(p, "DELETE", "/api/fs/card/Cache/index", b"");
    assert_eq!(status, 403);
    assert!(d.path().join("Cache").join("index").exists());
    server.stop();
}

#[test]
fn system_any_case_is_refused() {
    let (d, server) = start();
    std::fs::create_dir(d.path().join("System")).unwrap();
    std::fs::write(d.path().join("System").join("slot"), b"bin").unwrap();
    let p = server.port();
    for target in [
        "/api/fs/card/SYSTEM/slot",
        "/api/fs/card/system/slot",
        "/api/fs/card/cache",
        "/api/fs/card/CaChE",
    ] {
        let (status, _) = call(p, "GET", target, b"");
        assert_eq!(status, 403, "{target}");
    }
    let (status, _) = call(p, "PUT", "/api/fs/card/SYSTEM/new", b"");
    assert_eq!(status, 403);
    let (status, _) = call(p, "DELETE", "/api/fs/card/SYSTEM/slot", b"");
    assert_eq!(status, 403);
    let body = br#"{"to":"System/slot2"}"#;
    std::fs::create_dir(d.path().join("Games")).unwrap();
    std::fs::write(d.path().join("Games").join("a.gba"), b"a").unwrap();
    let (status, _) = call(p, "PATCH", "/api/fs/card/Games/a.gba", body);
    assert_eq!(status, 403);
    assert!(d.path().join("Games").join("a.gba").exists());
    assert!(d.path().join("System").join("slot").exists());
    assert!(!d.path().join("System").join("slot2").exists());
    server.stop();
}

#[cfg(unix)]
#[test]
fn symlink_out_of_the_card_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret.txt"), b"secret").unwrap();
    let (d, server) = start();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    std::os::unix::fs::symlink(outside.path(), d.path().join("Games").join("escape")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        d.path().join("Games").join("leak.txt"),
    )
    .unwrap();
    let p = server.port();
    for target in [
        "/api/roms/escape",
        "/api/roms/escape/secret.txt",
        "/api/roms/leak.txt",
        "/api/fs/card/Games/escape/secret.txt",
    ] {
        let (status, body) = call(p, "GET", target, b"");
        assert_eq!(status, 403, "{target} {body}");
        assert!(!body.contains("secret"), "{body}");
    }
    let (status, _) = upload(p, "/api/roms/escape", "new.gba", b"x");
    assert_eq!(status, 403);
    assert!(!outside.path().join("new.gba").exists());
    let (status, _) = call(p, "DELETE", "/api/roms/escape/secret.txt", b"");
    assert_eq!(status, 403);
    assert!(outside.path().join("secret.txt").exists());
    assert!(resolve(d.path(), &s(&["Games", "escape", "x"])).is_err());
    server.stop();
}

#[test]
fn upload_lands_in_folder() {
    let (d, server) = start();
    let p = server.port();
    let (status, body) = upload(p, "/api/roms/GBA", "Tetris.gb", b"rom bytes");
    assert_eq!(status, 200, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["ok"], true);
    assert_eq!(json["files"][0], "Tetris.gb");
    assert_eq!(
        std::fs::read(d.path().join("Games").join("GBA").join("Tetris.gb")).unwrap(),
        b"rom bytes"
    );
    let (_, body) = call(p, "GET", "/api/roms/GBA", b"");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["path"], "Games/GBA");
    assert_eq!(json["entries"][0]["name"], "Tetris.gb");
    assert_eq!(json["entries"][0]["type"], "file");
    assert_eq!(json["entries"][0]["size"], 9);
    let (status, body) = call(p, "GET", "/api/roms/GBA/Tetris.gb", b"");
    assert_eq!(status, 200);
    assert_eq!(body, "rom bytes");
    for (resource, folder) in [
        ("art", "Labels"),
        ("overlays", "Bezels"),
        ("saves", "Saves"),
        ("bios", "BIOS"),
        ("wallpapers", "Wallpapers"),
        ("guides", "Guides"),
    ] {
        let (status, body) = upload(p, &format!("/api/{resource}"), "f.bin", b"1");
        assert_eq!(status, 200, "{resource} {body}");
        assert!(d.path().join(folder).join("f.bin").exists(), "{resource}");
    }
    server.stop();
}

#[test]
fn trailing_space_before_extension_is_refused() {
    let (d, server) = start();
    let p = server.port();
    let (status, body) = upload(p, "/api/roms/GB", "Tetris .gb", b"x");
    assert_eq!(status, 400, "{body}");
    assert!(body.contains("Tetris .gb"), "{body}");
    let games = d.path().join("Games").join("GB");
    let leftovers: Vec<_> = std::fs::read_dir(&games)
        .map(|r| r.collect())
        .unwrap_or_default();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    server.stop();
}

#[test]
fn card_name_rules() {
    assert!(card_name("Pokémon Ruby.gba").is_ok());
    assert!(card_name("Tetris.gb").is_ok());
    assert!(card_name(".hidden").is_ok());
    assert!(card_name("Tetris .gb").is_err());
    assert!(card_name("Tetris.").is_err());
    assert!(card_name("Tetris ").is_err());
    assert!(card_name("").is_err());
    for c in ['\\', '/', ':', '*', '?', '"', '<', '>', '|', '\u{1}', '\n'] {
        assert!(card_name(&format!("a{c}b.gb")).is_err(), "{c:?}");
    }
    let err = card_name("Tetris .gb").unwrap_err();
    assert_eq!(err.status, 400);
    assert!(err.message.contains("Tetris .gb"));
}

#[test]
fn non_ascii_name_round_trips() {
    let (d, server) = start();
    let p = server.port();
    let (status, body) = upload(p, "/api/roms/GBA", "Pokémon Ruby.gba", b"ruby");
    assert_eq!(status, 200, "{body}");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["files"][0], "Pokémon Ruby.gba");
    assert!(d.path().join("Games/GBA").join("Pokémon Ruby.gba").exists());
    let (_, body) = call(p, "GET", "/api/roms/GBA", b"");
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["entries"][0]["name"], "Pokémon Ruby.gba");
    let (status, body) = call(p, "GET", "/api/roms/GBA/Pok%C3%A9mon%20Ruby.gba", b"");
    assert_eq!((status, body.as_str()), (200, "ruby"));
    let (status, body) = call(
        p,
        "PATCH",
        "/api/roms/GBA/Pok%C3%A9mon%20Ruby.gba",
        "{\"to\":\"GBA/Pokémon Sapphire.gba\"}".as_bytes(),
    );
    assert_eq!(status, 200, "{body}");
    assert!(!d.path().join("Games/GBA/Pokémon Ruby.gba").exists());
    assert!(d.path().join("Games/GBA/Pokémon Sapphire.gba").exists());
    let (status, _) = call(
        p,
        "DELETE",
        "/api/roms/GBA/Pok%C3%A9mon%20Sapphire.gba",
        b"",
    );
    assert_eq!(status, 200);
    assert!(!d.path().join("Games/GBA/Pokémon Sapphire.gba").exists());
    server.stop();
}

#[test]
fn interrupted_upload_keeps_the_old_file() {
    let (d, server) = start();
    let p = server.port();
    let dir = d.path().join("Games").join("GBA");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Tetris.gb"), b"old bytes").unwrap();
    let (ctype, body) = multipart("Tetris.gb", &vec![7u8; 4096]);
    let cut = &body[..body.len() / 2];
    let mut raw = format!(
        "POST /api/roms/GBA HTTP/1.1\r\nAuthorization: Basic {AUTH}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(cut);
    let (status, _) = send(p, &raw, true);
    assert_ne!(status, 200);
    assert_eq!(std::fs::read(dir.join("Tetris.gb")).unwrap(), b"old bytes");
    let names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    server.stop();
}

#[test]
fn upload_replaces_a_file() {
    let (d, server) = start();
    let p = server.port();
    assert_eq!(upload(p, "/api/saves/GBA", "a.sav", b"first").0, 200);
    assert_eq!(upload(p, "/api/saves/GBA", "a.sav", b"second").0, 200);
    let path = d.path().join("Saves/GBA/a.sav");
    assert_eq!(std::fs::read(&path).unwrap(), b"second");
    assert!(!Path::new(&format!("{}.part", path.display())).exists());
    server.stop();
}

#[test]
fn delete_removes_the_file() {
    let (d, server) = start();
    let p = server.port();
    upload(p, "/api/bios", "gba_bios.bin", b"b");
    let (status, body) = call(p, "DELETE", "/api/bios/gba_bios.bin", b"");
    assert_eq!(status, 200);
    assert_eq!(body, "{\"ok\":true}");
    assert!(!d.path().join("BIOS/gba_bios.bin").exists());
    let (status, _) = call(p, "DELETE", "/api/bios/gba_bios.bin", b"");
    assert_eq!(status, 404);
    let (status, _) = call(p, "DELETE", "/api/bios", b"");
    assert_eq!(status, 400);
    server.stop();
}

#[test]
fn mkdir_and_volumes() {
    let (d, server) = start();
    let p = server.port();
    let (status, _) = call(p, "PUT", "/api/roms/New%20Folder", b"");
    assert_eq!(status, 201);
    assert!(d.path().join("Games/New Folder").is_dir());
    let (status, body) = call(p, "PUT", "/api/roms/New%20Folder", b"");
    assert_eq!(status, 200);
    assert!(body.contains("\"existed\":true"), "{body}");
    let (status, body) = call(p, "PUT", "/api/roms/Bad%20", b"");
    assert_eq!(status, 400);
    assert!(body.contains("Bad "), "{body}");
    let (status, body) = call(p, "GET", "/api/fs", b"");
    assert_eq!(status, 200);
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["volumes"][0]["id"], "card");
    assert!(json["volumes"][0]["totalBytes"].is_u64());
    assert!(json["volumes"][0]["freeBytes"].is_u64());
    let (status, _) = call(p, "POST", "/api/fs", b"");
    assert_eq!(status, 405);
    let (status, _) = call(p, "GET", "/api/fs/other", b"");
    assert_eq!(status, 404);
    server.stop();
}

#[test]
fn trailing_dot_and_space_aliases_are_refused() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("System")).unwrap();
    for name in [
        "System.", "cache..", "SYSTEM ", "System. ", "system", "Cache",
    ] {
        let err = resolve(d.path(), &s(&[name, "x"])).unwrap_err();
        assert_eq!(err.status, 403, "{name:?}");
    }
    let err = resolve(d.path(), &s(&["Games", "a."])).unwrap_err();
    assert_eq!(err.status, 400);
    let err = resolve(d.path(), &s(&["Games", "a.gba", "b "])).unwrap_err();
    assert_eq!(err.status, 400);
    let (d, server) = start();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    std::fs::write(d.path().join("Games").join("a.gba"), b"a").unwrap();
    let p = server.port();
    for to in ["System./x", "cache../x", "SYSTEM /x", "System. /x"] {
        let body = format!("{{\"to\":\"{to}\"}}");
        let (status, _) = call(p, "PATCH", "/api/fs/card/Games/a.gba", body.as_bytes());
        assert_eq!(status, 403, "{to}");
    }
    let (status, _) = call(p, "GET", "/api/fs/card/System./x", b"");
    assert_eq!(status, 403);
    assert!(d.path().join("Games").join("a.gba").exists());
    server.stop();
}

#[cfg(unix)]
#[test]
fn symlink_into_system_is_refused() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("System")).unwrap();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    std::os::unix::fs::symlink(d.path().join("System"), d.path().join("Games").join("sys"))
        .unwrap();
    let err = resolve(d.path(), &s(&["Games", "sys", "x"])).unwrap_err();
    assert_eq!(err.status, 403);
}

#[test]
fn upload_named_like_a_hidden_folder_is_refused() {
    let (d, server) = start();
    let p = server.port();
    for name in ["Cache", "system", "System."] {
        let (status, body) = upload(p, "/api/fs/card", name, b"x");
        assert!(status == 403 || status == 400, "{name} {status} {body}");
        assert!(!d.path().join(name).exists(), "{name}");
    }
    let (status, _) = upload(p, "/api/fs/card", "System", b"x");
    assert_eq!(status, 403);
    server.stop();
}

#[test]
fn case_only_rename_works() {
    let (d, server) = start();
    let p = server.port();
    upload(p, "/api/roms/GBA", "tetris.gb", b"t");
    let (status, body) = call(
        p,
        "PATCH",
        "/api/roms/GBA/tetris.gb",
        br#"{"to":"GBA/Tetris.gb"}"#,
    );
    assert_eq!(status, 200, "{body}");
    let names: Vec<String> = std::fs::read_dir(d.path().join("Games/GBA"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["Tetris.gb"]);
    upload(p, "/api/roms/GBA", "Other.gb", b"o");
    let (status, _) = call(
        p,
        "PATCH",
        "/api/roms/GBA/Other.gb",
        br#"{"to":"GBA/Tetris.gb"}"#,
    );
    assert_eq!(status, 409);
    server.stop();
}

#[test]
fn flat_file_rename_at_resource_top() {
    let (d, server) = start();
    let p = server.port();
    upload(p, "/api/bios", "a.bin", b"a");
    let (status, body) = call(p, "PATCH", "/api/bios/a.bin", br#"{"to":"b.bin"}"#);
    assert_eq!(status, 200, "{body}");
    assert!(d.path().join("BIOS/b.bin").exists());
    let (status, _) = call(p, "PATCH", "/api/bios/b.bin", br#"{"to":"sub/b.bin"}"#);
    assert_eq!(status, 403);
    server.stop();
}

#[test]
fn missing_paths_and_put_over_file() {
    let (d, server) = start();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/roms", b"");
    assert_eq!(status, 200);
    assert!(body.contains("\"entries\":[]"), "{body}");
    std::fs::create_dir_all(d.path().join("Games")).unwrap();
    let (status, _) = call(p, "GET", "/api/roms/Nope", b"");
    assert_eq!(status, 404);
    upload(p, "/api/roms/GBA", "a.gb", b"a");
    let (status, _) = call(p, "PUT", "/api/roms/GBA/a.gb", b"");
    assert_eq!(status, 409);
    assert!(d.path().join("Games/GBA/a.gb").is_file());
    server.stop();
}

#[test]
fn rename_via_temp_changes_the_name() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("tetris.gb");
    let dst = d.path().join("Tetris.gb");
    std::fs::write(&src, b"t").unwrap();
    rename_via_temp(&src, &dst, &|a, b| std::fs::rename(a, b)).unwrap();
    let names: Vec<String> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["Tetris.gb"]);
    assert_eq!(std::fs::read(&dst).unwrap(), b"t");
}

#[test]
fn rename_via_temp_restores_the_original_when_the_second_step_fails() {
    let d = tempfile::tempdir().unwrap();
    let src = d.path().join("tetris.gb");
    let dst = d.path().join("Tetris.gb");
    std::fs::write(&src, b"t").unwrap();
    let calls = std::cell::Cell::new(0);
    let result = rename_via_temp(&src, &dst, &|a, b| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(std::io::Error::other("injected"))
        } else {
            std::fs::rename(a, b)
        }
    });
    assert!(result.is_err());
    let names: Vec<String> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["tetris.gb"]);
    assert_eq!(std::fs::read(&src).unwrap(), b"t");
}

fn names_in(body: &str) -> Vec<String> {
    let json: serde_json::Value = serde_json::from_str(body).unwrap();
    json["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn listing_hides_dotfiles() {
    let (d, server) = start();
    let dir = d.path().join("Games").join("GBA");
    std::fs::create_dir_all(dir.join(".Trashes")).unwrap();
    std::fs::write(dir.join(".DS_Store"), b"x").unwrap();
    std::fs::write(dir.join("._Tetris.gba"), b"x").unwrap();
    std::fs::write(dir.join("Tetris.gba"), b"x").unwrap();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/roms/GBA", b"");
    assert_eq!(status, 200);
    assert_eq!(names_in(&body), vec!["Tetris.gba"]);
    let (status, body) = call(p, "GET", "/api/roms/GBA/._Tetris.gba", b"");
    assert_eq!((status, body.as_str()), (200, "x"));
    server.stop();
}

#[test]
fn recursive_listing_hides_dotfiles() {
    let (d, server) = start();
    let dir = d.path().join("Games");
    std::fs::create_dir_all(dir.join("GBA").join(".hidden")).unwrap();
    std::fs::write(dir.join("GBA").join(".hidden").join("inside.gba"), b"x").unwrap();
    std::fs::write(dir.join("GBA").join("._a.gba"), b"x").unwrap();
    std::fs::write(dir.join("GBA").join("a.gba"), b"x").unwrap();
    std::fs::write(dir.join(".DS_Store"), b"x").unwrap();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/roms?recursive=true", b"");
    assert_eq!(status, 200);
    assert_eq!(names_in(&body), vec!["GBA/a.gba"]);
    server.stop();
}

#[test]
fn fs_root_hides_dot_folders() {
    let (d, server) = start();
    std::fs::create_dir(d.path().join(".Spotlight-V100")).unwrap();
    std::fs::create_dir(d.path().join(".Trashes")).unwrap();
    std::fs::write(d.path().join(".DS_Store"), b"x").unwrap();
    std::fs::create_dir(d.path().join("Games")).unwrap();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/fs/card", b"");
    assert_eq!(status, 200);
    assert_eq!(names_in(&body), vec!["Games"]);
    let (status, _) = call(p, "DELETE", "/api/fs/card/.Trashes", b"");
    assert_eq!(status, 403);
    assert!(d.path().join(".Trashes").exists());
    server.stop();
}

#[test]
fn top_level_folders_cannot_be_renamed_or_moved() {
    let (d, server) = start();
    let p = server.port();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    std::fs::create_dir_all(d.path().join("Saves")).unwrap();
    std::fs::write(d.path().join("loose.txt"), b"x").unwrap();
    let (status, _) = call(p, "PATCH", "/api/fs/card/Games", br#"{"to":"Roms"}"#);
    assert_eq!(status, 403);
    let (status, _) = call(p, "PATCH", "/api/fs/card/Games", br#"{"to":"Saves/Games"}"#);
    assert_eq!(status, 403);
    assert!(d.path().join("Games/GBA").is_dir());
    let (status, body) = call(
        p,
        "PATCH",
        "/api/fs/card/loose.txt",
        br#"{"to":"renamed.txt"}"#,
    );
    assert_eq!(status, 200, "{body}");
    assert!(d.path().join("renamed.txt").exists());
    let (status, body) = call(
        p,
        "PATCH",
        "/api/fs/card/Games/GBA",
        br#"{"to":"Games/Advance"}"#,
    );
    assert_eq!(status, 200, "{body}");
    assert!(d.path().join("Games/Advance").is_dir());
    server.stop();
}

#[test]
fn top_level_folders_cannot_be_deleted() {
    let (d, server) = start();
    let p = server.port();
    std::fs::create_dir_all(d.path().join("Games/GBA")).unwrap();
    std::fs::write(d.path().join("Games/GBA/a.gba"), b"rom").unwrap();
    std::fs::write(d.path().join("loose.txt"), b"x").unwrap();
    let (status, _) = call(p, "DELETE", "/api/fs/card/Games", b"");
    assert_eq!(status, 403);
    assert!(d.path().join("Games/GBA/a.gba").exists());
    let (status, _) = call(p, "DELETE", "/api/fs/card/Games/", b"");
    assert_eq!(status, 403);
    assert!(d.path().join("Games/GBA/a.gba").exists());
    let (status, _) = call(p, "DELETE", "/api/fs/card/loose.txt", b"");
    assert_eq!(status, 200);
    assert!(!d.path().join("loose.txt").exists());
    let (status, _) = call(p, "DELETE", "/api/fs/card/Games/GBA", b"");
    assert_eq!(status, 200);
    assert!(!d.path().join("Games/GBA").exists());
    server.stop();
}
