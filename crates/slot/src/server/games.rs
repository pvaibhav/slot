use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde_json::{json, Value};
use slot_store::{atomic_write, listing, scan_platform, Cart, Core, Platform, StateRing};

use super::files::same_inode;
use super::paths::{card_name, resolve, Refused};
use super::refs::{self, GuideMove};
use super::{Body, Reply, Request};

pub(super) const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const SAVE_EXTENSIONS: [&str; 2] = ["sav", "srm"];
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const MAX_ART: usize = 8 * 1024 * 1024;
const MAX_GUIDE: usize = 4 * 1024 * 1024;

pub(super) fn refused(r: Refused) -> Reply {
    Reply::error(r.status, &r.message)
}

pub(super) fn ok() -> Reply {
    Reply::json(json!({ "ok": true }))
}

fn platform_for(tag: &str) -> Option<Platform> {
    Platform::ALL.into_iter().find(|p| p.dir_name().eq_ignore_ascii_case(tag))
}

fn parse_id(text: &str) -> Option<usize> {
    let plain = !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if plain && (text == "0" || !text.starts_with('0')) {
        text.parse().ok()
    } else {
        None
    }
}

pub fn tags(req: Request) -> Reply {
    if req.method != "GET" && req.method != "HEAD" {
        return Reply::error(405, "method not allowed");
    }
    let tags: Vec<&str> = Platform::ALL.iter().map(|p| p.dir_name()).collect();
    Reply::json(json!({ "tags": tags, "groups": {} }))
}

pub(super) fn at(root: &Path, segments: &[&str]) -> Result<PathBuf, Refused> {
    let owned: Vec<String> = segments.iter().map(|s| s.to_string()).collect();
    resolve(root, &owned)
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

fn stem_of(name: &str) -> String {
    Path::new(name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

pub(super) fn modified_ms(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

struct Index {
    saves: HashMap<String, Vec<String>>,
    guides: HashMap<String, String>,
    states: Vec<(Core, HashSet<String>)>,
}

impl Index {
    fn new(root: &Path, platform: Platform) -> Index {
        let tag = platform.dir_name();
        let mut saves: HashMap<String, Vec<String>> = HashMap::new();
        if let Some(entries) = at(root, &["Saves", tag]).ok().and_then(|d| fs::read_dir(d).ok()) {
            for entry in entries.filter_map(|e| e.ok()) {
                let Ok(name) = entry.file_name().into_string() else { continue };
                if name.starts_with('.') || name.ends_with(".part") || !entry.path().is_file() {
                    continue;
                }
                saves.entry(stem_of(&name).to_lowercase()).or_default().push(name);
            }
        }
        for names in saves.values_mut() {
            names.sort_by_key(|n| n.to_lowercase());
        }
        let guides = at(root, &["Guides", tag])
            .map(|d| listing(&d))
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(lower, name)| lower.strip_suffix(".txt").map(|s| (s.to_string(), name)))
            .collect();
        let states = Core::ALL
            .iter()
            .filter(|c| c.runs(platform))
            .map(|c| {
                let dirs = at(root, &["States", tag, c.as_str()])
                    .ok()
                    .and_then(|d| fs::read_dir(d).ok())
                    .map(|rd| {
                        rd.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect::<HashSet<_>>()
                    })
                    .unwrap_or_default();
                (*c, dirs)
            })
            .collect();
        Index { saves, guides, states }
    }

    fn saves(&self, stem: &str) -> &[String] {
        self.saves.get(&stem.to_lowercase()).map(Vec::as_slice).unwrap_or(&[])
    }

    fn guide(&self, stem: &str) -> Option<&str> {
        self.guides.get(&stem.to_lowercase()).map(String::as_str)
    }

    fn state_count(&self, root: &Path, cart: &Cart) -> usize {
        self.states
            .iter()
            .filter(|(_, dirs)| dirs.contains(&cart.stem))
            .map(|(c, _)| {
                StateRing::new(root, cart.platform, *c, &cart.stem).list().map(|l| l.len()).unwrap_or(0)
            })
            .sum()
    }
}

fn resume_cores(root: &Path, cart: &Cart) -> Vec<&'static str> {
    Core::ALL
        .iter()
        .filter(|c| c.runs(cart.platform))
        .filter(|c| StateRing::new(root, cart.platform, **c, &cart.stem).has_resume())
        .map(|c| c.text())
        .collect()
}

fn carts(root: &Path, platform: Platform) -> Vec<Cart> {
    scan_platform(root, platform).unwrap_or_default()
}

fn row(root: &Path, index: &Index, cart: &Cart, id: usize) -> Value {
    let tag = cart.platform.dir_name();
    let rom = file_name(&cart.rom);
    json!({
        "id": id,
        "rom": rom,
        "displayName": cart.stem,
        "sortKey": cart.stem.to_lowercase(),
        "path": format!("{tag}/{rom}"),
        "folder": "",
        "hasArt": cart.label.is_some(),
        "cartUrl": format!("/api/games/{tag}/{id}/cart?v={}", super::carts::key(cart)),
        "savesCount": index.saves(&cart.stem).len(),
        "statesCount": index.state_count(root, cart),
        "guidesCount": usize::from(index.guide(&cart.stem).is_some()),
        "cheatsCount": 0,
        "multiDisc": false,
    })
}

fn detail(root: &Path, index: &Index, cart: &Cart, id: usize) -> Value {
    let mut value = row(root, index, cart, id);
    let meta = fs::metadata(&cart.rom);
    value["platform"] = json!(cart.platform.dir_name());
    value["platformDisplayName"] = json!(cart.platform.name());
    value["size"] = json!(meta.as_ref().map(|m| m.len()).unwrap_or(0));
    value["modified"] = json!(modified_ms(&cart.rom));
    value
}

pub fn games(root: &Path, mut req: Request) -> Reply {
    let Some(platform) = req.path.get(2).and_then(|t| platform_for(t)) else {
        return Reply::error(404, "platform not found");
    };
    if req.path.len() == 5 && req.path[4] == "cart" {
        if let Some(reply) = super::carts::cached(root, platform, &req) {
            return reply;
        }
    }
    let list = carts(root, platform);
    let index = || Index::new(root, platform);
    if req.path.len() == 3 {
        if req.method != "GET" && req.method != "HEAD" {
            return Reply::error(405, "method not allowed");
        }
        let index = index();
        let rows: Vec<Value> = list.iter().enumerate().map(|(i, c)| row(root, &index, c, i)).collect();
        return Reply::json(json!({
            "platform": platform.dir_name(),
            "displayName": platform.name(),
            "folders": [],
            "games": rows,
        }));
    }
    let Some(id) = parse_id(&req.path[3]).filter(|i| *i < list.len()) else {
        return Reply::error(404, "game not found");
    };
    let cart = &list[id];
    let method = req.method.clone();
    let method = method.as_str();
    let read = method == "GET" || method == "HEAD";
    let sub = req.path.get(4).cloned();
    if req.path.len() > 5 && sub.as_deref() != Some("states") {
        return Reply::error(404, "not found");
    }
    match sub.as_deref() {
        None => match method {
            "GET" | "HEAD" => Reply::json(detail(root, &index(), cart, id)),
            "DELETE" => delete(root, &index(), cart, &req),
            _ => Reply::error(405, "method not allowed"),
        },
        Some("art") => art(root, cart, &mut req),
        Some("cart") => super::carts::cart(root, cart, &req),
        Some("fetch-label") => {
            if method != "POST" {
                return Reply::error(405, "method not allowed");
            }
            match crate::art::jobs::fetch_label(root, cart) {
                Ok(true) => ok(),
                Ok(false) => Reply::error(404, "no label found"),
                Err(_) => Reply::error(502, "art server unavailable"),
            }
        }
        Some("rename") => {
            if method != "POST" {
                return Reply::error(405, "method not allowed");
            }
            match req.query.get("name") {
                Some(name) => match rename(root, &index(), cart, name) {
                    Ok(()) => ok(),
                    Err(r) => refused(r),
                },
                None => Reply::error(400, "name param required"),
            }
        }
        Some("saves") => saves(root, &index(), cart, &mut req),
        Some("guides") => guides(root, &index(), cart, &mut req),
        Some("states") => super::states::states(root, cart, &req),
        Some("roms") => {
            if !read {
                return Reply::error(405, "method not allowed");
            }
            let name = file_name(&cart.rom);
            let size = fs::metadata(&cart.rom).map(|m| m.len()).unwrap_or(0);
            Reply::json(json!({
                "files": [{ "name": name, "path": name, "size": size, "modified": modified_ms(&cart.rom) }]
            }))
        }
        Some("rom") => {
            if !read {
                return Reply::error(405, "method not allowed");
            }
            let name = file_name(&cart.rom);
            if req.query.get("file").is_some_and(|f| *f != name) {
                return Reply::error(404, "not found");
            }
            match at(root, &["Games", platform.dir_name(), &name]) {
                Ok(p) => Reply::file(p),
                Err(r) => refused(r),
            }
        }
        _ => Reply::error(404, "not found"),
    }
}

fn take_body(req: &mut Request) -> Body {
    std::mem::replace(&mut req.body, Body::from_bytes(Vec::new()))
}

fn drain(req: &mut Request) {
    let _ = io::copy(&mut req.body, &mut io::sink());
}

fn art(root: &Path, cart: &Cart, req: &mut Request) -> Reply {
    let tag = cart.platform.dir_name();
    match req.method.as_str() {
        "GET" | "HEAD" => {
            let Some(label) = &cart.label else { return Reply::error(404, "not found") };
            match at(root, &["Labels", tag, &file_name(label)]) {
                Ok(path) => Reply::file(path).with_header("Cache-Control", IMMUTABLE),
                Err(r) => refused(r),
            }
        }
        "POST" => {
            let name = cart.label.as_ref().map(|l| file_name(l)).unwrap_or_else(|| format!("{}.png", cart.stem));
            let dest = card_name(&name).and_then(|_| at(root, &["Labels", tag, &name]));
            let dest = match dest {
                Ok(d) => d,
                Err(r) => {
                    drain(req);
                    return refused(r);
                }
            };
            let bytes = match take_body(req).read_all(MAX_ART) {
                Ok(b) => b,
                Err(_) => return Reply::error(413, "image too large"),
            };
            if !bytes.starts_with(PNG_MAGIC) {
                return Reply::error(400, "label must be a png");
            }
            let made = dest.parent().map(fs::create_dir_all).unwrap_or(Ok(()));
            match made.and_then(|_| atomic_write(&dest, &bytes)) {
                Ok(()) => {
                    let mut labeled = cart.clone();
                    labeled.label = Some(dest);
                    labeled.read_header();
                    let _ = crate::label_cache::label_art(root, &labeled);
                    ok()
                }
                Err(_) => Reply::error(500, "upload failed"),
            }
        }
        "DELETE" => {
            let Some(label) = &cart.label else { return Reply::error(404, "not found") };
            match at(root, &["Labels", tag, &file_name(label)]) {
                Ok(p) => match fs::remove_file(p) {
                    Ok(()) => ok(),
                    Err(_) => Reply::error(500, "delete failed"),
                },
                Err(r) => refused(r),
            }
        }
        _ => Reply::error(405, "method not allowed"),
    }
}

fn state_dirs(root: &Path, cart: &Cart) -> Vec<PathBuf> {
    Core::ALL
        .iter()
        .filter_map(|c| at(root, &["States", cart.platform.dir_name(), c.as_str(), &cart.stem]).ok())
        .collect()
}

fn companions(root: &Path, index: &Index, cart: &Cart) -> Result<Vec<PathBuf>, Refused> {
    let tag = cart.platform.dir_name();
    let mut out = Vec::new();
    for name in index.saves(&cart.stem) {
        out.push(at(root, &["Saves", tag, name])?);
    }
    if let Some(label) = &cart.label {
        out.push(at(root, &["Labels", tag, &file_name(label)])?);
    }
    if let Some(guide) = index.guide(&cart.stem) {
        out.push(at(root, &["Guides", tag, guide])?);
    }
    Ok(out)
}

fn delete(root: &Path, index: &Index, cart: &Cart, req: &Request) -> Reply {
    let tag = cart.platform.dir_name();
    let purge = req.query.get("purge").map(String::as_str) == Some("true");
    let rom_name = file_name(&cart.rom);
    match req.query.get("rom") {
        Some(rom) if *rom != rom_name => return Reply::error(409, "the game at that id has changed"),
        None if purge => return Reply::error(400, "rom param required"),
        _ => {}
    }
    let rom = match at(root, &["Games", tag, &rom_name]) {
        Ok(p) => p,
        Err(r) => return refused(r),
    };
    let extras = if purge {
        match companions(root, index, cart) {
            Ok(v) => v,
            Err(r) => return refused(r),
        }
    } else {
        Vec::new()
    };
    if fs::remove_file(&rom).is_err() {
        return Reply::error(500, "delete failed");
    }
    forget_gift(root, cart.platform, &cart.stem, None);
    if purge {
        for path in extras {
            let _ = fs::remove_file(path);
        }
        for dir in state_dirs(root, cart) {
            let _ = fs::remove_dir_all(dir);
        }
        refs::drop_all(root, cart.platform, &cart.stem, index.guide(&cart.stem));
        super::carts::forget(root, cart.platform, &cart.stem);
    }
    ok()
}

fn forget_gift(root: &Path, platform: Platform, stem: &str, to: Option<&str>) {
    if !root.join(crate::romm::GIFTS).is_file() {
        return;
    }
    let mut gifts = crate::romm::Gifts::load(root);
    match to {
        Some(new) => gifts.rename(root, platform, stem, new),
        None => gifts.remove(root, platform, stem),
    }
}

fn new_rom_name(cart: &Cart, name: &str) -> Result<String, Refused> {
    card_name(name)?;
    if name.starts_with('.') {
        return Err(Refused::new(400, format!("{name:?} starts with a dot")));
    }
    if cart.platform.accepts(Path::new(name)) {
        return Ok(name.to_string());
    }
    let ext = cart.rom.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
    let full = format!("{name}.{ext}");
    card_name(&full)?;
    Ok(full)
}

fn renamed(old: &str, new_stem: &str) -> Result<String, Refused> {
    let rest = old
        .strip_prefix(stem_of(old).as_str())
        .ok_or_else(|| Refused::new(500, "rename failed"))?;
    let out = format!("{new_stem}{rest}");
    card_name(&out)?;
    Ok(out)
}

fn plan_step(
    plan: &mut Vec<(PathBuf, PathBuf)>,
    root: &Path,
    tag: &str,
    new_stem: &str,
    folder: &str,
    old: &str,
) -> Result<(), Refused> {
    let to = renamed(old, new_stem)?;
    plan.push((at(root, &[folder, tag, old])?, at(root, &[folder, tag, &to])?));
    Ok(())
}

fn rename(root: &Path, index: &Index, cart: &Cart, name: &str) -> Result<(), Refused> {
    let tag = cart.platform.dir_name();
    let new_rom = new_rom_name(cart, name)?;
    let new_stem = stem_of(&new_rom);
    if new_stem.is_empty() {
        return Err(Refused::new(400, "empty file name"));
    }
    let rom_name = file_name(&cart.rom);
    if new_rom == rom_name {
        return Ok(());
    }
    let mut plan: Vec<(PathBuf, PathBuf)> = Vec::new();
    plan.push((at(root, &["Games", tag, &rom_name])?, at(root, &["Games", tag, &new_rom])?));
    for save in index.saves(&cart.stem) {
        plan_step(&mut plan, root, tag, &new_stem, "Saves", save)?;
    }
    if let Some(label) = &cart.label {
        plan_step(&mut plan, root, tag, &new_stem, "Labels", &file_name(label))?;
    }
    let guide = index.guide(&cart.stem);
    let guide_to = match guide {
        Some(g) => {
            plan_step(&mut plan, root, tag, &new_stem, "Guides", g)?;
            Some(renamed(g, &new_stem)?)
        }
        None => None,
    };
    for core in Core::ALL {
        let from = at(root, &["States", tag, core.as_str(), &cart.stem])?;
        if from.is_dir() {
            plan.push((from, at(root, &["States", tag, core.as_str(), &new_stem])?));
        }
    }
    for (from, to) in &plan {
        if fs::symlink_metadata(to).is_ok() && !same_inode(from, to) {
            return Err(Refused::new(409, format!("{} already exists", file_name(to))));
        }
    }
    let mut done: Vec<&(PathBuf, PathBuf)> = Vec::new();
    for step in &plan {
        if let Err(e) = super::move_path(&step.0, &step.1) {
            for (from, to) in done.into_iter().rev() {
                let _ = fs::rename(to, from);
            }
            return Err(Refused::new(500, format!("rename failed: {e}")));
        }
        done.push(step);
    }
    let guide_move = match (guide, guide_to.as_deref()) {
        (Some(from), Some(to)) => Some(GuideMove { from, to }),
        _ => None,
    };
    refs::carry(root, cart.platform, &cart.stem, &new_stem, guide_move);
    forget_gift(root, cart.platform, &cart.stem, Some(&new_stem));
    super::carts::forget(root, cart.platform, &cart.stem);
    Ok(())
}

fn file_entry(path: &Path, name: &str) -> Value {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    json!({ "name": name, "size": size, "modified": modified_ms(path) })
}

fn list_files(root: &Path, folder: &str, tag: &str, names: &[&str]) -> Reply {
    let mut files = Vec::new();
    for name in names {
        match at(root, &[folder, tag, name]) {
            Ok(p) => files.push(file_entry(&p, name)),
            Err(r) => return refused(r),
        }
    }
    Reply::json(json!({ "files": files }))
}

fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| !e.is_empty())
}

fn upload(req: &mut Request, dest: Result<PathBuf, Refused>, max: Option<usize>) -> Result<(), Reply> {
    let dest = match dest {
        Ok(d) => d,
        Err(r) => {
            drain(req);
            return Err(refused(r));
        }
    };
    if dest.parent().map(fs::create_dir_all).is_some_and(|r| r.is_err()) {
        drain(req);
        return Err(Reply::error(500, "upload failed"));
    }
    let body = take_body(req);
    if max.is_some_and(|m| body.len() > m as u64) {
        return Err(Reply::error(413, "file too large"));
    }
    body.save_to(&dest).map_err(|_| Reply::error(500, "upload failed"))
}

fn saves(root: &Path, index: &Index, cart: &Cart, req: &mut Request) -> Reply {
    let tag = cart.platform.dir_name();
    let owned = index.saves(&cart.stem);
    match req.method.as_str() {
        "GET" | "HEAD" => match req.query.get("file") {
            None => {
                let names: Vec<&str> = owned.iter().map(String::as_str).collect();
                list_files(root, "Saves", tag, &names)
            }
            Some(file) => match owned.iter().find(|n| *n == file) {
                Some(name) => match at(root, &["Saves", tag, name]) {
                    Ok(p) => Reply::file(p),
                    Err(r) => refused(r),
                },
                None => Reply::error(404, "not found"),
            },
        },
        "DELETE" => {
            let Some(file) = req.query.get("file") else {
                return Reply::error(400, "file param required");
            };
            let Some(name) = owned.iter().find(|n| *n == file) else {
                return Reply::error(404, "not found");
            };
            match at(root, &["Saves", tag, name]) {
                Ok(p) => match fs::remove_file(p) {
                    Ok(()) => ok(),
                    Err(_) => Reply::error(500, "delete failed"),
                },
                Err(r) => refused(r),
            }
        }
        "POST" => {
            let Some(name) = req.query.get("name").cloned().filter(|n| !n.is_empty()) else {
                drain(req);
                return Reply::error(400, "name param required");
            };
            let dest = card_name(&name).and_then(|_| {
                let ext = extension(&name).ok_or_else(|| Refused::new(400, "name needs an extension"))?;
                if !SAVE_EXTENSIONS.contains(&ext.as_str()) {
                    return Err(Refused::new(400, "saves are .sav or .srm"));
                }
                let target = format!("{}.{ext}", cart.stem);
                card_name(&target)?;
                at(root, &["Saves", tag, &target])
            });
            match upload(req, dest, None) {
                Ok(()) => Reply::json(json!({ "ok": true, "resumeCores": resume_cores(root, cart) })),
                Err(reply) => reply,
            }
        }
        _ => Reply::error(405, "method not allowed"),
    }
}

fn guides(root: &Path, index: &Index, cart: &Cart, req: &mut Request) -> Reply {
    let tag = cart.platform.dir_name();
    let owned = index.guide(&cart.stem);
    match req.method.as_str() {
        "GET" | "HEAD" => match req.query.get("file") {
            None => list_files(root, "Guides", tag, &owned.into_iter().collect::<Vec<_>>()),
            Some(file) => match owned.filter(|n| n == file) {
                Some(name) => match at(root, &["Guides", tag, name]) {
                    Ok(p) => Reply::file(p),
                    Err(r) => refused(r),
                },
                None => Reply::error(404, "not found"),
            },
        },
        "DELETE" => {
            let Some(file) = req.query.get("file") else {
                return Reply::error(400, "file param required");
            };
            let Some(name) = owned.filter(|n| n == file) else {
                return Reply::error(404, "not found");
            };
            match at(root, &["Guides", tag, name]) {
                Ok(p) => match fs::remove_file(p) {
                    Ok(()) => {
                        refs::guide_spot(root, cart.platform, name, None);
                        ok()
                    }
                    Err(_) => Reply::error(500, "delete failed"),
                },
                Err(r) => refused(r),
            }
        }
        "POST" => {
            let Some(name) = req.query.get("name").cloned().filter(|n| !n.is_empty()) else {
                drain(req);
                return Reply::error(400, "name param required");
            };
            let dest = card_name(&name).and_then(|_| {
                if extension(&name).as_deref() != Some("txt") {
                    return Err(Refused::new(400, "guides are .txt"));
                }
                let target = owned.map(str::to_string).unwrap_or_else(|| format!("{}.txt", cart.stem));
                card_name(&target)?;
                at(root, &["Guides", tag, &target])
            });
            match upload(req, dest, Some(MAX_GUIDE)) {
                Ok(()) => ok(),
                Err(reply) => reply,
            }
        }
        _ => Reply::error(405, "method not allowed"),
    }
}
