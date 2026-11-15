use std::ffi::CStr;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
const NAME: &str = "slot";
const DOMAIN: &str = "local";
const MAX_JUMPS: usize = 8;
const MAX_QUESTIONS: usize = 16;
const TTL: u32 = 120;
const LEGACY_TTL: u32 = 10;
const POLL: Duration = Duration::from_millis(100);
const PROBE_WAIT: Duration = Duration::from_millis(250);
const PROBES: usize = 3;
const LOST_WAIT: Duration = Duration::from_secs(1);
const LOST_TRIES: usize = 3;
const ANNOUNCE_GAP: Duration = Duration::from_secs(1);
const MAX_SUFFIX: u32 = 9;
const BASEOS_CONF: &str = "/run/baseos.conf";

pub fn wifi_ip() -> Option<Ipv4Addr> {
    let mut head: *mut libc::ifaddrs = std::ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return None;
    }
    let mut wlan = None;
    let mut other = None;
    let mut cur = head;
    while !cur.is_null() {
        let ifa = unsafe { &*cur };
        cur = ifa.ifa_next;
        let up = ifa.ifa_flags & libc::IFF_UP as u32 != 0;
        let loopback = ifa.ifa_flags & libc::IFF_LOOPBACK as u32 != 0;
        if !up || loopback || ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
            continue;
        }
        if unsafe { (*ifa.ifa_addr).sa_family } as i32 != libc::AF_INET {
            continue;
        }
        let sin = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
        let ip = Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
        if ip.is_unspecified() || ip.is_loopback() {
            continue;
        }
        let name = unsafe { CStr::from_ptr(ifa.ifa_name) };
        if name.to_bytes() == b"wlan0" {
            wlan = Some(ip);
            break;
        }
        if other.is_none() {
            other = Some(ip);
        }
    }
    unsafe { libc::freeifaddrs(head) };
    if cfg!(all(
        target_os = "linux",
        any(target_arch = "aarch64", target_arch = "arm")
    )) {
        wlan
    } else {
        wlan.or(other)
    }
}

fn read_name(packet: &[u8], start: usize) -> Option<(Vec<String>, usize)> {
    let mut labels = Vec::new();
    let mut pos = start;
    let mut end = None;
    let mut jumps = 0;
    let mut total = 0;
    loop {
        let len = *packet.get(pos)? as usize;
        match len & 0xc0 {
            0 => {
                if len == 0 {
                    return Some((labels, end.unwrap_or(pos + 1)));
                }
                let raw = packet.get(pos + 1..pos + 1 + len)?;
                total += len + 1;
                if total > 255 {
                    return None;
                }
                labels.push(String::from_utf8_lossy(raw).to_ascii_lowercase());
                pos += 1 + len;
            }
            0xc0 => {
                let low = *packet.get(pos + 1)? as usize;
                if end.is_none() {
                    end = Some(pos + 2);
                }
                jumps += 1;
                if jumps > MAX_JUMPS {
                    return None;
                }
                pos = ((len & 0x3f) << 8) | low;
            }
            _ => return None,
        }
    }
}

fn u16_at(packet: &[u8], pos: usize) -> Option<u16> {
    let b = packet.get(pos..pos + 2)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

enum Kind {
    A,
    Nsec,
}

fn named(labels: &[String], name: &str) -> bool {
    matches!(labels, [host, domain] if host == name && domain == DOMAIN)
}

fn classify(query: &[u8], name: &str) -> Option<(Kind, u16, u16)> {
    let flags = u16_at(query, 2)?;
    if flags & 0x8000 != 0 || (flags >> 11) & 0xf != 0 {
        return None;
    }
    let count = u16_at(query, 4)? as usize;
    if count == 0 || count > MAX_QUESTIONS {
        return None;
    }
    let mut pos = 12;
    let mut found: Option<(Kind, u16, u16)> = None;
    for _ in 0..count {
        let (labels, next) = read_name(query, pos)?;
        let qtype = u16_at(query, next)?;
        let qclass = u16_at(query, next + 2)?;
        pos = next + 4;
        if !named(&labels, name) || qclass & 0x7fff != 1 {
            continue;
        }
        match qtype {
            1 | 255 => {
                found = Some((Kind::A, qtype, qclass));
            }
            28 if found.is_none() => {
                found = Some((Kind::Nsec, qtype, qclass));
            }
            _ => {}
        }
    }
    found
}

fn name_bytes(out: &mut Vec<u8>, name: &str) {
    for l in [name, DOMAIN] {
        out.push(l.len() as u8);
        out.extend_from_slice(l.as_bytes());
    }
    out.push(0);
}

fn build(query: &[u8], ip: Ipv4Addr, name: &str, legacy: bool) -> Option<Vec<u8>> {
    let (kind, qtype, qclass) = classify(query, name)?;
    let (class, ttl): (u16, u32) = if legacy {
        (1, LEGACY_TTL)
    } else {
        (0x8001, TTL)
    };
    let mut out = Vec::with_capacity(64);
    if legacy {
        out.extend_from_slice(&query[0..2]);
    } else {
        out.extend_from_slice(&[0, 0]);
    }
    out.extend_from_slice(&[0x84, 0, 0, if legacy { 1 } else { 0 }, 0, 1, 0, 0, 0, 0]);
    if legacy {
        name_bytes(&mut out, name);
        out.extend_from_slice(&qtype.to_be_bytes());
        out.extend_from_slice(&(qclass & 0x7fff).to_be_bytes());
    }
    name_bytes(&mut out, name);
    match kind {
        Kind::A => {
            out.extend_from_slice(&1u16.to_be_bytes());
            out.extend_from_slice(&class.to_be_bytes());
            out.extend_from_slice(&ttl.to_be_bytes());
            out.extend_from_slice(&[0, 4]);
            out.extend_from_slice(&ip.octets());
        }
        Kind::Nsec => {
            let mut next = Vec::new();
            name_bytes(&mut next, name);
            out.extend_from_slice(&47u16.to_be_bytes());
            out.extend_from_slice(&class.to_be_bytes());
            out.extend_from_slice(&ttl.to_be_bytes());
            out.extend_from_slice(&(next.len() as u16 + 3).to_be_bytes());
            out.extend_from_slice(&next);
            out.extend_from_slice(&[0, 1, 0x40]);
        }
    }
    Some(out)
}

pub fn answer(query: &[u8], ip: Ipv4Addr, name: &str) -> Option<Vec<u8>> {
    build(query, ip, name, false)
}

pub fn respond(
    query: &[u8],
    ip: Ipv4Addr,
    name: &str,
    src: SocketAddrV4,
) -> Option<(Vec<u8>, SocketAddrV4)> {
    if src.port() == MDNS_PORT {
        build(query, ip, name, false).map(|r| (r, SocketAddrV4::new(GROUP, MDNS_PORT)))
    } else {
        build(query, ip, name, true).map(|r| (r, src))
    }
}

pub fn model_name(model: &str) -> Option<String> {
    let first = model.split('/').next()?;
    let name: String = first
        .split_whitespace()
        .collect::<String>()
        .to_ascii_lowercase();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric())).then_some(name)
}

pub fn names() -> impl Iterator<Item = String> {
    std::iter::once(NAME.to_string()).chain((2..=MAX_SUFFIX).map(|n| format!("{NAME}-{n}")))
}

pub fn mdns_off(conf: &str) -> bool {
    conf.lines()
        .filter_map(|line| line.trim().split_once('='))
        .any(|(key, value)| key.trim() == "mdns" && value.trim() == "false")
}

fn a_record(out: &mut Vec<u8>, name: &str, class: u16, ttl: u32, ip: Ipv4Addr) {
    name_bytes(out, name);
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&class.to_be_bytes());
    out.extend_from_slice(&ttl.to_be_bytes());
    out.extend_from_slice(&[0, 4]);
    out.extend_from_slice(&ip.octets());
}

pub fn probe(name: &str, ip: Ipv4Addr) -> Vec<u8> {
    let mut out = vec![0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0];
    name_bytes(&mut out, name);
    out.extend_from_slice(&[0, 255, 0, 1]);
    a_record(&mut out, name, 1, TTL, ip);
    out
}

pub fn announcement(name: &str, ip: Ipv4Addr, ttl: u32) -> Vec<u8> {
    let mut out = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    a_record(&mut out, name, 0x8001, ttl, ip);
    out
}

struct Record<'a> {
    labels: Vec<String>,
    rtype: u16,
    rdata: &'a [u8],
}

struct Sections<'a> {
    flags: u16,
    authority: Vec<Record<'a>>,
    claims: Vec<Record<'a>>,
}

fn records(packet: &[u8]) -> Option<Sections<'_>> {
    let flags = u16_at(packet, 2)?;
    let mut pos = 12;
    for _ in 0..u16_at(packet, 4)? {
        pos = read_name(packet, pos)?.1 + 4;
    }
    let mut read = |count: u16| -> Option<Vec<Record<'_>>> {
        let mut out = Vec::new();
        for _ in 0..count {
            let (labels, next) = read_name(packet, pos)?;
            let rtype = u16_at(packet, next)?;
            let len = u16_at(packet, next + 8)? as usize;
            let rdata = packet.get(next + 10..next + 10 + len)?;
            pos = next + 10 + len;
            out.push(Record {
                labels,
                rtype,
                rdata,
            });
        }
        Some(out)
    };
    let mut claims = read(u16_at(packet, 6)?)?;
    let authority = read(u16_at(packet, 8)?)?;
    claims.extend(read(u16_at(packet, 10)?)?);
    Some(Sections {
        flags,
        authority,
        claims,
    })
}

pub fn claims(reply: &[u8], name: &str) -> Option<bool> {
    let s = records(reply)?;
    Some(s.flags & 0x8000 != 0 && s.claims.iter().any(|r| named(&r.labels, name)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    Nothing,
    Taken,
    Lost,
}

pub fn heard(packet: &[u8], from: SocketAddr, ip: Ipv4Addr, name: &str) -> Heard {
    if from.ip() == ip {
        return Heard::Nothing;
    }
    let Some(s) = records(packet) else {
        return Heard::Nothing;
    };
    if s.flags & 0x8000 != 0 {
        return match s.claims.iter().any(|r| named(&r.labels, name)) {
            true => Heard::Taken,
            false => Heard::Nothing,
        };
    }
    let theirs = s
        .authority
        .iter()
        .filter(|r| named(&r.labels, name) && r.rtype == 1)
        .map(|r| r.rdata)
        .max();
    match theirs {
        Some(theirs) if theirs > &ip.octets()[..] => Heard::Lost,
        _ => Heard::Nothing,
    }
}

pub fn conflicts(packet: &[u8], from: SocketAddr, ip: Ipv4Addr, name: &str) -> bool {
    if from.ip() == ip {
        return false;
    }
    let Some(s) = records(packet) else {
        return false;
    };
    s.flags & 0x8000 != 0
        && s.claims
            .iter()
            .any(|r| named(&r.labels, name) && r.rtype == 1 && r.rdata != ip.octets())
}

fn probe_once(sock: &UdpSocket, ip: Ipv4Addr, name: &str) -> Heard {
    let mut buf = [0u8; 1500];
    for _ in 0..PROBES {
        let _ = sock.send_to(&probe(name, ip), SocketAddrV4::new(GROUP, MDNS_PORT));
        let until = Instant::now() + PROBE_WAIT;
        while Instant::now() < until {
            if let Ok((n, from)) = sock.recv_from(&mut buf) {
                match heard(&buf[..n], from, ip, name) {
                    Heard::Nothing => {}
                    other => return other,
                }
            }
        }
    }
    Heard::Nothing
}

fn claim(sock: &UdpSocket, ip: Ipv4Addr, skip: usize) -> Option<(usize, String)> {
    for (at, name) in names().enumerate().skip(skip) {
        for _ in 0..LOST_TRIES {
            match probe_once(sock, ip, &name) {
                Heard::Nothing => {
                    let _ = sock.send_to(
                        &announcement(&name, ip, TTL),
                        SocketAddrV4::new(GROUP, MDNS_PORT),
                    );
                    return Some((at, name));
                }
                Heard::Taken => break,
                Heard::Lost => thread::sleep(LOST_WAIT),
            }
        }
    }
    None
}

fn setsockopt<T>(fd: i32, level: i32, name: i32, value: &T) -> io::Result<()> {
    let r = unsafe {
        libc::setsockopt(
            fd,
            level,
            name,
            value as *const T as *const libc::c_void,
            std::mem::size_of::<T>() as libc::socklen_t,
        )
    };
    if r == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn open(ip: Ipv4Addr) -> io::Result<UdpSocket> {
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) };
    let sock = unsafe { UdpSocket::from_raw_fd(fd) };
    let one: libc::c_int = 1;
    setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEADDR, &one)?;
    setsockopt(fd, libc::SOL_SOCKET, libc::SO_REUSEPORT, &one)?;
    let addr = libc::sockaddr_in {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        sin_len: std::mem::size_of::<libc::sockaddr_in>() as u8,
        sin_family: libc::AF_INET as libc::sa_family_t,
        sin_port: MDNS_PORT.to_be(),
        sin_addr: libc::in_addr { s_addr: 0 },
        sin_zero: [0; 8],
    };
    let r = unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    let iface = libc::in_addr {
        s_addr: u32::from(ip).to_be(),
    };
    setsockopt(fd, libc::IPPROTO_IP, libc::IP_MULTICAST_IF, &iface)?;
    sock.join_multicast_v4(&GROUP, &ip)?;
    sock.set_multicast_ttl_v4(255)?;
    sock.set_read_timeout(Some(POLL))?;
    Ok(sock)
}

pub struct Responder {
    name: Arc<Mutex<String>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Responder {
    pub fn start(ip: Ipv4Addr) -> Option<Responder> {
        let conf = std::fs::read_to_string(BASEOS_CONF).unwrap_or_default();
        Self::start_with(ip, mdns_off(&conf))
    }

    pub fn start_with(ip: Ipv4Addr, off: bool) -> Option<Responder> {
        if off {
            return None;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let sock = open(ip).ok()?;
        let (at, name) = claim(&sock, ip, 0)?;
        let name = Arc::new(Mutex::new(name));
        let (flag, served) = (stop.clone(), name.clone());
        let thread = thread::Builder::new()
            .name("slot-mdns".into())
            .spawn(move || serve(sock, ip, served, at, flag))
            .ok()?;
        Some(Responder {
            name,
            stop,
            thread: Some(thread),
        })
    }

    #[cfg(test)]
    pub(crate) fn named(name: &str) -> Responder {
        Responder {
            name: Arc::new(Mutex::new(name.into())),
            stop: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn rename(&self, name: &str) {
        *self.name.lock().unwrap() = name.into();
    }

    pub fn host(&self) -> String {
        let name = self.name.lock().unwrap_or_else(|e| e.into_inner());
        format!("{name}.{DOMAIN}").to_uppercase()
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn serve(
    sock: UdpSocket,
    ip: Ipv4Addr,
    name: Arc<Mutex<String>>,
    mut at: usize,
    stop: Arc<AtomicBool>,
) {
    let group = SocketAddrV4::new(GROUP, MDNS_PORT);
    let mut current = name.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let mut announce_at = Some(Instant::now() + ANNOUNCE_GAP);
    let mut buf = [0u8; 1500];
    while !stop.load(Ordering::SeqCst) {
        match sock.recv_from(&mut buf) {
            Ok((n, from)) if conflicts(&buf[..n], from, ip, &current) => {
                let _ = sock.send_to(&announcement(&current, ip, 0), group);
                let Some((next, renamed)) = claim(&sock, ip, at + 1) else {
                    return;
                };
                at = next;
                current = renamed;
                *name.lock().unwrap_or_else(|e| e.into_inner()) = current.clone();
                announce_at = Some(Instant::now() + ANNOUNCE_GAP);
            }
            Ok((n, SocketAddr::V4(src))) => {
                if let Some((reply, to)) = respond(&buf[..n], ip, &current, src) {
                    let _ = sock.send_to(&reply, to);
                }
            }
            Ok(_) => {}
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => thread::sleep(POLL),
        }
        if announce_at.is_some_and(|t| Instant::now() >= t) {
            let _ = sock.send_to(&announcement(&current, ip, TTL), group);
            announce_at = None;
        }
    }
    let _ = sock.send_to(&announcement(&current, ip, 0), group);
}
