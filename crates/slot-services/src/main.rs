//! Slot-owned home and Link networking. No commands run on the render thread.
mod config;
mod radio;
mod system;

use radio::{connected, Home, Interface};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use system::output;

const VERSION: &str = "1";
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}

struct Link {
    interface: Interface,
    owner: String,
    ready: bool,
    reply: Option<UnixStream>,
    deadline: Instant,
    role: String,
    freq: Option<u32>,
    record: PathBuf,
}
impl Link {
    fn new(run: &Path) -> Self {
        let record = run.join("link-owner");
        let mut link = Self {
            interface: Interface::new("wlan1", run),
            owner: String::new(),
            ready: false,
            reply: None,
            deadline: Instant::now(),
            role: String::new(),
            freq: None,
            record,
        };
        if link.interface.wpa.is_some() {
            let saved = fs::read_to_string(&link.record).unwrap_or_default();
            if let Some((owner, role)) = saved.split_once(' ') {
                if system::alive(owner) && ["host", "join"].contains(&role) {
                    link.owner = owner.into();
                    link.role = role.into();
                    link.freq = link.interface.observed_frequency();
                    link.ready = connected(&link.interface.status()) && link.interface.has_ip();
                    link.interface.restore_address(if role == "host" {
                        "10.42.0.1/24"
                    } else {
                        "10.42.0.2/24"
                    });
                }
            }
            if !link.ready {
                link.clear(1, "ORPHANED");
            }
        }
        link
    }
    fn clear(&mut self, code: u8, reason: &str) {
        if let Some(mut reply) = self.reply.take() {
            respond(&mut reply, code, reason);
        }
        self.interface.stop();
        self.owner.clear();
        self.role.clear();
        self.ready = false;
        self.freq = None;
        let _ = fs::remove_file(&self.record);
    }
    fn busy(&self) -> bool {
        !self.owner.is_empty()
    }
    fn start(
        &mut self,
        role: &str,
        owner: &str,
        reply: UnixStream,
        home: &Home,
    ) -> Result<(), (UnixStream, &'static str)> {
        if self.busy() {
            return Err((reply, "LINK_BUSY"));
        }
        let mut ours = home.interface.pids();
        ours.extend(self.interface.pids());
        if system::external_radio_owner(&ours) {
            return Err((reply, "EXTERNAL_OWNER"));
        }
        // A home attempt is also a channel owner: don't change the shared radio mid-association.
        let home_status = home.interface.status();
        if home.interface.wpa.is_some() && !connected(&home_status) {
            return Err((reply, "HOME_CONNECTING"));
        }
        let home_freq = if connected(&home_status) {
            home.interface.observed_frequency()
        } else {
            None
        };
        if connected(&home_status) && home_freq.is_none() {
            return Err((reply, "CHANNEL_UNKNOWN"));
        }
        let info = match radio::capabilities() {
            Ok(i) => i,
            Err(e) => return Err((reply, e)),
        };
        if home_freq.is_some_and(|f| !radio::permitted(&info, f)) {
            return Err((reply, "CHANNEL_NOT_ALLOWED"));
        }
        if role == "join" && home_freq.is_some() && !radio::dual_station(&info) {
            return Err((reply, "UNSUPPORTED_COMBINATION"));
        }
        let routes = match output("ip", &["-4", "route", "show", "table", "all"]) {
            Ok(r) => r,
            Err(e) => return Err((reply, e)),
        };
        if radio::subnet_conflict(&routes) {
            return Err((reply, "ADDRESS_CONFLICT"));
        }
        let freq = if role == "host" {
            let f = home_freq.or_else(|| {
                [5745, 2412, 2437, 2462]
                    .into_iter()
                    .find(|f| radio::permitted(&info, *f))
            });
            let Some(f) = f.filter(|f| radio::permitted(&info, *f)) else {
                return Err((reply, "CHANNEL_NOT_ALLOWED"));
            };
            Some(f)
        } else {
            home_freq
        };
        if !radio::net_exists("wlan1") {
            return Err((reply, "RADIO_UNAVAILABLE"));
        }
        if let Err(e) = home.interface.pin(home_freq) {
            return Err((reply, e));
        }
        let _ = output("rfkill", &["unblock", "wifi"]);
        let network = config::Network {
            ssid: "slotlink".into(),
            password: Some("slotlink0".into()),
        };
        if let Err(e) = self.interface.start(&network, freq, role == "host") {
            return Err((reply, e));
        }
        self.owner = owner.into();
        self.role = role.into();
        self.freq = freq;
        self.deadline = Instant::now() + Duration::from_secs(if role == "host" { 15 } else { 30 });
        self.reply = Some(reply);
        Ok(())
    }
    fn tick(&mut self) {
        if !self.busy() {
            return;
        }
        if !system::alive(&self.owner) {
            self.clear(1, "OWNER_EXITED");
            return;
        }
        if let Some(s) = self.reply.as_ref() {
            let mut b = 0u8;
            let count = unsafe {
                libc::recv(
                    s.as_raw_fd(),
                    &mut b as *mut u8 as *mut libc::c_void,
                    1,
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                )
            };
            if count == 0 {
                self.clear(1, "CANCELLED");
                return;
            }
        }
        let status = self.interface.status();
        if self.ready {
            if !connected(&status) {
                self.clear(1, "LINK_LOST");
            }
            return;
        }
        if connected(&status) {
            let actual = self.interface.observed_frequency();
            if actual.is_none() {
                self.clear(1, "CHANNEL_UNKNOWN");
                return;
            }
            if self.freq.is_some() && actual != self.freq {
                self.clear(1, "CHANNEL_CONFLICT");
                return;
            }
            self.freq = actual;
            let addr = if self.role == "host" {
                "10.42.0.1/24"
            } else {
                "10.42.0.2/24"
            };
            if let Err(e) = self.interface.address(addr) {
                self.clear(1, e);
                return;
            }
            if system::private_write(&self.record, &format!("{} {}", self.owner, self.role))
                .is_err()
            {
                self.clear(1, "RUNTIME_RECORD");
                return;
            }
            self.ready = true;
            if let Some(mut reply) = self.reply.take() {
                respond(&mut reply, 0, "READY");
            }
        } else if Instant::now() >= self.deadline {
            let code = if self.role == "join" { 3 } else { 1 };
            self.clear(code, "NO_HOST_OR_CHANNEL_CONFLICT");
        }
    }
}
fn respond(stream: &mut UnixStream, code: u8, reason: &str) {
    let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
    let _ = writeln!(stream, "{code} {reason}");
}

fn ensure_time() {
    let ctl = std::env::var("SLOT_NTP_CTL").unwrap_or_else(|_| "/usr/bin/timedatectl".into());
    let helper = std::env::var("SLOT_NTP_HELPER").unwrap_or_else(|_| "/usr/sbin/baseos-ntp".into());
    let enabled = output(&ctl, &["show", "-p", "NTP", "--value"]);
    match enabled {
        Ok(v) if v.trim() == "yes" => {
            // The BaseOS lock makes this idempotent, including its boot-launched worker.
            if let Ok(mut child) = Command::new(&helper)
                .arg("run")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
            }
        }
        Ok(_) => {
            let _ = output(&ctl, &["set-ntp", "true"]);
        }
        Err(_) => eprintln!("slot-services: NTP_UNAVAILABLE"),
    }
}

fn serve(root: &Path, run: &Path) -> std::io::Result<()> {
    fs::create_dir_all(run)?;
    fs::set_permissions(run, fs::Permissions::from_mode(0o700))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(run.join("lock"))?;
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Ok(());
    }
    let socket = run.join("control.sock");
    let _ = fs::remove_file(&socket);
    let listener = UnixListener::bind(&socket)?;
    listener.set_nonblocking(true)?;
    let mut home = Home::new(run);
    let mut link = Link::new(run);
    home.enable(slot_store::read_slot_state(root).home_wifi_enabled, root);
    // The frontend may update the restored preference over this socket.
    let mut warm: Option<(String, Instant)> = None;
    let mut was_link_busy = false;
    let mut ntp_at = Instant::now();
    let mut tick_at = Instant::now();
    let mut powered_down = false;
    unsafe {
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
    }
    while !STOP.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(100)));
                let mut request = Vec::new();
                // One short line per connection; never read credentials over this API.
                for _ in 0..2048 {
                    let mut b = [0];
                    if stream.read_exact(&mut b).is_err() {
                        break;
                    }
                    if b[0] == b'\n' {
                        break;
                    }
                    request.push(b[0]);
                }
                let text = String::from_utf8_lossy(&request);
                let parts: Vec<_> = text.split_whitespace().collect();
                if parts.len() != 5
                    || parts[0] != VERSION
                    || parts[1] != config::hex(root.to_string_lossy().as_ref())
                {
                    respond(&mut stream, 1, "PROTOCOL_OR_ROOT_MISMATCH");
                    continue;
                }
                let owner = parts[2];
                let domain = parts[3];
                let action = parts[4];
                if !system::alive(owner) {
                    respond(&mut stream, 1, "OWNER_INVALID");
                    continue;
                }
                match (domain, action) {
                    ("home", "on" | "off") => {
                        home.enable(action == "on", root);
                        respond(&mut stream, 0, "ACCEPTED");
                    }
                    ("home", "reload") => {
                        if home.enabled {
                            home.reload(root);
                        }
                        respond(
                            &mut stream,
                            if home.error.is_empty() { 0 } else { 1 },
                            if home.error.is_empty() {
                                "RELOADED"
                            } else {
                                home.error
                            },
                        );
                    }
                    ("service", "status") => {
                        respond(
                            &mut stream,
                            0,
                            &format!(
                                "home_enabled={} home_connected={} home_error={} link={} freq={:?}",
                                home.enabled,
                                home.enabled
                                    && connected(&home.interface.status())
                                    && home.interface.has_ip(),
                                home.error,
                                link.role,
                                link.freq
                            ),
                        );
                    }
                    ("service", "stop") => {
                        STOP.store(true, Ordering::Relaxed);
                        respond(&mut stream, 0, "STOPPING");
                    }
                    ("link", "warm") => {
                        warm = Some((owner.into(), Instant::now() + Duration::from_secs(60)));
                        let ready = radio::net_exists("wlan1");
                        if ready {
                            let _ = output("rfkill", &["unblock", "wifi"]);
                            powered_down = false;
                        }
                        respond(
                            &mut stream,
                            if ready { 0 } else { 1 },
                            if ready { "WARM" } else { "RADIO_UNAVAILABLE" },
                        );
                    }
                    ("link", "cool") => {
                        if warm.as_ref().is_some_and(|(o, _)| o == owner) {
                            warm = None;
                        }
                        respond(&mut stream, 0, "COOLED");
                    }
                    ("link", "down") => {
                        if link.busy() && link.owner != owner {
                            respond(&mut stream, 1, "NOT_OWNER");
                        } else {
                            link.clear(1, "CANCELLED");
                            warm = None;
                            respond(&mut stream, 0, "DOWN");
                        }
                    }
                    ("link", "host" | "join") => {
                        if let Err((mut reply, error)) = link.start(action, owner, stream, &home) {
                            if !link.busy() {
                                let _ = home.interface.pin(None);
                            }
                            respond(&mut reply, 1, error);
                        } else {
                            was_link_busy = true;
                        }
                        powered_down = false;
                    }
                    _ => respond(&mut stream, 2, "UNKNOWN_COMMAND"),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
        let now = Instant::now();
        if now >= ntp_at {
            ensure_time();
            ntp_at = now + Duration::from_secs(60);
        }
        if now >= tick_at {
            link.tick();
            if !link.busy() && was_link_busy {
                let _ = home.interface.pin(None);
            }
            was_link_busy = link.busy();
            let mut ours = home.interface.pids();
            ours.extend(link.interface.pids());
            home.tick(root, link.busy(), link.freq, &ours);
            if home.interface.wpa.is_some() {
                powered_down = false;
            }
            if warm
                .as_ref()
                .is_some_and(|(o, until)| now >= *until || !system::alive(o))
            {
                warm = None;
            }
            if !home.enabled
                && !link.busy()
                && warm.is_none()
                && !powered_down
                && !system::external_radio_owner(&ours)
            {
                // rfkill leaves the driver loaded, avoiding BaseOS's SDIO initialization race.
                if output("rfkill", &["block", "wifi"]).is_ok() {
                    powered_down = true;
                }
            }
            tick_at = now + Duration::from_millis(500);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    link.clear(1, "STOPPING");
    home.enable(false, root);
    let _ = fs::remove_file(socket);
    Ok(())
}

fn connect(root: &Path, run: &Path) -> std::io::Result<UnixStream> {
    if let Ok(s) = UnixStream::connect(run.join("control.sock")) {
        return Ok(s);
    }
    fs::create_dir_all(run)?;
    fs::set_permissions(run, fs::Permissions::from_mode(0o700))?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(run.join("service.log"))?;
    let binary = root.join("System/slot-services");
    let loader = Path::new("/lib/ld-linux-aarch64.so.1");
    let mut c = if loader.exists() {
        let mut c = Command::new(loader);
        c.arg(&binary);
        c
    } else {
        Command::new(&binary)
    };
    c.arg("--serve")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    use std::os::unix::process::CommandExt;
    unsafe {
        c.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = c.spawn()?;
    // Reap if it loses the startup lock; don't tie the daemon lifetime to this wrapper.
    for _ in 0..60 {
        if let Ok(s) = UnixStream::connect(run.join("control.sock")) {
            return Ok(s);
        }
        let _ = child.try_wait();
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "service startup",
    ))
}
fn main() {
    let root = PathBuf::from(std::env::var_os("SLOT_ROOT").unwrap_or_else(|| "/mnt/sdcard".into()));
    let root = match fs::canonicalize(root) {
        Ok(root) => root,
        Err(_) => {
            eprintln!("slot-services: CARD_UNAVAILABLE");
            std::process::exit(1);
        }
    };
    let run = std::env::var_os("SLOT_SERVICES_RUN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/slot-services"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--serve"] {
        if let Err(e) = serve(&root, &run) {
            eprintln!("slot-services: {e}");
            std::process::exit(1);
        }
        return;
    }
    if args.len() != 2 {
        eprintln!("usage: ags-net home on|off|reload | link warm|cool|host|join|down | service status|stop");
        std::process::exit(2);
    }
    let pid = std::env::var("SLOT_OWNER_PID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(std::process::id);
    let Some(owner) = system::identity(pid) else {
        std::process::exit(1);
    };
    let result = (|| -> std::io::Result<u8> {
        let mut s = connect(&root, &run)?;
        s.set_read_timeout(Some(Duration::from_secs(
            if args[0] == "link" && ["host", "join"].contains(&args[1].as_str()) {
                45
            } else {
                8
            },
        )))?;
        s.set_write_timeout(Some(Duration::from_secs(2)))?;
        writeln!(
            s,
            "{VERSION} {} {owner} {} {}",
            config::hex(root.to_string_lossy().as_ref()),
            args[0],
            args[1]
        )?;
        let mut response = String::new();
        s.take(2048).read_to_string(&mut response)?;
        let code = response
            .split_whitespace()
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        if args[1] == "status" {
            println!("{}", response.trim());
        } else if code != 0 {
            eprintln!("slot-services: {}", response.trim());
        }
        Ok(code)
    })();
    match result {
        Ok(code) => std::process::exit(code.into()),
        Err(e) => {
            eprintln!("slot-services: {e}");
            std::process::exit(1);
        }
    }
}
