use std::path::{Path, PathBuf};

use slot_store::{Core, Platform};

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

/// The names armGBA looks for a Game Boy, Pocket, Color or Advance boot ROM under.
const GB_BOOT_FILES: [&str; 8] = [
    "gb_bios.bin",
    "dmg_boot.bin",
    "mgb_boot.bin",
    "gbc_bios.bin",
    "cgb_boot.bin",
    "cgb_agb_boot.bin",
    "cgb_agb0_boot.bin",
    "agb_boot.bin",
];

pub fn has_gb_boot_rom(root: &Path) -> bool {
    let dir = bios_dir(root);
    GB_BOOT_FILES.iter().any(|name| dir.join(name).is_file())
}

const GB_BIOS: (&str, usize) = ("gb_bios.bin", 0x100);
const GBC_BIOS: (&str, usize) = ("gbc_bios.bin", 0x900);

/// armGBA's boot ROM names for a Game Boy game and a Game Boy Color game, in the order it tries
/// them, and how much of the file it reads.
const ARMGBA_GB_BOOT: [&str; 2] = ["gb_bios.bin", "dmg_boot.bin"];
const ARMGBA_GBC_BOOT: [&str; 2] = ["gbc_bios.bin", "cgb_boot.bin"];
const ARMGBA_BOOT_READ: usize = 0x901;

/// The logo every Nintendo boot ROM checks cartridges against, which SameBoy's do not hold.
const NINTENDO_LOGO: [u8; 48] = [
    0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D,
    0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99,
    0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
];

pub fn bios_crc(root: &Path, platform: Platform, core: Core) -> u32 {
    let (name, len) = match (platform.hardware(), core) {
        (Platform::Gba, Core::Mgba | Core::Gpsp) if has_real_bios(root) => {
            (BIOS_FILE, BIOS_BYTES as usize)
        }
        (Platform::Gba, Core::Armgba) => (BIOS_FILE, BIOS_BYTES as usize),
        (Platform::Gb, Core::Mgba | Core::Gambatte) => GB_BIOS,
        (Platform::Gbc, Core::Mgba | Core::Gambatte) => GBC_BIOS,
        (Platform::Gb, Core::Armgba) => return armgba_boot_crc(root, &ARMGBA_GB_BOOT),
        (Platform::Gbc, Core::Armgba) => return armgba_boot_crc(root, &ARMGBA_GBC_BOOT),
        _ => return 0,
    };
    match std::fs::read(bios_dir(root).join(name)) {
        Ok(bytes) if bytes.len() >= len => crc32fast::hash(&bytes[..len]),
        _ => 0,
    }
}

/// The boot ROM armGBA starts a Game Boy game through: the first of `names` that is Nintendo's,
/// else the first there at all.
fn armgba_boot_crc(root: &Path, names: &[&str]) -> u32 {
    let found: Vec<Vec<u8>> = names
        .iter()
        .filter_map(|name| std::fs::read(bios_dir(root).join(name)).ok())
        .filter(|bytes| !bytes.is_empty())
        .map(|mut bytes| {
            bytes.truncate(ARMGBA_BOOT_READ);
            bytes
        })
        .collect();
    let nintendo = |bytes: &&Vec<u8>| {
        bytes
            .windows(NINTENDO_LOGO.len())
            .any(|w| w == NINTENDO_LOGO)
    };
    found
        .iter()
        .find(nintendo)
        .or(found.first())
        .map_or(0, |bytes| crc32fast::hash(bytes))
}

pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("Logs")
}

pub fn saves_dir(root: &Path) -> PathBuf {
    root.join("Saves")
}
