//! Network, image decoding, SD writes and cartridge rasterization stay off the frame loop.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use slot_store::{Cart, Platform};
use slot_ui::CartFace;

pub(crate) struct Ready {
    pub cart: Cart,
    pub face: CartFace,
}

pub(crate) struct Labels {
    ready: Receiver<Ready>,
    // Dropping the sender wakes an idle worker. In-flight HTTP is bounded by its timeout;
    // shutdown never joins a network request. Dropping `ready` releases a blocked send.
    _stop: Sender<()>,
}

impl Labels {
    pub fn spawn(root: PathBuf, carts: Vec<Cart>) -> Self {
        let (out, ready) = mpsc::sync_channel(1);
        let (stop, stopped) = mpsc::channel();
        if let Err(e) = thread::Builder::new()
            .name("slot-labels".into())
            .spawn(move || {
                let mut downloader = slot_labels::Downloader::default();
                run(carts, stopped, out, |cart| downloader.prepare(&root, cart));
            })
        {
            eprintln!("slot: labels: worker failed to start: {e}");
        }
        Self { ready, _stop: stop }
    }

    pub fn take(&self) -> Option<Ready> {
        self.ready.try_recv().ok()
    }
}

fn run(
    carts: Vec<Cart>,
    stop: Receiver<()>,
    out: SyncSender<Ready>,
    prepare: impl FnMut(&Cart) -> Result<PathBuf, slot_labels::Error>,
) {
    run_with_timing(
        carts,
        stop,
        out,
        prepare,
        [Duration::from_secs(30), Duration::from_secs(120)],
        Duration::from_secs(1),
    );
}

fn run_with_timing(
    carts: Vec<Cart>,
    stop: Receiver<()>,
    out: SyncSender<Ready>,
    mut prepare: impl FnMut(&Cart) -> Result<PathBuf, slot_labels::Error>,
    delays: [Duration; 2],
    pace: Duration,
) {
    let now = Instant::now();
    let mut jobs: VecDeque<_> = carts
        .into_iter()
        // The artwork source only catalogues Game Boy Advance carts.
        .filter(|c| c.label.is_none() && c.platform == Platform::Gba)
        .map(|c| (c, now, 0u8))
        .collect();
    let mut network_retry = now;
    let mut network_failures = 0u8;
    while !jobs.is_empty() {
        let now = Instant::now();
        let Some(index) = jobs
            .iter()
            .position(|(_, due, _)| *due <= now && network_retry <= now)
        else {
            if stop.recv_timeout(pace) != Err(mpsc::RecvTimeoutError::Timeout) {
                return;
            }
            continue;
        };
        if stop.try_recv() != Err(mpsc::TryRecvError::Empty) {
            return;
        }
        let (mut cart, _, attempts) = jobs.remove(index).unwrap();
        if !cart.rom.is_file() {
            continue;
        }
        match prepare(&cart) {
            Ok(path) => {
                network_failures = 0;
                if stop.try_recv() != Err(mpsc::TryRecvError::Empty) {
                    return;
                }
                cart.label = Some(path);
                let face = slot_ui::cart_face(&cart);
                if out.send(Ready { cart, face }).is_err() {
                    return;
                }
            }
            Err(e) => {
                eprintln!("slot: labels: {}: {e}", cart.stem);
                if matches!(e, slot_labels::Error::Network(_)) {
                    network_failures += 1;
                    if network_failures >= 3 {
                        eprintln!("slot: labels: three network failures; deferring remaining labels until next launch");
                        return;
                    }
                    network_retry = Instant::now() + delays[usize::from(network_failures - 1)];
                } else {
                    network_failures = 0;
                }
                if !matches!(
                    e,
                    slot_labels::Error::Unavailable(_) | slot_labels::Error::Image(_)
                ) {
                    if let Some(delay) = delays.get(usize::from(attempts)) {
                        jobs.push_back((cart, Instant::now() + *delay, attempts + 1));
                    }
                }
            }
        }
        // Pace successful transfers and unsupported games as well as failures.
        if stop.recv_timeout(pace) != Err(mpsc::RecvTimeoutError::Timeout) {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_worker_exits_after_three_attempts_even_with_a_large_library() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Test.gba");
        std::fs::write(&rom, []).unwrap();
        let cart = Cart {
            platform: Platform::Gba,
            stem: "Test".into(),
            rom,
            label: None,
            title: "".into(),
            code: "".into(),
            shell: None,
        };
        let (_stop, stopped) = mpsc::channel();
        let (out, _ready) = mpsc::sync_channel(1);
        let mut calls = 0;
        run_with_timing(
            vec![cart; 1000],
            stopped,
            out,
            |_| {
                calls += 1;
                assert!(calls <= 3, "network failure budget did not stop the worker");
                Err(slot_labels::Error::Network("offline".into()))
            },
            [Duration::ZERO; 2],
            Duration::ZERO,
        );
        assert_eq!(calls, 3);
    }

    #[test]
    fn storage_failures_have_a_finite_budget_per_rom_and_skip_terminal_errors() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Test.gba");
        std::fs::write(&rom, []).unwrap();
        let cart = Cart {
            platform: Platform::Gba,
            stem: "Test".into(),
            rom,
            label: None,
            title: "".into(),
            code: "".into(),
            shell: None,
        };
        for terminal in [false, true] {
            let (_stop, stopped) = mpsc::channel();
            let (out, _ready) = mpsc::sync_channel(1);
            let mut calls = 0;
            run_with_timing(
                vec![cart.clone()],
                stopped,
                out,
                |_| {
                    calls += 1;
                    assert!(calls <= 3, "per-ROM budget did not stop the worker");
                    if terminal {
                        Err(slot_labels::Error::Unavailable("no match".into()))
                    } else {
                        Err(slot_labels::Error::Storage(std::io::Error::other(
                            "read only",
                        )))
                    }
                },
                [Duration::ZERO; 2],
                Duration::ZERO,
            );
            assert_eq!(calls, if terminal { 1 } else { 3 });
        }
    }

    #[test]
    fn dropping_the_owner_interrupts_a_retry_wait() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Test.gba");
        std::fs::write(&rom, []).unwrap();
        let cart = Cart {
            platform: Platform::Gba,
            stem: "Test".into(),
            rom,
            label: None,
            title: "".into(),
            code: "".into(),
            shell: None,
        };
        let (stop, stopped) = mpsc::channel();
        let (out, _ready) = mpsc::sync_channel(1);
        let (called, call) = mpsc::channel();
        let worker = thread::spawn(move || {
            run(vec![cart], stopped, out, |_| {
                called.send(()).unwrap();
                Err(slot_labels::Error::Network("offline".into()))
            })
        });
        call.recv_timeout(Duration::from_secs(1)).unwrap();
        let began = Instant::now();
        drop(stop);
        worker.join().unwrap();
        assert!(began.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn missing_labels_are_built_but_custom_labels_are_not_requested() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("Test.gba");
        std::fs::write(&rom, []).unwrap();
        let cart = Cart {
            platform: Platform::Gba,
            stem: "Test".into(),
            rom,
            label: None,
            title: "".into(),
            code: "".into(),
            shell: None,
        };
        let mut existing = cart.clone();
        existing.label = Some(dir.path().join("Custom.png"));
        let (stop, stopped) = mpsc::channel();
        let (out, ready) = mpsc::sync_channel(1);
        let path = dir.path().join("Test.png");
        let expected = path.clone();
        let worker =
            thread::spawn(move || run(vec![existing, cart], stopped, out, |_| Ok(path.clone())));
        let result = ready.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(result.cart.label, Some(expected));
        assert_eq!(
            result.face.rgba.len(),
            (result.face.w * result.face.h * 4) as usize
        );
        drop(stop);
        worker.join().unwrap();
        assert!(ready.try_recv().is_err());
    }

    #[test]
    fn an_unavailable_game_does_not_block_other_games() {
        let dir = tempfile::tempdir().unwrap();
        let carts: Vec<_> = ["Bad", "Good"]
            .iter()
            .map(|name| {
                let rom = dir.path().join(format!("{name}.gba"));
                std::fs::write(&rom, []).unwrap();
                Cart {
                    platform: Platform::Gba,
                    stem: name.to_string(),
                    rom,
                    label: None,
                    title: "".into(),
                    code: "".into(),
                    shell: None,
                }
            })
            .collect();
        let (stop, stopped) = mpsc::channel();
        let (out, ready) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            run(carts, stopped, out, |cart| {
                if cart.stem == "Bad" {
                    Err(slot_labels::Error::Unavailable("no artwork".into()))
                } else {
                    Ok(cart.rom.with_extension("png"))
                }
            })
        });
        assert_eq!(
            ready
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .cart
                .stem,
            "Good"
        );
        let began = Instant::now();
        drop(stop);
        worker.join().unwrap();
        assert!(began.elapsed() < Duration::from_secs(1));
    }
}
