use std::io;
use std::path::Path;

pub const CONFIG_DIR: &str = "Config";

const MOVED: [&str; 5] = [
    "slot.state",
    "selected_core.ini",
    "video_mode.ini",
    "cart_shell.ini",
    "retroachievements.toml",
];

/// The fork's own Wi-Fi left these behind. Networks are now joined from the Network menu,
/// which keeps them in `Config/wifi.conf`.
const RETIRED_FILES: [&str; 4] = [
    "System/wifi.toml",
    "Config/wifi.toml",
    "Config/wifi.toml.example",
    "System/slot-services",
];
const RETIRED_DIRS: [&str; 1] = ["System/slot-net"];

const RENAMED: [(&str, &str); 4] = [
    ("Config/selected_core.ini", crate::SELECTED_CORE_FILE),
    ("Config/video_mode.ini", "Config/video_mode.txt"),
    ("Config/cart_shell.ini", crate::CART_SHELL_FILE),
    ("Labels/cart_shell.ini", crate::LABELS_SHELL_FILE),
];

pub fn move_config(root: &Path) -> io::Result<()> {
    let config = root.join(CONFIG_DIR);
    std::fs::create_dir_all(&config)?;
    for name in MOVED {
        let old = root.join("System").join(name);
        let new = config.join(name);
        if old.is_file() && !new.exists() {
            std::fs::rename(&old, &new)?;
        }
    }
    for (old, new) in RENAMED {
        let old = root.join(old);
        let new = root.join(new);
        if old.is_file() && !new.exists() {
            std::fs::rename(&old, &new)?;
        }
    }
    for name in RETIRED_FILES {
        let path = root.join(name);
        if path.is_file() {
            std::fs::remove_file(&path)?;
        }
    }
    for name in RETIRED_DIRS {
        let path = root.join(name);
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        }
    }
    Ok(())
}
