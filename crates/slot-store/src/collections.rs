use crate::atomic::atomic_write;
use crate::scan::listing;
use crate::slot_state::{platform_key, recent_value, Recent};
use std::path::Path;

pub const COLLECTIONS_DIR: &str = "Collections";
pub const ORDER_FILE: &str = "order.txt";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collection {
    pub name: String,
    pub games: Vec<Recent>,
}

impl Collection {
    pub fn add(&mut self, games: &[Recent]) -> usize {
        let mut added = 0;
        for g in games {
            if !self.games.contains(g) {
                self.games.push(g.clone());
                added += 1;
            }
        }
        added
    }

    pub fn remove(&mut self, game: &Recent) -> bool {
        let before = self.games.len();
        self.games.retain(|g| g != game);
        self.games.len() != before
    }

    pub fn swap(&mut self, at: usize, by: i32) -> Option<usize> {
        let to = at.checked_add_signed(by as isize)?;
        if at >= self.games.len() || to >= self.games.len() {
            return None;
        }
        self.games.swap(at, to);
        Some(to)
    }
}

fn decode(bytes: &[u8]) -> String {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    text.strip_prefix('\u{feff}')
        .map(str::to_string)
        .unwrap_or(text)
}

fn parse_games(text: &str) -> Vec<Recent> {
    let mut games: Vec<Recent> = Vec::new();
    for line in text.lines() {
        if let Some(g) = recent_value(line.trim()) {
            if !games.contains(&g) {
                games.push(g);
            }
        }
    }
    games
}

pub fn read_collections(root: &Path) -> Vec<Collection> {
    let dir = root.join(COLLECTIONS_DIR);
    let files = listing(&dir);
    let mut stems: Vec<String> = files
        .iter()
        .filter(|(lower, _)| lower.ends_with(".txt") && lower.as_str() != ORDER_FILE)
        .map(|(_, name)| name[..name.len() - 4].to_string())
        .collect();
    stems.sort_by_key(|s| s.to_lowercase());
    let order_text = std::fs::read_to_string(dir.join(ORDER_FILE)).unwrap_or_default();
    let listed: Vec<&str> = order_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let mut names: Vec<String> = Vec::new();
    for line in &listed {
        if let Some(stem) = stems.iter().find(|s| s.eq_ignore_ascii_case(line)) {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(stem)) {
                names.push(stem.clone());
            }
        }
    }
    for stem in &stems {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(stem)) {
            names.push(stem.clone());
        }
    }
    if !stems.is_empty() && listed != names.iter().map(String::as_str).collect::<Vec<_>>() {
        let _ = write_order(root, &names);
    }
    names
        .into_iter()
        .map(|name| {
            let bytes = std::fs::read(dir.join(format!("{name}.txt"))).unwrap_or_default();
            let text = decode(&bytes);
            Collection {
                games: parse_games(&text),
                name,
            }
        })
        .collect()
}

pub fn write_collection(root: &Path, c: &Collection) -> std::io::Result<()> {
    let dir = root.join(COLLECTIONS_DIR);
    std::fs::create_dir_all(&dir)?;
    let body: String = c
        .games
        .iter()
        .map(|g| format!("{}/{}\n", platform_key(g.platform), g.stem))
        .collect();
    atomic_write(&dir.join(format!("{}.txt", c.name)), body.as_bytes())
}

pub fn delete_collection(root: &Path, name: &str) -> std::io::Result<()> {
    std::fs::remove_file(root.join(COLLECTIONS_DIR).join(format!("{name}.txt")))
}

pub fn write_order(root: &Path, names: &[String]) -> std::io::Result<()> {
    let dir = root.join(COLLECTIONS_DIR);
    std::fs::create_dir_all(&dir)?;
    let body: String = names.iter().map(|n| format!("{n}\n")).collect();
    atomic_write(&dir.join(ORDER_FILE), body.as_bytes())
}

pub fn name_ok(name: &str, existing: &[Collection]) -> bool {
    !name.trim().is_empty()
        && !name.eq_ignore_ascii_case("order")
        && !name.starts_with('.')
        && name.len() + ".txt".len() <= 255
        && !name.ends_with('.')
        && !name.ends_with(' ')
        && !name
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        && !existing.iter().any(|c| c.name.eq_ignore_ascii_case(name))
}
