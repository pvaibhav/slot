use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use super::attempt::Attempt;
use super::{Backend, Cmd, Failure, Phase, Snapshot, State, Status};
use crate::update::UpdateFail;

static OWNER: AtomicU64 = AtomicU64::new(0);
static HELD: AtomicBool = AtomicBool::new(false);
static ONLINE: AtomicBool = AtomicBool::new(false);

fn claim() -> u64 {
    let id = OWNER.fetch_add(1, Ordering::SeqCst) + 1;
    HELD.store(false, Ordering::SeqCst);
    ONLINE.store(false, Ordering::SeqCst);
    id
}

pub fn online() -> bool {
    ONLINE.load(Ordering::SeqCst)
}

pub fn wait_online(timeout: Duration) -> Result<(), UpdateFail> {
    let start = Instant::now();
    let mut held = false;
    loop {
        held |= HELD.load(Ordering::SeqCst);
        if online() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(match held {
                true => UpdateFail::Offline,
                false => UpdateFail::NoWifi,
            });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Intent {
    Auto,
    Manual(String),
    Disconnected,
}

const SCAN_EVERY: Duration = Duration::from_secs(10);
const STATUS_ATTEMPT: Duration = Duration::from_secs(1);
const STATUS_IDLE: Duration = Duration::from_secs(3);

enum Msg {
    Cmd(Cmd),
    Scans(bool),
}

struct Core {
    owner: u64,
    intent: Intent,
    backend: Box<dyn Backend>,
    snap: Snapshot,
    attempt: Option<Attempt>,
    joining: Option<u32>,
    scans: bool,
    next_scan: Option<Instant>,
    next_status: Option<Instant>,
}

impl Core {
    fn new(mut backend: Box<dyn Backend>, owner: u64) -> Core {
        let saved = backend.saved();
        Core {
            owner,
            intent: Intent::Auto,
            backend,
            snap: Snapshot {
                saved,
                ..Snapshot::default()
            },
            attempt: None,
            joining: None,
            scans: false,
            next_scan: None,
            next_status: None,
        }
    }

    fn status_gap(&self) -> Duration {
        let leasing = self.snap.status.state == State::Completed && self.snap.status.ip.is_none();
        if self.attempt.is_some() || leasing {
            STATUS_ATTEMPT
        } else {
            STATUS_IDLE
        }
    }

    fn publish(&self) {
        if OWNER.load(Ordering::SeqCst) != self.owner {
            return;
        }
        let online = self.snap.held
            && self.snap.status.state == State::Completed
            && self.snap.status.ip.is_some();
        HELD.store(self.snap.held, Ordering::SeqCst);
        ONLINE.store(online, Ordering::SeqCst);
    }

    fn handle(&mut self, msg: Msg, now: Instant) {
        match msg {
            Msg::Scans(on) => {
                if on && !self.scans {
                    self.next_scan = Some(now);
                }
                if !on {
                    self.next_scan = None;
                }
                self.scans = on;
            }
            Msg::Cmd(cmd) => self.command(cmd, now),
        }
    }

    fn command(&mut self, cmd: Cmd, now: Instant) {
        match cmd {
            Cmd::Hold => {
                self.snap.held = self.backend.start();
                if !self.snap.held {
                    self.backend.stop();
                }
                if self.snap.held {
                    self.snap.saved = self.backend.saved();
                    match &self.intent {
                        Intent::Manual(ssid) => {
                            if let Some(id) = self
                                .snap
                                .saved
                                .iter()
                                .find(|n| n.ssid == *ssid)
                                .map(|n| n.id)
                            {
                                self.connect(id, now);
                            }
                        }
                        Intent::Disconnected => self.backend.disconnect(),
                        Intent::Auto => {}
                    }
                    self.snap.status = self.backend.status();
                    self.next_status = Some(now + self.status_gap());
                    if self.scans {
                        self.next_scan = Some(now);
                    }
                }
            }
            Cmd::Release => self.release(false),
            Cmd::PowerOff => self.release(true),
            Cmd::Scan => self.next_scan = Some(now),
            Cmd::ClearScan => self.snap.scan = None,
            Cmd::Join {
                ssid,
                password,
                hidden,
            } => {
                if let Some(id) = self.backend.add(&ssid, &password, hidden) {
                    self.snap.failure = None;
                    self.snap.saved = self.backend.saved();
                    if !self.snap.held {
                        self.snap.failure = Some((ssid.clone(), Failure::Other));
                    }
                    self.intent = Intent::Manual(ssid);
                    self.connect(id, now);
                    self.joining = Some(id);
                }
            }
            Cmd::Connect(id) => {
                self.snap.failure = None;
                self.snap.saved = self.backend.saved();
                if let Some(n) = self.snap.saved.iter().find(|n| n.id == id) {
                    self.intent = Intent::Manual(n.ssid.clone());
                }
                self.connect(id, now);
            }
            Cmd::Disconnect => {
                self.intent = Intent::Disconnected;
                self.end_attempt();
                self.backend.disconnect();
                self.snap.attempt = None;
                self.next_status = Some(now);
            }
            Cmd::AutoJoin { id, on } => self.backend.set_auto_join(id, on),
            Cmd::Forget(id) => {
                if self.attempt.as_ref().is_some_and(|a| a.id == id) {
                    self.end_attempt();
                }
                if self.snap.attempt.as_ref().is_some_and(|(s, _)| {
                    self.snap.saved.iter().any(|n| n.id == id && n.ssid == *s)
                }) {
                    self.snap.attempt = None;
                }
                if self
                    .snap
                    .saved
                    .iter()
                    .any(|n| n.id == id && self.intent == Intent::Manual(n.ssid.clone()))
                {
                    self.intent = Intent::Auto;
                }
                if let (Some((s, _)), Some(n)) = (
                    self.snap.failure.as_ref(),
                    self.snap.saved.iter().find(|n| n.id == id),
                ) {
                    if *s == n.ssid {
                        self.snap.failure = None;
                    }
                }
                self.backend.forget(id);
            }
        }
        self.snap.saved = self.backend.saved();
        self.publish();
    }

    fn release(&mut self, block: bool) {
        self.end_attempt();
        self.backend.stop();
        if block {
            self.backend.block();
        }
        self.snap.held = false;
        self.snap.scan = None;
        self.snap.attempt = None;
        self.snap.status = Status {
            state: State::Off,
            ssid: None,
            ip: None,
        };
        self.next_status = None;
    }

    fn connect(&mut self, id: u32, now: Instant) {
        self.snap.saved = self.backend.saved();
        let Some(ssid) = self
            .snap
            .saved
            .iter()
            .find(|n| n.id == id)
            .map(|n| n.ssid.clone())
        else {
            return;
        };
        if !self.snap.held {
            return;
        }
        self.end_attempt();
        self.snap.failure = None;
        self.backend.connect(id);
        let a = Attempt::new(id, &ssid, now);
        self.snap.attempt = Some((ssid, a.phase()));
        self.attempt = Some(a);
        self.next_status = Some(now);
    }

    fn settle_join(&mut self, id: u32, ssid: &str, phase: Phase) {
        let doomed: Vec<u32> = match phase {
            Phase::Failed(Failure::WrongPassword) => vec![id],
            Phase::Connected => self
                .snap
                .saved
                .iter()
                .filter(|n| n.ssid == ssid && n.id != id)
                .map(|n| n.id)
                .collect(),
            _ => Vec::new(),
        };
        for id in doomed {
            self.backend.forget(id);
        }
        self.snap.saved = self.backend.saved();
    }

    fn end_attempt(&mut self) {
        if self.attempt.take().is_some() {
            self.backend.finish_connect();
        }
    }

    fn step(&mut self, now: Instant) {
        if !self.snap.held {
            return;
        }
        if self.next_status.is_some_and(|t| now >= t) {
            self.snap.status = self.backend.status();
            if let Some(a) = self.attempt.as_mut() {
                let temp = self.backend.temp_disabled(a.id);
                let seen = self
                    .snap
                    .scan
                    .as_ref()
                    .is_none_or(|v| v.iter().any(|n| n.ssid == a.ssid));
                let phase = a.observe(&self.snap.status, temp, seen, now);
                self.snap.attempt = Some((a.ssid.clone(), phase));
                if phase == Phase::Connected {
                    self.snap.failure = None;
                }
                if let Phase::Failed(f) = phase {
                    self.snap.failure = Some((a.ssid.clone(), f));
                    if f == Failure::WrongPassword && self.intent == Intent::Manual(a.ssid.clone())
                    {
                        self.intent = Intent::Auto;
                    }
                }
                if a.done() {
                    let (id, ssid) = (a.id, a.ssid.clone());
                    self.end_attempt();
                    if self.joining.take() == Some(id) {
                        self.settle_join(id, &ssid, phase);
                    }
                }
            }
            self.next_status = Some(now + self.status_gap());
        }
        if self.scans && self.next_scan.is_some_and(|t| now >= t) {
            self.snap.scan = Some(self.backend.scan());
            self.next_scan = Some(now + SCAN_EVERY);
        }
        self.publish();
    }

    fn next_deadline(&self) -> Option<Instant> {
        if !self.snap.held {
            return None;
        }
        [
            self.next_status,
            if self.scans { self.next_scan } else { None },
        ]
        .into_iter()
        .flatten()
        .min()
    }

    fn busy(&self) -> bool {
        self.attempt.is_some()
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        if OWNER.load(Ordering::SeqCst) == self.owner {
            HELD.store(false, Ordering::SeqCst);
            ONLINE.store(false, Ordering::SeqCst);
        }
    }
}

enum Mode {
    Inline {
        core: Box<Core>,
        queue: Vec<Msg>,
    },
    Thread {
        tx: Sender<Msg>,
        rx: Receiver<Report>,
    },
}

struct Report {
    snap: Snapshot,
    busy: bool,
    reply: bool,
}

pub struct Wifi {
    mode: Mode,
    snap: Snapshot,
    busy: bool,
    waiting: usize,
    posted: u64,
    replied: u64,
    auto_join: Vec<(u64, u32, bool)>,
}

impl Wifi {
    pub fn spawn(backend: Box<dyn Backend>) -> Wifi {
        let (tx, worker_rx) = mpsc::channel::<Msg>();
        let (worker_tx, rx) = mpsc::channel();
        let owner = claim();
        std::thread::spawn(move || {
            let mut core = Core::new(backend, owner);
            let _ = worker_tx.send(Report {
                snap: core.snap.clone(),
                busy: false,
                reply: false,
            });
            loop {
                let now = Instant::now();
                let wait = core
                    .next_deadline()
                    .map(|t| t.saturating_duration_since(now))
                    .unwrap_or(Duration::from_secs(60));
                let before = core.snap.clone();
                let handled = match worker_rx.recv_timeout(wait) {
                    Ok(msg) => {
                        core.handle(msg, Instant::now());
                        true
                    }
                    Err(RecvTimeoutError::Timeout) => false,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                core.step(Instant::now());
                if (handled || core.snap != before)
                    && worker_tx
                        .send(Report {
                            snap: core.snap.clone(),
                            busy: core.busy(),
                            reply: handled,
                        })
                        .is_err()
                {
                    break;
                }
            }
        });
        Wifi {
            mode: Mode::Thread { tx, rx },
            snap: Snapshot::default(),
            busy: false,
            waiting: 0,
            posted: 0,
            replied: 0,
            auto_join: Vec::new(),
        }
    }

    pub fn inline(backend: Box<dyn Backend>) -> Wifi {
        let core = Core::new(backend, claim());
        let snap = core.snap.clone();
        Wifi {
            mode: Mode::Inline {
                core: Box::new(core),
                queue: Vec::new(),
            },
            snap,
            busy: false,
            waiting: 0,
            posted: 0,
            replied: 0,
            auto_join: Vec::new(),
        }
    }

    fn post(&mut self, msg: Msg) {
        match &mut self.mode {
            Mode::Inline { queue, .. } => queue.push(msg),
            Mode::Thread { tx, .. } => {
                if tx.send(msg).is_ok() {
                    self.waiting += 1;
                    self.posted += 1;
                }
            }
        }
    }

    pub fn send(&mut self, cmd: Cmd) {
        let flip = match cmd {
            Cmd::AutoJoin { id, on } => Some((id, on)),
            _ => None,
        };
        if cmd == Cmd::ClearScan {
            self.snap.scan = None;
        }
        self.post(Msg::Cmd(cmd));
        if let (Some((id, on)), Mode::Thread { .. }) = (flip, &self.mode) {
            self.auto_join.push((self.posted, id, on));
            self.show_auto_join();
        }
    }

    fn show_auto_join(&mut self) {
        for &(_, id, on) in &self.auto_join {
            if let Some(n) = self.snap.saved.iter_mut().find(|n| n.id == id) {
                n.auto_join = on;
            }
        }
    }

    pub fn want_scans(&mut self, on: bool) {
        self.post(Msg::Scans(on));
    }

    pub fn poll(&mut self, now: Instant) -> bool {
        let before = self.snap.clone();
        match &mut self.mode {
            Mode::Inline { core, queue } => {
                for msg in queue.drain(..) {
                    core.handle(msg, now);
                }
                core.step(now);
                self.snap = core.snap.clone();
                self.busy = core.busy();
            }
            Mode::Thread { rx, .. } => {
                while let Ok(r) = rx.try_recv() {
                    self.snap = r.snap;
                    self.busy = r.busy;
                    if r.reply {
                        self.waiting = self.waiting.saturating_sub(1);
                        self.replied += 1;
                    }
                }
                let replied = self.replied;
                self.auto_join.retain(|(at, ..)| *at > replied);
                self.show_auto_join();
            }
        }
        self.snap != before
    }

    pub fn snapshot(&self) -> &Snapshot {
        &self.snap
    }

    pub fn busy(&self) -> bool {
        self.busy || self.waiting > 0
    }
}
