mod carts;
mod files;
mod games;
mod http;
mod net;
mod paths;
mod refs;
mod session;
mod states;

pub use carts::render_once;
pub use games::sweep_names;
pub use files::{move_path, rename_via_temp};
pub use http::{generate_pin, parse_target, Body, Handler, Reply, Request, Server};
pub use net::{
    announcement, answer, claims, conflicts, heard, model_name, names, probe, respond, mdns_off, wifi_ip,
    Heard, Responder,
};
pub use paths::{card_name, resolve, Refused};
pub use session::{ServerNet, ServerPaper, ServerSession, NO_WIFI, SERVER_FAILED};

use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

use qrcode::{EcLevel, QrCode, Version};

pub const PORTS: [u16; 2] = [80, 2003];

pub fn route(root: &Path, req: Request) -> Reply {
    if req.path.first().map(String::as_str) != Some("api") {
        return static_page(root, &req);
    }
    match req.path.get(1).map(String::as_str) {
        Some("fs") => files::fs(root, req),
        Some("tags") => games::tags(req),
        Some("games") => games::games(root, req),
        Some(name) if files::folder(name).is_some() => files::resource(root, req),
        _ => Reply::error(404, "not found"),
    }
}

fn web_file(root: &Path, segments: &[String]) -> Option<PathBuf> {
    let base = root.join("System").join("web").canonicalize().ok()?;
    let mut out = base.clone();
    for s in segments {
        if s.is_empty() || s.starts_with('.') || s.contains(['/', '\\', '\0']) {
            return None;
        }
        out.push(s);
    }
    let full = out.canonicalize().ok()?;
    (full.starts_with(&base) && full.is_file()).then_some(full)
}

const PAGE_ROUTES: [&str; 5] = ["dashboard", "platform", "browse", "tools", "view"];

fn static_page(root: &Path, req: &Request) -> Reply {
    if req.method != "GET" && req.method != "HEAD" {
        return Reply::error(405, "method not allowed");
    }
    if req.path.iter().any(|s| s == ".." || s.contains(['/', '\\', '\0'])) {
        return Reply::error(403, "forbidden");
    }
    if let Some(file) = web_file(root, &req.path) {
        return Reply::file(file);
    }
    let routed = req
        .path
        .first()
        .is_none_or(|first| PAGE_ROUTES.contains(&first.as_str()));
    if routed {
        if let Some(index) = web_file(root, &["index.html".to_string()]) {
            return Reply::file(index);
        }
    }
    Reply::error(404, "not found")
}

pub fn shown_address(host: &str, port: u16) -> String {
    if port == 80 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    }
}

pub fn qr_url(ip: Ipv4Addr, port: u16, pin: &str) -> String {
    format!("http://{}/?pin={pin}", shown_address(&ip.to_string(), port))
}

pub fn qr_modules(url: &str) -> Option<Vec<Vec<bool>>> {
    let code = QrCode::with_version(url.as_bytes(), Version::Normal(3), EcLevel::M).ok()?;
    let width = code.width();
    Some(
        code.to_colors()
            .chunks(width)
            .map(|row| row.iter().map(|c| *c == qrcode::Color::Dark).collect())
            .collect(),
    )
}
