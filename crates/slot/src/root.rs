use std::path::{Path, PathBuf};

pub const DIRS: &[&str] = &[
    "BIOS",
    "Bezels",
    "Bezels/GBA",
    "Bezels/GB",
    "Bezels/GBC",
    "Cheats",
    "Cheats/GBA",
    "Cheats/GBAV",
    "Cheats/GB",
    "Cheats/GBC",
    "Cache",
    "Cache/romm",
    "Config",
    "Games",
    "Games/GBA",
    "Games/GBAV",
    "Games/GB",
    "Games/GBC",
    #[cfg(feature = "nds")]
    "Games/NDS",
    "Guides",
    "Guides/GBA",
    "Guides/GBAV",
    "Guides/GB",
    "Guides/GBC",
    "Labels",
    "Labels/GBA",
    "Labels/GBAV",
    "Labels/GB",
    "Labels/GBC",
    #[cfg(feature = "nds")]
    "Labels/NDS",
    "Logs",
    "Overlays",
    "Overlays/GBA",
    "Overlays/GB",
    "Overlays/GBC",
    #[cfg(feature = "nds")]
    "Overlays/NDS",
    "Saves",
    "Saves/GBA",
    "Saves/GBAV",
    "Saves/GB",
    "Saves/GBC",
    #[cfg(feature = "nds")]
    "Saves/NDS",
    "States",
    "States/GBA",
    "States/GBAV",
    "States/GB",
    "States/GBC",
    #[cfg(feature = "nds")]
    "States/NDS",
    "System",
    "Wallpapers",
];

pub fn ensure(root: &Path) {
    for sub in DIRS {
        let _ = std::fs::create_dir_all(root.join(sub));
    }
    move_label_cache(root);
    crate::romm::install::sweep(root);
    if let Err(e) = slot_store::move_config(root) {
        eprintln!("slot: config: {e}");
    }
}

pub fn move_label_cache(root: &Path) {
    let old = root.join("System/Cache");
    let labels = old.join("Labels");
    let new = root.join("Cache/Labels");
    if labels.is_dir() && !new.exists() {
        let _ = std::fs::create_dir_all(root.join("Cache"));
        let _ = std::fs::rename(&labels, &new);
    }
    if old.exists() {
        let _ = std::fs::remove_dir_all(&old);
    }
}

pub fn sweep_temps(root: &Path) {
    for sub in ["Config", "Saves", "States"] {
        slot_store::sweep_temps(&root.join(sub));
    }
}

pub fn bios_dir(root: &Path) -> PathBuf {
    root.join("BIOS")
}

const BIOS_FILE: &str = "gba_bios.bin";

const BIOS_BYTES: u64 = 16 * 1024;
const BIOS_FIRST_BYTE: u8 = 0x18;

pub fn has_real_bios(root: &Path) -> bool {
    let Ok(mut f) = std::fs::File::open(bios_dir(root).join(BIOS_FILE)) else {
        return false;
    };
    if !f.metadata().is_ok_and(|m| m.len() == BIOS_BYTES) {
        return false;
    }
    let mut first = [0u8; 1];
    std::io::Read::read_exact(&mut f, &mut first).is_ok() && first[0] == BIOS_FIRST_BYTE
}

pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("Logs")
}

pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("Saves")
}
