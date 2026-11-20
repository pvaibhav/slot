use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::json;

use super::paths::{card_name, is_hidden, resolve, Refused};
use super::{Body, Reply, Request};

const RESOURCES: [(&str, &str); 7] = [
    ("roms", "Games"),
    ("art", "Labels"),
    ("overlays", "Bezels"),
    ("saves", "Saves"),
    ("bios", "BIOS"),
    ("wallpapers", "Wallpapers"),
    ("guides", "Guides"),
];

const VOLUME: &str = "card";
const MAX_HEADERS: usize = 16 * 1024;
const MAX_JSON: usize = 64 * 1024;
const CHUNK: usize = 64 * 1024;

pub fn folder(resource: &str) -> Option<&'static str> {
    RESOURCES
        .iter()
        .find(|(r, _)| *r == resource)
        .map(|(_, f)| *f)
}

struct Target {
    base: Vec<String>,
    sub: Vec<String>,
    display: String,
    same_top: bool,
}

pub fn resource(root: &Path, req: Request) -> Reply {
    let name = req.path[1].clone();
    let folder = folder(&name).unwrap_or_default().to_string();
    let sub: Vec<String> = req.path[2..].to_vec();
    let display = std::iter::once(folder.clone())
        .chain(sub.iter().cloned())
        .collect::<Vec<_>>()
        .join("/");
    serve(
        root,
        req,
        Target {
            base: vec![folder],
            sub,
            display,
            same_top: true,
        },
    )
}

pub fn fs(root: &Path, req: Request) -> Reply {
    if req.path.len() == 2 {
        if req.method != "GET" && req.method != "HEAD" {
            return Reply::error(405, "method not allowed");
        }
        return volumes(root);
    }
    if req.path[2] != VOLUME {
        return Reply::error(404, "unknown volume");
    }
    let sub: Vec<String> = req.path[3..].to_vec();
    let display = std::iter::once(VOLUME.to_string())
        .chain(sub.iter().cloned())
        .collect::<Vec<_>>()
        .join("/");
    serve(
        root,
        req,
        Target {
            base: Vec::new(),
            sub,
            display,
            same_top: false,
        },
    )
}

fn refused(r: Refused) -> Reply {
    Reply::error(r.status, &r.message)
}

fn ok() -> Reply {
    Reply::json(json!({ "ok": true }))
}

fn volumes(root: &Path) -> Reply {
    let (total, free) = space(root);
    Reply::json(json!({
        "volumes": [{ "id": VOLUME, "label": "SD Card", "totalBytes": total, "freeBytes": free }]
    }))
}

fn space(root: &Path) -> (u64, u64) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(root.as_os_str().as_bytes()) else {
        return (0, 0);
    };
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut st) } != 0 {
        return (0, 0);
    }
    let unit = st.f_frsize as u64;
    (st.f_blocks as u64 * unit, st.f_bavail as u64 * unit)
}

fn target_path(root: &Path, segs: &[String]) -> Result<PathBuf, Refused> {
    if segs.is_empty() {
        root.canonicalize()
            .map_err(|_| Refused::new(500, "card unavailable"))
    } else {
        resolve(root, segs)
    }
}

fn serve(root: &Path, mut req: Request, t: Target) -> Reply {
    let segs: Vec<String> = t.base.iter().chain(t.sub.iter()).cloned().collect();
    let path = match target_path(root, &segs) {
        Ok(p) => p,
        Err(r) => return refused(r),
    };
    let at_root = t.sub.is_empty() && t.base.is_empty();
    match req.method.as_str() {
        "GET" | "HEAD" => {
            if path.is_file() {
                Reply::file(path)
            } else if !path.exists() && !t.sub.is_empty() {
                Reply::error(404, "not found")
            } else {
                let recursive = req.query.get("recursive").map(String::as_str) == Some("true");
                list(&path, &t.display, recursive, at_root)
            }
        }
        "POST" => post(&mut req, root, &segs),
        "PUT" => {
            if segs.is_empty() || t.sub.is_empty() && !t.base.is_empty() {
                return Reply::error(400, "path required");
            }
            if path.is_file() {
                return Reply::error(409, "a file with that name exists");
            }
            mkdir(&path)
        }
        "DELETE" => {
            if t.sub.is_empty() {
                return Reply::error(400, "path required");
            }
            if segs.len() == 1 && path.is_dir() {
                return Reply::error(403, "top-level folders can't be deleted");
            }
            delete(&path)
        }
        "PATCH" => {
            if t.sub.is_empty() {
                return Reply::error(400, "path required");
            }
            if segs.len() == 1 && path.is_dir() {
                let _ = io::copy(&mut req.body, &mut io::sink());
                return Reply::error(403, "top-level folders can't be renamed or moved");
            }
            let body = std::mem::replace(&mut req.body, Body::from_bytes(Vec::new()));
            patch(root, &t, &path, body)
        }
        _ => Reply::error(405, "method not allowed"),
    }
}

fn dotfile(name: &str) -> bool {
    slot_store::is_hidden(Path::new(name))
}

fn sort_key(name: &str) -> String {
    name.to_lowercase()
}

fn list(dir: &Path, display: &str, recursive: bool, hide_reserved: bool) -> Reply {
    let mut entries: Vec<(bool, String, u64)> = Vec::new();
    if dir.is_dir() {
        if recursive {
            walk(dir, dir, hide_reserved, &mut entries);
            entries.sort_by_key(|e| sort_key(&e.1));
        } else if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if hide_reserved && is_hidden(&name) || dotfile(&name) {
                    continue;
                }
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_symlink() {
                    continue;
                }
                let size = if ft.is_file() {
                    e.metadata().map(|m| m.len()).unwrap_or(0)
                } else {
                    0
                };
                entries.push((ft.is_dir(), name, size));
            }
            entries.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| sort_key(&a.1).cmp(&sort_key(&b.1)))
            });
        }
    }
    let list: Vec<_> = entries
        .into_iter()
        .map(|(is_dir, name, size)| json!({ "name": name, "type": if is_dir { "dir" } else { "file" }, "size": size }))
        .collect();
    Reply::json(json!({ "path": display, "entries": list }))
}

fn walk(top: &Path, dir: &Path, hide_reserved: bool, out: &mut Vec<(bool, String, u64)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if hide_reserved && dir == top && is_hidden(&name) || dotfile(&name) {
            continue;
        }
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            walk(top, &e.path(), hide_reserved, out);
        } else if ft.is_file() {
            let rel = e
                .path()
                .strip_prefix(top)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or(name);
            out.push((false, rel, e.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
}

fn fresh_names(path: &Path) -> Result<(), Refused> {
    let mut p = path;
    while !p.exists() {
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            card_name(name)?;
        }
        match p.parent() {
            Some(parent) => p = parent,
            None => break,
        }
    }
    Ok(())
}

fn mkdir(path: &Path) -> Reply {
    if path.exists() {
        return Reply::json(json!({ "ok": true, "existed": true }));
    }
    if let Err(r) = fresh_names(path) {
        return refused(r);
    }
    match fs::create_dir_all(path) {
        Ok(()) => Reply::Json(201, json!({ "ok": true }).to_string()),
        Err(_) => Reply::error(500, "mkdir failed"),
    }
}

fn delete(path: &Path) -> Reply {
    if !path.exists() {
        return Reply::error(404, "not found");
    }
    let result = if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    };
    match result {
        Ok(()) => ok(),
        Err(_) => Reply::error(500, "delete failed"),
    }
}

fn patch(root: &Path, t: &Target, src: &Path, body: Body) -> Reply {
    let to = body
        .read_all(MAX_JSON)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("to").and_then(|s| s.as_str().map(str::to_string)))
        .unwrap_or_default();
    let to_segs: Vec<String> = to
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if to_segs.is_empty() {
        return Reply::error(400, "missing 'to' field");
    }
    if t.same_top {
        let flat = t.sub.len() == 1 && to_segs.len() == 1;
        let same = flat
            || t.sub.len() >= 2 && to_segs.len() >= 2 && t.sub[0].eq_ignore_ascii_case(&to_segs[0]);
        if !same {
            return Reply::error(403, "moves must stay within the same subdirectory");
        }
    }
    let full: Vec<String> = t.base.iter().chain(to_segs.iter()).cloned().collect();
    let mut dst = match resolve(root, &full) {
        Ok(p) => p,
        Err(r) => return refused(r),
    };
    if let Some(leaf) = to_segs.last() {
        dst.set_file_name(leaf);
    }
    if !src.exists() {
        return Reply::error(404, "source not found");
    }
    if dst.exists() && !same_inode(src, &dst) {
        return Reply::error(409, "destination already exists");
    }
    if let Some(leaf) = to_segs.last() {
        if let Err(r) = card_name(leaf) {
            return refused(r);
        }
    }
    if let Err(r) = fresh_names(&dst) {
        return refused(r);
    }
    if let Some(parent) = dst.parent() {
        if fs::create_dir_all(parent).is_err() {
            return Reply::error(500, "move failed");
        }
    }
    match move_path(src, &dst) {
        Ok(()) => ok(),
        Err(_) => Reply::error(500, "move failed"),
    }
}

pub fn move_path(src: &Path, dst: &Path) -> io::Result<()> {
    if src.file_name() != dst.file_name() && same_inode(src, dst) {
        rename_via_temp(src, dst, &|a, b| fs::rename(a, b))
    } else {
        fs::rename(src, dst)
    }
}

pub fn rename_via_temp(
    src: &Path,
    dst: &Path,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut n = 0u32;
    let temp = loop {
        let candidate = parent.join(format!(".slot-rename-{}-{stamp}-{n}", std::process::id()));
        if fs::symlink_metadata(&candidate).is_err() {
            break candidate;
        }
        n += 1;
    };
    rename(src, &temp)?;
    if let Err(e) = rename(&temp, dst) {
        let _ = fs::rename(&temp, src);
        return Err(e);
    }
    Ok(())
}

pub(super) fn same_inode(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

fn post(req: &mut Request, root: &Path, dir: &[String]) -> Reply {
    let ctype = req.headers.get("content-type").cloned().unwrap_or_default();
    if req
        .headers
        .get("transfer-encoding")
        .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
    {
        return Reply::error(400, "chunked upload not supported");
    }
    let Some(boundary) = boundary(&ctype) else {
        return Reply::error(400, "multipart upload required");
    };
    let result = upload(root, dir, &mut req.body, &boundary);
    match result {
        Ok(names) if names.is_empty() => Reply::error(400, "no file in upload"),
        Ok(names) => Reply::json(json!({ "ok": true, "files": names })),
        Err(Failure::Refused(r)) => {
            let _ = io::copy(&mut req.body, &mut io::sink());
            refused(r)
        }
        Err(Failure::Io) => Reply::error(500, "upload failed"),
    }
}

fn boundary(ctype: &str) -> Option<String> {
    if !ctype.to_ascii_lowercase().starts_with("multipart/") {
        return None;
    }
    let idx = ctype.to_ascii_lowercase().find("boundary=")?;
    let rest = &ctype[idx + "boundary=".len()..];
    let value = match rest.strip_prefix('"') {
        Some(q) => q.split('"').next()?,
        None => rest.split(';').next()?.trim(),
    };
    (!value.is_empty()).then(|| value.to_string())
}

enum Failure {
    Refused(Refused),
    Io,
}

impl From<io::Error> for Failure {
    fn from(_: io::Error) -> Failure {
        Failure::Io
    }
}

struct Parts<'a> {
    body: &'a mut Body,
    buf: Vec<u8>,
}

impl Parts<'_> {
    fn fill(&mut self) -> io::Result<()> {
        let mut chunk = vec![0u8; CHUNK];
        let n = self.body.read(&mut chunk)?;
        if n == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        self.buf.extend_from_slice(&chunk[..n]);
        Ok(())
    }

    fn find(&self, needle: &[u8]) -> Option<usize> {
        self.buf.windows(needle.len()).position(|w| w == needle)
    }

    fn need(&mut self, n: usize) -> io::Result<()> {
        while self.buf.len() < n {
            self.fill()?;
        }
        Ok(())
    }

    fn until(&mut self, delim: &[u8], mut out: Option<&mut File>) -> io::Result<()> {
        loop {
            if let Some(i) = self.find(delim) {
                if let Some(f) = out.as_deref_mut() {
                    f.write_all(&self.buf[..i])?;
                }
                self.buf.drain(..i);
                return Ok(());
            }
            let safe = self.buf.len().saturating_sub(delim.len() - 1);
            if let Some(f) = out.as_deref_mut() {
                f.write_all(&self.buf[..safe])?;
            }
            self.buf.drain(..safe);
            self.fill()?;
        }
    }

    fn headers(&mut self) -> io::Result<String> {
        loop {
            if let Some(i) = self.find(b"\r\n\r\n") {
                let text = String::from_utf8_lossy(&self.buf[..i]).into_owned();
                self.buf.drain(..i + 4);
                return Ok(text);
            }
            if self.buf.len() > MAX_HEADERS {
                return Err(io::ErrorKind::InvalidData.into());
            }
            self.fill()?;
        }
    }
}

fn filename_of(headers: &str) -> Option<String> {
    let line = headers
        .lines()
        .find(|l| l.to_ascii_lowercase().starts_with("content-disposition:"))?;
    let lower = line.to_ascii_lowercase();
    let idx = lower.find("filename=\"")? + "filename=\"".len();
    let end = line[idx..].find('"')?;
    let raw = &line[idx..idx + end];
    Some(raw.rsplit(['/', '\\']).next().unwrap_or(raw).to_string())
}

fn upload(
    root: &Path,
    dir: &[String],
    body: &mut Body,
    boundary: &str,
) -> Result<Vec<String>, Failure> {
    let delim = format!("\r\n--{boundary}").into_bytes();
    let mut parts = Parts {
        body,
        buf: b"\r\n".to_vec(),
    };
    let mut written = Vec::new();
    let mut made_dir = false;
    loop {
        parts.until(&delim, None)?;
        parts.buf.drain(..delim.len());
        parts.need(2)?;
        if parts.buf.starts_with(b"--") {
            return Ok(written);
        }
        while parts.find(b"\r\n").is_none() {
            if parts.buf.len() > MAX_HEADERS {
                return Err(io::Error::from(io::ErrorKind::InvalidData).into());
            }
            parts.fill()?;
        }
        let eol = parts.find(b"\r\n").unwrap_or(0);
        parts.buf.drain(..eol + 2);
        let headers = parts.headers()?;
        let Some(name) = filename_of(&headers).filter(|n| !n.is_empty()) else {
            parts.until(&delim, None)?;
            continue;
        };
        card_name(&name).map_err(Failure::Refused)?;
        let mut segs = dir.to_vec();
        segs.push(name.clone());
        let dest = resolve(root, &segs).map_err(Failure::Refused)?;
        let parent = dest.parent().map(Path::to_path_buf).unwrap_or_default();
        if !made_dir {
            fresh_names(&parent).map_err(Failure::Refused)?;
            fs::create_dir_all(&parent)?;
            made_dir = true;
        }
        let mut part_name = dest
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_default();
        part_name.push(".part");
        let part = dest.with_file_name(part_name);
        let result = (|| -> io::Result<()> {
            let _ = fs::remove_file(&part);
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&part)?;
            parts.until(&delim, Some(&mut f))?;
            f.sync_all()?;
            drop(f);
            fs::rename(&part, &dest)
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&part);
            return Err(e.into());
        }
        written.push(name);
    }
}
