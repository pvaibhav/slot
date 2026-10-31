use super::{Saved, Scanned, State, Status};

fn rows(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').collect())
}

pub fn parse_scan(text: &str) -> Vec<Scanned> {
    let mut out: Vec<Scanned> = Vec::new();
    for f in rows(text) {
        let (Some(signal), Some(flags), Some(ssid)) = (
            f.get(2).and_then(|v| v.trim().parse().ok()),
            f.get(3),
            f.get(4),
        ) else {
            continue;
        };
        if ssid.is_empty() {
            continue;
        }
        let secured = flags.contains("WPA") || flags.contains("WEP");
        let ssid = unescape(ssid);
        match out.iter_mut().find(|n| n.ssid == ssid) {
            Some(n) if n.signal < signal => {
                n.signal = signal;
                n.secured = secured;
            }
            Some(_) => {}
            None => out.push(Scanned {
                ssid,
                signal,
                secured,
            }),
        }
    }
    out.sort_by(|a, b| b.signal.cmp(&a.signal).then_with(|| a.ssid.cmp(&b.ssid)));
    out
}

pub(crate) fn blocks(conf: &str) -> Vec<Vec<&str>> {
    let mut out = Vec::new();
    let mut cur: Option<Vec<&str>> = None;
    for line in conf.lines() {
        let t = line.trim();
        if t.starts_with("network={") {
            cur = Some(Vec::new());
        } else if t == "}" {
            out.extend(cur.take());
        } else if let Some(b) = cur.as_mut() {
            b.push(t);
        }
    }
    out
}

pub fn parse_saved(list_networks: &str, conf: &str) -> Vec<Saved> {
    let blocks = blocks(conf);
    rows(list_networks)
        .enumerate()
        .filter_map(|(i, f)| {
            let id = f.first()?.trim().parse().ok()?;
            let ssid = unescape(f.get(1)?);
            let manual = blocks.get(i).is_some_and(|b| {
                b.contains(&"id_str=\"manual\"")
                    || (!b.iter().any(|l| l.starts_with("id_str=")) && b.contains(&"disabled=1"))
            });
            Some(Saved {
                id,
                ssid,
                auto_join: !manual,
            })
        })
        .collect()
}

pub fn parse_status(text: &str) -> Status {
    let mut s = Status {
        state: State::Idle,
        ssid: None,
        ip: None,
    };
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k {
            "wpa_state" => {
                s.state = match v {
                    "COMPLETED" => State::Completed,
                    "4WAY_HANDSHAKE" | "GROUP_HANDSHAKE" => State::Handshake,
                    "ASSOCIATING" | "ASSOCIATED" | "AUTHENTICATING" => State::Associating,
                    "SCANNING" => State::Scanning,
                    _ => State::Idle,
                }
            }
            "ssid" => s.ssid = Some(unescape(v)),
            "ip_address" => s.ip = Some(v.to_string()),
            _ => {}
        }
    }
    s
}

pub fn temp_disabled(list_networks: &str, id: u32) -> bool {
    rows(list_networks).any(|f| {
        f.first().and_then(|v| v.trim().parse::<u32>().ok()) == Some(id)
            && f.get(3).is_some_and(|fl| fl.contains("[TEMP-DISABLED]"))
    })
}

#[derive(Clone, Copy)]
pub enum Edit<'a> {
    Remove,
    IdStr(&'a str),
}

pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' || i + 1 >= b.len() {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let (byte, used) = match b[i + 1] {
            b'"' => (b'"', 2),
            b'\\' => (b'\\', 2),
            b'e' => (0x1b, 2),
            b'n' => (b'\n', 2),
            b'r' => (b'\r', 2),
            b't' => (b'\t', 2),
            b'x' => match s
                .get(i + 2..i + 4)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                Some(v) => (v, 4),
                None => (b'\\', 1),
            },
            _ => (b'\\', 1),
        };
        out.push(byte);
        i += used;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn edit_network(conf: &str, index: usize, edit: Edit) -> String {
    let mut out = String::new();
    let (mut at, mut inside, mut target, mut placed) = (0, false, false, false);
    for line in conf.split_inclusive('\n') {
        let t = line.trim();
        if !inside && t.starts_with("network={") {
            inside = true;
            target = at == index;
            at += 1;
            placed = false;
            if !(target && matches!(edit, Edit::Remove)) {
                out.push_str(line);
            }
            continue;
        }
        if inside && t == "}" {
            inside = false;
            if target {
                match edit {
                    Edit::Remove => continue,
                    Edit::IdStr(v) if !placed => out.push_str(&format!("\tid_str=\"{v}\"\n")),
                    _ => {}
                }
            }
            out.push_str(line);
            continue;
        }
        if inside && target {
            match edit {
                Edit::Remove => continue,
                Edit::IdStr(v) if t.starts_with("id_str=") => {
                    out.push_str(&format!("\tid_str=\"{v}\"\n"));
                    placed = true;
                    continue;
                }
                _ => {}
            }
        }
        out.push_str(line);
    }
    apply_auto_join(&out)
}

pub fn append_network(conf: &str, ssid: &str, password: &str, hidden: bool) -> String {
    let mut out = conf.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&format!("network={{\n\tssid={}\n", hex(ssid)));
    match password.is_empty() {
        true => out.push_str("\tkey_mgmt=NONE\n"),
        false => out.push_str(&format!("\tpsk=\"{password}\"\n")),
    }
    out.push_str("\tid_str=\"auto\"\n");
    if hidden {
        out.push_str("\tscan_ssid=1\n");
    }
    out.push_str("}\n");
    out
}

pub fn hex(ssid: &str) -> String {
    ssid.bytes().map(|b| format!("{b:02x}")).collect()
}

pub fn password_ok(pw: &str, secured: bool) -> bool {
    let n = pw.chars().count();
    if n == 0 {
        !secured
    } else {
        (8..=63).contains(&n)
    }
}

pub fn apply_auto_join(conf: &str) -> String {
    let mut out = String::new();
    let mut block: Option<Vec<&str>> = None;
    for line in conf.split_inclusive('\n') {
        let t = line.trim();
        match block.as_mut() {
            None if t.starts_with("network={") => {
                out.push_str(line);
                block = Some(Vec::new());
            }
            None => out.push_str(line),
            Some(b) if t == "}" => {
                let manual = b.iter().any(|l| l.trim() == "id_str=\"manual\"");
                let ours = b.iter().any(|l| l.trim().starts_with("id_str="));
                for l in b
                    .iter()
                    .filter(|l| !ours || !l.trim().starts_with("disabled="))
                {
                    out.push_str(l);
                }
                if manual {
                    out.push_str("\tdisabled=1\n");
                }
                out.push_str(line);
                block = None;
            }
            Some(b) => b.push(line),
        }
    }
    if let Some(b) = block {
        for l in b {
            out.push_str(l);
        }
    }
    out
}

pub fn ensure_header(conf: &str, ctrl: &str) -> String {
    let ours = |v: &str| {
        let dir = v.strip_prefix("DIR=").unwrap_or(v);
        dir.split_whitespace().next() == Some(ctrl)
    };
    let mut body = String::new();
    for line in conf.split_inclusive('\n') {
        let t = line.trim();
        match t.split_once('=') {
            Some(("ctrl_interface", v)) if !ours(v) => {
                body.push_str(&format!("ctrl_interface={ctrl}\n"))
            }
            Some(("update_config", v)) if v != "1" => body.push_str("update_config=1\n"),
            _ => body.push_str(line),
        }
    }
    let has = |key: &str| body.lines().any(|l| l.trim_start().starts_with(key));
    let mut head = String::new();
    if !has("ctrl_interface=") {
        head.push_str(&format!("ctrl_interface={ctrl}\n"));
    }
    if !has("update_config=") {
        head.push_str("update_config=1\n");
    }
    format!("{head}{body}")
}
