use std::sync::mpsc::{channel, Receiver, TryRecvError};

use std::net::Ipv4Addr;

use crate::link_lan::{self, Advertiser, HostInfo, Multicast};
use crate::link_net::{Cancel, TcpLink, HOST_BOUND};
use crate::link_radio::{self, LinkNet, LinkRole, RadioFail};

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

/// Four sentences rather than one, deliberately: "the link failed" does not tell a player
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFail {
    Radio,
    NobodyCame,
    PeerVanished,
    Cancelled,
}

/// The socket step on the home network: the host announces itself once it is listening, the
/// joiner asks who is hosting this game and connects to whoever answers.
///
/// Announcing waits for the listener because a joiner that found the host any sooner would be
/// refused, and stops when a peer is accepted because a host that goes on advertising after it
/// has its friend is telling a third handheld to knock on a door that no longer opens.
fn lan_socket(
    role: LinkRole,
    local: Ipv4Addr,
    port: u16,
    game: &str,
    cancel: &Cancel,
) -> std::io::Result<TcpLink> {
    match role {
        LinkRole::Host => {
            // The socket is opened before anything waits, so a network that will not carry
            // multicast is an error now and not a silent thirty seconds.
            let wire = Multicast::open(local)?;
            let info = HostInfo {
                instance: link_lan::instance_name(),
                addr: local,
                port,
                game: game.to_string(),
            };
            let mut advert = None;
            let link = TcpLink::host_greeted_until(
                &local.to_string(),
                port,
                HOST_BOUND,
                cancel,
                game,
                || advert = Some(Advertiser::start_with(Box::new(wire), info)),
            );
            drop(advert);
            link
        }
        LinkRole::Join => {
            let mut wire = Multicast::open(local)?;
            link_lan::find(&mut wire, game, cancel, HOST_BOUND, |host| {
                TcpLink::join_greeted(host.into(), game)
            })
        }
    }
}

impl LinkStep {
    pub const ALL: [LinkStep; 2] = [LinkStep::Radio, LinkStep::Waiting];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn line(self) -> &'static str {
        match self {
            LinkStep::Radio => "Bringing the radio up",
            LinkStep::Waiting => "Looking for the other player",
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

    pub fn line(self) -> &'static str {
        match self {
            LinkFail::Radio => "The radio did not come up",
            LinkFail::NobodyCame => "Nobody arrived",
            LinkFail::PeerVanished => "The other player vanished",
            LinkFail::Cancelled => "Cancelled",
        }
    }
}

pub enum LinkProgress {
    At(LinkStep),
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

type RadioUp = Box<dyn FnMut(LinkRole, &Cancel) -> Result<(), RadioFail> + Send>;
/// `RadioUp` that says which network it brought up, which the socket step needs to know.
type NetUp = Box<dyn FnMut(LinkRole, &Cancel) -> Result<LinkNet, RadioFail> + Send>;
type RadioDown = Box<dyn FnMut() + Send>;
type Socket = Box<dyn FnMut(u16, &Cancel) -> std::io::Result<TcpLink> + Send>;
/// `Socket` that is told which network it is on.
type NetSocket = Box<dyn FnMut(LinkNet, u16, &Cancel) -> std::io::Result<TcpLink> + Send>;

pub struct LinkStarter {
    rx: Receiver<LinkProgress>,
    cancel: Cancel,
    done: bool,
}

impl LinkStarter {
    /// The real thing: `link_radio` for the network, `TcpLink` for the socket. `game` is the
    /// cart's header code, which on the home network is how two handhelds know they are
    /// running the same game.
    pub fn spawn(role: LinkRole, port: u16, game: &str) -> LinkStarter {
        let game = game.to_string();
        LinkStarter::spawn_net(
            Box::new(link_radio::up),
            Box::new(link_radio::down),
            role,
            port,
            Box::new(move |net, port, cancel| match net {
                LinkNet::Direct => match role {
                    LinkRole::Host => TcpLink::host_until(HOST_ADDR, port, HOST_BOUND, cancel),
                    LinkRole::Join => TcpLink::join_until(HOST_ADDR, port, HOST_BOUND, cancel),
                },
                LinkNet::Lan { local } => lan_socket(role, local, port, &game, cancel),
            }),
        )
    }

    pub fn spawn_with(
        mut radio_up: RadioUp,
        radio_down: RadioDown,
        role: LinkRole,
        port: u16,
        mut socket: Socket,
    ) -> LinkStarter {
        // Every injected radio is the private network's, which is what these were written for.
        LinkStarter::spawn_net(
            Box::new(move |role, cancel| radio_up(role, cancel).map(|()| LinkNet::Direct)),
            radio_down,
            role,
            port,
            Box::new(move |_, port, cancel| socket(port, cancel)),
        )
    }

    /// `spawn_with` for a radio that says which network it brought up, and a socket step that
    /// is told.
    pub fn spawn_net(
        mut radio_up: NetUp,
        mut radio_down: RadioDown,
        role: LinkRole,
        port: u16,
        mut socket: NetSocket,
    ) -> LinkStarter {
        let (tx, rx) = channel();
        let cancel = Cancel::new();
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let _ = tx.send(LinkProgress::At(LinkStep::Radio));
            let net = match radio_up(role, &flag) {
                Ok(net) => net,
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
            let _ = tx.send(LinkProgress::At(LinkStep::Waiting));
            eprintln!("slot: link: {role:?} using {net:?}, port {port}");
            match socket(net, port, &flag) {
                Ok(link) => {
                    eprintln!("slot: link: {role:?} connected over {net:?}");
                    if tx.send(LinkProgress::Ready(link)).is_err() {
                        eprintln!("slot: link: nobody left to hand it to, radio back down");
                        radio_down();
                    }
                }
                Err(e) => {
                    eprintln!(
                        "slot: link: {role:?} failed over {net:?}, port {port}: {e} (kind {:?})",
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
            done: false,
        }
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
        assert_eq!(
            LinkStep::Radio.shown(true).line(),
            "Looking for the other player"
        );
    }

    #[test]
    fn a_cold_radio_still_says_it_is_bringing_the_radio_up() {
        assert_eq!(LinkStep::Radio.shown(false), LinkStep::Radio);
        assert_eq!(LinkStep::Radio.shown(false).line(), "Bringing the radio up");
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
