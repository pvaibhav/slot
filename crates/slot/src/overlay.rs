use std::path::Path;

use slot_store::overlay::{overlay_entries, overlay_entries_one_screen};
use slot_store::{OverlayEntry, Platform};

pub const OVERLAYS: &str = "Overlays";

pub fn png_size(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .ok()?;
    let info = reader.info();
    Some((info.width, info.height))
}

pub fn folder(root: &Path, platform: Platform) -> std::path::PathBuf {
    root.join(OVERLAYS).join(platform.hardware().dir_name())
}

pub fn usable(
    root: &Path,
    platform: Platform,
    frame: (u32, u32),
    picture: (u32, u32),
) -> Vec<OverlayEntry> {
    let dir = folder(root, platform);
    let files: Vec<String> = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter_map(|p| Some(p.file_name()?.to_str()?.to_owned()))
        .collect();
    let fits = |name: &Option<String>, ok: &dyn Fn((u32, u32)) -> bool| {
        name.clone()
            .filter(|n| png_size(&dir.join(n)).is_some_and(|size| ok(size)))
    };
    let tile_ok = |(w, h): (u32, u32)| w >= 1 && h >= 1 && w <= picture.0 && h <= picture.1;
    let full_ok = |size: (u32, u32)| size == frame;
    let entries = match two_screens(platform) {
        true => overlay_entries(&files),
        false => overlay_entries_one_screen(&files),
    };
    entries
        .into_iter()
        .map(|e| OverlayEntry {
            tile: fits(&e.tile, &tile_ok),
            both: fits(&e.both, &full_ok),
            top: fits(&e.top, &full_ok),
            bottom: fits(&e.bottom, &full_ok),
            name: e.name,
        })
        .filter(|e| !e.is_empty())
        .collect()
}

pub fn two_screens(platform: Platform) -> bool {
    #[cfg(feature = "nds")]
    if platform == Platform::Nds {
        return true;
    }
    let _ = platform;
    false
}

pub fn decode_rgba(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let file = std::fs::File::open(path).ok()?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let px = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => px
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return None,
    };
    Some((info.width, info.height, rgba))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverlayRequest {
    pub serial: u64,
    pub tile: Option<std::path::PathBuf>,
    pub both: Option<std::path::PathBuf>,
    pub top: Option<std::path::PathBuf>,
    pub bottom: Option<std::path::PathBuf>,
}

pub type Rgba = (u32, u32, Vec<u8>);

#[derive(Clone, Debug, Default)]
pub struct DecodedOverlay {
    pub serial: u64,
    pub tile: Option<Rgba>,
    pub both: Option<Rgba>,
    pub top: Option<Rgba>,
    pub bottom: Option<Rgba>,
}

pub struct OverlayLoader {
    requests: std::sync::mpsc::Sender<OverlayRequest>,
    loaded: std::sync::mpsc::Receiver<DecodedOverlay>,
}

impl OverlayLoader {
    pub fn spawn() -> Self {
        let (requests, inbox) = std::sync::mpsc::channel::<OverlayRequest>();
        let (outbox, loaded) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("slot-overlay".into())
            .spawn(move || {
                while let Ok(mut req) = inbox.recv() {
                    while let Ok(newer) = inbox.try_recv() {
                        req = newer;
                    }
                    let load = |p: &Option<std::path::PathBuf>| p.as_deref().and_then(decode_rgba);
                    let done = DecodedOverlay {
                        serial: req.serial,
                        tile: load(&req.tile),
                        both: load(&req.both),
                        top: load(&req.top),
                        bottom: load(&req.bottom),
                    };
                    if outbox.send(done).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: overlay: worker thread failed to start: {e}");
        }
        OverlayLoader { requests, loaded }
    }

    pub fn request(&self, req: OverlayRequest) {
        let _ = self.requests.send(req);
    }

    pub fn latest(&self) -> Option<DecodedOverlay> {
        self.loaded.try_iter().last()
    }
}
