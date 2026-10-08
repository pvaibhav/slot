use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot_retro::LinkChannel;

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Cancel {
        Cancel::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

const POLL_MS: u64 = 50;

pub const HOST_BOUND: Duration = Duration::from_secs(30);

/// How long either end gives the other to say or answer the greeting. Short: it is one small
/// packet on a network measured in milliseconds, and a host that cannot manage it in this time
/// is not the one this joiner wants.
const GREET_WAIT: Duration = Duration::from_millis(1500);

/// A greeting is `SLOT`, the greeting's own version, then the game's four-byte header code.
const GREETING_LEN: usize = 9;
const GREETING_VERSION: u8 = 1;
/// What a host answers a greeting it accepts with.
const GREETING_ACK: u8 = 0x06;

/// What opens a connection on a shared network. The game rides along so that two handhelds that
/// found each other but are not running the same cart never become a session; on the private
/// network nothing else could have connected, so nothing needed asking.
fn greeting(game: &str) -> [u8; GREETING_LEN] {
    let mut g = [b' '; GREETING_LEN];
    g[..4].copy_from_slice(b"SLOT");
    g[4] = GREETING_VERSION;
    for (slot, byte) in g[5..].iter_mut().zip(game.bytes()) {
        *slot = byte;
    }
    g
}

/// Read a stranger's greeting and answer it, or say it was not the right one. Never waits
/// longer than `GREET_WAIT`, so one connection that says nothing cannot hold the host up.
fn vet(stream: &mut TcpStream, want: &[u8; GREETING_LEN]) -> bool {
    let mut got = [0u8; GREETING_LEN];
    stream.set_read_timeout(Some(GREET_WAIT)).is_ok()
        && stream.set_write_timeout(Some(GREET_WAIT)).is_ok()
        && stream.read_exact(&mut got).is_ok()
        && &got == want
        && stream.write_all(&[GREETING_ACK]).is_ok()
        && stream.set_read_timeout(None).is_ok()
        && stream.set_write_timeout(None).is_ok()
}

const CONTROL_ENDED: u8 = 0x00;

const BYE_MS: u64 = 100;

enum Out {
    Packet(Vec<u8>),
    Control(u8, Sender<()>),
}

pub struct TcpLink {
    outbox: Sender<Out>,
    inbox: Receiver<Vec<u8>>,
    stream: TcpStream,
    closed: Arc<AtomicBool>,
    ended: Arc<AtomicBool>,
}

impl TcpLink {
    /// WiFi a link session actually runs over. On the private network that is the whole
    /// defence, since only the other handheld is there to connect; on the home network
    /// `host_greeted_until` is the one to use, which also turns away anything that does not
    /// open with the right greeting. See the module doc for why this transport does not
    /// hardcode which address that is.
    pub fn host_until(
        addr: &str,
        port: u16,
        bound: Duration,
        cancel: &Cancel,
    ) -> std::io::Result<TcpLink> {
        TcpLink::host_inner(addr, port, bound, cancel, None, || {})
    }

    /// `host_until` for a shared network, where anything on it can reach the listener. Only a
    /// peer that opens with this game's greeting is accepted; anything else is dropped without
    /// a word and the wait goes on, still bounded. `listening` runs once the port is bound and
    /// before the first wait, which is when a host on a shared network announces itself: a
    /// joiner that found it any sooner would be refused.
    pub fn host_greeted_until(
        addr: &str,
        port: u16,
        bound: Duration,
        cancel: &Cancel,
        game: &str,
        listening: impl FnOnce(),
    ) -> std::io::Result<TcpLink> {
        TcpLink::host_inner(addr, port, bound, cancel, Some(greeting(game)), listening)
    }

    fn host_inner(
        addr: &str,
        port: u16,
        bound: Duration,
        cancel: &Cancel,
        expect: Option<[u8; GREETING_LEN]>,
        listening: impl FnOnce(),
    ) -> std::io::Result<TcpLink> {
        let listener = TcpListener::bind((addr, port))?;
        listener.set_nonblocking(true)?;
        listening();
        let deadline = Instant::now() + bound;
        loop {
            if cancel.is_cancelled() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled while waiting for a peer",
                ));
            }
            match listener.accept() {
                Ok((mut stream, _peer)) => {
                    stream.set_nonblocking(false)?;
                    if let Some(want) = expect {
                        // Vetted before it is anyone's session: a stranger, a scanner or the
                        // wrong game gets dropped here and the host keeps waiting for the
                        // right one.
                        if !vet(&mut stream, &want) {
                            continue;
                        }
                    }
                    return TcpLink::wrap(stream);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "no peer arrived",
                ));
            }
            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    }

    pub fn host(addr: &str, port: u16) -> std::io::Result<TcpLink> {
        TcpLink::host_until(addr, port, HOST_BOUND, &Cancel::new())
    }

    pub fn join_until(
        addr: &str,
        port: u16,
        bound: Duration,
        cancel: &Cancel,
    ) -> std::io::Result<TcpLink> {
        let deadline = Instant::now() + bound;
        loop {
            if cancel.is_cancelled() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled while reaching for the host",
                ));
            }
            if let Ok(stream) = TcpStream::connect((addr, port)) {
                return TcpLink::wrap(stream);
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "never reached the host",
                ));
            }
            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    }

    pub fn join(addr: &str, port: u16) -> std::io::Result<TcpLink> {
        TcpLink::wrap(TcpStream::connect((addr, port))?)
    }

    /// One attempt to reach a host on a shared network: connect, open with this game's
    /// greeting, and wait for the host to accept it. Both waits are short and neither retries,
    /// because the caller is walking a list of hosts that answered and the next one is the
    /// retry. A host that turns it away closes the socket, which reads here as an error.
    pub fn join_greeted(addr: std::net::SocketAddr, game: &str) -> std::io::Result<TcpLink> {
        let mut stream = TcpStream::connect_timeout(&addr, GREET_WAIT)?;
        stream.set_write_timeout(Some(GREET_WAIT))?;
        stream.write_all(&greeting(game))?;
        stream.set_read_timeout(Some(GREET_WAIT))?;
        let mut ack = [0u8; 1];
        stream.read_exact(&mut ack)?;
        if ack[0] != GREETING_ACK {
            return Err(std::io::Error::other("host did not accept the greeting"));
        }
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        TcpLink::wrap(stream)
    }

    fn wrap(stream: TcpStream) -> std::io::Result<TcpLink> {
        stream.set_nodelay(true)?;
        let mut reader = stream.try_clone()?;
        let mut writer = stream.try_clone()?;
        let (rtx, inbox) = channel();
        let (wtx, wrx) = channel::<Out>();
        let closed = Arc::new(AtomicBool::new(false));
        let reader_closed = closed.clone();
        let ended = Arc::new(AtomicBool::new(false));
        let reader_ended = ended.clone();

        std::thread::spawn(move || {
            let mut header = [0u8; 2];
            let mut control = false;
            loop {
                if reader.read_exact(&mut header).is_err() {
                    reader_closed.store(true, Ordering::Release);
                    return;
                }
                let len = u16::from_be_bytes(header) as usize;
                if len == 0 && !control {
                    control = true;
                    continue;
                }
                let mut buf = vec![0u8; len];
                if len > 0 && reader.read_exact(&mut buf).is_err() {
                    reader_closed.store(true, Ordering::Release);
                    return;
                }
                if std::mem::take(&mut control) {
                    if buf.first() == Some(&CONTROL_ENDED) {
                        reader_ended.store(true, Ordering::Release);
                    }
                    continue;
                }
                if rtx.send(buf).is_err() {
                    return;
                }
            }
        });

        std::thread::spawn(move || {
            for out in wrx.iter() {
                match out {
                    Out::Packet(buf) => {
                        let Ok(len) = u16::try_from(buf.len()) else {
                            continue;
                        };
                        if writer.write_all(&len.to_be_bytes()).is_err() {
                            return;
                        }
                        if writer.write_all(&buf).is_err() {
                            return;
                        }
                    }
                    Out::Control(op, ack) => {
                        if writer.write_all(&[0, 0, 0, 1, op]).is_err() {
                            return;
                        }
                        let _ = ack.send(());
                    }
                }
            }
        });

        Ok(TcpLink {
            outbox: wtx,
            inbox,
            stream,
            closed,
            ended,
        })
    }

    pub fn send_end(&mut self) {
        let (ack, wrote) = channel();
        if self.outbox.send(Out::Control(CONTROL_ENDED, ack)).is_err() {
            return;
        }
        let _ = wrote.recv_timeout(Duration::from_millis(BYE_MS));
    }

    pub fn nodelay(&self) -> std::io::Result<bool> {
        self.stream.nodelay()
    }
}

impl Drop for TcpLink {
    fn drop(&mut self) {
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

impl LinkChannel for TcpLink {
    fn send(&mut self, _flags: i32, buf: &[u8]) {
        if buf.is_empty() {
            return;
        }
        let _ = self.outbox.send(Out::Packet(buf.to_vec()));
    }

    fn try_recv(&mut self) -> Option<Vec<u8>> {
        match self.inbox.try_recv() {
            Ok(p) => Some(p),
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => None,
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn send_end(&mut self) {
        TcpLink::send_end(self);
    }

    fn peer_ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }
}
