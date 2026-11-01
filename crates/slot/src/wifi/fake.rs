use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::wpa::{apply_auto_join, parse_saved};
use super::{Backend, Saved, Scanned, State, Status};

struct Net {
    id: u32,
    ssid: String,
    password: String,
    manual: bool,
    enabled: bool,
}

pub struct Fake {
    pub log: Arc<Mutex<Vec<String>>>,
    pub nearby: Arc<Mutex<Vec<Scanned>>>,
    pub conf: Arc<Mutex<String>>,
    pub blocked: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
    pub no_lease: Arc<AtomicBool>,
    passwords: Vec<(String, String)>,
    nets: Vec<Net>,
    next_id: u32,
    on: bool,
    broken: bool,
    silent: bool,
    current: Option<(u32, u32)>,
    rejected: Vec<u32>,
    joins_saved: bool,
}

impl Fake {
    pub fn new(nearby: Vec<(&str, i32, bool)>, passwords: &[(&str, &str)]) -> Fake {
        Fake {
            log: Arc::new(Mutex::new(Vec::new())),
            nearby: Arc::new(Mutex::new(
                nearby
                    .into_iter()
                    .map(|(s, signal, secured)| Scanned {
                        ssid: s.into(),
                        signal,
                        secured,
                    })
                    .collect(),
            )),
            conf: Arc::new(Mutex::new(String::new())),
            blocked: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(false)),
            no_lease: Arc::new(AtomicBool::new(false)),
            passwords: passwords
                .iter()
                .map(|(s, p)| (s.to_string(), p.to_string()))
                .collect(),
            nets: Vec::new(),
            next_id: 0,
            on: false,
            broken: false,
            silent: false,
            current: None,
            rejected: Vec::new(),
            joins_saved: false,
        }
    }

    pub fn demo() -> Fake {
        Fake::new(
            vec![
                ("Home", -48, true),
                ("Neighbor 5G", -63, true),
                ("CoffeeShop Guest", -71, false),
                ("xfinitywifi", -82, false),
            ],
            &[("Home", "password")],
        )
    }

    pub fn joined() -> Fake {
        let mut f = Fake::demo();
        f.add("Home", "password", false);
        f.joins_saved = true;
        f
    }

    pub fn broken() -> Fake {
        let mut f = Fake::demo();
        f.broken = true;
        f
    }

    pub fn silent() -> Fake {
        let mut f = Fake::demo();
        f.silent = true;
        f
    }

    fn note(&self, s: String) {
        self.log.lock().unwrap().push(s);
    }

    fn list(&self) -> String {
        let mut out = String::from("network id / ssid / bssid / flags\n");
        for n in &self.nets {
            out.push_str(&format!(
                "{}\t{}\tany\t{}\n",
                n.id,
                n.ssid,
                if n.enabled { "" } else { "[DISABLED]" }
            ));
        }
        out
    }

    fn save(&self) {
        let mut text = String::from("update_config=1\n");
        for n in &self.nets {
            text.push_str(&format!(
                "network={{\n\tssid=\"{}\"\n\tid_str=\"{}\"\n",
                n.ssid,
                if n.manual { "manual" } else { "auto" }
            ));
            if !n.enabled {
                text.push_str("\tdisabled=1\n");
            }
            text.push_str("}\n");
        }
        *self.conf.lock().unwrap() = apply_auto_join(&text);
    }
}

impl Backend for Fake {
    fn start(&mut self) -> bool {
        self.note("start".into());
        self.on = !self.broken;
        if self.on {
            self.blocked.store(false, Ordering::SeqCst);
            let old: Vec<u32> = self.nets.iter().map(|n| n.id).collect();
            for (i, n) in self.nets.iter_mut().enumerate() {
                n.id = i as u32;
            }
            self.rejected = self
                .rejected
                .iter()
                .filter_map(|r| old.iter().position(|o| o == r).map(|i| i as u32))
                .collect();
            self.next_id = self.nets.len() as u32;
            if self.joins_saved && self.current.is_none() {
                self.current = self.nets.iter().find(|n| !n.manual).map(|n| (n.id, 0));
            }
        }
        self.running.store(self.on, Ordering::SeqCst);
        self.on
    }

    fn block(&mut self) {
        self.note("block".into());
        self.blocked.store(true, Ordering::SeqCst);
    }

    fn stop(&mut self) {
        self.note("stop".into());
        self.on = false;
        self.running.store(false, Ordering::SeqCst);
        self.current = None;
    }

    fn scan(&mut self) -> Vec<Scanned> {
        self.note("scan".into());
        let mut v = self.nearby.lock().unwrap().clone();
        v.sort_by(|a, b| b.signal.cmp(&a.signal).then_with(|| a.ssid.cmp(&b.ssid)));
        v
    }

    fn saved(&mut self) -> Vec<Saved> {
        parse_saved(&self.list(), &self.conf.lock().unwrap())
    }

    fn status(&mut self) -> Status {
        if !self.on {
            return Status {
                state: State::Off,
                ssid: None,
                ip: None,
            };
        }
        let idle = Status {
            state: State::Scanning,
            ssid: None,
            ip: None,
        };
        if self.silent {
            return idle;
        }
        let Some((id, polls)) = self.current else {
            return idle;
        };
        self.current = Some((id, polls + 1));
        let Some(net) = self.nets.iter().find(|n| n.id == id) else {
            return idle;
        };
        let nearby = self
            .nearby
            .lock()
            .unwrap()
            .iter()
            .find(|n| n.ssid == net.ssid)
            .cloned();
        let Some(near) = nearby else { return idle };
        let want = self
            .passwords
            .iter()
            .find(|(s, _)| *s == net.ssid)
            .map(|(_, p)| p.as_str());
        let right = if near.secured {
            want == Some(net.password.as_str())
        } else {
            true
        };
        let ssid = Some(net.ssid.clone());
        if right {
            Status {
                state: State::Completed,
                ssid,
                ip: (!self.no_lease.load(Ordering::SeqCst)).then(|| "192.168.1.42".into()),
            }
        } else if polls == 0 {
            Status {
                state: State::Handshake,
                ssid,
                ip: None,
            }
        } else {
            if !self.rejected.contains(&id) {
                self.rejected.push(id);
            }
            idle
        }
    }

    fn temp_disabled(&mut self, id: u32) -> bool {
        self.rejected.contains(&id)
    }

    fn add(&mut self, ssid: &str, password: &str, hidden: bool) -> Option<u32> {
        self.note(format!("add {ssid}{}", if hidden { " hidden" } else { "" }));
        let id = self.next_id;
        self.next_id += 1;
        self.nets.push(Net {
            id,
            ssid: ssid.into(),
            password: password.into(),
            manual: false,
            enabled: true,
        });
        self.save();
        Some(id)
    }

    fn connect(&mut self, id: u32) {
        self.note(format!("connect {id}"));
        self.rejected.retain(|r| *r != id);
        for n in &mut self.nets {
            n.enabled = n.id == id;
        }
        self.current = Some((id, 0));
    }

    fn finish_connect(&mut self) {
        self.note("finish_connect".into());
        for n in &mut self.nets {
            n.enabled |= !n.manual;
        }
    }

    fn disconnect(&mut self) {
        self.note("disconnect".into());
        self.current = None;
    }

    fn set_auto_join(&mut self, id: u32, on: bool) {
        self.note(format!("set_auto_join {id} {on}"));
        if let Some(n) = self.nets.iter_mut().find(|n| n.id == id) {
            n.manual = !on;
            n.enabled = on || self.current.is_some_and(|(c, _)| c == id);
        }
        self.save();
    }

    fn forget(&mut self, id: u32) {
        self.note(format!("forget {id}"));
        self.nets.retain(|n| n.id != id);
        if self.current.is_some_and(|(c, _)| c == id) {
            self.current = None;
        }
        self.save();
    }
}
