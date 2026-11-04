use std::path::{Path, PathBuf};
use std::time::Duration;

use slot_store::atomic_write;

use super::wpa::{
    append_network, apply_auto_join, blocks, edit_network, ensure_header, hex, parse_saved,
    parse_scan, parse_status, temp_disabled, Edit,
};
use super::{Backend, Saved, Scanned, State, Status, Wifi};

pub trait Run: Send {
    fn run(&mut self, program: &str, args: &[&str]) -> Option<String>;
    fn spawn(&mut self, program: &str, args: &[&str]);
    fn sleep(&mut self, d: Duration);
}

pub struct System;

impl Run for System {
    fn run(&mut self, program: &str, args: &[&str]) -> Option<String> {
        let out = std::process::Command::new(program)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn spawn(&mut self, program: &str, args: &[&str]) {
        use std::process::{Command, Stdio};
        if let Ok(mut child) = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            std::thread::spawn(move || child.wait());
        }
    }

    fn sleep(&mut self, d: Duration) {
        std::thread::sleep(d);
    }
}

const WPA_CLI: &str = "/usr/sbin/wpa_cli";
const CTRL: [&str; 4] = ["-p", "/var/run/wpa_supplicant", "-i", "wlan0"];
const SH: &str = "/bin/sh";
const CTRL_DIR: &str = "/var/run/wpa_supplicant";
const UDHCPC: &str = "/sbin/udhcpc";
const IP: &str = "/sbin/ip";
const PIDFILE: &str = "/run/udhcpc.wlan0.pid";
const HEADER: &str = "network id / ssid / bssid / flags\n";

pub struct Wpa<R: Run> {
    runner: R,
    script: PathBuf,
    conf: PathBuf,
    disconnected: bool,
    renewed: bool,
}

impl<R: Run> Wpa<R> {
    pub fn new(runner: R, root: PathBuf) -> Wpa<R> {
        Wpa {
            runner,
            script: root.join("System/slotlink.sh"),
            conf: root.join("Config/wifi.conf"),
            disconnected: false,
            renewed: false,
        }
    }

    pub fn runner_mut(&mut self) -> &mut R {
        &mut self.runner
    }

    fn slotlink(&mut self, args: &[&str]) -> bool {
        let script = self.script.to_string_lossy().into_owned();
        let mut all: Vec<&str> = vec![&script];
        all.extend_from_slice(args);
        self.runner.run(SH, &all).is_some()
    }

    fn cli(&mut self, args: &[&str]) -> Option<String> {
        let mut all: Vec<&str> = CTRL.to_vec();
        all.extend_from_slice(args);
        self.runner.run(WPA_CLI, &all)
    }

    fn conf_text(&self) -> String {
        std::fs::read_to_string(&self.conf).unwrap_or_default()
    }

    fn save(&mut self) {
        self.cli(&["save_config"]);
        let text = self.conf_text();
        let fixed = apply_auto_join(&text);
        if fixed != text {
            let _ = atomic_write(&self.conf, fixed.as_bytes());
        }
    }

    fn list(&mut self) -> String {
        match self.cli(&["list_networks"]) {
            Some(text) if text.starts_with("network id") => text,
            _ => list_from_conf(&self.conf_text()),
        }
    }

    fn current(&mut self) -> Option<u32> {
        let text = self.cli(&["status"])?;
        text.lines()
            .find_map(|l| l.strip_prefix("id="))
            .and_then(|v| v.trim().parse().ok())
    }

    fn up(&mut self) -> bool {
        self.cli(&["ping"]).is_some_and(|o| o.trim() == "PONG")
    }

    fn edit_file(&mut self, id: u32, edit: Edit) {
        let text = self.conf_text();
        let _ = atomic_write(
            &self.conf,
            edit_network(&text, id as usize, edit).as_bytes(),
        );
    }

    fn flush(&mut self) {
        self.runner.run(IP, &["addr", "flush", "dev", "wlan0"]);
    }

    fn dhcp_running(&mut self) -> Option<String> {
        let pid = std::fs::read_to_string(PIDFILE).ok()?;
        let pid = pid.trim().to_string();
        std::path::Path::new("/proc")
            .join(&pid)
            .exists()
            .then_some(pid)
    }
}

fn unquote(v: &str) -> Option<String> {
    if let Some(q) = v.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return Some(q.to_string());
    }
    let bytes: Option<Vec<u8>> = (0..v.len())
        .step_by(2)
        .map(|i| v.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
        .collect();
    bytes.map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn list_from_conf(conf: &str) -> String {
    let mut out = String::from(HEADER);
    for (i, b) in blocks(conf).iter().enumerate() {
        let ssid = b
            .iter()
            .find_map(|l| l.strip_prefix("ssid="))
            .and_then(unquote)
            .unwrap_or_default();
        out.push_str(&format!("{i}\t{ssid}\tany\t\n"));
    }
    out
}

impl<R: Run> Backend for Wpa<R> {
    fn start(&mut self) -> bool {
        let text = self.conf_text();
        let fixed = ensure_header(&text, CTRL_DIR);
        if fixed != text {
            let _ = atomic_write(&self.conf, fixed.as_bytes());
        }
        self.disconnected = false;
        self.renewed = false;
        let conf = self.conf.to_string_lossy().into_owned();
        self.slotlink(&["wifi", "start", &conf])
    }

    fn stop(&mut self) {
        self.slotlink(&["wifi", "stop"]);
    }

    fn block(&mut self) {
        self.slotlink(&["wifi", "off"]);
    }

    fn scan(&mut self) -> Vec<Scanned> {
        self.cli(&["scan"]);
        let mut last: Option<usize> = None;
        let mut text = String::new();
        for _ in 0..9 {
            self.runner.sleep(Duration::from_secs(1));
            text = self.cli(&["scan_results"]).unwrap_or_default();
            let n = text.lines().count();
            if last == Some(n) && n > 1 {
                break;
            }
            last = Some(n);
        }
        parse_scan(&text)
    }

    fn saved(&mut self) -> Vec<Saved> {
        let list = self.list();
        parse_saved(&list, &self.conf_text())
    }

    fn status(&mut self) -> Status {
        let Some(text) = self.cli(&["status"]) else {
            return Status {
                state: State::Off,
                ssid: None,
                ip: None,
            };
        };
        let s = parse_status(&text);
        if s.state == State::Completed {
            if !self.renewed {
                match self.dhcp_running() {
                    Some(pid) => {
                        self.runner.run("kill", &["-USR1", &pid]);
                    }
                    None => self.runner.spawn(
                        UDHCPC,
                        &[
                            "-i", "wlan0", "-S", "-t", "5", "-T", "7", "-b", "-p", PIDFILE,
                        ],
                    ),
                }
                self.renewed = true;
            }
        } else {
            self.renewed = false;
        }
        s
    }

    fn temp_disabled(&mut self, id: u32) -> bool {
        let list = self.cli(&["list_networks"]).unwrap_or_default();
        temp_disabled(&list, id)
    }

    fn add(&mut self, ssid: &str, password: &str, hidden: bool) -> Option<u32> {
        if !self.up() {
            let text = self.conf_text();
            let id = blocks(&text).len() as u32;
            let _ = atomic_write(
                &self.conf,
                append_network(&text, ssid, password, hidden).as_bytes(),
            );
            return Some(id);
        }
        let id: u32 = self
            .cli(&["add_network"])?
            .lines()
            .last()?
            .trim()
            .parse()
            .ok()?;
        let n = id.to_string();
        self.cli(&["set_network", &n, "ssid", &hex(ssid)]);
        if password.is_empty() {
            self.cli(&["set_network", &n, "key_mgmt", "NONE"]);
        } else {
            self.cli(&["set_network", &n, "psk", &format!("\"{password}\"")]);
        }
        self.cli(&["set_network", &n, "id_str", "\"auto\""]);
        if hidden {
            self.cli(&["set_network", &n, "scan_ssid", "1"]);
        }
        self.cli(&["enable_network", &n]);
        self.save();
        Some(id)
    }

    fn connect(&mut self, id: u32) {
        if self.disconnected {
            self.cli(&["reconnect"]);
            self.disconnected = false;
        }
        self.flush();
        self.renewed = false;
        self.cli(&["select_network", &id.to_string()]);
    }

    fn finish_connect(&mut self) {
        let current = self.current();
        for n in self.saved() {
            if n.auto_join {
                self.cli(&["enable_network", &n.id.to_string()]);
            } else if current != Some(n.id) {
                self.cli(&["disable_network", &n.id.to_string()]);
            }
        }
    }

    fn disconnect(&mut self) {
        self.cli(&["disconnect"]);
        self.disconnected = true;
        self.flush();
    }

    fn set_auto_join(&mut self, id: u32, on: bool) {
        if !self.up() {
            return self.edit_file(id, Edit::IdStr(if on { "auto" } else { "manual" }));
        }
        let n = id.to_string();
        self.cli(&[
            "set_network",
            &n,
            "id_str",
            if on { "\"auto\"" } else { "\"manual\"" },
        ]);
        if on {
            self.cli(&["enable_network", &n]);
        } else if self.current() != Some(id) {
            self.cli(&["disable_network", &n]);
        }
        self.save();
    }

    fn forget(&mut self, id: u32) {
        if !self.up() {
            return self.edit_file(id, Edit::Remove);
        }
        self.cli(&["remove_network", &id.to_string()]);
        self.save();
    }
}

pub fn device_wifi(root: &Path) -> Wifi {
    Wifi::spawn(Box::new(Wpa::new(System, root.to_path_buf())))
}
