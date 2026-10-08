//! The one piece of DNS-SD a home-network link needs: announcing a host as
//! `<instance>._slotlink._tcp.local` and recognising one.
//!
//! BaseOS ships an Avahi that only publishes `<hostname>.local`: no service records, no client
//! library and nothing to browse with, so slot speaks the three record types it needs itself.
//! This is only the wire format. The sockets are in `link_lan`, and nothing here touches the
//! network, which is what lets every byte of it be tested.
//!
//! Everything read here came off a shared network from whoever is on it, so every read is
//! bounds-checked and a packet that does not parse is dropped, never trusted and never a panic.

use std::net::{Ipv4Addr, SocketAddrV4};

/// What a host advertises and a joiner asks for.
pub const SERVICE: &str = "_slotlink._tcp.local";
/// The TXT `v` this build speaks. A host advertising another is not one this build can join.
pub const VERSION: &str = "1";
/// How long a host's records may be cached. Short: a host is only listening for as long as its
/// screen is up, and the goodbye on the way out is best effort.
pub const TTL_SECS: u32 = 120;

const TYPE_A: u16 = 1;
const TYPE_PTR: u16 = 12;
const TYPE_TXT: u16 = 16;
const TYPE_SRV: u16 = 33;
const TYPE_ANY: u16 = 255;
const CLASS_IN: u16 = 1;
/// mDNS's "this record is the whole truth about its name" bit, set on the unique records.
const CACHE_FLUSH: u16 = 0x8000;
/// A response, authoritative: the flags every mDNS answer carries.
const FLAGS_RESPONSE: u16 = 0x8400;
/// Pointers to earlier names are followed at most this many times, so a loop in a hostile
/// packet ends instead of spinning.
const MAX_JUMPS: usize = 16;
const MAX_NAME: usize = 255;

/// One host, as it appears on the wire.
pub struct Announce<'a> {
    /// The host's own name, one DNS label, unique on the network.
    pub instance: &'a str,
    pub port: u16,
    pub addr: Ipv4Addr,
    /// The cart's header code. Joiners only consider hosts running the same game.
    pub game: &'a str,
    /// `TTL_SECS` to announce, 0 to say goodbye.
    pub ttl: u32,
}

/// A host seen on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub instance: String,
    pub addr: SocketAddrV4,
    pub game: String,
}

fn put_name(out: &mut Vec<u8>, name: &str) {
    for label in name.split('.').filter(|l| !l.is_empty()) {
        let bytes = label.as_bytes();
        let len = bytes.len().min(63);
        out.push(len as u8);
        out.extend_from_slice(&bytes[..len]);
    }
    out.push(0);
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn put_record(out: &mut Vec<u8>, name: &str, kind: u16, class: u16, ttl: u32, data: &[u8]) {
    put_name(out, name);
    put_u16(out, kind);
    put_u16(out, class);
    out.extend_from_slice(&ttl.to_be_bytes());
    put_u16(out, data.len() as u16);
    out.extend_from_slice(data);
}

fn header(out: &mut Vec<u8>, flags: u16, questions: u16, answers: u16, additional: u16) {
    put_u16(out, 0);
    put_u16(out, flags);
    put_u16(out, questions);
    put_u16(out, answers);
    put_u16(out, 0);
    put_u16(out, additional);
}

/// The question a joiner repeats: who is hosting?
pub fn query() -> Vec<u8> {
    let mut out = Vec::new();
    header(&mut out, 0, 1, 0, 0);
    put_name(&mut out, SERVICE);
    put_u16(&mut out, TYPE_PTR);
    put_u16(&mut out, CLASS_IN);
    out
}

/// A host's answer: the pointer from the service to the instance in the answer section, and
/// the instance's SRV, TXT and address alongside it so one packet is enough to connect.
pub fn response(a: &Announce) -> Vec<u8> {
    let full = format!("{}.{SERVICE}", a.instance);
    let target = format!("{}.local", a.instance);
    let mut out = Vec::new();
    header(&mut out, FLAGS_RESPONSE, 0, 1, 3);
    let mut ptr = Vec::new();
    put_name(&mut ptr, &full);
    put_record(&mut out, SERVICE, TYPE_PTR, CLASS_IN, a.ttl, &ptr);
    let mut srv = vec![0, 0, 0, 0];
    srv.extend_from_slice(&a.port.to_be_bytes());
    put_name(&mut srv, &target);
    put_record(
        &mut out,
        &full,
        TYPE_SRV,
        CLASS_IN | CACHE_FLUSH,
        a.ttl,
        &srv,
    );
    let mut txt = Vec::new();
    for s in [format!("v={VERSION}"), format!("game={}", a.game)] {
        txt.push(s.len().min(255) as u8);
        txt.extend_from_slice(&s.as_bytes()[..s.len().min(255)]);
    }
    put_record(
        &mut out,
        &full,
        TYPE_TXT,
        CLASS_IN | CACHE_FLUSH,
        a.ttl,
        &txt,
    );
    put_record(
        &mut out,
        &target,
        TYPE_A,
        CLASS_IN | CACHE_FLUSH,
        a.ttl,
        &a.addr.octets(),
    );
    out
}

/// A bounds-checked reader over one packet.
struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let b = *self.buf.get(self.at)?;
        self.at += 1;
        Some(b)
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes([self.u8()?, self.u8()?]))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes([
            self.u8()?,
            self.u8()?,
            self.u8()?,
            self.u8()?,
        ]))
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.buf.get(self.at..end)?;
        self.at = end;
        Some(s)
    }
    /// A possibly compressed name, lower-cased, leaving the cursor after where it began.
    fn name(&mut self) -> Option<String> {
        let mut name = String::new();
        let mut at = self.at;
        let mut resume = None;
        let mut jumps = 0;
        loop {
            let len = *self.buf.get(at)? as usize;
            at += 1;
            match len {
                0 => break,
                l if l & 0xc0 == 0xc0 => {
                    let low = *self.buf.get(at)? as usize;
                    at += 1;
                    resume.get_or_insert(at);
                    jumps += 1;
                    if jumps > MAX_JUMPS {
                        return None;
                    }
                    at = ((l & 0x3f) << 8) | low;
                }
                l if l & 0xc0 != 0 => return None,
                l => {
                    let label = self.buf.get(at..at.checked_add(l)?)?;
                    at += l;
                    if !name.is_empty() {
                        name.push('.');
                    }
                    name.push_str(&String::from_utf8_lossy(label).to_ascii_lowercase());
                    if name.len() > MAX_NAME {
                        return None;
                    }
                }
            }
        }
        self.at = resume.unwrap_or(at);
        Some(name)
    }
}

struct Record {
    name: String,
    kind: u16,
    ttl: u32,
    data: Vec<u8>,
    /// Where `data` began in the packet, for names that point back into it.
    at: usize,
}

/// A packet's header flags, the questions it asks and the records it carries.
struct Packet {
    flags: u16,
    questions: Vec<(String, u16)>,
    records: Vec<Record>,
}

fn records(buf: &[u8]) -> Option<Packet> {
    let mut r = Reader { buf, at: 0 };
    r.u16()?;
    let flags = r.u16()?;
    let (qd, an, ns, ar) = (r.u16()?, r.u16()?, r.u16()?, r.u16()?);
    let mut questions = Vec::new();
    for _ in 0..qd {
        let name = r.name()?;
        let kind = r.u16()?;
        r.u16()?;
        questions.push((name, kind));
    }
    let mut out = Vec::new();
    for _ in 0..(an as usize + ns as usize + ar as usize) {
        let name = r.name()?;
        let kind = r.u16()?;
        r.u16()?;
        let ttl = r.u32()?;
        let len = r.u16()? as usize;
        let at = r.at;
        let data = r.take(len)?.to_vec();
        out.push(Record {
            name,
            kind,
            ttl,
            data,
            at,
        });
    }
    Some(Packet {
        flags,
        questions,
        records: out,
    })
}

/// Whether this packet is someone asking who is hosting, which is all a host answers.
pub fn asks_for_hosts(buf: &[u8]) -> bool {
    match records(buf) {
        Some(p) => {
            p.flags & 0x8000 == 0
                && p.questions
                    .iter()
                    .any(|(name, kind)| name == SERVICE && [TYPE_PTR, TYPE_ANY].contains(kind))
        }
        None => false,
    }
}

/// The complete hosts in a packet: a pointer, and the SRV, TXT and address it leads to. A
/// host missing any of them is left out rather than guessed at, as is one saying goodbye, one
/// speaking a version this build does not, or one that is not on the address it advertises.
pub fn hosts(buf: &[u8]) -> Vec<Found> {
    let Some(Packet {
        flags,
        records: recs,
        ..
    }) = records(buf)
    else {
        return Vec::new();
    };
    if flags & 0x8000 == 0 {
        return Vec::new();
    }
    let mut found = Vec::new();
    for ptr in recs
        .iter()
        .filter(|r| r.kind == TYPE_PTR && r.name == SERVICE && r.ttl > 0)
    {
        let mut nr = Reader { buf, at: ptr.at };
        let Some(instance) = nr.name() else { continue };
        let Some(label) = instance.strip_suffix(&format!(".{SERVICE}")) else {
            continue;
        };
        let srv = recs
            .iter()
            .find(|r| r.kind == TYPE_SRV && r.name == instance && r.ttl > 0);
        let txt = recs
            .iter()
            .find(|r| r.kind == TYPE_TXT && r.name == instance && r.ttl > 0);
        let (Some(srv), Some(txt)) = (srv, txt) else {
            continue;
        };
        if srv.data.len() < 7 {
            continue;
        }
        let port = u16::from_be_bytes([srv.data[4], srv.data[5]]);
        let mut tr = Reader {
            buf,
            at: srv.at + 6,
        };
        let Some(target) = tr.name() else { continue };
        let Some(addr) = recs
            .iter()
            .find(|r| r.kind == TYPE_A && r.name == target && r.ttl > 0 && r.data.len() == 4)
            .map(|r| Ipv4Addr::new(r.data[0], r.data[1], r.data[2], r.data[3]))
        else {
            continue;
        };
        let (mut version, mut game) = (None, None);
        let mut at = 0;
        while at < txt.data.len() {
            let len = txt.data[at] as usize;
            let Some(s) = txt.data.get(at + 1..at + 1 + len) else {
                break;
            };
            at += 1 + len;
            let s = String::from_utf8_lossy(s);
            match s.split_once('=') {
                Some(("v", v)) => version = Some(v.to_string()),
                Some(("game", g)) => game = Some(g.to_string()),
                _ => {}
            }
        }
        if version.as_deref() != Some(VERSION) || port == 0 || addr.is_unspecified() {
            continue;
        }
        found.push(Found {
            instance: label.to_string(),
            addr: SocketAddrV4::new(addr, port),
            game: game.unwrap_or_default(),
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: usize = 12;

    fn announce(ttl: u32) -> Vec<u8> {
        response(&Announce {
            instance: "slot-a1b2",
            port: 7211,
            addr: Ipv4Addr::new(192, 168, 34, 81),
            game: "AMAE",
            ttl,
        })
    }

    #[test]
    fn a_host_survives_the_round_trip() {
        assert_eq!(
            hosts(&announce(TTL_SECS)),
            vec![Found {
                instance: "slot-a1b2".into(),
                addr: SocketAddrV4::new(Ipv4Addr::new(192, 168, 34, 81), 7211),
                game: "AMAE".into(),
            }]
        );
    }

    #[test]
    fn a_goodbye_names_nobody() {
        assert!(hosts(&announce(0)).is_empty());
    }

    #[test]
    fn only_a_question_for_the_service_is_a_question_for_hosts() {
        assert!(asks_for_hosts(&query()));
        assert!(!asks_for_hosts(&announce(TTL_SECS)));
        // Someone else's service, asked the same way.
        let mut other = Vec::new();
        header(&mut other, 0, 1, 0, 0);
        put_name(&mut other, "_airplay._tcp.local");
        put_u16(&mut other, TYPE_PTR);
        put_u16(&mut other, CLASS_IN);
        assert!(!asks_for_hosts(&other));
    }

    #[test]
    fn a_query_is_not_a_host() {
        assert!(hosts(&query()).is_empty());
    }

    #[test]
    fn names_compare_without_regard_to_case() {
        // Labels are length-prefixed on the wire, so shout the one that names the service.
        let mut p = announce(TTL_SECS);
        let label = b"_slotlink";
        let at = p.windows(label.len()).position(|w| w == label).unwrap();
        p[at..at + label.len()].copy_from_slice(b"_SLOTLINK");
        assert_eq!(hosts(&p).len(), 1);
    }

    #[test]
    fn a_host_missing_a_record_is_left_out() {
        // Same packet with the address record's type changed to something else.
        let mut p = announce(TTL_SECS);
        // The last record is the address: its type sits type + class + ttl + length + data
        // (2 + 2 + 4 + 2 + 4) bytes from the end.
        let at = p.len() - 14;
        p[at + 1] = TYPE_TXT as u8;
        assert!(hosts(&p).is_empty());
    }

    #[test]
    fn another_version_is_not_joinable() {
        let mut p = announce(TTL_SECS);
        let at = p.windows(3).position(|w| w == b"v=1").unwrap();
        p[at + 2] = b'2';
        assert!(hosts(&p).is_empty());
    }

    #[test]
    fn a_host_that_claims_no_port_or_address_is_left_out() {
        let none = response(&Announce {
            instance: "slot-a1b2",
            port: 0,
            addr: Ipv4Addr::new(192, 168, 34, 81),
            game: "AMAE",
            ttl: TTL_SECS,
        });
        assert!(hosts(&none).is_empty());
        let unspecified = response(&Announce {
            instance: "slot-a1b2",
            port: 7211,
            addr: Ipv4Addr::UNSPECIFIED,
            game: "AMAE",
            ttl: TTL_SECS,
        });
        assert!(hosts(&unspecified).is_empty());
    }

    /// Names as other responders write them: the instance and the target as pointers back to
    /// where the service name already appeared.
    #[test]
    fn compressed_names_are_followed() {
        let mut p = Vec::new();
        header(&mut p, FLAGS_RESPONSE, 0, 1, 3);
        let service_at = p.len();
        put_name(&mut p, SERVICE);
        put_u16(&mut p, TYPE_PTR);
        put_u16(&mut p, CLASS_IN);
        p.extend_from_slice(&TTL_SECS.to_be_bytes());
        // Rdata: the label "host1" then a pointer to the service name.
        let rdata_at = p.len() + 2;
        put_u16(&mut p, 1 + 5 + 2);
        p.push(5);
        p.extend_from_slice(b"host1");
        p.extend_from_slice(&(0xc000u16 | service_at as u16).to_be_bytes());
        // Owner names of the rest are pointers to that instance name.
        let instance_ptr = (0xc000u16 | (rdata_at as u16)).to_be_bytes();
        let target_at = p.len() + 2 + 2 + 2 + 4 + 2 + 6;
        // SRV
        p.extend_from_slice(&instance_ptr);
        put_u16(&mut p, TYPE_SRV);
        put_u16(&mut p, CLASS_IN);
        p.extend_from_slice(&TTL_SECS.to_be_bytes());
        put_u16(&mut p, 6 + 1 + 5 + 1);
        p.extend_from_slice(&[0, 0, 0, 0]);
        p.extend_from_slice(&7211u16.to_be_bytes());
        // Target "host1" (uncompressed, so the A record can point at it).
        p.push(5);
        p.extend_from_slice(b"host1");
        p.push(0);
        // TXT
        p.extend_from_slice(&instance_ptr);
        put_u16(&mut p, TYPE_TXT);
        put_u16(&mut p, CLASS_IN);
        p.extend_from_slice(&TTL_SECS.to_be_bytes());
        put_u16(&mut p, 4 + 10);
        p.push(3);
        p.extend_from_slice(b"v=1");
        p.push(9);
        p.extend_from_slice(b"game=AMAE");
        // A, owned by a pointer to the SRV target.
        p.extend_from_slice(&(0xc000u16 | target_at as u16).to_be_bytes());
        put_u16(&mut p, TYPE_A);
        put_u16(&mut p, CLASS_IN);
        p.extend_from_slice(&TTL_SECS.to_be_bytes());
        put_u16(&mut p, 4);
        p.extend_from_slice(&[10, 0, 0, 7]);
        let found = hosts(&p);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].addr,
            SocketAddrV4::new(Ipv4Addr::new(10, 0, 0, 7), 7211)
        );
    }

    #[test]
    fn hostile_packets_are_dropped_not_followed() {
        // A name that points at itself.
        let mut p = Vec::new();
        header(&mut p, FLAGS_RESPONSE, 0, 1, 0);
        p.extend_from_slice(&(0xc000u16 | HEADER as u16).to_be_bytes());
        put_u16(&mut p, TYPE_PTR);
        put_u16(&mut p, CLASS_IN);
        p.extend_from_slice(&TTL_SECS.to_be_bytes());
        put_u16(&mut p, 0);
        assert!(hosts(&p).is_empty());
        // Counts that promise more than the packet holds.
        let mut p = announce(TTL_SECS);
        p[6] = 0xff;
        p[7] = 0xff;
        assert!(hosts(&p).is_empty());
        // Every truncation of a good packet, and a spread of garbage, must return not panic.
        let good = announce(TTL_SECS);
        for n in 0..good.len() {
            let _ = hosts(&good[..n]);
            let _ = asks_for_hosts(&good[..n]);
        }
        let mut x = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..2000 {
            let mut buf = good.clone();
            for _ in 0..4 {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                let i = (x as usize) % buf.len();
                buf[i] = (x >> 24) as u8;
            }
            let _ = hosts(&buf);
            let _ = asks_for_hosts(&buf);
        }
    }
}
