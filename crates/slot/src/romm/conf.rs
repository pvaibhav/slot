use std::fs;
use std::io::{self, Read};
use std::path::Path;

pub const ROMM_CONF: &str = "Config/romm.conf";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Protocol {
    #[default]
    Https,
    Http,
}

impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Protocol::Https => "https",
            Protocol::Http => "http",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RommConf {
    pub protocol: Protocol,
    pub server: String,
    pub port: Option<u16>,
    pub self_signed: bool,
    pub client_id: String,
    pub device_id: Option<String>,
    pub token: Option<String>,
}

impl RommConf {
    pub fn load(root: &Path) -> RommConf {
        match fs::read_to_string(root.join(ROMM_CONF)) {
            Ok(text) => parse(&text),
            Err(_) => RommConf::default(),
        }
    }

    pub fn save(&self, root: &Path) -> io::Result<()> {
        let path = root.join(ROMM_CONF);
        let mut conf = self.clone();
        if conf.client_id.is_empty() {
            conf.client_id = match fs::read_to_string(&path) {
                Ok(text) => parse(&text).client_id,
                Err(_) => String::new(),
            };
        }
        if conf.client_id.is_empty() {
            conf.client_id = random_id()?;
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        slot_store::atomic_write(&path, render(&conf).as_bytes())
    }

    pub fn host(&self) -> &str {
        let typed = self.server.trim();
        let bare = ["https://", "http://"]
            .iter()
            .find_map(|scheme| {
                let head = typed.get(..scheme.len())?;
                head.eq_ignore_ascii_case(scheme)
                    .then(|| &typed[scheme.len()..])
            })
            .unwrap_or(typed);
        bare.split(['/', '?', '#']).next().unwrap_or("")
    }

    pub fn origin(&self) -> String {
        match self.port {
            Some(port) => format!("{}://{}:{}", self.protocol.name(), self.host(), port),
            None => format!("{}://{}", self.protocol.name(), self.host()),
        }
    }

    pub fn paired(&self) -> bool {
        self.device_id.is_some() && self.token.is_some()
    }

    pub fn unpair(&mut self) {
        self.device_id = None;
        self.token = None;
    }
}

fn parse(text: &str) -> RommConf {
    let mut conf = RommConf::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "protocol" => {
                conf.protocol = match value {
                    "http" => Protocol::Http,
                    _ => Protocol::Https,
                }
            }
            "server" => conf.server = value.to_string(),
            "port" => conf.port = value.parse().ok(),
            "self_signed" => conf.self_signed = value == "true",
            "client_id" => conf.client_id = value.to_string(),
            "device_id" => conf.device_id = some(value),
            "token" => conf.token = some(value),
            _ => {}
        }
    }
    conf
}

fn some(value: &str) -> Option<String> {
    match value.is_empty() {
        true => None,
        false => Some(value.to_string()),
    }
}

fn render(conf: &RommConf) -> String {
    let mut out = String::new();
    out.push_str(&format!("protocol={}\n", conf.protocol.name()));
    out.push_str(&format!("server={}\n", conf.server));
    if let Some(port) = conf.port {
        out.push_str(&format!("port={port}\n"));
    }
    out.push_str(&format!("self_signed={}\n", conf.self_signed));
    out.push_str(&format!("client_id={}\n", conf.client_id));
    if let Some(id) = &conf.device_id {
        out.push_str(&format!("device_id={id}\n"));
    }
    if let Some(token) = &conf.token {
        out.push_str(&format!("token={token}\n"));
    }
    out
}

fn random_id() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
