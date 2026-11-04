use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, UNIX_EPOCH};

use slot_store::{Cart, Platform};
use slot_ui::{cover, label_size};

const DIR: &str = "Cache/Labels";
const EXT: &str = "lz4";
const OLD_EXT: &str = "rgba";
const MAGIC: &[u8; 4] = b"SLC2";
const HEAD: usize = 32;
const STALL: Duration = Duration::from_secs(20);
pub const SCREEN_FROM: usize = 50;

type Stamp = (u64, u64);

fn stamps() -> &'static Mutex<HashMap<PathBuf, Stamp>> {
    static STAMPS: OnceLock<Mutex<HashMap<PathBuf, Stamp>>> = OnceLock::new();
    STAMPS.get_or_init(Mutex::default)
}

pub fn learn_stamps(root: &Path) {
    let mut learned = HashMap::new();
    for platform in Platform::ALL {
        let Ok(entries) = std::fs::read_dir(root.join("Labels").join(platform.dir_name())) else {
            continue;
        };
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                learned.insert(entry.path(), stamp_of(&meta));
            }
        }
    }
    *stamps().lock().unwrap_or_else(|e| e.into_inner()) = learned;
}

static REFRESHING: AtomicUsize = AtomicUsize::new(0);

pub fn relearn_stamps(root: &Path) -> Option<thread::JoinHandle<()>> {
    REFRESHING.fetch_add(1, Ordering::AcqRel);
    let root = root.to_path_buf();
    let spawned = thread::Builder::new()
        .name("slot-label-stamps".into())
        .spawn(move || {
            learn_stamps(&root);
            REFRESHING.fetch_sub(1, Ordering::AcqRel);
        });
    if spawned.is_err() {
        REFRESHING.fetch_sub(1, Ordering::AcqRel);
    }
    spawned.ok()
}

fn learned(src: &Path) -> Option<Stamp> {
    if REFRESHING.load(Ordering::Acquire) > 0 {
        return None;
    }
    stamps()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(src)
        .copied()
}

pub fn label_art(root: &Path, cart: &Cart) -> Option<Vec<u8>> {
    let src = cart.label.as_deref()?;
    let (w, h) = label_size(cart);
    let known = learned(src);
    let stamp = match known {
        Some(s) => s,
        None => stamp(src)?,
    };
    let cached = cache_path(root, cart);
    let bytes = std::fs::read(&cached).unwrap_or_default();
    if let Some(art) = parse(&bytes, stamp, w, h) {
        return art;
    }
    let stamp = match known {
        Some(old) => {
            let now = self::stamp(src)?;
            stamps()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(src.to_path_buf(), now);
            if now != old {
                if let Some(art) = parse(&bytes, now, w, h) {
                    return art;
                }
            }
            now
        }
        None => stamp,
    };
    let art = match old_art(&cached.with_extension(OLD_EXT), stamp, w, h) {
        Some(art) => art,
        None => cover(src, w, h),
    };
    if let Some(dir) = cached.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let (cw, ch) = if art.is_some() { (w, h) } else { (0, 0) };
    let body = art.as_deref().map(|rgba| {
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        lz4_flex::block::compress(&rgb)
    });
    let body = body.unwrap_or_default();
    let mut out = Vec::with_capacity(HEAD + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&stamp.0.to_le_bytes());
    out.extend_from_slice(&stamp.1.to_le_bytes());
    out.extend_from_slice(&cw.to_le_bytes());
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    if let Err(e) = slot_store::atomic_write(&cached, &out) {
        eprintln!("slot: label cache: {}: {e}", cached.display());
    }
    let _ = std::fs::remove_file(cached.with_extension(OLD_EXT));
    art
}

pub fn fresh(root: &Path, cart: &Cart) -> bool {
    let Some(src) = cart.label.as_deref() else {
        return true;
    };
    let Some(stamp) = stamp(src) else {
        return true;
    };
    let (w, h) = label_size(cart);
    std::fs::File::open(cache_path(root, cart))
        .and_then(|mut f| {
            let mut head = [0u8; HEAD];
            f.read_exact(&mut head)?;
            Ok((head, f.metadata()?.len()))
        })
        .is_ok_and(|(head, len)| {
            body_len(&head, stamp, w, h).is_some_and(|(_, body)| len == (HEAD + body) as u64)
        })
}

pub fn stale<'a>(root: &Path, carts: impl Iterator<Item = &'a Cart>) -> Vec<Cart> {
    let mut cached = HashMap::new();
    carts
        .filter(|c| c.label.is_some())
        .filter(|c| {
            !cached
                .entry(c.platform)
                .or_insert_with(|| slot_store::listing(&root.join(DIR).join(c.platform.dir_name())))
                .contains_key(&format!("{}.{EXT}", c.stem).to_lowercase())
        })
        .cloned()
        .collect()
}

pub fn cache_path(root: &Path, cart: &Cart) -> PathBuf {
    root.join(DIR)
        .join(cart.platform.dir_name())
        .join(format!("{}.{EXT}", cart.stem))
}

pub struct Rebuild {
    done: Arc<AtomicUsize>,
    total: usize,
    seen: usize,
    moved: Instant,
    stall: Duration,
}

impl Rebuild {
    pub fn start(root: &Path, carts: Vec<Cart>) -> Self {
        Self::start_with_stall(root, carts, STALL)
    }

    pub fn start_with_stall(root: &Path, carts: Vec<Cart>, stall: Duration) -> Self {
        let total = carts.len();
        let done = Arc::new(AtomicUsize::new(0));
        let next = Arc::new(AtomicUsize::new(0));
        let carts = Arc::new(carts);
        let threads = thread::available_parallelism().map_or(1, |n| n.get());
        for n in 0..threads.min(total) {
            let (root, carts, next, done) = (
                root.to_path_buf(),
                carts.clone(),
                next.clone(),
                done.clone(),
            );
            let spawned = thread::Builder::new()
                .name(format!("slot-labels-{n}"))
                .spawn(move || loop {
                    let Some(cart) = carts.get(next.fetch_add(1, Ordering::Relaxed)) else {
                        return;
                    };
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        slot_ui::shell_for(cart);
                        label_art(&root, cart)
                    }));
                    done.fetch_add(1, Ordering::Release);
                });
            if let Err(e) = spawned {
                eprintln!("slot: label cache: rebuild thread failed to start: {e}");
            }
        }
        Rebuild {
            done,
            total,
            seen: 0,
            moved: Instant::now(),
            stall,
        }
    }

    pub fn on_screen(&self) -> bool {
        self.total >= SCREEN_FROM
    }

    pub fn done(&self) -> usize {
        self.done.load(Ordering::Acquire).min(self.total)
    }

    pub fn total(&self) -> usize {
        self.total
    }

    pub fn finished(&self) -> bool {
        self.done() == self.total
    }

    pub fn stalled(&mut self) -> bool {
        let done = self.done();
        if done != self.seen {
            self.seen = done;
            self.moved = Instant::now();
        }
        self.moved.elapsed() >= self.stall
    }
}

fn stamp(src: &Path) -> Option<Stamp> {
    std::fs::metadata(src).ok().map(|m| stamp_of(&m))
}

fn stamp_of(meta: &std::fs::Metadata) -> Stamp {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    (meta.len(), mtime)
}

fn old_art(path: &Path, stamp: Stamp, w: u32, h: u32) -> Option<Option<Vec<u8>>> {
    let bytes = std::fs::read(path).ok()?;
    let (head, rgba) = bytes.split_at_checked(24)?;
    let word = |at: usize| u64::from_le_bytes(head[at..at + 8].try_into().unwrap_or_default());
    let half = |at: usize| u32::from_le_bytes(head[at..at + 4].try_into().unwrap_or_default());
    if (word(0), word(8)) != stamp {
        return None;
    }
    match (half(16), half(20)) {
        (0, 0) if rgba.is_empty() => Some(None),
        size if size == (w, h) && rgba.len() == (w * h * 4) as usize => Some(Some(rgba.to_vec())),
        _ => None,
    }
}

fn parse(bytes: &[u8], stamp: Stamp, w: u32, h: u32) -> Option<Option<Vec<u8>>> {
    let (head, body) = bytes.split_at_checked(HEAD)?;
    let ((cw, ch), len) = body_len(head, stamp, w, h)?;
    if body.len() != len {
        return None;
    }
    if (cw, ch) == (0, 0) {
        return Some(None);
    }
    let rgb = lz4_flex::block::decompress(body, (w * h * 3) as usize).ok()?;
    if rgb.len() != (w * h * 3) as usize {
        return None;
    }
    Some(Some(
        rgb.chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    ))
}

fn body_len(head: &[u8], stamp: Stamp, w: u32, h: u32) -> Option<((u32, u32), usize)> {
    let word = |at: usize| u64::from_le_bytes(head[at..at + 8].try_into().unwrap_or_default());
    let half = |at: usize| u32::from_le_bytes(head[at..at + 4].try_into().unwrap_or_default());
    if &head[..4] != MAGIC || (word(4), word(12)) != stamp {
        return None;
    }
    let size = (half(20), half(24));
    let len = half(28) as usize;
    match size {
        (0, 0) if len == 0 => Some((size, 0)),
        size if size == (w, h) => Some((size, len)),
        _ => None,
    }
}
