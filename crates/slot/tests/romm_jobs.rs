use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slot::romm::api::{Api, ApiError, Cancel, Install, Outcome, Pending, Poll, RomInfo};
use slot::romm::install::TEMP_DIR;
use slot::romm::jobs::{spawn, Dial, Label, Runner};
use slot::romm::socket::Source;
use slot::romm::{Push, RommConf, RommEvent, RommJob, RommJobs};
use slot_store::{Cart, Platform};
use tempfile::{tempdir, TempDir};

const NOTE: &str = "Installs off on server";

struct State {
    claims: VecDeque<Result<Vec<Install>, ApiError>>,
    exists: Result<bool, ApiError>,
    delete: Result<(), ApiError>,
    downloads: VecDeque<ApiError>,
    rom: Option<ApiError>,
    report: Option<ApiError>,
    enable: VecDeque<Result<(), ApiError>>,
    enabled: usize,
    claimed: usize,
    fetched: Vec<u64>,
    reports: Vec<(String, Outcome)>,
    deleted: usize,
    hold: bool,
    held: bool,
    stall: bool,
    stalled: bool,
}

#[derive(Clone)]
struct Fake(Arc<Mutex<State>>);

fn fake() -> Fake {
    Fake(Arc::new(Mutex::new(State {
        claims: VecDeque::new(),
        exists: Ok(true),
        delete: Ok(()),
        downloads: VecDeque::new(),
        rom: None,
        report: None,
        enable: VecDeque::new(),
        enabled: 0,
        claimed: 0,
        fetched: Vec::new(),
        reports: Vec::new(),
        deleted: 0,
        hold: false,
        held: false,
        stall: false,
        stalled: false,
    })))
}

impl Fake {
    fn claims(&self, list: Vec<Result<Vec<Install>, ApiError>>) {
        self.0.lock().unwrap().claims = list.into();
    }

    fn reports(&self) -> Vec<(String, Outcome)> {
        self.0.lock().unwrap().reports.clone()
    }

    fn fetched(&self) -> Vec<u64> {
        self.0.lock().unwrap().fetched.clone()
    }
}

impl Api for Fake {
    fn version(&self) -> Result<String, ApiError> {
        Ok("5.4.0".into())
    }

    fn pair_start(&self, _conf: &RommConf) -> Result<Pending, ApiError> {
        Err(ApiError::Offline)
    }

    fn pair_poll(&self, _device_code: &str) -> Result<Poll, ApiError> {
        Err(ApiError::Offline)
    }

    fn enable_install(&self, _device_id: &str, _token: &str) -> Result<(), ApiError> {
        let mut s = self.0.lock().unwrap();
        s.enabled += 1;
        s.enable.pop_front().unwrap_or(Ok(()))
    }

    fn claim(&self) -> Result<Vec<Install>, ApiError> {
        let stall = {
            let mut s = self.0.lock().unwrap();
            s.claimed += 1;
            s.stalled = s.stall;
            s.stall
        };
        while stall && self.0.lock().unwrap().stall {
            std::thread::sleep(Duration::from_millis(1));
        }
        let mut s = self.0.lock().unwrap();
        s.claims.pop_front().unwrap_or(Ok(Vec::new()))
    }

    fn device_exists(&self) -> Result<bool, ApiError> {
        self.0.lock().unwrap().exists.clone()
    }

    fn rom(&self, id: u64) -> Result<RomInfo, ApiError> {
        if let Some(e) = self.0.lock().unwrap().rom.clone() {
            return Err(e);
        }
        Ok(RomInfo {
            platform_slug: "gb".into(),
            fs_name: format!("Game{id}.gb"),
            files: Vec::new(),
        })
    }

    fn download(
        &self,
        rom: u64,
        _name: &str,
        _file_ids: &[u64],
        to: &Path,
        cancel: &Cancel,
    ) -> Result<(), ApiError> {
        std::fs::write(to, b"rom").unwrap();
        let hold = {
            let mut s = self.0.lock().unwrap();
            s.fetched.push(rom);
            if let Some(e) = s.downloads.pop_front() {
                return Err(e);
            }
            s.held = s.hold;
            s.hold
        };
        while hold && !cancel.is_set() {
            std::thread::sleep(Duration::from_millis(1));
        }
        match hold {
            true => Err(ApiError::Cancelled),
            false => Ok(()),
        }
    }

    fn report(&self, id: &str, outcome: &Outcome) -> Result<(), ApiError> {
        let mut s = self.0.lock().unwrap();
        s.reports.push((id.to_string(), outcome.clone()));
        match s.report.clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    fn delete_device(&self) -> Result<(), ApiError> {
        let mut s = self.0.lock().unwrap();
        s.deleted += 1;
        s.delete.clone()
    }
}

fn conf() -> RommConf {
    RommConf {
        server: "romm.local".into(),
        device_id: Some("d1".into()),
        token: Some("t1".into()),
        ..RommConf::default()
    }
}

fn req(id: &str, rom: u64) -> Install {
    Install {
        id: id.into(),
        rom_id: rom,
        file_ids: vec![1],
    }
}

fn no_label(_: &Path, _: &Cart) -> bool {
    false
}

fn card() -> TempDir {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    d
}

fn runner_with(root: &Path, api: &Fake, label: Label) -> Runner {
    let api = api.clone();
    let mut runner = Runner::new(
        PathBuf::from(root),
        Box::new(move |_| Box::new(api.clone())),
        label,
    );
    runner.start(&conf());
    runner
}

fn runner(root: &Path, api: &Fake) -> Runner {
    runner_with(root, api, no_label)
}

fn poll(runner: &mut Runner) -> Vec<RommEvent> {
    poll_all(runner)
        .into_iter()
        .filter(|e| !matches!(e, RommEvent::Online(_)))
        .collect()
}

fn reach(runner: &mut Runner) -> Vec<bool> {
    poll_all(runner)
        .into_iter()
        .filter_map(|e| match e {
            RommEvent::Online(up) => Some(up),
            _ => None,
        })
        .collect()
}

fn poll_all(runner: &mut Runner) -> Vec<RommEvent> {
    let mut events = Vec::new();
    let mut emit = |e: RommEvent| events.push(e);
    if let Some(task) = runner.claim(&mut emit) {
        let id = task.id().to_string();
        let result = task.run(&mut emit);
        runner.ran(&id, result, &mut emit);
    }
    events
}

fn temps(root: &Path) -> usize {
    std::fs::read_dir(root.join(TEMP_DIR)).unwrap().count()
}

#[test]
fn claim_installs_oldest_first_and_reports_done() {
    let d = card();
    let api = fake();
    api.claims(vec![
        Ok(vec![req("r1", 1), req("r2", 2)]),
        Ok(vec![req("r2", 2)]),
    ]);
    let mut r = runner(d.path(), &api);
    let events = poll(&mut r);
    assert_eq!(api.fetched(), vec![1]);
    assert_eq!(api.reports(), vec![("r1".to_string(), Outcome::Done)]);
    assert!(d.path().join("Games/GB/Game1.gb").exists());
    let [RommEvent::Installed {
        carts,
        platform,
        stem,
    }] = events.as_slice()
    else {
        panic!("{events:?}");
    };
    assert_eq!(*platform, Platform::Gb);
    assert_eq!(stem, "Game1");
    assert!(carts.iter().any(|c| c.stem == "Game1"));
    poll(&mut r);
    assert_eq!(api.fetched(), vec![1, 2]);
    assert_eq!(api.reports()[1], ("r2".to_string(), Outcome::Done));
}

#[test]
fn a_request_seen_again_is_not_started_twice() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]), Ok(vec![req("r1", 1)])]);
    let mut r = runner(d.path(), &api);
    let mut events = Vec::new();
    let mut emit = |e: RommEvent| events.push(e);
    let first = r.claim(&mut emit).expect("a task");
    assert!(r.claim(&mut emit).is_none());
    let result = first.run(&mut emit);
    r.ran("r1", result, &mut emit);
    assert_eq!(api.fetched(), vec![1]);
    assert_eq!(api.reports().len(), 1);
}

#[test]
fn a_second_request_waits_for_the_running_one() {
    let d = card();
    let api = fake();
    api.claims(vec![
        Ok(vec![req("r1", 1)]),
        Ok(vec![req("r1", 1), req("r2", 2)]),
    ]);
    let mut r = runner(d.path(), &api);
    let mut emit = |_: RommEvent| {};
    let _first = r.claim(&mut emit).expect("a task");
    assert!(r.claim(&mut emit).is_none());
}

#[test]
fn an_id_gone_from_claim_cancels_it() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]), Ok(Vec::new())]);
    let mut r = runner(d.path(), &api);
    let mut emit = |_: RommEvent| {};
    let task = r.claim(&mut emit).expect("a task");
    assert!(r.claim(&mut emit).is_none());
    let result = task.run(&mut emit);
    r.ran("r1", result, &mut emit);
    assert!(api.reports().is_empty());
    assert!(!d.path().join("Games/GB/Game1.gb").exists());
    assert_eq!(temps(d.path()), 0);
}

#[test]
fn stop_cancels_and_the_next_claim_starts_it_again() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]), Ok(vec![req("r1", 1)])]);
    let mut r = runner(d.path(), &api);
    let mut emit = |_: RommEvent| {};
    let task = r.claim(&mut emit).expect("a task");
    r.stop();
    let result = task.run(&mut emit);
    r.ran("r1", result, &mut emit);
    assert!(api.reports().is_empty());
    poll(&mut r);
    assert_eq!(api.reports(), vec![("r1".to_string(), Outcome::Done)]);
}

#[test]
fn five_hundred_retries_three_times_then_fails() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]); 4]);
    api.0.lock().unwrap().downloads = vec![ApiError::Server(500); 4].into();
    let mut r = runner(d.path(), &api);
    poll(&mut r);
    poll(&mut r);
    assert!(api.reports().is_empty());
    poll(&mut r);
    assert_eq!(api.fetched().len(), 3);
    assert_eq!(
        api.reports(),
        vec![(
            "r1".to_string(),
            Outcome::Failed("Download kept failing".into())
        )]
    );
    assert_eq!(temps(d.path()), 0);
}

#[test]
fn offline_keeps_the_request_with_no_report() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]); 5]);
    api.0.lock().unwrap().downloads = vec![ApiError::Offline; 4].into();
    let mut r = runner(d.path(), &api);
    for _ in 0..4 {
        assert!(poll(&mut r).is_empty());
    }
    assert!(api.reports().is_empty());
    assert_eq!(temps(d.path()), 0);
    poll(&mut r);
    assert_eq!(api.reports(), vec![("r1".to_string(), Outcome::Done)]);
}

#[test]
fn a_rom_gone_from_the_server_fails_plainly() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().rom = Some(ApiError::NotFound);
    let mut r = runner(d.path(), &api);
    assert!(poll(&mut r).is_empty());
    assert_eq!(
        api.reports(),
        vec![(
            "r1".to_string(),
            Outcome::Failed("Game is gone from the server".into())
        )]
    );
}

#[test]
fn a_401_while_installing_unpairs() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().rom = Some(ApiError::Unauthorized);
    let mut r = runner(d.path(), &api);
    assert_eq!(poll(&mut r), vec![RommEvent::Unpaired]);
    assert!(api.reports().is_empty());
}

#[test]
fn a_failed_report_leaves_the_installed_event_sent() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)]), Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().report = Some(ApiError::Offline);
    let mut r = runner(d.path(), &api);
    let events = poll(&mut r);
    assert!(matches!(events.as_slice(), [RommEvent::Installed { .. }]));
    api.0.lock().unwrap().report = None;
    assert!(poll(&mut r).is_empty());
    assert_eq!(
        api.reports().last(),
        Some(&("r1".to_string(), Outcome::AlreadyInstalled))
    );
}

#[test]
fn claim_401_unpairs() {
    let d = card();
    let api = fake();
    api.claims(vec![Err(ApiError::Unauthorized)]);
    let mut r = runner(d.path(), &api);
    assert_eq!(poll(&mut r), vec![RommEvent::Unpaired]);
    assert!(poll(&mut r).is_empty());
}

#[test]
fn claim_404_with_device_gone_unpairs() {
    let d = card();
    let api = fake();
    api.claims(vec![Err(ApiError::NotFound)]);
    api.0.lock().unwrap().exists = Ok(false);
    let mut r = runner(d.path(), &api);
    assert_eq!(poll(&mut r), vec![RommEvent::Unpaired]);
}

#[test]
fn claim_404_with_device_present_keeps_pairing() {
    let d = card();
    let api = fake();
    api.claims(vec![
        Err(ApiError::NotFound),
        Err(ApiError::NotFound),
        Err(ApiError::NotFound),
        Ok(Vec::new()),
        Err(ApiError::NotFound),
    ]);
    let mut r = runner(d.path(), &api);
    assert_eq!(poll(&mut r), vec![RommEvent::Note(NOTE.into())]);
    assert!(poll(&mut r).is_empty());
    assert!(poll(&mut r).is_empty());
    assert_eq!(poll(&mut r), vec![RommEvent::NoteCleared]);
    assert_eq!(poll(&mut r), vec![RommEvent::Note(NOTE.into())]);
}

#[test]
fn claim_404_with_the_check_failing_keeps_pairing() {
    let d = card();
    let api = fake();
    api.claims(vec![Err(ApiError::NotFound)]);
    api.0.lock().unwrap().exists = Err(ApiError::Server(500));
    let mut r = runner(d.path(), &api);
    assert_eq!(poll(&mut r), vec![RommEvent::Note(NOTE.into())]);
}

#[test]
fn offline_is_silent() {
    let d = card();
    let api = fake();
    api.claims(vec![Err(ApiError::Offline), Err(ApiError::NotFound)]);
    api.0.lock().unwrap().exists = Err(ApiError::Offline);
    let mut r = runner(d.path(), &api);
    assert!(poll(&mut r).is_empty());
    assert!(poll(&mut r).is_empty());
}

#[test]
fn a_failed_start_enables_on_the_next_claim() {
    let d = card();
    let api = fake();
    api.0.lock().unwrap().enable = vec![Err(ApiError::Offline)].into();
    let mut r = runner(d.path(), &api);
    assert_eq!(api.0.lock().unwrap().enabled, 1);
    poll(&mut r);
    poll(&mut r);
    assert_eq!(api.0.lock().unwrap().enabled, 2);
}

#[test]
fn disconnect_deletes_device_first_then_clears() {
    let d = card();
    let api = fake();
    let mut r = runner(d.path(), &api);
    let probe = api.clone();
    let mut seen = None;
    r.disconnect(&conf(), &mut |e| {
        seen = Some((e, probe.0.lock().unwrap().deleted));
    });
    assert_eq!(seen, Some((RommEvent::Unpaired, 1)));
}

#[test]
fn disconnect_clears_even_if_delete_fails() {
    let d = card();
    let api = fake();
    api.0.lock().unwrap().delete = Err(ApiError::Offline);
    let mut r = runner(d.path(), &api);
    let mut events = Vec::new();
    r.disconnect(&conf(), &mut |e| events.push(e));
    assert_eq!(events, vec![RommEvent::Unpaired]);
}

fn until(what: &str, mut done: impl FnMut() -> bool) {
    let began = Instant::now();
    while !done() {
        assert!(began.elapsed() < Duration::from_secs(10), "{what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn disconnect_mid_download_leaves_no_temp_and_no_report() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().hold = true;
    let made = api.clone();
    let mut jobs = spawn(Runner::new(
        PathBuf::from(d.path()),
        Box::new(move |_| Box::new(made.clone())),
        no_label,
    ));
    jobs.ask(RommJob::Start(conf()));
    jobs.ask(RommJob::Claim);
    until("the download never began", || api.0.lock().unwrap().held);
    assert_eq!(temps(d.path()), 1);
    jobs.ask(RommJob::Disconnect(conf()));
    until("no Unpaired", || jobs.take() == Some(RommEvent::Unpaired));
    until("the temp stayed", || temps(d.path()) == 0);
    assert!(api.reports().is_empty());
    assert_eq!(api.0.lock().unwrap().deleted, 1);
    assert!(!d.path().join("Games/GB/Game1.gb").exists());
}

#[test]
fn no_label_still_installs() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    let mut r = runner(d.path(), &api);
    let events = poll(&mut r);
    assert!(matches!(events.as_slice(), [RommEvent::Installed { .. }]));
    assert_eq!(api.reports(), vec![("r1".to_string(), Outcome::Done)]);
}

fn landing_label(root: &Path, cart: &Cart) -> bool {
    let path = slot::art::make::label_path(root, cart);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"png").is_ok()
}

#[test]
fn a_landed_label_is_in_the_carts_sent() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    let mut r = runner_with(d.path(), &api, landing_label);
    let events = poll(&mut r);
    let [RommEvent::Installed { carts, .. }] = events.as_slice() else {
        panic!("{events:?}");
    };
    let cart = carts.iter().find(|c| c.stem == "Game1").unwrap();
    assert!(cart.label.is_some());
}

struct Pushes {
    rx: Receiver<Push>,
    alive: Arc<Mutex<usize>>,
}

impl Drop for Pushes {
    fn drop(&mut self) {
        *self.alive.lock().unwrap() -= 1;
    }
}

impl Source for Pushes {
    fn read(&mut self) -> Result<Option<Push>, ApiError> {
        match self.rx.recv_timeout(Duration::from_millis(5)) {
            Ok(push) => Ok(Some(push)),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(ApiError::Offline),
        }
    }
}

#[derive(Clone, Default)]
struct Line {
    pushes: Arc<Mutex<Vec<Sender<Push>>>>,
    alive: Arc<Mutex<usize>>,
    refused: Arc<Mutex<bool>>,
    dials: Arc<Mutex<usize>>,
}

impl Line {
    fn refused() -> Line {
        let line = Line::default();
        *line.refused.lock().unwrap() = true;
        line
    }

    fn dial(&self) -> Dial {
        let line = self.clone();
        Arc::new(move |_| {
            *line.dials.lock().unwrap() += 1;
            if *line.refused.lock().unwrap() {
                return Err(ApiError::Offline);
            }
            let (tx, rx) = channel();
            line.pushes.lock().unwrap().push(tx);
            *line.alive.lock().unwrap() += 1;
            Ok(Box::new(Pushes {
                rx,
                alive: line.alive.clone(),
            }))
        })
    }

    fn push(&self, push: Push) {
        self.pushes
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .send(push)
            .unwrap();
    }

    fn dials(&self) -> usize {
        *self.dials.lock().unwrap()
    }

    fn alive(&self) -> usize {
        *self.alive.lock().unwrap()
    }
}

fn claimed(api: &Fake) -> usize {
    api.0.lock().unwrap().claimed
}

fn worker(root: &Path, api: &Fake, line: &Line) -> Box<dyn RommJobs> {
    let made = api.clone();
    let mut jobs = spawn(
        Runner::new(
            PathBuf::from(root),
            Box::new(move |_| Box::new(made.clone())),
            no_label,
        )
        .with_dial(line.dial()),
    );
    jobs.ask(RommJob::Start(conf()));
    jobs
}

fn settle() {
    std::thread::sleep(Duration::from_millis(60));
}

fn linked(root: &Path, api: &Fake, line: &Line) -> Box<dyn RommJobs> {
    let mut jobs = worker(root, api, line);
    jobs.ask(RommJob::Claim);
    until("no claim on connect", || claimed(api) == 1);
    jobs
}

#[test]
fn connecting_claims_once() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let _jobs = linked(d.path(), &api, &line);
    settle();
    assert_eq!(claimed(&api), 1);
    assert_eq!(line.dials(), 1);
}

#[test]
fn a_queued_push_claims_at_once() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let _jobs = linked(d.path(), &api, &line);
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    line.push(Push::Queued);
    until("no install", || api.reports().len() == 1);
    assert_eq!(api.fetched(), vec![1]);
}

#[test]
fn a_cancelled_push_stops_only_that_id() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().hold = true;
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("the download never began", || api.0.lock().unwrap().held);
    line.push(Push::Cancelled("other".into()));
    settle();
    assert_eq!(temps(d.path()), 1);
    assert_eq!(claimed(&api), 1);
    line.push(Push::Cancelled("r1".into()));
    until("the temp stayed", || temps(d.path()) == 0);
    until("no claim after the cancel", || claimed(&api) == 2);
    assert!(api.reports().is_empty());
}

#[test]
fn while_the_socket_is_up_ticks_do_not_claim() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let mut jobs = linked(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    jobs.ask(RommJob::Claim);
    settle();
    assert_eq!(claimed(&api), 1);
    assert_eq!(line.dials(), 1);
}

#[test]
fn a_failed_enable_retries_on_a_tick_with_the_socket_up() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.0.lock().unwrap().enable = vec![
        Err(ApiError::Offline),
        Err(ApiError::Offline),
        Err(ApiError::Offline),
    ]
    .into();
    let mut jobs = linked(d.path(), &api, &line);
    until("enable did not retry on the claim", || {
        api.0.lock().unwrap().enabled == 2
    });
    jobs.ask(RommJob::Claim);
    until("no retry", || api.0.lock().unwrap().enabled == 3);
    jobs.ask(RommJob::Claim);
    until("no retry", || api.0.lock().unwrap().enabled == 4);
    jobs.ask(RommJob::Claim);
    settle();
    assert_eq!(api.0.lock().unwrap().enabled, 4);
    assert_eq!(claimed(&api), 1);
}

#[test]
fn a_failed_download_is_claimed_on_the_next_tick_with_the_socket_up() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![Ok(vec![req("r1", 1)]), Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().downloads = vec![ApiError::Offline].into();
    let mut jobs = linked(d.path(), &api, &line);
    until("no attempt", || api.fetched().len() == 1);
    settle();
    assert_eq!(claimed(&api), 1);
    jobs.ask(RommJob::Claim);
    until("no install", || api.reports().len() == 1);
}

#[test]
fn a_failed_claim_is_repeated_on_the_next_tick_with_the_socket_up() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![Err(ApiError::Server(500))]);
    let mut jobs = linked(d.path(), &api, &line);
    settle();
    jobs.ask(RommJob::Claim);
    until("no second claim", || claimed(&api) == 2);
    jobs.ask(RommJob::Claim);
    settle();
    assert_eq!(claimed(&api), 2);
}

#[test]
fn a_refused_socket_falls_back_to_a_claim_per_tick_silently() {
    let d = card();
    let (api, line) = (fake(), Line::refused());
    let mut jobs = worker(d.path(), &api, &line);
    for n in 1..=3 {
        jobs.ask(RommJob::Claim);
        until("no fallback claim", || claimed(&api) == n);
        assert_eq!(line.dials(), n);
        settle();
    }
    assert_eq!(claimed(&api), 3);
    assert_eq!(jobs.take(), Some(RommEvent::Online(true)));
    assert_eq!(jobs.take(), None);
}

#[test]
fn a_dropped_socket_redials_at_the_next_tick() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let mut jobs = linked(d.path(), &api, &line);
    line.push(Push::Closed);
    until("the source lived on", || line.alive() == 0);
    settle();
    assert_eq!(claimed(&api), 1);
    jobs.ask(RommJob::Claim);
    until("no redial", || claimed(&api) == 2);
    assert_eq!(line.dials(), 2);
}

#[test]
fn a_finished_download_claims_again_at_once_on_the_socket() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![
        Ok(vec![req("r1", 1), req("r2", 2)]),
        Ok(vec![req("r2", 2)]),
        Ok(Vec::new()),
    ]);
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("no second install", || api.reports().len() == 2);
    until("no last claim", || claimed(&api) == 3);
}

#[test]
fn a_finished_download_claims_again_at_once_when_polling() {
    let d = card();
    let (api, line) = (fake(), Line::refused());
    api.claims(vec![
        Ok(vec![req("r1", 1), req("r2", 2)]),
        Ok(vec![req("r2", 2)]),
        Ok(Vec::new()),
    ]);
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("no second install", || api.reports().len() == 2);
    until("no last claim", || claimed(&api) == 3);
    assert_eq!(line.dials(), 1);
}

#[test]
fn stop_closes_the_socket_and_a_new_tick_connects_and_claims_once() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let mut jobs = linked(d.path(), &api, &line);
    jobs.ask(RommJob::Stop);
    until("the socket stayed open", || line.alive() == 0);
    settle();
    assert_eq!(claimed(&api), 1);
    jobs.ask(RommJob::Claim);
    until("no claim on reconnect", || claimed(&api) == 2);
    settle();
    assert_eq!(claimed(&api), 2);
    assert_eq!(line.dials(), 2);
    assert_eq!(line.alive(), 1);
}

#[test]
fn a_download_stopped_by_the_gate_does_not_claim_again() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().hold = true;
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("the download never began", || api.0.lock().unwrap().held);
    jobs.ask(RommJob::Stop);
    until("the temp stayed", || temps(d.path()) == 0);
    settle();
    assert_eq!(claimed(&api), 1);
}

#[test]
fn a_tick_during_the_stop_unwind_still_restarts_the_request() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![
        Ok(vec![req("r1", 1)]),
        Ok(vec![req("r1", 1)]),
        Ok(vec![req("r1", 1)]),
    ]);
    api.0.lock().unwrap().hold = true;
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("the download never began", || api.0.lock().unwrap().held);
    api.0.lock().unwrap().hold = false;
    jobs.ask(RommJob::Stop);
    jobs.ask(RommJob::Claim);
    until("no install", || api.reports().len() == 1);
}

#[test]
fn losing_the_pairing_closes_the_socket() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let mut jobs = linked(d.path(), &api, &line);
    api.claims(vec![Err(ApiError::Unauthorized)]);
    line.push(Push::Queued);
    until("no Unpaired", || jobs.take() == Some(RommEvent::Unpaired));
    until("the socket stayed open", || line.alive() == 0);
    jobs.ask(RommJob::Claim);
    settle();
    assert_eq!(line.dials(), 1);
}

#[test]
fn stop_reaches_the_download_and_the_socket_past_a_stalled_claim() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    api.0.lock().unwrap().hold = true;
    let mut jobs = worker(d.path(), &api, &line);
    jobs.ask(RommJob::Claim);
    until("the download never began", || api.0.lock().unwrap().held);
    api.0.lock().unwrap().stall = true;
    line.push(Push::Queued);
    until("the claim never stalled", || api.0.lock().unwrap().stalled);
    jobs.ask(RommJob::Stop);
    until("the download kept writing", || temps(d.path()) == 0);
    until("the reader stayed up", || line.alive() == 0);
    api.0.lock().unwrap().stall = false;
    settle();
    assert_eq!(claimed(&api), 2);
}

#[test]
fn a_stop_clears_so_the_next_download_runs() {
    let d = card();
    let (api, line) = (fake(), Line::default());
    let mut jobs = linked(d.path(), &api, &line);
    jobs.ask(RommJob::Stop);
    until("the socket stayed open", || line.alive() == 0);
    api.claims(vec![Ok(vec![req("r1", 1)])]);
    jobs.ask(RommJob::Claim);
    until("no install", || api.reports().len() == 1);
    assert_eq!(api.reports()[0], ("r1".to_string(), Outcome::Done));
}

#[test]
fn claim_403_notes_once_and_clears() {
    let d = card();
    let api = fake();
    api.claims(vec![
        Err(ApiError::Client(403)),
        Err(ApiError::Client(403)),
        Ok(Vec::new()),
    ]);
    let mut r = runner(d.path(), &api);
    assert_eq!(
        poll(&mut r),
        vec![RommEvent::Note("Pairing lacks permissions".into())]
    );
    assert!(poll(&mut r).is_empty());
    assert_eq!(poll(&mut r), vec![RommEvent::NoteCleared]);
}

#[test]
fn a_claim_says_online_once_and_offline_once() {
    let d = card();
    let api = fake();
    api.claims(vec![
        Ok(Vec::new()),
        Ok(Vec::new()),
        Err(ApiError::Offline),
        Err(ApiError::Server(502)),
        Ok(Vec::new()),
    ]);
    let mut r = runner(d.path(), &api);
    assert_eq!(reach(&mut r), vec![true]);
    assert!(reach(&mut r).is_empty());
    assert_eq!(reach(&mut r), vec![false]);
    assert!(reach(&mut r).is_empty());
    assert_eq!(reach(&mut r), vec![true]);
}

#[test]
fn a_server_that_answers_with_a_note_is_online() {
    let d = card();
    let api = fake();
    api.claims(vec![Err(ApiError::Client(403))]);
    let mut r = runner(d.path(), &api);
    assert_eq!(reach(&mut r), vec![true]);
}

#[test]
fn a_device_check_that_cannot_reach_the_server_is_offline() {
    let d = card();
    let api = fake();
    api.claims(vec![Ok(Vec::new()), Err(ApiError::NotFound)]);
    api.0.lock().unwrap().exists = Err(ApiError::Offline);
    let mut r = runner(d.path(), &api);
    assert_eq!(reach(&mut r), vec![true]);
    assert_eq!(reach(&mut r), vec![false]);
}
