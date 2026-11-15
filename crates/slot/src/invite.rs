use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use slot_input::Millis;
use slot_store::{Core, Platform};

use crate::ble::Heard;
use crate::link_kind::LinkKind;

const COMPANY: [u8; 2] = [0xFF, 0xFF];
const MAGIC: [u8; 2] = *b"sl";
const VERSION: u8 = 1;
const AD_MANUFACTURER: u8 = 0xFF;
const AD_LOCAL_NAME: u8 = 0x09;
const PAYLOAD_LEN: usize = 26;
const NAME_MAX: usize = 29;
pub const FRESH_MS: Millis = 3000;
pub const NAME_WAIT_MS: Millis = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Advert {
    pub session: u16,
    pub game_crc: u32,
    pub core: Core,
    pub bios_crc: u32,
    pub kind: LinkKind,
    pub freq_mhz: u16,
    pub bssid: [u8; 6],
    pub platform: Platform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Shelf,
    Playing { platform: Platform, crc: u32 },
    Elsewhere,
}

fn core_byte(core: Core) -> u8 {
    match core {
        Core::Mgba => 1,
        Core::Gpsp => 2,
        Core::Gambatte => 3,
        #[cfg(feature = "nds")]
        Core::Dsperate => 0,
    }
}

fn core_from(byte: u8) -> Option<Core> {
    match byte {
        1 => Some(Core::Mgba),
        2 => Some(Core::Gpsp),
        3 => Some(Core::Gambatte),
        _ => None,
    }
}

fn platform_byte(platform: Platform) -> u8 {
    match platform {
        Platform::Gb => 1,
        Platform::Gbc => 2,
        Platform::Gba | Platform::Gbav => 3,
        #[cfg(feature = "nds")]
        Platform::Nds => 0,
    }
}

fn platform_from(byte: u8) -> Option<Platform> {
    match byte {
        1 => Some(Platform::Gb),
        2 => Some(Platform::Gbc),
        3 => Some(Platform::Gba),
        _ => None,
    }
}

pub fn encode(a: &Advert) -> [u8; 31] {
    let mut b = [0u8; 31];
    b[..5].copy_from_slice(&[0x02, 0x01, 0x06, 0x1B, AD_MANUFACTURER]);
    b[5..7].copy_from_slice(&COMPANY);
    b[7..9].copy_from_slice(&MAGIC);
    b[9] = VERSION;
    b[10..12].copy_from_slice(&a.session.to_le_bytes());
    b[12..16].copy_from_slice(&a.game_crc.to_le_bytes());
    b[16] = core_byte(a.core);
    b[17..21].copy_from_slice(&a.bios_crc.to_le_bytes());
    b[21] = match a.kind {
        LinkKind::Cable => 0,
        LinkKind::Wireless => 1,
    };
    b[22..24].copy_from_slice(&a.freq_mhz.to_le_bytes());
    b[24..30].copy_from_slice(&a.bssid);
    b[30] = platform_byte(a.platform);
    b
}

fn ads(data: &[u8]) -> impl Iterator<Item = (u8, &[u8])> {
    let mut rest = data;
    std::iter::from_fn(move || {
        let len = *rest.first()? as usize;
        if len == 0 || rest.len() < len + 1 {
            return None;
        }
        let ad = &rest[1..=len];
        rest = &rest[len + 1..];
        Some((ad[0], &ad[1..]))
    })
}

pub fn decode(data: &[u8]) -> Option<Advert> {
    let (_, p) = ads(data)
        .find(|(t, p)| *t == AD_MANUFACTURER && p.len() >= PAYLOAD_LEN && p[..2] == COMPANY)?;
    if p[2..4] != MAGIC || p[4] != VERSION {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([p[i], p[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([p[i], p[i + 1], p[i + 2], p[i + 3]]);
    Some(Advert {
        session: u16_at(5),
        game_crc: u32_at(7),
        core: core_from(p[11])?,
        bios_crc: u32_at(12),
        kind: match p[16] {
            0 => LinkKind::Cable,
            1 => LinkKind::Wireless,
            _ => return None,
        },
        freq_mhz: u16_at(17),
        bssid: p[19..25].try_into().ok()?,
        platform: platform_from(p[25])?,
    })
}

pub fn cut_name(name: &str) -> &str {
    let mut end = name.len().min(NAME_MAX);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

pub fn scan_response(nickname: &str) -> Vec<u8> {
    let name = cut_name(nickname).as_bytes();
    let mut out = Vec::with_capacity(name.len() + 2);
    out.push(name.len() as u8 + 1);
    out.push(AD_LOCAL_NAME);
    out.extend_from_slice(name);
    out
}

pub fn nickname_of(data: &[u8]) -> Option<String> {
    let (_, name) = ads(data).find(|(t, _)| *t == AD_LOCAL_NAME)?;
    Some(String::from_utf8_lossy(name).into_owned())
}

pub fn worth_a_look(a: &Advert, spot: &Spot, ignored: &HashSet<u16>, local_bios: u32) -> bool {
    if ignored.contains(&a.session) || a.bios_crc != local_bios {
        return false;
    }
    match spot {
        Spot::Shelf => true,
        Spot::Playing { platform, crc } => *crc == a.game_crc && *platform == a.platform,
        Spot::Elsewhere => false,
    }
}

pub fn best_host(
    heard: &HashMap<u16, Heard>,
    want: impl Fn(&Advert) -> bool,
    now: Millis,
) -> Option<&Heard> {
    heard
        .values()
        .filter(|h| now.saturating_sub(h.at) <= FRESH_MS && want(&h.advert))
        .max_by_key(|h| h.rssi)
}

pub fn new_session() -> u16 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(0);
    let mixed = nanos ^ std::process::id();
    let folded = (mixed ^ (mixed >> 16)) as u16;
    if folded == 0 {
        1
    } else {
        folded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Advert {
        Advert {
            session: 0x1234,
            game_crc: 0xDEAD_BEEF,
            core: Core::Gpsp,
            bios_crc: 0x81977335,
            kind: LinkKind::Wireless,
            freq_mhz: 2437,
            bssid: [0xA0, 0xB1, 0xC2, 0xD3, 0xE4, 0xF5],
            platform: Platform::Gba,
        }
    }

    #[test]
    fn an_advert_is_exactly_31_bytes_and_round_trips() {
        let a = Advert {
            session: 0xBEEF,
            game_crc: 0x1234_5678,
            core: Core::Mgba,
            bios_crc: 0x81977335,
            kind: LinkKind::Cable,
            freq_mhz: 5180,
            bssid: [1, 2, 3, 4, 5, 6],
            platform: Platform::Gba,
        };
        let bytes = encode(&a);
        assert_eq!(
            &bytes[..10],
            &[0x02, 0x01, 0x06, 0x1B, 0xFF, 0xFF, 0xFF, b's', b'l', 1]
        );
        assert_eq!(&bytes[10..12], &[0xEF, 0xBE]);
        assert_eq!(bytes[30], 3);
        assert_eq!(decode(&bytes), Some(a));
    }

    #[test]
    fn foreign_adverts_are_ignored() {
        let mut bytes = encode(&sample());
        bytes[7] = b'x';
        assert_eq!(decode(&bytes), None);
        assert_eq!(decode(&[0x02, 0x01, 0x06]), None);
        let mut v2 = encode(&sample());
        v2[9] = 2;
        assert_eq!(decode(&v2), None);
    }

    #[test]
    fn a_long_nickname_is_cut_to_29_bytes_on_a_char_boundary() {
        let r = scan_response(&"é".repeat(20));
        assert!(r.len() <= 31);
        assert_eq!(r[1], 0x09);
        assert_eq!(nickname_of(&r).unwrap(), "é".repeat(14));
    }

    fn heard(session: u16, rssi: i8, at: Millis) -> (u16, Heard) {
        (
            session,
            Heard {
                advert: Advert {
                    session,
                    ..sample()
                },
                nickname: None,
                rssi,
                at,
            },
        )
    }

    #[test]
    fn the_strongest_fresh_host_is_best() {
        let all = HashMap::from([
            heard(1, -70, 10_000),
            heard(2, -50, 10_000),
            heard(3, -30, 6_999),
        ]);
        let best =
            |want: fn(&Advert) -> bool, now| best_host(&all, want, now).map(|h| h.advert.session);
        assert_eq!(best(|_| true, 10_000), Some(2));
        assert_eq!(best(|a| a.session != 2, 10_000), Some(1));
        assert_eq!(best(|_| true, 13_001), None);
        assert_eq!(best(|_| true, 9_999), Some(3));
    }

    #[test]
    fn only_a_matching_game_is_worth_a_look() {
        let a = sample();
        let none = HashSet::new();
        assert!(worth_a_look(&a, &Spot::Shelf, &none, a.bios_crc));
        assert!(worth_a_look(
            &a,
            &Spot::Playing {
                platform: a.platform,
                crc: a.game_crc
            },
            &none,
            a.bios_crc
        ));
        assert!(!worth_a_look(
            &a,
            &Spot::Playing {
                platform: a.platform,
                crc: 7
            },
            &none,
            a.bios_crc
        ));
        assert!(!worth_a_look(&a, &Spot::Shelf, &none, 0));
        assert!(!worth_a_look(&a, &Spot::Elsewhere, &none, a.bios_crc));
        assert!(!worth_a_look(
            &a,
            &Spot::Shelf,
            &HashSet::from([a.session]),
            a.bios_crc
        ));
    }
}
