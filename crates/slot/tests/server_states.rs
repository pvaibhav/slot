use slot::server::Server;
use slot_store::{Core, Platform, StateRing};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

const AUTH: &str = "c2xvdDoxMjM0";
const A: &str = "2026-10-01_10-00-00";
const B: &str = "2026-10-02_10-00-00";

fn start() -> (tempfile::TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    let server = Server::start(dir.path().to_path_buf(), 0, "1234".to_string()).unwrap();
    (dir, server)
}

fn call(port: u16, method: &str, target: &str) -> (u16, String) {
    let raw = format!(
        "{method} {target} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Length: 0\r\n\r\n"
    );
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
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

fn json(body: &str) -> serde_json::Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("{e}: {body}"))
}

fn library(root: &Path) {
    std::fs::create_dir_all(root.join("Games/GBA")).unwrap();
    std::fs::write(root.join("Games/GBA/Alpha.gba"), b"rom").unwrap();
}

fn ring(root: &Path, core: Core) -> StateRing {
    StateRing::new(root, Platform::Gba, core, "Alpha")
}

#[test]
fn states_group_by_core() {
    let (d, server) = start();
    library(d.path());
    ring(d.path(), Core::Mgba).push(b"one", b"png1", A).unwrap();
    ring(d.path(), Core::Mgba)
        .push(b"three", b"png2", B)
        .unwrap();
    ring(d.path(), Core::Gpsp).push(b"gp", b"png3", A).unwrap();
    let p = server.port();
    let (status, body) = call(p, "GET", "/api/games/GBA/0/states");
    assert_eq!(status, 200, "{body}");
    let v = json(&body);
    let cores = v["cores"].as_array().unwrap();
    assert_eq!(cores.len(), 2);
    assert_eq!(cores[0]["core"], "mGBA");
    assert_eq!(cores[0]["id"], "mgba");
    assert_eq!(cores[0]["resume"], false);
    assert_eq!(cores[0]["states"].as_array().unwrap().len(), 2);
    assert_eq!(cores[0]["states"][0]["stamp"], B);
    assert_eq!(cores[0]["states"][0]["size"], 5);
    assert_eq!(cores[1]["core"], "gpSP");
    let (status, body) = call(p, "GET", &format!("/api/games/GBA/0/states/mgba/{A}"));
    assert_eq!((status, body.as_str()), (200, "one"));
    let (status, body) = call(p, "GET", &format!("/api/games/GBA/0/states/mgba/{A}/thumb"));
    assert_eq!((status, body.as_str()), (200, "png1"));
    assert_eq!(
        call(p, "GET", &format!("/api/games/GBA/0/states/nope/{A}")).0,
        400
    );
    assert_eq!(
        call(p, "GET", "/api/games/GBA/0/states/mgba/2026-10-09_10-00-00").0,
        404
    );
    server.stop();
}

#[test]
fn resume_is_listed() {
    let (d, server) = start();
    library(d.path());
    ring(d.path(), Core::Mgba).write_resume(b"resumed").unwrap();
    let p = server.port();
    let v = json(&call(p, "GET", "/api/games/GBA/0/states").1);
    assert_eq!(v["cores"][0]["resume"], true);
    assert_eq!(v["cores"][0]["states"].as_array().unwrap().len(), 0);
    let (status, body) = call(p, "GET", "/api/games/GBA/0/states/mgba/resume");
    assert_eq!((status, body.as_str()), (200, "resumed"));
    assert_eq!(
        call(p, "GET", "/api/games/GBA/0/states/mgba/resume/thumb").0,
        404
    );
    server.stop();
}

#[test]
fn refused_resume_files_are_not_shown() {
    let (d, server) = start();
    library(d.path());
    let r = ring(d.path(), Core::Mgba);
    r.write_resume(b"x").unwrap();
    r.retire_resume(A).unwrap();
    let p = server.port();
    let v = json(&call(p, "GET", "/api/games/GBA/0/states").1);
    assert_eq!(v["cores"].as_array().unwrap().len(), 0);
    let status = call(
        p,
        "GET",
        &format!("/api/games/GBA/0/states/mgba/resume-refused-{A}"),
    )
    .0;
    assert_eq!(status, 400);
    server.stop();
}

#[test]
fn delete_removes_state_and_thumb() {
    let (d, server) = start();
    library(d.path());
    ring(d.path(), Core::Mgba).push(b"one", b"png1", A).unwrap();
    let p = server.port();
    let (status, body) = call(p, "DELETE", &format!("/api/games/GBA/0/states/mgba/{A}"));
    assert_eq!(status, 200, "{body}");
    let dir = d.path().join("States/GBA/mgba/Alpha");
    assert!(!dir.join(format!("{A}.state")).exists());
    assert!(!dir.join(format!("{A}.png")).exists());
    assert_eq!(
        call(p, "DELETE", &format!("/api/games/GBA/0/states/mgba/{A}")).0,
        404
    );
    server.stop();
}

#[test]
fn delete_resume_removes_it() {
    let (d, server) = start();
    library(d.path());
    ring(d.path(), Core::Mgba).write_resume(b"r").unwrap();
    let p = server.port();
    assert_eq!(
        call(p, "DELETE", "/api/games/GBA/0/states/mgba/resume").0,
        200
    );
    assert!(!ring(d.path(), Core::Mgba).has_resume());
    server.stop();
}

#[test]
fn post_is_refused() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    assert_eq!(call(p, "POST", "/api/games/GBA/0/states").0, 405);
    assert_eq!(
        call(p, "POST", &format!("/api/games/GBA/0/states/mgba/{A}")).0,
        405
    );
    server.stop();
}

#[test]
fn bad_stamp_is_refused() {
    let (d, server) = start();
    library(d.path());
    let p = server.port();
    assert_eq!(call(p, "GET", "/api/games/GBA/0/states/mgba/..").0, 400);
    assert_eq!(call(p, "DELETE", "/api/games/GBA/0/states/mgba/..").0, 400);
    assert_eq!(
        call(p, "GET", "/api/games/GBA/0/states/mgba/x.state").0,
        400
    );
    server.stop();
}

#[cfg(unix)]
#[test]
fn symlinked_stem_dir_is_not_followed() {
    let (d, server) = start();
    library(d.path());
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join(format!("{A}.state")), b"secret").unwrap();
    std::fs::write(outside.path().join("resume.state"), b"secret").unwrap();
    let parent = d.path().join("States/GBA/mgba");
    std::fs::create_dir_all(&parent).unwrap();
    std::os::unix::fs::symlink(outside.path(), parent.join("Alpha")).unwrap();
    let p = server.port();
    let v = json(&call(p, "GET", "/api/games/GBA/0/states").1);
    assert_eq!(v["cores"].as_array().unwrap().len(), 0);
    assert_eq!(
        call(p, "GET", &format!("/api/games/GBA/0/states/mgba/{A}")).0,
        403
    );
    assert_eq!(
        call(p, "DELETE", &format!("/api/games/GBA/0/states/mgba/{A}")).0,
        403
    );
    assert_eq!(
        call(p, "DELETE", "/api/games/GBA/0/states/mgba/resume").0,
        403
    );
    assert!(outside.path().join(format!("{A}.state")).exists());
    assert!(outside.path().join("resume.state").exists());
    server.stop();
}

fn lists_for(platform: Platform, tag: &str, file: &str) -> Vec<String> {
    let (d, server) = start();
    std::fs::create_dir_all(d.path().join("Games").join(tag)).unwrap();
    std::fs::write(d.path().join("Games").join(tag).join(file), b"rom").unwrap();
    for core in Core::ALL {
        StateRing::new(d.path(), platform, core, "Alpha")
            .push(b"s", b"png", A)
            .unwrap();
    }
    let (status, body) = call(server.port(), "GET", &format!("/api/games/{tag}/0/states"));
    assert_eq!(status, 200, "{body}");
    let ids = json(&body)["cores"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect();
    server.stop();
    ids
}

#[test]
fn a_gb_game_lists_mgba_and_gambatte_only() {
    assert_eq!(
        lists_for(Platform::Gb, "GB", "Alpha.gb"),
        ["mgba", "gambatte"]
    );
}

#[test]
fn a_gba_game_lists_mgba_and_gpsp_only() {
    assert_eq!(
        lists_for(Platform::Gba, "GBA", "Alpha.gba"),
        ["mgba", "gpsp"]
    );
}
