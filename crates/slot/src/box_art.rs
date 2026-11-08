use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use slot_store::{Cart, Platform};

const BOX_MAX_H: u32 = 512;
const LOGO_MAX_W: u32 = 448;

pub type Image = (u32, u32, Vec<u8>);

pub struct DecodedBoxArt {
    pub stem: String,
    pub platform: Platform,
    pub boxed: Option<Image>,
    pub logo: Option<Image>,
}

pub struct BoxArtLoader {
    requests: Sender<(PathBuf, Cart)>,
    decoded: Receiver<DecodedBoxArt>,
}

impl BoxArtLoader {
    pub fn spawn() -> Self {
        let (requests, inbox) = mpsc::channel::<(PathBuf, Cart)>();
        let (outbox, decoded) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-box-art".into())
            .spawn(move || {
                while let Ok(mut job) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        job = newer;
                    }
                    let (root, cart) = job;
                    let (boxed, logo) = crate::art::jobs::box_art_paths(&root, &cart);
                    let art = DecodedBoxArt {
                        stem: cart.stem,
                        platform: cart.platform,
                        boxed: read(&boxed, |w, h| fit(w, h, u32::MAX, BOX_MAX_H)),
                        logo: read(&logo, |w, h| fit(w, h, LOGO_MAX_W, u32::MAX)),
                    };
                    if outbox.send(art).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: box art: worker thread failed to start: {e}");
        }
        BoxArtLoader { requests, decoded }
    }

    pub fn request(&self, root: PathBuf, cart: Cart) {
        let _ = self.requests.send((root, cart));
    }

    pub fn take(&self) -> Option<DecodedBoxArt> {
        let mut newest = None;
        while let Ok(art) = self.decoded.try_recv() {
            newest = Some(art);
        }
        newest
    }
}

fn fit(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let k = (max_w as f32 / w as f32)
        .min(max_h as f32 / h as f32)
        .min(1.0);
    (
        ((w as f32 * k).round() as u32).max(1),
        ((h as f32 * k).round() as u32).max(1),
    )
}

fn read(path: &std::path::Path, size: impl Fn(u32, u32) -> (u32, u32)) -> Option<Image> {
    let (rgba, w, h) = slot_ui::decode_png(path)?;
    if w == 0 || h == 0 {
        return None;
    }
    let (tw, th) = size(w, h);
    if (tw, th) == (w, h) {
        return Some((w, h, rgba));
    }
    Some((tw, th, shrink(&rgba, w, h, tw, th)))
}

fn shrink(src: &[u8], sw: u32, sh: u32, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        let (y0, y1) = (y * sh / h, ((y + 1) * sh / h).max(y * sh / h + 1));
        for x in 0..w {
            let (x0, x1) = (x * sw / w, ((x + 1) * sw / w).max(x * sw / w + 1));
            let mut acc = [0u32; 4];
            for sy in y0..y1 {
                for sx in x0..x1 {
                    let o = ((sy * sw + sx) * 4) as usize;
                    for c in 0..4 {
                        acc[c] += u32::from(src[o + c]);
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            let o = ((y * w + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (acc[c] / n) as u8;
            }
        }
    }
    out
}
