use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use slot_store::{Cart, Platform};

use super::index::{ArtIndex, Entry, Look};
use super::make::{self, Kind};
use slot_ui::house_label::{glance, logo_tag, looks_same};
use super::rom;
use crate::update::{self, UpdateFail};

pub const ART_BASE: &str = "https://art.slot-cfw.fyi/";
const CACHE: &str = "Cache/art";
const FETCH_THREADS: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub enum ArtJob {
    FetchMissing(Kind),
    Cancel,
    StudioFetch(Cart),
    StudioClose {
        cart: Cart,
        ground: Option<[u8; 3]>,
        look: Option<Look>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StudioArt {
    pub stem: String,
    pub real: Option<PathBuf>,
    pub logo: Option<PathBuf>,
    pub wheel: Option<PathBuf>,
    pub duplicates: bool,
    pub logo_worn: Option<[u8; 3]>,
    pub look: Option<Look>,
    pub failed: Option<UpdateFail>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtEvent {
    Counted(usize),
    Landed {
        stem: String,
        platform: Platform,
        path: PathBuf,
    },
    Missed {
        stem: String,
    },
    Done {
        landed: usize,
        missed: Vec<String>,
    },
    Failed(UpdateFail),
    StudioArt(StudioArt),
    StudioSaved(String),
}

pub enum IndexFetch {
    Fresh { body: Vec<u8>, etag: Option<String> },
    NotModified,
}

pub fn load_index(
    root: &Path,
    get: &mut dyn FnMut(Option<&str>) -> Result<IndexFetch, UpdateFail>,
) -> Result<ArtIndex, UpdateFail> {
    let dir = root.join(CACHE);
    let (json, tag) = (dir.join("index.json"), dir.join("index.etag"));
    let cached = std::fs::read_to_string(&json)
        .ok()
        .and_then(|text| ArtIndex::parse(&text));
    let etag = cached
        .as_ref()
        .and_then(|_| std::fs::read_to_string(&tag).ok());
    let from_cache = |fail: UpdateFail| cached.ok_or(fail);
    match get(etag.as_deref()) {
        Ok(IndexFetch::Fresh { body, etag }) => {
            let Some(index) = std::str::from_utf8(&body).ok().and_then(ArtIndex::parse) else {
                return from_cache(UpdateFail::Bad);
            };
            let _ = std::fs::remove_file(&tag);
            let saved = make::write_replacing(&json, &body).is_ok();
            if let (true, Some(t)) = (saved, etag) {
                let _ = make::write_replacing(&tag, t.as_bytes());
            }
            Ok(index)
        }
        Ok(IndexFetch::NotModified) => from_cache(UpdateFail::Bad),
        Err(e) => from_cache(e),
    }
}

fn other(kind: Kind) -> Kind {
    match kind {
        Kind::Real => Kind::Logo,
        Kind::Logo => Kind::Real,
    }
}

fn path_of(entry: &Entry, kind: Kind) -> Option<&str> {
    match kind {
        Kind::Real => entry.real.as_deref(),
        Kind::Logo => entry.logo.as_deref(),
    }
}

fn make_one(kind: Kind, bytes: &[u8], look: Option<&Look>, cart: &Cart) -> Option<Vec<u8>> {
    match kind {
        Kind::Real => make::real_label(bytes, cart.platform),
        Kind::Logo => make::logo_label(bytes, look, &cart.stem, cart.platform),
    }
}

fn lost(e: &UpdateFail) -> bool {
    matches!(e, UpdateFail::Offline | UpdateFail::Clock | UpdateFail::NoWifi)
}

fn got(
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
    url: &str,
) -> Result<Option<Vec<u8>>, UpdateFail> {
    match fetch(url) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if lost(&e) => Err(e),
        Err(_) => Ok(None),
    }
}

fn image(
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
    path: &str,
) -> Result<Option<Vec<u8>>, UpdateFail> {
    if let Some(stem) = path.strip_suffix(".png") {
        if let Some(bytes) = got(fetch, &format!("{ART_BASE}{stem}.webp"))? {
            return Ok(Some(bytes));
        }
    }
    got(fetch, &format!("{ART_BASE}{path}"))
}

fn label_for(
    cart: &Cart,
    kind: Kind,
    index: &ArtIndex,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
) -> Result<Option<Vec<u8>>, UpdateFail> {
    let Ok(crc) = rom::rom_crc(&cart.rom) else {
        return Ok(None);
    };
    let Some(entry) = index.get(crc) else {
        return Ok(None);
    };
    for k in [kind, other(kind)] {
        let Some(path) = path_of(entry, k) else {
            continue;
        };
        let Some(bytes) = image(fetch, path)? else {
            continue;
        };
        if let Some(png) = make_one(k, &bytes, entry.look.as_ref(), cart) {
            return Ok(Some(png));
        }
    }
    Ok(None)
}

enum Outcome {
    Landed(Cart, PathBuf),
    Kept,
    Missed(String),
    Lost(UpdateFail),
}

fn one_cart(
    root: &Path,
    cart: Cart,
    kind: Kind,
    index: &ArtIndex,
    fetch: &(dyn Fn(&str) -> Result<Vec<u8>, UpdateFail> + Sync),
) -> Outcome {
    let path = make::label_path(root, &cart);
    let spent = std::cell::Cell::new(std::time::Duration::ZERO);
    let mut timed = |url: &str| {
        let at = std::time::Instant::now();
        let out = fetch(url);
        spent.set(spent.get() + at.elapsed());
        out
    };
    let began = std::time::Instant::now();
    let made = match label_for(&cart, kind, index, &mut timed) {
        Ok(made) => made,
        Err(e) => return Outcome::Lost(e),
    };
    let made_in = began.elapsed();
    let written = made.map(|png| make::write_new(&path, &png));
    eprintln!(
        "slot: art: {} download {} ms, hash and make {} ms, write {} ms",
        cart.stem,
        spent.get().as_millis(),
        made_in.saturating_sub(spent.get()).as_millis(),
        began.elapsed().saturating_sub(made_in).as_millis(),
    );
    match written {
        Some(Ok(true)) => {
            let mut labeled = cart.clone();
            labeled.label = Some(path.clone());
            let _ = crate::label_cache::label_art(root, &labeled);
            Outcome::Landed(cart, path)
        }
        Some(Ok(false)) => Outcome::Kept,
        Some(Err(_)) | None => Outcome::Missed(cart.stem),
    }
}

pub fn fetch_one(root: &Path, cart: Cart, index: &ArtIndex, agent: &ureq::Agent) -> bool {
    let outcome = one_cart(root, cart, Kind::Real, index, &|url| fetch_art(agent, url));
    matches!(outcome, Outcome::Landed(..))
}

pub fn load_art_index(root: &Path) -> Result<ArtIndex, UpdateFail> {
    load_index(root, &mut get_index)
}

pub fn fetch_missing_with(
    root: &Path,
    kind: Kind,
    index: &ArtIndex,
    fetch: &(dyn Fn(&str) -> Result<Vec<u8>, UpdateFail> + Sync),
    cancelled: &(dyn Fn() -> bool + Sync),
    emit: &mut dyn FnMut(ArtEvent),
) {
    let carts = rom::missing_art(root);
    emit(ArtEvent::Counted(carts.len()));
    let next = AtomicUsize::new(0);
    let lost = AtomicBool::new(false);
    let (tx, rx) = channel::<Outcome>();
    let (mut landed, mut missed) = (0, Vec::new());
    let mut failed = None;
    std::thread::scope(|scope| {
        for _ in 0..FETCH_THREADS {
            let (tx, carts, next, lost) = (tx.clone(), &carts, &next, &lost);
            scope.spawn(move || loop {
                if cancelled() || lost.load(Ordering::SeqCst) {
                    return;
                }
                let Some(cart) = carts.get(next.fetch_add(1, Ordering::SeqCst)).cloned() else {
                    return;
                };
                let outcome = one_cart(root, cart, kind, index, fetch);
                if matches!(outcome, Outcome::Lost(_)) {
                    lost.store(true, Ordering::SeqCst);
                }
                if tx.send(outcome).is_err() {
                    return;
                }
            });
        }
        drop(tx);
        for outcome in rx {
            match outcome {
                Outcome::Landed(cart, path) => {
                    landed += 1;
                    emit(ArtEvent::Landed {
                        stem: cart.stem,
                        platform: cart.platform,
                        path,
                    });
                }
                Outcome::Kept => {}
                Outcome::Missed(stem) => {
                    missed.push(stem.clone());
                    emit(ArtEvent::Missed { stem });
                }
                Outcome::Lost(e) => failed = Some(e),
            }
        }
    });
    match failed {
        Some(e) => emit(ArtEvent::Failed(e)),
        None => emit(ArtEvent::Done { landed, missed }),
    }
}

pub fn studio_art_with(
    root: &Path,
    cart: &Cart,
    index: &ArtIndex,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
) -> StudioArt {
    let mut art = StudioArt {
        stem: cart.stem.clone(),
        ..StudioArt::default()
    };
    let Ok(crc) = rom::rom_crc(&cart.rom) else {
        return art;
    };
    let entry = index.get(crc);
    art.look = entry.and_then(|e| e.look.clone());
    let dir = root.join(make::STUDIO_CACHE);
    let label = make::label_path(root, cart);
    let worn = std::fs::read(&label).ok();
    let worn_glance = worn.as_deref().and_then(glance);
    let worn_tagged = worn.as_deref().is_some_and(|png| logo_tag(png).is_some());
    let mut seen: Vec<Vec<u8>> = worn_glance.iter().cloned().collect();
    for kind in [Kind::Real, Kind::Logo] {
        let mut bytes = None;
        if let Some(path) = entry.and_then(|e| path_of(e, kind)) {
            match image(fetch, path) {
                Ok(b) => bytes = b,
                Err(e) => {
                    art.failed = Some(e);
                    return art;
                }
            }
        }
        let Some(bytes) = bytes else {
            continue;
        };
        let Some(png) = make_one(kind, &bytes, entry.and_then(|e| e.look.as_ref()), cart) else {
            continue;
        };
        let name = match kind {
            Kind::Real => "real.png",
            Kind::Logo => "logo.png",
        };
        if kind == Kind::Logo {
            let wheel = dir.join("wheel.png");
            let written = make::as_png(&bytes).map(|png| make::write_replacing(&wheel, &png));
            if matches!(written, Some(Ok(()))) {
                art.wheel = Some(wheel);
            }
        }
        let Some(look) = glance(&png) else {
            continue;
        };
        let worn_it = worn_glance.as_ref().is_some_and(|w| looks_same(w, &look));
        if kind == Kind::Logo && worn_it && !worn_tagged {
            let _ = make::relabel(root, cart, &png);
            art.logo_worn = logo_tag(&png);
        }
        if seen.iter().any(|s| looks_same(s, &look)) {
            art.duplicates = true;
            continue;
        }
        seen.push(look);
        let out = dir.join(name);
        if make::write_replacing(&out, &png).is_err() {
            continue;
        }
        match kind {
            Kind::Real => art.real = Some(out),
            Kind::Logo => art.logo = Some(out),
        }
    }
    art
}

pub trait ArtJobs: Send {
    fn ask(&mut self, job: ArtJob);

    fn take(&mut self) -> Option<ArtEvent>;
}

struct NoArt;

impl ArtJobs for NoArt {
    fn ask(&mut self, _job: ArtJob) {}

    fn take(&mut self) -> Option<ArtEvent> {
        None
    }
}

pub fn no_art_jobs() -> Box<dyn ArtJobs> {
    Box::new(NoArt)
}

#[derive(Default)]
pub struct Runs {
    current: Option<Arc<AtomicBool>>,
}

impl Runs {
    pub fn start(&mut self) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.current = Some(flag.clone());
        flag
    }

    pub fn cancel(&mut self) {
        if let Some(flag) = &self.current {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

enum Work {
    Fetch(Kind, Arc<AtomicBool>),
    Studio(Cart),
    Close(Cart, Option<[u8; 3]>, Option<Look>),
}

struct ArtQueue {
    jobs: Sender<Work>,
    events: Receiver<ArtEvent>,
    runs: Runs,
}

impl ArtJobs for ArtQueue {
    fn ask(&mut self, job: ArtJob) {
        match job {
            ArtJob::FetchMissing(kind) => {
                let _ = self.jobs.send(Work::Fetch(kind, self.runs.start()));
            }
            ArtJob::StudioFetch(cart) => {
                let _ = self.jobs.send(Work::Studio(cart));
            }
            ArtJob::StudioClose { cart, ground, look } => {
                let _ = self.jobs.send(Work::Close(cart, ground, look));
            }
            ArtJob::Cancel => self.runs.cancel(),
        }
    }

    fn take(&mut self) -> Option<ArtEvent> {
        self.events.try_recv().ok()
    }
}

pub fn art_jobs(root: PathBuf) -> Box<dyn ArtJobs> {
    let (jobs, rx) = channel::<Work>();
    let (tx, events) = channel::<ArtEvent>();
    std::thread::spawn(move || {
        for work in rx {
            match work {
                Work::Fetch(kind, flag) => {
                    if flag.load(Ordering::SeqCst) {
                        let _ = tx.send(ArtEvent::Done { landed: 0, missed: Vec::new() });
                    } else {
                        run(&root, kind, &flag, &tx);
                    }
                }
                Work::Studio(cart) => {
                    let _ = tx.send(ArtEvent::StudioArt(studio_run(&root, &cart)));
                }
                Work::Close(cart, ground, look) => {
                    studio_save(&root, &cart, ground, look.as_ref());
                    let _ = tx.send(ArtEvent::StudioSaved(cart.stem));
                }
            }
        }
    });
    Box::new(ArtQueue {
        jobs,
        events,
        runs: Runs::default(),
    })
}

pub fn studio_save(root: &Path, cart: &Cart, ground: Option<[u8; 3]>, look: Option<&Look>) {
    let dir = root.join(make::STUDIO_CACHE);
    if let Ok(wheel) = std::fs::read(dir.join("wheel.png")) {
        let look = match ground {
            Some(ground) => Some(Look { ground, band: look.and_then(|l| l.band) }),
            None => look.cloned(),
        };
        if let Some(png) = make::logo_label(&wheel, look.as_ref(), &cart.stem, cart.platform) {
            let _ = make::relabel(root, cart, &png);
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

fn studio_run(root: &Path, cart: &Cart) -> StudioArt {
    let failed = |e| StudioArt {
        stem: cart.stem.clone(),
        failed: Some(e),
        ..StudioArt::default()
    };
    if update::clock_is_wrong(update::now_unix()) {
        return failed(UpdateFail::Clock);
    }
    let _ = std::fs::remove_dir_all(root.join(make::STUDIO_CACHE));
    if let Err(e) = update::wait_online() {
        return failed(e);
    }
    let index = match load_index(root, &mut get_index) {
        Ok(index) => index,
        Err(e) => return failed(e),
    };
    let agent = update::agent(update::DOWNLOAD_TIMEOUT);
    studio_art_with(root, cart, &index, &mut |url| fetch_art(&agent, url))
}

fn run(root: &Path, kind: Kind, cancel: &AtomicBool, tx: &Sender<ArtEvent>) {
    if update::clock_is_wrong(update::now_unix()) {
        let _ = tx.send(ArtEvent::Failed(UpdateFail::Clock));
        return;
    }
    if let Err(e) = update::wait_online() {
        let _ = tx.send(ArtEvent::Failed(e));
        return;
    }
    let agent = update::agent(update::DOWNLOAD_TIMEOUT);
    match load_index(root, &mut get_index) {
        Ok(index) => fetch_missing_with(
            root,
            kind,
            &index,
            &|url| fetch_art(&agent, url),
            &|| cancel.load(Ordering::SeqCst),
            &mut |e| {
                let _ = tx.send(e);
            },
        ),
        Err(e) => {
            let _ = tx.send(ArtEvent::Failed(e));
        }
    }
}

pub fn fetch_label(root: &Path, cart: &Cart) -> Result<bool, UpdateFail> {
    if update::clock_is_wrong(update::now_unix()) {
        return Err(UpdateFail::Clock);
    }
    let agent = update::agent(update::DOWNLOAD_TIMEOUT);
    let index = load_index(root, &mut get_index)?;
    let made = label_for(cart, Kind::Real, &index, &mut |url| fetch_art(&agent, url))?;
    let Some(png) = made else {
        return Ok(false);
    };
    let path = make::label_path(root, cart);
    make::write_replacing(&path, &png).map_err(|_| UpdateFail::Bad)?;
    let mut labeled = cart.clone();
    labeled.label = Some(path);
    let _ = crate::label_cache::label_art(root, &labeled);
    Ok(true)
}

fn fetch_art(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, UpdateFail> {
    let response = match agent.get(url).call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(_)) => return Err(UpdateFail::Download),
        Err(e) => return Err(update::request_fail(&e)),
    };
    let mut body = Vec::new();
    response
        .into_body()
        .into_with_config()
        .limit(update::DOWNLOAD_LIMIT)
        .reader()
        .read_to_end(&mut body)
        .map_err(|_| UpdateFail::Offline)?;
    Ok(body)
}

fn get_index(etag: Option<&str>) -> Result<IndexFetch, UpdateFail> {
    let mut request = update::agent(update::API_TIMEOUT).get(format!("{ART_BASE}index.json"));
    if let Some(t) = etag {
        request = request.header("If-None-Match", t);
    }
    let response = match request.call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(304)) => return Ok(IndexFetch::NotModified),
        Err(e) => return Err(update::request_fail(&e)),
    };
    if response.status() == 304 {
        return Ok(IndexFetch::NotModified);
    }
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let mut body = Vec::new();
    response
        .into_body()
        .into_with_config()
        .limit(update::DOWNLOAD_LIMIT)
        .reader()
        .read_to_end(&mut body)
        .map_err(|_| UpdateFail::Download)?;
    Ok(IndexFetch::Fresh { body, etag })
}
