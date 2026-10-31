use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use slot_store::{Cart, Outline, ShellChoice, ShellFinish};
use slot_ui::house_label::{pale_for, Label};
use slot_ui::{
    bare_face, cart_backing, cart_face_with, cart_interior, cover, gb_shell_of, label_size,
    shell_for, shell_presets, shows_interior, CartFace, Finish,
};

use crate::studio::LabelKind;

pub const CHIP: u32 = 48;

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewJob {
    pub session: u64,
    pub gen: u64,
    pub cart: Cart,
    pub ground: Option<[u8; 3]>,
    pub text: bool,
    pub real: Option<PathBuf>,
    pub logo: Option<PathBuf>,
    pub wheel: Option<PathBuf>,
    pub existing: Option<PathBuf>,
    pub look: Option<crate::art::index::Look>,
}

pub struct StudioFaces {
    pub session: u64,
    pub gen: u64,
    pub faces: Vec<(LabelKind, CartFace)>,
    pub bare: CartFace,
    pub under: Option<[CartFace; 2]>,
}

pub struct ChipAtlas {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

#[derive(Default)]
pub struct Cache {
    session: u64,
    covers: HashMap<(LabelKind, (u32, u32)), Vec<u8>>,
    logo: Option<(Label, [u8; 3])>,
}

fn logo_art(job: &PreviewJob, size: (u32, u32), cache: &mut Cache) -> Option<Vec<u8>> {
    let tint = job.ground.filter(|_| !job.text);
    if let (None, Some(logo)) = (tint, job.logo.as_deref()) {
        return cover(logo, size.0, size.1);
    }
    if cache.logo.is_none() {
        let wheel = std::fs::read(job.wheel.as_deref()?).ok()?;
        let wheel = crate::art::make::as_png(&wheel)?;
        let look = job.look.as_ref();
        let auto = crate::art::make::automatic_ground(&wheel, look, &job.cart.stem);
        let band = look.and_then(|l| l.band);
        let label = Label::from_png(&wheel, auto, job.cart.platform, band)?;
        cache.logo = Some((label, auto));
    }
    let (label, auto) = cache.logo.as_mut()?;
    label.set_deep(&tint.unwrap_or(*auto));
    label.covered(size.0, size.1)
}

pub fn build(job: PreviewJob, cache: &mut Cache) -> StudioFaces {
    if cache.session != job.session {
        *cache = Cache {
            session: job.session,
            ..Cache::default()
        };
    }
    let size = label_size(&job.cart);
    let tinted = job.ground.filter(|_| job.text).map(|c| slot_ui::text_label(&job.cart, c));
    let mut faces = vec![(LabelKind::Title, cart_face_with(&job.cart, tinted))];
    for (kind, path) in [
        (LabelKind::Existing, &job.existing),
        (LabelKind::Real, &job.real),
    ] {
        let Some(path) = path else { continue };
        if let std::collections::hash_map::Entry::Vacant(slot) = cache.covers.entry((kind, size)) {
            if let Some(art) = cover(path, size.0, size.1) {
                slot.insert(art);
            }
        }
        if let Some(art) = cache.covers.get(&(kind, size)) {
            faces.push((kind, cart_face_with(&job.cart, Some(art.clone()))));
        }
    }
    if job.logo.is_some() || job.wheel.is_some() {
        if let Some(art) = logo_art(&job, size, cache) {
            faces.push((LabelKind::Logo, cart_face_with(&job.cart, Some(art))));
        }
    }
    StudioFaces {
        session: job.session,
        gen: job.gen,
        bare: bare_face(&job.cart),
        under: shows_interior(&job.cart)
            .then(|| [cart_interior(&job.cart), cart_backing(gb_shell_of(&job.cart))]),
        faces,
    }
}

fn disc(cell: &mut [u8]) {
    let r = CHIP as f32 / 2.0;
    for y in 0..CHIP {
        for x in 0..CHIP {
            let (dx, dy) = (x as f32 + 0.5 - r, y as f32 + 0.5 - r);
            let edge = (r - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0);
            let i = ((y * CHIP + x) * 4 + 3) as usize;
            cell[i] = (cell[i] as f32 * edge) as u8;
        }
    }
}

fn solid(rgb: [u8; 3]) -> Vec<u8> {
    [rgb[0], rgb[1], rgb[2], 255].repeat((CHIP * CHIP) as usize)
}

fn ground_chip(deep: [u8; 3]) -> Vec<u8> {
    let pale = pale_for(deep);
    let mut out = Vec::with_capacity((CHIP * CHIP * 4) as usize);
    for y in 0..CHIP {
        for x in 0..CHIP {
            let t = (x + y) as f32 / (2 * CHIP - 2) as f32;
            for c in 0..3 {
                out.push((deep[c] as f32 + (pale[c] as f32 - deep[c] as f32) * t) as u8);
            }
            out.push(255);
        }
    }
    out
}

fn plastic_chip(cart: &Cart, shell: ShellChoice) -> Vec<u8> {
    let mut tinted = cart.clone();
    tinted.shell = Some(shell);
    let face = bare_face(&tinted);
    let backing = cart_backing(gb_shell_of(&tinted));
    let (x0, y0) = (face.w / 2 - 10, face.h / 2 - 10);
    let mut out = Vec::with_capacity((CHIP * CHIP * 4) as usize);
    for y in 0..CHIP {
        for x in 0..CHIP {
            let (sx, sy) = (x0 + x * 20 / CHIP, y0 + y * 20 / CHIP);
            let i = ((sy * face.w + sx) * 4) as usize;
            let a = face.rgba[i + 3] as u32;
            for c in 0..3 {
                let under = match backing.w == face.w {
                    true => backing.rgba[((sy * backing.w + sx) * 4) as usize + c] as u32,
                    false => 0,
                };
                out.push(((face.rgba[i + c] as u32 * a + under * (255 - a)) / 255) as u8);
            }
            out.push(255);
        }
    }
    out
}

fn rainbow() -> Vec<u8> {
    let mut out = Vec::with_capacity((CHIP * CHIP * 4) as usize);
    for y in 0..CHIP {
        for x in 0..CHIP {
            let mid = CHIP as f32 / 2.0 - 0.5;
            let angle = (y as f32 - mid).atan2(x as f32 - mid).to_degrees() + 180.0;
            let rgb = slot_ui::hue::hsl_to_rgb(angle, 0.75, 0.55);
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    out
}

fn shell_of(preset: slot_ui::Shell) -> ShellChoice {
    ShellChoice {
        outline: Outline::Auto,
        colour: preset.colour,
        finish: match preset.finish {
            Finish::Solid => ShellFinish::Solid,
            Finish::Translucent => ShellFinish::Clear,
            Finish::Glitter => ShellFinish::Glitter,
        },
    }
}

pub fn chip_atlas(cart: &Cart, ground: Option<[u8; 3]>) -> ChipAtlas {
    let presets = shell_presets();
    let n = presets.len() as u32 + 3;
    let (w, h) = (n * CHIP, 2 * CHIP);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let dark = [0x2a, 0x2a, 0x2e];
    let current = shell_of(shell_for(cart));
    let mut put = |cell: u32, row: u32, mut chip: Vec<u8>| {
        disc(&mut chip);
        for y in 0..CHIP {
            let from = (y * CHIP * 4) as usize;
            let to = (((row * CHIP + y) * w + cell * CHIP) * 4) as usize;
            rgba[to..to + (CHIP * 4) as usize]
                .copy_from_slice(&chip[from..from + (CHIP * 4) as usize]);
        }
    };
    for (i, (_, preset)) in presets.iter().enumerate() {
        put(i as u32, 0, plastic_chip(cart, shell_of(*preset)));
        put(i as u32, 1, ground_chip(preset.colour));
    }
    let tail = presets.len() as u32;
    put(tail, 0, plastic_chip(cart, current));
    put(tail, 1, ground.map_or_else(|| solid(dark), ground_chip));
    put(tail + 1, 0, solid(dark));
    put(tail + 1, 1, solid(dark));
    put(tail + 2, 0, rainbow());
    put(tail + 2, 1, rainbow());
    ChipAtlas { rgba, w, h }
}

pub enum StudioAsk {
    Preview(PreviewJob),
    Chips(Cart, Option<[u8; 3]>),
}

pub enum StudioDone {
    Preview(StudioFaces),
    Chips(ChipAtlas),
}

pub struct StudioWorker {
    jobs: Sender<StudioAsk>,
    done: Receiver<StudioDone>,
}

impl StudioWorker {
    pub fn spawn() -> StudioWorker {
        let (jobs, rx) = channel::<StudioAsk>();
        let (tx, done) = channel::<StudioDone>();
        std::thread::spawn(move || {
            let mut cache = Cache::default();
            while let Ok(first) = rx.recv() {
                let mut newest = None;
                let mut chips = Vec::new();
                for ask in std::iter::once(first).chain(rx.try_iter()) {
                    match ask {
                        StudioAsk::Preview(job) => newest = Some(job),
                        StudioAsk::Chips(cart, ground) => chips.push((cart, ground)),
                    }
                }
                for (cart, ground) in chips {
                    if tx.send(StudioDone::Chips(chip_atlas(&cart, ground))).is_err() {
                        return;
                    }
                }
                if let Some(job) = newest {
                    if tx.send(StudioDone::Preview(build(job, &mut cache))).is_err() {
                        return;
                    }
                }
            }
        });
        StudioWorker { jobs, done }
    }

    pub fn ask(&self, job: PreviewJob) {
        let _ = self.jobs.send(StudioAsk::Preview(job));
    }

    pub fn ask_chips(&self, cart: Cart, ground: Option<[u8; 3]>) {
        let _ = self.jobs.send(StudioAsk::Chips(cart, ground));
    }

    pub fn take(&self) -> Option<StudioDone> {
        self.done.try_recv().ok()
    }
}
