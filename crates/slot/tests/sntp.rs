use slot::sntp::{parse, request};

const NTP_2026: u32 = 3_984_537_600;
const UNIX_2026: i64 = 1_775_548_800;

fn reply(flags: u8, stratum: u8, secs: u32) -> [u8; 48] {
    let mut r = [0u8; 48];
    r[0] = flags;
    r[1] = stratum;
    r[40..44].copy_from_slice(&secs.to_be_bytes());
    r
}

#[test]
fn the_request_is_a_version_four_client_packet() {
    let r = request();
    assert_eq!(r.len(), 48);
    assert_eq!(r[0], 0x23);
}

#[test]
fn a_server_reply_gives_unix_seconds() {
    assert_eq!(parse(&reply(0x24, 2, NTP_2026)), Some(UNIX_2026));
}

#[test]
fn the_era_after_2036_still_counts_forward() {
    assert_eq!(
        parse(&reply(0x24, 2, 100)),
        Some(4_294_967_296 + 100 - 2_208_988_800)
    );
}

#[test]
fn a_short_reply_is_ignored() {
    assert_eq!(parse(&reply(0x24, 2, NTP_2026)[..47]), None);
}

#[test]
fn a_packet_that_is_not_from_a_server_is_ignored() {
    assert_eq!(parse(&reply(0x23, 2, NTP_2026)), None);
}

#[test]
fn a_kiss_of_death_is_ignored() {
    assert_eq!(parse(&reply(0x24, 0, NTP_2026)), None);
}

#[test]
fn an_unsynchronized_server_is_ignored() {
    assert_eq!(parse(&reply(0xe4, 2, NTP_2026)), None);
}
