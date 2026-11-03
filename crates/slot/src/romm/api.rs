use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::conf::RommConf;
use crate::update::CONNECT_TIMEOUT;

const READ_TIMEOUT: Duration = Duration::from_secs(30);
const CHUNK: usize = 64 * 1024;
const SCOPES: [&str; 3] = ["devices.read", "devices.write", "roms.read"];

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub device_code: String,
    pub user_code: String,
    pub path: String,
    pub interval: u64,
    pub expires_in: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Poll {
    Waiting,
    SlowDown,
    Denied,
    Expired,
    Paired { token: String, device_id: String },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    pub id: String,
    pub rom_id: u64,
    pub file_ids: Vec<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RomFile {
    pub id: u64,
    pub file_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RomInfo {
    pub platform_slug: String,
    pub fs_name: String,
    pub files: Vec<RomFile>,
}

#[derive(Clone, Debug, Default)]
pub struct Cancel {
    own: Arc<AtomicBool>,
    halt: Arc<AtomicBool>,
}

impl Cancel {
    pub fn new(halt: Arc<AtomicBool>) -> Cancel {
        Cancel {
            own: Arc::default(),
            halt,
        }
    }

    pub fn set(&self) {
        self.own.store(true, Ordering::SeqCst);
    }

    pub fn is_set(&self) -> bool {
        self.own.load(Ordering::SeqCst) || self.halt.load(Ordering::SeqCst)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done,
    AlreadyInstalled,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum ApiError {
    Unauthorized,
    NotFound,
    Client(u16),
    Server(u16),
    Offline,
    Cancelled,
    Truncated,
    Write(String),
}

pub trait Api: Send {
    fn version(&self) -> Result<String, ApiError>;
    fn pair_start(&self, conf: &RommConf) -> Result<Pending, ApiError>;
    fn pair_poll(&self, device_code: &str) -> Result<Poll, ApiError>;
    fn enable_install(&self, device_id: &str, token: &str) -> Result<(), ApiError>;
    fn claim(&self) -> Result<Vec<Install>, ApiError>;
    fn device_exists(&self) -> Result<bool, ApiError>;
    fn rom(&self, id: u64) -> Result<RomInfo, ApiError>;
    fn download(
        &self,
        rom: u64,
        name: &str,
        file_ids: &[u64],
        to: &Path,
        cancel: &Cancel,
    ) -> Result<(), ApiError>;
    fn report(&self, id: &str, outcome: &Outcome) -> Result<(), ApiError>;
    fn delete_device(&self) -> Result<(), ApiError>;
}

pub fn version_ok(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or("");
    let mut parts = core.split('.');
    let major = parts.next().and_then(|p| p.parse::<u64>().ok());
    if major.is_none() {
        return true;
    }
    let minor = parts.next().and_then(|p| p.parse::<u64>().ok());
    let patch = parts.next().and_then(|p| p.parse::<u64>().ok());
    match (major, minor, patch, parts.next()) {
        (Some(major), Some(minor), Some(_), None) => (major, minor) >= (5, 4),
        _ => false,
    }
}

pub fn status_error(status: u16) -> ApiError {
    match status {
        401 => ApiError::Unauthorized,
        404 => ApiError::NotFound,
        400..=499 => ApiError::Client(status),
        _ => ApiError::Server(status),
    }
}

pub fn parse_version(json: &str) -> Result<String, ApiError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| ApiError::Server(200))?;
    v["SYSTEM"]["VERSION"]
        .as_str()
        .map(str::to_string)
        .ok_or(ApiError::Server(200))
}

pub fn parse_pending(json: &str) -> Result<Pending, ApiError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| ApiError::Server(200))?;
    let text = |key: &str| v[key].as_str().map(str::to_string);
    Ok(Pending {
        device_code: text("device_code").ok_or(ApiError::Server(200))?,
        user_code: text("user_code").ok_or(ApiError::Server(200))?,
        path: text("verification_path_complete").ok_or(ApiError::Server(200))?,
        interval: v["interval"].as_u64().unwrap_or(5),
        expires_in: v["expires_in"].as_u64().unwrap_or(600),
    })
}

pub fn parse_poll(status: u16, body: &str) -> Result<Poll, ApiError> {
    let v: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);
    match status {
        200 => {
            let token = v["access_token"].as_str().ok_or(ApiError::Server(200))?;
            let device_id = v["device_id"].as_str().ok_or(ApiError::Server(200))?;
            Ok(Poll::Paired {
                token: token.to_string(),
                device_id: device_id.to_string(),
            })
        }
        400 => match v["detail"].as_str() {
            Some("authorization_pending") => Ok(Poll::Waiting),
            Some("slow_down") => Ok(Poll::SlowDown),
            Some("access_denied") => Ok(Poll::Denied),
            Some("expired_token") => Ok(Poll::Expired),
            _ => Err(ApiError::Client(400)),
        },
        other => Err(status_error(other)),
    }
}

pub fn parse_installs(json: &str) -> Result<Vec<Install>, ApiError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| ApiError::Server(200))?;
    let list = v.as_array().ok_or(ApiError::Server(200))?;
    list.iter()
        .map(|item| {
            Ok(Install {
                id: item["id"]
                    .as_str()
                    .ok_or(ApiError::Server(200))?
                    .to_string(),
                rom_id: item["rom_id"].as_u64().ok_or(ApiError::Server(200))?,
                file_ids: item["file_ids"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|n| n.as_u64()).collect())
                    .unwrap_or_default(),
            })
        })
        .collect()
}

pub fn parse_rom(json: &str) -> Result<RomInfo, ApiError> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|_| ApiError::Server(200))?;
    Ok(RomInfo {
        platform_slug: v["platform_slug"]
            .as_str()
            .ok_or(ApiError::Server(200))?
            .to_string(),
        fs_name: v["fs_name"]
            .as_str()
            .ok_or(ApiError::Server(200))?
            .to_string(),
        files: v["files"]
            .as_array()
            .map(|files| {
                files
                    .iter()
                    .filter_map(|file| {
                        Some(RomFile {
                            id: file["id"].as_u64()?,
                            file_name: file["file_name"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub fn bearer(token: Option<&str>) -> Option<String> {
    token.map(|t| format!("Bearer {t}"))
}

pub fn percent_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub fn content_path(rom: u64, name: &str, file_ids: &[u64]) -> String {
    let mut path = format!("/api/roms/{rom}/content/{}", percent_encode(name));
    if !file_ids.is_empty() {
        let ids: Vec<String> = file_ids.iter().map(u64::to_string).collect();
        path.push_str(&format!("?file_ids={}", ids.join(",")));
    }
    path
}

pub fn report_body(outcome: &Outcome) -> String {
    match outcome {
        Outcome::Done => serde_json::json!({"status": "done"}),
        Outcome::AlreadyInstalled => serde_json::json!({"status": "already_installed"}),
        Outcome::Failed(reason) => serde_json::json!({"status": "failed", "reason": reason}),
    }
    .to_string()
}

pub fn device_name(model: Option<&str>) -> String {
    match model {
        Some(model) => format!("slot on {model}"),
        None => "slot".to_string(),
    }
}

pub fn pair_body(conf: &RommConf) -> String {
    serde_json::json!({
        "client_device_identifier": conf.client_id,
        "name": device_name(
            crate::baseos::var("BASEOS_MODEL")
                .and_then(|model| crate::server::model_name(&model))
                .as_deref(),
        ),
        "client": "slot",
        "platform": "linux",
        "client_version": crate::build_info::Build::current().version,
        "requested_scopes": SCOPES,
    })
    .to_string()
}

struct HttpApi {
    agent: ureq::Agent,
    origin: String,
    device_id: Option<String>,
    token: Option<String>,
}

pub fn http_api(conf: &RommConf) -> Box<dyn Api> {
    let mut builder = ureq::Agent::config_builder()
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .timeout_per_read(Some(READ_TIMEOUT))
        .http_status_as_error(false)
        .redirect_auth_headers(ureq::config::RedirectAuthHeaders::SameHost)
        .user_agent(format!(
            "slot/{}",
            crate::build_info::Build::current().version
        ));
    if conf.self_signed {
        builder = builder.tls_config(
            ureq::tls::TlsConfig::builder()
                .disable_verification(true)
                .build(),
        );
    }
    Box::new(HttpApi {
        agent: builder.build().into(),
        origin: conf.origin(),
        device_id: conf.device_id.clone(),
        token: conf.token.clone(),
    })
}

type Reply = ureq::http::Response<ureq::Body>;

impl HttpApi {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.origin, path)
    }

    fn device(&self) -> Result<&str, ApiError> {
        self.device_id.as_deref().ok_or(ApiError::Unauthorized)
    }

    fn auth(&self, token: Option<&str>) -> Option<String> {
        bearer(token)
    }

    fn get(&self, path: &str) -> Result<Reply, ApiError> {
        let mut req = self.agent.get(self.url(path));
        if let Some(h) = self.auth(self.token.as_deref()) {
            req = req.header("Authorization", h);
        }
        req.call().map_err(|_| ApiError::Offline)
    }

    fn bodyless(&self, method: &str, path: &str) -> Result<Reply, ApiError> {
        let url = self.url(path);
        let header = self.auth(self.token.as_deref());
        let reply = match method {
            "DELETE" => {
                let mut req = self.agent.delete(url);
                if let Some(h) = header {
                    req = req.header("Authorization", h);
                }
                req.call()
            }
            _ => {
                let mut req = self.agent.post(url);
                if let Some(h) = header {
                    req = req.header("Authorization", h);
                }
                req.send_empty()
            }
        };
        reply.map_err(|_| ApiError::Offline)
    }

    fn with_json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: String,
    ) -> Result<Reply, ApiError> {
        let url = self.url(path);
        let header = self.auth(token);
        let reply = match method {
            "PUT" => {
                let mut req = self
                    .agent
                    .put(url)
                    .header("Content-Type", "application/json");
                if let Some(h) = header {
                    req = req.header("Authorization", h);
                }
                req.send(body)
            }
            _ => {
                let mut req = self
                    .agent
                    .post(url)
                    .header("Content-Type", "application/json");
                if let Some(h) = header {
                    req = req.header("Authorization", h);
                }
                req.send(body)
            }
        };
        reply.map_err(|_| ApiError::Offline)
    }
}

fn ok(reply: Reply) -> Result<Reply, ApiError> {
    match reply.status().as_u16() {
        200..=299 => Ok(reply),
        other => Err(status_error(other)),
    }
}

fn text(mut reply: Reply) -> Result<String, ApiError> {
    reply
        .body_mut()
        .read_to_string()
        .map_err(|_| ApiError::Offline)
}

impl Api for HttpApi {
    fn version(&self) -> Result<String, ApiError> {
        parse_version(&text(ok(self.get("/api/heartbeat")?)?)?)
    }

    fn pair_start(&self, conf: &RommConf) -> Result<Pending, ApiError> {
        let reply = self.with_json("POST", "/api/auth/device/init", None, pair_body(conf))?;
        parse_pending(&text(ok(reply)?)?)
    }

    fn pair_poll(&self, device_code: &str) -> Result<Poll, ApiError> {
        let body = serde_json::json!({"device_code": device_code}).to_string();
        let reply = self.with_json("POST", "/api/auth/device/token", None, body)?;
        let status = reply.status().as_u16();
        parse_poll(status, &text(reply)?)
    }

    fn enable_install(&self, device_id: &str, token: &str) -> Result<(), ApiError> {
        let body = r#"{"capabilities":{"remote_install":true}}"#.to_string();
        let path = format!("/api/devices/{device_id}");
        ok(self.with_json("PUT", &path, Some(token), body)?).map(|_| ())
    }

    fn claim(&self) -> Result<Vec<Install>, ApiError> {
        let path = format!("/api/devices/{}/installs/claim", self.device()?);
        parse_installs(&text(ok(self.bodyless("POST", &path)?)?)?)
    }

    fn device_exists(&self) -> Result<bool, ApiError> {
        let path = format!("/api/devices/{}", self.device()?);
        match ok(self.get(&path)?) {
            Ok(_) => Ok(true),
            Err(ApiError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }

    fn rom(&self, id: u64) -> Result<RomInfo, ApiError> {
        parse_rom(&text(ok(self.get(&format!("/api/roms/{id}"))?)?)?)
    }

    fn download(
        &self,
        rom: u64,
        name: &str,
        file_ids: &[u64],
        to: &Path,
        cancel: &Cancel,
    ) -> Result<(), ApiError> {
        if cancel.is_set() {
            return Err(ApiError::Cancelled);
        }
        let reply = ok(self.get(&content_path(rom, name, file_ids))?)?;
        let expected = reply
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let mut reader = reply.into_body().into_reader();
        let mut file = File::create(to).map_err(|e| ApiError::Write(e.to_string()))?;
        let mut buf = vec![0u8; CHUNK];
        let mut total = 0u64;
        loop {
            if cancel.is_set() {
                return Err(ApiError::Cancelled);
            }
            let n = match reader.read(&mut buf) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(ApiError::Truncated)
                }
                Err(_) => return Err(ApiError::Offline),
            };
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| ApiError::Write(e.to_string()))?;
            total += n as u64;
        }
        file.flush().map_err(|e| ApiError::Write(e.to_string()))?;
        match expected {
            Some(len) if total < len => Err(ApiError::Truncated),
            _ => Ok(()),
        }
    }

    fn report(&self, id: &str, outcome: &Outcome) -> Result<(), ApiError> {
        let path = format!("/api/devices/{}/installs/{id}", self.device()?);
        ok(self.with_json("PUT", &path, self.token.as_deref(), report_body(outcome))?).map(|_| ())
    }

    fn delete_device(&self) -> Result<(), ApiError> {
        let path = format!("/api/devices/{}", self.device()?);
        ok(self.bodyless("DELETE", &path)?).map(|_| ())
    }
}
