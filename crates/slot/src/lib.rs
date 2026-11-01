macro_rules! eprintln {
    ($($arg:tt)*) => {{
        use std::io::Write as _;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

pub mod app;
pub mod art;
pub mod audio;
pub mod baseos;
pub mod bezel;
pub mod boot_time;
pub mod bootlogo;
pub mod build_info;
pub mod cable;
pub mod card_log;
pub mod cart_faces;
pub mod core;
pub mod core_picker;
#[cfg(unix)]
pub mod crash;
#[cfg(not(unix))]
pub mod crash {
    pub fn install() {}
}
pub mod drc;
pub mod emu;
pub mod face_builder;
pub mod fetch_art;
pub mod fetch_faces;
pub mod frames;
pub mod frontend;
pub mod input;
pub mod label_cache;
pub mod latency;
pub mod link_art_builder;
pub mod link_kind;
pub mod link_net;
pub mod link_radio;
pub mod link_screen;
pub mod link_start;
pub mod link_state;
pub mod loading_mark;
pub mod net_time;
pub mod network_faces;
pub mod networks;
pub mod panel_turn;
pub mod persist;
pub mod prefetch;
pub mod print_scene;
pub mod resample;
pub mod rewind;
pub mod root;
pub mod server;
pub mod session;
pub mod shot_loader;
pub mod shot_writer;
pub mod sntp;
pub mod state_writer;
pub mod studio;
pub mod studio_faces;
pub mod studio_view;
pub mod thumb;
pub mod update;
pub mod video_mode;
pub mod wallpaper;
pub mod wifi;
