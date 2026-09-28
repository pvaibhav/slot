use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Config {
    pub enabled: bool,
    pub username: String,
    pub token: String,
    pub password: String,
}

impl Config {
    pub fn read(path: &Path) -> Result<Self, &'static str> {
        let text = std::fs::read_to_string(path).map_err(|_| "Cannot read achievement config")?;
        // TOML diagnostics include source lines, which may contain credentials.
        toml::from_str(&text).map_err(|_| "Invalid achievement config")
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Auth {
    pub username: String,
    pub token: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Achievement {
    #[serde(rename = "BadgeName", default)]
    pub badge: String,
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Points")]
    pub points: u32,
    #[serde(rename = "Flags")]
    pub flags: u32,
    #[serde(rename = "MemAddr")]
    pub definition: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Game {
    #[serde(rename = "ID")]
    pub id: u32,
    #[serde(rename = "ConsoleID")]
    pub console: u32,
    #[serde(rename = "Title")]
    pub title: String,
    #[serde(rename = "Achievements")]
    pub achievements: Vec<Achievement>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Unlock {
    pub id: u32,
    pub hash: String,
    pub earned_at: u64,
    pub synced: bool,
}

/// Confirmed records are retained to suppress repeated unlocks after an offline restart.
/// One account owns a ledger. A token is never copied into an unlock or game cache.
pub(crate) struct Store {
    pub dir: PathBuf,
    pub unlocks: BTreeMap<u32, Unlock>,
}

impl Store {
    pub fn open(root: &Path, username: &str) -> Result<Self, String> {
        let account = format!("{:x}", md5::compute(username.to_lowercase()));
        let dir = root.join("Saves/RetroAchievements").join(account);
        std::fs::create_dir_all(&dir).map_err(|_| "Cannot create achievement cache")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        let path = dir.join("unlocks.json");
        // A broken ledger is an error, never an empty ledger that overwrites unsynced awards.
        let unlocks = if path.exists() {
            read(&path)?
        } else {
            BTreeMap::new()
        };
        Ok(Self { dir, unlocks })
    }

    pub fn record(&mut self, unlock: Unlock) -> Result<bool, String> {
        if self.unlocks.contains_key(&unlock.id) {
            return Ok(false);
        }
        let id = unlock.id;
        self.unlocks.insert(id, unlock);
        if let Err(e) = self.save() {
            self.unlocks.remove(&id);
            return Err(e);
        }
        Ok(true)
    }

    pub fn ack(&mut self, id: u32) -> Result<(), String> {
        let was_synced = self.unlocks.get(&id).is_some_and(|u| u.synced);
        if let Some(unlock) = self.unlocks.get_mut(&id) {
            unlock.synced = true;
        }
        if let Err(e) = self.save() {
            if let Some(unlock) = self.unlocks.get_mut(&id) {
                unlock.synced = was_synced;
            }
            return Err(e);
        }
        Ok(())
    }

    fn save(&self) -> Result<(), String> {
        write(&self.dir.join("unlocks.json"), &self.unlocks)
    }
}

pub(crate) fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let file = std::fs::File::open(path).map_err(|_| "Cannot read achievement data")?;
    serde_json::from_reader(file).map_err(|_| "Invalid achievement data".into())
}

pub(crate) fn write(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|_| "Cannot encode achievement data")?;
    slot_store::atomic_write(path, &bytes).map_err(|_| "Cannot save achievement data")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // FAT cards may not support Unix permissions. No credentials are ever logged.
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
