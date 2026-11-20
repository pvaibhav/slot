use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

use crate::link_kind::LinkKind;
use crate::link_net::{Cancel, TcpLink, HOST_BOUND};
use crate::link_radio::{self, HostAp, RadioFail, RadioRole};

#[cfg(feature = "device")]
pub const HOST_ADDR: &str = "10.42.0.1";

#[cfg(not(feature = "device"))]
pub const HOST_ADDR: &str = "127.0.0.1";

pub const DEFAULT_LINK_PORT: u16 = 7211;

const PORT_ENV: &str = "SLOT_LINK_PORT";

pub fn link_port() -> u16 {
    let Some(raw) = std::env::var_os(PORT_ENV) else {
        return DEFAULT_LINK_PORT;
    };
    if raw.to_str().is_some_and(|s| s.trim().is_empty()) {
        return DEFAULT_LINK_PORT;
    }
    match raw.to_str().and_then(|s| s.trim().parse::<u16>().ok()) {
        None | Some(0) => {
            eprintln!(
                "slot: {PORT_ENV}={:?} is not a port a link can meet on, using {DEFAULT_LINK_PORT}",
                raw.to_string_lossy()
            );
            DEFAULT_LINK_PORT
        }
        Some(port) => port,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStep {
    Radio,
    Waiting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFail {
    Radio,
    NobodyCame,
    PeerVanished,
    Cancelled,
}

impl LinkStep {
    pub const ALL: [LinkStep; 2] = [LinkStep::Radio, LinkStep::Waiting];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn line(self) -> &'static str {
        match self {
            LinkStep::Radio => "Waiting for a friend",
            LinkStep::Waiting => "Waiting for a friend",
        }
    }

    pub fn shown(self, warm: bool) -> LinkStep {
        match self {
            LinkStep::Radio if warm => LinkStep::Waiting,
            step => step,
        }
    }
}

impl LinkFail {
    pub const SHOWN: [LinkFail; 3] = [
        LinkFail::Radio,
        LinkFail::NobodyCame,
        LinkFail::PeerVanished,
    ];

    pub fn shown(self) -> Option<usize> {
        LinkFail::SHOWN.iter().position(|f| *f == self)
    }

    pub fn line(self, kind: LinkKind) -> &'static str {
        match self {
            LinkFail::Radio => "The radio did not come up",
            LinkFail::NobodyCame => "No one joined, try again",
            LinkFail::PeerVanished if kind == LinkKind::Wireless => "Lost the connection",
            LinkFail::PeerVanished => "The cable came loose",
            LinkFail::Cancelled => "Cancelled",
        }
    }

    pub fn hint(self) -> Option<&'static str> {
        match self {
            LinkFail::Radio => Some("Check logs"),
            _ => None,
        }
    }
}

pub enum LinkProgress {
    At(LinkStep),
    Hosting(HostAp),
    Ready(TcpLink),
    Failed(LinkFail),
}

fn classify(e: &std::io::Error) -> LinkFail {
    match e.kind() {
        std::io::ErrorKind::Interrupted => LinkFail::Cancelled,
        std::io::ErrorKind::TimedOut => LinkFail::NobodyCame,
        _ => LinkFail::PeerVanished,
    }
}

type RadioUp = Box<dyn FnMut(RadioRole, &Cancel) -> Result<Option<HostAp>, RadioFail> + Send>;
type RadioDown = Box<dyn FnMut() + Send>;
type Socket = Box<dyn FnMut(u16, &Cancel) -> std::io::Result<TcpLink> + Send>;

pub struct LinkStarter {
    rx: Receiver<LinkProgress>,
    cancel: Cancel,
    go: Arc<AtomicBool>,
    done: bool,
}

impl LinkStarter {
    pub fn spawn(role: RadioRole, port: u16) -> LinkStarter {
        LinkStarter::spawn_held(role, port, false)
    }

    pub fn spawn_held(role: RadioRole, port: u16, held: bool) -> LinkStarter {
        LinkStarter::spawn_held_with(
            Box::new(link_radio::up),
            Box::new(link_radio::down),
            role,
            port,
            Box::new(move |port, cancel| match role {
                RadioRole::Host => TcpLink::host_until(HOST_ADDR, port, HOST_BOUND, cancel),
                RadioRole::Join(_) => TcpLink::join_until(HOST_ADDR, port, HOST_BOUND, cancel),
            }),
            held,
        )
    }

    pub fn spawn_with(
        radio_up: RadioUp,
        radio_down: RadioDown,
        role: RadioRole,
        port: u16,
        socket: Socket,
    ) -> LinkStarter {
        LinkStarter::spawn_held_with(radio_up, radio_down, role, port, socket, false)
    }

    pub fn spawn_held_with(
        mut radio_up: RadioUp,
        mut radio_down: RadioDown,
        role: RadioRole,
        port: u16,
        mut socket: Socket,
        held: bool,
    ) -> LinkStarter {
        let (tx, rx) = channel();
        let cancel = Cancel::new();
        let flag = cancel.clone();
        let go = Arc::new(AtomicBool::new(!held));
        let gate = go.clone();
        std::thread::spawn(move || {
            let _ = tx.send(LinkProgress::At(LinkStep::Radio));
            let hosted = match radio_up(role, &flag) {
                Ok(hosted) => hosted,
                Err(e) => {
                    let fail = match &e {
                        RadioFail::NoHost => LinkFail::NobodyCame,
                        RadioFail::Cancelled => LinkFail::Cancelled,
                        RadioFail::Radio(why) => {
                            eprintln!("slot: link: {role:?} could not bring the radio up: {why}");
                            LinkFail::Radio
                        }
                    };
                    radio_down();
                    let _ = tx.send(LinkProgress::Failed(fail));
                    return;
                }
            };
            if let Some(ap) = hosted {
                let _ = tx.send(LinkProgress::Hosting(ap));
            }
            if !gate.load(Ordering::SeqCst) {
                eprintln!(
                    "slot: link: {role:?} radio up at {:.0} ms, held for go",
                    crate::boot_time::ms()
                );
                while !gate.load(Ordering::SeqCst) {
                    if flag.is_cancelled() {
                        radio_down();
                        let _ = tx.send(LinkProgress::Failed(LinkFail::Cancelled));
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            let _ = tx.send(LinkProgress::At(LinkStep::Waiting));
            eprintln!("slot: link: {role:?} using {HOST_ADDR}:{port}");
            match socket(port, &flag) {
                Ok(link) => {
                    eprintln!("slot: link: {role:?} connected on {HOST_ADDR}:{port}");
                    if tx.send(LinkProgress::Ready(link)).is_err() {
                        eprintln!("slot: link: nobody left to hand it to, radio back down");
                        radio_down();
                    }
                }
                Err(e) => {
                    eprintln!(
                        "slot: link: {role:?} failed on {HOST_ADDR}:{port}: {e} (kind {:?})",
                        e.kind()
                    );
                    radio_down();
                    let _ = tx.send(LinkProgress::Failed(classify(&e)));
                }
            }
        });
        LinkStarter {
            rx,
            cancel,
            go,
            done: false,
        }
    }

    pub fn go(&self) {
        self.go.store(true, Ordering::SeqCst);
    }

    pub fn without_target() -> LinkStarter {
        LinkStarter::spawn_with(
            Box::new(|_, _| Err(RadioFail::NoHost)),
            Box::new(|| {}),
            RadioRole::Host,
            0,
            Box::new(|_, _| Err(std::io::Error::other("no target"))),
        )
    }

    pub fn poll(&mut self) -> Option<LinkProgress> {
        if self.done {
            return None;
        }
        match self.rx.try_recv() {
            Ok(progress) => {
                self.done = matches!(progress, LinkProgress::Ready(_) | LinkProgress::Failed(_));
                Some(progress)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.done = true;
                Some(LinkProgress::Failed(LinkFail::PeerVanished))
            }
        }
    }

    pub fn cancel(&mut self) {
        self.cancel.cancel();
    }
}

impl Drop for LinkStarter {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

#[cfg(test)]
mod port_tests {
    use super::*;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn unset_means_the_number_both_devices_already_agree_on() {
        let _g = lock();
        std::env::remove_var(PORT_ENV);
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
    }

    #[test]
    fn a_port_in_the_environment_wins() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "7300");
        assert_eq!(link_port(), 7300);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn a_padded_port_is_still_a_port() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "  7300 ");
        assert_eq!(link_port(), 7300);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn a_value_that_is_not_a_port_falls_back() {
        let _g = lock();
        for bad in ["banana", "-1", "70000", "7300x"] {
            std::env::set_var(PORT_ENV, bad);
            assert_eq!(
                link_port(),
                DEFAULT_LINK_PORT,
                "{bad:?} should not be taken"
            );
        }
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn an_empty_value_is_an_unset_value() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "   ");
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
        std::env::remove_var(PORT_ENV);
    }

    #[test]
    fn zero_is_refused_even_though_it_parses() {
        let _g = lock();
        std::env::set_var(PORT_ENV, "0");
        assert_eq!(link_port(), DEFAULT_LINK_PORT);
        std::env::remove_var(PORT_ENV);
    }
}

#[cfg(test)]
mod step_tests {
    use super::*;

    #[test]
    fn a_warm_radio_captions_the_first_step_as_the_search() {
        assert_eq!(LinkStep::Radio.shown(true), LinkStep::Waiting);
        assert_eq!(LinkStep::Radio.shown(true).line(), "Waiting for a friend");
    }

    #[test]
    fn a_cold_radio_keeps_its_own_step_and_waits_for_a_friend_too() {
        assert_eq!(LinkStep::Radio.shown(false), LinkStep::Radio);
        assert_eq!(LinkStep::Radio.shown(false).line(), "Waiting for a friend");
    }

    #[test]
    fn a_dropped_link_names_the_hardware_it_ran_on() {
        assert_eq!(
            LinkFail::PeerVanished.line(LinkKind::Cable),
            "The cable came loose"
        );
        assert_eq!(
            LinkFail::PeerVanished.line(LinkKind::Wireless),
            "Lost the connection"
        );
        for fail in [LinkFail::Radio, LinkFail::NobodyCame, LinkFail::Cancelled] {
            assert_eq!(fail.line(LinkKind::Cable), fail.line(LinkKind::Wireless));
        }
    }

    #[test]
    fn only_a_radio_failure_points_at_the_logs() {
        assert_eq!(
            LinkFail::Radio.line(LinkKind::Cable),
            "The radio did not come up"
        );
        assert_eq!(LinkFail::Radio.hint(), Some("Check logs"));
        assert_eq!(LinkFail::NobodyCame.hint(), None);
        assert_eq!(LinkFail::PeerVanished.hint(), None);
    }

    #[test]
    fn the_socket_step_says_the_same_thing_in_both_states() {
        assert_eq!(LinkStep::Waiting.shown(true), LinkStep::Waiting);
        assert_eq!(LinkStep::Waiting.shown(false), LinkStep::Waiting);
    }

    #[test]
    fn every_sentence_a_step_can_show_is_still_one_of_the_faces() {
        for warm in [true, false] {
            for step in LinkStep::ALL {
                assert!(
                    LinkStep::ALL.contains(&step.shown(warm)),
                    "{step:?} at warm={warm} shows a sentence with no face uploaded for it"
                );
            }
        }
    }
}
