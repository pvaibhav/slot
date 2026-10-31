macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod atomic;
mod bindings;
pub mod cart_shell;
mod config;
mod core;
pub mod gb;
mod gb_palette;
mod gba;
mod guides;
pub mod ini;
mod platform;
mod ring;
mod scan;
mod shots;
mod slot_state;
mod stamp;
mod zone;

pub use atomic::{atomic_write, sweep_temps};
pub use bindings::{
    read_carousel, read_controls, read_shortcuts, write_carousel, write_controls, write_shortcuts,
    Binding, CarouselAction, CarouselKeys, Controls, Device, GameButton, Modifier, Shortcut,
    ShortcutKeys,
};
pub use cart_shell::{Outline, ShellChoice, ShellFinish, CART_SHELL_FILE, LABELS_SHELL_FILE};
pub use config::{move_config, CONFIG_DIR};
pub use core::{
    core_for, core_for_platform, read_selected_cores, write_selected_core, Core, SELECTED_CORE_FILE,
};
pub use gb_palette::{GbColors, GbPalette};
pub use gba::{header_clean, header_code, header_title, header_title_code};
pub use guides::{
    decode_guide, find_guide, read_guide, read_guide_spot, write_guide_spot, GuideSpot, GUIDES_DIR,
    GUIDE_SPOTS_FILE,
};
pub use platform::Platform;
pub use ring::{is_stamp, StateEntry, StateRing, RING_MAX};
pub use scan::{initial, is_hidden, listing, scan, scan_platform, sort_key, Cart, StoreError};
pub use shots::{shot_stamp, ShotDir, ShotEntry, SCREENSHOTS_DIR};
pub use slot_state::{
    bezel_for_scale, bezel_scale, read_slot_state, write_slot_state, Appearance, BezelPick,
    Blending, Recent, Scale, Shader, SlotState, Stretch, WallpaperPick, WallpaperTint,
    BLUE_LIGHT_MAX, BRIGHTNESS_MAX, CONTRAST_MAX, FF_SPEEDS, FF_SPEED_DEFAULT, LID_OFF_MIN,
    RECENTS_MAX, SATURATION_MAX, SUNLIGHT_MAX, VOLUME_MAX,
};
pub use stamp::{
    civil_from_days, days_from_civil, days_in_month, format_stamp, parse_stamp, stamp_now,
};
pub use zone::Zone;
