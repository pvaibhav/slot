//! Run real service processes with fake Linux networking tools. Never needs a radio/root.
#![cfg(target_os = "linux")]
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tempfile::TempDir;

// The service scans the shared /proc namespace for competing radio owners.
// Keep each fake radio exclusive even with the default parallel test runner.
static RADIO: Mutex<()> = Mutex::new(());

struct Rig {
    _radio: MutexGuard<'static, ()>,
    dir: TempDir,
    daemon: Option<Child>,
}
impl Rig {
    fn new() -> Self {
        Self::with_radio(true)
    }
    fn with_radio(ready: bool) -> Self {
        let radio = RADIO.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        for p in ["System", "bin", "net/wlan0", "net/wlan1", "run"] {
            fs::create_dir_all(dir.path().join(p)).unwrap();
        }
        if !ready {
            fs::remove_dir(dir.path().join("net/wlan0")).unwrap();
        }
        let script = dir.path().join("bin/mock");
        fs::write(&script,r#"#!/bin/sh
set -eu
name=${0##*/}
echo "$name $*" >> "$MOCK_ROOT/commands"
case "$name" in
rfkill) exit 0 ;;
iw)
 if [ "$1" = dev ]; then echo "channel 1 (2412 MHz), width: 20 MHz"; exit 0; fi
 cat <<'CAP'
valid interface combinations:
 * #{ managed } <= 2, total <= 2, #channels <= 1
 * 2412 MHz [1] (20.0 dBm)
 * 5745 MHz [149] (20.0 dBm)
CAP
 ;;
wpa_supplicant) while :; do sleep 1; done ;;
wpa_cli)
 case " $* " in
 *' set_network '*) echo OK ;;
 *' wlan1 '* )
  if [ -f "$MOCK_ROOT/block-link" ]; then echo wpa_state=SCANNING
  else printf 'wpa_state=COMPLETED\nfreq=2412\n'; fi ;;
 *) if grep -q 'ssid=416273656e74' "$MOCK_ROOT/run/wlan0.conf" 2>/dev/null; then echo wpa_state=SCANNING; else printf 'wpa_state=COMPLETED\nfreq=2412\n'; fi ;;
 esac ;;
udhcpc)
 touch "$MOCK_ROOT/home-ip"
 while :; do sleep 1; done ;;
ip)
 case "$*" in
 '-4 -o addr show dev wlan0') [ ! -f "$MOCK_ROOT/home-ip" ] || echo '2: wlan0 inet 192.168.1.24/24' ;;
 '-4 -o addr show dev wlan1') [ ! -f "$MOCK_ROOT/link-ip" ] || echo '3: wlan1 inet 10.42.0.1/24' ;;
 '-4 route show table all') echo '192.168.1.0/24 dev wlan0' ;;
 'addr add '*) touch "$MOCK_ROOT/link-ip" ;;
 'addr del '*) rm -f "$MOCK_ROOT/link-ip" ;;
 '-4 addr flush dev wlan0') rm -f "$MOCK_ROOT/home-ip" ;;
 esac ;;
timedatectl)
 case "$1" in
 show) if [ -f "$MOCK_ROOT/ntp-enabled" ]; then echo yes; else echo no; fi ;;
 set-ntp) touch "$MOCK_ROOT/ntp-enabled" ;;
 esac ;;
baseos-ntp) exit 0 ;;
esac
exit 0
"#).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        for tool in [
            "rfkill",
            "iw",
            "wpa_cli",
            "wpa_supplicant",
            "udhcpc",
            "ip",
            "timedatectl",
            "baseos-ntp",
        ] {
            symlink(&script, dir.path().join("bin").join(tool)).unwrap();
        }
        fs::copy(
            env!("CARGO_BIN_EXE_slot-services"),
            dir.path().join("System/slot-services"),
        )
        .unwrap();
        fs::write(
            dir.path().join("System/wifi.toml"),
            "[[networks]]\nssid='Home'\npassword='private-password'\n[[networks]]\nssid='Absent'\npassword='password-two'\n",
        )
        .unwrap();
        slot_store::write_slot_state(
            dir.path(),
            &slot_store::SlotState {
                home_wifi_enabled: true,
                clock_set: true,
                ..Default::default()
            },
        )
        .unwrap();
        let mut r = Self {
            _radio: radio,
            dir,
            daemon: None,
        };
        r.start();
        r
    }
    fn command(&self) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_slot-services"));
        let root = self.dir.path();
        c.env("SLOT_ROOT", root)
            .env("SLOT_SERVICES_RUN", root.join("run"))
            .env("SLOT_NET_SYS", root.join("net"))
            .env("MOCK_ROOT", root)
            .env("SLOT_NTP_CTL", root.join("bin/timedatectl"))
            .env("SLOT_NTP_HELPER", root.join("bin/baseos-ntp"))
            .env("SLOT_OWNER_PID", std::process::id().to_string())
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            );
        c
    }
    fn start(&mut self) {
        self.daemon = Some(
            self.command()
                .arg("--serve")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.dir.path().join("run/control.sock").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    fn call(&self, args: &[&str]) -> String {
        let o = self.command().args(args).output().unwrap();
        assert!(
            o.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8(o.stdout).unwrap()
    }
    fn wait_connected(&self) {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if self
                .call(&["service", "status"])
                .contains("home_connected=true")
            {
                break;
            }
            assert!(Instant::now() < deadline, "home did not connect");
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    fn record(&self, name: &str) -> String {
        fs::read_to_string(self.dir.path().join("run").join(name)).unwrap()
    }
}
impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.command().args(["service", "stop"]).output();
        if let Some(mut child) = self.daemon.take() {
            let _ = child.wait();
        }
    }
}

#[test]
fn home_link_off_and_crash_recovery_keep_independent_ownership() {
    let mut r = Rig::new();
    r.wait_connected();
    let home = r.record("wlan0.wpa.pid");
    let dhcp = r.record("wlan0.dhcp.pid");
    r.call(&["link", "host"]);
    let link = r.record("wlan1.wpa.pid");
    assert_eq!(r.record("wlan0.wpa.pid"), home);
    assert_eq!(r.record("wlan0.dhcp.pid"), dhcp);
    // A service crash must adopt surviving children, not take down SSH or the AP.
    let mut child = r.daemon.take().unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    fs::remove_file(r.dir.path().join("run/control.sock")).unwrap();
    r.start();
    r.wait_connected();
    assert_eq!(r.record("wlan0.wpa.pid"), home);
    assert_eq!(r.record("wlan1.wpa.pid"), link);
    r.call(&["home", "off"]);
    assert!(r
        .call(&["service", "status"])
        .contains("home_connected=false"));
    assert_eq!(r.record("wlan1.wpa.pid"), link);
    assert!(r.dir.path().join("System/wifi.toml").exists());
    assert!(r.dir.path().join("ntp-enabled").exists());
    r.call(&["link", "down"]);
    // With home Off, joining must not demand dual-station or turn home back on.
    r.call(&["link", "join"]);
    assert!(r
        .call(&["service", "status"])
        .contains("home_connected=false"));
    r.call(&["link", "down"]);
    assert!(!r.dir.path().join("run/wlan1.wpa.pid").exists());
    assert!(!fs::read_to_string(r.dir.path().join("commands"))
        .unwrap()
        .contains("private-password"));
}

#[test]
fn cancelling_setup_cleans_only_link_and_invalid_reload_keeps_home() {
    let r = Rig::new();
    r.wait_connected();
    let home = r.record("wlan0.wpa.pid");
    fs::write(
        r.dir.path().join("System/wifi.toml"),
        "password='never-log-this",
    )
    .unwrap();
    assert!(!r
        .command()
        .args(["home", "reload"])
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(r.record("wlan0.wpa.pid"), home);
    fs::write(r.dir.path().join("block-link"), "").unwrap();
    let mut client = r.command().args(["link", "host"]).spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !r.dir.path().join("run/wlan1.wpa.pid").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    client.kill().unwrap();
    client.wait().unwrap();
    while r.dir.path().join("run/wlan1.wpa.pid").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(r.record("wlan0.wpa.pid"), home);
}

#[test]
fn an_unavailable_first_profile_falls_back_and_missing_config_stops_only_home() {
    let r = Rig::new();
    r.wait_connected();
    fs::write(r.dir.path().join("System/wifi.toml"),"[[networks]]\nssid='Absent'\npassword='password-one'\n[[networks]]\nssid='Home'\npassword='password-two'\n").unwrap();
    r.call(&["home", "reload"]);
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        let status = r.call(&["service", "status"]);
        let conf = fs::read_to_string(r.dir.path().join("run/wlan0.conf")).unwrap_or_default();
        if status.contains("home_connected=true") && conf.contains("ssid=486f6d65") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "did not fall back to second network"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    r.call(&["link", "host"]);
    let link = r.record("wlan1.wpa.pid");
    fs::remove_file(r.dir.path().join("System/wifi.toml")).unwrap();
    r.call(&["home", "reload"]);
    assert!(r
        .call(&["service", "status"])
        .contains("home_connected=false"));
    assert_eq!(r.record("wlan1.wpa.pid"), link);
}

#[test]
fn delayed_boot_radio_does_not_skip_the_preferred_network() {
    let r = Rig::with_radio(false);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if r.call(&["service", "status"])
            .contains("home_error=RADIO_UNAVAILABLE")
        {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!r.dir.path().join("run/wlan0.wpa.pid").exists());
    fs::create_dir(r.dir.path().join("net/wlan0")).unwrap();
    // The preferred network connects within the radio retry interval. Trying
    // the absent second profile first would instead stall for 25 seconds.
    r.wait_connected();
    assert!(r.record("wlan0.conf").contains("ssid=486f6d65"));
}
