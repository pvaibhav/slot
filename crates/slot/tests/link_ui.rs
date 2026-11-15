mod common;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot::app::{
    App, GameMenu, LinkLegend, LinkRow, Phase, LINKED_HOLD_MS, LINK_LOST_MS, LINK_SCRIM,
    UNPLUG_HOLD_MS,
};
use slot::ble::{FakeBle, FakeBleRemote};
use slot::emu::{CoreState, EmuHandle, Speed};
use slot::link_kind::LinkKind;
use slot::link_net::{Cancel, TcpLink};
use slot::link_radio::{HostAp, LinkRole, RadioJob, RadioJobs, RadioRole};
use slot::link_start::{LinkFail, LinkStarter, LinkStep};
use slot::persist::{self, Snapshot};
use slot::session::Session;
use slot_input::{Action, Btn, Millis, RawEvent};
use slot_retro::{ButtonMask, LinkChannel};
use slot_store::{write_slot_state, Core, Platform, SlotState};
use slot_ui::{arrows_hint_face, hint_face, Draw, TexId, Toast, HINT_EDGE, MOUTH_H, OUT_H, OUT_W};
use tempfile::TempDir;

const BAIL: Duration = Duration::from_secs(5);

fn playing_on(core: Core) -> (App, TempDir) {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(core);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Playing { .. }), "never seated");
    (app, d)
}

const AP: HostAp = HostAp {
    freq_mhz: 5180,
    bssid: [2, 0, 0, 0, 0, 1],
};

fn fake_starter(
    socket: impl FnMut(u16, &Cancel) -> io::Result<TcpLink> + Send + 'static,
) -> LinkStarter {
    LinkStarter::spawn_with(
        Box::new(|_, _| Ok(None)),
        Box::new(|| {}),
        RadioRole::Host,
        0,
        Box::new(socket),
    )
}

fn io_err(kind: io::ErrorKind) -> io::Error {
    io::Error::new(kind, "from a test")
}

fn settle(app: &mut App) {
    let deadline = Instant::now() + BAIL;
    while matches!(app.game_menu(), Some(GameMenu::Working { .. })) {
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
        assert!(Instant::now() < deadline, "the overlay never left Working");
    }
}

fn fake_roles(app: &mut App) -> Vec<(TexId, u32, u32)> {
    let faces: Vec<(TexId, u32, u32)> = (0..LinkRow::ALL.len())
        .map(|i| (TexId::from_raw(700 + i), 120 + 40 * i as u32, 40))
        .collect();
    app.set_link_menu_faces(faces.clone());
    faces
}

#[test]
fn select_and_menu_open_the_link_screen_on_host() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    assert_eq!(app.game_menu(), Some(GameMenu::Pick(LinkRow::Host)));
    assert!(matches!(app.phase(), Phase::Playing { .. }));
}

#[test]
fn the_link_screen_opens_under_mgba_on_its_own_cable() {
    let (mut app, _d) = playing_on(Core::Mgba);
    app.apply(Action::GameMenu);
    assert_eq!(
        app.game_menu(),
        Some(GameMenu::Pick(LinkRow::Host)),
        "mGBA did not offer the link it carries"
    );
    assert_eq!(
        app.toast(),
        None,
        "it opened the screen and banished it too"
    );
}

#[test]
fn the_link_screen_on_gpsp_says_nothing_in_the_banner() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open());
    assert_eq!(app.toast(), None);
}

#[test]
fn left_and_right_swap_host_and_join_and_the_screen_remembers() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    assert_eq!(app.game_menu(), Some(GameMenu::Pick(LinkRow::Join)));
    app.apply(Action::GbaDown(Btn::Left));
    assert_eq!(app.game_menu(), Some(GameMenu::Pick(LinkRow::Host)));
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::B));
    assert!(!app.game_menu_open());
    app.apply(Action::GameMenu);
    assert_eq!(
        app.game_menu(),
        Some(GameMenu::Pick(LinkRow::Join)),
        "the last role was forgotten"
    );
}

#[test]
fn b_on_pick_hands_the_game_back() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::B));
    assert!(!app.game_menu_open());
    assert!(matches!(app.phase(), Phase::Playing { .. }));
}

#[test]
fn the_game_menu_does_not_open_on_the_shelf() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut app = common::boot(d.path());
    app.apply(Action::GameMenu);
    assert!(!app.game_menu_open(), "the shelf raised the in-game menu");
    assert!(matches!(app.phase(), Phase::Shelf));
    app.apply(Action::QuickMenu);
    assert!(
        matches!(app.phase(), Phase::QuickMenu { .. }),
        "the shelf lost its quick menu"
    );
}

#[test]
fn the_host_is_client_zero_and_the_joiner_client_one() {
    assert_eq!(LinkRow::Host.client_id(), 0);
    assert_eq!(LinkRow::Join.client_id(), 1);
    assert_eq!(LinkRow::Host.role(), LinkRole::Host);
    assert_eq!(LinkRow::Join.role(), LinkRole::Join);
    assert_eq!(LinkRow::from_client_id(0), LinkRow::Host);
    assert_eq!(LinkRow::from_client_id(1), LinkRow::Join);
    assert_eq!(LinkRow::Host.other(), LinkRow::Join);
}

#[test]
fn a_on_pick_starts_the_link_in_the_picked_role() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Working {
                role: LinkRow::Join,
                step: LinkStep::Waiting,
                ..
            })
        ),
        "A did not start a joiner: {:?}",
        app.game_menu()
    );
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
}

#[test]
fn each_failure_says_which_one_it_was() {
    for (kind, want) in [
        (io::ErrorKind::TimedOut, LinkFail::NobodyCame),
        (io::ErrorKind::ConnectionRefused, LinkFail::PeerVanished),
    ] {
        let (mut app, _d) = playing_on(Core::Gpsp);
        app.apply(Action::GameMenu);
        app.start_link(fake_starter(move |_, _| Err(io_err(kind))), 0);
        settle(&mut app);
        assert!(matches!(app.game_menu(), Some(GameMenu::Failed { fail, .. }) if fail == want));
    }
    let lines: Vec<&str> = [
        LinkFail::Radio,
        LinkFail::NobodyCame,
        LinkFail::PeerVanished,
    ]
    .iter()
    .map(|f| f.line(LinkKind::Cable))
    .collect();
    assert_eq!(
        lines.len(),
        lines.iter().collect::<std::collections::HashSet<_>>().len(),
        "two failures share a sentence, which is a generic 'link failed' in disguise"
    );
}

#[test]
fn b_on_a_failure_puts_the_player_back_in_the_game() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.start_link(fake_starter(|_, _| Err(io_err(io::ErrorKind::TimedOut))), 0);
    settle(&mut app);
    assert!(matches!(
        app.game_menu(),
        Some(GameMenu::Failed {
            fail: LinkFail::NobodyCame,
            ..
        })
    ));
    app.apply(Action::GbaDown(Btn::B));
    assert!(!app.game_menu_open());
    assert!(
        matches!(app.phase(), Phase::Playing { .. }),
        "a failed link ate the session"
    );
    assert!(!app.link_active(), "a failed link started a session anyway");
}

#[test]
fn a_cancelled_link_says_nothing_and_returns_to_the_game() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.start_link(
        fake_starter(|_, cancel: &Cancel| {
            let deadline = Instant::now() + BAIL;
            while !cancel.is_cancelled() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            Err(io_err(io::ErrorKind::Interrupted))
        }),
        0,
    );
    app.update(1.0 / 60.0);
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
    assert!(!app.game_menu_open(), "the cancel left a screen behind");
    assert!(matches!(app.phase(), Phase::Playing { .. }));
}

#[test]
fn a_link_that_comes_up_holds_linked_for_a_second_then_hands_the_game_back() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    let port = common::free_port();
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    app.start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", port)),
        0,
    );
    settle(&mut app);
    let _far = far.join().expect("host thread");

    assert!(matches!(
        app.game_menu(),
        Some(GameMenu::Linked {
            role: LinkRow::Host,
            ..
        })
    ));
    assert!(
        app.link_active(),
        "the session waited for the screen instead of starting"
    );
    let (client_id, _transport) = app.take_link_transport().expect("no transport handed on");
    assert_eq!(client_id, 0);

    for press in [Btn::A, Btn::B] {
        app.apply(Action::GbaDown(press));
    }
    app.apply(Action::GameMenu);
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Linked { .. })),
        "the hold took a press"
    );

    let frames = (LINKED_HOLD_MS as f32 / (1000.0 / 60.0)) as usize;
    for _ in 0..frames - 2 {
        app.update(1.0 / 60.0);
    }
    assert!(app.game_menu_open(), "LINKED left before its second");
    for _ in 0..4 {
        app.update(1.0 / 60.0);
    }
    assert!(!app.game_menu_open(), "LINKED never handed the game back");
    assert!(app.link_active());
}

#[test]
fn a_peer_lost_during_the_hold_closes_the_screen_and_breaks_the_badge() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    let port = common::free_port();
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    app.start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", port)),
        1,
    );
    settle(&mut app);
    let _far = far.join().expect("host thread");
    assert!(matches!(app.game_menu(), Some(GameMenu::Linked { .. })));
    app.peer_lost();
    assert!(
        !app.game_menu_open(),
        "LINKED stayed up over a link that just died"
    );
    assert_eq!(app.link_badge(), slot_ui::LinkBadge::JoinedLost);
}

#[test]
fn b_during_the_radio_step_does_not_hand_the_game_back_early() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    let (release, held) = channel::<()>();
    app.start_link(
        LinkStarter::spawn_with(
            Box::new(move |_, _| {
                held.recv().expect("released");
                Ok(None)
            }),
            Box::new(|| {}),
            RadioRole::Host,
            0,
            Box::new(|_, cancel: &Cancel| {
                Err(io_err(if cancel.is_cancelled() {
                    io::ErrorKind::Interrupted
                } else {
                    io::ErrorKind::TimedOut
                }))
            }),
        ),
        0,
    );
    for _ in 0..10 {
        app.update(1.0 / 60.0);
    }
    app.apply(Action::GbaDown(Btn::B));
    for _ in 0..10 {
        app.update(1.0 / 60.0);
    }
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Working { .. })),
        "B handed the game back while the radio was still coming up behind it"
    );
    release.send(()).expect("release the radio");
    settle(&mut app);
    assert!(!app.game_menu_open(), "the cancel never landed at all");
}

#[test]
fn a_shut_lid_cancels_the_link_it_interrupted() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    let cancelled = Arc::new(AtomicBool::new(false));
    let seen = cancelled.clone();
    app.start_link(
        fake_starter(move |_, cancel: &Cancel| {
            let deadline = Instant::now() + BAIL;
            while !cancel.is_cancelled() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(2));
            }
            seen.store(cancel.is_cancelled(), Ordering::SeqCst);
            Err(io_err(io::ErrorKind::Interrupted))
        }),
        0,
    );
    app.update(1.0 / 60.0);
    app.apply(Action::LidClose);
    assert!(
        !app.game_menu_open(),
        "the overlay outlived the game it was drawn over"
    );
    let deadline = Instant::now() + BAIL;
    while !cancelled.load(Ordering::SeqCst) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        cancelled.load(Ordering::SeqCst),
        "a starter dropped mid-wait leaves a host's access point up for thirty seconds"
    );
}

#[test]
fn the_shortcut_opens_the_connected_screen_over_a_live_session() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    let log = watched(&mut app);
    app.begin_link(0);
    app.apply(Action::GameMenu);
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Linked { opened: true, .. })),
        "the shortcut did not open the connected screen"
    );
    assert!(app.link_active(), "opening the screen ended the session");
    assert_eq!(app.toast(), None, "nothing has happened to announce yet");
    assert!(
        log.jobs().is_empty(),
        "the radio was touched by a screen that only opened"
    );
}

#[test]
fn b_leaves_the_session_running() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.begin_link(0);
    app.apply(Action::GameMenu);
    let log = watched(&mut app);
    app.apply(Action::GbaDown(Btn::B));
    assert!(!app.game_menu_open(), "B did not leave the screen");
    assert!(app.link_active(), "B ended the session it was opened over");
    assert!(
        !log.jobs().contains(&RadioJob::Cool),
        "leaving a live session cooled the radio it runs on"
    );
}

#[test]
fn a_ends_the_session_and_says_so() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.begin_link(0);
    app.apply(Action::GameMenu);
    let log = watched(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert!(!app.link_active(), "A left the session running");
    assert_eq!(app.toast(), Some(Toast::LinkEnded));
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Unplug { .. })),
        "A did not put the plug back out: {:?}",
        app.game_menu()
    );
    for _ in 0..((UNPLUG_HOLD_MS / 16 + 4) as usize) {
        app.update(1.0 / 60.0);
    }
    assert!(!app.game_menu_open(), "the screen stayed up over the game");
    assert_eq!(log.jobs(), vec![RadioJob::Down, RadioJob::Cool]);
}

#[test]
fn a_peer_ending_the_link_unplugs_on_this_device_too() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.begin_link(0);
    assert!(!app.game_menu_open(), "nothing should be on screen yet");

    app.peer_ended();

    assert!(
        !app.link_active(),
        "the ending waited on the animation instead of the other way round"
    );
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Unplug { .. })),
        "the far end's ending did not unplug on this device: {:?}",
        app.game_menu()
    );
    assert_eq!(app.toast(), Some(Toast::PeerEnded));

    for _ in 0..((UNPLUG_HOLD_MS / 16 + 4) as usize) {
        app.update(1.0 / 60.0);
    }
    assert!(
        !app.game_menu_open(),
        "the unplug screen never left by itself"
    );
}

#[test]
fn the_link_screen_draws_its_role_over_the_game() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    let roles = fake_roles(&mut app);
    let shot = TexId::from_raw(7);
    app.set_link_shot(Some(shot));
    app.apply(Action::GameMenu);
    let mut out = Vec::new();
    app.draw(&mut out);
    let shot_at = out
        .iter()
        .position(|d| {
            matches!(*d, Draw::Tex { tex, w, h, .. }
                if tex == shot && w == OUT_W as f32 && h == OUT_H as f32)
        })
        .expect("the screen drew no screenshot of the game");
    let scrim = out
        .iter()
        .position(|d| {
            matches!(*d, Draw::Rect { w, h, colour, .. }
        if w == OUT_W as f32 && h == OUT_H as f32 && colour == LINK_SCRIM)
        })
        .expect("the screen drew no scrim over the game");
    let host = out
        .iter()
        .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == roles[0].0))
        .expect("HOST never reached the frame");
    assert!(shot_at < scrim && scrim < host);
}

fn session_playing_on_gpsp() -> (Session, TempDir, Millis) {
    let d = common::tmp_root_with_carts(&["Emerald"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", Core::Gpsp)
        .expect("write core");
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    (s, d, now)
}

fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
    *now += 16;
    s.feed(events.iter().copied(), *now);
    s.update(1.0 / 60.0);
}

fn runs_at(s: &mut Session, now: &mut Millis, want: Speed) -> bool {
    let deadline = Instant::now() + BAIL;
    while s.observed_speed() != Some(want) {
        if Instant::now() >= deadline {
            return false;
        }
        step(s, now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}

#[test]
fn the_open_menu_pauses_the_game_underneath_it() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never started running, so pausing it proves nothing"
    );
    step(
        &mut s,
        &mut now,
        &[RawEvent::Down(Btn::Select), RawEvent::Down(Btn::Menu)],
    );
    assert!(
        s.app().game_menu_open(),
        "SELECT+MENU never reached the app through the gesture layer"
    );
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "the game ran on behind the menu"
    );
}

#[test]
fn a_started_link_reaches_the_emulator_thread_with_its_transport() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    let port = common::free_port();
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    s.app_mut().start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", port)),
        1,
    );
    let deadline = Instant::now() + BAIL;
    while s.app().game_menu_open() {
        assert!(Instant::now() < deadline, "the link never came up");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    let _far = far.join().expect("host thread");
    assert!(s.app().link_active(), "no session started at all");
    assert_eq!(s.app().link_client_id(), Some(1), "the joiner is client 1");
    let deadline = Instant::now() + BAIL;
    while !s.emu().is_some_and(|e| e.net().is_active()) {
        assert!(
            Instant::now() < deadline,
            "the transport never reached the emulator thread"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_button_the_menu_is_using_never_reaches_the_game() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    step(
        &mut s,
        &mut now,
        &[RawEvent::Down(Btn::Select), RawEvent::Down(Btn::Menu)],
    );
    assert!(s.app().game_menu_open(), "the chord never reached the app");
    step(&mut s, &mut now, &[RawEvent::Down(Btn::A)]);
    assert_eq!(
        s.emu().expect("a core is running").input(),
        ButtonMask(0),
        "the press that picked a row was handed to the game as well"
    );
}

#[test]
fn a_dropped_peer_breaks_the_badge_and_ends_the_session_end_to_end() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    let port = common::free_port();
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    s.app_mut().start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", port)),
        1,
    );
    let deadline = Instant::now() + BAIL;
    while s.app().game_menu_open() {
        assert!(Instant::now() < deadline, "the link never came up");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    let far = far.join().expect("host thread");

    let deadline = Instant::now() + BAIL;
    while !(s.app().link_active() && s.emu().is_some_and(|e| e.net().is_active())) {
        assert!(
            Instant::now() < deadline,
            "the link never went live on both sides"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }

    drop(far);

    let deadline = Instant::now() + BAIL;
    while s.app().link_badge() != slot_ui::LinkBadge::JoinedLost {
        assert!(Instant::now() < deadline, "the badge never broke");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }

    let margin_steps = (LINK_LOST_MS / 16) as usize + 30;
    for _ in 0..margin_steps {
        step(&mut s, &mut now, &[]);
    }
    assert!(!s.app().link_active(), "the session never ended");

    let deadline = Instant::now() + BAIL;
    while s.emu().is_some_and(|e| e.net().is_active()) {
        assert!(
            Instant::now() < deadline,
            "bridge_link never carried the ending to the emulator thread"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_peer_that_ends_the_link_ends_this_session_without_waiting_out_the_timeout() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    let port = common::free_port();
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    s.app_mut().start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", port)),
        1,
    );
    let deadline = Instant::now() + BAIL;
    while s.app().game_menu_open() {
        assert!(Instant::now() < deadline, "the link never came up");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut far = far.join().expect("host thread");

    let deadline = Instant::now() + BAIL;
    while !(s.app().link_active() && s.emu().is_some_and(|e| e.net().is_active())) {
        assert!(
            Instant::now() < deadline,
            "the link never went live on both sides"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }

    far.send_end();
    assert!(
        !far.is_closed(),
        "the far socket was already closed, so nothing below is about the message"
    );

    let began = now;
    let deadline = Instant::now() + BAIL;
    while s.app().link_active() {
        assert!(
            Instant::now() < deadline,
            "the far end's ending never reached this session"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }

    assert!(
        now - began < LINK_LOST_MS,
        "the session took {}ms to end, which is the lost-peer timeout rather than the message",
        now - began
    );
    assert_eq!(
        s.app().toast(),
        Some(Toast::PeerEnded),
        "the banner did not say the link had been ended from the other end"
    );
    assert_eq!(
        s.app().link_badge(),
        slot_ui::LinkBadge::Off,
        "a deliberate ending broke the badge as if the peer had vanished"
    );

    let deadline = Instant::now() + BAIL;
    while s.emu().is_some_and(|e| e.net().is_active()) {
        assert!(
            Instant::now() < deadline,
            "bridge_link never carried the ending to the emulator thread"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }
}

struct CutRadio(Arc<AtomicBool>);

impl RadioJobs for CutRadio {
    fn ask(&mut self, job: RadioJob) {
        if job == RadioJob::Down {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn warmed(&self) -> bool {
        false
    }
}

fn pipe(mut from: std::net::TcpStream, mut to: std::net::TcpStream, cut: Arc<AtomicBool>) {
    use std::io::{Read, Write};
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while let Ok(n) = from.read(&mut buf) {
            if n == 0 {
                break;
            }
            if cut.load(Ordering::SeqCst) {
                continue;
            }
            if to.write_all(&buf[..n]).is_err() {
                break;
            }
        }
        if !cut.load(Ordering::SeqCst) {
            let _ = to.shutdown(std::net::Shutdown::Write);
        }
    });
}

fn air(far_port: u16, near_port: u16, cut: Arc<AtomicBool>) {
    let listener = std::net::TcpListener::bind(("127.0.0.1", near_port)).expect("bind the air");
    std::thread::spawn(move || {
        let (near, _) = listener.accept().expect("the near end never came");
        let far = std::net::TcpStream::connect(("127.0.0.1", far_port)).expect("reach the far end");
        pipe(
            near.try_clone().expect("clone"),
            far.try_clone().expect("clone"),
            cut.clone(),
        );
        pipe(far, near, cut);
    });
}

#[test]
fn ending_the_link_tells_the_far_end_before_the_radio_goes_down() {
    let (mut s, _d, mut now) = session_playing_on_gpsp();
    let cut = Arc::new(AtomicBool::new(false));
    s.app_mut().set_radio_jobs(Box::new(CutRadio(cut.clone())));
    let (far_port, near_port) = (common::free_port(), common::free_port());
    let far = std::thread::spawn(move || TcpLink::host("127.0.0.1", far_port).expect("host"));
    std::thread::sleep(Duration::from_millis(150));
    air(far_port, near_port, cut.clone());
    s.app_mut().start_link(
        fake_starter(move |_, _| TcpLink::join("127.0.0.1", near_port)),
        1,
    );
    let deadline = Instant::now() + BAIL;
    while s.app().game_menu_open() {
        assert!(Instant::now() < deadline, "the link never came up");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    let far = far.join().expect("host thread");
    let deadline = Instant::now() + BAIL;
    while !(s.app().link_active() && s.emu().is_some_and(|e| e.net().is_active())) {
        assert!(
            Instant::now() < deadline,
            "the link never went live on both sides"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(2));
    }

    s.app_mut().apply(Action::GameMenu);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::A)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::A)]);
    assert!(!s.app().link_active(), "A did not end the session");

    let deadline = Instant::now() + Duration::from_secs(2);
    while !(far.peer_ended() && cut.load(Ordering::SeqCst)) && Instant::now() < deadline {
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(16));
    }
    assert!(
        far.peer_ended(),
        "the radio came down before the far end heard the link end (radio down: {})",
        cut.load(Ordering::SeqCst)
    );
    assert!(cut.load(Ordering::SeqCst), "the radio never came down");
}

fn fake_link_sprites() -> slot::link_screen::LinkSprites {
    let s = |n: usize| slot::link_screen::Sprite {
        tex: TexId::from_raw(n),
        w: 10,
        h: 10,
    };
    slot::link_screen::LinkSprites {
        port: s(1),
        plug_host: s(2),
        plug_join: s(3),
        adapter: s(4),
        arcs_right: [s(7), s(8), s(9)],
        arcs_left: [s(10), s(11), s(12)],
        clicks: s(13),
        arrow_left: s(14),
        arrow_right: s(15),
    }
}

fn seated_on_gpsp(d: &TempDir) -> App {
    seated_on(d, Core::Gpsp)
}

fn seated_on(d: &TempDir, core: Core) -> App {
    seated_on_platform(d, core, Platform::Gba)
}

fn seated_on_platform(d: &TempDir, core: Core, platform: Platform) -> App {
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(core);
    app.set_platform(platform);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    app.set_link_sprites(fake_link_sprites());
    app
}

fn open_link_screen(d: &TempDir) -> Vec<Draw> {
    let mut app = seated_on_gpsp(d);
    app.apply(Action::GameMenu);
    let mut out = Vec::new();
    app.draw(&mut out);
    out
}

#[test]
fn a_pokemon_cart_shows_the_adapter() {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Pokemon Emerald", "POKEMON EMER", "BPEE");
    let out = open_link_screen(&d);
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(4))),
        "no adapter"
    );
    assert!(
        !out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(2))),
        "a plug on a wireless cart"
    );
}

#[test]
fn a_pokemon_hack_shows_the_cable() {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Pokemon Emerald", "POKEMON EMER", "BPEE");
    let rom = d.path().join("Games/GBA").join("Pokemon Emerald.gba");
    let mut bytes = std::fs::read(&rom).expect("read rom");
    bytes[3] = 0;
    std::fs::write(&rom, bytes).expect("rewrite rom");
    let out = open_link_screen(&d);
    assert!(
        out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(2))),
        "no plug"
    );
    assert!(
        !out.iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == TexId::from_raw(4))),
        "the adapter on a Pokémon hack"
    );
}

fn select(app: &mut App) {
    app.apply(Action::GbaDown(Btn::Select));
    app.apply(Action::GbaUp(Btn::Select));
}

fn drawn_hardware(app: &App) -> LinkKind {
    let mut out = Vec::new();
    app.draw(&mut out);
    let drew = |n: usize| {
        out.iter().any(|d| {
            matches!(*d, Draw::Tex { tex, .. } | Draw::Part { tex, .. } | Draw::Turned { tex, .. }
                if tex == TexId::from_raw(n))
        })
    };
    match (drew(2) || drew(3), drew(4)) {
        (true, false) => LinkKind::Cable,
        (false, true) => LinkKind::Wireless,
        other => panic!("the screen drew (plug, adapter) = {other:?}"),
    }
}

fn reaches_waiting(app: &mut App) -> bool {
    let deadline = Instant::now() + BAIL;
    while !matches!(
        app.game_menu(),
        Some(GameMenu::Working {
            step: LinkStep::Waiting,
            ..
        })
    ) {
        if Instant::now() >= deadline {
            return false;
        }
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}

fn idle(app: &mut App, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn listening(app: &mut App) -> FakeBleRemote {
    let (ble, remote) = FakeBle::new();
    app.set_ble(Box::new(ble));
    remote
}

fn joins(app: &mut App, ble: &FakeBleRemote, d: &TempDir, stem: &str) -> bool {
    joins_on(app, ble, d, stem, LinkKind::Cable)
}

fn joins_on(app: &mut App, ble: &FakeBleRemote, d: &TempDir, stem: &str, kind: LinkKind) -> bool {
    let mut heard = common::heard_host(d, stem, 7, AP, -60);
    heard.advert.kind = kind;
    ble.hear(heard);
    let deadline = Instant::now() + BAIL;
    while app.join_target() != Some(AP) {
        if Instant::now() >= deadline {
            return false;
        }
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    reaches_waiting(app)
}

fn switched_and_picked() -> (App, TempDir, FakeBleRemote) {
    let (mut app, d) = playing_on(Core::Gpsp);
    let ble = listening(&mut app);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    (app, d, ble)
}

#[test]
fn select_on_pick_switches_the_hardware_and_the_cart_keeps_it() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.set_link_sprites(fake_link_sprites());
    app.apply(Action::GameMenu);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "the test cart's own header links by cable"
    );
    select(&mut app);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Wireless,
        "SELECT switched nothing"
    );
    assert_eq!(
        app.game_menu(),
        Some(GameMenu::Pick(LinkRow::Host)),
        "SELECT did more than switch the hardware"
    );
    app.apply(Action::GbaDown(Btn::B));
    app.apply(Action::GameMenu);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Wireless,
        "the switch was forgotten when the screen closed"
    );
    select(&mut app);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "SELECT only goes one way"
    );
}

#[test]
fn a_in_the_mode_the_game_already_runs_starts_the_link_straight_away() {
    let _link = common::link_port_lock();
    let (mut app, d) = playing_on(Core::Gpsp);
    let ble = listening(&mut app);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_link_reload(),
        None,
        "a mode the game already runs asked for a reload"
    );
    assert!(joins(&mut app, &ble, &d, "Emerald"), "A started no link");
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
}

#[test]
fn a_in_a_switched_mode_asks_for_the_game_to_reload_first() {
    let _link = common::link_port_lock();
    let (mut app, _d, _ble) = switched_and_picked();
    assert_eq!(
        app.take_link_reload(),
        Some(("Emerald".to_string(), "rfu")),
        "no reload asked for, or the wrong one"
    );
    assert_eq!(
        app.take_link_reload(),
        None,
        "the request was handed on twice"
    );
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Working {
                role: LinkRow::Join,
                step: LinkStep::Radio,
                ..
            })
        ),
        "the screen did not go to its first step: {:?}",
        app.game_menu()
    );
    idle(&mut app, 150);
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Working {
                step: LinkStep::Radio,
                ..
            })
        ),
        "a link started before the game reloaded: {:?}",
        app.game_menu()
    );
}

#[test]
fn the_reload_finishing_starts_the_link() {
    let _link = common::link_port_lock();
    let (mut app, d, ble) = switched_and_picked();
    let Some(GameMenu::Working { since, .. }) = app.game_menu() else {
        panic!("A did not start working: {:?}", app.game_menu());
    };
    app.take_link_reload().expect("no reload asked for");
    idle(&mut app, 50);
    app.link_reload_done();
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Working { role: LinkRow::Join, since: s, .. }) if s == since
        ),
        "the screen started over: {:?}",
        app.game_menu()
    );
    assert!(
        joins(&mut app, &ble, &d, "Emerald"),
        "the reload finished and no link started"
    );
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
}

#[test]
fn b_during_the_reload_hands_the_game_back_once_it_finishes() {
    let _link = common::link_port_lock();
    let (mut app, _d, _ble) = switched_and_picked();
    app.take_link_reload().expect("no reload asked for");
    app.apply(Action::GbaDown(Btn::B));
    idle(&mut app, 50);
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Working { .. })),
        "B handed the game back while it was still loading"
    );
    app.link_reload_done();
    assert!(!app.game_menu_open(), "the cancel left a screen behind");
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    idle(&mut app, 150);
    assert!(!app.game_menu_open(), "a link started anyway");
    assert!(!app.link_active());
}

#[test]
fn a_reload_that_fails_goes_back_to_the_mode_the_game_came_from() {
    let _link = common::link_port_lock();
    let (mut app, _d, _ble) = switched_and_picked();
    app.set_link_sprites(fake_link_sprites());
    assert_eq!(app.take_link_reload(), Some(("Emerald".to_string(), "rfu")));
    app.link_reload_failed();
    assert_eq!(
        app.take_link_reload(),
        Some(("Emerald".to_string(), "auto")),
        "the reload that failed did not go back to the mode the game came from"
    );
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Working { .. })),
        "the screen closed with the game still loading behind it"
    );
    app.link_reload_done();
    assert!(
        !app.game_menu_open(),
        "the screen stayed up over a switch that never happened"
    );
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    assert!(
        app.refusal_active(app.now()),
        "the game came back without saying the switch was refused"
    );
    idle(&mut app, 150);
    assert!(!app.game_menu_open(), "a link started anyway");
    assert!(!app.link_active());
    app.apply(Action::GameMenu);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "the cart kept the switch that failed"
    );
}

#[test]
fn a_game_that_loads_in_neither_mode_comes_back_out_refused() {
    let _link = common::link_port_lock();
    let (mut app, _d, _ble) = switched_and_picked();
    app.take_link_reload().expect("no reload asked for");
    app.link_reload_failed();
    app.take_link_reload().expect("no way back asked for");
    app.link_reload_failed();
    assert_eq!(app.take_link_reload(), None, "a third load was asked for");
    assert!(!app.game_menu_open(), "the screen outlived the game");
    assert!(
        matches!(app.phase(), Phase::Ejecting { .. }),
        "the cart stayed seated with no game: {:?}",
        app.phase()
    );
    assert!(
        app.alert_visible(),
        "the cart came out without saying it was refused"
    );
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Shelf), "{:?}", app.phase());
}

#[test]
fn a_lid_shut_over_a_game_that_loads_in_neither_mode_opens_onto_the_shelf() {
    let _link = common::link_port_lock();
    let (mut app, _d, _ble) = switched_and_picked();
    app.take_link_reload().expect("no reload asked for");
    app.apply(Action::LidClose);
    assert!(matches!(app.phase(), Phase::Doze { .. }));
    app.link_reload_failed();
    assert_eq!(
        app.take_link_reload(),
        Some(("Emerald".to_string(), "auto")),
        "the shut lid dropped the way back"
    );
    app.link_reload_failed();
    app.apply(Action::LidOpen);
    assert!(
        matches!(app.phase(), Phase::Shelf),
        "the lid opened onto a cart with no game: {:?}",
        app.phase()
    );
}

#[test]
fn select_is_refused_where_gpsp_would_link_the_same_either_way() {
    let _link = common::link_port_lock();
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Mario Golf", "MARIO GOLF", "BMGE");
    let mut app = seated_on_gpsp(&d);
    let ble = listening(&mut app);
    app.apply(Action::GameMenu);
    assert_eq!(drawn_hardware(&app), LinkKind::Wireless);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    assert!(app.refusal_active(app.now()), "SELECT was not refused");
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Wireless,
        "a plug drawn over a game gpSP links by adapter"
    );
    assert_eq!(app.game_menu(), Some(GameMenu::Pick(LinkRow::Join)));
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_link_reload(),
        None,
        "a reload for a mode gpSP would not change"
    );
    assert!(joins(&mut app, &ble, &d, "Mario Golf"), "A started no link");
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
}

#[test]
fn a_reloads_only_for_a_serial_the_core_was_not_loaded_with() {
    let _link = common::link_port_lock();
    let (mut app, d) = playing_on(Core::Gpsp);
    let ble = listening(&mut app);
    app.set_link_loaded("rfu");
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_link_reload(),
        None,
        "the adapter, picked over a core loaded on rfu, asked for a reload"
    );
    assert!(
        joins_on(&mut app, &ble, &d, "Emerald", LinkKind::Wireless),
        "A started no link"
    );
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
    app.apply(Action::GameMenu);
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_link_reload(),
        Some(("Emerald".to_string(), "auto")),
        "the cable, picked over a core loaded on rfu, linked without a reload"
    );
}

struct RefusedResume;

impl Snapshot for RefusedResume {
    fn state(&self) -> Option<Vec<u8>> {
        Some(vec![0; 8])
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        None
    }

    fn load(&self, _state: Vec<u8>) {}

    fn resume_trusted(&self) -> bool {
        false
    }
}

#[test]
fn a_switch_is_refused_over_a_resume_the_core_would_not_take() {
    let _link = common::link_port_lock();
    let (mut app, d) = playing_on(Core::Gpsp);
    let ble = listening(&mut app);
    app.set_snapshot(Box::new(RefusedResume));
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert_eq!(
        app.take_link_reload(),
        None,
        "a reload over a state that cannot be saved"
    );
    assert!(app.refusal_active(app.now()), "A was not refused");
    assert_eq!(
        app.game_menu(),
        Some(GameMenu::Pick(LinkRow::Join)),
        "the refusal left Pick"
    );
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    assert!(
        joins(&mut app, &ble, &d, "Emerald"),
        "the mode the game already runs did not link"
    );
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
}

#[test]
fn pick_names_cancel_and_mode_across_the_strip() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    let width = |k: LinkLegend| match k {
        LinkLegend::Cancel => hint_face("B", "Cancel").w,
        LinkLegend::Mode => hint_face("SELECT", "Mode").w,
        LinkLegend::Swap => arrows_hint_face("Swap").w,
        LinkLegend::Link => hint_face("A", "Link").w,
        LinkLegend::Ok => hint_face("A", "OK").w,
        LinkLegend::Back => hint_face("B", "Back").w,
        LinkLegend::EndLink => hint_face("A", "End Link").w,
    };
    let faces: Vec<(TexId, u32)> = LinkLegend::ALL
        .iter()
        .map(|k| (TexId::from_raw(900 + k.index()), width(*k)))
        .collect();
    app.set_link_legend_faces(faces.clone());
    app.apply(Action::GameMenu);
    let mut out = Vec::new();
    app.draw(&mut out);
    let mut keys: Vec<(f32, f32, f32, LinkLegend)> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex { x, y, w, tex, .. } => LinkLegend::ALL
                .iter()
                .find(|k| faces[k.index()].0 == tex)
                .map(|k| (x, y, w, *k)),
            _ => None,
        })
        .collect();
    keys.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        keys.iter().map(|k| k.3).collect::<Vec<_>>(),
        [LinkLegend::Cancel, LinkLegend::Mode],
    );
    assert!(
        keys.iter().all(|k| k.1 == OUT_H as f32 - MOUTH_H),
        "the keys do not fill the carousel footer"
    );
    let left = keys[0].0;
    let (x, _, w, _) = keys[keys.len() - 1];
    let right = x + w - HINT_EDGE as f32;
    assert!(
        left.abs() < 1.0,
        "Cancel is not on the left of the strip: {left}"
    );
    assert!(
        (right - OUT_W as f32).abs() < 1.0,
        "Mode is not on the right of the strip: {right}"
    );
    let cancel_end = keys[0].0 + keys[0].2 - HINT_EDGE as f32;
    let mode_start = keys[1].0;
    assert!(
        mode_start - cancel_end > 80.0,
        "no gap between Cancel and Mode for the socket: {cancel_end}..{mode_start}"
    );
}

fn counter(s: &Session) -> u64 {
    let state = s
        .emu()
        .expect("a core is running")
        .request_state()
        .recv_timeout(BAIL)
        .expect("the core gave up no state");
    u64::from_le_bytes(state.try_into().expect("mock state is 8 bytes"))
}

#[test]
fn a_link_in_a_switched_mode_reloads_the_game_and_then_starts_the_link() {
    let (mut s, d, mut now) = session_playing_on_gpsp();
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never started running, so a fresh core would look the same"
    );
    let ble = listening(s.app_mut());
    step(
        &mut s,
        &mut now,
        &[RawEvent::Down(Btn::Select), RawEvent::Down(Btn::Menu)],
    );
    step(
        &mut s,
        &mut now,
        &[RawEvent::Up(Btn::Menu), RawEvent::Up(Btn::Select)],
    );
    assert!(s.app().game_menu_open(), "the chord never reached the app");
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "the game ran on behind the menu"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Right)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Right)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);

    let played = counter(&s);
    assert!(
        played > 0 && s.frames_published() > 0,
        "nothing ran before the reload"
    );

    step(&mut s, &mut now, &[RawEvent::Down(Btn::A)]);
    assert_eq!(
        s.frames_published(),
        0,
        "the emulator that was running is still the one in the slot"
    );
    let deadline = Instant::now() + BAIL;
    while s.emu().map(EmuHandle::state) != Some(CoreState::Ready) {
        assert!(Instant::now() < deadline, "the reloaded game never loaded");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        counter(&s),
        played,
        "the reloaded game did not come back where it left off"
    );

    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    let deadline = Instant::now() + BAIL;
    while s.app().join_target() != Some(AP)
        || !matches!(
            s.app().game_menu(),
            Some(GameMenu::Working {
                step: LinkStep::Waiting,
                ..
            })
        )
    {
        assert!(
            Instant::now() < deadline,
            "the game reloaded and no link started: {:?}",
            s.app().game_menu()
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }

    step(&mut s, &mut now, &[RawEvent::Down(Btn::B)]);
    let deadline = Instant::now() + BAIL;
    while s.app().game_menu_open() {
        assert!(Instant::now() < deadline, "the cancel never landed");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn session_on_a_real_core() -> Option<(Session, TempDir, Millis)> {
    let core = common::vendored_core()?;
    let d = common::tmp_root_with_real_carts(&["Emerald"]);
    common::write_real_cart_as(&d, "Emerald", "POKEMON EMER", "BPEE");
    let rom = d.path().join("Games/GBA").join("Emerald.gba");
    let mut bytes = std::fs::read(&rom).expect("read rom");
    bytes[0xb2] = 0;
    bytes[0xbd] = bytes[0xbd].wrapping_add(0x96);
    std::fs::write(&rom, bytes).expect("rewrite rom");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", Core::Gpsp)
        .expect("write core");
    std::fs::copy(
        &core,
        d.path()
            .join("System")
            .join(slot::core::dylib_name(Core::Gpsp)),
    )
    .expect("plant a core under gpSP's name");
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    Some((s, d, now))
}

#[test]
fn a_game_that_will_not_load_again_comes_back_out_of_the_slot() {
    let _g = common::core_lock();
    let Some((mut s, d, mut now)) = session_on_a_real_core() else {
        eprintln!("no host-openable dylib on this machine, skipping");
        return;
    };
    step(
        &mut s,
        &mut now,
        &[RawEvent::Down(Btn::Select), RawEvent::Down(Btn::Menu)],
    );
    step(
        &mut s,
        &mut now,
        &[RawEvent::Up(Btn::Menu), RawEvent::Up(Btn::Select)],
    );
    assert!(s.app().game_menu_open(), "the chord never reached the app");
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Right)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Right)]);
    step(&mut s, &mut now, &[RawEvent::Down(Btn::Select)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::Select)]);
    std::fs::remove_file(d.path().join("Games/GBA").join("Emerald.gba"))
        .expect("take the rom away");

    step(&mut s, &mut now, &[RawEvent::Down(Btn::A)]);
    let deadline = Instant::now() + BAIL;
    while !matches!(s.app().phase(), Phase::Ejecting { .. }) {
        assert!(
            Instant::now() < deadline,
            "the cart never came back out: {:?}",
            s.app().phase()
        );
        assert!(
            s.has_core() || s.app().game_menu_open(),
            "a seated cart was left playing with no core behind it"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        s.app().alert_visible(),
        "the cart came out without the alert"
    );
    assert!(
        persist::read_resume(d.path(), Platform::Gba, Core::Gpsp, "Emerald")
            .unwrap()
            .is_some(),
        "the state flushed before the reload is gone"
    );
    let deadline = Instant::now() + BAIL;
    while !matches!(s.app().phase(), Phase::Shelf) {
        assert!(
            Instant::now() < deadline,
            "the refused cart never reached the shelf"
        );
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn a_cart_gpsp_cannot_carry_is_refused_the_link_screen_and_told_why() {
    let d = common::tmp_root_with_carts(&["Apotris", "Zzz"]);
    common::write_retail_header(&d, "Apotris", "APOTRIS", "2ATE");
    let mut app = seated_on_gpsp(&d);
    app.apply(Action::GameMenu);
    assert!(
        !app.game_menu_open(),
        "a cart gpSP has no protocol for was offered a link screen"
    );
    assert_eq!(
        app.toast(),
        Some(slot_ui::Toast::NoLink),
        "the press did nothing and said nothing"
    );
    assert!(
        matches!(app.phase(), Phase::Playing { .. }),
        "the refusal took the game away: {:?}",
        app.phase()
    );
    assert!(
        !app.link_active(),
        "a session started for a cart gpSP will not link"
    );
}

#[test]
fn a_device_without_wifi_is_refused_the_link_screen_and_told_why() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_retail_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let mut app = seated_on_gpsp(&d);
    let wlan0 = d.path().join("net/wlan0");
    app.set_wifi_probe(wlan0.clone());
    app.apply(Action::GameMenu);
    assert!(
        !app.game_menu_open(),
        "a device with no radio offered a link"
    );
    assert_eq!(app.toast(), Some(Toast::NoWifi));
    assert!(matches!(app.phase(), Phase::Playing { .. }));
    std::fs::create_dir_all(&wlan0).unwrap();
    app.apply(Action::GameMenu);
    assert!(
        app.game_menu_open(),
        "wlan0 arrived and the link stayed refused"
    );
}

#[test]
fn a_cart_gpsp_carries_still_opens_the_link_screen() {
    for (stem, title, code) in [
        ("Mario Golf", "MARIO GOLF", "BMGE"),
        ("Emerald", "POKEMON EMER", "BPEE"),
        ("Advance Wars", "ADVANCEWARS", "AWRE"),
    ] {
        let d = common::tmp_root_with_carts(&["Zzz"]);
        common::write_retail_header(&d, stem, title, code);
        let mut app = seated_on_gpsp(&d);
        app.apply(Action::GameMenu);
        assert!(app.game_menu_open(), "{code} was refused its link screen");
        assert_eq!(
            app.toast(),
            None,
            "{code} opened the screen and said so too"
        );
    }
}

#[test]
fn a_cart_gpsp_cannot_link_is_refused_on_gpsp_and_carried_by_mgbas_cable() {
    let refused = |core, open: bool| {
        let d = common::tmp_root_with_carts(&["Apotris", "Zzz"]);
        common::write_retail_header(&d, "Apotris", "APOTRIS", "2ATE");
        let mut app = seated_on(&d, core);
        app.apply(Action::GameMenu);
        assert_eq!(
            app.game_menu_open(),
            open,
            "{core:?} answered Apotris with the wrong screen"
        );
        app.toast()
    };
    assert_eq!(refused(Core::Gpsp, false), Some(Toast::NoLink));
    assert_eq!(refused(Core::Mgba, true), None);
}

#[test]
fn a_wireless_adapter_cart_on_mgba_still_says_to_switch_to_gpsp() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_retail_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let mut app = seated_on(&d, Core::Mgba);
    app.apply(Action::GameMenu);
    assert!(
        !app.game_menu_open(),
        "mGBA offered a cable to a cart that talks to the Wireless Adapter"
    );
    assert_eq!(
        app.toast(),
        Some(Toast::NeedsGpsp),
        "a cart gpSP can carry was told there is no link support for it"
    );
}

#[test]
fn the_link_overlay_on_gambatte_says_to_switch_to_mgba() {
    let d = common::tmp_root_with_gb_carts(&["Pokemon Red", "Zzz"]);
    let mut app = seated_on_platform(&d, Core::Gambatte, Platform::Gb);
    app.apply(Action::GameMenu);
    assert!(!app.game_menu_open(), "Gambatte opened the link overlay");
    assert_eq!(app.toast(), Some(Toast::NeedsMgba));
}

#[test]
fn a_game_boy_cart_links_on_mgba_and_is_refused_on_gpsp() {
    for (core, open) in [(Core::Mgba, true), (Core::Gpsp, false)] {
        let d = common::tmp_root_with_gb_carts(&["Pokemon Red", "Zzz"]);
        let mut app = seated_on_platform(&d, core, Platform::Gb);
        app.apply(Action::GameMenu);
        assert_eq!(
            app.game_menu_open(),
            open,
            "{core:?} answered a Game Boy cart with the wrong screen"
        );
        assert_ne!(
            app.toast(),
            Some(Toast::NeedsGpsp),
            "{core:?} told a Game Boy cart to switch to gpSP, which cannot run it at all"
        );
        if open {
            continue;
        }
        assert_eq!(
            app.toast(),
            Some(Toast::NoLink),
            "{core:?} answered a Game Boy cart with the wrong banner"
        );
        assert!(
            matches!(app.phase(), Phase::Playing { .. }),
            "{core:?}: the refusal took the game away: {:?}",
            app.phase()
        );
        assert!(
            !app.link_active(),
            "{core:?} started a link session for a Game Boy cart"
        );
    }
}

#[test]
fn select_on_mgba_keeps_the_cable_and_the_screen_opens_again() {
    let d = common::tmp_root_with_carts(&["Kirby", "Zzz"]);
    common::write_retail_header(&d, "Kirby", "AGB KIRBY AM", "B8KE");
    let mut app = seated_on(&d, Core::Mgba);
    app.apply(Action::GameMenu);
    assert_eq!(drawn_hardware(&app), LinkKind::Cable);
    select(&mut app);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "mGBA switched to an adapter it cannot emulate"
    );
    app.apply(Action::GbaDown(Btn::B));
    app.apply(Action::GameMenu);
    assert!(
        app.game_menu_open(),
        "the link screen never opened again, #131"
    );
    assert_ne!(app.toast(), Some(Toast::NeedsGpsp));
}

#[test]
fn only_a_pokemon_cart_with_both_links_switches_on_gpsp() {
    for (title, code, switches) in [
        ("POKEMON RUBY", "AXVE", false),
        ("ADVANCEWARS", "AWRE", false),
        ("MARIO GOLF", "BMGE", false),
        ("POKEMON EMER", "BPEE", true),
        ("POKEMON FIRE", "BPRE", true),
    ] {
        let d = common::tmp_root_with_carts(&["Cart", "Zzz"]);
        common::write_retail_header(&d, "Cart", title, code);
        let mut app = seated_on_gpsp(&d);
        app.apply(Action::GameMenu);
        let before = drawn_hardware(&app);
        select(&mut app);
        assert_eq!(
            drawn_hardware(&app) != before,
            switches,
            "{code} answered SELECT the wrong way"
        );
    }
}

#[derive(Clone, Default)]
struct RadioLog(Arc<std::sync::Mutex<Vec<RadioJob>>>);

impl RadioLog {
    fn jobs(&self) -> Vec<RadioJob> {
        self.0.lock().expect("radio log").clone()
    }
}

impl RadioJobs for RadioLog {
    fn ask(&mut self, job: RadioJob) {
        self.0.lock().expect("radio log").push(job);
    }

    fn warmed(&self) -> bool {
        false
    }
}

fn watched(app: &mut App) -> RadioLog {
    let log = RadioLog::default();
    app.set_radio_jobs(Box::new(log.clone()));
    log
}

#[test]
fn opening_the_link_screen_warms_the_radio() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    let log = watched(&mut app);
    app.apply(Action::GameMenu);
    assert_eq!(log.jobs(), vec![RadioJob::Warm]);
}

#[test]
fn leaving_the_link_screen_without_a_session_cools_it() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    let log = watched(&mut app);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::B));
    assert_eq!(log.jobs(), vec![RadioJob::Warm, RadioJob::Cool]);
}

#[test]
fn a_screen_that_closes_over_a_live_session_leaves_the_radio_alone() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    let log = watched(&mut app);
    app.begin_link(0);
    app.apply(Action::GbaDown(Btn::B));
    assert!(
        !log.jobs().contains(&RadioJob::Cool),
        "cooled the radio a live session was running over"
    );
}

#[test]
fn the_shortcut_still_opens_the_screen_when_nothing_is_linked() {
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open());
    assert_eq!(app.toast(), None);
}

#[test]
fn a_join_that_hears_no_host_says_nobody_arrived() {
    let _link = common::link_port_lock();
    let (mut app, _d) = playing_on(Core::Gpsp);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    select(&mut app);
    select(&mut app);
    app.apply(Action::GbaDown(Btn::A));
    let deadline = Instant::now() + BAIL;
    while !matches!(app.game_menu(), Some(GameMenu::Failed { .. })) {
        assert!(Instant::now() < deadline, "the join never failed");
        app.update(1.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(matches!(
        app.game_menu(),
        Some(GameMenu::Failed {
            fail: LinkFail::NobodyCame,
            ..
        })
    ));
}

fn failed_face(app: &App) -> Option<TexId> {
    let mut out = Vec::new();
    app.draw(&mut out);
    out.iter().find_map(|d| match *d {
        Draw::Tex { tex, .. } if (970..990).any(|n| TexId::from_raw(n) == tex) => Some(tex),
        _ => None,
    })
}

fn dropped_on(title: &str, code: &str) -> App {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", title, code);
    let mut app = seated_on_gpsp(&d);
    app.set_link_fail_faces(
        LinkFail::SHOWN
            .iter()
            .enumerate()
            .map(|(i, _)| (TexId::from_raw(970 + i), 100, 30))
            .collect(),
        vec![None; LinkFail::SHOWN.len()],
    );
    app.set_link_fail_wireless_faces(
        LinkFail::SHOWN
            .iter()
            .enumerate()
            .map(|(i, f)| {
                (f.line(LinkKind::Wireless) != f.line(LinkKind::Cable))
                    .then(|| (TexId::from_raw(980 + i), 100, 30))
            })
            .collect(),
    );
    app.apply(Action::GameMenu);
    app.start_link(
        fake_starter(|_, _| Err(io_err(io::ErrorKind::ConnectionRefused))),
        0,
    );
    settle(&mut app);
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Failed {
                fail: LinkFail::PeerVanished,
                ..
            })
        ),
        "the link never dropped: {:?}",
        app.game_menu()
    );
    app
}

#[test]
fn a_dropped_link_names_the_hardware_it_ran_on() {
    let dropped = LinkFail::PeerVanished
        .shown()
        .expect("a dropped link has a face");
    let cable = dropped_on("POKEMON RUBY", "AXVE");
    assert_eq!(drawn_hardware(&cable), LinkKind::Cable);
    assert_eq!(
        failed_face(&cable),
        Some(TexId::from_raw(970 + dropped)),
        "a dropped cable link did not say The cable came loose"
    );
    let wireless = dropped_on("POKEMON EMER", "BPEE");
    assert_eq!(drawn_hardware(&wireless), LinkKind::Wireless);
    assert_eq!(
        failed_face(&wireless),
        Some(TexId::from_raw(980 + dropped)),
        "a dropped adapter link did not say Lost the connection"
    );
}

#[test]
fn a_joiner_draws_the_hosts_adapter_and_its_own_cable_afterwards() {
    let _link = common::link_port_lock();
    let (mut app, d) = playing_on(Core::Gpsp);
    app.set_link_sprites(fake_link_sprites());
    app.set_link_spawn(Box::new(|role, _, held| {
        LinkStarter::spawn_held_with(
            Box::new(|_, _| Ok(None)),
            Box::new(|| {}),
            role,
            0,
            Box::new(|_, cancel: &Cancel| {
                while !cancel.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(2));
                }
                Err(io_err(io::ErrorKind::Interrupted))
            }),
            held,
        )
    }));
    let ble = listening(&mut app);
    app.apply(Action::GameMenu);
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "the test cart's own header links by cable"
    );
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));
    let mut heard = common::heard_host(&d, "Emerald", 7, AP, -60);
    heard.advert.kind = LinkKind::Wireless;
    ble.hear(heard);
    let deadline = Instant::now() + BAIL;
    let reload = loop {
        if let Some(reload) = app.take_link_reload() {
            break reload;
        }
        assert!(
            Instant::now() < deadline,
            "the wireless host was never joined"
        );
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(reload, ("Emerald".to_string(), "rfu"));
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Wireless,
        "the joiner drew a plug over the reload for a host on the adapter"
    );
    app.set_link_loaded("rfu");
    app.link_reload_done();
    assert!(reaches_waiting(&mut app), "the join never started");
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Wireless,
        "the joiner drew a plug for a host on the adapter"
    );
    app.apply(Action::GbaDown(Btn::B));
    settle(&mut app);
    let deadline = Instant::now() + BAIL;
    let back = loop {
        if let Some(back) = app.take_link_reload() {
            break back;
        }
        assert!(
            Instant::now() < deadline,
            "the game never went back to its own serial"
        );
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(back, ("Emerald".to_string(), "auto"));
    app.set_link_loaded("auto");
    app.link_reload_done();
    idle(&mut app, 50);
    app.apply(Action::GameMenu);
    assert_eq!(
        app.game_menu(),
        Some(GameMenu::Pick(LinkRow::Join)),
        "the link screen did not open again"
    );
    assert_eq!(
        drawn_hardware(&app),
        LinkKind::Cable,
        "the host's adapter stayed on the joiner's own link screen"
    );
}
