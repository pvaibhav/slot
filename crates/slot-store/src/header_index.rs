use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

use crate::gb::Header;

const FILE: &str = "Cache/headers";

type Stamp = (u64, u64);

#[derive(Clone)]
enum Known {
    Gb(Option<Header>),
    Gba(String),
}

#[derive(Clone)]
struct Entry {
    stamp: Option<Stamp>,
    known: Known,
}

static DIRTY: AtomicBool = AtomicBool::new(false);

fn entries() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static ENTRIES: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    ENTRIES.get_or_init(Mutex::default)
}

fn lookup(rom: &Path) -> Option<Known> {
    entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(rom)
        .map(|e| e.known.clone())
}

fn remember(rom: &Path, known: Known) {
    let stamp = stamp_of(rom);
    entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(rom.to_path_buf(), Entry { stamp, known });
    DIRTY.store(true, Ordering::Release);
}

pub fn gb_header(rom: &Path) -> Option<Header> {
    if let Some(Known::Gb(h)) = lookup(rom) {
        return h;
    }
    let h = crate::gb::header(rom);
    remember(rom, Known::Gb(h.clone()));
    h
}

pub fn gba_code(rom: &Path) -> String {
    if let Some(Known::Gba(code)) = lookup(rom) {
        return code;
    }
    let code = crate::header_code(rom).unwrap_or_default();
    remember(rom, Known::Gba(code.clone()));
    code
}

pub fn peek_gb_header(rom: &Path) -> Option<Option<Header>> {
    match lookup(rom)? {
        Known::Gb(h) => Some(h),
        Known::Gba(_) => None,
    }
}

pub fn peek_gba_code(rom: &Path) -> Option<String> {
    match lookup(rom)? {
        Known::Gba(code) => Some(code),
        Known::Gb(_) => None,
    }
}

pub fn verify(rom: &Path) {
    let now = stamp_of(rom);
    let mut map = entries().lock().unwrap_or_else(|e| e.into_inner());
    if map.get(rom).is_some_and(|e| e.stamp != now) {
        map.remove(rom);
        DIRTY.store(true, Ordering::Release);
    }
}

pub fn retain(roms: &[PathBuf]) {
    let keep: std::collections::HashSet<&PathBuf> = roms.iter().collect();
    let mut map = entries().lock().unwrap_or_else(|e| e.into_inner());
    let before = map.len();
    map.retain(|rom, _| keep.contains(rom));
    if map.len() != before {
        DIRTY.store(true, Ordering::Release);
    }
}

pub fn dirty() -> bool {
    DIRTY.load(Ordering::Acquire)
}

pub fn save(root: &Path) -> std::io::Result<()> {
    DIRTY.store(false, Ordering::Release);
    let snapshot: Vec<(PathBuf, Entry)> = entries()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .map(|(p, e)| (p.clone(), e.clone()))
        .collect();
    let mut out = String::new();
    for (path, entry) in snapshot {
        let Some(rel) = path.strip_prefix(root).ok().and_then(|r| r.to_str()) else {
            continue;
        };
        let (size, mtime) = match entry.stamp {
            Some((s, m)) => (s.to_string(), m.to_string()),
            None => ("-".into(), "-".into()),
        };
        let line = match &entry.known {
            Known::Gb(Some(h)) => format!(
                "G\t{}\t{size}\t{mtime}\t{}\t{}\t{}\t{}",
                clean(rel),
                h.cgb,
                u8::from(h.japan),
                clean(&h.code),
                clean(&h.title)
            ),
            Known::Gb(None) => format!("N\t{}\t{size}\t{mtime}", clean(rel)),
            Known::Gba(code) => format!("A\t{}\t{size}\t{mtime}\t{}", clean(rel), clean(code)),
        };
        out.push_str(&line);
        out.push('\n');
    }
    let path = root.join(FILE);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let result = crate::atomic_write(&path, out.as_bytes());
    if result.is_err() {
        DIRTY.store(true, Ordering::Release);
    }
    result
}

pub fn load(root: &Path) {
    let text = std::fs::read(root.join(FILE))
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    let loaded: HashMap<PathBuf, Entry> = text.lines().filter_map(|l| parse(root, l)).collect();
    *entries().lock().unwrap_or_else(|e| e.into_inner()) = loaded;
    DIRTY.store(false, Ordering::Release);
}

fn parse(root: &Path, line: &str) -> Option<(PathBuf, Entry)> {
    let f: Vec<&str> = line.split('\t').collect();
    let stamp = match (f.get(2)?.parse::<u64>(), f.get(3)?.parse::<u64>()) {
        (Ok(s), Ok(m)) => Some((s, m)),
        _ => None,
    };
    let known = match (*f.first()?, f.len()) {
        ("G", 8) => Known::Gb(Some(Header {
            cgb: f[4].parse().ok()?,
            japan: f[5] == "1",
            code: f[6].to_string(),
            title: f[7].to_string(),
        })),
        ("N", 4) => Known::Gb(None),
        ("A", 5) => Known::Gba(f[4].to_string()),
        _ => return None,
    };
    Some((root.join(f[1]), Entry { stamp, known }))
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

fn stamp_of(rom: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(rom).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Some((meta.len(), mtime))
}
