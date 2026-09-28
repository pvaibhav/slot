//! Optional, softcore RetroAchievements. Emulator threads only copy RAM into a bounded,
//! nonblocking queue; evaluation/storage and HTTPS each have their own worker.
mod badges;
mod core;
mod library;
mod network;
mod runtime;
mod storage;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

pub use badges::{load_badge, BadgeImage};
use slot_retro::RetroCore;
use storage::{Config, Store, Unlock};

const RAM_SIZE: usize = 0x58000;
const QUEUE_SIZE: usize = 8;

/// A passive shelf indicator. Reading it never acquires a worker's lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SyncStatus {
    Disabled,
    Syncing,
    Ready,
    Offline,
    Attention,
}

#[derive(Default)]
struct Status {
    network: AtomicU8,
    unsaved: AtomicBool,
    reconnect: AtomicBool,
}

impl Status {
    fn set(&self, value: SyncStatus) {
        self.network.store(value as u8, Ordering::Release);
    }

    fn get(&self) -> SyncStatus {
        if self.unsaved.load(Ordering::Acquire) {
            return SyncStatus::Attention;
        }
        match self.network.load(Ordering::Acquire) {
            0 => SyncStatus::Disabled,
            1 => SyncStatus::Syncing,
            2 => SyncStatus::Ready,
            3 => SyncStatus::Offline,
            _ => SyncStatus::Attention,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Status,
    Earned,
}

#[derive(Clone, Debug)]
pub struct Notice {
    /// Local cached image, read only by the notification raster worker.
    pub badge: Option<PathBuf>,
    pub generation: u64,
    pub title: String,
    pub detail: String,
    pub kind: NoticeKind,
}

impl Notice {
    fn status(generation: u64, title: &str) -> Self {
        // Background diagnostics only; routine status is represented by the shelf icon.
        eprintln!("slot: {title}");
        Self {
            badge: None,
            generation,
            title: title.into(),
            detail: String::new(),
            kind: NoticeKind::Status,
        }
    }
}

enum Control {
    Load(network::Load),
    Unload(u64),
    Flush(Arc<AtomicBool>),
}

struct Frame {
    generation: u64,
    sequence: u64,
    timeline: u64,
    earned_at: u64,
    ram: Vec<u8>,
    valid: [usize; 3],
    recycle: mpsc::SyncSender<Vec<u8>>,
}

/// Outlives games, so pending unlocks sync while the player is on the shelf, too.
pub struct Service {
    controls: mpsc::Sender<Control>,
    frames: mpsc::SyncSender<Frame>,
    notices: mpsc::Receiver<Notice>,
    enabled: Arc<AtomicBool>,
    current: Arc<AtomicU64>,
    flushing: AtomicBool,
    flushed: Arc<AtomicBool>,
    status: Arc<Status>,
}

impl Service {
    pub fn start(root: PathBuf) -> Self {
        Self::start_with(root, network::Http::new())
    }

    fn start_with(root: PathBuf, transport: impl network::Transport + Send + 'static) -> Self {
        let (controls, control_rx) = mpsc::channel();
        let (frames, frame_rx) = mpsc::sync_channel(QUEUE_SIZE);
        let (notice_tx, notices) = mpsc::channel();
        let enabled = Arc::new(AtomicBool::new(false));
        let current = Arc::new(AtomicU64::new(0));
        let flushed = Arc::new(AtomicBool::new(false));
        let worker_flushed = flushed.clone();
        let worker_enabled = enabled.clone();
        let status = Arc::new(Status::default());
        let worker_status = status.clone();
        let spawned = std::thread::Builder::new()
            .name("slot-ra".into())
            .spawn(move || {
                run(
                    root,
                    worker_enabled.clone(),
                    control_rx,
                    frame_rx,
                    notice_tx,
                    worker_status,
                    transport,
                );
                worker_enabled.store(false, Ordering::Release);
                worker_flushed.store(true, Ordering::Release);
            });
        if spawned.is_err() {
            status.set(SyncStatus::Attention);
            flushed.store(true, Ordering::Release);
            eprintln!("slot: achievements worker could not start");
        }
        Self {
            controls,
            frames,
            notices,
            enabled,
            current,
            flushing: AtomicBool::new(false),
            flushed,
            status,
        }
    }

    pub fn sync_status(&self) -> SyncStatus {
        self.status.get()
    }

    /// The Wi-Fi worker observed a new connection. Wake retries without blocking the UI.
    pub fn network_available(&self) {
        self.status.reconnect.store(true, Ordering::Release);
    }

    /// Poll during power-off. The UI keeps rendering while the evaluator drains final
    /// frames and fsyncs their awards; an HTTP request is never part of this barrier.
    pub fn flush_ready(&self) -> bool {
        if !self.flushing.swap(true, Ordering::AcqRel)
            && self
                .controls
                .send(Control::Flush(self.flushed.clone()))
                .is_err()
        {
            self.flushed.store(true, Ordering::Release);
        }
        self.flushed.load(Ordering::Acquire)
    }

    pub fn wrap(&self, core: Box<dyn RetroCore>) -> Box<dyn RetroCore> {
        Box::new(core::Tracked::new(core, self))
    }

    pub fn take_notice(&self) -> Option<Notice> {
        while let Ok(notice) = self.notices.try_recv() {
            // Earned notices survive an eject; stale loading notices do not.
            if notice.kind == NoticeKind::Earned
                || notice.generation == 0
                || notice.generation == self.current.load(Ordering::Acquire)
            {
                return Some(notice);
            }
        }
        None
    }
}

struct Playing {
    generation: u64,
    hash: String,
    runtime: runtime::Runtime,
    achievements: BTreeMap<u32, storage::Achievement>,
    previous: Option<(u64, u64)>,
    warned_gap: bool,
}

fn run(
    root: PathBuf,
    enabled: Arc<AtomicBool>,
    controls: mpsc::Receiver<Control>,
    frames: mpsc::Receiver<Frame>,
    notices: mpsc::Sender<Notice>,
    status: Arc<Status>,
    transport: impl network::Transport + Send + 'static,
) {
    let config_path = root.join("System/retroachievements.toml");
    if !config_path.exists() {
        return;
    }
    let config = match Config::read(&config_path) {
        Ok(config) => config,
        Err(_) => {
            status.set(SyncStatus::Attention);
            let _ = notices.send(Notice::status(0, "Invalid RetroAchievements config"));
            return;
        }
    };
    if !config.enabled {
        return;
    }
    if config.username.trim().is_empty() {
        status.set(SyncStatus::Attention);
        let _ = notices.send(Notice::status(0, "Achievements need an account"));
        return;
    }
    let store = match Store::open(&root, &config.username) {
        Ok(store) => Arc::new(Mutex::new(store)),
        Err(error) => {
            status.set(SyncStatus::Attention);
            let _ = notices.send(Notice::status(0, &error));
            return;
        }
    };
    let (loads, load_rx) = mpsc::channel();
    let (prepared_tx, prepared_rx) = mpsc::channel();
    let network_store = store.clone();
    let network_notices = notices.clone();
    let cached_prepared = prepared_tx.clone();
    let network_status = status.clone();
    status.set(SyncStatus::Syncing);
    if std::thread::Builder::new()
        .name("slot-ra-http".into())
        .spawn(move || {
            network::run(
                root,
                config,
                network_store,
                load_rx,
                prepared_tx,
                network_notices,
                network_status,
                transport,
            );
        })
        .is_err()
    {
        status.set(SyncStatus::Attention);
        let _ = notices.send(Notice::status(0, "Achievements network worker failed"));
        return;
    }
    enabled.store(true, Ordering::Release);
    let mut generation = 0;
    let mut playing: Option<Playing> = None;
    let mut unsaved = BTreeMap::new();
    let mut retried = Instant::now();
    loop {
        if retried.elapsed() >= Duration::from_secs(1) {
            retry_unsaved(&mut unsaved, &store, &notices);
            retried = Instant::now();
        }
        loop {
            // Frames queued before an eject/load must be evaluated before retiring that game.
            for _ in 0..QUEUE_SIZE {
                let Ok(frame) = frames.try_recv() else {
                    break;
                };
                evaluate(frame, &mut playing, &store, &notices, &mut unsaved);
            }
            match controls.try_recv() {
                Ok(Control::Flush(done)) => {
                    retry_unsaved(&mut unsaved, &store, &notices);
                    done.store(true, Ordering::Release);
                }
                Ok(Control::Load(load)) => {
                    generation = load.generation;
                    playing = None;
                    // Cached games can start evaluating even while the HTTP worker is
                    // waiting on a timeout or preparing another ROM in the library.
                    let dir = store.lock().unwrap().dir.clone();
                    if storage::read::<storage::Auth>(&dir.join("auth.json")).is_ok() {
                        if let Ok(hash) = library::hash(&dir, &load.path) {
                            if let Ok((game, unlocked)) =
                                storage::read::<(storage::Game, std::collections::BTreeSet<u32>)>(
                                    &dir.join(format!("{hash}.json")),
                                )
                            {
                                if game.console == 5 {
                                    let _ = cached_prepared.send(network::Prepared {
                                        generation,
                                        hash,
                                        game,
                                        unlocked,
                                        cached: true,
                                    });
                                }
                            }
                        }
                    }
                    let _ = loads.send(Some(load));
                }
                Ok(Control::Unload(id)) if id == generation => {
                    generation = 0;
                    playing = None;
                    let _ = loads.send(None);
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Disconnected) => return,
                Err(mpsc::TryRecvError::Empty) => break,
            }
        }
        for prepared in prepared_rx.try_iter() {
            if prepared.generation != generation {
                continue;
            }
            if let Some(game) = playing.as_mut() {
                // Preserve hits for unchanged definitions. Retire removed/unlocked entries
                // and activate new or revised definitions when online data arrives.
                let store = store.lock().unwrap();
                let mut updated = BTreeMap::new();
                for achievement in prepared.game.achievements {
                    if achievement.flags != 3
                        || prepared.unlocked.contains(&achievement.id)
                        || store.unlocks.contains_key(&achievement.id)
                        || unsaved.contains_key(&achievement.id)
                    {
                        continue;
                    }
                    let unchanged = game
                        .achievements
                        .get(&achievement.id)
                        .is_some_and(|old| old.definition == achievement.definition);
                    if unchanged
                        || game
                            .runtime
                            .activate(achievement.id, &achievement.definition)
                    {
                        updated.insert(achievement.id, achievement);
                    }
                }
                for id in game
                    .achievements
                    .keys()
                    .filter(|id| !updated.contains_key(id))
                {
                    game.runtime.deactivate(*id);
                }
                game.achievements = updated;
                continue;
            }
            let Some(mut runtime) = runtime::Runtime::new() else {
                continue;
            };
            let store = store.lock().unwrap();
            let mut achievements = BTreeMap::new();
            let mut unsupported = 0;
            for achievement in prepared.game.achievements {
                if achievement.flags != 3
                    || prepared.unlocked.contains(&achievement.id)
                    || store.unlocks.contains_key(&achievement.id)
                {
                    continue;
                }
                if runtime.activate(achievement.id, &achievement.definition) {
                    achievements.insert(achievement.id, achievement);
                } else {
                    unsupported += 1;
                }
            }
            drop(store);
            let state = if prepared.cached { "cached" } else { "online" };
            let title = if prepared.game.id == 0 {
                "No achievements for this ROM".into()
            } else {
                format!("Achievements ready ({state}, softcore)")
            };
            let detail = if unsupported == 0 {
                prepared.game.title
            } else {
                format!("{} - {unsupported} unsupported", prepared.game.title)
            };
            let _ = notices.send(Notice {
                badge: None,
                generation,
                title,
                detail,
                kind: NoticeKind::Status,
            });
            playing = Some(Playing {
                generation,
                hash: prepared.hash,
                runtime,
                achievements,
                previous: None,
                warned_gap: false,
            });
        }
        match frames.recv_timeout(Duration::from_millis(5)) {
            Ok(frame) => {
                evaluate(frame, &mut playing, &store, &notices, &mut unsaved);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        status.unsaved.store(!unsaved.is_empty(), Ordering::Release);
    }
}

fn evaluate(
    frame: Frame,
    playing: &mut Option<Playing>,
    store: &Arc<Mutex<Store>>,
    notices: &mpsc::Sender<Notice>,
    unsaved: &mut BTreeMap<u32, (Unlock, Notice)>,
) {
    if let Some(game) = playing
        .as_mut()
        .filter(|g| g.generation == frame.generation)
    {
        if let Some((sequence, timeline)) = game.previous {
            if timeline != frame.timeline || sequence + 1 != frame.sequence {
                game.runtime.reset();
                if timeline == frame.timeline && !game.warned_gap {
                    let _ = notices.send(Notice::status(
                        game.generation,
                        "Achievement tracking interrupted",
                    ));
                    game.warned_gap = true;
                }
            }
        }
        game.previous = Some((frame.sequence, frame.timeline));
        if frame.valid[0] == 0 || frame.valid[1] == 0 {
            if !game.warned_gap {
                let _ = notices.send(Notice::status(
                    game.generation,
                    "Core does not expose achievement RAM",
                ));
                game.warned_gap = true;
            }
        } else {
            let earned = game.runtime.frame(&frame.ram, &frame.valid).to_vec();
            for id in earned {
                let unlock = Unlock {
                    id,
                    hash: game.hash.clone(),
                    earned_at: frame.earned_at,
                    synced: false,
                };
                if let Some(achievement) = game.achievements.get(&id) {
                    let notice = Notice {
                        badge: badges::path(&store.lock().unwrap().dir, &achievement.badge),
                        generation: game.generation,
                        title: format!("{} (+{})", achievement.title, achievement.points),
                        detail: achievement.description.clone(),
                        kind: NoticeKind::Earned,
                    };
                    match store.lock().unwrap().record(unlock.clone()) {
                        Ok(true) => {
                            let _ = notices.send(notice);
                        }
                        Ok(false) => {}
                        Err(error) => {
                            let _ = notices.send(Notice::status(game.generation, &error));
                            unsaved.insert(id, (unlock, notice));
                        }
                    }
                }
                game.runtime.deactivate(id);
            }
        }
    }
    let _ = frame.recycle.try_send(frame.ram);
}

fn retry_unsaved(
    unsaved: &mut BTreeMap<u32, (Unlock, Notice)>,
    store: &Arc<Mutex<Store>>,
    notices: &mpsc::Sender<Notice>,
) {
    unsaved.retain(
        |_, (unlock, notice)| match store.lock().unwrap().record(unlock.clone()) {
            Ok(true) => {
                let _ = notices.send(notice.clone());
                false
            }
            Ok(false) => false,
            Err(_) => true,
        },
    );
}
