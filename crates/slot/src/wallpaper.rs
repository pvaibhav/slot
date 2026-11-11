use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

pub struct LoadedWallpaper {
    pub name: String,
    pub size: (u32, u32),
    pub rgba: Vec<u8>,
}

struct LoadRequest {
    path: PathBuf,
    name: String,
    size: (u32, u32),
}

pub struct WallpaperLoader {
    requests: Sender<LoadRequest>,
    loaded: Receiver<LoadedWallpaper>,
}

impl WallpaperLoader {
    pub fn spawn() -> Self {
        let (requests, inbox) = mpsc::channel::<LoadRequest>();
        let (outbox, loaded) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-wallpaper".into())
            .spawn(move || {
                while let Ok(req) = inbox.recv() {
                    let (w, h) = req.size;
                    let Some(rgba) = slot_ui::wallpaper_face(&req.path, w, h) else {
                        continue;
                    };
                    let done = LoadedWallpaper {
                        name: req.name,
                        size: req.size,
                        rgba,
                    };
                    if outbox.send(done).is_err() {
                        return;
                    }
                }
            });
        if let Err(e) = spawned {
            eprintln!("slot: wallpaper: worker thread failed to start: {e}");
        }
        WallpaperLoader { requests, loaded }
    }

    pub fn request(&self, root: &Path, name: &str, size: (u32, u32)) {
        let _ = self.requests.send(LoadRequest {
            path: root.join("Wallpapers").join(name),
            name: name.to_owned(),
            size,
        });
    }

    pub fn take(&self) -> Option<LoadedWallpaper> {
        self.loaded.try_recv().ok()
    }
}

pub const SLOT_FILE: &str = "Slot.png";
pub const SLOT_BLUR_FILE: &str = "Slot Blur.png";

pub fn give(root: &Path) -> std::io::Result<()> {
    let dir = root.join("Wallpapers");
    std::fs::create_dir_all(&dir)?;
    for (name, bytes) in [
        (SLOT_FILE, slot_ui::SLOT_WALLPAPER_PNG),
        (SLOT_BLUR_FILE, slot_ui::SLOT_BLUR_WALLPAPER_PNG),
    ] {
        let path = dir.join(name);
        if !path.exists() {
            slot_store::atomic_write(&path, bytes)?;
        }
    }
    Ok(())
}

pub fn list(root: &Path) -> Vec<String> {
    let mut files = pngs(&root.join("Wallpapers"));
    files.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
    files
}

pub fn collection_list(root: &Path) -> Vec<String> {
    let mut files = pngs(&root.join("Wallpapers").join("Collections"));
    files.sort();
    files
}

pub fn pick(root: &Path, seed: u64) -> Option<PathBuf> {
    let dir = root.join("Wallpapers");
    let mut files = pngs(&dir);
    if files.is_empty() {
        return None;
    }
    files.sort();
    let mut rng = Rng(seed);
    let path = root.join("Config").join("wallpaper.deck");
    let (mut shown, mut queued) = read_deck(&path);
    shown.retain(|n| files.contains(n));
    queued.retain(|n| files.contains(n));
    for n in &files {
        if !shown.contains(n) && !queued.contains(n) {
            let at = rng.below(queued.len() + 1);
            queued.insert(at, n.clone());
        }
    }
    if queued.is_empty() {
        let last = shown.pop();
        queued = files;
        for i in (1..queued.len()).rev() {
            queued.swap(i, rng.below(i + 1));
        }
        if queued.len() > 1 && last.as_ref() == queued.first() {
            let end = queued.len() - 1;
            queued.swap(0, end);
        }
        shown.clear();
    }
    let next = queued.remove(0);
    shown.push(next.clone());
    if let Err(e) = slot_store::atomic_write(&path, write_deck(&shown, &queued).as_bytes()) {
        eprintln!("slot: wallpaper.deck: {e}");
    }
    Some(dir.join(next))
}

fn pngs(dir: &Path) -> Vec<String> {
    std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| !slot_store::is_hidden(p))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .filter_map(|p| Some(p.file_name()?.to_str()?.to_owned()))
        .collect()
}

fn read_deck(path: &Path) -> (Vec<String>, Vec<String>) {
    let Some(text) = std::fs::read(path)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
    else {
        return (Vec::new(), Vec::new());
    };
    let mut lines = text.lines();
    let Some(shown) = lines.next().and_then(|l| l.parse::<usize>().ok()) else {
        return (Vec::new(), Vec::new());
    };
    let mut order: Vec<String> = lines.map(str::to_owned).collect();
    let queued = order.split_off(shown.min(order.len()));
    (order, queued)
}

fn write_deck(shown: &[String], queued: &[String]) -> String {
    let mut text = format!("{}\n", shown.len());
    for n in shown.iter().chain(queued) {
        text.push_str(n);
        text.push('\n');
    }
    text
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        (z % n as u64) as usize
    }
}
