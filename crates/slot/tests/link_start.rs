use std::io;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot::link_net::{Cancel, TcpLink};
use slot::link_radio::{LinkNet, LinkRole, RadioFail};
use slot::link_start::{LinkFail, LinkProgress, LinkStarter, LinkStep};

const BAIL: Duration = Duration::from_secs(5);

fn drain(starter: &mut LinkStarter) -> LinkProgress {
    let deadline = Instant::now() + BAIL;
    loop {
        match starter.poll() {
            Some(LinkProgress::At(_)) => {}
            Some(outcome) => return outcome,
            None => std::thread::sleep(Duration::from_millis(2)),
        }
        assert!(
            Instant::now() < deadline,
            "the worker never reached an outcome"
        );
    }
}

fn drain_steps(starter: &mut LinkStarter) -> Vec<LinkStep> {
    let deadline = Instant::now() + BAIL;
    let mut steps = Vec::new();
    loop {
        match starter.poll() {
            Some(LinkProgress::At(step)) => steps.push(step),
            Some(_) => return steps,
            None => std::thread::sleep(Duration::from_millis(2)),
        }
        assert!(
            Instant::now() < deadline,
            "the worker never reached an outcome"
        );
    }
}

/// The network the radio step brings up is what the socket step is told, and a device already on
/// the home network has no radio to take down when the session ends the ordinary way.
#[test]
fn the_socket_step_is_told_which_network_the_radio_step_chose() {
    let lan = LinkNet::Lan {
        local: std::net::Ipv4Addr::new(192, 168, 1, 24),
    };
    let seen = Arc::new(std::sync::Mutex::new(None));
    let record = seen.clone();
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let mut starter = LinkStarter::spawn_net(
        Box::new(move |_, _| Ok(lan)),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Join,
        0,
        Box::new(move |net, _, _| {
            *record.lock().unwrap() = Some(net);
            Err(io::Error::from(io::ErrorKind::TimedOut))
        }),
    );
    let outcome = drain(&mut starter);
    assert!(matches!(
        outcome,
        LinkProgress::Failed(LinkFail::NobodyCame)
    ));
    assert_eq!(*seen.lock().unwrap(), Some(lan));
    // A failure tears down whichever network it was on, so a service that took a lease has it
    // released; the direct path's own test asserts the count for that side.
    assert_eq!(downs.load(Ordering::SeqCst), 1);
}

#[test]
fn a_direct_radio_still_reaches_the_socket_as_direct() {
    let seen = Arc::new(std::sync::Mutex::new(None));
    let record = seen.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(move |_, _| {
            *record.lock().unwrap() = Some(());
            Err(io::Error::other("stop here"))
        }),
    );
    let _ = drain(&mut starter);
    assert!(seen.lock().unwrap().is_some());
}

#[test]
fn the_services_answer_to_link_lan_is_an_address_or_nothing() {
    use slot::link_radio::parse_lan_reply;
    assert_eq!(
        parse_lan_reply("0 192.168.34.81\n"),
        Some(std::net::Ipv4Addr::new(192, 168, 34, 81))
    );
    assert_eq!(parse_lan_reply("1 NO_HOME_LAN"), None);
    assert_eq!(parse_lan_reply("0 not-an-address"), None);
    assert_eq!(parse_lan_reply("0"), None);
    assert_eq!(parse_lan_reply(""), None);
}

#[test]
fn a_radio_that_will_not_come_up_stops_before_the_socket() {
    let tried_socket = Arc::new(AtomicBool::new(false));
    let seen = tried_socket.clone();
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_role, _| Err(RadioFail::Radio("no ap".into()))),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        0,
        Box::new(move |_, _| {
            seen.store(true, Ordering::SeqCst);
            Err(io::Error::other("must not be reached"))
        }),
    );
    let outcome = drain(&mut starter);
    assert!(matches!(outcome, LinkProgress::Failed(LinkFail::Radio)));
    assert!(
        !tried_socket.load(Ordering::SeqCst),
        "opened a socket on a network that never came up"
    );
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a radio that only half came up must still be taken down"
    );
}

#[test]
fn a_failure_always_takes_the_radio_back_down() {
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::TimedOut, "nobody"))),
    );
    let outcome = drain(&mut starter);
    assert!(matches!(
        outcome,
        LinkProgress::Failed(LinkFail::NobodyCame)
    ));
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a failed link must never leave the radio up"
    );
    assert!(
        starter.poll().is_none(),
        "the worker said how it ended; there is nothing after that"
    );
}

#[test]
fn a_link_that_comes_up_leaves_the_radio_up() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let peer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        TcpLink::join("127.0.0.1", port)
    });
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Host,
        port,
        Box::new(|port, cancel: &Cancel| {
            TcpLink::host_until("127.0.0.1", port, Duration::from_secs(10), cancel)
        }),
    );
    let outcome = drain(&mut starter);
    assert!(
        matches!(outcome, LinkProgress::Ready(_)),
        "the peer arrived inside the bound, so this is a link"
    );
    let _joiner = peer.join().unwrap().expect("joiner connected");
    assert_eq!(
        downs.load(Ordering::SeqCst),
        0,
        "tearing the radio down on success kills the session it was brought up for"
    );
    assert!(
        starter.poll().is_none(),
        "a worker that has handed over its link must not then invent a failure"
    );
}

#[test]
fn the_steps_are_reported_in_order_before_the_outcome() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::TimedOut, "nobody"))),
    );
    let steps = drain_steps(&mut starter);
    assert_eq!(steps, vec![LinkStep::Radio, LinkStep::Waiting]);
}

#[test]
fn cancelling_reports_cancelled_rather_than_a_timeout() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, cancel: &Cancel| {
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        }),
    );
    starter.cancel();
    let outcome = drain(&mut starter);
    assert!(
        matches!(outcome, LinkProgress::Failed(LinkFail::Cancelled)),
        "a player who backed out is not a player nobody joined"
    );
}

#[test]
fn a_socket_fault_that_is_neither_a_deadline_nor_a_cancel_blames_the_wire() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(|_, _| Err(io::Error::new(io::ErrorKind::ConnectionRefused, "refused"))),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::PeerVanished)
    ));
}

#[test]
fn a_worker_that_dies_is_reported_rather_than_polled_forever() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| panic!("the radio call blew up")),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(|_, _| Err(io::Error::other("must not be reached"))),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::PeerVanished)
    ));
}

#[test]
fn a_link_that_comes_up_after_the_player_left_puts_the_radio_back() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let peer = std::thread::spawn(move || {
        let accepted = listener.accept();
        std::thread::sleep(Duration::from_millis(300));
        drop(accepted);
    });

    let downs = Arc::new(AtomicUsize::new(0));
    let count = downs.clone();
    let gate = Arc::new(AtomicBool::new(false));
    let open = gate.clone();

    let starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        }),
        LinkRole::Join,
        port,
        Box::new(move |p, _| {
            while !open.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            TcpLink::join("127.0.0.1", p)
        }),
    );

    drop(starter);
    gate.store(true, Ordering::SeqCst);

    let deadline = Instant::now() + Duration::from_secs(5);
    while downs.load(Ordering::SeqCst) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        downs.load(Ordering::SeqCst),
        1,
        "a link nobody was left to receive must still put the radio back"
    );
    let _ = peer.join();
}

#[test]
fn a_join_that_found_no_host_says_nobody_arrived() {
    let tried_socket = Arc::new(AtomicBool::new(false));
    let seen = tried_socket.clone();
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, _| Err(RadioFail::NoHost)),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(move |_, _| {
            seen.store(true, Ordering::SeqCst);
            Err(io::Error::other("must not be reached"))
        }),
    );
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::NobodyCame)
    ));
    assert!(
        !tried_socket.load(Ordering::SeqCst),
        "there was no host to reach, so there is nothing to open a socket to"
    );
}

#[test]
fn a_cancel_while_the_radio_is_coming_up_is_not_a_fault() {
    let mut starter = LinkStarter::spawn_with(
        Box::new(|_, cancel: &Cancel| {
            while !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(RadioFail::Cancelled)
        }),
        Box::new(|| {}),
        LinkRole::Join,
        0,
        Box::new(|_, _| Err(io::Error::other("must not be reached"))),
    );
    starter.cancel();
    assert!(matches!(
        drain(&mut starter),
        LinkProgress::Failed(LinkFail::Cancelled)
    ));
}

#[test]
fn a_starter_that_is_dropped_stops_waiting() {
    let gave_up = Arc::new(AtomicBool::new(false));
    let noticed = gave_up.clone();
    let starter = LinkStarter::spawn_with(
        Box::new(|_, _| Ok(())),
        Box::new(|| {}),
        LinkRole::Host,
        0,
        Box::new(move |_, cancel| {
            let deadline = Instant::now() + BAIL;
            while !cancel.is_cancelled() {
                assert!(
                    Instant::now() < deadline,
                    "the dropped starter was never told to give up"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
            noticed.store(true, Ordering::SeqCst);
            Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
        }),
    );

    drop(starter);

    let deadline = Instant::now() + BAIL;
    while !gave_up.load(Ordering::SeqCst) {
        assert!(
            Instant::now() < deadline,
            "the worker kept waiting for a peer nobody was left to play with"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
}
