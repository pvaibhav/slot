use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use slot_store::Platform;

use super::api::{Api, ApiError, Cancel, Install, Outcome};

pub const TEMP_DIR: &str = "Cache/romm";

const CHUNK: usize = 64 * 1024;
const CARD_FULL: &str = "Card is full";
const SAVE_FAILED: &str = "Could not save the game";
const UNSUPPORTED_FILE: &str = "Unsupported file type";

#[derive(Clone, Debug, PartialEq)]
pub struct Finished {
    pub outcome: Outcome,
    pub stem: Option<String>,
    pub platform: Option<Platform>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Stopped {
    Cancelled,
    Offline,
    Transient,
    Lost(ApiError),
}

pub fn target_dir(slug: &str) -> Option<Platform> {
    match slug.to_ascii_lowercase().as_str() {
        "gb" => Some(Platform::Gb),
        "gbc" => Some(Platform::Gbc),
        "gba" => Some(Platform::Gba),
        _ => None,
    }
}

pub fn free_space(path: &Path) -> u64 {
    let Ok(c_path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) else {
        return 0;
    };
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return 0;
    }
    (stat.f_bavail as u64).saturating_mul(stat.f_frsize as u64)
}

pub fn sweep(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root.join(TEMP_DIR)) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn failed(reason: &str) -> Result<Finished, Stopped> {
    Ok(Finished {
        outcome: Outcome::Failed(reason.to_string()),
        stem: None,
        platform: None,
    })
}

fn stopped(error: ApiError) -> Result<Finished, Stopped> {
    match error {
        ApiError::Offline => Err(Stopped::Offline),
        ApiError::Server(_) | ApiError::Truncated => Err(Stopped::Transient),
        ApiError::Cancelled => Err(Stopped::Cancelled),
        ApiError::Write(_) => failed(CARD_FULL),
        ApiError::Client(n) => failed(&format!("RomM refused the request ({n})")),
        lost @ (ApiError::Unauthorized | ApiError::NotFound) => Err(Stopped::Lost(lost)),
    }
}

fn base_name(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or("");
    last.trim_end_matches([' ', '.']).trim_start().to_string()
}

fn temp_stem(id: &str) -> String {
    let clean: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if clean.is_empty() {
        "request".to_string()
    } else {
        clean
    }
}

fn is_installed(dir: &Path, name: &str) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
    })
}

struct Temps(Vec<PathBuf>);

impl Drop for Temps {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn install(
    root: &Path,
    api: &dyn Api,
    req: &Install,
    cancel: &Cancel,
    free: fn(&Path) -> u64,
) -> Result<Finished, Stopped> {
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    let rom = match api.rom(req.rom_id) {
        Ok(rom) => rom,
        Err(e) => return stopped(e),
    };
    let Some(platform) = target_dir(&rom.platform_slug) else {
        return failed("slot only plays GB, GBC and GBA");
    };
    if req.file_ids.len() > 1 {
        return failed("Multi-file ROMs are not supported");
    }
    let fs_name = match req.file_ids.as_slice() {
        [id] => rom.files.iter().find(|file| file.id == *id),
        _ => None,
    }
    .map_or(rom.fs_name.as_str(), |file| file.file_name.as_str());
    let name = base_name(fs_name);
    if name.is_empty() {
        return failed(UNSUPPORTED_FILE);
    }
    let is_zip = Path::new(&name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("zip"));
    if !is_zip && !platform.accepts(Path::new(&name)) {
        return failed(UNSUPPORTED_FILE);
    }
    let games = root.join("Games").join(platform.dir_name());
    if !is_zip && is_installed(&games, &name) {
        return Ok(Finished {
            outcome: Outcome::AlreadyInstalled,
            stem: None,
            platform: None,
        });
    }

    let temp_dir = root.join(TEMP_DIR);
    if std::fs::create_dir_all(&temp_dir).is_err() {
        return failed(CARD_FULL);
    }
    let stem = temp_stem(&req.id);
    let part = temp_dir.join(format!("{stem}.part"));
    let unpacked = temp_dir.join(format!("{stem}.rom"));
    let _temps = Temps(vec![part.clone(), unpacked.clone()]);

    if let Err(e) = api.download(req.rom_id, fs_name, &req.file_ids, &part, cancel) {
        return stopped(e);
    }
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    let compressed = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);

    let (source, final_name) = if is_zip {
        let inner = match unpack(&part, &unpacked, platform, compressed, root, free, cancel) {
            Ok(inner) => inner,
            Err(result) => return result,
        };
        (unpacked.clone(), inner)
    } else {
        (part.clone(), name)
    };

    if is_installed(&games, &final_name) {
        return Ok(Finished {
            outcome: Outcome::AlreadyInstalled,
            stem: None,
            platform: None,
        });
    }
    if cancel.is_set() {
        return Err(Stopped::Cancelled);
    }
    let size = std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
    let same_device = match (std::fs::metadata(&source), std::fs::metadata(&games)) {
        (Ok(a), Ok(b)) => a.dev() == b.dev(),
        _ => true,
    };
    if !same_device && free(&games) < size {
        return failed(CARD_FULL);
    }
    let target = games.join(&final_name);
    if std::fs::rename(&source, &target).is_err() {
        return failed(SAVE_FAILED);
    }
    let rom_stem = Path::new(&final_name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned());
    Ok(Finished {
        outcome: Outcome::Done,
        stem: rom_stem,
        platform: Some(platform),
    })
}

fn unpack(
    part: &Path,
    unpacked: &Path,
    platform: Platform,
    compressed: u64,
    root: &Path,
    free: fn(&Path) -> u64,
    cancel: &Cancel,
) -> Result<String, Result<Finished, Stopped>> {
    let file = File::open(part).map_err(|_| failed("Zip could not be read"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| failed("Zip could not be read"))?;
    let mut found: Vec<(usize, String)> = Vec::new();
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index_raw(index) else {
            continue;
        };
        if entry.is_dir() {
            continue;
        }
        let name = base_name(entry.name());
        if !name.is_empty() && platform.accepts(Path::new(&name)) {
            found.push((index, name));
        }
    }
    let (index, name) = match found.len() {
        0 => return Err(failed("Zip holds no playable ROM")),
        1 => found.remove(0),
        _ => return Err(failed("Zip holds more than one ROM")),
    };
    let mut entry = archive
        .by_index(index)
        .map_err(|_| failed("Zip could not be read"))?;
    if free(root) < compressed.saturating_add(entry.size()) {
        return Err(failed(CARD_FULL));
    }
    let mut out = File::create(unpacked).map_err(|_| failed(CARD_FULL))?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        if cancel.is_set() {
            return Err(Err(Stopped::Cancelled));
        }
        let n = entry
            .read(&mut buf)
            .map_err(|_| failed("Zip could not be read"))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|_| failed(CARD_FULL))?;
    }
    out.flush().map_err(|_| failed(CARD_FULL))?;
    Ok(name)
}
