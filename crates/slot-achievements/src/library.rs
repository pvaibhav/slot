//! Background library preparation. A fingerprint avoids re-reading unchanged ROMs at boot.
use std::collections::{BTreeMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::storage;

#[derive(Serialize, Deserialize)]
struct Entry {
    size: u64,
    modified_ns: u128,
    hash: String,
    #[serde(default)]
    checked_at: u64,
    #[serde(default)]
    badge_version: u8,
}

type Index = BTreeMap<PathBuf, Entry>;

fn fingerprint(path: &Path) -> Result<(u64, u128), String> {
    let metadata = path.metadata().map_err(|_| "Cannot read ROM metadata")?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    Ok((metadata.len(), modified))
}

pub(crate) fn hash(dir: &Path, path: &Path) -> Result<String, String> {
    let (size, modified_ns) = fingerprint(path)?;
    let index_path = dir.join("library.json");
    let mut index: Index = storage::read(&index_path).unwrap_or_default();
    if let Some(entry) = index
        .get(path)
        .filter(|e| e.size == size && e.modified_ns == modified_ns && modified_ns != 0)
    {
        return Ok(entry.hash.clone());
    }
    if size == 0 || size > 64 * 1024 * 1024 {
        return Err("Unsupported GBA ROM size".into());
    }
    let mut file = std::fs::File::open(path).map_err(|_| "Cannot open ROM")?;
    let mut buffer = [0; 64 * 1024];
    let mut md5 = md5::Context::new();
    loop {
        let n = file.read(&mut buffer).map_err(|_| "Cannot read ROM")?;
        if n == 0 {
            break;
        }
        md5.consume(&buffer[..n]);
    }
    let hash = format!("{:x}", md5.compute());
    // Fingerprinting is a convenience cache; a failure here must not stop identification.
    index.insert(
        path.into(),
        Entry {
            size,
            modified_ns,
            hash: hash.clone(),
            checked_at: 0,
            badge_version: 0,
        },
    );
    let _ = storage::write(&index_path, &index);
    Ok(hash)
}

pub(crate) fn pending(root: &Path, dir: &Path, now: u64) -> VecDeque<PathBuf> {
    let index: Index = storage::read(&dir.join("library.json")).unwrap_or_default();
    let mut paths: Vec<_> = std::fs::read_dir(root.join("Games/GBA"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("gba"))
                && !path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        })
        .filter(|path| {
            let Some(entry) = index.get(path) else {
                return true;
            };
            // Refresh definitions and server unlocks weekly, and reconsider unknown ROMs.
            fingerprint(path).ok() != Some((entry.size, entry.modified_ns))
                || entry.checked_at == 0
                || entry.badge_version < 2
                || now.saturating_sub(entry.checked_at) >= 7 * 86400
                || !dir.join(format!("{}.json", entry.hash)).exists()
        })
        .collect();
    paths.sort();
    paths.into()
}

pub(crate) fn checked(dir: &Path, path: &Path, now: u64) {
    let index_path = dir.join("library.json");
    let mut index: Index = storage::read(&index_path).unwrap_or_default();
    if let Some(entry) = index.get_mut(path) {
        entry.checked_at = now;
        entry.badge_version = 2;
    }
    let _ = storage::write(&index_path, &index);
}
