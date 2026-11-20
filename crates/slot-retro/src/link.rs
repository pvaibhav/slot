use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::ffi::NETPACKET_RELIABLE;

pub trait LinkChannel: Send {
    fn send(&mut self, flags: i32, buf: &[u8]);

    fn try_recv(&mut self) -> Option<Vec<u8>>;

    fn is_closed(&self) -> bool {
        false
    }

    fn send_end(&mut self) {}

    fn peer_ended(&self) -> bool {
        false
    }
}

#[derive(Default)]
pub struct LoopbackLink {
    queue: VecDeque<Vec<u8>>,
}

impl LinkChannel for LoopbackLink {
    fn send(&mut self, _flags: i32, buf: &[u8]) {
        self.queue.push_back(buf.to_vec());
    }

    fn try_recv(&mut self) -> Option<Vec<u8>> {
        self.queue.pop_front()
    }
}

#[derive(Clone, Default)]
pub struct Link(Arc<LinkState>);

#[derive(Default)]
struct LinkState {
    inbound: Mutex<VecDeque<Vec<u8>>>,
    outbound: Mutex<VecDeque<Vec<u8>>>,
    wire: Mutex<Option<Box<dyn LinkChannel>>>,
    active: AtomicBool,
}

fn lock<T>(held: &Mutex<T>) -> MutexGuard<'_, T> {
    held.lock().unwrap_or_else(|e| e.into_inner())
}

impl Link {
    pub fn push_inbound(&self, packet: Vec<u8>) {
        lock(&self.0.inbound).push_back(packet);
    }

    pub fn take_inbound(&self) -> Option<Vec<u8>> {
        let queued = lock(&self.0.inbound).pop_front();
        queued.or_else(|| lock(&self.0.wire).as_mut()?.try_recv())
    }

    pub fn push_outbound(&self, packet: Vec<u8>) {
        if let Some(wire) = lock(&self.0.wire).as_mut() {
            wire.send(NETPACKET_RELIABLE, &packet);
            return;
        }
        lock(&self.0.outbound).push_back(packet);
    }

    /// Carries the core's packets over `wire` as it sends and polls them, instead of queueing
    /// them for the frontend to move between frames. A core that waits inside a frame for its
    /// peer's answer needs its packet on the wire, and the answer read, before that frame ends.
    pub fn plug(&self, wire: Box<dyn LinkChannel>) {
        *lock(&self.0.wire) = Some(wire);
    }

    pub fn unplug(&self) {
        *lock(&self.0.wire) = None;
    }

    pub fn take_outbound(&self) -> Option<Vec<u8>> {
        lock(&self.0.outbound).pop_front()
    }

    pub fn is_active(&self) -> bool {
        self.0.active.load(Ordering::Acquire)
    }

    pub fn set_active(&self, active: bool) {
        self.0.active.store(active, Ordering::Release);
    }

    pub fn clear(&self) {
        lock(&self.0.inbound).clear();
        lock(&self.0.outbound).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poison(link: &Link, queue: fn(&LinkState) -> &Mutex<VecDeque<Vec<u8>>>) {
        let state = link.0.clone();
        std::thread::spawn(move || {
            let _held = queue(&state)
                .lock()
                .expect("the queue was poisoned already");
            panic!("a thread driving the transport died mid packet");
        })
        .join()
        .expect_err("the poisoning thread was supposed to panic");
        assert!(
            queue(&link.0).lock().is_err(),
            "the queue was not left poisoned, so this proves nothing"
        );
    }

    #[test]
    fn a_dead_transport_does_not_take_the_emulator_thread_with_it() {
        let link = Link::default();
        link.set_active(true);
        link.push_inbound(b"arrived before the fault".to_vec());
        poison(&link, |s| &s.inbound);
        poison(&link, |s| &s.outbound);

        let worker = link.clone();
        let frame = std::thread::spawn(move || {
            let got = worker.take_inbound();
            worker.push_outbound(b"this frame's traffic".to_vec());
            (got, worker.take_outbound())
        })
        .join();

        let (got, sent) = frame.expect("the emulator thread died with the transport");
        assert_eq!(
            got.as_deref(),
            Some(&b"arrived before the fault"[..]),
            "a packet queued before the fault is still the core's to read"
        );
        assert_eq!(
            sent.as_deref(),
            Some(&b"this frame's traffic"[..]),
            "the core must still be able to send after the fault"
        );
    }

    #[test]
    fn a_poisoned_inbound_queue_keeps_carrying_packets_to_the_core() {
        let link = Link::default();
        poison(&link, |s| &s.inbound);

        link.push_inbound(b"first".to_vec());
        link.push_inbound(b"second".to_vec());

        assert_eq!(link.take_inbound().as_deref(), Some(&b"first"[..]));
        assert_eq!(
            link.take_inbound().as_deref(),
            Some(&b"second"[..]),
            "order is not something a poisoned lock can disturb"
        );
        assert_eq!(link.take_inbound(), None);
    }

    #[test]
    fn a_poisoned_outbound_queue_keeps_carrying_packets_to_the_wire() {
        let link = Link::default();
        poison(&link, |s| &s.outbound);

        link.push_outbound(b"first".to_vec());
        link.push_outbound(b"second".to_vec());

        assert_eq!(link.take_outbound().as_deref(), Some(&b"first"[..]));
        assert_eq!(link.take_outbound().as_deref(), Some(&b"second"[..]));
        assert_eq!(link.take_outbound(), None);
    }

    #[test]
    fn ending_a_session_still_empties_both_poisoned_queues() {
        let link = Link::default();
        link.push_inbound(b"stale".to_vec());
        link.push_outbound(b"stale".to_vec());
        poison(&link, |s| &s.inbound);
        poison(&link, |s| &s.outbound);

        link.clear();

        assert_eq!(link.take_inbound(), None, "session one's inbound survived");
        assert_eq!(
            link.take_outbound(),
            None,
            "session one's outbound survived"
        );
    }
}
