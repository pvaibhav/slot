mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::romm::{Gifts, RommEvent, RommJob, RommJobs, GIFTS};
use slot::server::Server;
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, Platform, SlotState};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::{tempdir, TempDir};

const AUTH: &str = "c2xvdDoxMjM0";

fn booted_with(gifts: &str) -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    if !gifts.is_empty() {
        std::fs::create_dir_all(d.path().join("Config")).unwrap();
        std::fs::write(d.path().join(GIFTS), gifts).unwrap();
    }
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    common::connect(&mut a);
    (d, a)
}

fn settle(a: &mut App) {
    for _ in 0..4 {
        a.update(1.0 / 60.0);
    }
}

#[test]
fn survive_a_reload() {
    let d = tempdir().unwrap();
    let mut gifts = Gifts::load(d.path());
    gifts.add(d.path(), Platform::Gba, "Emerald");
    gifts.add(d.path(), Platform::Gb, "Tetris");
    let again = Gifts::load(d.path());
    assert!(again.holds(Platform::Gba, "Emerald"));
    assert!(again.holds(Platform::Gb, "Tetris"));
    assert_eq!(again, gifts);
}

#[test]
fn remove_clears_the_line() {
    let d = tempdir().unwrap();
    let mut gifts = Gifts::load(d.path());
    gifts.add(d.path(), Platform::Gba, "Emerald");
    gifts.add(d.path(), Platform::Gba, "Fusion");
    gifts.remove(d.path(), Platform::Gba, "Emerald");
    let text = std::fs::read_to_string(d.path().join(GIFTS)).unwrap();
    assert_eq!(text, "GBA/Fusion\n");
    assert!(!Gifts::load(d.path()).holds(Platform::Gba, "Emerald"));
}

#[test]
fn rename_follows_the_game() {
    let d = tempdir().unwrap();
    let mut gifts = Gifts::load(d.path());
    gifts.add(d.path(), Platform::Gba, "Emerald");
    gifts.rename(d.path(), Platform::Gba, "Emerald", "Ruby");
    let again = Gifts::load(d.path());
    assert!(again.holds(Platform::Gba, "Ruby"));
    assert!(!again.holds(Platform::Gba, "Emerald"));
}

#[test]
fn same_stem_on_two_platforms_stays_apart() {
    let d = tempdir().unwrap();
    let mut gifts = Gifts::load(d.path());
    gifts.add(d.path(), Platform::Gb, "Tetris");
    assert!(!gifts.holds(Platform::Gbc, "Tetris"));
    gifts.add(d.path(), Platform::Gbc, "Tetris");
    gifts.remove(d.path(), Platform::Gb, "Tetris");
    let again = Gifts::load(d.path());
    assert!(again.holds(Platform::Gbc, "Tetris"));
    assert!(!again.holds(Platform::Gb, "Tetris"));
}

#[test]
fn a_on_a_wrapped_cart_unwraps_and_does_not_insert() {
    let (d, mut a) = booted_with("GBA/Emerald\n");
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaUp(Btn::A));
    assert!(matches!(a.phase(), Phase::Unwrap { .. }), "{:?}", a.phase());
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert!(!Gifts::load(d.path()).holds(Platform::Gba, "Emerald"));
    assert!(!d.path().join(GIFTS).exists());
}

#[test]
fn insert_action_on_a_wrapped_cart_unwraps_too() {
    let (_d, mut a) = booted_with("GBA/Emerald\n");
    a.apply(Action::Insert);
    assert!(matches!(a.phase(), Phase::Unwrap { .. }), "{:?}", a.phase());
}

#[test]
fn a_on_an_opened_cart_inserts() {
    let (_d, mut a) = booted_with("GBA/Fusion\n");
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaUp(Btn::A));
    assert!(matches!(a.phase(), Phase::Inserting { .. }), "{:?}", a.phase());
}

#[test]
fn hold_a_on_a_wrapped_cart_unwraps() {
    let (d, mut a) = booted_with("GBA/Emerald\n");
    a.apply(Action::GbaDown(Btn::A));
    let now = a.now();
    a.tick_ms(now + 2000);
    settle(&mut a);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert!(!Gifts::load(d.path()).holds(Platform::Gba, "Emerald"));
}

#[test]
fn an_app_side_add_keeps_a_gamebit_side_rename() {
    let d = tempdir().unwrap();
    let mut app_side = Gifts::default();
    app_side.add(d.path(), Platform::Gba, "Alpha");
    let mut gamebit_side = Gifts::load(d.path());
    gamebit_side.rename(d.path(), Platform::Gba, "Alpha", "Omega");
    app_side.add(d.path(), Platform::Gba, "Fusion");
    let disk = Gifts::load(d.path());
    assert!(disk.holds(Platform::Gba, "Omega"));
    assert!(disk.holds(Platform::Gba, "Fusion"));
    assert!(!disk.holds(Platform::Gba, "Alpha"));
    assert_eq!(app_side, disk);
}

#[test]
fn an_app_side_remove_does_not_bring_back_a_gamebit_deleted_gift() {
    let d = tempdir().unwrap();
    let mut app_side = Gifts::default();
    app_side.add(d.path(), Platform::Gba, "Alpha");
    app_side.add(d.path(), Platform::Gba, "Beta");
    Gifts::load(d.path()).remove(d.path(), Platform::Gba, "Alpha");
    app_side.remove(d.path(), Platform::Gba, "Beta");
    assert!(!d.path().join(GIFTS).exists());
}

#[derive(Clone, Default)]
struct Script {
    events: Arc<Mutex<Vec<RommEvent>>>,
}

impl RommJobs for Script {
    fn ask(&mut self, _: RommJob) {}

    fn take(&mut self) -> Option<RommEvent> {
        let mut events = self.events.lock().unwrap();
        match events.is_empty() {
            true => None,
            false => Some(events.remove(0)),
        }
    }
}

#[test]
fn an_installed_game_becomes_a_gift_and_a_gift_with_no_game_is_dropped_on_rescan() {
    let (d, mut a) = booted_with("GBA/Gone\n");
    let script = Script::default();
    a.set_romm_jobs(Box::new(script.clone()));
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    script.events.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    let now = a.now();
    a.tick_ms(now + 1);
    let gifts = Gifts::load(d.path());
    assert!(gifts.holds(Platform::Gba, "Sent"));
    assert!(!gifts.holds(Platform::Gba, "Gone"));
}

#[test]
fn a_rescan_asks_for_the_boxes_again() {
    let (d, mut a) = booted_with("GBA/Emerald\n");
    let script = Script::default();
    a.set_romm_jobs(Box::new(script.clone()));
    let wants = a.gift_wants();
    assert_eq!(wants.len(), 1, "{wants:?}");
    a.set_gift_box(wants[0].0, slot_gfx::TexId::from_raw(9));
    assert!(a.gift_wants().is_empty());
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    script.events.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    let now = a.now();
    a.tick_ms(now + 1);
    let stems: Vec<String> = a.gift_wants().into_iter().map(|(_, (_, stem))| stem).collect();
    assert_eq!(stems, ["Emerald", "Sent"]);
}

fn request(port: u16, head: &str) -> u16 {
    let raw = format!("{head} HTTP/1.1\r\nHost: x\r\nAuthorization: Basic {AUTH}\r\nContent-Length: 0\r\n\r\n");
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(raw.as_bytes()).unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
}

fn gamebit(gifts: &str) -> (TempDir, Server) {
    let dir = tempdir().unwrap();
    let games = dir.path().join("Games/GBA");
    std::fs::create_dir_all(&games).unwrap();
    std::fs::write(games.join("Alpha.gba"), b"rom").unwrap();
    if !gifts.is_empty() {
        std::fs::create_dir_all(dir.path().join("Config")).unwrap();
        std::fs::write(dir.path().join(GIFTS), gifts).unwrap();
    }
    let server = Server::start(dir.path().to_path_buf(), 0, "1234".to_string()).unwrap();
    (dir, server)
}

fn read(root: &Path) -> String {
    std::fs::read_to_string(root.join(GIFTS)).unwrap_or_default()
}

#[test]
fn gamebit_delete_removes_the_gift() {
    let (dir, server) = gamebit("GBA/Alpha\nGBA/Other\n");
    assert_eq!(request(server.port(), "DELETE /api/games/GBA/0"), 200);
    assert_eq!(read(dir.path()), "GBA/Other\n");
    server.stop();
}

#[test]
fn gamebit_rename_moves_the_gift() {
    let (dir, server) = gamebit("GBA/Alpha\n");
    assert_eq!(request(server.port(), "POST /api/games/GBA/0/rename?name=Omega.gba"), 200);
    assert_eq!(read(dir.path()), "GBA/Omega\n");
    server.stop();
}

#[test]
fn gamebit_rename_leaves_an_absent_gifts_file_absent() {
    let (dir, server) = gamebit("");
    assert_eq!(request(server.port(), "POST /api/games/GBA/0/rename?name=Omega.gba"), 200);
    assert!(!dir.path().join(GIFTS).exists());
    server.stop();
}
