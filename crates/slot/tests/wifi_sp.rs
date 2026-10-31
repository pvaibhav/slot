use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use slot::wifi::sp::{Run, Wpa};
use slot::wifi::wpa::hex;
use slot::wifi::Backend;

type Calls = Arc<Mutex<Vec<Vec<String>>>>;

struct Rec {
    calls: Calls,
    conf: PathBuf,
    list: String,
    pings: usize,
    saved_text: String,
    status: String,
}

impl Run for Rec {
    fn run(&mut self, program: &str, args: &[&str]) -> Option<String> {
        let mut call = vec![program.to_string()];
        call.extend(args.iter().map(|a| a.to_string()));
        self.calls.lock().unwrap().push(call);
        if program == "/bin/sh" {
            return Some(String::new());
        }
        let cmd = args.get(4).copied().unwrap_or("");
        match cmd {
            "ping" if self.pings > 0 => {
                self.pings -= 1;
                Some("FAIL\n".into())
            }
            "ping" => Some("PONG\n".into()),
            "add_network" => Some("0\n".into()),
            "list_networks" => Some(self.list.clone()),
            "save_config" => {
                std::fs::write(&self.conf, &self.saved_text).unwrap();
                Some("OK\n".into())
            }
            "status" => Some(self.status.clone()),
            _ => Some("OK\n".into()),
        }
    }

    fn spawn(&mut self, program: &str, args: &[&str]) {
        let mut call = vec!["spawn".to_string(), program.to_string()];
        call.extend(args.iter().map(|a| a.to_string()));
        self.calls.lock().unwrap().push(call);
    }

    fn sleep(&mut self, _d: Duration) {}
}

struct Down;

impl Run for Down {
    fn run(&mut self, _program: &str, _args: &[&str]) -> Option<String> {
        None
    }

    fn spawn(&mut self, _program: &str, _args: &[&str]) {}

    fn sleep(&mut self, _d: Duration) {}
}

fn wpa(list: &str, saved_text: &str) -> (Wpa<Rec>, Calls, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("Config")).unwrap();
    let conf = dir.path().join("Config/wifi.conf");
    std::fs::write(&conf, saved_text).unwrap();
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let rec = Rec {
        calls: calls.clone(),
        conf: conf.clone(),
        list: list.into(),
        pings: 0,
        saved_text: saved_text.into(),
        status: "wpa_state=COMPLETED\nid=0\nssid=Home\nip_address=10.0.0.2\n".into(),
    };
    (Wpa::new(rec, dir.path().to_path_buf()), calls, dir)
}

fn wpa_calls(calls: &Calls) -> Vec<Vec<String>> {
    calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c[0] == "/usr/sbin/wpa_cli")
        .map(|c| c[5..].to_vec())
        .collect()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

const TWO: &str = "network id / ssid / bssid / flags\n0\tHome\tany\t\n1\tCabin\tany\t\n";
const TWO_CONF: &str = "update_config=1\nnetwork={\n\tssid=\"Home\"\n\tid_str=\"auto\"\n}\nnetwork={\n\tssid=\"Cabin\"\n\tid_str=\"manual\"\n}\n";

#[test]
fn start_adds_the_header_to_a_hand_written_conf() {
    let (mut w, _, d) = wpa(TWO, "network={\n\tssid=\"Home\"\n\tpsk=\"pw\"\n}\n");
    assert!(w.start());
    let conf = std::fs::read_to_string(d.path().join("Config/wifi.conf")).unwrap();
    assert!(
        conf.starts_with("ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\nnetwork={"),
        "{conf}"
    );
}

#[test]
fn start_creates_the_conf_when_missing() {
    let (mut w, _, d) = wpa(TWO, "");
    std::fs::remove_file(d.path().join("Config/wifi.conf")).unwrap();
    assert!(w.start());
    let conf = std::fs::read_to_string(d.path().join("Config/wifi.conf")).unwrap();
    assert!(
        conf.contains("ctrl_interface=/var/run/wpa_supplicant"),
        "{conf}"
    );
}

#[test]
fn start_leaves_a_complete_conf_alone() {
    let text = "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\n";
    let (mut w, _, d) = wpa(TWO, text);
    assert!(w.start());
    assert_eq!(
        std::fs::read_to_string(d.path().join("Config/wifi.conf")).unwrap(),
        text
    );
}

#[test]
fn start_runs_slotlink_with_the_card_conf() {
    let (mut w, calls, d) = wpa(TWO, TWO_CONF);
    assert!(w.start());
    let root = d.path().display().to_string();
    let c = calls.lock().unwrap();
    assert_eq!(
        c[0],
        [
            "/bin/sh".to_string(),
            format!("{root}/System/slotlink.sh"),
            "wifi".into(),
            "start".into(),
            format!("{root}/Config/wifi.conf"),
        ]
    );
}

#[test]
fn start_fails_when_slotlink_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut w = Wpa::new(Down, dir.path().to_path_buf());
    assert!(!w.start());
}

#[test]
fn stop_runs_wifi_stop() {
    let (mut w, calls, d) = wpa(TWO, TWO_CONF);
    w.stop();
    let root = d.path().display().to_string();
    assert_eq!(
        calls.lock().unwrap()[0],
        [
            "/bin/sh".to_string(),
            format!("{root}/System/slotlink.sh"),
            "wifi".into(),
            "stop".into()
        ]
    );
}

#[test]
fn block_runs_wifi_off() {
    let (mut w, calls, d) = wpa(TWO, TWO_CONF);
    w.block();
    let root = d.path().display().to_string();
    assert_eq!(
        calls.lock().unwrap()[0],
        [
            "/bin/sh".to_string(),
            format!("{root}/System/slotlink.sh"),
            "wifi".into(),
            "off".into()
        ]
    );
}

#[test]
fn join_passes_the_ssid_as_one_argument() {
    let (mut w, calls, _d) = wpa("network id / ssid / bssid / flags\n", "update_config=1\n");
    let ssid = "a b; rm -rf /";
    assert_eq!(w.add(ssid, "pw", false), Some(0));
    let c = wpa_calls(&calls);
    assert!(
        c.contains(&strs(&["set_network", "0", "ssid", &hex(ssid)])),
        "{c:?}"
    );
    assert!(calls
        .lock()
        .unwrap()
        .iter()
        .all(|c| c[0] != "sh" && !c.iter().any(|a| a == "-c")));
}

#[test]
fn odd_names_reach_wpa_cli_intact() {
    let (mut w, calls, _d) = wpa("network id / ssid / bssid / flags\n", "update_config=1\n");
    assert_eq!(w.add("Caf\u{e9} \"5G\"", "pass \"word\"", false), Some(0));
    let c = wpa_calls(&calls);
    assert!(
        c.contains(&strs(&[
            "set_network",
            "0",
            "ssid",
            &hex("Caf\u{e9} \"5G\"")
        ])),
        "{c:?}"
    );
    assert!(
        c.contains(&strs(&["set_network", "0", "psk", "\"pass \"word\"\""])),
        "{c:?}"
    );
    assert!(
        c.contains(&strs(&["set_network", "0", "id_str", "\"auto\""])),
        "{c:?}"
    );
    assert!(c.contains(&strs(&["save_config"])));
    let all = calls.lock().unwrap();
    assert!(all.iter().all(
        |c| c[1..5] == ["-p", "/var/run/wpa_supplicant", "-i", "wlan0"]
            || c[0] != "/usr/sbin/wpa_cli"
    ));
}

#[test]
fn open_and_hidden_networks_are_set_up_for_it() {
    let (mut w, calls, _d) = wpa("network id / ssid / bssid / flags\n", "update_config=1\n");
    w.add("Attic", "", true);
    let c = wpa_calls(&calls);
    assert!(c.contains(&strs(&["set_network", "0", "key_mgmt", "NONE"])));
    assert!(c.contains(&strs(&["set_network", "0", "scan_ssid", "1"])));
    assert!(!c
        .iter()
        .any(|a| a.get(2).map(String::as_str) == Some("psk")));
}

#[test]
fn saving_reapplies_auto_join() {
    let (mut w, _, d) = wpa(TWO, TWO_CONF);
    w.set_auto_join(1, false);
    let conf = std::fs::read_to_string(d.path().join("Config/wifi.conf")).unwrap();
    let cabin = conf
        .split("network={")
        .find(|b| b.contains("Cabin"))
        .unwrap();
    assert!(cabin.contains("disabled=1"), "{conf}");
}

#[test]
fn finishing_a_connect_enables_auto_networks_only() {
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.finish_connect();
    let enables: Vec<_> = wpa_calls(&calls)
        .into_iter()
        .filter(|c| c[0] == "enable_network")
        .collect();
    assert_eq!(enables, [strs(&["enable_network", "0"])]);
}

#[test]
fn finishing_a_connect_disables_manual_networks_that_are_not_current() {
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.finish_connect();
    assert!(wpa_calls(&calls).contains(&strs(&["disable_network", "1"])));
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.runner_mut().status = "wpa_state=COMPLETED\nid=1\nssid=Cabin\nip_address=10.0.0.2\n".into();
    w.finish_connect();
    assert!(!wpa_calls(&calls).iter().any(|c| c[0] == "disable_network"));
}

#[test]
fn saved_networks_come_from_the_file_when_the_service_is_off() {
    let (_w, _, d) = wpa(TWO, TWO_CONF);
    let mut off = Wpa::new(Down, d.path().to_path_buf());
    let s = off.saved();
    assert_eq!(
        s.iter()
            .map(|n| (n.id, n.ssid.as_str(), n.auto_join))
            .collect::<Vec<_>>(),
        [(0, "Home", true), (1, "Cabin", false)]
    );
}

#[test]
fn edits_reach_the_file_while_the_service_is_off() {
    let (_w, _, d) = wpa(TWO, TWO_CONF);
    let mut off = Wpa::new(Down, d.path().to_path_buf());
    off.set_auto_join(0, false);
    assert_eq!(
        off.saved()
            .iter()
            .map(|n| (n.ssid.as_str(), n.auto_join))
            .collect::<Vec<_>>(),
        [("Home", false), ("Cabin", false)]
    );
    off.forget(1);
    assert_eq!(
        off.saved()
            .iter()
            .map(|n| n.ssid.as_str())
            .collect::<Vec<_>>(),
        ["Home"]
    );
}

#[test]
fn a_join_while_the_service_is_off_is_written_to_the_file() {
    let (_w, _, d) = wpa(TWO, TWO_CONF);
    let mut off = Wpa::new(Down, d.path().to_path_buf());
    assert_eq!(off.add("Cafe", "", true), Some(2));
    assert_eq!(off.add("Attic", "password", false), Some(3));
    assert_eq!(
        off.saved()
            .iter()
            .map(|n| (n.id, n.ssid.as_str(), n.auto_join))
            .collect::<Vec<_>>(),
        [
            (0, "Home", true),
            (1, "Cabin", false),
            (2, "Cafe", true),
            (3, "Attic", true)
        ]
    );
    let conf = std::fs::read_to_string(d.path().join("Config/wifi.conf")).unwrap();
    assert!(
        conf.ends_with("network={\n\tssid=43616665\n\tkey_mgmt=NONE\n\tid_str=\"auto\"\n\tscan_ssid=1\n}\nnetwork={\n\tssid=4174746963\n\tpsk=\"password\"\n\tid_str=\"auto\"\n}\n"),
        "{conf}"
    );
}

#[test]
fn dhcp_runs_in_the_background_on_every_association() {
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.status();
    w.status();
    let spawns: Vec<_> = calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c[0] == "spawn")
        .cloned()
        .collect();
    assert_eq!(spawns.len(), 1);
    assert_eq!(
        spawns[0],
        strs(&[
            "spawn",
            "/sbin/udhcpc",
            "-i",
            "wlan0",
            "-S",
            "-t",
            "5",
            "-T",
            "7",
            "-b",
            "-p",
            "/run/udhcpc.wlan0.pid"
        ])
    );
    w.runner_mut().status = "wpa_state=SCANNING\n".into();
    w.status();
    w.runner_mut().status = "wpa_state=COMPLETED\nid=1\nssid=Cabin\nip_address=10.0.0.2\n".into();
    w.status();
    assert_eq!(
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c[0] == "spawn")
            .count(),
        2
    );
}

#[test]
fn connecting_and_disconnecting_drop_the_old_address() {
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.connect(1);
    w.disconnect();
    let c: Vec<String> = calls.lock().unwrap().iter().map(|c| c.join(" ")).collect();
    let flush = "/sbin/ip addr flush dev wlan0";
    let select = c
        .iter()
        .position(|l| l.ends_with("select_network 1"))
        .unwrap();
    assert!(c[..select].iter().any(|l| l == flush), "{c:?}");
    let disc = c.iter().position(|l| l.ends_with(" disconnect")).unwrap();
    assert!(c[disc..].iter().any(|l| l == flush), "{c:?}");
}

#[test]
fn disconnecting_then_connecting_reconnects_first() {
    let (mut w, calls, _d) = wpa(TWO, TWO_CONF);
    w.disconnect();
    w.connect(1);
    let c: Vec<String> = wpa_calls(&calls)
        .into_iter()
        .map(|c| c.join(" "))
        .filter(|c| c != "status")
        .collect();
    assert_eq!(c, ["disconnect", "reconnect", "select_network 1"]);
}
