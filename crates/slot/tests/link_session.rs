mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use common::{
    app_playing_in, app_playing_with, panel_with_battery, tmp_root_with_carts, StubSnapshot,
};
use slot::app::Phase;
use slot::emu::Speed;
use slot::link_net::{Cancel, TcpLink};
use slot::session::Session;
use slot_input::{Action, Btn, Millis, RawEvent};
use slot_power::{Battery, Charge};
use slot_retro::{LinkChannel, LoopbackLink, NETPACKET_RELIABLE};
use slot_store::{write_slot_state, Core, Platform, SlotState, StateRing};
use slot_ui::{LinkBadge, Toast};

#[test]
fn a_packet_survives_the_wire_intact() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(std::time::Duration::from_millis(150));
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let mut host = server.join().expect("host thread");

    host.send(NETPACKET_RELIABLE, b"\x01\x02\x03");
    client.send(NETPACKET_RELIABLE, b"from the other side");

    let got = wait_for(&mut client);
    assert_eq!(got.as_deref(), Some(&b"\x01\x02\x03"[..]));
    let got = wait_for(&mut host);
    assert_eq!(got.as_deref(), Some(&b"from the other side"[..]));
}

#[test]
fn batched_writes_keep_their_boundaries() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(std::time::Duration::from_millis(150));
    let mut raw = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut host = server.join().expect("host thread");

    let mut batch = Vec::new();
    batch.extend_from_slice(&5u16.to_be_bytes());
    batch.extend_from_slice(b"first");
    batch.extend_from_slice(&6u16.to_be_bytes());
    batch.extend_from_slice(b"second");
    raw.write_all(&batch).expect("write batch");

    assert_eq!(wait_for(&mut host).as_deref(), Some(&b"first"[..]));
    assert_eq!(wait_for(&mut host).as_deref(), Some(&b"second"[..]));
}

#[test]
fn try_recv_never_blocks_on_an_idle_link() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(std::time::Duration::from_millis(150));
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let _host = server.join().expect("host thread");

    let start = std::time::Instant::now();
    assert_eq!(client.try_recv(), None);
    assert!(
        start.elapsed() < std::time::Duration::from_millis(5),
        "try_recv blocked, which would cost frames"
    );
}

#[test]
fn peer_disconnecting_does_not_panic_or_hang() {
    let port = common::free_port();
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    let acceptor = std::thread::spawn(move || listener.accept().expect("accept").0);
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let host_raw = acceptor.join().expect("accept thread");

    drop(host_raw);

    std::thread::sleep(std::time::Duration::from_millis(200));
    for _ in 0..10 {
        assert_eq!(client.try_recv(), None);
    }
    for _ in 0..20 {
        client.send(NETPACKET_RELIABLE, b"into the void");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn dropping_the_link_closes_the_wire() {
    let port = common::free_port();
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    let acceptor = std::thread::spawn(move || listener.accept().expect("accept").0);
    let client = TcpLink::join("127.0.0.1", port).expect("join");
    let mut host_raw = acceptor.join().expect("accept thread");
    host_raw
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .expect("set read timeout");

    drop(client);

    let mut buf = [0u8; 1];
    let n = host_raw.read(&mut buf).expect("read after drop");
    assert_eq!(
        n, 0,
        "peer should observe a clean EOF, not a hang or an error"
    );
}

#[test]
fn send_never_blocks_on_a_peer_that_stopped_reading() {
    let port = common::free_port();
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    let acceptor = std::thread::spawn(move || listener.accept().expect("accept").0);
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let _peer = acceptor.join().expect("accept thread");

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let payload = vec![0u8; 60_000];
        for _ in 0..300 {
            client.send(NETPACKET_RELIABLE, &payload);
        }
        let _ = tx.send(());
    });

    assert!(
        rx.recv_timeout(Duration::from_secs(2)).is_ok(),
        "send blocked on a peer that stopped reading"
    );
}

#[test]
fn wrap_disables_nagle_on_both_sockets() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    let client = TcpLink::join("127.0.0.1", port).expect("join");
    let host = server.join().expect("host thread");

    assert!(
        host.nodelay().expect("nodelay"),
        "the host socket must disable Nagle"
    );
    assert!(
        client.nodelay().expect("nodelay"),
        "the joiner socket must disable Nagle"
    );
}

#[test]
fn host_binds_the_address_it_is_given_not_every_interface() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let kind = TcpLink::host("203.0.113.1", 0).err().map(|e| e.kind());
        let _ = tx.send(kind);
    });
    match rx.recv_timeout(Duration::from_secs(2)) {
        Ok(Some(kind)) => assert_eq!(kind, std::io::ErrorKind::AddrNotAvailable),
        Ok(None) => panic!("must not silently bind 0.0.0.0"),
        Err(_) => panic!(
            "host() did not return within 2s -- a bind-address regression blocks forever in \
             accept() instead of failing to bind, which is exactly what this test exists to \
             catch without hanging the suite to do it"
        ),
    }
}

fn wait_for(link: &mut TcpLink) -> Option<Vec<u8>> {
    for _ in 0..200 {
        if let Some(p) = link.try_recv() {
            return Some(p);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    None
}

#[test]
fn a_live_session_disables_rewind_and_state_loading() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert!(
        a.may_rewind() && a.may_load_state(),
        "not what a fresh app should refuse"
    );

    a.begin_link(0);
    assert!(a.link_active());
    assert!(!a.may_rewind(), "rewind interrupts communication");
    assert!(
        !a.may_load_state(),
        "a state load desynchronises the other device"
    );
}

#[test]
fn a_live_session_refuses_a_rewind_press_instead_of_dropping_it() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);

    a.apply(Action::RewindStart);
    assert!(
        a.refusal_active(a.now()),
        "a refused rewind must shake, not vanish silently"
    );
}

#[test]
fn a_live_session_refuses_a_state_load_even_when_one_exists() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.begin_link(0);

    a.apply(Action::LoadState);
    assert!(
        a.refusal_active(a.now()),
        "a refused load must shake, not vanish silently"
    );
}

#[test]
fn a_live_session_refuses_a_switcher_pick_even_when_one_exists() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    r.push(&[0u8; 64], b"png", "2026-08-09_00-00-00").unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::Polaroids);
    a.begin_link(0);

    a.apply(Action::GbaDown(Btn::A));
    assert!(
        a.refusal_active(a.now()),
        "picking a state in the switcher must be refused during a session"
    );
    assert!(
        matches!(a.phase(), Phase::Polaroids { .. }),
        "a refused pick must not also close the switcher"
    );
}

#[test]
fn a_live_session_refuses_to_undo_a_load() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    r.push(&[7u8; 64], b"png", "2026-08-09_00-00-00").unwrap();
    let (snapshot, loaded) = StubSnapshot::pair();
    let mut a = app_playing_with(d.path(), "Emerald", snapshot);

    a.apply(Action::Polaroids);
    a.apply(Action::GbaDown(Btn::A));
    assert!(a.undo_available(a.now()), "the load did not offer an undo");
    *loaded.lock().unwrap() = None;

    a.begin_link(0);
    a.undo(a.now());

    assert!(
        a.refusal_active(a.now()),
        "undoing a load must be refused during a session, the same hazard load_file guards"
    );
    assert!(
        loaded.lock().unwrap().is_none(),
        "the core must not have been moved to the prior state"
    );
    assert!(
        a.undo_available(a.now()),
        "a refused undo must not consume the offer"
    );
}

#[test]
fn a_live_session_refuses_to_open_the_switcher() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    r.push(&[0u8; 64], b"png", "2026-08-09_00-00-00").unwrap();
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);

    a.apply(Action::Polaroids);
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "opening the switcher pauses the core, which a session forbids"
    );
    assert!(
        a.refusal_active(a.now()),
        "a refused switcher open must shake"
    );
}

/// Holding POWER ends the link before shutdown pauses the core.
#[test]
fn a_power_hold_ends_a_live_session_and_powers_off() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    a.apply(Action::PowerHold);
    assert!(!a.link_active());
    assert!(a.powering_off());
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none(),
        "a linked state must not be saved"
    );
}

#[test]
fn ejecting_during_a_session_ends_it() {
    let d = tmp_root_with_carts(&["Emerald", "Ruby"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    assert!(a.link_active());

    a.apply(Action::Eject);
    assert!(
        !a.link_active(),
        "an ejected cart must not leave a phantom session behind"
    );
    assert!(a.may_rewind());
    assert!(a.may_load_state());
}

#[test]
fn a_power_press_ends_a_live_session_instead_of_flushing_only() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    assert!(a.link_active());

    a.apply(Action::PowerPress);
    assert!(!a.link_active(), "a power press must end a live session");
    assert!(
        matches!(a.phase(), Phase::Playing { .. }),
        "ending the session is not an eject or a doze"
    );

    a.apply(Action::PowerPress);
    assert!(!a.link_active());
}

#[test]
fn a_power_tap_ends_a_live_session_and_dozes_in_the_same_tap() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    assert!(a.link_active());

    a.apply(Action::PowerTap);
    assert!(!a.link_active(), "a power tap must end a live session");
    assert!(
        matches!(a.phase(), Phase::Doze { .. }),
        "ending the session must not leave the device awake behind the tap that closed it"
    );

    a.apply(Action::PowerTap);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

/// `LidClose` calls `doze` directly, so it
#[test]
fn a_lid_close_ends_a_live_session_and_dozes() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    assert!(a.link_active());

    a.apply(Action::LidClose);
    assert!(!a.link_active(), "closing the lid must end a live session");
    assert!(
        matches!(a.phase(), Phase::Doze { .. }),
        "ending the session must not leave the device awake behind a shut lid"
    );

    a.apply(Action::LidOpen);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn a_critical_battery_reading_ends_a_live_session_instead_of_pausing_it() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);
    assert!(a.link_active());

    a.on_battery(Battery {
        percent: 3,
        charge: Charge::Discharging,
    });
    assert!(
        !a.link_active(),
        "a critical battery reading must end a live session, not merely pause it"
    );
    assert!(a.powering_off(), "the shutdown itself must still proceed");
}

#[test]
fn doze_never_expires_while_a_session_is_live() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_power(common::panel(d.path(), Duration::from_secs(2)).0);
    a.apply(Action::LidClose);
    a.begin_link(0);

    for _ in 0..180 {
        a.update(1.0 / 60.0);
    }
    assert!(
        !a.powering_off(),
        "a link session was dropped by the doze timer"
    );
    assert!(matches!(a.phase(), Phase::Doze { .. }));
}

#[test]
fn ending_a_session_restores_normal_behaviour() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.begin_link(0);

    a.end_link();
    assert!(!a.link_active());
    assert!(a.may_rewind());
    assert!(a.may_load_state());
    assert_eq!(a.link_client_id(), None);
}

#[test]
fn link_client_id_reports_which_side_of_the_session_this_device_is() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    assert_eq!(a.link_client_id(), None, "nothing to ask about yet");

    a.begin_link(1);
    assert_eq!(a.link_client_id(), Some(1));
}

fn step(s: &mut Session, now: &mut Millis, ev: Option<RawEvent>) {
    *now += 16;
    s.feed(ev, *now);
    s.update(1.0 / 60.0);
}

fn wait_until(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if cond() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn ending_a_session_reaches_the_emulator_thread_too() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.emu()
        .expect("a cart is seated, a core must be running")
        .begin_link(0, Box::new(LoopbackLink::default()));
    assert!(
        wait_until(|| s.emu().is_some_and(|e| e.net().is_active())),
        "begin_link never took"
    );
    s.app_mut().begin_link(0);
    assert!(s.app().link_active());

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::Power)));
    assert!(
        !s.app().link_active(),
        "the app's own bookkeeping should have ended"
    );
    assert!(
        wait_until(|| s.emu().is_some_and(|e| !e.net().is_active())),
        "ending a session at the App level must reach the emulator thread too"
    );
}

#[test]
fn a_critical_battery_reading_reaches_the_emulator_thread_too() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.emu()
        .expect("a cart is seated, a core must be running")
        .begin_link(0, Box::new(LoopbackLink::default()));
    assert!(
        wait_until(|| s.emu().is_some_and(|e| e.net().is_active())),
        "begin_link never took"
    );
    s.app_mut().begin_link(0);
    assert!(s.app().link_active());

    let (power, _) = panel_with_battery(d.path(), Duration::from_secs(300), 1, 3);
    s.app_mut().set_power(power);
    step(&mut s, &mut now, None);

    assert!(
        !s.app().link_active(),
        "a critical battery reading must end a live session"
    );
    assert!(
        wait_until(|| s.emu().is_some_and(|e| !e.net().is_active())),
        "ending a session from inside `update` must reach the emulator thread too"
    );
}

#[test]
fn a_live_session_refuses_fast_forward() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.app_mut().begin_link(0);
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));
    for _ in 0..5 {
        step(&mut s, &mut now, None);
    }

    assert!(
        wait_until(|| s.emu().is_some_and(|e| e.observed_speed() == Speed::Normal)),
        "fast forward must be refused while a session is live"
    );
}

#[test]
fn the_link_screen_over_a_session_leaves_the_core_running() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.app_mut().begin_link(0);
    s.app_mut().apply(Action::GameMenu);
    assert!(s.app().game_menu_open(), "the screen never opened");
    for _ in 0..5 {
        step(&mut s, &mut now, None);
    }

    assert!(
        wait_until(|| s.emu().is_some_and(|e| e.observed_speed() == Speed::Normal)),
        "the screen paused a session that cannot survive being paused"
    );
    assert!(
        s.app().link_active(),
        "opening the screen ended the session"
    );

    step(&mut s, &mut now, Some(RawEvent::Down(Btn::A)));
    for _ in 0..3 {
        step(&mut s, &mut now, None);
    }
    assert!(
        s.emu().is_some_and(|e| e.input().0 == 0),
        "a press reached the game from under an open menu"
    );
}

#[test]
fn a_live_session_hides_the_fast_forward_badge() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;

    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, None);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.app_mut().begin_link(0);
    step(&mut s, &mut now, Some(RawEvent::Down(Btn::R2)));

    assert_eq!(
        s.app().ff_badge(),
        None,
        "the badge must not claim fast forward is happening while a session withholds it"
    );
}

#[test]
fn a_host_that_nobody_joins_gives_up_instead_of_waiting_forever() {
    let cancel = Cancel::new();
    let started = Instant::now();
    let Err(err) = TcpLink::host_until("127.0.0.1", 0, Duration::from_millis(300), &cancel) else {
        panic!("nobody connected, so this must not succeed");
    };
    assert_eq!(err.kind(), std::io::ErrorKind::TimedOut);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "waited past its bound"
    );
}

#[test]
fn a_host_can_be_cancelled_while_it_is_waiting() {
    let cancel = Cancel::new();
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.cancel();
    });
    let started = Instant::now();
    let Err(err) = TcpLink::host_until("127.0.0.1", 0, Duration::from_secs(60), &cancel) else {
        panic!("cancelled, so this must not succeed");
    };
    assert_eq!(err.kind(), std::io::ErrorKind::Interrupted);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "cancel did not take effect"
    );
}

#[test]
fn a_bounded_host_still_accepts_a_peer_that_does_arrive() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let cancel = Cancel::new();
    let peer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        TcpLink::join("127.0.0.1", port)
    });
    let mut host = TcpLink::host_until("127.0.0.1", port, Duration::from_secs(10), &cancel)
        .expect("the peer arrived inside the bound");
    let mut joiner = peer.join().unwrap().expect("joiner connected");

    host.send(0, b"ping");
    let got = wait_for(&mut joiner);
    assert_eq!(
        got.as_deref(),
        Some(&b"ping"[..]),
        "a bounded accept must yield a working link"
    );

    joiner.send(0, b"pong");
    let got = wait_for(&mut host);
    assert_eq!(
        got.as_deref(),
        Some(&b"pong"[..]),
        "the accepted socket must be blocking again, or the host never hears its peer"
    );
}

#[test]
fn a_joiner_waits_for_a_host_that_is_not_listening_yet() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let cancel = Cancel::new();
    let started = Instant::now();
    let host = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        TcpLink::host("127.0.0.1", port)
    });

    let mut joiner = TcpLink::join_until("127.0.0.1", port, Duration::from_secs(10), &cancel)
        .expect("the joiner must wait for a host that is merely late");
    let mut hosted = host.join().unwrap().expect("host bound");
    assert!(
        started.elapsed() >= Duration::from_millis(400),
        "connected before the host existed, so this proves nothing"
    );

    joiner.send(0, b"up");
    assert_eq!(wait_for(&mut hosted), Some(b"up".to_vec()));
    hosted.send(0, b"down");
    assert_eq!(wait_for(&mut joiner), Some(b"down".to_vec()));
}

#[test]
fn a_joiner_gives_up_when_no_host_ever_appears() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let cancel = Cancel::new();
    let started = Instant::now();
    let Err(e) = TcpLink::join_until("127.0.0.1", port, Duration::from_millis(300), &cancel) else {
        panic!("connected to a host that does not exist");
    };
    assert_eq!(
        e.kind(),
        std::io::ErrorKind::TimedOut,
        "a host that never came is `nobody arrived`, not a peer that vanished"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "waited past its bound"
    );
}

#[test]
fn a_joiner_can_be_cancelled_while_it_is_retrying() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let cancel = Cancel::new();
    let flag = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        flag.cancel();
    });
    let started = Instant::now();
    let Err(e) = TcpLink::join_until("127.0.0.1", port, Duration::from_secs(60), &cancel) else {
        panic!("cancelled, so this must not succeed");
    };
    assert_eq!(e.kind(), std::io::ErrorKind::Interrupted);
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "the bound was a minute; cancellation is what ended this"
    );
}

#[test]
fn a_link_whose_peer_goes_away_reports_itself_closed() {
    let port = common::free_port();
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    let acceptor = std::thread::spawn(move || listener.accept().expect("accept").0);
    let client = TcpLink::join("127.0.0.1", port).expect("join");
    let host_raw = acceptor.join().expect("accept thread");
    assert!(!client.is_closed(), "closed before anything went away");
    drop(host_raw);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !client.is_closed() {
        assert!(
            std::time::Instant::now() < deadline,
            "never noticed the peer leave"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn a_quiet_link_is_not_closed() {
    let port = common::free_port();
    let listener = TcpListener::bind(("127.0.0.1", port)).expect("bind");
    let acceptor = std::thread::spawn(move || listener.accept().expect("accept").0);
    let client = TcpLink::join("127.0.0.1", port).expect("join");
    let _host_raw = acceptor.join().expect("accept thread");
    std::thread::sleep(std::time::Duration::from_millis(200));
    assert!(!client.is_closed());
    assert!(!slot_retro::LoopbackLink::default().is_closed());
}

#[test]
fn ending_a_link_tells_the_peer_rather_than_only_dropping_the_socket() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let mut host = server.join().expect("host thread");

    assert!(!client.peer_ended(), "ended before anyone said so");
    host.send_end();

    let deadline = Instant::now() + Duration::from_secs(2);
    while !client.peer_ended() {
        assert!(
            Instant::now() < deadline,
            "the peer was never told the link had ended"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        client.try_recv(),
        None,
        "the control frame was delivered to the core as if it were a packet"
    );
}

#[test]
fn an_unknown_control_frame_is_ignored_and_the_stream_stays_in_step() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    let mut raw = TcpStream::connect(("127.0.0.1", port)).expect("connect");
    let mut host = server.join().expect("host thread");

    let mut batch = Vec::new();
    batch.extend_from_slice(&0u16.to_be_bytes());
    batch.extend_from_slice(&1u16.to_be_bytes());
    batch.push(0x7f);
    batch.extend_from_slice(&5u16.to_be_bytes());
    batch.extend_from_slice(b"after");
    raw.write_all(&batch).expect("write");

    let got = wait_for(&mut host);
    assert_eq!(
        got.as_deref(),
        Some(&b"after"[..]),
        "the packet behind an unknown control frame was lost or misread"
    );
    assert!(
        !host.peer_ended(),
        "an opcode this build does not know ended the session anyway"
    );
}

#[test]
fn an_empty_payload_is_refused_rather_than_framed_as_the_control_marker() {
    let port = common::free_port();
    let server = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    let mut client = TcpLink::join("127.0.0.1", port).expect("join");
    let mut host = server.join().expect("host thread");

    host.send(NETPACKET_RELIABLE, b"");
    host.send(NETPACKET_RELIABLE, b"real");

    let got = wait_for(&mut client);
    assert_eq!(
        got.as_deref(),
        Some(&b"real"[..]),
        "an empty packet was framed and arrived as one"
    );
    assert!(
        !client.peer_ended(),
        "an empty packet forged a control frame and ended the session"
    );
}

#[test]
fn a_live_session_shows_the_badge_for_its_role() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut app = common::app_playing_in(d.path(), "Emerald");
    assert_eq!(app.link_badge(), LinkBadge::Off);
    app.begin_link(0);
    assert_eq!(app.link_badge(), LinkBadge::Hosting);
    app.end_link();
    app.begin_link(1);
    assert_eq!(app.link_badge(), LinkBadge::Joined);
}

#[test]
fn a_peer_that_leaves_breaks_the_badge_for_two_seconds_then_ends_the_session() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut app = common::app_playing_in(d.path(), "Emerald");
    app.begin_link(0);
    app.peer_lost();
    assert_eq!(app.link_badge(), LinkBadge::HostingLost);
    for _ in 0..114 {
        app.update(1.0 / 60.0);
    }
    assert!(
        app.link_active(),
        "ended before the broken badge had its two seconds"
    );
    assert_eq!(app.link_badge(), LinkBadge::HostingLost);
    for _ in 0..8 {
        app.update(1.0 / 60.0);
    }
    assert!(!app.link_active(), "a lost peer's session never ended");
    assert_eq!(app.link_badge(), LinkBadge::Off);
}

#[test]
fn a_session_ended_on_this_device_shows_no_broken_badge() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut app = common::app_playing_in(d.path(), "Emerald");
    app.begin_link(1);
    app.apply(Action::PowerPress);
    assert!(!app.link_active());
    assert_eq!(app.link_badge(), LinkBadge::Off);
}

#[test]
fn a_peer_that_ends_the_link_ends_the_session_at_once_and_says_so() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut app = common::app_playing_in(d.path(), "Emerald");
    app.begin_link(0);
    app.peer_ended();

    assert!(
        !app.link_active(),
        "a session whose far end ended it deliberately was left running"
    );
    assert_eq!(
        app.link_badge(),
        LinkBadge::Off,
        "a deliberate ending broke the badge as if the peer had vanished"
    );
    assert_eq!(app.toast(), Some(Toast::PeerEnded));
}

#[test]
fn a_host_on_another_bios_ends_the_session_and_says_so() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut app = common::app_playing_in(d.path(), "Emerald");
    app.begin_link(1);
    app.bios_mismatch();

    assert!(!app.link_active());
    assert_eq!(app.toast(), Some(Toast::BiosMismatch));
}
