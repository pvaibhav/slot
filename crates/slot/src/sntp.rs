use std::net::UdpSocket;
use std::time::Duration;

const PACKET: usize = 48;
const CLIENT_V4: u8 = 0x23;
const SERVER_MODE: u8 = 4;
const UNSYNCED: u8 = 3;
const SECS_AT: usize = 40;
const NTP_TO_UNIX: i64 = 2_208_988_800;
const NEXT_ERA: i64 = 1 << 32;
const SERVER: &str = "pool.ntp.org:123";
const WAIT: Duration = Duration::from_secs(3);

pub fn request() -> [u8; PACKET] {
    let mut packet = [0; PACKET];
    packet[0] = CLIENT_V4;
    packet
}

pub fn parse(reply: &[u8]) -> Option<i64> {
    if reply.len() < PACKET {
        return None;
    }
    let (leap, mode, stratum) = (reply[0] >> 6, reply[0] & 7, reply[1]);
    if leap == UNSYNCED || mode != SERVER_MODE || stratum == 0 {
        return None;
    }
    let secs = u32::from_be_bytes(reply[SECS_AT..SECS_AT + 4].try_into().ok()?);
    let era = if secs >> 31 == 0 { NEXT_ERA } else { 0 };
    Some(i64::from(secs) + era - NTP_TO_UNIX)
}

pub fn fetch() -> Option<i64> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_read_timeout(Some(WAIT)).ok()?;
    socket.connect(SERVER).ok()?;
    socket.send(&request()).ok()?;
    let mut reply = [0; PACKET];
    let n = socket.recv(&mut reply).ok()?;
    parse(&reply[..n])
}
