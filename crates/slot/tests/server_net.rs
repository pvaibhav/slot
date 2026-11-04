use slot::server::{answer, claims, model_name, probe, respond, rival, wifi_ip, Responder};
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

#[test]
fn a_probe_draws_an_answer_that_claims_the_name() {
    let reply = answer(&probe("rgsp"), IP, "rgsp").unwrap();
    assert_eq!(claims(&reply, "rgsp"), Some(true));
    assert_eq!(claims(&reply, "slot"), Some(false));
    assert!(answer(&probe("slot"), IP, "rgsp").is_none());
}

#[test]
fn a_legacy_reply_claims_past_its_question() {
    let (reply, _) = respond(&probe("rgsp"), IP, "rgsp", src(49152)).unwrap();
    assert_eq!(claims(&reply, "rgsp"), Some(true));
}

#[test]
fn a_query_or_a_short_reply_claims_nothing() {
    assert_eq!(claims(&probe("rgsp"), "rgsp"), Some(false));
    let reply = answer(&probe("rgsp"), IP, "rgsp").unwrap();
    for n in 0..24 {
        assert_ne!(claims(&reply[..n], "rgsp"), Some(true), "truncated at {n}");
    }
}

#[test]
fn only_another_device_is_a_rival() {
    let reply = answer(&probe("rgsp"), IP, "rgsp").unwrap();
    let me = std::net::SocketAddr::V4(SocketAddrV4::new(IP, 5353));
    let other = std::net::SocketAddr::V4(src(5353));
    assert!(!rival(&reply, me, IP, "rgsp"));
    assert!(rival(&reply, other, IP, "rgsp"));
    assert!(!rival(&reply, other, IP, "slot"));
}

#[test]
fn a_started_responder_names_a_host() {
    std::env::set_var("BASEOS_MODEL", "RG SP");
    let Some(ip) = wifi_ip() else {
        return;
    };
    let Some(first) = Responder::start(ip) else {
        return;
    };
    assert!(["SLOT.LOCAL", "RGSP.LOCAL"].contains(&first.host().as_str()));
}
