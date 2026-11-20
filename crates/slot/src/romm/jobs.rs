use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use slot_store::{scan, Cart, Platform};

use super::api::{version_ok, Api, ApiError, Cancel, Install, Outcome, Pending, Poll};
use super::conf::RommConf;
use super::install::{free_space, install, Finished, Stopped};
use super::socket::{dial, Push, Source};
use crate::art::jobs::{fetch_one, load_art_index};
use crate::update;

const SLOW_DOWN_STEP: u64 = 5;
const POLL_FAILURES: u32 = 3;
const MAX_PENDING: u64 = 600;
const MAX_TRIES: u8 = 3;
const TRIES_SPENT: &str = "Download kept failing";
const ROM_GONE: &str = "Game is gone from the server";
const INSTALLS_OFF: &str = "Installs off on server";
const NO_PERMISSIONS: &str = "Pairing lacks permissions";

#[derive(Clone, Debug, PartialEq)]
pub enum RommJob {
    Pair(RommConf),
    CancelPair,
    Start(RommConf),
    Claim,
    Stop,
    Disconnect(RommConf),
}

#[derive(Clone, Debug, PartialEq)]
pub enum PairFail {
    Offline,
    TooOld,
    Denied,
    Expired,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RommEvent {
    PairShow {
        url: String,
        code: String,
    },
    PairFailed(PairFail),
    Paired {
        token: String,
        device_id: String,
    },
    Unpaired,
    Installed {
        carts: Vec<Cart>,
        platform: Platform,
        stem: String,
    },
    Note(String),
    NoteCleared,
    Online(bool),
}

pub trait RommJobs: Send {
    fn ask(&mut self, job: RommJob);

    fn take(&mut self) -> Option<RommEvent>;
}

struct NoRomm;

impl RommJobs for NoRomm {
    fn ask(&mut self, _job: RommJob) {}

    fn take(&mut self) -> Option<RommEvent> {
        None
    }
}

pub fn no_romm_jobs() -> Box<dyn RommJobs> {
    Box::new(NoRomm)
}

pub type MakeApi = Box<dyn Fn(&RommConf) -> Box<dyn Api> + Send>;

pub type Label = fn(&Path, &Cart) -> bool;

pub type Dial = Arc<dyn Fn(&RommConf) -> Result<Box<dyn Source>, ApiError> + Send + Sync>;

struct Active {
    id: String,
    cancel: Cancel,
}

struct Link {
    conf: RommConf,
    api: Box<dyn Api>,
    enabled: bool,
}

impl Link {
    fn enable(&mut self) {
        let (Some(device_id), Some(token)) = (&self.conf.device_id, &self.conf.token) else {
            return;
        };
        self.enabled = self.api.enable_install(device_id, token).is_ok();
    }
}

pub struct Task {
    req: Install,
    api: Box<dyn Api>,
    cancel: Cancel,
    root: PathBuf,
    label: Label,
}

impl Task {
    pub fn id(&self) -> &str {
        &self.req.id
    }

    pub fn run(self, emit: &mut dyn FnMut(RommEvent)) -> Result<Finished, Stopped> {
        let done = install(
            &self.root,
            self.api.as_ref(),
            &self.req,
            &self.cancel,
            free_space,
        )?;
        if let (Outcome::Done, Some(stem), Some(platform)) =
            (&done.outcome, &done.stem, done.platform)
        {
            self.announce(platform, stem, emit);
        }
        match self.api.report(&self.req.id, &done.outcome) {
            Ok(()) => Ok(done),
            Err(ApiError::Unauthorized) => Err(Stopped::Lost(ApiError::Unauthorized)),
            Err(_) => Err(Stopped::Offline),
        }
    }

    fn announce(&self, platform: Platform, stem: &str, emit: &mut dyn FnMut(RommEvent)) {
        let Ok(mut carts) = scan(&self.root) else {
            return;
        };
        let cart = carts
            .iter()
            .find(|c| c.platform == platform && c.stem == stem);
        let labeled =
            !self.cancel.is_set() && cart.is_some_and(|cart| (self.label)(&self.root, cart));
        if labeled {
            carts = scan(&self.root).unwrap_or(carts);
        }
        emit(RommEvent::Installed {
            carts,
            platform,
            stem: stem.to_string(),
        });
    }
}

pub struct Runner {
    make: MakeApi,
    root: PathBuf,
    label: Label,
    dial: Dial,
    link: Option<Link>,
    active: Option<Active>,
    tries: HashMap<String, u8>,
    noted: bool,
    owed: bool,
    online: Option<bool>,
    halt: Arc<AtomicBool>,
}

impl Runner {
    pub fn new(root: PathBuf, make: MakeApi, label: Label) -> Runner {
        Runner {
            make,
            root,
            label,
            dial: Arc::new(|_| Err(ApiError::Offline)),
            link: None,
            active: None,
            tries: HashMap::new(),
            noted: false,
            owed: false,
            online: None,
            halt: Arc::default(),
        }
    }

    pub fn with_dial(mut self, dial: Dial) -> Runner {
        self.dial = dial;
        self
    }

    pub fn pair(
        &mut self,
        conf: &RommConf,
        wait: &mut dyn FnMut(Duration) -> bool,
        emit: &mut dyn FnMut(RommEvent),
    ) {
        let api = (self.make)(conf);
        let pending = match start(api.as_ref(), conf) {
            Ok(pending) => pending,
            Err(fail) => return emit(RommEvent::PairFailed(fail)),
        };
        emit(RommEvent::PairShow {
            url: format!("{}{}", conf.origin(), pending.path),
            code: pending.user_code.clone(),
        });
        let Some((token, device_id)) = poll_until(api.as_ref(), &pending, wait, emit) else {
            return;
        };
        let _ = api.enable_install(&device_id, &token);
        emit(RommEvent::Paired { token, device_id });
    }

    pub fn start(&mut self, conf: &RommConf) {
        if !conf.paired() {
            return;
        }
        let mut link = Link {
            conf: conf.clone(),
            api: (self.make)(conf),
            enabled: false,
        };
        link.enable();
        self.link = Some(link);
        self.online = None;
    }

    pub fn claim(&mut self, emit: &mut dyn FnMut(RommEvent)) -> Option<Task> {
        let link = self.link.as_mut()?;
        if !link.enabled {
            link.enable();
        }
        let claimed = link.api.claim();
        self.owed = claimed.is_err();
        match claimed {
            Ok(list) => {
                self.reach(true, emit);
                if std::mem::take(&mut self.noted) {
                    emit(RommEvent::NoteCleared);
                }
                self.next(list)
            }
            Err(ApiError::Unauthorized) => {
                self.unpair(emit);
                None
            }
            Err(ApiError::NotFound) => {
                match link.api.device_exists() {
                    Ok(false) => self.unpair(emit),
                    Err(ApiError::Offline) => self.reach(false, emit),
                    _ => {
                        self.reach(true, emit);
                        self.note(INSTALLS_OFF, emit);
                    }
                }
                None
            }
            Err(ApiError::Client(403)) => {
                self.reach(true, emit);
                self.note(NO_PERMISSIONS, emit);
                None
            }
            Err(_) => {
                self.reach(false, emit);
                None
            }
        }
    }

    fn reach(&mut self, up: bool, emit: &mut dyn FnMut(RommEvent)) {
        if self.online.replace(up) != Some(up) {
            emit(RommEvent::Online(up));
        }
    }

    fn note(&mut self, text: &str, emit: &mut dyn FnMut(RommEvent)) {
        if !std::mem::replace(&mut self.noted, true) {
            emit(RommEvent::Note(text.to_string()));
        }
    }

    fn tick(&mut self, emit: &mut dyn FnMut(RommEvent)) -> Option<Task> {
        if self.owed {
            return self.claim(emit);
        }
        let link = self.link.as_mut()?;
        if !link.enabled {
            link.enable();
        }
        None
    }

    fn cancel(&mut self, id: &str) {
        self.tries.remove(id);
        if let Some(active) = self.active.as_ref().filter(|a| a.id == id) {
            active.cancel.set();
        }
    }

    fn next(&mut self, list: Vec<Install>) -> Option<Task> {
        let held = |id: &str| list.iter().any(|r| r.id == id);
        self.tries.retain(|id, _| held(id));
        if let Some(active) = &self.active {
            if !held(&active.id) {
                active.cancel.set();
            }
            return None;
        }
        let req = list.into_iter().next()?;
        let api = (self.make)(&self.link.as_ref()?.conf);
        let cancel = Cancel::new(self.halt.clone());
        self.active = Some(Active {
            id: req.id.clone(),
            cancel: cancel.clone(),
        });
        Some(Task {
            req,
            api,
            cancel,
            root: self.root.clone(),
            label: self.label,
        })
    }

    pub fn stop(&mut self) {
        if let Some(active) = &self.active {
            active.cancel.set();
        }
    }

    pub fn disconnect(&mut self, conf: &RommConf, emit: &mut dyn FnMut(RommEvent)) {
        self.stop();
        let _ = (self.make)(conf).delete_device();
        self.unpair(emit);
    }

    fn unpair(&mut self, emit: &mut dyn FnMut(RommEvent)) {
        self.stop();
        self.link = None;
        self.tries.clear();
        self.noted = false;
        self.owed = false;
        self.online = None;
        emit(RommEvent::Unpaired);
    }

    pub fn ran(
        &mut self,
        id: &str,
        result: Result<Finished, Stopped>,
        emit: &mut dyn FnMut(RommEvent),
    ) -> bool {
        self.active = None;
        let again = matches!(result, Ok(_) | Err(Stopped::Cancelled));
        self.owed |= !again;
        match result {
            Ok(_) => {
                self.tries.remove(id);
            }
            Err(Stopped::Cancelled | Stopped::Offline) => {}
            Err(Stopped::Transient) => {
                let tries = self.tries.entry(id.to_string()).or_insert(0);
                *tries += 1;
                if *tries >= MAX_TRIES {
                    self.fail(id, TRIES_SPENT, emit);
                }
            }
            Err(Stopped::Lost(ApiError::Unauthorized)) => {
                if self.link.is_some() {
                    self.unpair(emit);
                }
            }
            Err(Stopped::Lost(_)) => self.fail(id, ROM_GONE, emit),
        }
        again
    }

    fn fail(&mut self, id: &str, reason: &str, emit: &mut dyn FnMut(RommEvent)) {
        let Some(link) = &self.link else {
            return;
        };
        match link.api.report(id, &Outcome::Failed(reason.to_string())) {
            Ok(()) => {
                self.tries.remove(id);
            }
            Err(ApiError::Unauthorized) => self.unpair(emit),
            Err(_) => {}
        }
    }
}

fn start(api: &dyn Api, conf: &RommConf) -> Result<Pending, PairFail> {
    let version = api.version().map_err(|_| PairFail::Offline)?;
    if !version_ok(&version) {
        return Err(PairFail::TooOld);
    }
    api.pair_start(conf).map_err(|_| PairFail::Offline)
}

fn poll_until(
    api: &dyn Api,
    pending: &Pending,
    wait: &mut dyn FnMut(Duration) -> bool,
    emit: &mut dyn FnMut(RommEvent),
) -> Option<(String, String)> {
    let mut interval = pending.interval.max(1);
    let expires = pending.expires_in.min(MAX_PENDING);
    let began = Instant::now();
    let mut spent = 0u64;
    let mut failures = 0u32;
    loop {
        if wait(Duration::from_secs(interval)) {
            return None;
        }
        spent += interval;
        let late = |spent: u64| spent >= expires || began.elapsed().as_secs() >= expires;
        let fail = match api.pair_poll(&pending.device_code) {
            Ok(Poll::Paired { token, device_id }) => return Some((token, device_id)),
            Ok(Poll::Denied) => PairFail::Denied,
            Ok(Poll::Expired) => PairFail::Expired,
            Ok(Poll::SlowDown) => {
                interval += SLOW_DOWN_STEP;
                failures = 0;
                if late(spent) {
                    PairFail::Expired
                } else {
                    continue;
                }
            }
            Ok(Poll::Waiting) => {
                failures = 0;
                if late(spent) {
                    PairFail::Expired
                } else {
                    continue;
                }
            }
            Err(ApiError::Cancelled) => return None,
            Err(_) => {
                failures += 1;
                if failures < POLL_FAILURES && !late(spent) {
                    continue;
                }
                PairFail::Offline
            }
        };
        emit(RommEvent::PairFailed(fail));
        return None;
    }
}

enum Work {
    Job(RommJob),
    Ran(String, Result<Finished, Stopped>),
    Feed(u64, Feed),
}

enum Feed {
    Up,
    Push(Push),
}

#[derive(Clone, Copy, PartialEq)]
enum LineState {
    Down,
    Dialing,
    Up,
}

struct Line {
    state: LineState,
    turn: u64,
    stop: Arc<AtomicBool>,
    halt: Arc<AtomicBool>,
    dial: Dial,
    back: Sender<Work>,
}

impl Line {
    fn new(dial: Dial, halt: Arc<AtomicBool>, back: Sender<Work>) -> Line {
        Line {
            state: LineState::Down,
            turn: 0,
            stop: Arc::new(AtomicBool::new(false)),
            halt,
            dial,
            back,
        }
    }

    fn open(&mut self, conf: RommConf) {
        self.turn += 1;
        self.state = LineState::Dialing;
        self.stop = Arc::new(AtomicBool::new(false));
        let dial = self.dial.clone();
        let stop = self.stop.clone();
        let halt = self.halt.clone();
        let back = self.back.clone();
        let turn = self.turn;
        std::thread::spawn(move || feed(dial, conf, turn, [stop, halt], back));
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.turn += 1;
        self.state = LineState::Down;
    }
}

fn feed(dial: Dial, conf: RommConf, turn: u64, stops: [Arc<AtomicBool>; 2], back: Sender<Work>) {
    let send = |feed: Feed| {
        let _ = back.send(Work::Feed(turn, feed));
    };
    let mut source = match dial(&conf) {
        Ok(source) => source,
        Err(_) => return send(Feed::Push(Push::Closed)),
    };
    send(Feed::Up);
    while !stops.iter().any(|stop| stop.load(Ordering::SeqCst)) {
        match source.read() {
            Ok(None) => {}
            Ok(Some(Push::Closed)) | Err(_) => return send(Feed::Push(Push::Closed)),
            Ok(Some(push)) => send(Feed::Push(push)),
        }
    }
}

struct Queue {
    jobs: Sender<Work>,
    events: Receiver<RommEvent>,
    halt: Arc<AtomicBool>,
}

impl RommJobs for Queue {
    fn ask(&mut self, job: RommJob) {
        if matches!(job, RommJob::Stop | RommJob::Disconnect(_)) {
            self.halt.store(true, Ordering::SeqCst);
        }
        let _ = self.jobs.send(Work::Job(job));
    }

    fn take(&mut self) -> Option<RommEvent> {
        self.events.try_recv().ok()
    }
}

fn fetch_label(root: &Path, cart: &Cart) -> bool {
    let Ok(index) = load_art_index(root) else {
        return false;
    };
    let agent = update::agent(update::API_TIMEOUT);
    fetch_one(root, cart.clone(), &index, &agent)
}

pub fn romm_jobs(root: PathBuf, api: fn(&RommConf) -> Box<dyn Api>) -> Box<dyn RommJobs> {
    spawn(Runner::new(root, Box::new(move |conf| api(conf)), fetch_label).with_dial(Arc::new(dial)))
}

pub fn spawn(mut runner: Runner) -> Box<dyn RommJobs> {
    let (jobs, rx) = channel::<Work>();
    let (tx, events) = channel::<RommEvent>();
    let back = jobs.clone();
    let halt: Arc<AtomicBool> = Arc::default();
    runner.halt = halt.clone();
    std::thread::spawn(move || work(runner, back, rx, tx));
    Box::new(Queue { jobs, events, halt })
}

fn begin(task: Task, back: Sender<Work>, tx: Sender<RommEvent>) {
    std::thread::spawn(move || {
        let id = task.id().to_string();
        let result = task.run(&mut |e| {
            let _ = tx.send(e);
        });
        let _ = back.send(Work::Ran(id, result));
    });
}

fn claim(runner: &mut Runner, back: &Sender<Work>, tx: &Sender<RommEvent>) {
    let task = runner.claim(&mut |e| {
        let _ = tx.send(e);
    });
    if let Some(task) = task {
        begin(task, back.clone(), tx.clone());
    }
}

fn work(mut runner: Runner, back: Sender<Work>, rx: Receiver<Work>, tx: Sender<RommEvent>) {
    let mut later: VecDeque<Work> = VecDeque::new();
    let mut line = Line::new(runner.dial.clone(), runner.halt.clone(), back.clone());
    let mut live = false;
    loop {
        let job = match later.pop_front() {
            Some(job) => job,
            None => match rx.recv() {
                Ok(job) => job,
                Err(_) => return,
            },
        };
        let mut emit = |e: RommEvent| {
            let _ = tx.send(e);
        };
        match job {
            Work::Job(RommJob::Pair(conf)) => {
                let mut wait = |d: Duration| listen(&rx, &mut later, d);
                runner.pair(&conf, &mut wait, &mut emit);
            }
            Work::Job(RommJob::Start(conf)) => {
                line.close();
                runner.start(&conf);
            }
            Work::Job(RommJob::Claim) => {
                live = true;
                match line.state {
                    LineState::Up => {
                        if let Some(task) = runner.tick(&mut emit) {
                            begin(task, back.clone(), tx.clone());
                        }
                    }
                    LineState::Dialing => claim(&mut runner, &back, &tx),
                    LineState::Down => {
                        if let Some(link) = &runner.link {
                            line.open(link.conf.clone());
                        }
                    }
                }
            }
            Work::Job(RommJob::Stop) => {
                live = false;
                line.close();
                runner.stop();
                runner.halt.store(false, Ordering::SeqCst);
            }
            Work::Job(RommJob::Disconnect(conf)) => {
                line.close();
                runner.disconnect(&conf, &mut emit);
                runner.halt.store(false, Ordering::SeqCst);
            }
            Work::Job(RommJob::CancelPair) => {}
            Work::Ran(id, result) => {
                if runner.ran(&id, result, &mut emit) && live {
                    claim(&mut runner, &back, &tx);
                }
            }
            Work::Feed(turn, _) if turn != line.turn => {}
            Work::Feed(_, Feed::Up) => {
                line.state = LineState::Up;
                claim(&mut runner, &back, &tx);
            }
            Work::Feed(_, Feed::Push(Push::Closed)) => {
                let dialing = line.state == LineState::Dialing;
                line.state = LineState::Down;
                if dialing {
                    claim(&mut runner, &back, &tx);
                }
            }
            Work::Feed(_, Feed::Push(Push::Queued)) => claim(&mut runner, &back, &tx),
            Work::Feed(_, Feed::Push(Push::Cancelled(id))) => runner.cancel(&id),
        }
        if runner.link.is_none() {
            line.close();
        }
    }
}

fn listen(rx: &Receiver<Work>, later: &mut VecDeque<Work>, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(Work::Job(RommJob::CancelPair)) => return true,
            Ok(job @ Work::Job(RommJob::Pair(_))) => {
                later.push_back(job);
                return true;
            }
            Ok(job) => later.push_back(job),
            Err(RecvTimeoutError::Timeout) => return false,
            Err(RecvTimeoutError::Disconnected) => return true,
        }
    }
}
