use crate::config::{self, Network};
use crate::system::{self, field, output, Process};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub fn net_exists(name: &str) -> bool {
    std::env::var_os("SLOT_NET_SYS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/sys/class/net"))
        .join(name)
        .exists()
}

pub struct Interface {
    pub name: &'static str,
    run: PathBuf,
    pub wpa: Option<Process>,
    dhcp: Option<Process>,
    address: Option<&'static str>,
}
impl Interface {
    pub fn new(name: &'static str, run: &Path) -> Self {
        let wpa = Process::adopt(run.join(format!("{name}.wpa.pid")), run);
        let address = if name == "wlan1" && wpa.is_some() {
            let conf = fs::read_to_string(run.join("wlan1.conf")).unwrap_or_default();
            Some(if conf.lines().any(|l| l == "mode=2") {
                "10.42.0.1/24"
            } else {
                "10.42.0.2/24"
            })
        } else {
            None
        };
        Self {
            name,
            run: run.to_owned(),
            wpa,
            dhcp: Process::adopt(run.join(format!("{name}.dhcp.pid")), run),
            address,
        }
    }
    fn ctrl(&self) -> PathBuf {
        self.run.join(self.name)
    }
    pub fn pids(&self) -> Vec<u32> {
        self.wpa
            .iter()
            .chain(self.dhcp.iter())
            .map(Process::pid)
            .collect()
    }
    pub fn observed_frequency(&self) -> Option<u32> {
        let info = output("iw", &["dev", self.name, "info"]).ok()?;
        observed_frequency(&info)
    }
    pub fn pin(&self, frequency: Option<u32>) -> Result<(), &'static str> {
        if self.wpa.is_none() {
            return Ok(());
        }
        let value = frequency.map(|f| f.to_string()).unwrap_or_default();
        for field in ["scan_freq", "freq_list"] {
            let result = output(
                "wpa_cli",
                &[
                    "-p",
                    self.ctrl().to_str().unwrap(),
                    "-i",
                    self.name,
                    "set_network",
                    "0",
                    field,
                    &value,
                ],
            )?;
            if result.trim() != "OK" {
                return Err("CHANNEL_PIN_FAILED");
            }
        }
        Ok(())
    }
    pub fn status(&self) -> String {
        if self.wpa.is_none() {
            return String::new();
        }
        output(
            "wpa_cli",
            &[
                "-p",
                self.ctrl().to_str().unwrap(),
                "-i",
                self.name,
                "status",
            ],
        )
        .unwrap_or_default()
    }
    pub fn start(&mut self, n: &Network, freq: Option<u32>, ap: bool) -> Result<(), &'static str> {
        self.stop();
        let ctrl = self.ctrl();
        fs::create_dir_all(&ctrl).map_err(|_| "RUNTIME_DIRECTORY")?;
        let _ = fs::remove_file(ctrl.join(self.name));
        let conf = self.run.join(format!("{}.conf", self.name));
        system::private_write(&conf, &config::supplicant(n, &ctrl, freq, ap))
            .map_err(|_| "RUNTIME_CONFIG")?;
        output("ip", &["link", "set", self.name, "up"])?;
        self.wpa = Some(Process::spawn(
            "wpa_supplicant",
            &["-i", self.name, "-c", conf.to_str().unwrap(), "-Dnl80211"],
            self.run.join(format!("{}.wpa.pid", self.name)),
        )?);
        Ok(())
    }
    pub fn dhcp(&mut self) -> Result<(), &'static str> {
        if self.dhcp.as_mut().is_some_and(Process::running) {
            return Ok(());
        }
        // Stay foreground for renewals. -n exits after a bounded initial failure.
        self.dhcp = Some(Process::spawn(
            "udhcpc",
            &[
                "-f",
                "-n",
                "-i",
                self.name,
                "-t",
                "3",
                "-T",
                "3",
                "-s",
                "/usr/share/udhcpc/default.script",
                "-p",
                self.run.join("home-dhcp.pid").to_str().unwrap(),
            ],
            self.run.join(format!("{}.dhcp.pid", self.name)),
        )?);
        Ok(())
    }
    pub fn has_ip(&self) -> bool {
        output("ip", &["-4", "-o", "addr", "show", "dev", self.name])
            .is_ok_and(|s| s.contains(" inet "))
    }
    pub fn restore_address(&mut self, address: &'static str) {
        self.address = Some(address);
    }
    pub fn address(&mut self, address: &'static str) -> Result<(), &'static str> {
        // Retain ownership even if the command adds the address but its reply times out.
        self.address = Some(address);
        output("ip", &["addr", "add", address, "dev", self.name])?;
        Ok(())
    }
    pub fn stop(&mut self) {
        let owned = self.wpa.is_some() || self.dhcp.is_some() || self.address.is_some();
        self.dhcp.take();
        self.wpa.take();
        if let Some(addr) = self.address.take() {
            let _ = output("ip", &["addr", "del", addr, "dev", self.name]);
        }
        if owned {
            if self.name == "wlan0" {
                // We exclusively owned this interface and its DHCP lease. Never flush Link.
                let _ = output("ip", &["-4", "addr", "flush", "dev", self.name]);
                let _ = output("ip", &["route", "flush", "dev", self.name]);
            }
            let _ = output("ip", &["link", "set", self.name, "down"]);
        }
        let _ = fs::remove_file(self.run.join(format!("{}.conf", self.name)));
    }
}
impl Drop for Interface {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn observed_frequency(info: &str) -> Option<u32> {
    info.lines().find_map(|line| {
        if !line.trim_start().starts_with("channel ") {
            return None;
        }
        line.split_once('(')?
            .1
            .split_whitespace()
            .next()?
            .parse()
            .ok()
    })
}

pub fn connected(status: &str) -> bool {
    field(status, "wpa_state") == Some("COMPLETED")
}

// Do not infer a safe transmit channel from a configured frequency. Read regulatory flags.
pub fn permitted(info: &str, freq: u32) -> bool {
    info.lines().any(|l| {
        l.contains(&format!("{freq} MHz"))
            && !["disabled", "no IR", "radar", "passive"]
                .iter()
                .any(|s| l.contains(s))
    })
}
pub fn capabilities() -> Result<String, &'static str> {
    output("iw", &["list"])
}

// Conservative parser of nl80211's advertised combination, not just supported modes.
pub fn dual_station(info: &str) -> bool {
    let Some(combinations) = info.split("valid interface combinations:").nth(1) else {
        return false;
    };
    combinations.split(" * ").any(|c| {
        let Some((before, after)) = c.split_once("managed }") else {
            return false;
        };
        before.trim_end().ends_with("#{")
            && after.trim_start().starts_with("<= 2")
            && c.contains("total <= 2")
            && c.contains("#channels <= 1")
    })
}

pub fn subnet_conflict(routes: &str) -> bool {
    routes.lines().any(|line| {
        if line.contains(" dev wlan1") {
            return false;
        }
        let mut words = line.split_whitespace();
        let first = words.next().unwrap_or("");
        let Some(prefix) = (if ["local", "broadcast", "unreachable", "blackhole"].contains(&first) {
            words.next()
        } else {
            Some(first)
        }) else {
            return false;
        };
        if prefix == "default" {
            return false;
        }
        let (ip, bits) = prefix.split_once('/').unwrap_or((prefix, "32"));
        let Ok(ip) = ip.parse::<std::net::Ipv4Addr>() else {
            return false;
        };
        let Ok(bits) = bits.parse::<u32>() else {
            return true;
        };
        if bits > 32 {
            return true;
        }
        let mask = if bits == 0 {
            0
        } else {
            u32::MAX << (32 - bits.min(24))
        };
        (u32::from(ip) & mask) == (u32::from(std::net::Ipv4Addr::new(10, 42, 0, 0)) & mask)
    })
}

pub struct Home {
    pub interface: Interface,
    pub enabled: bool,
    pub error: &'static str,
    profiles: Vec<Network>,
    next: usize,
    deadline: Instant,
    retry: Instant,
    connected: bool,
}
impl Home {
    pub fn new(run: &Path) -> Self {
        let interface = Interface::new("wlan0", run);
        let connected = connected(&interface.status()) && interface.has_ip();
        Self {
            interface,
            enabled: false,
            error: "",
            profiles: Vec::new(),
            next: 0,
            deadline: Instant::now() + Duration::from_secs(25),
            retry: Instant::now(),
            connected,
        }
    }
    pub fn enable(&mut self, enabled: bool, root: &Path) {
        self.enabled = enabled;
        if !enabled {
            self.interface.stop();
            self.connected = false;
        } else {
            self.reload(root);
        }
    }
    pub fn reload(&mut self, root: &Path) {
        match config::read(root) {
            Ok(p) => {
                if self.profiles != p && !self.profiles.is_empty() || p.is_empty() {
                    self.interface.stop();
                    self.connected = false;
                }
                if self.profiles.is_empty() && self.interface.wpa.is_some() && !p.is_empty() {
                    let conf = fs::read_to_string(self.interface.run.join("wlan0.conf"))
                        .unwrap_or_default();
                    if !p
                        .iter()
                        .any(|n| conf == config::supplicant(n, &self.interface.ctrl(), None, false))
                    {
                        self.interface.stop();
                        self.connected = false;
                    }
                }
                self.profiles = p;
                self.next = 0;
                self.retry = Instant::now();
                self.error = "";
            }
            Err(e) => {
                self.error = e;
                eprintln!("slot-services: {e}");
            }
        }
    }
    pub fn tick(&mut self, root: &Path, link_busy: bool, link_freq: Option<u32>, ours: &[u32]) {
        if !self.enabled {
            return;
        }
        let now = Instant::now();
        if self.interface.wpa.is_some() {
            let status = self.interface.status();
            if connected(&status) {
                // Also reap/restart a renewal worker that died after the first lease.
                if let Err(e) = self.interface.dhcp() {
                    self.error = e;
                }
                if self.interface.has_ip() {
                    self.connected = true;
                    self.error = "";
                    return;
                }
            } else if self.connected {
                self.deadline = now; // Lost a working association: select again, with backoff.
            }
            if now < self.deadline {
                return;
            }
            self.interface.stop();
            self.connected = false;
            self.error = "HOME_CONNECT_FAILED";
            self.retry = now + Duration::from_secs(2);
        }
        if now < self.retry {
            return;
        }
        // Conservatively defer new scans/associations for the whole Link session. Existing
        // home associations continue. No off-channel background scanning during a game.
        if link_busy || link_freq.is_some() {
            self.retry = now + Duration::from_secs(5);
            return;
        }
        if system::external_radio_owner(ours) {
            self.error = "EXTERNAL_OWNER";
            self.retry = now + Duration::from_secs(10);
            return;
        }
        if self.next >= self.profiles.len() {
            self.reload(root);
            self.retry = now + Duration::from_secs(15);
            if self.profiles.is_empty() {
                if self.error.is_empty() {
                    self.error = "NO_NETWORKS";
                }
                return;
            }
        }
        let Some(profile) = self.profiles.get(self.next) else {
            return;
        };
        // BaseOS initializes the radio asynchronously. Waiting for the interface
        // must not consume a profile and skip the preferred network at boot.
        if !net_exists("wlan0") {
            self.error = "RADIO_UNAVAILABLE";
            self.retry = now + Duration::from_secs(5);
            return;
        }
        if let Err(e) = output("rfkill", &["unblock", "wifi"]) {
            self.error = e;
            self.retry = now + Duration::from_secs(3);
            return;
        }
        match self.interface.start(profile, None, false) {
            Ok(()) => {
                self.next += 1;
                self.error = "";
                self.deadline = now + Duration::from_secs(25);
            }
            Err(e) => {
                self.error = e;
                self.retry = now + Duration::from_secs(3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn routes_conflict_without_mistaking_default_or_our_interface() {
        assert!(subnet_conflict("10.0.0.0/8 dev wlan0"));
        assert!(subnet_conflict("10.42.0.2 dev usb0"));
        assert!(!subnet_conflict(
            "default via 192.168.1.1 dev wlan0\n192.168.1.0/24 dev wlan0\n10.42.0.0/24 dev wlan1"
        ));
    }
    #[test]
    fn capabilities_fail_closed() {
        assert!(!dual_station(
            "Supported interface modes:\n * managed\n * AP"
        ));
        assert!(dual_station(
            "valid interface combinations:\n * #{ managed } <= 2, total <= 2, #channels <= 1"
        ));
        assert!(!permitted(
            "* 5580 MHz [116] (20.0 dBm) (radar detection)",
            5580
        ));
        assert!(!permitted("* 5745 MHz [149] (disabled)", 5745));
        assert!(permitted("* 2412 MHz [1] (20.0 dBm)", 2412));
    }
}
