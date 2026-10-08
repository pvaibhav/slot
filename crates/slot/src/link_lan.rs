//! Finding the other handheld on the home network, so a link does not need a network of its own.
//!
//! The host announces itself as `<instance>._slotlink._tcp.local` and answers anyone asking; the
//! joiner asks once a second and connects to whoever answers for the same game. What goes on the
//! wire is `link_mdns`. This module is the sockets and the timing, with the packet I/O behind a
//! trait so the search and the announcing can be tested without a network.
//!
//! BaseOS's own Avahi shares UDP 5353 with this and does not mind: it publishes the hostname
//! and knows nothing of this service, and both sides ignore what they do not recognise. None of
//! it depends on Avahi running, which the player can turn off.

use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::link_mdns::{self, Announce};
use crate::link_net::Cancel;

const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const PORT: u16 = 5353;
/// How often a joiner asks again. mDNS asks less and less often; a search that lasts thirty
/// seconds on a link the player is waiting on can afford to ask every second.
const ASK_EVERY: Duration = Duration::from_secs(1);
/// How long a host that did not take a connection is left alone before it is tried again.
/// Long enough not to hammer it, short enough to catch a host that was not listening yet.
const RETRY_AFTER: Duration = Duration::from_secs(2);
/// How long one look at the socket lasts, and so how late a cancel can be noticed.
const LOOK: Duration = Duration::from_millis(100);
/// The fewest a host waits between answers to questions, however many arrive.
const ANSWER_GAP: Duration = Duration::from_millis(200);

/// A place to put mDNS packets and take them from.
pub trait Wire {
    fn send(&mut self, packet: &[u8]);
    /// The next packet, or `None` if nothing arrived within `wait`.
    fn recv(&mut self, wait: Duration) -> Option<Vec<u8>>;
}

/// The real thing: UDP 5353 joined to the group on the interface that owns `local`.
pub struct Multicast {
    sock: UdpSocket,
}

fn check(rc: libc::c_int) -> io::Result<()> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn set<T>(fd: libc::c_int, level: libc::c_int, name: libc::c_int, value: &T) -> io::Result<()> {
    check(unsafe {
        libc::setsockopt(
            fd,
            level,
            name,
            value as *const T as *const libc::c_void,
            std::mem::size_of::<T>() as libc::socklen_t,
        )
    })
}

impl Multicast {
    /// Every socket option here is one mDNS asks of a responder that shares its port: address
    /// and port reuse so Avahi and this can both listen, membership on the one interface the
    /// link runs over, and a hop limit of 255 as the RFC says (a packet that has been routed
    /// anywhere did not come from the local link).
    pub fn open(local: Ipv4Addr) -> io::Result<Multicast> {
        let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        check(fd)?;
        // From here the fd is owned by `sock`, so an error below closes it.
        let sock = unsafe { UdpSocket::from_raw_fd(fd) };
        let on: libc::c_int = 1;
        set(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, &on)?;
        set(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, &on)?;
        let bind = libc::sockaddr_in {
            sin_family: libc::AF_INET as libc::sa_family_t,
            sin_port: PORT.to_be(),
            sin_addr: libc::in_addr { s_addr: 0 },
            sin_zero: [0; 8],
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
            sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        };
        check(unsafe {
            libc::bind(
                fd,
                &bind as *const libc::sockaddr_in as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
            )
        })?;
        let member = libc::ip_mreq {
            imr_multiaddr: libc::in_addr {
                s_addr: u32::from(GROUP).to_be(),
            },
            imr_interface: libc::in_addr {
                s_addr: u32::from(local).to_be(),
            },
        };
        set(fd, libc::IPPROTO_IP, libc::IP_ADD_MEMBERSHIP, &member)?;
        let out = libc::in_addr {
            s_addr: u32::from(local).to_be(),
        };
        set(fd, libc::IPPROTO_IP, libc::IP_MULTICAST_IF, &out)?;
        let hops: libc::c_int = 255;
        set(fd, libc::IPPROTO_IP, libc::IP_MULTICAST_TTL, &hops)?;
        Ok(Multicast { sock })
    }
}

impl Wire for Multicast {
    fn send(&mut self, packet: &[u8]) {
        // Best effort: a lost question is asked again in a second, a lost answer on the next
        // question.
        let _ = self.sock.send_to(packet, SocketAddrV4::new(GROUP, PORT));
    }

    fn recv(&mut self, wait: Duration) -> Option<Vec<u8>> {
        self.sock.set_read_timeout(Some(wait.max(LOOK / 10))).ok()?;
        let mut buf = [0u8; 1500];
        let n = self.sock.recv(&mut buf).ok()?;
        Some(buf[..n].to_vec())
    }
}

/// Announces a host until dropped, then says goodbye.
pub struct Advertiser {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// What a host says about itself.
#[derive(Clone)]
pub struct HostInfo {
    pub instance: String,
    pub addr: Ipv4Addr,
    pub port: u16,
    pub game: String,
}

impl Advertiser {
    pub fn start(local: Ipv4Addr, info: HostInfo) -> io::Result<Advertiser> {
        Ok(Advertiser::start_with(
            Box::new(Multicast::open(local)?),
            info,
        ))
    }

    pub fn start_with(mut wire: Box<dyn Wire + Send>, info: HostInfo) -> Advertiser {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::spawn(move || answer(wire.as_mut(), &info, &flag));
        Advertiser {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Advertiser {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn packet(info: &HostInfo, ttl: u32) -> Vec<u8> {
    link_mdns::response(&Announce {
        instance: &info.instance,
        port: info.port,
        addr: info.addr,
        game: &info.game,
        ttl,
    })
}

/// Announce, answer questions until told to stop, then say goodbye. Twice at the start and
/// twice at the end, because one lost datagram is enough to make either half unreliable.
fn answer(wire: &mut dyn Wire, info: &HostInfo, stop: &AtomicBool) {
    let up = packet(info, link_mdns::TTL_SECS);
    wire.send(&up);
    let mut repeat = Some(Instant::now() + Duration::from_millis(250));
    let mut last: Option<Instant> = None;
    while !stop.load(Ordering::SeqCst) {
        let now = Instant::now();
        if repeat.is_some_and(|t| now >= t) {
            wire.send(&up);
            repeat = None;
        }
        if let Some(p) = wire.recv(LOOK) {
            if link_mdns::asks_for_hosts(&p)
                && last.is_none_or(|t| now.duration_since(t) >= ANSWER_GAP)
            {
                wire.send(&up);
                last = Some(now);
            }
        }
    }
    let bye = packet(info, 0);
    wire.send(&bye);
    wire.send(&bye);
}

/// Ask until a host for `game` answers and `connect` gets through to it, the player gives up,
/// or `bound` passes: `Interrupted` for the first, `TimedOut` for the last, the same two kinds
/// `TcpLink` uses so the screen above says the same thing for both transports.
///
/// Hosts are tried in the order they answered. One that does not take the connection is left
/// alone for a moment and then tried again, since it may only have been slow to listen; the
/// others are not held up for it.
pub fn find<T>(
    wire: &mut dyn Wire,
    game: &str,
    cancel: &Cancel,
    bound: Duration,
    mut connect: impl FnMut(SocketAddrV4) -> io::Result<T>,
) -> io::Result<T> {
    let deadline = Instant::now() + bound;
    let mut next_ask = Instant::now();
    let mut tried: HashMap<SocketAddrV4, Instant> = HashMap::new();
    loop {
        if cancel.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "cancelled while looking for a host",
            ));
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "no host answered on this network",
            ));
        }
        if now >= next_ask {
            wire.send(&link_mdns::query());
            next_ask = now + ASK_EVERY;
        }
        let Some(p) = wire.recv(LOOK.min(deadline.saturating_duration_since(now))) else {
            continue;
        };
        for host in link_mdns::hosts(&p) {
            if host.game != game {
                continue;
            }
            let now = Instant::now();
            if tried
                .get(&host.addr)
                .is_some_and(|t| now.duration_since(*t) < RETRY_AFTER)
            {
                continue;
            }
            tried.insert(host.addr, now);
            if let Ok(t) = connect(host.addr) {
                return Ok(t);
            }
            if cancel.is_cancelled() {
                break;
            }
        }
    }
}

/// A name for this host that no other on the network is likely to share, and that says only
/// what it needs to. Not probed for, as mDNS asks: a clash needs two handhelds hosting the same
/// game at once on a network that is not the player's own to have picked the same four digits.
pub fn instance_name() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mix = nanos ^ std::process::id().rotate_left(16);
    format!("slot-{:04x}", (mix ^ (mix >> 16)) & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// A scripted network: packets to hand out in order, and everything sent.
    struct Fake {
        inbox: Vec<Vec<u8>>,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl Wire for Fake {
        fn send(&mut self, packet: &[u8]) {
            self.sent.lock().unwrap().push(packet.to_vec());
        }
        fn recv(&mut self, wait: Duration) -> Option<Vec<u8>> {
            if self.inbox.is_empty() {
                std::thread::sleep(wait.min(Duration::from_millis(10)));
                None
            } else {
                Some(self.inbox.remove(0))
            }
        }
    }

    fn host(port: u16, last: u8, game: &str) -> Vec<u8> {
        link_mdns::response(&Announce {
            instance: &format!("slot-{last:02x}"),
            port,
            addr: Ipv4Addr::new(192, 168, 1, last),
            game,
            ttl: link_mdns::TTL_SECS,
        })
    }

    fn fake(inbox: Vec<Vec<u8>>) -> (Fake, Arc<Mutex<Vec<Vec<u8>>>>) {
        let sent = Arc::new(Mutex::new(Vec::new()));
        (
            Fake {
                inbox,
                sent: sent.clone(),
            },
            sent,
        )
    }

    #[test]
    fn the_first_host_that_takes_the_connection_wins() {
        let (mut wire, sent) = fake(vec![host(7211, 5, "AMAE"), host(7211, 6, "AMAE")]);
        let got = find(
            &mut wire,
            "AMAE",
            &Cancel::new(),
            Duration::from_secs(5),
            |a| Ok(*a.ip()),
        )
        .unwrap();
        assert_eq!(got, Ipv4Addr::new(192, 168, 1, 5));
        // It asked before anyone answered.
        assert_eq!(sent.lock().unwrap()[0], link_mdns::query());
    }

    #[test]
    fn hosts_are_tried_in_the_order_they_answered_and_a_refusal_moves_on() {
        let (mut wire, _) = fake(vec![
            host(7211, 5, "AMAE"),
            host(7211, 6, "AMAE"),
            host(7211, 7, "AMAE"),
        ]);
        let mut order = Vec::new();
        let got = find(
            &mut wire,
            "AMAE",
            &Cancel::new(),
            Duration::from_secs(5),
            |a| {
                order.push(a.ip().octets()[3]);
                if a.ip().octets()[3] == 7 {
                    Ok(a)
                } else {
                    Err(io::Error::from(io::ErrorKind::ConnectionRefused))
                }
            },
        )
        .unwrap();
        assert_eq!(got.ip().octets()[3], 7);
        assert_eq!(order, vec![5, 6, 7]);
    }

    #[test]
    fn a_host_running_another_game_is_never_tried() {
        let (mut wire, _) = fake(vec![host(7211, 5, "ADVE"), host(7211, 6, "AMAE")]);
        let mut tried = Vec::new();
        let _ = find(
            &mut wire,
            "AMAE",
            &Cancel::new(),
            Duration::from_secs(5),
            |a| {
                tried.push(a.ip().octets()[3]);
                Ok(())
            },
        );
        assert_eq!(tried, vec![6]);
    }

    #[test]
    fn nobody_answering_is_nobody_arriving() {
        let (mut wire, _) = fake(Vec::new());
        let err = find(
            &mut wire,
            "AMAE",
            &Cancel::new(),
            Duration::from_millis(300),
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn a_cancel_is_heard_within_a_look() {
        let (mut wire, _) = fake(Vec::new());
        let cancel = Cancel::new();
        cancel.cancel();
        let started = Instant::now();
        let err = find(&mut wire, "AMAE", &cancel, Duration::from_secs(30), |_| {
            Ok(())
        })
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_host_that_refused_is_tried_again_after_a_pause_not_at_once() {
        let mut inbox = vec![host(7211, 5, "AMAE"); 3];
        inbox.push(host(7211, 5, "AMAE"));
        let (mut wire, _) = fake(inbox);
        let mut attempts = 0;
        let _ = find(
            &mut wire,
            "AMAE",
            &Cancel::new(),
            Duration::from_millis(400),
            |_| -> io::Result<()> {
                attempts += 1;
                Err(io::Error::from(io::ErrorKind::ConnectionRefused))
            },
        );
        assert_eq!(attempts, 1, "answers inside the pause must not be retried");
    }

    #[test]
    fn a_host_announces_answers_and_says_goodbye() {
        let sent = Arc::new(Mutex::new(Vec::new()));
        // One question, then nothing.
        let wire = Fake {
            inbox: vec![link_mdns::query()],
            sent: sent.clone(),
        };
        let info = HostInfo {
            instance: "slot-1234".into(),
            addr: Ipv4Addr::new(192, 168, 1, 9),
            port: 7211,
            game: "AMAE".into(),
        };
        let adv = Advertiser::start_with(Box::new(wire), info);
        std::thread::sleep(Duration::from_millis(500));
        drop(adv);
        let sent = sent.lock().unwrap();
        let named: Vec<usize> = sent.iter().map(|p| link_mdns::hosts(p).len()).collect();
        // Announce, repeat, one answer to the question, then two goodbyes, which name nobody.
        assert!(named.iter().filter(|n| **n == 1).count() >= 3, "{named:?}");
        assert_eq!(&named[named.len() - 2..], &[0, 0], "{named:?}");
    }

    #[test]
    fn instance_names_are_one_short_label() {
        let n = instance_name();
        assert!(
            n.starts_with("slot-") && n.len() == 9 && !n.contains('.'),
            "{n}"
        );
    }
}
