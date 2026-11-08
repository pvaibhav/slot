use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::UNIX_EPOCH;

use slot_input::Millis;
use slot_store::{scan, Cart, Platform, ShellChoice};
use slot_ui::ReceiptText;

use super::{generate_pin, qr_modules, qr_url, shown_address, wifi_ip, Responder, Server, PORTS};
use crate::label_cache::fresh;
use crate::print_scene::{Cue, PaperJob, PrintArtState, PrintArtWorker, PrintFrame, PrintScene, PrintTextures};

pub const NO_WIFI: &str = "NO WI-FI";
pub const SERVER_FAILED: &str = "GAMEBIT FAILED";

#[derive(Clone)]
pub struct ServerNet {
    pub ip: fn() -> Option<Ipv4Addr>,
    pub ports: Vec<u16>,
    pub responder: fn(Ipv4Addr) -> Option<Responder>,
}

impl Default for ServerNet {
    fn default() -> ServerNet {
        ServerNet {
            ip: wifi_ip,
            ports: PORTS.to_vec(),
            responder: Responder::start,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerPaper {
    Code {
        url: String,
        host: Option<String>,
        ip: String,
        pin: String,
    },
    Plain(&'static str),
}

struct Serving {
    server: Server,
    responder: Option<Responder>,
    modules: Vec<Vec<bool>>,
    before: Seen,
}

type Sig = (Option<(u64, u64)>, Option<ShellChoice>);
type Seen = HashMap<(Platform, String), Sig>;

fn sig(cart: &Cart) -> Sig {
    let stamp = cart
        .label
        .as_deref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok());
            (m.len(), mtime.map_or(0, |d| d.as_secs()))
        });
    (stamp, cart.shell)
}

fn seen(root: &Path) -> Seen {
    scan(root)
        .map(|carts| {
            carts
                .iter()
                .map(|c| ((c.platform, c.stem.clone()), sig(c)))
                .collect()
        })
        .unwrap_or_default()
}

struct Up {
    serving: Option<Serving>,
    paper: ServerPaper,
    job: PaperJob,
}

fn plain(text: &'static str) -> Up {
    Up {
        serving: None,
        paper: ServerPaper::Plain(text),
        job: PaperJob::Blank(vec![text.to_string()]),
    }
}

fn bring_up(root: PathBuf, net: ServerNet) -> Up {
    let Some(ip) = (net.ip)() else {
        return plain(NO_WIFI);
    };
    let before = seen(&root);
    let pin = generate_pin();
    let server = net
        .ports
        .iter()
        .find_map(|port| Server::start(root.clone(), *port, pin.clone()).ok());
    let Some(server) = server else {
        return plain(SERVER_FAILED);
    };
    let port = server.port();
    let url = qr_url(ip, port, &pin);
    let Some(modules) = qr_modules(&url) else {
        server.stop();
        return plain(SERVER_FAILED);
    };
    let responder = (net.responder)(ip);
    let host = responder.as_ref().map(|r| shown_address(&r.host(), port));
    let shown = shown_address(&ip.to_string(), port);
    Up {
        serving: Some(Serving {
            server,
            responder,
            modules: modules.clone(),
            before,
        }),
        paper: ServerPaper::Code {
            url,
            host: host.clone(),
            ip: shown.clone(),
            pin: pin.clone(),
        },
        job: PaperJob::Code {
            modules,
            text: ReceiptText { host, ip: shown, pin },
        },
    }
}

fn renamed(paper: &mut ServerPaper, host: Option<String>) -> Option<ReceiptText> {
    let ServerPaper::Code { host: shown, ip, pin, .. } = paper else {
        return None;
    };
    if host.is_none() || host == *shown {
        return None;
    }
    shown.clone_from(&host);
    Some(ReceiptText {
        host,
        ip: ip.clone(),
        pin: pin.clone(),
    })
}

pub struct Rescan {
    pub carts: Vec<Cart>,
    pub stale: Vec<Cart>,
}

fn tear_down(serving: Serving, root: &Path, hurry: &AtomicBool) -> Option<Rescan> {
    if let Some(responder) = serving.responder {
        responder.stop();
    }
    serving.server.stop();
    let carts = scan(root).ok()?;
    let stale = carts
        .iter()
        .take_while(|_| !hurry.load(Ordering::SeqCst))
        .filter(|c| serving.before.get(&(c.platform, c.stem.clone())) != Some(&sig(c)))
        .filter(|c| !fresh(root, c))
        .cloned()
        .collect();
    Some(Rescan { carts, stale })
}

pub struct ServerSession {
    root: PathBuf,
    rising: Option<Receiver<Up>>,
    serving: Option<Serving>,
    paper: Option<ServerPaper>,
    art: Option<PrintArtWorker>,
    tex: Option<PrintTextures>,
    paper_up: bool,
    scene: Option<PrintScene>,
    stop_asked: bool,
    hurry: Arc<AtomicBool>,
    owed: Vec<Cue>,
    stopping: Option<Receiver<Option<Rescan>>>,
    stopped: Option<Option<Rescan>>,
    failed: bool,
}

impl ServerSession {
    pub fn open(root: PathBuf, net: ServerNet) -> ServerSession {
        let (outbox, rising) = mpsc::channel();
        let card = root.clone();
        let spawned = thread::Builder::new()
            .name("slot-server-up".into())
            .spawn(move || {
                let _ = outbox.send(bring_up(card, net));
            });
        if let Err(e) = spawned {
            eprintln!("slot: server: worker thread failed to start: {e}");
        }
        ServerSession {
            root,
            rising: Some(rising),
            serving: None,
            paper: None,
            art: None,
            tex: None,
            paper_up: false,
            scene: None,
            stop_asked: false,
            hurry: Arc::new(AtomicBool::new(false)),
            owed: Vec::new(),
            stopping: None,
            stopped: None,
            failed: false,
        }
    }

    pub fn poll(&mut self, now: Millis) {
        if let Some(rising) = &self.rising {
            match rising.try_recv() {
                Ok(up) => self.risen(up),
                Err(TryRecvError::Disconnected) => self.risen(plain(SERVER_FAILED)),
                Err(TryRecvError::Empty) => {}
            }
        }
        self.follow_rename();
        if self.stop_asked && self.rising.is_none() && self.stopped.is_none() && self.stopping.is_none() {
            self.begin_stop();
        }
        if let Some(stopping) = &self.stopping {
            match stopping.try_recv() {
                Ok(rescan) => self.stopped = Some(rescan),
                Err(TryRecvError::Disconnected) => self.stopped = Some(None),
                Err(TryRecvError::Empty) => return,
            }
            self.stopping = None;
        }
        if self.tex.is_some() && self.scene.is_none() && !self.stop_asked {
            self.scene = Some(PrintScene::new(now));
        }
        if let Some(scene) = self.scene.as_mut().filter(|_| !self.paper_up) {
            scene.hold_print(now);
        }
    }

    fn follow_rename(&mut self) {
        if self.stop_asked {
            return;
        }
        let (Some(serving), Some(paper)) = (&self.serving, &mut self.paper) else {
            return;
        };
        let host = serving
            .responder
            .as_ref()
            .map(|r| shown_address(&r.host(), serving.server.port()));
        if let Some(text) = renamed(paper, host) {
            self.art = Some(PrintArtWorker::spawn(PaperJob::Code {
                modules: serving.modules.clone(),
                text,
            }));
        }
    }

    fn risen(&mut self, up: Up) {
        self.rising = None;
        self.serving = up.serving;
        self.paper = Some(up.paper);
        if !self.stop_asked {
            self.art = Some(PrintArtWorker::spawn(up.job));
        }
    }

    fn begin_stop(&mut self) {
        let Some(serving) = self.serving.take() else {
            self.stopped = Some(None);
            return;
        };
        let (outbox, stopping) = mpsc::channel();
        let root = self.root.clone();
        let hurry = self.hurry.clone();
        let spawned = thread::Builder::new()
            .name("slot-server-down".into())
            .spawn(move || {
                let _ = outbox.send(tear_down(serving, &root, &hurry));
            });
        match spawned {
            Ok(_) => self.stopping = Some(stopping),
            Err(e) => {
                eprintln!("slot: server: stop thread failed to start: {e}");
                self.stopped = Some(None);
            }
        }
    }

    pub fn paper(&self) -> Option<&ServerPaper> {
        self.paper.as_ref()
    }

    pub fn take_art(&mut self) -> PrintArtState {
        let Some(art) = &mut self.art else {
            return PrintArtState::Pending;
        };
        let state = art.take();
        if !matches!(state, PrintArtState::Pending) {
            self.art = None;
        }
        state
    }

    pub fn set_tex(&mut self, tex: PrintTextures) {
        self.tex = Some(tex);
    }

    pub fn set_paper_up(&mut self) {
        self.paper_up = true;
    }

    pub fn paper_up(&self) -> bool {
        self.paper_up
    }

    pub fn fail(&mut self, now: Millis) {
        self.failed = true;
        self.abandon(now);
    }

    pub fn abandon(&mut self, now: Millis) {
        self.stop_asked = true;
        self.hurry.store(true, Ordering::SeqCst);
        self.art = None;
        if let Some(mut scene) = self.scene.take() {
            scene.stop(now);
            self.owed.extend(scene.cues(now).into_iter().filter(|c| *c == Cue::BuzzOff));
        }
    }

    pub fn stop(&mut self, now: Millis) {
        self.stop_asked = true;
        self.art = None;
        if let Some(scene) = &mut self.scene {
            scene.stop(now);
        }
    }

    pub fn skip(&mut self, now: Millis) {
        if let Some(scene) = self.scene.as_mut().filter(|_| self.paper_up) {
            scene.skip(now);
        }
    }

    pub fn hurried(&self) -> bool {
        self.hurry.load(Ordering::SeqCst)
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn cues(&mut self, now: Millis) -> Vec<Cue> {
        let mut out = std::mem::take(&mut self.owed);
        if let Some(scene) = &mut self.scene {
            out.extend(scene.cues(now));
        }
        out
    }

    pub fn frame(&self, now: Millis) -> Option<(PrintFrame, PrintTextures)> {
        Some((self.scene.as_ref()?.frame(now), self.tex?))
    }

    pub fn scene_running(&self) -> bool {
        self.scene.is_some()
    }

    pub fn finished(&self, now: Millis) -> bool {
        self.stopped.is_some() && self.scene.as_ref().is_none_or(|s| s.done(now))
    }

    pub fn take_rescan(&mut self) -> Option<Rescan> {
        self.stopped.take().flatten()
    }
}

#[cfg(test)]
mod rename_tests {
    use super::*;

    fn code(host: &str) -> ServerPaper {
        ServerPaper::Code {
            url: "http://192.168.4.77/?pin=1234".into(),
            host: Some(host.into()),
            ip: "192.168.4.77".into(),
            pin: "1234".into(),
        }
    }

    #[test]
    fn a_renamed_responder_reprints_the_receipt_with_its_new_name() {
        let responder = Responder::named("slot");
        let mut paper = code("SLOT.LOCAL");
        assert!(renamed(&mut paper, Some(responder.host())).is_none());
        responder.rename("slot-2");
        let text = renamed(&mut paper, Some(responder.host())).unwrap();
        assert_eq!(text.host.as_deref(), Some("SLOT-2.LOCAL"));
        assert_eq!((text.ip.as_str(), text.pin.as_str()), ("192.168.4.77", "1234"));
        assert_eq!(paper, code("SLOT-2.LOCAL"));
        assert!(renamed(&mut paper, Some(responder.host())).is_none(), "printed once");
    }

    #[test]
    fn a_plain_receipt_or_no_responder_is_left_alone() {
        let mut plain = ServerPaper::Plain(NO_WIFI);
        assert!(renamed(&mut plain, Some("SLOT.LOCAL".into())).is_none());
        let mut paper = code("SLOT.LOCAL");
        assert!(renamed(&mut paper, None).is_none());
        assert_eq!(paper, code("SLOT.LOCAL"));
    }
}
