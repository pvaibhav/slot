use std::ffi::CStr;
use std::net::{Ipv4Addr, UdpSocket};
use std::os::fd::AsRawFd;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GROUP: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;
const PTR: u16 = 12;
const MAX_JUMPS: usize = 8;
const ASK_WAIT: Duration = Duration::from_millis(1000);

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

pub fn model_name(model: &str) -> Option<String> {
    let first = model.split('/').next()?;
    let name: String = first.split_whitespace().collect::<String>().to_ascii_lowercase();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric())).then_some(name)
}

pub fn reverse_query(ip: Ipv4Addr, id: u16) -> Vec<u8> {
    let mut q = id.to_be_bytes().to_vec();
    q.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    let [a, b, c, d] = ip.octets();
    for label in [d.to_string(), c.to_string(), b.to_string(), a.to_string(), "in-addr".into(), "arpa".into()] {
        q.push(label.len() as u8);
        q.extend_from_slice(label.as_bytes());
    }
    q.extend_from_slice(&[0, 0, PTR as u8, 0, 1]);
    q
}

fn u16_at(packet: &[u8], pos: usize) -> Option<u16> {
    let b = packet.get(pos..pos + 2)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

fn read_name(packet: &[u8], start: usize) -> Option<(Vec<String>, usize)> {
    let mut labels = Vec::new();
    let mut pos = start;
    let mut end = None;
    let mut jumps = 0;
    loop {
        let len = *packet.get(pos)? as usize;
        match len & 0xc0 {
            0 if len == 0 => return Some((labels, end.unwrap_or(pos + 1))),
            0 => {
                labels.push(String::from_utf8_lossy(packet.get(pos + 1..pos + 1 + len)?).into_owned());
                pos += 1 + len;
            }
            0xc0 => {
                end.get_or_insert(pos + 2);
                jumps += 1;
                if jumps > MAX_JUMPS {
                    return None;
                }
                pos = ((len & 0x3f) << 8) | *packet.get(pos + 1)? as usize;
            }
            _ => return None,
        }
    }
}

pub fn ptr_name(reply: &[u8], id: u16) -> Option<String> {
    if u16_at(reply, 0)? != id || u16_at(reply, 2)? & 0x8000 == 0 {
        return None;
    }
    let mut pos = 12;
    for _ in 0..u16_at(reply, 4)? {
        pos = read_name(reply, pos)?.1 + 4;
    }
    for _ in 0..u16_at(reply, 6)? {
        let at = read_name(reply, pos)?.1;
        let data = at + 10;
        if u16_at(reply, at)? == PTR {
            let labels = read_name(reply, data)?.0;
            return (!labels.is_empty()).then(|| labels.join(".").to_uppercase());
        }
        pos = data + u16_at(reply, at + 8)? as usize;
    }
    None
}

pub fn local_host(ip: Ipv4Addr) -> Option<String> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    let id = (nanos >> 8) as u16;
    let sock = UdpSocket::bind((ip, 0)).ok()?;
    let iface = libc::in_addr { s_addr: u32::from(ip).to_be() };
    let set = unsafe {
        libc::setsockopt(
            sock.as_raw_fd(),
            libc::IPPROTO_IP,
            libc::IP_MULTICAST_IF,
            &iface as *const libc::in_addr as *const libc::c_void,
            std::mem::size_of::<libc::in_addr>() as libc::socklen_t,
        )
    };
    if set != 0 {
        return None;
    }
    sock.send_to(&reverse_query(ip, id), (GROUP, MDNS_PORT)).ok()?;
    let until = Instant::now() + ASK_WAIT;
    let mut buf = [0u8; 1500];
    while let Some(left) = until.checked_duration_since(Instant::now()).filter(|d| !d.is_zero()) {
        sock.set_read_timeout(Some(left)).ok()?;
        let n = sock.recv(&mut buf).ok()?;
        if let Some(name) = ptr_name(&buf[..n], id) {
            return Some(name);
        }
    }
    None
}
