use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::evdev::{decode, device_name, Ev, EVENT_BYTES};

const EV_SYN: u16 = 0;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;

pub const TOUCH_NAME: &str = "Goodix Capacitive TouchScreen";
const DS_W: i32 = 256;
const DS_H: i32 = 192;
const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TouchMap {
    pub max_x: i32,
    pub max_y: i32,
    pub swap_xy: bool,
    pub invert_x: bool,
    pub invert_y: bool,
}

impl TouchMap {
    pub fn to_ds(&self, raw_x: i32, raw_y: i32) -> (u8, u8) {
        let (x, y, mx, my) = match self.swap_xy {
            true => (raw_y, raw_x, self.max_y, self.max_x),
            false => (raw_x, raw_y, self.max_x, self.max_y),
        };
        let scale = |v: i32, max: i32, out: i32, invert: bool| {
            let v = v.clamp(0, max.max(1));
            let v = if invert { max - v } else { v };
            (v as i64 * out as i64 / (max as i64 + 1)).min(out as i64 - 1) as u8
        };
        (
            scale(x, mx, DS_W, self.invert_x),
            scale(y, my, DS_H, self.invert_y),
        )
    }
}

#[derive(Default)]
pub struct TouchTracker {
    slot: i32,
    down: bool,
    at: (i32, i32),
}

impl TouchTracker {
    pub fn feed(&mut self, ev: Ev) -> Option<Option<(i32, i32)>> {
        match (ev.kind, ev.code) {
            (EV_ABS, ABS_MT_SLOT) => self.slot = ev.value,
            (EV_ABS, ABS_MT_TRACKING_ID) if self.slot == 0 => self.down = ev.value >= 0,
            (EV_ABS, ABS_MT_POSITION_X) if self.slot == 0 => self.at.0 = ev.value,
            (EV_ABS, ABS_MT_POSITION_Y) if self.slot == 0 => self.at.1 = ev.value,
            (EV_SYN, SYN_REPORT) => return Some(self.down.then_some(self.at)),
            _ => {}
        }
        None
    }
}

pub struct Touch {
    packed: Arc<AtomicU32>,
}

impl Touch {
    pub fn get(&self) -> Option<(u8, u8)> {
        let p = self.packed.load(Ordering::Relaxed);
        (p != NONE).then_some(((p >> 8) as u8, p as u8))
    }
}

fn find(dev: &Path, sys: &Path) -> Option<PathBuf> {
    let mut nodes: Vec<PathBuf> = std::fs::read_dir(dev)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("event"))
        })
        .collect();
    nodes.sort();
    nodes.into_iter().find(|n| device_name(sys, n) == TOUCH_NAME)
}

#[cfg(target_os = "linux")]
fn abs_max(file: &File, code: u16) -> Option<i32> {
    use std::os::fd::AsRawFd;
    let mut info = [0i32; 6];
    let request = 0x8018_4540u64 + code as u64;
    let rc = unsafe { libc::ioctl(file.as_raw_fd(), request as _, info.as_mut_ptr()) };
    (rc == 0 && info[2] > 0).then_some(info[2])
}

#[cfg(not(target_os = "linux"))]
fn abs_max(_file: &File, _code: u16) -> Option<i32> {
    None
}

pub fn open(dev: &Path, sys: &Path) -> Option<Touch> {
    let node = find(dev, sys)?;
    let mut file = match File::open(&node) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: touch {}: {e}", node.display());
            return None;
        }
    };
    let map = TouchMap {
        max_x: abs_max(&file, ABS_MT_POSITION_X).unwrap_or(1023),
        max_y: abs_max(&file, ABS_MT_POSITION_Y).unwrap_or(767),
        swap_xy: false,
        invert_x: false,
        invert_y: false,
    };
    eprintln!(
        "slot: touch {} ({TOUCH_NAME}), x 0..{} y 0..{}",
        node.display(),
        map.max_x,
        map.max_y
    );
    let packed = Arc::new(AtomicU32::new(NONE));
    let out = packed.clone();
    let spawned = std::thread::Builder::new()
        .name("slot-touch".into())
        .spawn(move || {
            let mut tracker = TouchTracker::default();
            let mut buf = [0u8; EVENT_BYTES * 32];
            loop {
                let read = match file.read(&mut buf) {
                    Ok(0) => return,
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => {
                        eprintln!("slot: touch: {e}");
                        return;
                    }
                };
                for ev in buf[..read].chunks_exact(EVENT_BYTES).filter_map(decode) {
                    if let Some(state) = tracker.feed(ev) {
                        let p = state.map_or(NONE, |(x, y)| {
                            let (dx, dy) = map.to_ds(x, y);
                            (dx as u32) << 8 | dy as u32
                        });
                        out.store(p, Ordering::Relaxed);
                    }
                }
            }
        });
    if let Err(e) = spawned {
        eprintln!("slot: touch thread: {e}");
        return None;
    }
    Some(Touch { packed })
}
