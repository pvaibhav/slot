use std::path::PathBuf;
use std::sync::Mutex;

use slot_retro::{
    ButtonMask, LibretroCore, Link, LinkChannel, LoopbackLink, MockCore, RetroCore,
    NETPACKET_BROADCAST, NETPACKET_RELIABLE,
};

#[test]
fn the_loopback_double_returns_what_was_sent() {
    let mut link = LoopbackLink::default();
    assert_eq!(link.try_recv(), None, "a fresh link has nothing waiting");

    link.send(NETPACKET_RELIABLE, b"hello");
    link.send(NETPACKET_RELIABLE, b"again");

    assert_eq!(link.try_recv().as_deref(), Some(&b"hello"[..]));
    assert_eq!(
        link.try_recv().as_deref(),
        Some(&b"again"[..]),
        "order was not kept"
    );
    assert_eq!(link.try_recv(), None);
}

#[test]
fn flag_values_match_libretro() {
    assert_eq!(NETPACKET_RELIABLE, 1);
    assert_eq!(NETPACKET_BROADCAST, 0xFFFF);
}

#[test]
fn link_hands_back_inbound_packets_in_the_order_they_arrived() {
    let link = Link::default();
    assert_eq!(
        link.take_inbound(),
        None,
        "a fresh link has nothing waiting"
    );

    link.push_inbound(b"first".to_vec());
    link.push_inbound(b"second".to_vec());

    assert_eq!(link.take_inbound().as_deref(), Some(&b"first"[..]));
    assert_eq!(
        link.take_inbound().as_deref(),
        Some(&b"second"[..]),
        "order was not kept"
    );
    assert_eq!(link.take_inbound(), None);
}

#[test]
fn link_hands_back_outbound_packets_in_the_order_they_were_sent() {
    let link = Link::default();
    assert_eq!(link.take_outbound(), None);

    link.push_outbound(b"a".to_vec());
    link.push_outbound(b"b".to_vec());

    assert_eq!(link.take_outbound().as_deref(), Some(&b"a"[..]));
    assert_eq!(
        link.take_outbound().as_deref(),
        Some(&b"b"[..]),
        "order was not kept"
    );
    assert_eq!(link.take_outbound(), None);
}

#[test]
fn link_is_inactive_until_something_marks_it_live() {
    let link = Link::default();
    assert!(!link.is_active(), "a fresh link is not backing a session");

    link.set_active(true);
    assert!(link.is_active());

    link.set_active(false);
    assert!(
        !link.is_active(),
        "ending the session must be observable too"
    );
}

#[test]
fn a_clone_shares_the_same_queues_as_the_original() {
    let link = Link::default();
    let other = link.clone();

    other.push_inbound(b"shared".to_vec());

    assert_eq!(link.take_inbound().as_deref(), Some(&b"shared"[..]));
}

#[test]
fn a_core_that_never_registers_netpacket_hands_back_an_inert_link() {
    let core: Box<dyn RetroCore> = Box::new(MockCore::new());
    let link = core.net();
    assert!(!link.is_active());
    assert_eq!(link.take_inbound(), None);
    assert_eq!(link.take_outbound(), None);
}

/// One end of a two-way pipe: what one end sends, the other receives.
struct End {
    out: std::sync::Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    into: std::sync::Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
}

fn pipe() -> (End, End) {
    let (a, b) = (Default::default(), std::sync::Arc::<Mutex<_>>::default());
    let a: std::sync::Arc<Mutex<_>> = a;
    (
        End {
            out: a.clone(),
            into: b.clone(),
        },
        End { out: b, into: a },
    )
}

impl LinkChannel for End {
    fn send(&mut self, _flags: i32, buf: &[u8]) {
        self.out.lock().unwrap().push_back(buf.to_vec());
    }

    fn try_recv(&mut self) -> Option<Vec<u8>> {
        self.into.lock().unwrap().pop_front()
    }
}

#[test]
fn a_plugged_link_puts_the_cores_packet_straight_on_the_wire() {
    let link = Link::default();
    let (here, mut there) = pipe();
    link.plug(Box::new(here));

    link.push_outbound(b"mid-frame".to_vec());

    assert_eq!(there.try_recv().as_deref(), Some(&b"mid-frame"[..]));
    assert_eq!(
        link.take_outbound(),
        None,
        "the packet also waited for the frontend to move it"
    );
}

#[test]
fn a_plugged_link_reads_the_wire_once_its_queue_is_empty() {
    let link = Link::default();
    let (here, mut there) = pipe();
    link.push_inbound(b"queued first".to_vec());
    link.plug(Box::new(here));
    there.send(NETPACKET_RELIABLE, b"on the wire");

    assert_eq!(link.take_inbound().as_deref(), Some(&b"queued first"[..]));
    assert_eq!(link.take_inbound().as_deref(), Some(&b"on the wire"[..]));
    assert_eq!(link.take_inbound(), None);
}

#[test]
fn an_unplugged_link_queues_for_the_frontend_again() {
    let link = Link::default();
    let (here, mut there) = pipe();
    link.plug(Box::new(here));
    link.unplug();

    link.push_outbound(b"after".to_vec());
    there.send(NETPACKET_RELIABLE, b"left on the wire");

    assert_eq!(
        there.try_recv(),
        None,
        "an unplugged link still wrote to the wire"
    );
    assert_eq!(link.take_outbound().as_deref(), Some(&b"after"[..]));
    assert_eq!(
        link.take_inbound(),
        None,
        "an unplugged link still read the wire"
    );
}

#[test]
fn a_clone_of_a_plugged_link_sends_on_the_same_wire() {
    let link = Link::default();
    let (here, mut there) = pipe();
    link.plug(Box::new(here));

    link.clone()
        .push_outbound(b"from the core's handle".to_vec());

    assert_eq!(
        there.try_recv().as_deref(),
        Some(&b"from the core's handle"[..])
    );
}

static CORE_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    CORE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn mgba() -> Option<LibretroCore> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../vendor/mgba_libretro.dylib");
    if !p.exists() {
        return None;
    }
    Some(LibretroCore::open(&p).expect("vendored core is present but would not open"))
}

fn header_only_rom() -> PathBuf {
    let mut rom = vec![0u8; 0x8000];
    rom[0..4].copy_from_slice(&0xea00002eu32.to_le_bytes());
    rom[0xa0..0xac].copy_from_slice(b"SLOT TEST\0\0\0");
    rom[0xac..0xb0].copy_from_slice(b"SLTE");
    rom[0xb0..0xb2].copy_from_slice(b"00");
    rom[0xb2] = 0x96;
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("netpacket.gba");
    std::fs::write(&path, &rom).expect("write test rom");
    path
}

#[test]
fn a_libretrocores_net_handle_is_shared_across_clones() {
    let _g = lock();
    let Some(core) = mgba() else {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    };

    let a = core.net();
    let b = core.net();
    a.push_inbound(b"seen from a clone".to_vec());

    assert_eq!(b.take_inbound().as_deref(), Some(&b"seen from a clone"[..]));
}

#[test]
fn pump_link_is_harmless_on_a_core_that_never_registered_netpacket() {
    let _g = lock();
    let Some(mut core) = mgba() else {
        eprintln!("no mgba dylib on this host, skipping");
        return;
    };
    core.load(&header_only_rom()).expect("load");
    core.run_frame(ButtonMask::default());

    core.pump_link();
}
