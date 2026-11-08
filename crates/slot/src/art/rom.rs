use std::io::Read;
use std::path::Path;

use slot_store::Cart;

pub fn rom_crc(path: &Path) -> std::io::Result<u32> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = crc32fast::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(hasher.finalize());
        }
        hasher.update(&buf[..n]);
    }
}

const CRC_CACHE: &str = "Cache/art/crc.txt";

static CRC_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn cached_crc(root: &Path, rom: &Path) -> std::io::Result<u32> {
    let meta = std::fs::metadata(rom)?;
    let len = meta.len();
    let secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let key = rom.strip_prefix(root).unwrap_or(rom).to_string_lossy().into_owned();
    let path = root.join(CRC_CACHE);
    {
        let _held = CRC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        if let Some(crc) = text.lines().find_map(|l| cached_line(l, &key, len, secs)) {
            return Ok(crc);
        }
    }
    let crc = rom_crc(rom)?;
    let _held = CRC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut out: String = text
        .lines()
        .filter(|l| l.split('\t').next() != Some(key.as_str()) && l.split('\t').count() == 4)
        .map(|l| format!("{l}\n"))
        .collect();
    out.push_str(&format!("{key}\t{len}\t{secs}\t{crc:08x}\n"));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    super::make::write_replacing(&path, out.as_bytes())?;
    Ok(crc)
}

fn cached_line(line: &str, key: &str, len: u64, secs: u64) -> Option<u32> {
    let mut parts = line.split('\t');
    let (Some(k), Some(l), Some(s), Some(c), None) =
        (parts.next(), parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    if k != key || l.parse::<u64>().ok()? != len || s.parse::<u64>().ok()? != secs {
        return None;
    }
    u32::from_str_radix(c, 16).ok()
}

pub fn missing_art(root: &Path) -> Vec<Cart> {
    slot_store::scan(root)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.label.is_none())
        .collect()
}
