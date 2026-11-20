#[cfg(feature = "device")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(feature = "device")]
use std::sync::mpsc::{channel, Sender};
#[cfg(feature = "device")]
use std::sync::OnceLock;
#[cfg(any(feature = "device", test))]
use std::{path::Path, process::Command};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostAp {
    pub freq_mhz: u16,
    pub bssid: [u8; 6],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioRole {
    Host,
    Join(HostAp),
}

impl RadioRole {
    pub fn arg(self) -> &'static str {
        match self {
            RadioRole::Host => "host",
            RadioRole::Join(_) => "join",
        }
    }
}

pub fn bssid_text(b: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5]
    )
}

pub fn parse_ap(line: &str) -> Option<HostAp> {
    let mut words = line.split_whitespace();
    let freq_mhz = words.next()?.parse::<u16>().ok()?;
    let text = words.next()?;
    if words.next().is_some() {
        return None;
    }
    let mut bssid = [0u8; 6];
    let mut parts = text.split(':');
    for byte in bssid.iter_mut() {
        let part = parts.next()?;
        if part.len() != 2 {
            return None;
        }
        *byte = u8::from_str_radix(part, 16).ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(HostAp { freq_mhz, bssid })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RadioFail {
    NoHost,
    Cancelled,
    Radio(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioJob {
    Warm,
    Cool,
    Down,
}

pub trait RadioJobs: Send {
    fn ask(&mut self, job: RadioJob);

    fn warmed(&self) -> bool;
}

pub struct RadioQueue;

pub fn radio_jobs() -> Box<dyn RadioJobs> {
    Box::new(RadioQueue)
}

#[cfg(feature = "device")]
impl RadioJobs for RadioQueue {
    fn ask(&mut self, job: RadioJob) {
        let _ = queue().send(job);
    }

    fn warmed(&self) -> bool {
        WARM.load(Ordering::SeqCst)
    }
}

#[cfg(feature = "device")]
static WARM: AtomicBool = AtomicBool::new(false);

#[cfg(feature = "device")]
fn queue() -> &'static Sender<RadioJob> {
    static Q: OnceLock<Sender<RadioJob>> = OnceLock::new();
    Q.get_or_init(|| {
        let (tx, rx) = channel::<RadioJob>();
        std::thread::spawn(move || {
            for job in rx {
                match job {
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
            }
        });
        tx
    })
}

#[cfg(any(feature = "device", test))]
fn link_command(root: &Path, verb: &str, extra: &[String]) -> Command {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg(root.join("System/slotlink.sh"))
        .arg("link")
        .arg(verb)
        .args(extra)
        .env("AGS_LINK_TRACE", root.join("Logs/slotlink.log"));
    cmd
}

#[cfg(feature = "device")]
fn link(verb: &str, extra: &[String]) -> Command {
    let root = std::env::var_os("SLOT_ROOT").unwrap_or_else(|| "/mnt/sdcard".into());
    link_command(Path::new(&root), verb, extra)
}

#[cfg(feature = "device")]
fn read_ap() -> Result<HostAp, RadioFail> {
    let run = std::env::var_os("RUN").unwrap_or_else(|| "/run".into());
    let path = Path::new(&run).join("slotlink.ap");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| RadioFail::Radio(format!("{} unreadable: {e}", path.display())))?;
    parse_ap(&text).ok_or_else(|| RadioFail::Radio(format!("{} holds no address", path.display())))
}

#[cfg(feature = "device")]
fn run(sub: &str) -> bool {
    link(sub, &[]).status().is_ok_and(|status| status.success())
}

#[cfg(feature = "device")]
pub fn up(role: RadioRole, cancel: &Cancel) -> Result<Option<HostAp>, RadioFail> {
    let extra = match role {
        RadioRole::Host => Vec::new(),
        RadioRole::Join(ap) => vec![ap.freq_mhz.to_string(), bssid_text(&ap.bssid)],
    };
    let mut child = link(role.arg(), &extra)
        .spawn()
        .map_err(|e| RadioFail::Radio(format!("link {} would not start: {e}", role.arg())))?;
    loop {
        if cancel.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(RadioFail::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => {
                return match role {
                    RadioRole::Host => read_ap().map(Some),
                    RadioRole::Join(_) => Ok(None),
                }
            }
            Ok(Some(status)) if status.code() == Some(3) => return Err(RadioFail::NoHost),
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
    let _ = link("down", &[]).status();
}

#[cfg(not(feature = "device"))]
pub fn up(_role: RadioRole, _cancel: &Cancel) -> Result<Option<HostAp>, RadioFail> {
    Ok(None)
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

    fn argv(cmd: &Command) -> Vec<String> {
        std::iter::once(cmd.get_program())
            .chain(cmd.get_args())
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_link_runs_the_cards_script_through_sh() {
        let d = tempfile::tempdir().unwrap();
        let script = d.path().join("System/slotlink.sh");
        assert_eq!(
            argv(&link_command(d.path(), "host", &[])),
            ["/bin/sh", script.to_str().unwrap(), "link", "host"]
        );
    }

    #[test]
    fn a_join_passes_the_frequency_and_bssid() {
        let d = tempfile::tempdir().unwrap();
        let script = d.path().join("System/slotlink.sh");
        let ap = HostAp {
            freq_mhz: 5180,
            bssid: [2, 0x11, 0x22, 0x33, 0x44, 0x55],
        };
        let extra = [ap.freq_mhz.to_string(), bssid_text(&ap.bssid)];
        assert_eq!(
            argv(&link_command(d.path(), "join", &extra)),
            [
                "/bin/sh",
                script.to_str().unwrap(),
                "link",
                "join",
                "5180",
                "02:11:22:33:44:55"
            ]
        );
    }

    #[test]
    fn an_address_line_parses() {
        assert_eq!(
            parse_ap("5180 02:11:22:33:44:55\n"),
            Some(HostAp {
                freq_mhz: 5180,
                bssid: [2, 0x11, 0x22, 0x33, 0x44, 0x55]
            })
        );
    }

    #[test]
    fn a_line_that_is_not_an_address_does_not_parse() {
        for bad in [
            "junk",
            "",
            "5180",
            "5180 02:11:22:33:44",
            "5180 02:11:22:33:44:55:66",
            "x 02:11:22:33:44:55",
            "5180 zz:11:22:33:44:55",
            "5180 02:11:22:33:44:55 extra",
        ] {
            assert_eq!(parse_ap(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_bssid_round_trips() {
        let b = [0xaa, 0xbb, 0x0c, 0, 1, 0xff];
        assert_eq!(
            parse_ap(&format!("2412 {}", bssid_text(&b))).unwrap().bssid,
            b
        );
    }

    #[test]
    fn a_link_traces_into_the_cards_logs() {
        let d = tempfile::tempdir().unwrap();
        let cmd = link_command(d.path(), "host", &[]);
        let trace = cmd
            .get_envs()
            .find(|(k, _)| *k == "AGS_LINK_TRACE")
            .and_then(|(_, v)| v);
        assert_eq!(trace, Some(d.path().join("Logs/slotlink.log").as_os_str()));
    }

    #[cfg(not(feature = "device"))]
    #[test]
    fn a_host_build_has_no_driver_left_to_load() {
        assert!(radio_jobs().warmed());
    }
}
