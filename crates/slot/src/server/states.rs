use std::fs;
use std::path::Path;

use serde_json::{json, Value};
use slot_store::{is_stamp, Cart, Core, StateRing};

use super::games::{at, ok, refused};
use super::paths::Refused;
use super::{Reply, Request};

const RESUME: &str = "resume";

fn core_for(name: &str) -> Option<Core> {
    Core::ALL.into_iter().find(|c| c.as_str() == name)
}

fn ring_for(root: &Path, cart: &Cart, core: Core) -> Result<StateRing, Refused> {
    at(root, &["States", cart.platform.dir_name(), core.as_str(), &cart.stem])?;
    Ok(StateRing::new(root, cart.platform, core, &cart.stem))
}

fn core_entry(root: &Path, cart: &Cart, core: Core) -> Option<Value> {
    let ring = ring_for(root, cart, core).ok()?;
    let states: Vec<Value> = ring
        .list()
        .unwrap_or_default()
        .iter()
        .map(|e| {
            let size = fs::metadata(&e.state).map(|m| m.len()).unwrap_or(0);
            json!({ "stamp": e.stamp, "size": size })
        })
        .collect();
    let resume = ring.has_resume();
    if states.is_empty() && !resume {
        return None;
    }
    Some(json!({ "id": core.as_str(), "core": core.text(), "resume": resume, "states": states }))
}

fn listing(root: &Path, cart: &Cart) -> Reply {
    let cores: Vec<Value> = Core::ALL.into_iter().filter(|c| c.runs(cart.platform)).filter_map(|c| core_entry(root, cart, c)).collect();
    Reply::json(json!({ "cores": cores }))
}

fn known(ring: &StateRing, stamp: &str) -> bool {
    if stamp == RESUME {
        return ring.has_resume();
    }
    ring.list()
        .map(|l| l.iter().any(|e| e.stamp == stamp))
        .unwrap_or(false)
}

fn file_in(root: &Path, cart: &Cart, core: Core, name: &str) -> Result<std::path::PathBuf, Reply> {
    at(root, &["States", cart.platform.dir_name(), core.as_str(), &cart.stem, name]).map_err(refused)
}

pub fn states(root: &Path, cart: &Cart, req: &Request) -> Reply {
    let method = req.method.as_str();
    let read = method == "GET" || method == "HEAD";
    if req.path.len() == 5 {
        return if read { listing(root, cart) } else { Reply::error(405, "method not allowed") };
    }
    let Some(core) = req.path.get(5).and_then(|c| core_for(c)) else {
        return Reply::error(400, "unknown core");
    };
    let Some(stamp) = req.path.get(6) else { return Reply::error(404, "not found") };
    let resume = stamp == RESUME;
    if !resume && !is_stamp(stamp) {
        return Reply::error(400, "bad state stamp");
    }
    let thumb = match req.path.get(7).map(String::as_str) {
        None => false,
        Some("thumb") if req.path.len() == 8 && !resume => true,
        _ => return Reply::error(404, "not found"),
    };
    if !read && (method != "DELETE" || thumb) {
        return Reply::error(405, "method not allowed");
    }
    let ring = match ring_for(root, cart, core) {
        Ok(r) => r,
        Err(r) => return refused(r),
    };
    if !known(&ring, stamp) {
        return Reply::error(404, "not found");
    }
    if method == "DELETE" {
        let removed = if resume {
            file_in(root, cart, core, "resume.state").and_then(|p| fs::remove_file(p).map_err(|_| Reply::error(500, "delete failed")))
        } else {
            ring.remove(stamp).map_err(|_| Reply::error(500, "delete failed"))
        };
        return match removed {
            Ok(()) => ok(),
            Err(r) => r,
        };
    }
    let name = if thumb { format!("{stamp}.png") } else { format!("{stamp}.state") };
    match file_in(root, cart, core, &name) {
        Ok(p) if p.is_file() => Reply::file(p),
        Ok(_) => Reply::error(404, "not found"),
        Err(r) => r,
    }
}
