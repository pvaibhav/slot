use slot::server::{
    announcement, answer, claims, conflicts, heard, mdns_off, model_name, names, probe, respond, wifi_ip, Heard,
    Responder,
};
use std::net::SocketAddr;
use std::net::{Ipv4Addr, SocketAddrV4};

const IP: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 77);

fn query_type(labels: &[&str], qtype: u8) -> Vec<u8> {
    let mut q = query(labels);
    let n = q.len();
    q[n - 3] = qtype;
    q
}

fn query(labels: &[&str]) -> Vec<u8> {
    let mut q = vec![0x12, 0x34, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    for l in labels {
        q.push(l.len() as u8);
        q.extend_from_slice(l.as_bytes());
    }
    q.extend_from_slice(&[0, 0, 1, 0, 1]);
    q
}

const HOST: &str = "slot";
const NAME: [u8; 12] = *b"\x04slot\x05local\x00";

fn expected_a(id: [u8; 2], question: bool, class: [u8; 2], ttl: u8) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend_from_slice(&[0x84, 0, 0, question as u8, 0, 1, 0, 0, 0, 0]);
    if question {
        v.extend_from_slice(&NAME);
        v.extend_from_slice(&[0, 1, 0, 1]);
    }
    v.extend_from_slice(&NAME);
    v.extend_from_slice(&[0, 1]);
    v.extend_from_slice(&class);
    v.extend_from_slice(&[0, 0, 0, ttl, 0, 4, 192, 168, 4, 77]);
    v
}

fn src(port: u16) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(192, 168, 4, 20), port)
}

#[test]
fn answers_slot_local_a_query() {
    let reply = answer(&query(&["slot", "local"]), IP, HOST).unwrap();
    assert_eq!(reply, expected_a([0, 0], false, [0x80, 1], 120));
}

#[test]
fn legacy_unicast_a_query() {
    let (reply, to) = respond(&query(&["slot", "local"]), IP, HOST, src(49152)).unwrap();
    assert_eq!(to, src(49152));
    assert_eq!(reply, expected_a([0x12, 0x34], true, [0, 1], 10));
}

#[test]
fn port_5353_query_goes_to_the_group() {
    let (reply, to) = respond(&query(&["slot", "local"]), IP, HOST, src(5353)).unwrap();
    assert_eq!(to, SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353));
    assert_eq!(reply, expected_a([0, 0], false, [0x80, 1], 120));
}

#[test]
fn aaaa_query_gets_nsec_saying_only_a_exists() {
    let reply = answer(&query_type(&["slot", "local"], 28), IP, HOST).unwrap();
    let mut want = vec![0, 0, 0x84, 0, 0, 0, 0, 1, 0, 0, 0, 0];
    want.extend_from_slice(&NAME);
    want.extend_from_slice(&[0, 47, 0x80, 1, 0, 0, 0, 120, 0, 15]);
    want.extend_from_slice(&NAME);
    want.extend_from_slice(&[0, 1, 0x40]);
    assert_eq!(reply, want);
}

#[test]
fn any_case_name_matches() {
    assert!(answer(&query(&["SLOT", "Local"]), IP, HOST).is_some());
}

#[test]
fn ignores_other_names() {
    assert!(answer(&query(&["other", "local"]), IP, HOST).is_none());
    assert!(answer(&query(&["slot", "local", "x"]), IP, HOST).is_none());
}

#[test]
fn ignores_non_a_queries_and_responses() {
    let mut q = query(&["slot", "local"]);
    let n = q.len();
    q[n - 4] = 0;
    q[n - 3] = 16;
    assert!(answer(&q, IP, HOST).is_none());
    let mut r = query(&["slot", "local"]);
    r[2] = 0x84;
    assert!(answer(&r, IP, HOST).is_none());
}

#[test]
fn ignores_malformed_packets() {
    let q = query(&["slot", "local"]);
    for n in 0..q.len() {
        assert!(answer(&q[..n], IP, HOST).is_none(), "truncated at {n}");
    }
    let mut p = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    p.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1]);
    assert!(answer(&p, IP, HOST).is_none());
    let mut p = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    p.extend_from_slice(&[1, b'a', 0xc0, 12, 0, 1, 0, 1]);
    assert!(answer(&p, IP, HOST).is_none());
    let mut p = vec![0, 1, 0, 0, 0xff, 0xff, 0, 0, 0, 0, 0, 0];
    p.extend_from_slice(&[0xff; 64]);
    assert!(answer(&p, IP, HOST).is_none());
}

#[test]
fn compressed_name_pointing_forward_is_followed() {
    let mut p = vec![0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0];
    p.extend_from_slice(&[4, b's', b'l', b'o', b't', 0xc0, 23, 0, 1, 0, 1, 5, b'l', b'o', b'c', b'a', b'l', 0]);
    assert!(answer(&p, IP, HOST).is_some());
}

#[test]
fn every_baseos_model_makes_a_name() {
    for (model, name) in [
        ("RG28XX", "rg28xx"),
        ("RG34XX", "rg34xx"),
        ("RG34XX SP", "rg34xxsp"),
        ("RG35XX Plus / RG35XX 2024", "rg35xxplus"),
        ("RG35XX H", "rg35xxh"),
        ("RG35XX Pro", "rg35xxpro"),
        ("RG35XX SP", "rg35xxsp"),
        ("RG40XX H", "rg40xxh"),
        ("RG40XX V", "rg40xxv"),
        ("RG CubeXX", "rgcubexx"),
        ("RG SP", "rgsp"),
    ] {
        assert_eq!(model_name(model).as_deref(), Some(name));
    }
    assert_eq!(model_name(""), None);
    assert_eq!(model_name("a.b"), None);
}

const OTHER: Ipv4Addr = Ipv4Addr::new(192, 168, 4, 20);

fn from(ip: Ipv4Addr) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(ip, 5353))
}

#[test]
fn slot_numbers_its_name_like_avahi_and_never_takes_the_device_name() {
    let all: Vec<String> = names().collect();
    assert_eq!(all[..3], ["slot", "slot-2", "slot-3"]);
    assert_eq!(all.len(), 9);
}

#[test]
fn mdns_false_in_baseos_settings_turns_slot_off_too() {
    assert!(!mdns_off(""));
    assert!(!mdns_off("hostname=slot\nmdns=true\n"));
    assert!(mdns_off("hostname=rgsp\nmdns=false\n"));
    assert!(Responder::start_with(IP, true).is_none());
}

#[test]
fn a_probe_asks_for_replies_every_responder_on_the_port_hears() {
    let p = probe("slot", IP);
    assert_eq!(p[12 + NAME.len()..12 + NAME.len() + 4], [0, 255, 0, 1]);
}

#[test]
fn a_numbered_name_says_it_has_no_ipv6_address_in_a_well_formed_record() {
    for host in ["slot", "slot-2"] {
        let reply = answer(&query_type(&[host, "local"], 28), IP, host).unwrap();
        let encoded = host.len() + 1 + 6 + 1;
        let rdlength = u16::from_be_bytes([reply[12 + encoded + 8], reply[12 + encoded + 9]]) as usize;
        assert_eq!(rdlength, reply.len() - (12 + encoded + 10), "{host}");
        assert_eq!(rdlength, encoded + 3, "{host}");
    }
}

#[test]
fn an_address_in_the_additional_section_claims_the_name_too() {
    let theirs = announcement("slot", OTHER, 120);
    let mut moved = theirs.clone();
    moved[6..12].copy_from_slice(&[0, 0, 0, 0, 0, 1]);
    assert!(conflicts(&moved, from(OTHER), IP, "slot"));
    assert_eq!(heard(&moved, from(OTHER), IP, "slot"), Heard::Taken);
    assert_eq!(claims(&moved, "slot"), Some(true));
}

#[test]
fn a_probe_draws_an_answer_that_claims_the_name() {
    let reply = answer(&probe("slot", IP), IP, "slot").unwrap();
    assert_eq!(claims(&reply, "slot"), Some(true));
    assert_eq!(claims(&reply, "slot-2"), Some(false));
    assert!(answer(&probe("slot-2", IP), IP, "slot").is_none());
}

#[test]
fn a_legacy_reply_claims_past_its_question() {
    let (reply, _) = respond(&probe("slot", IP), IP, "slot", src(49152)).unwrap();
    assert_eq!(claims(&reply, "slot"), Some(true));
}

#[test]
fn a_query_or_a_short_reply_claims_nothing() {
    assert_eq!(claims(&probe("slot", IP), "slot"), Some(false));
    let reply = answer(&probe("slot", IP), IP, "slot").unwrap();
    for n in 0..24 {
        assert_ne!(claims(&reply[..n], "slot"), Some(true), "truncated at {n}");
    }
}

#[test]
fn only_another_device_answering_takes_the_name() {
    let reply = answer(&probe("slot", OTHER), OTHER, "slot").unwrap();
    assert_eq!(heard(&reply, from(IP), IP, "slot"), Heard::Nothing);
    assert_eq!(heard(&reply, from(OTHER), IP, "slot"), Heard::Taken);
    assert_eq!(heard(&reply, from(OTHER), IP, "slot-2"), Heard::Nothing);
}

#[test]
fn of_two_devices_probing_at_once_the_higher_address_keeps_the_name() {
    let higher = Ipv4Addr::new(192, 168, 4, 200);
    assert_eq!(heard(&probe("slot", higher), from(higher), IP, "slot"), Heard::Lost);
    assert_eq!(heard(&probe("slot", OTHER), from(OTHER), IP, "slot"), Heard::Nothing);
    assert_eq!(heard(&probe("slot-2", higher), from(higher), IP, "slot"), Heard::Nothing);
    assert_eq!(heard(&probe("slot", IP), from(IP), IP, "slot"), Heard::Nothing);
}

#[test]
fn another_address_for_the_held_name_is_a_conflict_and_the_same_one_is_not() {
    let theirs = announcement("slot", OTHER, 120);
    assert!(conflicts(&theirs, from(OTHER), IP, "slot"));
    assert!(!conflicts(&theirs, from(OTHER), IP, "slot-2"));
    assert!(!conflicts(&announcement("slot", IP, 120), from(OTHER), IP, "slot"));
    assert!(!conflicts(&probe("slot", OTHER), from(OTHER), IP, "slot"));
    assert!(!conflicts(&theirs, from(IP), IP, "slot"));
}

#[test]
fn an_announcement_claims_the_name_and_a_goodbye_has_no_lifetime() {
    let hello = announcement("slot", IP, 120);
    assert_eq!(claims(&hello, "slot"), Some(true));
    let bye = announcement("slot", IP, 0);
    let n = bye.len();
    assert_eq!(bye[n - 10..n - 6], [0, 0, 0, 0]);
    assert_eq!(bye[n - 4..], IP.octets());
}

#[test]
fn a_started_responder_names_a_host() {
    let Some(ip) = wifi_ip() else {
        return;
    };
    let Some(first) = Responder::start_with(ip, false) else {
        return;
    };
    let host = first.host();
    assert!(host == "SLOT.LOCAL" || host.starts_with("SLOT-"), "{host}");
}
