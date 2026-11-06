use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use slot_input::{Btn, InputSource, Millis, RawEvent};

use super::evdev::{decode, device_name, Ev, Hat, Keymap, EVENT_BYTES, EV_ABS, EV_KEY, EV_SYN};
use super::trace;

const DEV: &str = "/dev/input";
const SYS: &str = "/sys/class/input";
const PSY: &str = "/sys/class/power_supply";

const LID_POLL_MS: Millis = 150;

pub struct DeviceInput {
    held: Vec<Btn>,
    pending: Arc<Mutex<Vec<RawEvent>>>,
    stick: Arc<AtomicBool>,
    hall: Option<PathBuf>,
    lid_shut: Option<bool>,
    next_lid_poll: Millis,
    touch: Option<super::touch::Touch>,
}

impl DeviceInput {
    pub fn open(keymap: Keymap, root: &Path) -> Self {
        DeviceInput::open_in(
            keymap,
            Path::new(DEV),
            Path::new(SYS),
            root,
            trace::enabled(),
        )
    }

    pub fn open_in(keymap: Keymap, dev: &Path, sys: &Path, root: &Path, trace: bool) -> Self {
        let trace = trace
            .then(|| Trace::start(keymap, root, dev, sys))
            .flatten();
        DeviceInput::open_traced(keymap, dev, sys, trace)
    }

    fn open_traced(keymap: Keymap, dev: &Path, sys: &Path, trace: Option<Arc<Trace>>) -> Self {
        let pending: Arc<Mutex<Vec<RawEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let stick = Arc::new(AtomicBool::new(false));
        let mut held = Vec::new();
        for node in keymap.pick_devices(dev, sys) {
            for b in keymap.held_keys(&node) {
                if !held.contains(&b) {
                    held.push(b);
                }
            }
            eprintln!(
                "slot: input {} ({})",
                node.display(),
                device_name(sys, &node)
            );
            quicken_poll(sys, &node);
            let queue = pending.clone();
            let steers = stick.clone();
            let name = node.display().to_string();
            let trace = trace.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("slot-input-{name}"))
                .spawn(move || read_node(keymap, &node, &queue, &steers, trace.as_deref()));
            if let Err(e) = spawned {
                eprintln!("slot: input {name}: {e}");
            }
        }
        DeviceInput {
            held,
            pending,
            stick,
            hall: find_hall(Path::new(PSY)),
            lid_shut: None,
            next_lid_poll: 0,
            touch: (keymap == Keymap::DsOs)
                .then(|| super::touch::open(dev, sys))
                .flatten(),
        }
    }

    pub fn held_at_open(&self) -> &[Btn] {
        &self.held
    }

    pub fn touch(&self) -> Option<(u8, u8)> {
        self.touch.as_ref().and_then(|t| t.get())
    }
}

const POLL_MS: u32 = 10;

fn quicken_poll(sys: &Path, node: &Path) {
    let Some(name) = node.file_name() else {
        return;
    };
    let attr = sys.join(name).join("device/poll");
    let Some(now) = std::fs::read_to_string(&attr)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
    else {
        return;
    };
    if now > POLL_MS {
        match std::fs::write(&attr, POLL_MS.to_string()) {
            Ok(()) => eprintln!(
                "slot: input {}: poll {now} ms -> {POLL_MS} ms",
                node.display()
            ),
            Err(e) => eprintln!("slot: input {}: poll: {e}", node.display()),
        }
    }
}

fn read_node(
    keymap: Keymap,
    node: &Path,
    queue: &Mutex<Vec<RawEvent>>,
    steers: &AtomicBool,
    trace: Option<&Trace>,
) {
    let label = node
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| node.display().to_string());
    let mut hat = Hat::new(keymap);
    let mut file = match File::open(node) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("slot: input {}: {e}", node.display());
            return;
        }
    };
    let mut buf = [0u8; EVENT_BYTES * 16];
    loop {
        let read = match file.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => {
                eprintln!("slot: input {}: {e}", node.display());
                return;
            }
        };
        let mut events = Vec::new();
        for ev in buf[..read].chunks_exact(EVENT_BYTES).filter_map(decode) {
            if let Some(trace) = trace {
                trace.event(&label, ev);
            }
            if (ev.kind == EV_KEY && ev.value == 1) || (ev.kind == EV_ABS && ev.value != 0) {
                crate::latency::read();
            }
            match hat.takes(ev) {
                true => {
                    hat.steer_with_stick(steers.load(Ordering::Relaxed));
                    events.extend(hat.feed(ev));
                }
                false => events.extend(keymap.to_raw(ev)),
            }
        }
        if events.is_empty() {
            continue;
        }
        queue
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(events);
    }
}

struct Trace {
    keymap: Keymap,
    out: Mutex<File>,
    began: Instant,
}

impl Trace {
    fn start(keymap: Keymap, root: &Path, dev: &Path, sys: &Path) -> Option<Arc<Trace>> {
        let dir = crate::root::logs_dir(root);
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(trace::TRACE_FILE);
        let mut file = match File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("slot: {}: {e}", path.display());
                return None;
            }
        };
        for line in trace::survey(keymap, dev, sys) {
            let _ = writeln!(file, "{line}");
        }
        let _ = writeln!(file, "--");
        let _ = file.flush();
        Some(Arc::new(Trace {
            keymap,
            out: Mutex::new(file),
            began: Instant::now(),
        }))
    }

    fn event(&self, node: &str, ev: Ev) {
        if ev.kind == EV_SYN {
            return;
        }
        let line = trace::event_line(self.keymap, node, self.began.elapsed().as_millis(), ev);
        let mut out = self.out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

impl DeviceInput {
    pub fn steer_with_stick(&self, on: bool) {
        self.stick.store(on, Ordering::Relaxed);
    }

    fn read_lid(&self) -> Option<bool> {
        let raw = std::fs::read_to_string(self.hall.as_ref()?).ok()?;
        match raw.trim() {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }
    }
}

impl InputSource for DeviceInput {
    fn poll(&mut self, now: Millis) -> Vec<RawEvent> {
        let mut out = std::mem::take(&mut *self.pending.lock().unwrap_or_else(|e| e.into_inner()));
        if self.hall.is_some() && now >= self.next_lid_poll {
            self.next_lid_poll = now + LID_POLL_MS;
            if let Some(shut) = self.read_lid() {
                if self.lid_shut != Some(shut) {
                    self.lid_shut = Some(shut);
                    out.push(if shut {
                        RawEvent::Down(Btn::Lid)
                    } else {
                        RawEvent::Up(Btn::Lid)
                    });
                }
            }
        }
        out
    }
}

fn find_hall(psy: &Path) -> Option<PathBuf> {
    let mut supplies: Vec<PathBuf> = std::fs::read_dir(psy)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    supplies.sort();
    supplies
        .into_iter()
        .map(|d| d.join("hallkey"))
        .find(|p| p.is_file())
}
