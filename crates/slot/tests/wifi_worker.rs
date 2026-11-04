use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use slot::update::UpdateFail;
use slot::wifi::{Cmd, Failure, Fake, Wifi};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn home() -> Fake {
    Fake::new(
        vec![
            ("Cabin", -70, true),
            ("Home", -50, true),
            ("Cafe", -60, false),
        ],
        &[("Home", "password"), ("Cabin", "cabinpass")],
    )
}

fn secs(t: Instant, s: u64) -> Instant {
    t + Duration::from_secs(s)
}

fn join(ssid: &str, password: &str) -> Cmd {
    Cmd::Join {
        ssid: ssid.into(),
        password: password.into(),
        hidden: false,
    }
}

fn id_of(w: &Wifi, ssid: &str) -> u32 {
    w.snapshot()
        .saved
        .iter()
        .find(|n| n.ssid == ssid)
        .unwrap()
        .id
}

fn lines(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock().unwrap().clone()
}

fn settle(w: &mut Wifi, t: Instant, from: u64, to: u64) {
    for s in from..to {
        w.poll(secs(t, s));
    }
}

#[test]
fn a_missing_address_is_checked_every_second() {
    let _g = serial();
    let f = Fake::joined();
    let no_lease = f.no_lease.clone();
    no_lease.store(true, Ordering::SeqCst);
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.poll(t);
    assert_eq!(w.snapshot().status.state, slot::wifi::State::Completed);
    assert_eq!(w.snapshot().status.ip, None);
    no_lease.store(false, Ordering::SeqCst);
    w.poll(secs(t, 2));
    assert!(w.snapshot().status.ip.is_some());
}

#[test]
fn hold_starts_and_release_stops() {
    let _g = serial();
    let f = home();
    let running = f.running.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.poll(t);
    assert!(w.snapshot().held);
    assert!(running.load(Ordering::SeqCst));
    w.send(Cmd::Release);
    w.poll(secs(t, 1));
    assert!(!w.snapshot().held);
    assert!(!running.load(Ordering::SeqCst));
}

#[test]
fn power_off_blocks_the_radio() {
    let _g = serial();
    let f = home();
    let blocked = f.blocked.clone();
    let running = f.running.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.poll(t);
    assert!(!blocked.load(Ordering::SeqCst));
    w.send(Cmd::PowerOff);
    w.poll(secs(t, 1));
    assert!(blocked.load(Ordering::SeqCst));
    assert!(!running.load(Ordering::SeqCst));
    assert!(!w.snapshot().held);
}

#[test]
fn start_clears_the_block() {
    let _g = serial();
    let f = home();
    let blocked = f.blocked.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::PowerOff);
    w.send(Cmd::Hold);
    w.poll(t);
    assert!(!blocked.load(Ordering::SeqCst));
}

#[test]
fn a_manual_network_is_rejoined_after_a_release() {
    let _g = serial();
    let f = home();
    let log = f.log.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    w.send(join("Cabin", "cabinpass"));
    w.poll(t);
    let cabin = id_of(&w, "Cabin");
    w.send(Cmd::AutoJoin {
        id: cabin,
        on: false,
    });
    w.send(Cmd::Connect(cabin));
    settle(&mut w, t, 1, 4);
    w.send(Cmd::Release);
    w.poll(secs(t, 4));
    log.lock().unwrap().clear();
    w.send(Cmd::Hold);
    w.poll(secs(t, 5));
    let l = lines(&log);
    assert_eq!(
        l.iter().rev().find(|x| x.starts_with("connect")),
        Some(&format!("connect {cabin}"))
    );
}

#[test]
fn a_manual_network_is_rejoined_by_name_after_a_forget_renumbers() {
    let _g = serial();
    let f = home();
    let log = f.log.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    w.send(join("Cabin", "cabinpass"));
    w.poll(t);
    let cabin = id_of(&w, "Cabin");
    w.send(Cmd::AutoJoin {
        id: cabin,
        on: false,
    });
    w.send(Cmd::Connect(cabin));
    settle(&mut w, t, 1, 4);
    let home = id_of(&w, "Home");
    w.send(Cmd::Forget(home));
    w.poll(secs(t, 4));
    w.send(Cmd::Release);
    w.poll(secs(t, 5));
    log.lock().unwrap().clear();
    w.send(Cmd::Hold);
    w.poll(secs(t, 6));
    let now = id_of(&w, "Cabin");
    assert_ne!(now, cabin);
    assert_eq!(
        lines(&log).iter().rev().find(|x| x.starts_with("connect")),
        Some(&format!("connect {now}"))
    );
}

#[test]
fn a_join_with_the_radio_down_is_saved_and_says_it_could_not_connect() {
    let _g = serial();
    let mut w = Wifi::inline(Box::new(Fake::broken()));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    w.poll(t);
    let s = w.snapshot();
    assert!(s.saved.iter().any(|n| n.ssid == "Home"));
    assert_eq!(s.failure, Some(("Home".to_string(), Failure::Other)));
}

#[test]
fn a_disconnect_survives_a_release() {
    let _g = serial();
    let f = home();
    let log = f.log.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    settle(&mut w, t, 0, 3);
    w.send(Cmd::Disconnect);
    w.poll(secs(t, 3));
    w.send(Cmd::Release);
    w.poll(secs(t, 4));
    log.lock().unwrap().clear();
    w.send(Cmd::Hold);
    settle(&mut w, t, 5, 9);
    let l = lines(&log);
    assert!(l.iter().any(|x| x == "disconnect"), "{l:?}");
    assert!(!l.iter().any(|x| x.starts_with("connect")), "{l:?}");
    assert_ne!(w.snapshot().status.state, slot::wifi::State::Completed);
}

#[test]
fn a_failure_stays_until_the_next_attempt() {
    let _g = serial();
    let mut w = Wifi::inline(Box::new(home()));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "wrongpass"));
    settle(&mut w, t, 0, 6);
    let want = Some(("Home".to_string(), Failure::WrongPassword));
    assert_eq!(w.snapshot().failure, want);
    w.send(Cmd::Release);
    w.poll(secs(t, 6));
    assert_eq!(w.snapshot().failure, want);
    w.send(Cmd::Hold);
    w.poll(secs(t, 7));
    assert_eq!(w.snapshot().failure, want);
    w.send(join("Home", "password"));
    w.poll(secs(t, 8));
    assert_eq!(w.snapshot().failure, None);
}

#[test]
fn forgetting_the_manual_network_returns_to_auto() {
    let _g = serial();
    let f = home();
    let log = f.log.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    w.send(join("Cabin", "cabinpass"));
    w.poll(t);
    settle(&mut w, t, 1, 4);
    let cabin = id_of(&w, "Cabin");
    w.send(Cmd::Forget(cabin));
    w.poll(secs(t, 4));
    w.send(Cmd::Release);
    w.poll(secs(t, 5));
    log.lock().unwrap().clear();
    w.send(Cmd::Hold);
    settle(&mut w, t, 6, 9);
    let l = lines(&log);
    assert!(
        !l.iter()
            .any(|x| x.starts_with("connect") || x == "disconnect"),
        "{l:?}"
    );
}

#[test]
fn wait_online_succeeds_once_connected() {
    let _g = serial();
    let mut w = Wifi::inline(Box::new(home()));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    settle(&mut w, t, 0, 4);
    assert!(slot::wifi::online());
    assert!(slot::wifi::wait_online(Duration::from_millis(100)).is_ok());
    w.send(Cmd::Release);
    w.poll(secs(t, 5));
    assert!(!slot::wifi::online());
}

#[test]
fn wait_online_times_out_when_the_supplicant_never_answers() {
    let _g = serial();
    let mut w = Wifi::spawn(Box::new(Fake::silent()));
    w.send(Cmd::Hold);
    let t = Instant::now();
    while !w.snapshot().held && t.elapsed() < Duration::from_secs(5) {
        w.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(w.snapshot().held);
    let start = Instant::now();
    let r = slot::wifi::wait_online(Duration::from_millis(300));
    assert!(matches!(r, Err(UpdateFail::Offline)), "{r:?}");
    assert!(start.elapsed() < Duration::from_secs(3));
    w.poll(Instant::now());
    w.poll(Instant::now());
}

#[test]
fn wait_online_is_no_wifi_when_not_held() {
    let _g = serial();
    let _w = Wifi::inline(Box::new(home()));
    let r = slot::wifi::wait_online(Duration::from_millis(50));
    assert!(matches!(r, Err(UpdateFail::NoWifi)), "{r:?}");
}

#[test]
fn wait_online_waits_for_a_hold_still_in_flight() {
    let _g = serial();
    let mut w = Wifi::spawn(Box::new(home()));
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    let r = slot::wifi::wait_online(Duration::from_secs(5));
    assert!(r.is_ok(), "{r:?}");
    w.send(Cmd::Release);
}

#[test]
fn a_successful_rejoin_clears_an_old_failure() {
    let _g = serial();
    let f = home();
    let nearby = f.nearby.clone();
    let saved_nearby = nearby.lock().unwrap().clone();
    nearby.lock().unwrap().clear();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.want_scans(true);
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    settle(&mut w, t, 0, 30);
    assert_eq!(
        w.snapshot().failure,
        Some(("Home".to_string(), Failure::NotFound))
    );
    w.send(Cmd::Release);
    w.poll(secs(t, 30));
    *nearby.lock().unwrap() = saved_nearby;
    w.send(Cmd::Hold);
    settle(&mut w, t, 31, 36);
    assert_eq!(
        w.snapshot().attempt,
        Some(("Home".to_string(), slot::wifi::Phase::Connected))
    );
    assert_eq!(w.snapshot().failure, None);
}

#[test]
fn a_wrong_password_is_not_retried_after_a_release() {
    let _g = serial();
    let f = home();
    let log = f.log.clone();
    let mut w = Wifi::inline(Box::new(f));
    let t = Instant::now();
    w.send(Cmd::Hold);
    w.send(join("Home", "wrongpass"));
    settle(&mut w, t, 0, 6);
    w.send(Cmd::Release);
    w.poll(secs(t, 6));
    log.lock().unwrap().clear();
    w.send(Cmd::Hold);
    settle(&mut w, t, 7, 10);
    assert!(!lines(&log).iter().any(|x| x.starts_with("connect")));
    assert_eq!(
        w.snapshot().failure,
        Some(("Home".to_string(), Failure::WrongPassword))
    );
}

#[test]
fn auto_join_flips_the_moment_it_is_sent() {
    let _g = serial();
    let mut w = Wifi::spawn(Box::new(home()));
    w.send(Cmd::Hold);
    w.send(join("Home", "password"));
    let t = Instant::now();
    while (w.busy() || w.snapshot().saved.is_empty()) && t.elapsed() < Duration::from_secs(5) {
        w.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(5));
    }
    let id = id_of(&w, "Home");
    let auto = |w: &Wifi| {
        w.snapshot()
            .saved
            .iter()
            .find(|n| n.id == id)
            .unwrap()
            .auto_join
    };
    assert!(auto(&w));
    w.send(Cmd::AutoJoin { id, on: false });
    assert!(!auto(&w), "Off waits for the worker");
    w.poll(Instant::now());
    assert!(!auto(&w), "a report sent before the command undid it");
    w.send(Cmd::AutoJoin { id, on: true });
    assert!(auto(&w), "On waits for the worker");
    let t = Instant::now();
    while w.busy() && t.elapsed() < Duration::from_secs(5) {
        w.poll(Instant::now());
        assert!(auto(&w), "it flickered back while the worker caught up");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(auto(&w));
    w.send(Cmd::Release);
}

#[test]
fn the_mac_fake_joins_its_saved_network_when_held() {
    let _g = serial();
    let mut w = Wifi::inline(Box::new(Fake::joined()));
    let t = Instant::now();
    w.send(Cmd::Hold);
    settle(&mut w, t, 0, 3);
    let snap = w.snapshot();
    assert_eq!(snap.status.state, slot::wifi::State::Completed);
    assert_eq!(snap.status.ssid.as_deref(), Some("Home"));
    assert!(snap.status.ip.is_some());
    assert!(snap.saved.iter().any(|n| n.ssid == "Home" && n.auto_join));
    w.send(Cmd::Release);
    w.poll(secs(t, 4));
}
