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

pub fn missing_art(root: &Path) -> Vec<Cart> {
    slot_store::scan(root)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c.label.is_none())
        .collect()
}
