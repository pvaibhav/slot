use std::path::Path;

use slot_store::cart_shell::{key, CART_SHELL_FILE, LABELS_SHELL_FILE};
use slot_store::{
    atomic_write, ini, read_slot_state, write_slot_state, Platform, CONFIG_DIR, GUIDE_SPOTS_FILE,
};

pub struct GuideMove<'a> {
    pub from: &'a str,
    pub to: &'a str,
}

pub fn carry(root: &Path, platform: Platform, stem: &str, new_stem: &str, guide: Option<GuideMove>) {
    lists(root, platform, stem, Some(new_stem));
    shells(root, stem, Some(new_stem));
    if let Some(g) = guide {
        guide_spot(root, platform, g.from, Some(g.to));
    }
}

pub fn drop_all(root: &Path, platform: Platform, stem: &str, guide: Option<&str>) {
    lists(root, platform, stem, None);
    shells(root, stem, None);
    if let Some(g) = guide {
        guide_spot(root, platform, g, None);
    }
}

pub fn guide_spot(root: &Path, platform: Platform, from: &str, to: Option<&str>) {
    let tag = platform.dir_name();
    let old = format!("{tag}/{from}");
    let new = to.map(|t| format!("{tag}/{t}"));
    let file = format!("{CONFIG_DIR}/{GUIDE_SPOTS_FILE}");
    rekey(root, &file, &|k| k.eq_ignore_ascii_case(&old), new.as_deref());
}

fn shells(root: &Path, stem: &str, to: Option<&str>) {
    let want = key(stem);
    let new = to.map(key);
    for file in [CART_SHELL_FILE, LABELS_SHELL_FILE] {
        rekey(root, file, &|k| key(k) == want, new.as_deref());
    }
}

fn rekey(root: &Path, file: &str, matches: &dyn Fn(&str) -> bool, to: Option<&str>) {
    let path = root.join(file);
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let found: Vec<(String, String)> = ini::parse(&text).into_iter().filter(|(k, _)| matches(k)).collect();
    if found.is_empty() {
        return;
    }
    let mut out = text;
    for (k, _) in &found {
        let Ok(next) = ini::set(&out, k, None) else { return };
        out = next;
    }
    if let (Some(new), Some((_, value))) = (to, found.first()) {
        let Ok(next) = ini::set(&out, new, Some(value)) else { return };
        out = next;
    }
    let _ = atomic_write(&path, out.as_bytes());
}

fn lists(root: &Path, platform: Platform, stem: &str, to: Option<&str>) {
    let mut state = read_slot_state(root);
    let before = (
        state.recents.clone(),
        state.favorites.clone(),
        state.cart.clone(),
        state.shelf_cart.clone(),
    );
    for list in [&mut state.recents, &mut state.favorites] {
        match to {
            Some(new) => {
                for r in list.iter_mut().filter(|r| r.platform == platform && r.stem == stem) {
                    r.stem = new.to_string();
                }
                let mut seen: Vec<(Platform, String)> = Vec::new();
                list.retain(|r| {
                    let id = (r.platform, r.stem.clone());
                    let fresh = !seen.contains(&id);
                    seen.push(id);
                    fresh
                });
            }
            None => list.retain(|r| !(r.platform == platform && r.stem == stem)),
        }
    }
    if state.cart_platform == Some(platform) && state.cart.as_deref() == Some(stem) {
        state.cart = to.map(str::to_string);
    }
    if state.shelf_cart.as_ref().is_some_and(|r| r.platform == platform && r.stem == stem) {
        state.shelf_cart = to.map(|new| slot_store::Recent { platform, stem: new.to_string() });
    }
    let after = (
        state.recents.clone(),
        state.favorites.clone(),
        state.cart.clone(),
        state.shelf_cart.clone(),
    );
    if before != after {
        let _ = write_slot_state(root, &state);
    }
}
