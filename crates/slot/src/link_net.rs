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
    pub fn host_until(
        addr: &str,
        port: u16,
        bound: Duration,
        cancel: &Cancel,
    ) -> std::io::Result<TcpLink> {
        let listener = TcpListener::bind((addr, port))?;
        listener.set_nonblocking(true)?;
        let deadline = Instant::now() + bound;
        loop {
            if cancel.is_cancelled() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "cancelled while waiting for a peer",
                ));
            }
            match listener.accept() {
                Ok((stream, _peer)) => {
                    stream.set_nonblocking(false)?;
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
