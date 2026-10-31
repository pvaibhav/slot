use slot::server::Server;
use slot_store::{Core, Platform, StateRing};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

const AUTH: &str = "c2xvdDoxMjM0";

fn start() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(dir.path().to_path_buf(), 0, "1234".to_string()).unwrap();
    (dir, server)
}

fn call_full(port: u16, method: &str, target: &str, body: &[u8]) -> (u16, String, String) {
    let mut raw = format!(
        "{method} {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(body);
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(&raw).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let status = text.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let (head, body) = text.split_once("\r\n\r\n").map(|(h, b)| (h.to_string(), b.to_string())).unwrap_or_default();
    (status, head, body)
}

fn call(port: u16, method: &str, target: &str, body: &[u8]) -> (u16, String) {
    let (status, _, body) = call_full(port, method, target, body);
    (status, body)
}

fn put(root: &Path, rel: &str, data: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, data).unwrap();
}

fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("{e}: {body}"))
}

fn library(root: &Path) {
    put(root, "Games/GBA/Alpha.gba", b"alpha-rom");
    put(root, "Games/GBA/Beta.gba", b"beta-rom");
    put(root, "Labels/GBA/Alpha.png", b"png-bytes");
    put(root, "Saves/GBA/Alpha.sav", b"save-bytes");
    put(root, "Guides/GBA/Alpha.txt", b"guide text");
}

#[test]
fn tags_are_the_four_platforms() {
    let (_d, server) = start();
    let (status, body) = call(server.port(), "GET", "/api/tags", b"");
    assert_eq!(status, 200, "{body}");
    let v = json(&body);
    let mut tags: Vec<String> =
        v["tags"].as_array().unwrap().iter().map(|t| t.as_str().unwrap().to_string()).collect();
    tags.sort();
    assert_eq!(tags, ["GB", "GBA", "GBAV", "GBC"]);
    server.stop();
}

#[test]
fn game_lists_with_label_url() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/games/GBA", b"");
    assert_eq!(status, 200, "{body}");
    let v = json(&body);
    assert_eq!(v["platform"], "GBA");
    let games = v["games"].as_array().unwrap();
    assert_eq!(games.len(), 2);
    let alpha = &games[0];
    assert_eq!(alpha["id"], 0);
    assert_eq!(alpha["rom"], "Alpha.gba");
    assert_eq!(alpha["displayName"], "Alpha");
    assert_eq!(alpha["hasArt"], true);
    assert_eq!(alpha["savesCount"], 1);
    assert_eq!(alpha["guidesCount"], 1);
    assert_eq!(alpha["statesCount"], 0);
    assert_eq!(alpha["multiDisc"], false);
    assert!(alpha.get("artUrl").is_none());
    let (status, head, art) = call_full(p, "GET", "/api/games/GBA/0/art", b"");
    assert_eq!(status, 200);
    assert_eq!(art, "png-bytes");
    assert!(head.to_ascii_lowercase().contains("cache-control: public, max-age=31536000, immutable"), "{head}");
    let (status, body) = call(p, "GET", "/api/games/GBA/0", b"");
    assert_eq!(status, 200, "{body}");
    let detail = json(&body);
    assert_eq!(detail["size"], 9);
    assert_eq!(detail["platformDisplayName"], "Game Boy Advance");
    assert!(detail["modified"].as_u64().unwrap() > 0);
    server.stop();
}

#[test]
fn game_without_label_has_no_art() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (_, body) = call(p, "GET", "/api/games/GBA", b"");
    let beta = &json(&body)["games"][1];
    assert_eq!(beta["hasArt"], false);
    let (status, _) = call(p, "GET", "/api/games/GBA/1/art", b"");
    assert_eq!(status, 404);
    server.stop();
}

#[test]
fn rename_moves_save_label_and_guide() {
    let (d, server) = start();
    library(d.path());
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Alpha");
    ring.write_resume(b"resume").unwrap();
    let p = server.port();
    let (status, body) = call(p, "POST", "/api/games/GBA/0/rename?name=Gamma.gba", b"");
    assert_eq!(status, 200, "{body}");
    let r = d.path();
    assert!(r.join("Games/GBA/Gamma.gba").exists());
    assert!(!r.join("Games/GBA/Alpha.gba").exists());
    assert_eq!(std::fs::read(r.join("Saves/GBA/Gamma.sav")).unwrap(), b"save-bytes");
    assert_eq!(std::fs::read(r.join("Labels/GBA/Gamma.png")).unwrap(), b"png-bytes");
    assert_eq!(std::fs::read(r.join("Guides/GBA/Gamma.txt")).unwrap(), b"guide text");
    assert!(!r.join("Saves/GBA/Alpha.sav").exists());
    assert!(!r.join("Labels/GBA/Alpha.png").exists());
    assert!(!r.join("Guides/GBA/Alpha.txt").exists());
    let moved = StateRing::new(r, Platform::Gba, Core::Mgba, "Gamma");
    assert_eq!(moved.read_resume().unwrap().unwrap(), b"resume");
    server.stop();
}

#[test]
fn rename_refuses_bad_names_and_moves_nothing() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    for name in ["Gamma%20.gba", "Gamma.gba%20", "Gamma.", "..%2Fx.gba", "a%3Ab.gba", "", ".hidden.gba"] {
        let (status, body) = call(p, "POST", &format!("/api/games/GBA/0/rename?name={name}"), b"");
        assert!(status == 400 || status == 403, "{name}: {status} {body}");
    }
    let r = d.path();
    assert!(r.join("Games/GBA/Alpha.gba").exists());
    assert!(r.join("Saves/GBA/Alpha.sav").exists());
    assert!(r.join("Labels/GBA/Alpha.png").exists());
    assert!(r.join("Guides/GBA/Alpha.txt").exists());
    let entries: Vec<_> = std::fs::read_dir(r.join("Games/GBA")).unwrap().collect();
    assert_eq!(entries.len(), 2);
    server.stop();
}

#[test]
fn rename_onto_an_existing_game_conflicts() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (status, _) = call(p, "POST", "/api/games/GBA/0/rename?name=Beta.gba", b"");
    assert_eq!(status, 409);
    assert!(d.path().join("Games/GBA/Alpha.gba").exists());
    assert!(d.path().join("Saves/GBA/Alpha.sav").exists());
    server.stop();
}

#[test]
fn delete_without_purge_keeps_save_label_and_guide() {
    let (d, server) = start();
    library(d.path());
    let (status, _) = call(server.port(), "DELETE", "/api/games/GBA/0", b"");
    assert_eq!(status, 200);
    let r = d.path();
    assert!(!r.join("Games/GBA/Alpha.gba").exists());
    assert!(r.join("Saves/GBA/Alpha.sav").exists());
    assert!(r.join("Labels/GBA/Alpha.png").exists());
    assert!(r.join("Guides/GBA/Alpha.txt").exists());
    server.stop();
}

#[test]
fn purge_removes_save_label_and_guide() {
    let (d, server) = start();
    library(d.path());
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Alpha").write_resume(b"r").unwrap();
    let (status, _) = call(server.port(), "DELETE", "/api/games/GBA/0?purge=true&rom=Alpha.gba", b"");
    assert_eq!(status, 200);
    let r = d.path();
    assert!(!r.join("Games/GBA/Alpha.gba").exists());
    assert!(!r.join("Saves/GBA/Alpha.sav").exists());
    assert!(!r.join("Labels/GBA/Alpha.png").exists());
    assert!(!r.join("Guides/GBA/Alpha.txt").exists());
    assert!(!r.join("States/GBA/mgba/Alpha").exists());
    assert!(r.join("Games/GBA/Beta.gba").exists());
    server.stop();
}

#[test]
fn save_upload_names_cores_with_a_resume_state() {
    let (d, server) = start();
    library(d.path());
    StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Alpha").write_resume(b"resume").unwrap();
    let (status, body) = call(server.port(), "POST", "/api/games/GBA/0/saves?name=whatever.SAV", b"new-save");
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body), serde_json::json!({ "ok": true, "resumeCores": ["mGBA"] }));
    assert_eq!(std::fs::read(d.path().join("Saves/GBA/Alpha.sav")).unwrap(), b"new-save");
    server.stop();
}

#[test]
fn save_upload_with_no_resume_reports_none() {
    let (d, server) = start();
    library(d.path());
    let (status, body) = call(server.port(), "POST", "/api/games/GBA/1/saves?name=x.srm", b"fresh");
    assert_eq!(status, 200, "{body}");
    assert_eq!(json(&body), serde_json::json!({ "ok": true, "resumeCores": [] }));
    assert_eq!(std::fs::read(d.path().join("Saves/GBA/Beta.srm")).unwrap(), b"fresh");
    server.stop();
}

#[test]
fn resume_state_is_left_alone() {
    let (d, server) = start();
    library(d.path());
    let ring = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Alpha");
    ring.write_resume(b"resume-bytes").unwrap();
    call(server.port(), "POST", "/api/games/GBA/0/saves?name=a.sav", b"new-save");
    assert_eq!(ring.read_resume().unwrap().unwrap(), b"resume-bytes");
    server.stop();
}

#[test]
fn save_upload_with_a_bad_name_writes_nothing() {
    let (d, server) = start();
    library(d.path());
    for name in ["a%20.sav", "noext", "..%2Fa.sav"] {
        let (status, _) = call(server.port(), "POST", &format!("/api/games/GBA/0/saves?name={name}"), b"x");
        assert!(status == 400 || status == 403, "{name}: {status}");
    }
    assert_eq!(std::fs::read(d.path().join("Saves/GBA/Alpha.sav")).unwrap(), b"save-bytes");
    server.stop();
}

#[test]
fn saves_list_download_and_delete() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (_, body) = call(p, "GET", "/api/games/GBA/0/saves", b"");
    let files = json(&body)["files"].clone();
    assert_eq!(files[0]["name"], "Alpha.sav");
    assert_eq!(files[0]["size"], 10);
    let (status, body) = call(p, "GET", "/api/games/GBA/0/saves?file=Alpha.sav", b"");
    assert_eq!((status, body.as_str()), (200, "save-bytes"));
    let (status, _) = call(p, "GET", "/api/games/GBA/1/saves?file=Alpha.sav", b"");
    assert_eq!(status, 404);
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0/saves?file=Alpha.sav", b"");
    assert_eq!(status, 200);
    assert!(!d.path().join("Saves/GBA/Alpha.sav").exists());
    server.stop();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_default()
}

fn references(root: &Path) {
    put(root, "Config/favorites.txt", b"gba/Alpha\ngba/Beta\n");
    put(root, "Config/recents.txt", b"gba/Alpha\ngbc/Alpha\n");
    put(root, "Config/cart_shell.txt", b"Alpha = auto\nBeta = auto\n");
    put(root, "Config/guides.txt", b"GBA/Alpha.txt = 3 4 1\nGBA/Beta.txt = 1 1 1\n");
}

#[test]
fn rename_carries_favorites_recents_shell_and_guide_spot() {
    let (d, server) = start();
    library(d.path());
    references(d.path());
    let (status, body) = call(server.port(), "POST", "/api/games/GBA/0/rename?name=Gamma.gba", b"");
    assert_eq!(status, 200, "{body}");
    let r = d.path();
    assert_eq!(read(r, "Config/favorites.txt"), "gba/Gamma\ngba/Beta\n");
    assert_eq!(read(r, "Config/recents.txt"), "gba/Gamma\ngbc/Alpha\n");
    let shell = read(r, "Config/cart_shell.txt");
    assert!(shell.contains("Gamma = auto") && shell.contains("Beta = auto") && !shell.contains("Alpha"), "{shell}");
    let spots = read(r, "Config/guides.txt");
    assert!(spots.contains("GBA/Gamma.txt = 3 4 1") && spots.contains("GBA/Beta.txt") && !spots.contains("Alpha"), "{spots}");
    server.stop();
}

#[test]
fn purge_removes_favorites_recents_shell_and_guide_spot() {
    let (d, server) = start();
    library(d.path());
    references(d.path());
    let (status, body) = call(server.port(), "DELETE", "/api/games/GBA/0?purge=true&rom=Alpha.gba", b"");
    assert_eq!(status, 200, "{body}");
    let r = d.path();
    assert_eq!(read(r, "Config/favorites.txt"), "gba/Beta\n");
    assert_eq!(read(r, "Config/recents.txt"), "gbc/Alpha\n");
    let shell = read(r, "Config/cart_shell.txt");
    assert!(shell.contains("Beta") && !shell.contains("Alpha"), "{shell}");
    let spots = read(r, "Config/guides.txt");
    assert!(spots.contains("Beta") && !spots.contains("Alpha"), "{spots}");
    server.stop();
}

#[test]
fn delete_without_purge_leaves_references() {
    let (d, server) = start();
    library(d.path());
    references(d.path());
    call(server.port(), "DELETE", "/api/games/GBA/0", b"");
    assert_eq!(read(d.path(), "Config/favorites.txt"), "gba/Alpha\ngba/Beta\n");
    server.stop();
}

#[test]
fn purge_needs_the_rom_name_and_refuses_a_shifted_id() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0?purge=true", b"");
    assert_eq!(status, 400);
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0?purge=true&rom=Beta.gba", b"");
    assert_eq!(status, 409);
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0?rom=Beta.gba", b"");
    assert_eq!(status, 409);
    let r = d.path();
    assert!(r.join("Games/GBA/Alpha.gba").exists());
    assert!(r.join("Saves/GBA/Alpha.sav").exists());
    assert!(r.join("Labels/GBA/Alpha.png").exists());
    server.stop();
}

#[test]
fn ids_are_plain_digits_without_leading_zeros() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    for id in ["00", "01", "%2B1", "1x", "-1", "99"] {
        let (status, _) = call(p, "GET", &format!("/api/games/GBA/{id}"), b"");
        assert_eq!(status, 404, "{id}");
    }
    let (status, _) = call(p, "GET", "/api/games/GBA/0", b"");
    assert_eq!(status, 200);
    server.stop();
}

#[test]
fn save_upload_takes_only_sav_and_srm() {
    let (d, server) = start();
    library(d.path());
    for name in ["a.exe", "a.sh", "a.txt", "a.state"] {
        let (status, _) = call(server.port(), "POST", &format!("/api/games/GBA/0/saves?name={name}"), b"x");
        assert_eq!(status, 400, "{name}");
    }
    assert_eq!(std::fs::read_dir(d.path().join("Saves/GBA")).unwrap().count(), 1);
    server.stop();
}

#[test]
fn guides_list_download_upload_and_delete() {
    let (d, server) = start();
    library(d.path());
    put(d.path(), "Config/guides.txt", b"GBA/Alpha.txt = 3 4 1\n");
    let p = server.port();
    let (_, body) = call(p, "GET", "/api/games/GBA/0/guides", b"");
    assert_eq!(json(&body)["files"][0]["name"], "Alpha.txt");
    let (status, body) = call(p, "GET", "/api/games/GBA/0/guides?file=Alpha.txt", b"");
    assert_eq!((status, body.as_str()), (200, "guide text"));
    let (_, body) = call(p, "GET", "/api/games/GBA/1/guides", b"");
    assert_eq!(json(&body)["files"].as_array().unwrap().len(), 0);
    let (status, _) = call(p, "POST", "/api/games/GBA/1/guides?name=whatever.txt", b"beta guide");
    assert_eq!(status, 200);
    assert_eq!(read(d.path(), "Guides/GBA/Beta.txt"), "beta guide");
    let (status, _) = call(p, "POST", "/api/games/GBA/1/guides?name=x.pdf", b"nope");
    assert_eq!(status, 400);
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0/guides?file=Alpha.txt", b"");
    assert_eq!(status, 200);
    assert!(!d.path().join("Guides/GBA/Alpha.txt").exists());
    assert!(!read(d.path(), "Config/guides.txt").contains("Alpha"));
    server.stop();
}

#[test]
fn art_upload_caches_the_label_for_the_shelf() {
    let (d, server) = start();
    library(d.path());
    let (status, _) = call(server.port(), "POST", "/api/games/GBA/1/art?name=cover.png", b"\x89PNG\r\n\x1a\nrest");
    assert_eq!(status, 200);
    assert!(d.path().join("Cache/Labels/GBA/Beta.rgba").exists());
    server.stop();
}

#[test]
fn art_upload_replaces_the_label_and_delete_removes_it() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let png = b"\x89PNG\r\n\x1a\nrest";
    let (status, _) = call(p, "POST", "/api/games/GBA/1/art?name=cover.jpg", png);
    assert_eq!(status, 200);
    assert_eq!(std::fs::read(d.path().join("Labels/GBA/Beta.png")).unwrap(), png);
    let (status, _) = call(p, "POST", "/api/games/GBA/0/art?name=x.png", b"not a png");
    assert_eq!(status, 400);
    assert_eq!(std::fs::read(d.path().join("Labels/GBA/Alpha.png")).unwrap(), b"png-bytes");
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0/art", b"");
    assert_eq!(status, 200);
    assert!(!d.path().join("Labels/GBA/Alpha.png").exists());
    let (status, _) = call(p, "DELETE", "/api/games/GBA/0/art", b"");
    assert_eq!(status, 404);
    server.stop();
}

#[test]
fn roms_list_and_download() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    let (_, body) = call(p, "GET", "/api/games/GBA/0/roms", b"");
    let file = &json(&body)["files"][0];
    assert_eq!(file["name"], "Alpha.gba");
    assert_eq!(file["size"], 9);
    let (status, body) = call(p, "GET", "/api/games/GBA/0/rom?file=Alpha.gba", b"");
    assert_eq!((status, body.as_str()), (200, "alpha-rom"));
    let (status, _) = call(p, "GET", "/api/games/GBA/0/rom?file=Beta.gba", b"");
    assert_eq!(status, 404);
    server.stop();
}
