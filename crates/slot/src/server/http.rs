use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::route;

pub type Handler = Arc<dyn Fn(&Path, Request) -> Reply + Send + Sync>;

const MAX_CONNECTIONS: usize = 8;
const POLL: Duration = Duration::from_millis(100);
const IDLE: Duration = Duration::from_secs(30);
const MAX_HEAD: usize = 16 * 1024;
const REFUSE_WAIT: Duration = Duration::from_millis(500);

pub struct Body {
    reader: Box<dyn Read + Send>,
    remaining: u64,
}

impl Body {
    pub fn new(reader: Box<dyn Read + Send>, length: u64) -> Body {
        Body { reader, remaining: length }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Body {
        let n = bytes.len() as u64;
        Body::new(Box::new(io::Cursor::new(bytes)), n)
    }

    pub fn len(&self) -> u64 {
        self.remaining
    }

    pub fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    pub fn read_all(mut self, limit: usize) -> io::Result<Vec<u8>> {
        if self.remaining > limit as u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "body too large"));
        }
        let mut out = Vec::with_capacity(self.remaining as usize);
        io::copy(&mut self, &mut out)?;
        if self.remaining > 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(out)
    }

    pub fn save_to(mut self, dest: &Path) -> io::Result<()> {
        let mut name = dest.file_name().map(|n| n.to_os_string()).unwrap_or_default();
        name.push(".part");
        let part = dest.with_file_name(name);
        let result = (|| {
            let mut f = File::create(&part)?;
            io::copy(&mut self, &mut f)?;
            if self.remaining > 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            f.sync_all()?;
            drop(f);
            std::fs::rename(&part, dest)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        result
    }
}

impl Read for Body {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Ok(0);
        }
        let want = buf.len().min(self.remaining as usize);
        let n = self.reader.read(&mut buf[..want])?;
        self.remaining -= n as u64;
        Ok(n)
    }
}

pub struct Request {
    pub method: String,
    pub path: Vec<String>,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: Body,
    pub stop: Arc<AtomicBool>,
}

pub enum Reply {
    Json(u16, String),
    File(PathBuf),
    Empty(u16),
    Headed(Vec<(String, String)>, Box<Reply>),
}

impl Reply {
    pub fn json(value: serde_json::Value) -> Reply {
        Reply::Json(200, value.to_string())
    }

    pub fn error(status: u16, msg: &str) -> Reply {
        Reply::Json(status, serde_json::json!({ "error": msg }).to_string())
    }

    pub fn file(path: impl Into<PathBuf>) -> Reply {
        Reply::File(path.into())
    }

    pub fn with_header(self, name: &str, value: &str) -> Reply {
        let pair = (name.to_string(), value.to_string());
        match self {
            Reply::Headed(mut headers, inner) => {
                headers.push(pair);
                Reply::Headed(headers, inner)
            }
            other => Reply::Headed(vec![pair], Box::new(other)),
        }
    }
}

fn percent_decode(s: &str, plus: bool) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && hex(b[i + 1]).is_some() && hex(b[i + 2]).is_some() => {
                out.push(hex(b[i + 1]).unwrap() << 4 | hex(b[i + 2]).unwrap());
                i += 3;
            }
            b'+' if plus => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

pub fn parse_target(target: &str) -> (Vec<String>, HashMap<String, String>) {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let segments = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| percent_decode(s, false))
        .collect();
    let mut map = HashMap::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        map.entry(percent_decode(k, true)).or_insert_with(|| percent_decode(v, true));
    }
    (segments, map)
}

pub fn generate_pin() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    h.write_u32(std::process::id());
    format!("{:04}", h.finish() % 10_000)
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in input.trim().bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

fn authorized(headers: &HashMap<String, String>, pin: &str) -> bool {
    let Some(value) = headers.get("authorization") else { return false };
    let Some((scheme, token)) = value.split_once(' ') else { return false };
    if !scheme.eq_ignore_ascii_case("basic") {
        return false;
    }
    let Some(decoded) = base64_decode(token) else { return false };
    let Ok(text) = String::from_utf8(decoded) else { return false };
    text.split_once(':').is_some_and(|(user, pass)| user == "slot" && pass == pin)
}

struct StopReader {
    stream: TcpStream,
    stop: Arc<AtomicBool>,
    last: Instant,
}

impl Read for StopReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "server stopping"));
            }
            match self.stream.read(buf) {
                Ok(n) => {
                    self.last = Instant::now();
                    return Ok(n);
                }
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if self.last.elapsed() > IDLE {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }
}

fn read_line(r: &mut impl BufRead, budget: &mut usize) -> io::Result<String> {
    let mut line = String::new();
    let n = r.take(*budget as u64 + 1).read_line(&mut line)?;
    if n == 0 || n > *budget {
        return Err(io::ErrorKind::InvalidData.into());
    }
    *budget -= n;
    Ok(line.trim_end_matches(['\r', '\n']).to_string())
}

fn parse_request(stream: &TcpStream, stop: &Arc<AtomicBool>) -> io::Result<Request> {
    let reader = StopReader { stream: stream.try_clone()?, stop: stop.clone(), last: Instant::now() };
    let mut reader = BufReader::new(reader);
    let mut budget = MAX_HEAD;
    let first = read_line(&mut reader, &mut budget)?;
    let mut parts = first.split_whitespace();
    let method = parts.next().ok_or(io::ErrorKind::InvalidData)?.to_ascii_uppercase();
    let target = parts.next().ok_or(io::ErrorKind::InvalidData)?;
    let (path, query) = parse_target(target);
    let mut headers = HashMap::new();
    loop {
        let line = read_line(&mut reader, &mut budget)?;
        if line.is_empty() {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let length = match headers.get("content-length") {
        Some(v) => v.parse::<u64>().map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?,
        None => 0,
    };
    Ok(Request { method, path, query, headers, body: Body::new(Box::new(reader), length), stop: stop.clone() })
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("txt") | Some("ini") | Some("log") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn head(code: u16, ctype: &str, length: u64, extra: &str) -> String {
    format!(
        "HTTP/1.1 {code} {}\r\nContent-Type: {ctype}\r\nContent-Length: {length}\r\n\
         {extra}Connection: close\r\n\r\n",
        status_text(code)
    )
}

struct StopWriter {
    stream: TcpStream,
    stop: Arc<AtomicBool>,
    last: Instant,
}

impl Write for StopWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        loop {
            if self.stop.load(Ordering::SeqCst) {
                return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "server stopping"));
            }
            match self.stream.write(buf) {
                Ok(n) => {
                    self.last = Instant::now();
                    return Ok(n);
                }
                Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                    if self.last.elapsed() > IDLE {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

fn write_reply(raw: &TcpStream, stop: &Arc<AtomicBool>, reply: Reply, head_only: bool) -> io::Result<()> {
    let _ = raw.set_write_timeout(Some(POLL));
    let mut stream = StopWriter { stream: raw.try_clone()?, stop: stop.clone(), last: Instant::now() };
    let stream = &mut stream;
    let (extra, reply) = match reply {
        Reply::Headed(headers, inner) => {
            (headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect::<String>(), *inner)
        }
        other => (String::new(), other),
    };
    let extra = extra.as_str();
    match reply {
        Reply::Json(code, body) => {
            stream.write_all(head(code, "application/json", body.len() as u64, extra).as_bytes())?;
            if !head_only {
                stream.write_all(body.as_bytes())?;
            }
        }
        Reply::Headed(..) => {}
        Reply::Empty(code) => {
            stream.write_all(head(code, "text/plain", 0, extra).as_bytes())?;
        }
        Reply::File(path) => match File::open(&path).and_then(|f| f.metadata().map(|m| (f, m))) {
            Ok((mut f, m)) if m.is_file() => {
                stream.write_all(head(200, mime(&path), m.len(), extra).as_bytes())?;
                if !head_only {
                    io::copy(&mut f, stream)?;
                }
            }
            _ => {
                let body = serde_json::json!({ "error": "not found" }).to_string();
                stream.write_all(head(404, "application/json", body.len() as u64, "").as_bytes())?;
                stream.write_all(body.as_bytes())?;
            }
        },
    }
    stream.flush()
}

fn handle(stream: TcpStream, root: &Path, pin: &str, stop: &Arc<AtomicBool>, handler: &Handler) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(POLL));
    let _ = stream.set_nodelay(true);
    let req = match parse_request(&stream, stop) {
        Ok(r) => r,
        Err(_) => {
            let _ = write_reply(&stream, stop, Reply::error(400, "bad request"), false);
            return;
        }
    };
    let head_only = req.method == "HEAD";
    let reply = if req.path.first().map(String::as_str) == Some("api") {
        if req.method == "GET" && req.path.get(1).map(String::as_str) == Some("auth") && req.path.len() == 2 {
            Reply::json(serde_json::json!({ "required": true }))
        } else if !authorized(&req.headers, pin) {
            Reply::error(401, "unauthorized")
        } else {
            handler(root, req)
        }
    } else {
        handler(root, req)
    };
    let _ = write_reply(&stream, stop, reply, head_only);
    let _ = stream.shutdown(Shutdown::Both);
}

pub struct Server {
    port: u16,
    stop: Arc<AtomicBool>,
    accept: Option<JoinHandle<()>>,
}

impl Server {
    pub fn start(root: PathBuf, port: u16, pin: String) -> io::Result<Server> {
        Server::start_with(root, port, pin, Arc::new(route))
    }

    pub fn start_with(root: PathBuf, port: u16, pin: String, handler: Handler) -> io::Result<Server> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let accept = thread::spawn(move || accept_loop(listener, root, pin, flag, handler));
        Ok(Server { port, stop, accept: Some(accept) })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(h) = self.accept.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Slot(Arc<AtomicUsize>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn refuse(mut stream: &TcpStream, stop: &Arc<AtomicBool>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(POLL));
    let begun = Instant::now();
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while head.len() < MAX_HEAD && !stop.load(Ordering::SeqCst) && begun.elapsed() < REFUSE_WAIT {
        match stream.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => {
                head.extend_from_slice(&buf[..n]);
                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {}
            Err(_) => return,
        }
    }
    let _ = write_reply(stream, stop, Reply::error(503, "busy"), false);
    let _ = stream.shutdown(Shutdown::Both);
}

fn accept_loop(listener: TcpListener, root: PathBuf, pin: String, stop: Arc<AtomicBool>, handler: Handler) {
    let root = Arc::new(root);
    let pin = Arc::new(pin);
    let active = Arc::new(AtomicUsize::new(0));
    let refusing = Arc::new(AtomicUsize::new(0));
    let workers: Arc<Mutex<Vec<JoinHandle<()>>>> = Arc::new(Mutex::new(Vec::new()));
    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if active.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    if refusing.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                        continue;
                    }
                    refusing.fetch_add(1, Ordering::SeqCst);
                    let guard = Slot(refusing.clone());
                    let stop = stop.clone();
                    let h = thread::spawn(move || {
                        let _guard = guard;
                        refuse(&stream, &stop);
                    });
                    let mut w = workers.lock().unwrap();
                    w.retain(|h| !h.is_finished());
                    w.push(h);
                    continue;
                }
                active.fetch_add(1, Ordering::SeqCst);
                let guard = Slot(active.clone());
                let (root, pin, stop, handler) = (root.clone(), pin.clone(), stop.clone(), handler.clone());
                let h = thread::spawn(move || {
                    let _guard = guard;
                    handle(stream, &root, &pin, &stop, &handler);
                });
                let mut w = workers.lock().unwrap();
                w.retain(|h| !h.is_finished());
                w.push(h);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(15)),
            Err(_) => thread::sleep(Duration::from_millis(15)),
        }
    }
    drop(listener);
    let handles: Vec<_> = workers.lock().unwrap().drain(..).collect();
    for h in handles {
        let _ = h.join();
    }
}
