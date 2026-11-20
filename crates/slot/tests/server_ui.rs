mod common;

use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::time::{Duration, Instant};

use common::{app_booting_at, app_playing_in, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot::audio::{ring_capacity, Ring, Sfx};
use slot::server::{Responder, ServerNet, ServerPaper, NO_WIFI, SERVER_FAILED};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, Platform, Recent, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

fn lan() -> Option<Ipv4Addr> {
    Some(Ipv4Addr::LOCALHOST)
}

fn no_lan() -> Option<Ipv4Addr> {
    None
}

fn no_mdns(_: Ipv4Addr) -> Option<Responder> {
    None
}

fn net(ports: &[u16]) -> ServerNet {
    ServerNet {
        ip: lan,
        ports: ports.to_vec(),
        responder: no_mdns,
    }
}

fn free_port() -> u16 {
    TcpListener::bind(("0.0.0.0", 0))
        .and_then(|l| l.local_addr())
        .unwrap()
        .port()
}

fn answers(port: u16) -> bool {
    TcpStream::connect(("127.0.0.1", port)).is_ok()
}

fn on_shelf() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    common::connect(&mut a);
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open(a: &mut App) {
    a.apply(Action::QuickMenu);
    for _ in 0..QuickRow::Network.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    press(a, Btn::A);
    for _ in 0..QuickRow::Server.position(&a.hidden_rows()) {
        press(a, Btn::Down);
    }
    assert!(matches!(
        a.phase(),
        Phase::QuickMenu {
            row: QuickRow::Server
        }
    ));
    press(a, Btn::A);
    assert!(a.server_open());
}

fn until(a: &mut App, mut done: impl FnMut(&mut App) -> bool) {
    let by = Instant::now() + Duration::from_secs(10);
    while !done(a) {
        assert!(Instant::now() < by, "timed out");
        let t = a.now();
        a.tick_ms(t + 16);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn paper(a: &mut App) -> ServerPaper {
    until(a, |a| a.server_paper().is_some());
    a.server_paper().unwrap().clone()
}

fn code_url(p: &ServerPaper) -> String {
    match p {
        ServerPaper::Code { url, .. } => url.clone(),
        ServerPaper::Plain(text) => panic!("plain paper: {text}"),
    }
}

#[test]
fn server_row_is_listed() {
    let (_d, a) = on_shelf();
    assert!(QuickRow::NETWORK.contains(&QuickRow::Server));
    assert!(!a.hidden_rows().contains(&QuickRow::Server));
}

#[test]
fn server_row_is_hidden_in_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let a = app_playing_in(d.path(), "Emerald");
    assert!(a.hidden_rows().contains(&QuickRow::Server));
    for page in [
        &QuickRow::PLAY[..],
        &QuickRow::SETTINGS,
        &QuickRow::PLAY_SCREEN,
    ] {
        assert!(!page.contains(&QuickRow::Server));
    }
}

#[test]
fn opening_starts_the_server() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    let p = paper(&mut a);
    assert!(code_url(&p).starts_with(&format!("http://127.0.0.1:{port}/?pin=")));
    assert!(answers(port));
}

#[test]
fn nothing_listens_before_the_row_is_opened() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    a.apply(Action::QuickMenu);
    assert!(!a.server_open());
    assert!(!answers(port));
}

#[test]
fn b_stops_the_server() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    paper(&mut a);
    assert!(answers(port));
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert!(!answers(port));
    assert!(matches!(
        a.phase(),
        Phase::QuickMenu {
            row: QuickRow::Server
        }
    ));
}

#[test]
fn stop_rescans_the_shelf() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    std::fs::write(d.path().join("Games/GBA/Added.gba"), vec![0u8; 512]).unwrap();
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert!(a.carts().any(|c| c.stem == "Added"));
    assert!(a.take_rescanned());
}

#[test]
fn stop_drops_a_removed_game_and_keeps_the_selection() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    a.apply(Action::ShelfRight);
    let picked = a.selected_stem().map(str::to_string);
    open(&mut a);
    paper(&mut a);
    let gone = if picked.as_deref() == Some("Emerald") {
        "Fusion"
    } else {
        "Emerald"
    };
    std::fs::remove_file(d.path().join(format!("Games/GBA/{gone}.gba"))).unwrap();
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert!(a.carts().all(|c| c.stem != gone));
    assert_eq!(a.selected_stem().map(str::to_string), picked);
}

#[test]
fn close_reloads_what_the_server_edited() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    let mut disk = slot_store::read_slot_state(d.path());
    disk.favorites = vec![Recent {
        platform: Platform::Gba,
        stem: "Fusion".into(),
    }];
    write_slot_state(d.path(), &disk).unwrap();
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    a.flush_state();
    let after = slot_store::read_slot_state(d.path());
    assert_eq!(after.favorites.len(), 1);
    assert!(a.is_favorite(Platform::Gba, "Fusion"));
}

#[test]
fn close_reloads_the_collections_the_server_edited() {
    let (d, mut a) = on_shelf();
    let cols = d.path().join("Collections");
    std::fs::create_dir_all(&cols).unwrap();
    std::fs::write(cols.join("Puzzlers.txt"), "gba/Emerald\n").unwrap();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    std::fs::write(cols.join("Puzzlers.txt"), "gba/Fusion\n").unwrap();
    std::fs::write(cols.join("Arcade.txt"), "gba/Emerald\n").unwrap();
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert_eq!(a.collection_names(), ["Arcade", "Puzzlers"]);
}

#[test]
fn nothing_overwrites_the_servers_edits_during_a_session() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    let mut disk = slot_store::read_slot_state(d.path());
    disk.favorites = vec![Recent {
        platform: Platform::Gba,
        stem: "Fusion".into(),
    }];
    write_slot_state(d.path(), &disk).unwrap();
    a.set_wifi_off(true);
    a.flush_state();
    assert_eq!(slot_store::read_slot_state(d.path()).favorites.len(), 1);
}

#[test]
fn screen_blocks_game_launch() {
    let (_d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    for action in [
        Action::Insert,
        Action::Eject,
        Action::QuickMenu,
        Action::GbaDown(Btn::A),
        Action::GbaDown(Btn::Start),
        Action::SelectTap,
    ] {
        a.apply(action);
        assert!(a.server_open());
        assert!(matches!(
            a.phase(),
            Phase::QuickMenu {
                row: QuickRow::Server
            }
        ));
    }
}

#[test]
fn no_wifi_prints_a_plain_message() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(ServerNet {
        ip: no_lan,
        ..net(&[port])
    });
    open(&mut a);
    assert_eq!(paper(&mut a), ServerPaper::Plain(NO_WIFI));
    assert_eq!(NO_WIFI, "NO WI-FI");
    assert!(!answers(port));
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
}

#[test]
fn falls_back_to_2003_when_80_is_taken() {
    let (_d, mut a) = on_shelf();
    let held = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    let first = held.local_addr().unwrap().port();
    a.set_server_net(net(&[first, 2003]));
    open(&mut a);
    let p = paper(&mut a);
    assert!(code_url(&p).starts_with("http://127.0.0.1:2003/?pin="));
    let ServerPaper::Code { ip, .. } = &p else {
        panic!("plain paper");
    };
    assert_eq!(ip, "127.0.0.1:2003");
    assert!(answers(2003));
}

#[test]
fn both_ports_taken_prints_a_plain_message() {
    let (_d, mut a) = on_shelf();
    let one = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    let two = TcpListener::bind(("0.0.0.0", 0)).unwrap();
    a.set_server_net(net(&[
        one.local_addr().unwrap().port(),
        two.local_addr().unwrap().port(),
    ]));
    open(&mut a);
    assert_eq!(paper(&mut a), ServerPaper::Plain(SERVER_FAILED));
    assert_eq!(SERVER_FAILED, "GAMEBIT FAILED");
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
}

#[test]
fn responder_failure_omits_host() {
    let (_d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    let ServerPaper::Code { host, .. } = paper(&mut a) else {
        panic!("plain paper");
    };
    assert_eq!(host, None);
}

#[test]
fn a_started_responder_prints_the_host() {
    let Some(probe) = Responder::start(Ipv4Addr::LOCALHOST) else {
        return;
    };
    probe.stop();
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(ServerNet {
        responder: Responder::start,
        ..net(&[port])
    });
    open(&mut a);
    let ServerPaper::Code { host, .. } = paper(&mut a) else {
        panic!("plain paper");
    };
    assert!(host.is_some_and(|h| h.ends_with(&format!(".LOCAL:{port}"))));
}

#[test]
fn a_failed_drawing_stops_and_says_so_and_b_returns() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    paper(&mut a);
    a.server_art_failed();
    until(&mut a, |a| !a.server_open());
    assert!(!answers(port));
    assert!(a.update_shown());
    let (title, _) = a.update_text().unwrap();
    assert_eq!(title, "Gamebit failed");
    press(&mut a, Btn::B);
    let t = a.now();
    a.tick_ms(t + 2000);
    assert!(!a.update_shown());
}

#[test]
fn closing_the_lid_stops_the_server() {
    let (_d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    paper(&mut a);
    a.apply(Action::LidClose);
    until(&mut a, |a| !a.server_open());
    assert!(!answers(port));
}

fn power_goes_down_after_the_server(mut a: App, d: TempDir, port: u16) {
    let mut disk = slot_store::read_slot_state(d.path());
    disk.favorites = vec![Recent {
        platform: Platform::Gba,
        stem: "Fusion".into(),
    }];
    write_slot_state(d.path(), &disk).unwrap();
    until(&mut a, |a| a.ready_to_power_off());
    assert!(!a.server_open());
    assert!(!answers(port));
    a.flush_state();
    assert_eq!(slot_store::read_slot_state(d.path()).favorites.len(), 1);
    assert!(a.is_favorite(Platform::Gba, "Fusion"));
}

#[test]
fn holding_power_stops_the_server_before_powering_off() {
    let (d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    paper(&mut a);
    a.apply(Action::PowerHold);
    power_goes_down_after_the_server(a, d, port);
}

#[test]
fn powering_off_in_a_session_remembers_the_shelf() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    a.apply(Action::ShelfRight);
    let picked = a.selected_stem().map(str::to_string);
    open(&mut a);
    paper(&mut a);
    a.apply(Action::PowerHold);
    until(&mut a, |a| a.ready_to_power_off());
    a.flush_state();
    let cart = slot_store::read_slot_state(d.path()).shelf_cart;
    assert_eq!(cart.map(|c| c.stem), picked);
}

#[test]
fn a_critical_battery_stops_the_server_before_powering_off() {
    let (d, mut a) = on_shelf();
    let port = free_port();
    a.set_server_net(net(&[port]));
    open(&mut a);
    paper(&mut a);
    a.on_battery(slot_power::Battery {
        percent: 1,
        charge: slot_power::Charge::Discharging,
    });
    power_goes_down_after_the_server(a, d, port);
}

#[test]
fn a_plain_session_keeps_a_brightness_change() {
    let (d, mut a) = on_shelf();
    a.set_server_net(ServerNet {
        ip: no_lan,
        ..net(&[free_port()])
    });
    let before = slot_store::read_slot_state(d.path()).brightness;
    open(&mut a);
    paper(&mut a);
    a.apply(Action::BrightnessDown);
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    a.flush_state();
    assert_ne!(slot_store::read_slot_state(d.path()).brightness, before);
}

#[test]
fn the_screen_takes_only_volume_mute_and_brightness() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    a.apply(Action::BlueLightUp);
    a.apply(Action::ShaderNext);
    a.apply(Action::SunlightUp);
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    a.flush_state();
    let after = slot_store::read_slot_state(d.path());
    let base = SlotState::default();
    assert_eq!(after.blue_light, base.blue_light);
}

fn solid_png(path: &std::path::Path, w: u32, h: u32, rgb: [u8; 3]) {
    let file = std::fs::File::create(path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .unwrap()
        .write_image_data(&rgb.repeat((w * h) as usize))
        .unwrap();
}

#[test]
fn stop_hands_new_and_replaced_labels_to_the_rebuild() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    open(&mut a);
    paper(&mut a);
    let dir = d.path().join("Labels/GBA");
    std::fs::create_dir_all(&dir).unwrap();
    solid_png(&dir.join("Emerald.png"), 64, 32, [200, 20, 20]);
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    let cart = a.carts().find(|c| c.stem == "Emerald").unwrap();
    assert!(cart.label.is_some());
    assert!(!slot::label_cache::fresh(d.path(), cart));
    let again = cart.clone();
    let stale = a.take_stale_labels();
    assert_eq!(
        stale.iter().map(|c| c.stem.as_str()).collect::<Vec<_>>(),
        ["Emerald"]
    );
    let _ = slot::label_cache::label_art(d.path(), &stale[0]);
    let stamp = std::time::SystemTime::now() + Duration::from_secs(5);
    open_from_menu(&mut a);
    paper(&mut a);
    solid_png(&dir.join("Emerald.png"), 64, 32, [20, 20, 200]);
    std::fs::File::options()
        .write(true)
        .open(dir.join("Emerald.png"))
        .unwrap()
        .set_modified(stamp)
        .unwrap();
    assert!(!slot::label_cache::fresh(d.path(), &again));
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert_eq!(a.take_stale_labels().len(), 1);
}

#[test]
fn stop_leaves_labels_that_were_stale_before_it_opened() {
    let (d, mut a) = on_shelf();
    a.set_server_net(net(&[free_port()]));
    let dir = d.path().join("Labels/GBA");
    std::fs::create_dir_all(&dir).unwrap();
    solid_png(&dir.join("Emerald.png"), 64, 32, [200, 20, 20]);
    open(&mut a);
    paper(&mut a);
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    let cart = a.carts().find(|c| c.stem == "Emerald").unwrap().clone();
    let _ = a.take_stale_labels();
    let _ = slot::label_cache::label_art(d.path(), &cart);
    let later = std::time::SystemTime::now() + Duration::from_secs(5);
    std::fs::File::options()
        .write(true)
        .open(dir.join("Emerald.png"))
        .unwrap()
        .set_modified(later)
        .unwrap();
    assert!(!slot::label_cache::fresh(d.path(), &cart));
    open_from_menu(&mut a);
    paper(&mut a);
    press(&mut a, Btn::B);
    until(&mut a, |a| !a.server_open());
    assert_eq!(a.take_stale_labels(), Vec::new());
}

fn open_from_menu(a: &mut App) {
    if !matches!(
        a.phase(),
        Phase::QuickMenu {
            row: QuickRow::Server
        }
    ) {
        panic!("not on the Server row");
    }
    press(a, Btn::A);
    assert!(a.server_open());
}

#[test]
fn a_second_clip_does_not_cut_the_first() {
    let r = Ring::new(ring_capacity(48_000));
    r.reopen(48_000);
    let mut out = vec![0i16; 1024];
    while r.queued_frames() > 0 {
        r.fill(&mut out);
    }
    let long = 100_000;
    r.mix(&vec![1000i16; long]);
    r.fill(&mut out);
    r.mix(&vec![500i16; 2048]);
    let mut seen = Vec::new();
    while r.queued_frames() > 0 {
        r.fill(&mut out);
        seen.extend_from_slice(&out);
    }
    assert_eq!(seen.len() + 1024, 100_000usize.div_ceil(1024) * 1024);
    assert!(seen[..1024].iter().all(|s| *s == 1500));
    let end = long - 1024;
    assert!(seen[end - 200..end].iter().all(|s| *s == 1000));
}

#[test]
fn the_sounds_play_at_their_levels() {
    fn peak(pcm: &[u8]) -> f32 {
        pcm.chunks_exact(2)
            .map(|b| (i16::from_le_bytes([b[0], b[1]]) as f32).abs())
            .fold(0.0, f32::max)
    }
    let rendered = |s: Sfx| {
        s.render(48_000)
            .iter()
            .map(|v| (*v as f32).abs())
            .fold(0.0, f32::max)
    };
    let buzz = peak(include_bytes!("../../../assets/server/buzz.pcm"));
    let tear = peak(include_bytes!("../../../assets/server/tear.pcm"));
    assert!((rendered(Sfx::PrintBuzz) - buzz * 0.40).abs() < 2.0);
    assert!((rendered(Sfx::Tear) - tear * 0.45).abs() < 2.0);
}

#[test]
fn hush_silences_a_clip_longer_than_the_ring() {
    let r = Ring::new(ring_capacity(48_000));
    r.reopen(48_000);
    let mut out = vec![0i16; 1024];
    while r.queued_frames() > 0 {
        r.fill(&mut out);
    }
    r.mix(&vec![1000i16; 200_000]);
    r.fill(&mut out);
    assert!(out.iter().any(|s| *s != 0));
    r.hush();
    for _ in 0..200 {
        r.fill(&mut out);
        assert!(out.iter().all(|s| *s == 0));
    }
}

#[cfg(target_os = "macos")]
mod screen {
    use super::*;
    use common::clocked;
    use slot::audio::{AudioSink, StubSink};
    use slot::frontend::Frontend;
    use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
    use slot_input::{InputSource, Millis, RawEvent};
    use slot_power::SimPlatform;
    use std::collections::VecDeque;

    const PRINT_AT: Millis = 260 + 640 + 260;
    const TEAR_AT: Millis = PRINT_AT + 3200 + 350 + 710;
    const REST: Millis = 260 + 640 + 260 + 3200 + 350 + 960 + 340 + 140 + 560;

    struct Script(VecDeque<Vec<RawEvent>>);

    impl InputSource for Script {
        fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
            self.0.pop_front().unwrap_or_default()
        }
    }

    struct Rig {
        c: Compositor,
        f: Frontend,
        input: Script,
        now: Millis,
        d: TempDir,
        window: (u32, u32),
        menu: Vec<u8>,
        _surface: HeadlessSurface,
    }

    impl Rig {
        fn new(net: ServerNet) -> Option<Rig> {
            let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
            clocked(d.path());
            let platform = Box::new(SimPlatform::at(d.path().to_path_buf()));
            Rig::on(
                net,
                d,
                Frontend::boot_with(platform, Box::new(StubSink::draining())),
                (OUT_W, OUT_H),
            )
        }

        fn sized(net: ServerNet, window: (u32, u32)) -> Option<Rig> {
            let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
            clocked(d.path());
            let platform = Box::new(SimPlatform::at(d.path().to_path_buf()));
            Rig::on(
                net,
                d,
                Frontend::boot_with(platform, Box::new(StubSink::draining())),
                window,
            )
        }

        fn on(net: ServerNet, d: TempDir, mut f: Frontend, window: (u32, u32)) -> Option<Rig> {
            let surface = HeadlessSurface::new().ok()?;
            let mut c = Compositor::new(&surface).ok()?;
            c.fit(window);
            f.upload_faces(&mut c);
            f.app_mut().set_server_net(net);
            common::connect(f.app_mut());
            let mut r = Rig {
                c,
                f,
                input: Script(VecDeque::new()),
                now: 1000,
                d,
                window,
                menu: Vec::new(),
                _surface: surface,
            };
            r.tap(Btn::Menu);
            for _ in 0..QuickRow::Network.position(&[]) {
                r.tap(Btn::Down);
            }
            r.tap(Btn::A);
            for _ in 0..QuickRow::Server.position(&[]) {
                r.tap(Btn::Down);
            }
            r.menu = r.shot("menu");
            r.tap(Btn::A);
            Some(r)
        }

        fn step(&mut self, ms: Millis) {
            self.now += ms;
            self.f
                .advance_at(&mut self.input, self.now, ms as f32 / 1000.0);
            self.f.render(&mut self.c, self.window);
        }

        fn tap(&mut self, btn: Btn) {
            self.input.0.push_back(vec![RawEvent::Down(btn)]);
            self.step(16);
            self.input.0.push_back(vec![RawEvent::Up(btn)]);
            self.step(16);
        }

        fn run(&mut self, ms: Millis) {
            let end = self.now + ms;
            while self.now < end {
                self.step(16);
            }
        }

        fn wait_scene(&mut self) {
            let by = Instant::now() + Duration::from_secs(20);
            while !self.f.app().server_paper_ready() {
                assert!(Instant::now() < by, "the scene never started");
                self.step(16);
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn wait_closed(&mut self) {
            let by = Instant::now() + Duration::from_secs(20);
            while self.f.app().server_open() {
                assert!(Instant::now() < by, "the screen never closed");
                self.step(16);
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        fn shot(&mut self, name: &str) -> Vec<u8> {
            self.f.render(&mut self.c, self.window);
            let px = self.c.read_frame();
            if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
                let path = format!("{dir}/server-{name}.png");
                let file = std::fs::File::create(&path).expect("create png");
                let (w, h) = self.window;
                let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header()
                    .expect("png header")
                    .write_image_data(&px)
                    .expect("png data");
            }
            px
        }
    }

    fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
        let o = (y * OUT_W as usize + x) * 4;
        [px[o], px[o + 1], px[o + 2]]
    }

    fn paper_lit(px: &[u8]) -> usize {
        (60..440)
            .flat_map(|y| (450..660).map(move |x| (x, y)))
            .filter(|&(x, y)| at(px, x, y).iter().all(|c| *c > 200))
            .count()
    }

    #[test]
    fn a_skips_the_print_and_leaves_the_paper_at_rest() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        r.run(1500);
        let printing = r.shot("printing");
        r.tap(Btn::A);
        r.run(32);
        let skipped = r.shot("skipped");
        r.run(REST);
        let rest = r.shot("rested");
        assert!(paper_lit(&skipped) > paper_lit(&printing));
        assert_eq!(skipped, rest);
        assert!(r.f.app().server_open());
    }

    #[test]
    fn the_paper_rests_on_screen_and_b_brings_the_menu_back() {
        let port = free_port();
        let Some(mut r) = Rig::new(net(&[port])) else {
            return;
        };
        r.wait_scene();
        r.run(130);
        let fading = r.shot("fade");
        let split = |a: u8, b: u8| a.abs_diff(b) > 4;
        let changed = fading
            .iter()
            .zip(&r.menu)
            .filter(|(a, b)| split(**a, **b))
            .count();
        assert!(changed > 0);
        r.run(800);
        let risen = r.shot("rise");
        assert_ne!(fading, risen);
        r.run(REST);
        let rest = r.shot("rest");
        assert!(paper_lit(&rest) > 20_000, "{}", paper_lit(&rest));
        assert!(answers(port));
        r.tap(Btn::B);
        r.wait_closed();
        r.step(16);
        assert!(!answers(port));
        let back = r.shot("back");
        assert!(paper_lit(&back) < 2_000, "{}", paper_lit(&back));
        assert!(matches!(
            r.f.app().phase(),
            Phase::QuickMenu {
                row: QuickRow::Server
            }
        ));
        let off = back
            .iter()
            .zip(&r.menu)
            .filter(|(a, b)| split(**a, **b))
            .count();
        assert!(off < 2_000, "the menu came back different: {off}");
    }

    #[test]
    fn b_during_the_print_still_returns_only_when_the_scene_is_done() {
        let port = free_port();
        let Some(mut r) = Rig::new(net(&[port])) else {
            return;
        };
        r.wait_scene();
        r.run(2000);
        r.tap(Btn::B);
        r.run(200);
        assert!(r.f.app().server_open());
        r.wait_closed();
    }

    #[test]
    fn the_failure_papers_print() {
        let taken = TcpListener::bind(("0.0.0.0", 0)).unwrap();
        let taken_port = taken.local_addr().unwrap().port();
        let no_wifi = ServerNet {
            ip: no_lan,
            ..net(&[free_port()])
        };
        for (name, net) in [("no-wifi", no_wifi), ("failed", net(&[taken_port]))] {
            let Some(mut r) = Rig::new(net) else {
                return;
            };
            r.wait_scene();
            r.run(REST);
            let px = r.shot(name);
            assert!(paper_lit(&px) > 20_000);
            r.tap(Btn::B);
            r.wait_closed();
        }
    }

    #[test]
    fn the_resting_screen_fits_the_640_panel() {
        let Some(mut r) = Rig::sized(net(&[free_port()]), (640, 480)) else {
            return;
        };
        r.wait_scene();
        r.run(REST);
        let px = r.shot("rest-640");
        assert_eq!(r.c.frame_size(), (640, 480));
        let at = |x: usize, y: usize| {
            let o = (y * 640 + x) * 4;
            [px[o], px[o + 1], px[o + 2]]
        };
        let lit = (60..440)
            .flat_map(|y| (404..628).map(move |x| (x, y)))
            .filter(|&(x, y)| at(x, y).iter().all(|c| *c > 200))
            .count();
        assert!(lit > 20_000, "{lit}");
        for y in 60..440 {
            assert!(
                !at(632, y).iter().all(|c| *c > 200),
                "paper past the edge at {y}"
            );
        }
        let legend = (405..429)
            .flat_map(|y| (404..628).map(move |x| (x, y)))
            .filter(|&(x, y)| at(x, y).iter().all(|c| *c < 100))
            .count();
        assert!(legend > 20, "{legend}");
    }

    #[test]
    fn a_replaced_wallpaper_with_the_same_name_shows_after_a_rescan() {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        clocked(d.path());
        std::fs::create_dir_all(d.path().join("Wallpapers")).unwrap();
        let file = d.path().join("Wallpapers/w.png");
        solid_png(&file, 720, 480, [230, 20, 20]);
        let mut state = slot_store::read_slot_state(d.path());
        state.wallpaper = slot_store::WallpaperPick::File("w.png".into());
        write_slot_state(d.path(), &state).unwrap();
        let platform = Box::new(SimPlatform::at(d.path().to_path_buf()));
        let f = Frontend::boot_with(platform, Box::new(StubSink::draining()));
        let Some(mut r) = Rig::on(net(&[free_port()]), d, f, (OUT_W, OUT_H)) else {
            return;
        };
        r.wait_scene();
        solid_png(&file, 720, 480, [20, 20, 230]);
        r.tap(Btn::B);
        r.wait_closed();
        r.run(100);
        let px = r.shot("wallpaper");
        let [red, _, blue] = at(&px, 5, 5);
        assert!(blue > red, "{red} {blue}");
    }

    fn cue_rig() -> Option<(Rig, std::sync::Arc<Ring>)> {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        let mut state = slot_store::read_slot_state(d.path());
        state.clock_set = true;
        state.wallpapers_given = true;
        slot_store::write_slot_state(d.path(), &state).unwrap();
        let mut sink = StubSink::new();
        sink.open(48_000).unwrap();
        let ring = sink.ring();
        let platform = Box::new(SimPlatform::at(d.path().to_path_buf()));
        let f = Frontend::boot_with(platform, Box::new(sink));
        Some((Rig::on(net(&[free_port()]), d, f, (OUT_W, OUT_H))?, ring))
    }

    fn drained_loud(ring: &Ring) -> bool {
        let mut out = vec![0i16; 2048];
        let mut loud = false;
        for _ in 0..40 {
            ring.fill(&mut out);
            loud |= out.iter().any(|s| *s != 0);
        }
        loud
    }

    #[test]
    fn the_buzz_plays_and_a_lid_close_silences_it() {
        let Some((mut r, ring)) = cue_rig() else {
            return;
        };
        r.wait_scene();
        r.run(PRINT_AT - 100);
        assert!(!drained_loud(&ring));
        r.run(200);
        assert!(drained_loud(&ring));
        r.f.app_mut().apply(Action::LidClose);
        r.step(16);
        r.step(16);
        assert!(!drained_loud(&ring));
        ring.fill(&mut vec![0i16; 2048]);
        assert!(!drained_loud(&ring));
    }

    #[test]
    fn b_silences_the_buzz() {
        let Some((mut r, ring)) = cue_rig() else {
            return;
        };
        r.wait_scene();
        r.run(PRINT_AT + 100);
        assert!(drained_loud(&ring));
        r.tap(Btn::B);
        assert!(!drained_loud(&ring));
    }

    #[test]
    fn the_tear_plays_after_the_print() {
        let Some((mut r, ring)) = cue_rig() else {
            return;
        };
        r.wait_scene();
        r.run(TEAR_AT - 100);
        ring.hush();
        assert!(!drained_loud(&ring));
        r.run(200);
        assert!(drained_loud(&ring));
    }

    #[test]
    fn stopping_the_frontend_stops_the_server_and_keeps_its_edits() {
        let port = free_port();
        let Some(mut r) = Rig::new(net(&[port])) else {
            return;
        };
        r.wait_scene();
        let mut disk = slot_store::read_slot_state(r.d.path());
        let brightness = disk.brightness;
        disk.favorites = vec![Recent {
            platform: Platform::Gba,
            stem: "Fusion".into(),
        }];
        write_slot_state(r.d.path(), &disk).unwrap();
        r.f.app_mut().apply(Action::BrightnessDown);
        r.f.stop();
        assert!(!r.f.app().server_open());
        assert!(!answers(port));
        let after = slot_store::read_slot_state(r.d.path());
        assert_eq!(after.favorites.len(), 1);
        assert_ne!(after.brightness, brightness);
    }

    #[test]
    fn a_stop_shows_the_rebuild_for_changed_labels() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        let dir = r.d.path().join("Labels/GBA");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..8 {
            std::fs::write(
                r.d.path().join(format!("Games/GBA/L{i}.gba")),
                vec![0u8; 512],
            )
            .unwrap();
            solid_png(&dir.join(format!("L{i}.png")), 640, 320, [200, 20, 20]);
        }
        r.input.0.push_back(vec![RawEvent::Down(Btn::B)]);
        let mut seen = false;
        for _ in 0..5000 {
            r.step(16);
            seen |= r.f.rebuilding_labels();
            if seen && !r.f.rebuilding_labels() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(seen);
        assert!(!r.f.app().server_open());
        for cart in r.f.app().carts().filter(|c| c.stem.starts_with('L')) {
            assert!(slot::label_cache::fresh(r.d.path(), cart));
        }
    }

    #[test]
    fn a_quick_rebuild_never_reaches_the_screen() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        let dir = r.d.path().join("Labels/GBA");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..8 {
            std::fs::write(
                r.d.path().join(format!("Games/GBA/Q{i}.gba")),
                vec![0u8; 512],
            )
            .unwrap();
            solid_png(&dir.join(format!("Q{i}.png")), 640, 320, [200, 20, 20]);
        }
        r.input.0.push_back(vec![RawEvent::Down(Btn::B)]);
        let mut seen = false;
        for _ in 0..5000 {
            r.step(16);
            seen |= r.f.rebuilding_labels();
            if seen && !r.f.rebuilding_labels() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(seen);
        assert!(!r.f.rebuild_drawn());
    }

    #[test]
    fn buttons_work_while_a_small_rebuild_runs() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        let dir = r.d.path().join("Labels/GBA");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..8 {
            std::fs::write(
                r.d.path().join(format!("Games/GBA/B{i}.gba")),
                vec![0u8; 512],
            )
            .unwrap();
            solid_png(&dir.join(format!("B{i}.png")), 640, 320, [200, 20, 20]);
        }
        r.input.0.push_back(vec![RawEvent::Down(Btn::B)]);
        r.step(16);
        r.input.0.push_back(vec![RawEvent::Up(Btn::B)]);
        for _ in 0..5000 {
            r.step(16);
            if r.f.rebuilding_labels() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(r.f.rebuilding_labels());
        let before = r.f.app().quick_menu();
        assert!(before.is_some(), "{:?}", r.f.app().phase());
        r.f.advance(&mut r.input);
        r.input.0.push_back(vec![RawEvent::Down(Btn::Down)]);
        r.f.advance(&mut r.input);
        r.input.0.push_back(vec![RawEvent::Up(Btn::Down)]);
        r.f.advance(&mut r.input);
        assert!(r.f.rebuilding_labels());
        assert_ne!(r.f.app().quick_menu(), before);
    }

    fn slow_mdns(_: std::net::Ipv4Addr) -> Option<slot::server::Responder> {
        std::thread::sleep(Duration::from_millis(1500));
        None
    }

    #[test]
    fn the_scene_starts_before_the_paper_is_ready() {
        let net = ServerNet {
            responder: slow_mdns,
            ..net(&[free_port()])
        };
        let Some(mut r) = Rig::new(net) else {
            return;
        };
        while !r.f.app().server_scene_running() {
            r.step(16);
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(r.f.app().server_paper().is_none());
    }

    #[test]
    fn a_hurried_stop_never_rebuilds_labels_and_the_lid_still_wakes() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        let dir = r.d.path().join("Labels/GBA");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..40 {
            let rom = r.d.path().join(format!("Games/GBA/M{i:02}.gba"));
            std::fs::write(rom, vec![0u8; 512]).unwrap();
        }
        solid_png(&dir.join("M20.png"), 64, 32, [200, 20, 20]);
        r.input.0.push_back(vec![RawEvent::Down(Btn::Lid)]);
        r.step(16);
        r.wait_closed();
        let cart = r.f.app().carts().find(|c| c.stem == "M20").unwrap().clone();
        assert!(cart.label.is_some());
        assert!(matches!(r.f.app().phase(), Phase::Doze { .. }));
        for _ in 0..100 {
            r.step(16);
            std::thread::sleep(Duration::from_millis(2));
            assert!(!r.f.rebuilding_labels());
        }
        let root = r.d.path().to_path_buf();
        assert_eq!(slot::label_cache::stale(&root, r.f.app().carts()).len(), 1);
        r.input.0.push_back(vec![RawEvent::Up(Btn::Lid)]);
        r.f.advance(&mut r.input);
        assert!(!matches!(r.f.app().phase(), Phase::Doze { .. }));
    }

    #[test]
    fn a_rescan_with_a_new_game_returns_to_the_menu() {
        let Some(mut r) = Rig::new(net(&[free_port()])) else {
            return;
        };
        r.wait_scene();
        std::fs::write(r.d.path().join("Games/GBA/Added.gba"), vec![0u8; 512]).unwrap();
        r.tap(Btn::B);
        r.wait_closed();
        assert!(r.f.app().carts().any(|c| c.stem == "Added"));
    }
}
