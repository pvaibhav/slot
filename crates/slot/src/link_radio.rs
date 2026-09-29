#[cfg(feature = "device")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "device")]
use std::sync::mpsc::{channel, Sender};
#[cfg(feature = "device")]
use std::sync::OnceLock;

use crate::link_net::Cancel;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRole {
    Host,
    Join,
}

impl LinkRole {
    pub fn arg(self) -> &'static str {
        match self {
            LinkRole::Host => "host",
            LinkRole::Join => "join",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadioFail {
    NoHost,
    /// `ags-net link` exited 4: Home Wi-Fi is connected on a channel the link cannot share, or
    /// is still associating. The radio is fine and the player can fix it by turning Home Wi-Fi off.
    HomeWifi,
    Cancelled,
    Radio(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioJob {
    /// Desired home connectivity, independent of the Link lease.
    Home(bool),
    Warm,
    /// Release Link preparation, retaining the home connection.
    Cool,
    Down,
}

pub trait RadioJobs: Send {
    fn ask(&mut self, job: RadioJob);

    fn warmed(&self) -> bool;

    /// Observed home association plus a LAN address, never the desired menu flag.
    fn home_connected(&self) -> bool {
        false
    }
}

pub struct RadioQueue;

pub fn radio_jobs() -> Box<dyn RadioJobs> {
    Box::new(RadioQueue)
}

#[cfg(feature = "device")]
impl RadioJobs for RadioQueue {
    fn ask(&mut self, job: RadioJob) {
        if job == RadioJob::Home(false) {
            HOME_CONNECTED.store(false, Ordering::SeqCst);
        }
        let _ = queue().send(job);
    }

    fn warmed(&self) -> bool {
        WARM.load(Ordering::SeqCst)
    }
    fn home_connected(&self) -> bool {
        HOME_CONNECTED.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "device")]
static WARM: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "device")]
static HOME_CONNECTED: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "device")]
fn queue() -> &'static Sender<RadioJob> {
    static Q: OnceLock<Sender<RadioJob>> = OnceLock::new();
    Q.get_or_init(|| {
        let (tx, rx) = channel::<RadioJob>();
        std::thread::spawn(move || loop {
            let job = match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                Ok(job) => job,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let connected = helper()
                        .args(["service", "status"])
                        .output()
                        .ok()
                        .filter(|o| o.status.success())
                        .is_some_and(|o| {
                            String::from_utf8_lossy(&o.stdout)
                                .split_whitespace()
                                .any(|v| v == "home_connected=true")
                        });
                    HOME_CONNECTED.store(connected, Ordering::SeqCst);
                    continue;
                }
            };
            match job {
                RadioJob::Home(enabled) => {
                    let _ = helper()
                        .args(["home", if enabled { "on" } else { "off" }])
                        .status();
                }
                RadioJob::Warm => WARM.store(run("warm"), Ordering::SeqCst),
                RadioJob::Cool => {
                    WARM.store(false, Ordering::SeqCst);
                    run("cool");
                }
                RadioJob::Down => {
                    WARM.store(false, Ordering::SeqCst);
                    down();
                }
            }
        });
        tx
    })
}

/// Always resolve the helper from this card, never PATH's OS helper.
#[cfg(feature = "device")]
fn helper() -> std::process::Command {
    let root = std::env::var_os("SLOT_ROOT").unwrap_or_else(|| "/mnt/sdcard".into());
    let mut c = std::process::Command::new("/bin/sh");
    c.arg(std::path::Path::new(&root).join("System/ags-net"));
    c.env("SLOT_ROOT", root)
        .env("SLOT_OWNER_PID", std::process::id().to_string());
    c
}

/// A best-effort preparation/cleanup command served by the bundled daemon.
#[cfg(feature = "device")]
fn run(sub: &str) -> bool {
    helper()
        .arg("link")
        .arg(sub)
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(feature = "device")]
pub fn up(role: LinkRole, cancel: &Cancel) -> Result<(), RadioFail> {
    let mut child = helper().arg("link").arg(role.arg()).spawn().map_err(|e| {
        RadioFail::Radio(format!("ags-net link {} would not start: {e}", role.arg()))
    })?;
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RadioFail::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) if status.code() == Some(3) => return Err(RadioFail::NoHost),
            // 4 is the service refusing because Home Wi-Fi holds the radio. Reported as itself
            // because it is the one radio failure the player can undo from the menu.
            Ok(Some(status)) if status.code() == Some(4) => return Err(RadioFail::HomeWifi),
            Ok(Some(status)) => {
                return Err(RadioFail::Radio(format!(
                    "link {} failed: {status}",
                    role.arg()
                )))
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(e) => return Err(RadioFail::Radio(format!("link {}: {e}", role.arg()))),
        }
    }
}

#[cfg(feature = "device")]
pub fn down() {
    let _ = helper().arg("link").arg("down").status();
}

#[cfg(not(feature = "device"))]
pub fn up(_role: LinkRole, _cancel: &Cancel) -> Result<(), RadioFail> {
    Ok(())
}

#[cfg(not(feature = "device"))]
pub fn down() {}

#[cfg(not(feature = "device"))]
impl RadioJobs for RadioQueue {
    fn ask(&mut self, _job: RadioJob) {}

    fn warmed(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_role_asks_for_its_own_subcommand() {
        assert_eq!(LinkRole::Host.arg(), "host");
        assert_eq!(LinkRole::Join.arg(), "join");
    }

    #[test]
    fn asking_for_a_job_is_never_an_error() {
        let mut jobs = radio_jobs();
        jobs.ask(RadioJob::Warm);
        jobs.ask(RadioJob::Cool);
        jobs.ask(RadioJob::Down);
    }

    #[cfg(not(feature = "device"))]
    #[test]
    fn a_host_build_has_no_driver_left_to_load() {
        assert!(radio_jobs().warmed());
    }
}
