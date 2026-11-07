use std::io::{Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub const STAGING: &str = "System.new";
pub const MANIFEST: &str = ".manifest";
const REQUIRED: [&str; 5] = [
    "slot",
    "launch_frontend.sh",
    "mgba_libretro.so",
    "gpsp_libretro.so",
    "gambatte_libretro.so",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Slot,
    BaseOs,
}

impl Kind {
    pub fn index(self) -> usize {
        match self {
            Kind::Slot => 0,
            Kind::BaseOs => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub kind: Kind,
    pub running: String,
    pub version: String,
    pub url: String,
    pub sha256: [u8; 32],
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateFail {
    NoWifi,
    Offline,
    Clock,
    Download,
    Checksum,
    Bad,
    Space,
    Rejected(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Device {
    H700,
    RgDsPlus,
}

impl Device {
    pub fn of_target(target: &str) -> Device {
        match target {
            "rgdsplus" => Device::RgDsPlus,
            _ => Device::H700,
        }
    }

    fn zip_suffix(self) -> &'static str {
        match self {
            Device::H700 => "h700",
            Device::RgDsPlus => "rgdsplus",
        }
    }
}

pub fn parse_release(json: &str, device: Device) -> Result<Release, UpdateFail> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| UpdateFail::Bad)?;
    let stable = &v["stable"];
    let version = stable["version"]
        .as_str()
        .ok_or(UpdateFail::Bad)?
        .to_string();
    let tag = stable["tag"].as_str().ok_or(UpdateFail::Bad)?;
    let asset = match device {
        Device::H700 => stable,
        Device::RgDsPlus => &stable["rgdsplus"],
    };
    let zip = asset["zip"].as_str().ok_or(UpdateFail::Bad)?;
    if zip != format!("slot-v{version}-{}.zip", device.zip_suffix()) {
        return Err(UpdateFail::Bad);
    }
    let size = asset["size"].as_u64().ok_or(UpdateFail::Bad)?;
    let sha256 = asset["sha256"]
        .as_str()
        .and_then(hex32)
        .ok_or(UpdateFail::Bad)?;
    Ok(Release {
        kind: Kind::Slot,
        running: String::new(),
        url: format!("{RELEASES}/{tag}/{zip}"),
        version,
        sha256,
        size,
    })
}

pub fn parse_baseos(json: &str, target: &str, running: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let baseos = &v["baseos"];
    let version = baseos["version"].as_str()?;
    let tag = baseos["tag"].as_str()?;
    let file = &baseos["files"][target];
    let name = file["name"].as_str()?;
    if !plain(version) || name != format!("baseos-{target}-{version}.bosupd") {
        return None;
    }
    Some(Release {
        kind: Kind::BaseOs,
        running: running.to_string(),
        version: version.to_string(),
        url: format!("{BASEOS_RELEASES}/{tag}/{name}"),
        sha256: file["sha256"].as_str().and_then(hex32)?,
        size: file["size"].as_u64()?,
    })
}

pub fn plain(v: &str) -> bool {
    parts(v).is_some_and(|p| p.len() == 3)
}

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

fn parts(v: &str) -> Option<Vec<u64>> {
    v.split('.').map(|p| p.parse().ok()).collect()
}

pub fn newer(release: &str, running: &str) -> bool {
    match (parts(release), parts(running)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub fn verify(bytes: &[u8], release: &Release) -> Result<(), UpdateFail> {
    let got: [u8; 32] = Sha256::digest(bytes).into();
    match got == release.sha256 {
        true => Ok(()),
        false => Err(UpdateFail::Checksum),
    }
}

pub fn stage(root: &Path, zip: &[u8]) -> Result<(), UpdateFail> {
    let dir = root.join(STAGING);
    let _ = std::fs::remove_dir_all(&dir);
    let result = unpack(&dir, zip);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    result
}

fn unpack(dir: &Path, zip: &[u8]) -> Result<(), UpdateFail> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|_| UpdateFail::Bad)?;
    let mut listed = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|_| UpdateFail::Bad)?;
        let rel = match system_path(entry.name())? {
            Some(rel) if !entry.is_dir() => rel,
            _ => continue,
        };
        let out = dir.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|_| UpdateFail::Bad)?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|_| UpdateFail::Bad)?;
        std::fs::write(&out, bytes).map_err(|_| UpdateFail::Bad)?;
        listed.push(rel.to_string_lossy().replace('\\', "/"));
    }
    if REQUIRED.iter().any(|r| !listed.iter().any(|l| l == r)) {
        return Err(UpdateFail::Bad);
    }
    let mut manifest = listed.join("\n");
    manifest.push('\n');
    slot_store::atomic_write(&dir.join(MANIFEST), manifest.as_bytes()).map_err(|_| UpdateFail::Bad)
}

fn system_path(name: &str) -> Result<Option<PathBuf>, UpdateFail> {
    let path = Path::new(name);
    if path
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(UpdateFail::Bad);
    }
    let mut parts = path.components();
    let _top = parts.next();
    if parts.next().map(|c| c.as_os_str()) != Some("System".as_ref()) {
        return Ok(None);
    }
    let rel: PathBuf = parts.collect();
    Ok((!rel.as_os_str().is_empty()).then_some(rel))
}

pub fn clock_is_wrong(now_unix: u64) -> bool {
    build_day().is_some_and(|day| now_unix < day * 86_400)
}

fn build_day() -> Option<u64> {
    let mut it = env!("SLOT_BUILD_DATE")
        .split('-')
        .map(|p| p.parse::<i64>().ok());
    let (y, m, d) = (it.next()??, it.next()??, it.next()??);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    u64::try_from(era * 146_097 + doe - 719_468).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateJob {
    Check,
    Install(Release),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvent {
    Available(Vec<Release>),
    UpToDate,
    Progress(u64),
    Staged,
    Cancelled,
    Failed(UpdateFail),
}

const RUNNING: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTED: u8 = 2;

#[derive(Debug, Clone, Default)]
pub struct Ticket(Arc<AtomicU8>);

impl Ticket {
    pub fn cancel(&self) -> bool {
        self.swap(CANCELLED)
    }

    pub fn commit(&self) -> bool {
        self.swap(COMMITTED)
    }

    pub fn cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst) == CANCELLED
    }

    pub fn settle(&self, event: UpdateEvent) -> UpdateEvent {
        match self.cancelled() {
            true => UpdateEvent::Cancelled,
            false => event,
        }
    }

    fn swap(&self, to: u8) -> bool {
        self.0
            .compare_exchange(RUNNING, to, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
}

pub struct Stoppable<R> {
    inner: R,
    ticket: Ticket,
}

impl<R> Stoppable<R> {
    pub fn new(inner: R, ticket: Ticket) -> Self {
        Self { inner, ticket }
    }
}

impl<R: Read> Read for Stoppable<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.ticket.cancelled() {
            return Err(std::io::Error::other("cancelled"));
        }
        self.inner.read(buf)
    }
}

pub const WIFI_CONF: &str = "Config/wifi.conf";
pub const LATEST: &str = "https://slot-cfw.fyi/versions.json";
const RELEASES: &str = "https://github.com/slot-cfw/slot/releases/download";
const BASEOS_RELEASES: &str = "https://github.com/pvaibhav/BaseOS/releases/download";

#[derive(Debug, Clone, Copy)]
pub struct Running<'a> {
    pub slot: &'a str,
    pub baseos: Option<(&'a str, &'a str)>,
}

pub fn parse_rssi(text: &str) -> Option<i32> {
    text.lines()
        .find_map(|line| line.strip_prefix("rssi:"))
        .and_then(|value| value.trim().parse().ok())
}

pub fn signal_bars(rssi: i32) -> u8 {
    match rssi {
        r if r >= -60 => 3,
        r if r >= -72 => 2,
        _ => 1,
    }
}

#[cfg(feature = "device")]
pub fn wifi_rssi() -> Option<i32> {
    parse_rssi(&std::fs::read_to_string("/proc/net/rtl8821cs/wlan0/rx_signal").ok()?)
}

#[cfg(not(feature = "device"))]
pub fn wifi_rssi() -> Option<i32> {
    None
}

pub fn wifi_conf(root: &Path) -> bool {
    root.join(WIFI_CONF).is_file() || host_wifi()
}

#[cfg(feature = "device")]
fn host_wifi() -> bool {
    false
}

#[cfg(not(feature = "device"))]
fn host_wifi() -> bool {
    std::env::var_os("SLOT_WIFI").is_some()
}

pub fn check_with(
    now_unix: u64,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
    running: Running,
) -> UpdateEvent {
    if clock_is_wrong(now_unix) {
        return UpdateEvent::Failed(UpdateFail::Clock);
    }
    let body = match fetch(LATEST).and_then(|b| String::from_utf8(b).map_err(|_| UpdateFail::Bad)) {
        Ok(body) => body,
        Err(e) => return UpdateEvent::Failed(e),
    };
    let device = Device::of_target(running.baseos.map_or("", |(target, _)| target));
    let slot = match parse_release(&body, device) {
        Ok(r) => Release {
            running: running.slot.to_string(),
            ..r
        },
        Err(e) => return UpdateEvent::Failed(e),
    };
    let baseos = running
        .baseos
        .and_then(|(target, version)| parse_baseos(&body, target, version));
    let offers: Vec<Release> = std::iter::once(slot)
        .chain(baseos)
        .filter(|r| newer(&r.version, &r.running))
        .collect();
    match offers.is_empty() {
        true => UpdateEvent::UpToDate,
        false => UpdateEvent::Available(offers),
    }
}

pub fn install_with(
    root: &Path,
    fetch: &mut dyn FnMut(&str) -> Result<Vec<u8>, UpdateFail>,
    release: &Release,
) -> UpdateEvent {
    let staged = fetch(&release.url)
        .and_then(|bytes| match bytes.len() as u64 == release.size {
            true => Ok(bytes),
            false => Err(UpdateFail::Download),
        })
        .and_then(|bytes| verify(&bytes, release).map(|()| bytes))
        .and_then(|bytes| stage(root, &bytes));
    match staged {
        Ok(()) => UpdateEvent::Staged,
        Err(e) => UpdateEvent::Failed(e),
    }
}

pub fn read_counted(
    reader: &mut dyn Read,
    limit: u64,
    report: &mut dyn FnMut(u64),
) -> Result<Vec<u8>, UpdateFail> {
    let mut body = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|_| UpdateFail::Download)?;
        if n == 0 {
            return Ok(body);
        }
        body.extend_from_slice(&chunk[..n]);
        if body.len() as u64 > limit {
            return Err(UpdateFail::Download);
        }
        report(body.len() as u64);
    }
}

pub fn copy_counted(
    reader: &mut dyn Read,
    out: &mut dyn Write,
    limit: u64,
    report: &mut dyn FnMut(u64),
) -> Result<[u8; 32], UpdateFail> {
    let mut hash = Sha256::new();
    copy_into(reader, out, &mut hash, &mut 0, limit, report)?;
    Ok(hash.finalize().into())
}

pub fn fetch_ranged(
    open: &mut dyn FnMut(u64, u64) -> Result<Box<dyn Read>, UpdateFail>,
    size: u64,
    chunk: u64,
    out: &mut dyn Write,
    report: &mut dyn FnMut(u64),
) -> Result<[u8; 32], UpdateFail> {
    let mut hash = Sha256::new();
    let mut got = 0u64;
    while got < size {
        let to = (got + chunk).min(size) - 1;
        let mut body = open(got, to)?;
        copy_into(&mut body, out, &mut hash, &mut got, to + 1, report)?;
        if got != to + 1 {
            return Err(UpdateFail::Download);
        }
    }
    Ok(hash.finalize().into())
}

fn copy_into(
    reader: &mut dyn Read,
    out: &mut dyn Write,
    hash: &mut Sha256,
    got: &mut u64,
    limit: u64,
    report: &mut dyn FnMut(u64),
) -> Result<(), UpdateFail> {
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk).map_err(|_| UpdateFail::Download)?;
        if n == 0 {
            return Ok(());
        }
        *got += n as u64;
        if *got > limit {
            return Err(UpdateFail::Download);
        }
        out.write_all(&chunk[..n])
            .map_err(|e| match e.raw_os_error() {
                Some(libc::ENOSPC) => UpdateFail::Space,
                _ => UpdateFail::Download,
            })?;
        hash.update(&chunk[..n]);
        report(*got);
    }
}

pub fn bosupd_name(release: &Release) -> String {
    release
        .url
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_string()
}

pub fn will_apply(status: &str, name: &str) -> Result<(), String> {
    let ours = format!("/{name}");
    let mut lines = status.lines().map(str::trim);
    lines.find(|l| {
        l.strip_prefix("payload:")
            .is_some_and(|p| p.trim().ends_with(&ours))
    });
    for line in lines.take_while(|l| !l.starts_with("payload:")) {
        if line.starts_with("WILL APPLY") {
            return Ok(());
        }
        for verdict in ["skipped:", "rejected:", "cannot apply:"] {
            if let Some(why) = line.strip_prefix(verdict) {
                return Err(why.trim().to_string());
            }
        }
    }
    Err("BaseOS did not see the update".to_string())
}

pub type FetchInto<'a> = dyn FnMut(&str, &mut dyn Write) -> Result<[u8; 32], UpdateFail> + 'a;

pub fn install_baseos_with(
    root: &Path,
    free: u64,
    fetch_into: &mut FetchInto<'_>,
    status: &mut dyn FnMut() -> Option<String>,
    release: &Release,
) -> UpdateEvent {
    if free < release.size {
        return UpdateEvent::Failed(UpdateFail::Space);
    }
    let name = bosupd_name(release);
    let done = root.join(&name);
    let part = root.join(format!("{name}.part"));
    let result = download_bosupd(&part, fetch_into, release)
        .and_then(|()| std::fs::rename(&part, &done).map_err(|_| UpdateFail::Download))
        .and_then(|()| match status() {
            Some(text) => will_apply(&text, &name).map_err(UpdateFail::Rejected),
            None => Err(UpdateFail::Rejected("BaseOS did not see the update".into())),
        });
    match result {
        Ok(()) => UpdateEvent::Staged,
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            let _ = std::fs::remove_file(&done);
            UpdateEvent::Failed(e)
        }
    }
}

fn download_bosupd(
    part: &Path,
    fetch_into: &mut FetchInto<'_>,
    release: &Release,
) -> Result<(), UpdateFail> {
    let mut file = std::fs::File::create(part).map_err(|_| UpdateFail::Download)?;
    let digest = fetch_into(&release.url, &mut file)?;
    file.sync_all().map_err(|_| UpdateFail::Download)?;
    let size = file.metadata().map_err(|_| UpdateFail::Download)?.len();
    if size != release.size {
        return Err(UpdateFail::Download);
    }
    match digest == release.sha256 {
        true => Ok(()),
        false => Err(UpdateFail::Checksum),
    }
}

pub fn clean_baseos(root: &Path, target: &str, running: &str) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let prefix = format!("baseos-{target}-");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        let stale = match (
            rest.strip_suffix(".bosupd.part"),
            rest.strip_suffix(".bosupd"),
        ) {
            (Some(_), _) => true,
            (None, Some(version)) => plain(version) && !newer(version, running),
            _ => false,
        };
        if stale {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

pub trait UpdateJobs: Send {
    fn ask(&mut self, job: UpdateJob);

    fn take(&mut self) -> Option<UpdateEvent>;

    fn cancel(&mut self) -> bool {
        false
    }
}

struct NoUpdates;

impl UpdateJobs for NoUpdates {
    fn ask(&mut self, _job: UpdateJob) {}

    fn take(&mut self) -> Option<UpdateEvent> {
        None
    }
}

pub fn no_update_jobs() -> Box<dyn UpdateJobs> {
    Box::new(NoUpdates)
}

struct UpdateQueue {
    jobs: Sender<(UpdateJob, Ticket)>,
    events: Receiver<UpdateEvent>,
    ticket: Ticket,
}

impl UpdateJobs for UpdateQueue {
    fn ask(&mut self, job: UpdateJob) {
        self.ticket = Ticket::default();
        let _ = self.jobs.send((job, self.ticket.clone()));
    }

    fn take(&mut self) -> Option<UpdateEvent> {
        self.events.try_recv().ok()
    }

    fn cancel(&mut self) -> bool {
        self.ticket.cancel()
    }
}

pub fn update_jobs(root: PathBuf) -> Box<dyn UpdateJobs> {
    let (jobs, rx) = channel::<(UpdateJob, Ticket)>();
    let (tx, events) = channel::<UpdateEvent>();
    std::thread::spawn(move || {
        for (job, ticket) in rx {
            if tx
                .send(ticket.settle(run(&root, &job, &ticket, &tx)))
                .is_err()
            {
                return;
            }
        }
    });
    Box::new(UpdateQueue {
        jobs,
        events,
        ticket: Ticket::default(),
    })
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn run(root: &Path, job: &UpdateJob, ticket: &Ticket, tx: &Sender<UpdateEvent>) -> UpdateEvent {
    if clock_is_wrong(now_unix()) {
        return UpdateEvent::Failed(UpdateFail::Clock);
    }
    if let Err(e) = wait_online() {
        return UpdateEvent::Failed(e);
    }
    let event = match job {
        UpdateJob::Check => {
            let target = crate::baseos::var("BASEOS_TARGET");
            let version = crate::baseos::var("BASEOS_VERSION");
            check_with(
                now_unix(),
                &mut |url| fetch(url, API_TIMEOUT, &mut |_| {}),
                Running {
                    slot: crate::build_info::Build::current().version,
                    baseos: target.as_deref().zip(version.as_deref()),
                },
            )
        }
        UpdateJob::Install(release) => {
            let mut report = |got| {
                let _ = tx.send(UpdateEvent::Progress(got));
            };
            match release.kind {
                Kind::Slot => install_with(
                    root,
                    &mut |url| {
                        fetch_until(url, DOWNLOAD_TIMEOUT, ticket, &mut report)
                            .and_then(|bytes| committed(ticket, bytes))
                    },
                    release,
                ),
                Kind::BaseOs => install_baseos_with(
                    root,
                    free_space(root),
                    &mut |url, out| {
                        fetch_ranged(
                            &mut |from, to| {
                                open_range(url, from, to).map(|r| {
                                    Box::new(Stoppable::new(r, ticket.clone())) as Box<dyn Read>
                                })
                            },
                            release.size,
                            RANGE_BYTES,
                            out,
                            &mut report,
                        )
                        .and_then(|digest| committed(ticket, digest))
                    },
                    &mut baseos_status,
                    release,
                ),
            }
        }
    };
    event
}

#[cfg(feature = "device")]
pub(crate) fn wait_online() -> Result<(), UpdateFail> {
    crate::wifi::wait_online(ONLINE_TIMEOUT)
}

#[cfg(feature = "device")]
fn baseos_status() -> Option<String> {
    let out = std::process::Command::new("baseos-update")
        .arg("status")
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(feature = "device"))]
fn baseos_status() -> Option<String> {
    None
}

fn free_space(root: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(root.as_os_str().as_bytes()) else {
        return 0;
    };
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    match unsafe { libc::statvfs(path.as_ptr(), &mut st) } {
        0 => (st.f_bavail as u64).saturating_mul(st.f_frsize as u64),
        _ => 0,
    }
}

#[cfg(not(feature = "device"))]
pub(crate) fn wait_online() -> Result<(), UpdateFail> {
    Ok(())
}

pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "device")]
const ONLINE_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const API_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const DOWNLOAD_LIMIT: u64 = 64 * 1024 * 1024;
const RANGE_TIMEOUT: Duration = Duration::from_secs(90);
const RANGE_BYTES: u64 = 4 * 1024 * 1024;

fn committed<T>(ticket: &Ticket, value: T) -> Result<T, UpdateFail> {
    match ticket.commit() {
        true => Ok(value),
        false => Err(UpdateFail::Download),
    }
}

fn fetch_until(
    url: &str,
    timeout: Duration,
    ticket: &Ticket,
    report: &mut dyn FnMut(u64),
) -> Result<Vec<u8>, UpdateFail> {
    let body = open(url, timeout, DOWNLOAD_LIMIT)?;
    read_counted(
        &mut Stoppable::new(body, ticket.clone()),
        DOWNLOAD_LIMIT,
        report,
    )
}

pub(crate) fn fetch(url: &str, timeout: Duration, report: &mut dyn FnMut(u64)) -> Result<Vec<u8>, UpdateFail> {
    let mut body = open(url, timeout, DOWNLOAD_LIMIT)?;
    read_counted(&mut body, DOWNLOAD_LIMIT, report)
}

fn open_range(url: &str, from: u64, to: u64) -> Result<impl Read, UpdateFail> {
    let response = agent(RANGE_TIMEOUT)
        .get(url)
        .header("Range", format!("bytes={from}-{to}"))
        .call()
        .map_err(|e| request_fail(&e))?;
    if response.status() != 206 {
        return Err(UpdateFail::Download);
    }
    Ok(response
        .into_body()
        .into_with_config()
        .limit(to - from + 2)
        .reader())
}

fn open(url: &str, timeout: Duration, limit: u64) -> Result<impl Read, UpdateFail> {
    let response = agent(timeout)
        .get(url)
        .call()
        .map_err(|e| request_fail(&e))?;
    Ok(response
        .into_body()
        .into_with_config()
        .limit(limit)
        .reader())
}

pub(crate) fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_global(Some(timeout))
        .user_agent(format!(
            "slot/{}",
            crate::build_info::Build::current().version
        ))
        .build()
        .into()
}

pub(crate) fn request_fail(e: &ureq::Error) -> UpdateFail {
    let text = format!("{e:?}");
    match text.contains("Expired") || text.contains("NotValidYet") {
        true => UpdateFail::Clock,
        false => UpdateFail::Offline,
    }
}
