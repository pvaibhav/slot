use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use slot_store::{Cart, Platform};

pub const GIFTS: &str = "Config/romm_gifts.txt";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gifts {
    held: BTreeSet<(String, String)>,
}

fn key(platform: Platform, stem: &str) -> (String, String) {
    (platform.dir_name().to_string(), stem.to_string())
}

impl Gifts {
    fn sync(&mut self, root: &Path) {
        self.held = Gifts::load(root).held;
    }

    pub fn load(root: &Path) -> Gifts {
        let mut held = BTreeSet::new();
        if let Ok(text) = fs::read_to_string(root.join(GIFTS)) {
            for line in text.lines() {
                let Some((dir, stem)) = line.split_once('/') else {
                    continue;
                };
                if stem.is_empty() || !Platform::ALL.iter().any(|p| p.dir_name() == dir) {
                    continue;
                }
                held.insert((dir.to_string(), stem.to_string()));
            }
        }
        Gifts { held }
    }

    pub fn add(&mut self, root: &Path, platform: Platform, stem: &str) {
        self.sync(root);
        if self.held.insert(key(platform, stem)) {
            self.save(root);
        }
    }

    pub fn remove(&mut self, root: &Path, platform: Platform, stem: &str) {
        self.sync(root);
        if self.held.remove(&key(platform, stem)) {
            self.save(root);
        }
    }

    pub fn rename(&mut self, root: &Path, platform: Platform, old: &str, new: &str) {
        self.sync(root);
        if self.held.remove(&key(platform, old)) {
            self.held.insert(key(platform, new));
            self.save(root);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    pub fn holds(&self, platform: Platform, stem: &str) -> bool {
        self.held.contains(&key(platform, stem))
    }

    pub fn retain_games(&mut self, root: &Path, carts: &[Cart]) {
        self.sync(root);
        let before = self.held.len();
        self.held.retain(|(dir, stem)| {
            carts
                .iter()
                .any(|c| c.platform.dir_name() == dir && c.stem == *stem)
        });
        if self.held.len() != before {
            self.save(root);
        }
    }

    fn save(&self, root: &Path) {
        let path = root.join(GIFTS);
        if self.held.is_empty() {
            let _ = fs::remove_file(&path);
            return;
        }
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let mut text = String::new();
        for (dir, stem) in &self.held {
            text.push_str(dir);
            text.push('/');
            text.push_str(stem);
            text.push('\n');
        }
        if let Err(e) = slot_store::atomic_write(&path, text.as_bytes()) {
            eprintln!("slot: could not save {GIFTS}: {e}");
        }
    }
}
