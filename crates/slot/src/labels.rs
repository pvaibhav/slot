//! Labels from `Labels/Artwork.pack`. Checksums, image decoding, SD writes and cartridge
//! rasterization stay off the frame loop.
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;

use slot_store::Cart;
use slot_ui::CartFace;

pub(crate) struct Ready {
    pub cart: Cart,
    pub face: CartFace,
}

pub(crate) struct Labels {
    ready: Receiver<Ready>,
    // Dropping the sender stops the worker between carts. Dropping `ready` releases a blocked send.
    _stop: Sender<()>,
}

impl Labels {
    pub fn spawn(root: PathBuf, carts: Vec<Cart>) -> Self {
        let (out, ready) = mpsc::sync_channel(1);
        let (stop, stopped) = mpsc::channel();
        if !root.join("Labels").join(slot_labels::build::PACK).is_file() {
            return Self { ready, _stop: stop };
        }
        if let Err(e) = thread::Builder::new()
            .name("slot-labels".into())
            .spawn(move || {
                run(carts, stopped, out, |cart| {
                    slot_labels::from_pack(&root, cart)
                })
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
    mut prepare: impl FnMut(&Cart) -> Result<Option<PathBuf>, slot_labels::Error>,
) {
    for mut cart in carts.into_iter().filter(|c| c.label.is_none()) {
        if stop.try_recv() != Err(mpsc::TryRecvError::Empty) {
            return;
        }
        if !cart.rom.is_file() {
            continue;
        }
        match prepare(&cart) {
            Ok(Some(path)) => {
                cart.label = Some(path);
                let face = slot_ui::cart_face(&cart);
                if out.send(Ready { cart, face }).is_err() {
                    return;
                }
            }
            Ok(None) => {}
            Err(e) => eprintln!("slot: labels: {}: {e}", cart.stem),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_store::Platform;
    use std::time::{Duration, Instant};

    fn cart(dir: &std::path::Path, name: &str) -> Cart {
        let rom = dir.join(format!("{name}.gba"));
        std::fs::write(&rom, []).unwrap();
        Cart {
            platform: Platform::Gba,
            stem: name.into(),
            rom,
            label: None,
            title: "".into(),
            code: "".into(),
            shell: None,
            sealed: false,
        }
    }

    #[test]
    fn labels_are_prepared_at_the_size_of_the_well_they_are_drawn_in() {
        assert_eq!(
            slot_labels::label_size(Platform::Gba),
            (slot_ui::LABEL_W, slot_ui::LABEL_H)
        );
        assert_eq!(
            slot_labels::label_size(Platform::Gb),
            (slot_ui::GB_LABEL_W, slot_ui::GB_LABEL_H)
        );
        assert_eq!(
            slot_labels::label_size(Platform::Gbc),
            (slot_ui::GBC_LABEL_W, slot_ui::GBC_LABEL_H)
        );
    }

    #[test]
    fn without_a_pack_no_worker_reads_the_card() {
        let dir = tempfile::tempdir().unwrap();
        let labels = Labels::spawn(dir.path().into(), vec![cart(dir.path(), "Test")]);
        thread::sleep(Duration::from_millis(50));
        assert!(labels.take().is_none());
        assert!(!dir.path().join("Labels").exists());
    }

    #[test]
    fn missing_labels_are_built_but_custom_labels_are_not_requested() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = cart(dir.path(), "Test");
        let mut existing = cart(dir.path(), "Custom");
        existing.label = Some(dir.path().join("Custom.png"));
        let (stop, stopped) = mpsc::channel();
        let (out, ready) = mpsc::sync_channel(1);
        let path = dir.path().join("Test.png");
        let expected = path.clone();
        let worker = thread::spawn(move || {
            run(vec![existing, fresh], stopped, out, |c| {
                assert_eq!(c.stem, "Test");
                Ok(Some(path.clone()))
            })
        });
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
    fn carts_the_pack_does_not_know_or_cannot_read_do_not_block_others() {
        let dir = tempfile::tempdir().unwrap();
        let carts: Vec<_> = ["Unknown", "Broken", "Good"]
            .iter()
            .map(|name| cart(dir.path(), name))
            .collect();
        let (_stop, stopped) = mpsc::channel();
        let (out, ready) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            run(carts, stopped, out, |c| match c.stem.as_str() {
                "Unknown" => Ok(None),
                "Broken" => Err(slot_labels::Error::Image("bad".into())),
                _ => Ok(Some(c.rom.with_extension("png"))),
            })
        });
        let got = ready.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(got.cart.stem, "Good");
        worker.join().unwrap();
    }

    #[test]
    fn dropping_the_owner_stops_the_worker_between_carts() {
        let dir = tempfile::tempdir().unwrap();
        let carts = vec![cart(dir.path(), "Test"); 1000];
        let (stop, stopped) = mpsc::channel();
        let (out, _ready) = mpsc::sync_channel(1);
        let (called, call) = mpsc::channel();
        let worker = thread::spawn(move || {
            run(carts, stopped, out, |_| {
                let _ = called.send(());
                thread::sleep(Duration::from_millis(5));
                Ok(None)
            })
        });
        call.recv_timeout(Duration::from_secs(1)).unwrap();
        let began = Instant::now();
        drop(stop);
        worker.join().unwrap();
        assert!(began.elapsed() < Duration::from_secs(1));
    }
}
