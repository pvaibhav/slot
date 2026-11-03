use std::sync::{Arc, Mutex};
use std::time::Duration;

use slot::romm::api::{Api, ApiError, Install, Outcome, Pending, Poll, RomInfo};
use slot::romm::jobs::Runner;
use slot::romm::{PairFail, RommConf, RommEvent};

#[derive(Default)]
struct State {
    version: String,
    polls: Vec<Result<Poll, ApiError>>,
    polled: usize,
    enable: Vec<Result<(), ApiError>>,
    enabled: Vec<(String, String)>,
}

#[derive(Clone)]
struct Fake(Arc<Mutex<State>>);

fn fake(version: &str, polls: Vec<Result<Poll, ApiError>>) -> Fake {
    Fake(Arc::new(Mutex::new(State {
        version: version.into(),
        polls,
        ..State::default()
    })))
}

impl Api for Fake {
    fn version(&self) -> Result<String, ApiError> {
        Ok(self.0.lock().unwrap().version.clone())
    }

    fn pair_start(&self, _conf: &RommConf) -> Result<Pending, ApiError> {
        Ok(Pending {
            device_code: "dc".into(),
            user_code: "K7M2QX9P".into(),
            path: "/pair/device?user_code=K7M2QX9P".into(),
            interval: 5,
            expires_in: 600,
        })
    }

    fn pair_poll(&self, _device_code: &str) -> Result<Poll, ApiError> {
        let mut s = self.0.lock().unwrap();
        s.polled += 1;
        match s.polls.is_empty() {
            true => Ok(Poll::Waiting),
            false => s.polls.remove(0),
        }
    }

    fn enable_install(&self, device_id: &str, token: &str) -> Result<(), ApiError> {
        let mut s = self.0.lock().unwrap();
        s.enabled.push((device_id.into(), token.into()));
        match s.enable.is_empty() {
            true => Ok(()),
            false => s.enable.remove(0),
        }
    }

    fn claim(&self) -> Result<Vec<Install>, ApiError> {
        Ok(Vec::new())
    }

    fn device_exists(&self) -> Result<bool, ApiError> {
        Ok(true)
    }

    fn rom(&self, _id: u64) -> Result<RomInfo, ApiError> {
        Err(ApiError::NotFound)
    }

    fn download(
        &self,
        _rom: u64,
        _name: &str,
        _file_ids: &[u64],
        _to: &std::path::Path,
        _cancel: &slot::romm::Cancel,
    ) -> Result<(), ApiError> {
        Ok(())
    }

    fn report(&self, _id: &str, _outcome: &Outcome) -> Result<(), ApiError> {
        Ok(())
    }

    fn delete_device(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

fn conf() -> RommConf {
    RommConf {
        server: "romm.example.com".into(),
        client_id: "cid".into(),
        ..RommConf::default()
    }
}

fn paired() -> Poll {
    Poll::Paired {
        token: "rmm_tok".into(),
        device_id: "dev-1".into(),
    }
}

fn runner(api: &Fake) -> Runner {
    let api = api.clone();
    Runner::new(
        std::path::PathBuf::new(),
        Box::new(move |_| Box::new(api.clone())),
        |_, _| false,
    )
}

fn run(runner: &mut Runner, waits: &mut Vec<u64>, cancel_after: usize) -> Vec<RommEvent> {
    let mut events = Vec::new();
    let mut wait = |d: Duration| {
        waits.push(d.as_secs());
        waits.len() > cancel_after
    };
    runner.pair(&conf(), &mut wait, &mut |e| events.push(e));
    events
}

#[test]
fn pairs_after_pending_then_success() {
    let api = fake("5.4.0", vec![Ok(Poll::Waiting), Ok(paired())]);
    let mut waits = Vec::new();
    let events = run(&mut runner(&api), &mut waits, usize::MAX);
    assert_eq!(
        events,
        vec![
            RommEvent::PairShow {
                url: "https://romm.example.com/pair/device?user_code=K7M2QX9P".into(),
                code: "K7M2QX9P".into(),
            },
            RommEvent::Paired {
                token: "rmm_tok".into(),
                device_id: "dev-1".into(),
            },
        ]
    );
    assert_eq!(waits, vec![5, 5]);
    assert_eq!(
        api.0.lock().unwrap().enabled,
        vec![("dev-1".to_string(), "rmm_tok".to_string())]
    );
}

#[test]
fn old_server_fails_as_too_old() {
    let api = fake("5.3.1", vec![]);
    let mut waits = Vec::new();
    let events = run(&mut runner(&api), &mut waits, usize::MAX);
    assert_eq!(events, vec![RommEvent::PairFailed(PairFail::TooOld)]);
    assert!(waits.is_empty());
    assert_eq!(api.0.lock().unwrap().polled, 0);
}

#[test]
fn denied_and_expired_report() {
    for (poll, fail) in [
        (Poll::Denied, PairFail::Denied),
        (Poll::Expired, PairFail::Expired),
    ] {
        let api = fake("5.4.0", vec![Ok(poll)]);
        let mut waits = Vec::new();
        let events = run(&mut runner(&api), &mut waits, usize::MAX);
        assert_eq!(events.last(), Some(&RommEvent::PairFailed(fail)));
        assert!(api.0.lock().unwrap().enabled.is_empty());
    }
}

#[test]
fn slow_down_lengthens_the_poll() {
    let api = fake(
        "5.4.0",
        vec![Ok(Poll::Waiting), Ok(Poll::SlowDown), Ok(Poll::Waiting), Ok(paired())],
    );
    let mut waits = Vec::new();
    run(&mut runner(&api), &mut waits, usize::MAX);
    assert_eq!(waits, vec![5, 5, 10, 10]);
}

#[test]
fn cancel_stops_polling() {
    let api = fake("5.4.0", vec![]);
    let mut waits = Vec::new();
    let events = run(&mut runner(&api), &mut waits, 2);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], RommEvent::PairShow { .. }));
    assert_eq!(api.0.lock().unwrap().polled, 2);
    assert_eq!(waits.len(), 3);
}

#[test]
fn the_pairing_expires_locally_when_the_server_stays_quiet() {
    let api = fake("5.4.0", vec![]);
    let mut waits = Vec::new();
    let events = run(&mut runner(&api), &mut waits, usize::MAX);
    assert_eq!(events.last(), Some(&RommEvent::PairFailed(PairFail::Expired)));
    assert_eq!(waits.iter().sum::<u64>(), 600);
}

#[test]
fn every_start_enables_install_for_a_paired_conf() {
    let api = fake("5.4.0", vec![]);
    api.0.lock().unwrap().enable = vec![Err(ApiError::Offline), Ok(())];
    let mut saved = conf();
    saved.device_id = Some("dev-1".into());
    saved.token = Some("rmm_tok".into());
    runner(&api).start(&saved);
    runner(&api).start(&saved);
    runner(&api).start(&saved);
    let enabled = api.0.lock().unwrap().enabled.clone();
    assert_eq!(enabled, vec![("dev-1".to_string(), "rmm_tok".to_string()); 3]);
}

#[test]
fn start_does_nothing_when_unpaired() {
    let api = fake("5.4.0", vec![]);
    runner(&api).start(&conf());
    assert!(api.0.lock().unwrap().enabled.is_empty());
}

#[test]
fn a_failed_capability_write_still_reports_paired() {
    let api = fake("5.4.0", vec![Ok(paired())]);
    api.0.lock().unwrap().enable = vec![Err(ApiError::Offline)];
    let mut waits = Vec::new();
    let events = run(&mut runner(&api), &mut waits, usize::MAX);
    assert!(matches!(events.last(), Some(RommEvent::Paired { .. })));
}
