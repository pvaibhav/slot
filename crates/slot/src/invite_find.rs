use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};

use slot_store::Platform;

struct Ask {
    platform: Platform,
    crc: u32,
}

fn carts_for(root: &Path, platform: Platform) -> Vec<(String, PathBuf)> {
    let platforms: &[Platform] = match platform.hardware() {
        Platform::Gba => &[Platform::Gba, Platform::Gbav],
        Platform::Gb => &[Platform::Gb],
        Platform::Gbc => &[Platform::Gbc],
        _ => &[],
    };
    platforms
        .iter()
        .filter_map(|p| slot_store::scan_platform(root, *p).ok())
        .flatten()
        .map(|c| (c.stem, c.rom))
        .collect()
}

fn fingerprint(carts: &[(String, PathBuf)]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut paths: Vec<&PathBuf> = carts.iter().map(|(_, p)| p).collect();
    paths.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    paths.hash(&mut hasher);
    hasher.finish()
}

fn search(root: &Path, carts: &[(String, PathBuf)], crc: u32) -> (Option<String>, usize) {
    let paths: Vec<&Path> = carts.iter().map(|(_, p)| p.as_path()).collect();
    let known = crate::art::rom::remembered_crcs(root, &paths);
    if let Some(((stem, _), _)) = carts.iter().zip(&known).find(|(_, k)| **k == Some(crc)) {
        return (Some(stem.clone()), 0);
    }
    let mut hashed = 0;
    for ((stem, path), _) in carts.iter().zip(&known).filter(|(_, k)| k.is_none()) {
        hashed += 1;
        if crate::art::rom::cached_crc(root, path).ok() == Some(crc) {
            return (Some(stem.clone()), hashed);
        }
    }
    (None, hashed)
}

pub struct GameFinder {
    asks: Sender<Ask>,
    answers: Receiver<(u32, Option<String>)>,
}

impl GameFinder {
    pub fn spawn(root: PathBuf) -> GameFinder {
        let (asks, ask_rx) = channel::<Ask>();
        let (answer_tx, answers) = channel();
        std::thread::Builder::new()
            .name("invite-find".into())
            .spawn(move || {
                let mut hits: HashMap<u32, String> = HashMap::new();
                let mut misses: HashMap<u32, u64> = HashMap::new();
                while let Ok(ask) = ask_rx.recv() {
                    let from = crate::boot_time::ms();
                    let (found, how) = if let Some(stem) = hits.get(&ask.crc) {
                        (Some(stem.clone()), "from memory".to_string())
                    } else {
                        let carts = carts_for(&root, ask.platform);
                        let print = fingerprint(&carts);
                        if misses.get(&ask.crc) == Some(&print) {
                            (None, format!("from memory, {} carts listed", carts.len()))
                        } else {
                            let (found, hashed) = search(&root, &carts, ask.crc);
                            match &found {
                                Some(stem) => {
                                    misses.remove(&ask.crc);
                                    hits.insert(ask.crc, stem.clone());
                                }
                                None => {
                                    misses.insert(ask.crc, print);
                                }
                            }
                            (
                                found,
                                format!("after a search of {} carts, {hashed} hashed", carts.len()),
                            )
                        }
                    };
                    let at = crate::boot_time::ms();
                    eprintln!(
                        "slot: invite: finder answered {:08x} {} {how} at {at:.0} ms, took {:.0} ms",
                        ask.crc,
                        if found.is_some() { "hit" } else { "miss" },
                        at - from
                    );
                    if answer_tx.send((ask.crc, found)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        GameFinder { asks, answers }
    }

    pub fn ask(&mut self, platform: Platform, crc: u32) {
        let _ = self.asks.send(Ask { platform, crc });
    }

    pub fn answer(&mut self) -> Option<(u32, Option<String>)> {
        self.answers.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait(finder: &mut GameFinder) -> (u32, Option<String>) {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(a) = finder.answer() {
                return a;
            }
            assert!(Instant::now() < end, "no answer");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn put(dir: &Path, folder: &str, stem: &str, body: &[u8]) -> PathBuf {
        let games = dir.join("Games").join(folder);
        std::fs::create_dir_all(&games).unwrap();
        let rom = games.join(format!("{stem}.gba"));
        std::fs::write(&rom, body).unwrap();
        rom
    }

    fn setup() -> (tempfile::TempDir, u32) {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), Platform::Gba.dir_name(), "a", b"first rom");
        let b = put(dir.path(), Platform::Gba.dir_name(), "b", b"second rom");
        let crc = crate::art::rom::rom_crc(&b).unwrap();
        (dir, crc)
    }

    #[test]
    fn a_rom_on_the_card_is_found_by_crc() {
        let (dir, crc) = setup();
        let mut finder = GameFinder::spawn(dir.path().to_path_buf());
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, Some("b".to_string())));
    }

    #[test]
    fn a_remembered_crc_is_found_without_hashing() {
        let (dir, crc) = setup();
        let carts = carts_for(dir.path(), Platform::Gba);
        let (found, hashed) = search(dir.path(), &carts, crc);
        assert_eq!(found, Some("b".to_string()));
        assert!(hashed >= 1);
        assert_eq!(search(dir.path(), &carts, crc), (Some("b".to_string()), 0));
        search(dir.path(), &carts, crc ^ 1);
        assert_eq!(search(dir.path(), &carts, crc ^ 1), (None, 0));
    }

    #[test]
    fn a_gba_video_folder_is_searched_for_a_gba_advert() {
        let dir = tempfile::tempdir().unwrap();
        let v = put(dir.path(), Platform::Gbav.dir_name(), "v", b"video rom");
        let crc = crate::art::rom::rom_crc(&v).unwrap();
        let mut finder = GameFinder::spawn(dir.path().to_path_buf());
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, Some("v".to_string())));
    }

    #[test]
    fn a_missing_crc_answers_none() {
        let (dir, crc) = setup();
        let mut finder = GameFinder::spawn(dir.path().to_path_buf());
        finder.ask(Platform::Gba, crc ^ 1);
        assert_eq!(wait(&mut finder), (crc ^ 1, None));
    }

    #[test]
    fn a_repeat_question_is_answered_from_memory() {
        let (dir, crc) = setup();
        let mut finder = GameFinder::spawn(dir.path().to_path_buf());
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, Some("b".to_string())));
        std::fs::remove_dir_all(dir.path().join("Games")).unwrap();
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, Some("b".to_string())));
    }

    #[test]
    fn a_miss_is_asked_again_when_the_cart_list_changes() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), Platform::Gba.dir_name(), "a", b"first rom");
        let elsewhere = tempfile::tempdir().unwrap();
        let later = put(elsewhere.path(), "x", "b", b"second rom");
        let crc = crate::art::rom::rom_crc(&later).unwrap();
        let mut finder = GameFinder::spawn(dir.path().to_path_buf());
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, None));
        put(dir.path(), Platform::Gba.dir_name(), "b", b"second rom");
        finder.ask(Platform::Gba, crc);
        assert_eq!(wait(&mut finder), (crc, Some("b".to_string())));
    }
}
