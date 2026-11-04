use std::time::{Duration, Instant};

use slot::wifi::attempt::Attempt;
use slot::wifi::wpa::{
    apply_auto_join, edit_network, ensure_header, hex, parse_saved, parse_scan, parse_status,
    password_ok, temp_disabled, unescape, Edit,
};
use slot::wifi::{Failure, Phase, State, Status};

const SCAN: &str = "bssid / frequency / signal level / flags / ssid\naa:bb\t2412\t-70\t[WPA2-PSK-CCMP][ESS]\tHome\naa:bc\t5180\t-50\t[WPA2-PSK-CCMP][ESS]\tHome\naa:bd\t2437\t-80\t[ESS]\tCafe \"Guest\"\naa:be\t2437\t-40\t[WPA2-PSK-CCMP][ESS]\t\naa:bf\t2462\t-80\t[WEP][ESS]\tAttic\n";

#[test]
fn scans_merge_drop_hidden_and_sort() {
    let s = parse_scan(SCAN);
    let got: Vec<_> = s
        .iter()
        .map(|n| (n.ssid.as_str(), n.signal, n.secured))
        .collect();
    assert_eq!(
        got,
        [
            ("Home", -50, true),
            ("Attic", -80, true),
            ("Cafe \"Guest\"", -80, false)
        ]
    );
}

#[test]
fn saved_networks_read_auto_join_from_the_file() {
    let list =
        "network id / ssid / bssid / flags\n0\tHome\tany\t[CURRENT]\n1\tCabin\tany\t[DISABLED]\n";
    let conf = "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\nnetwork={\n\tssid=\"Home\"\n\tid_str=\"auto\"\n}\nnetwork={\n\tssid=\"Cabin\"\n\tid_str=\"manual\"\n\tdisabled=1\n}\n";
    let s = parse_saved(list, conf);
    let got: Vec<_> = s
        .iter()
        .map(|n| (n.id, n.ssid.as_str(), n.auto_join))
        .collect();
    assert_eq!(got, [(0, "Home", true), (1, "Cabin", false)]);
}

#[test]
fn status_reads_state_name_and_address() {
    let s = parse_status("bssid=aa:bb\nssid=Home\nwpa_state=COMPLETED\nip_address=192.168.1.42\n");
    assert_eq!(
        (s.state, s.ssid.as_deref(), s.ip.as_deref()),
        (State::Completed, Some("Home"), Some("192.168.1.42"))
    );
    assert_eq!(
        parse_status("wpa_state=4WAY_HANDSHAKE\n").state,
        State::Handshake
    );
    assert_eq!(
        parse_status("wpa_state=ASSOCIATING\n").state,
        State::Associating
    );
    assert_eq!(parse_status("wpa_state=SCANNING\n").state, State::Scanning);
    assert_eq!(parse_status("wpa_state=INACTIVE\n").state, State::Idle);
}

#[test]
fn ssids_go_to_wpa_cli_as_hex() {
    assert_eq!(hex("a b\""), "61206222");
    assert_eq!(hex("\u{e9}"), "c3a9");
}

#[test]
fn passwords_are_empty_or_8_to_63_characters() {
    assert!(password_ok("", false));
    assert!(!password_ok("", true));
    assert!(!password_ok("1234567", true));
    assert!(password_ok("12345678", true));
    assert!(password_ok(&"\u{20ac}".repeat(63), true));
    assert!(!password_ok(&"a".repeat(64), true));
}

#[test]
fn auto_join_flags_follow_id_str() {
    let conf = "update_config=1\nnetwork={\n\tssid=\"A\"\n\tid_str=\"manual\"\n}\nnetwork={\n\tssid=\"B\"\n\tid_str=\"auto\"\n\tdisabled=1\n}\n";
    let want = "update_config=1\nnetwork={\n\tssid=\"A\"\n\tid_str=\"manual\"\n\tdisabled=1\n}\nnetwork={\n\tssid=\"B\"\n\tid_str=\"auto\"\n}\n";
    assert_eq!(apply_auto_join(conf), want);
    assert_eq!(apply_auto_join(want), want);
}

#[test]
fn a_hand_disabled_network_stays_disabled_and_reads_as_auto_join_off() {
    let conf = "network={\n\tssid=\"A\"\n\tdisabled=1\n}\nnetwork={\n\tssid=\"B\"\n}\n";
    assert_eq!(apply_auto_join(conf), conf);
    let list = "network id / ssid / bssid / flags\n0\tA\tany\t[DISABLED]\n1\tB\tany\t\n";
    assert_eq!(
        parse_saved(list, conf)
            .iter()
            .map(|n| n.auto_join)
            .collect::<Vec<_>>(),
        [false, true]
    );
}

#[test]
fn escaped_names_from_wpa_cli_are_decoded() {
    let scan = "bssid / frequency / signal level / flags / ssid\naa\t1\t-50\t[ESS]\tCaf\\xc3\\xa9 \\\"5G\\\" a\\\\b\n";
    assert_eq!(parse_scan(scan)[0].ssid, "Caf\u{e9} \"5G\" a\\b");
    let list = "network id / ssid / bssid / flags\n0\tCaf\\xc3\\xa9\tany\t\n";
    assert_eq!(parse_saved(list, "")[0].ssid, "Caf\u{e9}");
    assert_eq!(
        parse_status("ssid=Caf\\xc3\\xa9\nwpa_state=COMPLETED\n")
            .ssid
            .as_deref(),
        Some("Caf\u{e9}")
    );
    assert_eq!(unescape("tab\\there\\e"), "tab\there\u{1b}");
}

#[test]
fn networks_can_be_edited_in_the_file() {
    let conf = "update_config=1\nnetwork={\n\tssid=\"A\"\n\tpsk=\"old password\"\n\tid_str=\"auto\"\n}\nnetwork={\n\tssid=\"B\"\n\tid_str=\"auto\"\n}\n";
    assert_eq!(
        edit_network(conf, 1, Edit::Remove),
        "update_config=1\nnetwork={\n\tssid=\"A\"\n\tpsk=\"old password\"\n\tid_str=\"auto\"\n}\n"
    );
    let manual = edit_network(conf, 0, Edit::IdStr("manual"));
    assert!(
        manual.contains("\tid_str=\"manual\"\n\tdisabled=1\n}\nnetwork={\n\tssid=\"B\""),
        "{manual}"
    );
    assert_eq!(edit_network(conf, 5, Edit::Remove), conf);
}

#[test]
fn temp_disabled_reads_the_flag() {
    let list = "network id / ssid / bssid / flags\n3\tHome\tany\t[TEMP-DISABLED]\n";
    assert!(temp_disabled(list, 3));
    assert!(!temp_disabled(list, 4));
}

#[test]
fn ensure_header_adds_ctrl_and_update_config() {
    let out = ensure_header("network={\n\tssid=\"Home\"\n}\n", "/var/run/wpa_supplicant");
    assert!(
        out.starts_with("ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\n"),
        "{out}"
    );
    assert!(out.contains("ssid=\"Home\""), "{out}");
}

#[test]
fn ensure_header_leaves_a_complete_file_alone() {
    let conf =
        "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\nnetwork={\n\tssid=\"Home\"\n}\n";
    assert_eq!(ensure_header(conf, "/var/run/wpa_supplicant"), conf);
}

#[test]
fn ensure_header_points_a_foreign_ctrl_interface_at_ours() {
    let ctrl = "/var/run/wpa_supplicant";
    let conf = "ctrl_interface=/tmp/wpa\nupdate_config=1\nnetwork={\n\tssid=\"Home\"\n}\n";
    assert_eq!(
        ensure_header(conf, ctrl),
        "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=1\nnetwork={\n\tssid=\"Home\"\n}\n"
    );
    let dir = "ctrl_interface=DIR=/var/run/wpa_supplicant GROUP=0\nupdate_config=1\n";
    assert_eq!(ensure_header(dir, ctrl), dir);
}

#[test]
fn ensure_header_turns_update_config_on() {
    let conf =
        "ctrl_interface=/var/run/wpa_supplicant\nupdate_config=0\nnetwork={\n\tssid=\"Home\"\n}\n";
    assert_eq!(
        ensure_header(conf, "/var/run/wpa_supplicant"),
        conf.replace("update_config=0", "update_config=1")
    );
}

#[test]
fn an_ssid_with_quotes_and_unicode_round_trips() {
    let name = "Caf\u{e9} \"5G\"";
    let conf = format!("network={{\n\tssid={}\n}}\n", hex(name));
    let saved = edit_network(&conf, 0, Edit::IdStr("auto"));
    let list = format!(
        "network id / ssid / bssid / flags\n0\t{}\tany\t\n",
        name.replace('"', "\\\"")
    );
    assert_eq!(parse_saved(&list, &saved)[0].ssid, name);
}

fn st(state: State, ssid: Option<&str>, ip: Option<&str>) -> Status {
    Status {
        state,
        ssid: ssid.map(String::from),
        ip: ip.map(String::from),
    }
}

fn at(base: Instant, secs: u64) -> Instant {
    base + Duration::from_secs(secs)
}

#[test]
fn phases_follow_wpa_state() {
    let t = Instant::now();
    let mut a = Attempt::new(0, "Home", t);
    assert_eq!(
        a.observe(&st(State::Scanning, None, None), false, true, at(t, 1)),
        Phase::Connecting
    );
    assert_eq!(
        a.observe(
            &st(State::Associating, Some("Home"), None),
            false,
            true,
            at(t, 2)
        ),
        Phase::Connecting
    );
    assert_eq!(
        a.observe(
            &st(State::Handshake, Some("Home"), None),
            false,
            true,
            at(t, 3)
        ),
        Phase::Authenticating
    );
    assert_eq!(
        a.observe(
            &st(State::Completed, Some("Home"), None),
            false,
            true,
            at(t, 4)
        ),
        Phase::GettingIp
    );
    assert_eq!(
        a.observe(
            &st(State::Completed, Some("Home"), Some("10.0.0.2")),
            false,
            true,
            at(t, 5)
        ),
        Phase::Connected
    );
    assert!(a.done());
    assert_eq!(Phase::GettingIp.label(), "Getting IP");
    assert_eq!(Phase::Authenticating.label(), "Authenticating");
    assert_eq!(Phase::Connecting.label(), "Connecting");
    assert_eq!(Phase::Connected.label(), "Connected");
}

#[test]
fn completed_on_another_network_is_not_connected() {
    let t = Instant::now();
    let mut a = Attempt::new(0, "Home", t);
    assert_eq!(
        a.observe(
            &st(State::Completed, Some("Cabin"), Some("10.0.0.2")),
            false,
            true,
            at(t, 1)
        ),
        Phase::Connecting
    );
}

#[test]
fn failures_are_named() {
    let t = Instant::now();
    let mut a = Attempt::new(0, "Home", t);
    a.observe(
        &st(State::Handshake, Some("Home"), None),
        false,
        true,
        at(t, 2),
    );
    assert_eq!(
        a.observe(&st(State::Scanning, None, None), true, true, at(t, 3)),
        Phase::Failed(Failure::WrongPassword)
    );

    let mut a = Attempt::new(0, "Gone", t);
    assert_eq!(
        a.observe(&st(State::Scanning, None, None), false, false, at(t, 19)),
        Phase::Connecting
    );
    assert_eq!(
        a.observe(&st(State::Scanning, None, None), false, false, at(t, 20)),
        Phase::Failed(Failure::NotFound)
    );

    let mut a = Attempt::new(0, "Home", t);
    a.observe(
        &st(State::Completed, Some("Home"), None),
        false,
        true,
        at(t, 5),
    );
    assert_eq!(
        a.observe(
            &st(State::Completed, Some("Home"), None),
            false,
            true,
            at(t, 20)
        ),
        Phase::Failed(Failure::NoIp)
    );

    let mut a = Attempt::new(0, "Home", t);
    a.observe(
        &st(State::Associating, Some("Home"), None),
        false,
        true,
        at(t, 5),
    );
    assert_eq!(
        a.observe(&st(State::Scanning, None, None), false, true, at(t, 20)),
        Phase::Failed(Failure::Other)
    );

    let labels = [
        Failure::WrongPassword,
        Failure::NotFound,
        Failure::NoIp,
        Failure::Other,
    ]
    .map(|f| Phase::Failed(f).label());
    assert_eq!(
        labels,
        [
            "Wrong Password",
            "Not Found",
            "No IP Address",
            "Could Not Connect"
        ]
    );
}

#[test]
fn a_finished_attempt_stays_finished() {
    let t = Instant::now();
    let mut a = Attempt::new(0, "Home", t);
    a.observe(
        &st(State::Handshake, Some("Home"), None),
        false,
        true,
        at(t, 2),
    );
    a.observe(&st(State::Idle, None, None), true, true, at(t, 3));
    assert_eq!(
        a.observe(
            &st(State::Completed, Some("Home"), Some("10.0.0.2")),
            false,
            true,
            at(t, 4)
        ),
        Phase::Failed(Failure::WrongPassword)
    );
    assert!(a.done());
}

#[test]
fn a_literal_backslash_x_in_a_scanned_name_is_not_decoded_twice() {
    let scan = "bssid / frequency / signal level / flags / ssid\naa\t1\t-50\t[ESS]\ta\\\\x41\n";
    assert_eq!(parse_scan(scan)[0].ssid, "a\\x41");
}
