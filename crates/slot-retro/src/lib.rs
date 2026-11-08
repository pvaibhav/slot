macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod achievement_memory;
mod core;
mod dsperate;
mod ffi;
mod libretro;
mod link;
mod mock;
pub mod rtc;
mod rumble;

pub use achievement_memory::{AchievementMemory, AchievementRegion, ACHIEVEMENT_RAM_SIZE};
pub use core::{mask_for, AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};
pub use dsperate::{ds_buttons, DsperateCore, DS_H, DS_W};
pub use ffi::{
    NETPACKET_BROADCAST, NETPACKET_FLUSH_HINT, NETPACKET_RELIABLE, NETPACKET_UNRELIABLE,
    NETPACKET_UNSEQUENCED,
};
pub use libretro::LibretroCore;
pub use link::{Link, LinkChannel, LoopbackLink};
pub use mock::MockCore;
pub use rumble::Rumble;
