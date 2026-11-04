use slot::server::{model_name, ptr_name, reverse_query};
use std::net::Ipv4Addr;

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

const SP: Ipv4Addr = Ipv4Addr::new(192, 168, 1, 250);
const QUERY: &str = "123400000001000000000000033235300131033136380331393207696e2d61646472046172706100000c0001";
const AVAHI_REPLY: &str = "123484000001000100000000033235300131033136380331393207696e2d61646472046172706100000c0001c00c000c00010000000a000c0472677370056c6f63616c00";

fn bytes(hex: &str) -> Vec<u8> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
}

#[test]
fn the_reverse_query_asks_for_the_address() {
    assert_eq!(reverse_query(SP, 0x1234), bytes(QUERY));
}

#[test]
fn avahi_names_the_device_in_its_reply() {
    assert_eq!(ptr_name(&bytes(AVAHI_REPLY), 0x1234).as_deref(), Some("RGSP.LOCAL"));
}

#[test]
fn a_renamed_device_reports_its_new_name() {
    let mut reply = bytes(AVAHI_REPLY);
    let at = reply.len() - 12;
    reply.splice(at.., [6, b'r', b'g', b's', b'p', b'-', b'2', 5, b'l', b'o', b'c', b'a', b'l', 0]);
    let n = reply.len();
    reply[n - 15] = 14;
    assert_eq!(ptr_name(&reply, 0x1234).as_deref(), Some("RGSP-2.LOCAL"));
}

#[test]
fn a_reply_to_another_question_is_ignored() {
    assert_eq!(ptr_name(&bytes(AVAHI_REPLY), 0x4321), None);
}

#[test]
fn a_query_or_a_short_packet_names_nothing() {
    assert_eq!(ptr_name(&bytes(QUERY), 0x1234), None);
    assert_eq!(ptr_name(&bytes(&AVAHI_REPLY[..60]), 0x1234), None);
}
