use std::ffi::CStr;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::os::fd::FromRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
const NAME: [&str; 2] = ["slot", "local"];
const MAX_JUMPS: usize = 8;
const MAX_QUESTIONS: usize = 16;
const TTL: u32 = 120;
const LEGACY_TTL: u32 = 10;
const POLL: Duration = Duration::from_millis(100);

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
    if cfg!(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm"))) {
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

fn classify(query: &[u8]) -> Option<(Kind, u16, u16)> {
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
        let name_ok = labels.len() == NAME.len() && labels.iter().zip(NAME).all(|(a, b)| a == b);
        if !name_ok || qclass & 0x7fff != 1 {
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

fn name_bytes(out: &mut Vec<u8>) {
    for l in NAME {
        out.push(l.len() as u8);
        out.extend_from_slice(l.as_bytes());
    }
    out.push(0);
}

fn build(query: &[u8], ip: Ipv4Addr, legacy: bool) -> Option<Vec<u8>> {
    let (kind, qtype, qclass) = classify(query)?;
    let (class, ttl): (u16, u32) = if legacy { (1, LEGACY_TTL) } else { (0x8001, TTL) };
    let mut out = Vec::with_capacity(64);
    if legacy {
        out.extend_from_slice(&query[0..2]);
    } else {
        out.extend_from_slice(&[0, 0]);
    }
    out.extend_from_slice(&[0x84, 0, 0, if legacy { 1 } else { 0 }, 0, 1, 0, 0, 0, 0]);
    if legacy {
        name_bytes(&mut out);
        out.extend_from_slice(&qtype.to_be_bytes());
        out.extend_from_slice(&(qclass & 0x7fff).to_be_bytes());
    }
    name_bytes(&mut out);
    match kind {
        Kind::A => {
            out.extend_from_slice(&1u16.to_be_bytes());
            out.extend_from_slice(&class.to_be_bytes());
            out.extend_from_slice(&ttl.to_be_bytes());
            out.extend_from_slice(&[0, 4]);
            out.extend_from_slice(&ip.octets());
        }
        Kind::Nsec => {
            out.extend_from_slice(&47u16.to_be_bytes());
            out.extend_from_slice(&class.to_be_bytes());
            out.extend_from_slice(&ttl.to_be_bytes());
            out.extend_from_slice(&[0, 15]);
            name_bytes(&mut out);
            out.extend_from_slice(&[0, 1, 0x40]);
        }
    }
    Some(out)
}

pub fn answer(query: &[u8], ip: Ipv4Addr) -> Option<Vec<u8>> {
    build(query, ip, false)
}

pub fn respond(query: &[u8], ip: Ipv4Addr, src: SocketAddrV4) -> Option<(Vec<u8>, SocketAddrV4)> {
    if src.port() == MDNS_PORT {
        build(query, ip, false).map(|r| (r, SocketAddrV4::new(GROUP, MDNS_PORT)))
    } else {
        build(query, ip, true).map(|r| (r, src))
    }
}

fn setsockopt<T>(fd: i32, level: i32, name: i32, value: &T) -> io::Result<()> {
    let r = unsafe {
        libc::setsockopt(fd, level, name, value as *const T as *const libc::c_void, std::mem::size_of::<T>() as libc::socklen_t)
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
        libc::bind(fd, &addr as *const libc::sockaddr_in as *const libc::sockaddr, std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t)
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    let iface = libc::in_addr { s_addr: u32::from(ip).to_be() };
    setsockopt(fd, libc::IPPROTO_IP, libc::IP_MULTICAST_IF, &iface)?;
    sock.join_multicast_v4(&GROUP, &ip)?;
    sock.set_multicast_ttl_v4(255)?;
    sock.set_read_timeout(Some(POLL))?;
    Ok(sock)
}

pub struct Responder {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Responder {
    pub fn start(ip: Ipv4Addr) -> Option<Responder> {
        let sock = open(ip).ok()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = thread::Builder::new().name("slot-mdns".into()).spawn(move || serve(sock, ip, flag)).ok()?;
        Some(Responder { stop, thread: Some(thread) })
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

fn serve(sock: UdpSocket, ip: Ipv4Addr, stop: Arc<AtomicBool>) {
    let mut buf = [0u8; 1500];
    while !stop.load(Ordering::SeqCst) {
        match sock.recv_from(&mut buf) {
            Ok((n, SocketAddr::V4(src))) => {
                if let Some((reply, to)) = respond(&buf[..n], ip, src) {
                    let _ = sock.send_to(&reply, to);
                }
            }
            Ok(_) => {}
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {}
            Err(_) => thread::sleep(POLL),
        }
    }
}
