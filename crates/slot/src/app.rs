use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slot_gfx::{Canvas, GameLayout, Sizing, OUT_H, OUT_W, WHOLE_TEXTURE};
use slot_input::{Action, Btn, RawEvent, MENU_DOUBLE_TAP_MS, MUTE_CHORD_MS};
use slot_power::{Battery, Charge, LedState, LidPolicy, Power, RepairResult};
use slot_retro::LinkChannel;
use slot_store::{
    format_stamp, parse_stamp, read_slot_state, scan, write_slot_state, BezelPick, Blending, Cart,
    Core, GbColors, Platform, Recent, Scale, Shader, ShotDir, ShotEntry, SlotState, StateEntry,
    StateRing, Stretch, WallpaperPick, WallpaperTint, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, CONTRAST_MAX,
    FF_SPEEDS, RING_MAX, SATURATION_MAX, SUNLIGHT_MAX, VOLUME_MAX,
};
use slot_ui::{
    board_from, board_zoom, draw_empty_slot, draw_footer, draw_ground, draw_sticker, ease,
    footer_ink, grown, lid_at, lid_from, lift_of, on_board, repeat_gap, shelf_cart_at, BezelArt,
    ClockFaces, ClockPicker, ClockRow, ClockScreen, Draw, FfState, GbShell, Guide, Hud, HudKind,
    Icon, LinkBadge, Millis, Placed, Polaroids, PowerChoice, QuickMenu, QuickMenuFaces, QuickRow,
    QuickValue, Refusal, Shelf, ShotMode, ShotViewer, SlotChrome, TexId, Toast, CART_W, CHIP_H,
    CHIP_U, CHIP_V, CHIP_W, HINT_EDGE, HOP_LIFT, MOUTH_H, REPEAT_DELAY_MS, SHADOW_H, SHADOW_W,
    SOCKET_H, SOCKET_U, SOCKET_V, SOCKET_W, TURN_PAD,
};

use crate::audio::Sfx;
use crate::core_picker::{Chip, CorePicker, Outcome, Press};
use crate::link_kind::{link_carried, link_dual, link_kind, serial_option, LinkKind};
use crate::link_radio::{radio_jobs, LinkRole, RadioJob, RadioJobs};
use crate::link_screen::LinkSprites;
use crate::link_start::{link_port, LinkFail, LinkProgress, LinkStarter, LinkStep};
use crate::loading_mark::LoadingMark;
use crate::persist::{self, Snapshot};
use crate::print_scene::{print_draws, Cue, PrintArtState, PrintFrame, PrintTextures};
use crate::server::{ServerNet, ServerPaper, ServerSession};
use crate::art::jobs::{no_art_jobs, ArtEvent, ArtJob, ArtJobs};
use crate::art::make::Kind as ArtKind;
use crate::fetch_art::{stage_at, FetchArt, FetchTex, Landed as ArtLanded, Run as ArtRun, Scene as ArtScene, Stage};
use crate::update::{
    no_update_jobs, Kind, Release, UpdateEvent, UpdateFail, UpdateJob, UpdateJobs,
};
use crate::video_mode::{self, VideoMode};
use slot_store::{
    read_carousel, read_controls, read_shortcuts, write_carousel, write_controls, write_shortcuts,
    Binding, CarouselKeys, Controls, Device, GameButton, Modifier, ShortcutKeys, LID_OFF_MIN,
};

pub const RECORDING: &str = "Recording";
pub const CLASH_MARK: &str = "!";

#[derive(Copy, Clone, Debug)]
struct BindCapture {
    row: QuickRow,
    menu: bool,
    first: Option<Device>,
}

pub const INSERT_S: f32 = 0.73;
const INSERT_HOLD_S: f32 = 0.28;

pub const SEATED_AT: f32 = INSERT_S - INSERT_HOLD_S;

pub const EJECT_S: f32 = SEATED_AT;

const EJECT_HOLD_S: f32 = 0.35;

pub const POWER_ON_S: f32 = 0.22;

const POWER_OFF_S: f32 = 0.16;

const FOOTER_SLIDE_S: f32 = POWER_ON_S;

const BLANK_WARM_S: f32 = 1.5;
const BLANK_DIM: f32 = 0.1;

const VOLUME_STEP: u8 = 5;

const AUTOSAVE_MS: Millis = 60_000;
const RESUME_MS: Millis = 300_000;
const SETTLED_MS: Millis = 10_000;
const CARD_CHECK_MS: Millis = 2_000;

const BATTERY_CRITICAL: u8 = 5;

const REPAIR_BATTERY_MIN: u8 = 20;

const BATTERY_POLL_MS: Millis = 10_000;
const MENU_REVEAL_MS: Millis = 180;

const CHARGE_POLL_MS: Millis = 1_000;
const HEADPHONES_POLL_MS: Millis = 500;

const BATTERY_LOW: u8 = 20;

pub const UNDO_GRACE_MS: Millis = 30_000;

pub const LINK_LOST_MS: Millis = 2000;

const PLAY_HOLD_MS: Millis = 500;

const SHUTDOWN_SHOW_MS: Millis = 250;

const REPAIR_GAP: u32 = 12;

const CORE_PICKER_RECEDE: f32 = 0.26;

const SLOT_NAME_IN_MS: Millis = 200;
const SLOT_NAME_HOLD_MS: Millis = 1200;
const SLOT_NAME_OUT_MS: Millis = 800;
const SLOT_NAME_ALPHA: f32 = 1.0;
const CORE_PICKER_DIM: f32 = 0.25 / (1.0 - CORE_PICKER_RECEDE);
const LINK_TEXT_Y: f32 = 44.0;
const LINK_LEGEND_Y: f32 = OUT_H as f32 - MOUTH_H;

#[derive(Clone, Debug, PartialEq)]
pub struct StudioTex {
    pub faces: Vec<(crate::studio::LabelKind, TexId)>,
    pub bare: TexId,
    pub under: Option<[TexId; 2]>,
    pub w: u32,
    pub h: u32,
}

const WIFI_SETTLE_MS: Millis = 2_000;
const STUDIO_SLIDE_MS: f32 = 240.0;
const WIFI_SAMPLE_MS: Millis = 5000;
const WIFI_RETRY_MS: Millis = 60_000;
const NET_TOOLS: [QuickRow; 3] = [QuickRow::Server, QuickRow::CheckUpdates, QuickRow::FetchArt];
const WIFI_PROBE_MS: Millis = 1_000;
const SUMMARY_WAIT_MS: f64 = 1500.0;
const SLIP_SHUFFLE_PX: f32 = 28.0;
pub const LINK_SCRIM: [f32; 4] = [0.0, 0.0, 0.0, 0.5];
const LINK_LEGEND_GAP: f32 = 40.0;
const LINK_LEGEND_EDGE: f32 = 0.0;

fn link_keys_w(faces: &[(TexId, u32)]) -> f32 {
    faces
        .iter()
        .map(|(_, w)| w.saturating_sub(HINT_EDGE) as f32)
        .sum::<f32>()
        + LINK_LEGEND_GAP * faces.len().saturating_sub(1) as f32
}
const LID_SHADOW_W: f32 = 168.0;
const LID_SHADOW_H: f32 = 18.0;
const LID_SHADOW_DROP: f32 = 27.1;
const LID_SHADOW_ALPHA: f32 = 0.8;
const FACES_WAIT_MS: Millis = 1500;

const ALERT_HOLD: f32 = 0.45;
const ALERT_GONE: f32 = 0.9;

const CLOCK_FLOOR: i64 = 1_577_836_800;
const CLOCK_DRIFT_SECS: i64 = 2;

pub enum PendingUndo {
    Save {
        stamp: String,
        evicted: Option<(String, Vec<u8>, Vec<u8>)>,
    },
    Load {
        prior: Vec<u8>,
    },
    Reset {
        prior: Vec<u8>,
    },
}

struct LinkSession {
    client_id: u16,
    lost_at: Option<Millis>,
}

struct LinkStarting {
    starter: LinkStarter,
    client_id: u16,
}

struct Reload {
    stem: String,
    role: LinkRow,
    cancelled: bool,
    from: LinkKind,
    from_serial: &'static str,
    fallback: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameMenu {
    Pick(LinkRow),
    Working {
        role: LinkRow,
        step: LinkStep,
        since: Millis,
    },
    Linked {
        role: LinkRow,
        worked: Millis,
        since: Millis,
        opened: bool,
    },
    Failed {
        role: LinkRow,
        fail: LinkFail,
        worked: Millis,
        since: Millis,
    },
    Unplug {
        role: LinkRow,
        since: Millis,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkRow {
    Host,
    Join,
}

impl LinkRow {
    pub const ALL: [LinkRow; 2] = [LinkRow::Host, LinkRow::Join];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            LinkRow::Host => "Host",
            LinkRow::Join => "Join",
        }
    }

    pub fn role(self) -> LinkRole {
        match self {
            LinkRow::Host => LinkRole::Host,
            LinkRow::Join => LinkRole::Join,
        }
    }

    pub fn client_id(self) -> u16 {
        match self {
            LinkRow::Host => 0,
            LinkRow::Join => 1,
        }
    }

    pub fn other(self) -> LinkRow {
        match self {
            LinkRow::Host => LinkRow::Join,
            LinkRow::Join => LinkRow::Host,
        }
    }

    pub fn from_client_id(id: u16) -> LinkRow {
        if id == 0 {
            LinkRow::Host
        } else {
            LinkRow::Join
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LinkLegend {
    Cancel,
    Mode,
    Swap,
    Link,
    Ok,
    Back,
    EndLink,
}

impl LinkLegend {
    pub const ALL: [LinkLegend; 7] = [
        LinkLegend::Cancel,
        LinkLegend::Mode,
        LinkLegend::Swap,
        LinkLegend::Link,
        LinkLegend::Ok,
        LinkLegend::Back,
        LinkLegend::EndLink,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

pub const LINKED_HOLD_MS: Millis = 1000;

pub const UNPLUG_HOLD_MS: Millis = 420;

#[derive(Debug)]
pub enum Phase {
    SetClock {
        picker: ClockPicker,
        seed: i64,
        from_menu: bool,
    },
    Shelf,
    QuickMenu {
        row: QuickRow,
    },
    Inserting {
        cart: String,
        t: f32,
        core_ready: bool,
        resumed: bool,
        clean: bool,
    },
    Playing {
        cart: String,
    },
    Ejecting {
        cart: String,
        t: f32,
    },
    Polaroids {
        cart: String,
    },
    About,
    Doze {
        cart: Option<String>,
    },
    Networks {
        pick: crate::networks::NetPick,
    },
    NetworkScan {
        pick: crate::networks::NetPick,
    },
    NetworkPage {
        ssid: String,
        row: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardFor {
    Password { ssid: String, hidden: bool },
    NetworkName,
}

pub struct NetLegends {
    pub join: [(TexId, u32); 2],
    pub scan: [(TexId, u32); 2],
    pub cancel: [(TexId, u32); 2],
    pub connect: [(TexId, u32); 2],
    pub disconnect: [(TexId, u32); 2],
    pub forget: [(TexId, u32); 2],
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Rack {
    Recents,
    Favorites,
    Of(Platform),
}

impl Rack {
    fn platform(self) -> Option<Platform> {
        match self {
            Rack::Of(p) => Some(p),
            Rack::Recents | Rack::Favorites => None,
        }
    }

    fn key(self) -> String {
        match self {
            Rack::Recents => "recents".into(),
            Rack::Favorites => "favorites".into(),
            Rack::Of(p) => p.dir_name().to_ascii_lowercase(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Rack::Recents => "Recents",
            Rack::Favorites => "Favorites",
            Rack::Of(p) => p.name(),
        }
    }
}

const RECENTS_AT: usize = 0;
const FAVORITES_AT: usize = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
enum UpdateState {
    Checking,
    Available(Vec<Release>, usize),
    Battery(Release),
    UpToDate,
    Downloading(Release, u64),
    Cancelling(Release, u64),
    Closing(Release),
    Failed(UpdateFail),
    Notice(NetNotice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetNotice {
    Password,
    ServerFailed,
}

#[derive(Debug, Clone, Copy)]
pub struct UpdateCart {
    pub cart: TexId,
    pub interior: TexId,
    pub face: TexId,
}

#[derive(Debug, Clone, Copy)]
pub struct UpdateScene {
    pub carts: [UpdateCart; 2],
    pub chip: TexId,
    pub board: TexId,
    pub logo: (TexId, u32, u32),
    pub logo_ground: [f32; 4],
}

pub fn update_cart() -> Cart {
    shelled([0x7b, 0x3f, 0xe4], slot_store::ShellFinish::Clear)
}

pub fn baseos_cart() -> Cart {
    shelled([0x1b, 0x1b, 0x1e], slot_store::ShellFinish::Solid)
}

pub fn cart_for(kind: Kind) -> Cart {
    match kind {
        Kind::Slot => update_cart(),
        Kind::BaseOs => baseos_cart(),
    }
}

fn shelled(colour: [u8; 3], finish: slot_store::ShellFinish) -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: String::new(),
        rom: PathBuf::new(),
        label: None,
        title: String::new(),
        code: String::new(),
        shell: Some(slot_store::ShellChoice {
            outline: slot_store::Outline::Auto,
            colour,
            finish,
        }),
    }
}

const UPDATE_DROP_MS: Millis = 500;
const UPDATE_BOB_MS: f32 = 2800.0;
const UPDATE_BOB_PX: f32 = 7.0;
const UPDATE_SWAY: f32 = 0.8 * std::f32::consts::PI / 180.0;
const UPDATE_OVERSHOOT: f32 = 1.2;
const UPDATE_CHECK_Y: f32 = 170.0;
const UPDATE_PAIR_SCALE: f32 = 0.88;
const UPDATE_PAIR_GAP: f32 = 24.0;
const UPDATE_PAIR_SIDE: f32 = 0.78;
const UPDATE_BACK_MS: f32 = 350.0;
const UPDATE_TITLE_Y: f32 = 30.0;
const TARNISH: [f32; 3] = [0.24, 0.22, 0.16];
const TARNISH_ALPHA: f32 = 0.68;
const UPDATE_HOLD_MS: Millis = 300;
const UPDATE_SWAP_PAUSE_MS: Millis = 1000;
const UPDATE_BLINK_MS: Millis = 400;

pub struct App {
    phase: Phase,
    shelves: Vec<(Rack, Shelf)>,
    shelf_at: usize,
    orphaned: Vec<TexId>,
    play_held: Option<Millis>,
    held_dir: Option<(Btn, Millis, usize)>,
    refusal: Option<Refusal>,
    refused_from: Option<f32>,
    alert_face: Option<TexId>,
    shutdown_faces: Vec<(TexId, u32, u32)>,
    repair_faces: Vec<(TexId, u32, u32)>,
    core_picker: Option<CorePicker>,
    core_board_face: Option<TexId>,
    core_interior_face: Option<TexId>,
    core_lid_face: Option<TexId>,
    core_faces_stem: Option<String>,
    core_socket_faces: Vec<TexId>,
    core_chip_faces: Vec<TexId>,
    core_blank_chip_face: Option<TexId>,
    core_chip_shadow_face: Option<TexId>,
    core_legend_faces: [Vec<(TexId, u32)>; 2],
    game_menu: Option<GameMenu>,
    link_sprites: Option<LinkSprites>,
    link_shot: Option<TexId>,
    link_hardware: LinkKind,
    last_role: LinkRow,
    link_choices: HashMap<String, LinkKind>,
    link_loaded: Option<&'static str>,
    link_reload: Option<(String, &'static str)>,
    reload: Option<Reload>,
    link_menu_faces: Vec<(TexId, u32, u32)>,
    link_linked_face: Option<(TexId, u32, u32)>,
    link_step_faces: Vec<(TexId, u32, u32)>,
    link_fail_faces: Vec<(TexId, u32, u32)>,
    link_legend_faces: Vec<(TexId, u32)>,
    starting: Option<LinkStarting>,
    link_transport: Option<(u16, Box<dyn LinkChannel>)>,
    restarting: bool,
    relaunching: bool,
    repairing: bool,
    repair_prompt: bool,
    reset_prompt: Option<QuickRow>,
    reset_faces: Vec<(TexId, u32, u32)>,
    delete_prompt: bool,
    delete_faces: Vec<(TexId, u32, u32)>,
    act_at: Millis,
    root: Option<PathBuf>,
    state: SlotState,
    left_stick: bool,
    controls: Controls,
    capture: Option<BindCapture>,
    plugged_face: Option<TexId>,
    binding_faces: HashMap<String, [(TexId, u32, u32); 2]>,
    carousel: CarouselKeys,
    shortcuts: ShortcutKeys,
    vol_before: Vec<(u8, bool, Millis)>,
    snapshot: Option<Box<dyn Snapshot>>,
    core: Core,
    colour_pending: Option<bool>,
    blending_pending: Option<Blending>,
    palette_live: bool,
    palette_pending: Option<GbColors>,
    sunlight_manual: Option<u8>,
    link_player: Option<u8>,
    platform: Platform,
    named_core: bool,
    video_mode: VideoMode,
    link: Option<LinkSession>,
    sfx: Option<Sfx>,
    polaroids: Option<Polaroids>,
    pending: Option<(PendingUndo, Millis)>,
    legend_faces: Vec<TexId>,
    undo_face: Option<TexId>,
    clock_faces: Option<ClockFaces>,
    clock_legends: Option<[[(TexId, u32); 6]; 2]>,
    clock_title: Option<(TexId, u32, u32)>,
    quick_menu_faces: Option<QuickMenuFaces>,
    quick_clock_faces: Option<[(TexId, u32, u32); 2]>,
    quick_wallpaper_faces: Option<[(TexId, u32, u32); 2]>,
    quick_network_faces: Option<[(TexId, u32, u32); 2]>,
    quick_palette_faces: Option<[(TexId, u32, u32); 2]>,
    quick_bezel_faces: [Option<[(TexId, u32, u32); 2]>; 3],
    game_bezel: Option<TexId>,
    bezel_column: Vec<u8>,
    in_game_menu: Option<QuickRow>,
    /// 0 while the pause footer is off the bottom, 1 when it sits in the carousel's place.
    footer: f32,
    footer_row: Option<QuickRow>,
    states_from_menu: bool,
    menu_tap: Option<(Millis, Option<QuickRow>)>,
    sticker_face: Option<TexId>,
    ground: Option<TexId>,
    blur: Option<TexId>,
    wallpaper: Option<TexId>,
    random_wallpaper: Option<Option<String>>,
    shelf_wallpaper: Option<String>,
    battery_percent: slot_ui::Printed,
    bolt: Option<TexId>,
    hud_bolt: Option<TexId>,
    slot_faces: Option<(TexId, TexId)>,
    shelf_platform: slot_ui::Printed,
    shelf_title: slot_ui::Printed,
    name_pending: bool,
    slot_letter: Option<char>,
    shelf_named: Option<Millis>,
    shelf_clock: slot_ui::Printed,
    hud: Hud,
    screen: f32,
    blank_s: f32,
    lift: f32,
    sp_strip: Option<TexId>,
    eject_overlay: f32,
    panel: (u32, u32),
    game_ready: bool,
    clock: f64,
    power: Option<Power>,
    dozed_at: Millis,
    woke_on_press: bool,
    autosave_at: Millis,
    resume_at: Millis,
    loading: Option<LoadingMark>,
    marked: bool,
    playing_since: Option<Millis>,
    pending_save: Option<std::thread::JoinHandle<()>>,
    card_writes: Arc<crate::card_log::CardWrites>,
    resume_unread: bool,
    card_read_only: bool,
    card_check_at: Millis,
    battery_at: Millis,
    charge_at: Millis,
    headphones: bool,
    headphones_at: Millis,
    battery: Option<Battery>,
    last_led: Option<LedState>,
    powering_off: bool,
    radio: Box<dyn RadioJobs>,
    update_jobs: Box<dyn UpdateJobs>,
    art_jobs: Box<dyn ArtJobs>,
    fetch_art: Option<FetchArt>,
    wifi_held: bool,
    wifi_boot_blocked: bool,
    wifi_probe: Option<(PathBuf, Millis)>,
    core_stale: bool,
    studio_saving: Option<String>,
    studio_write: Option<(std::thread::JoinHandle<bool>, Box<crate::studio::Studio>)>,
    studio_hex: Option<(String, crate::studio_view::Style)>,
    wifi_wanted_at: Option<Millis>,
    wifi: Option<(crate::wifi::Wifi, std::time::Instant, Millis)>,
    net_time: crate::net_time::NetTime,
    joined_hidden: Option<(String, bool)>,
    wifi_bars: u8,
    wifi_faces: Option<[[slot_ui::NetFace; 4]; 2]>,
    wifi_sampled: Millis,
    wifi_scanning: bool,
    keyboard: Option<(slot_ui::Keyboard, KeyboardFor)>,
    keyboard_tex: Option<slot_ui::KeyboardTex>,
    net_lines: Vec<slot_ui::NetLine>,
    net_message: Option<slot_ui::NetFace>,
    net_icons: Option<slot_ui::NetIcons>,
    net_legends: Option<NetLegends>,
    forget_prompt: bool,
    forget_faces: Vec<(TexId, u32, u32)>,
    forgotten: Option<String>,
    studio_art: Option<crate::art::jobs::StudioArt>,
    studio_fetched: bool,
    studio_stale: usize,
    studio_session: u64,
    studio_tex: Option<StudioTex>,
    studio_chips: Option<(TexId, u32, u32)>,
    studio_text: std::collections::HashMap<(String, crate::studio_view::Style), (TexId, u32, u32)>,
    studio_legends: std::collections::HashMap<(crate::studio::Legend, bool, bool), (TexId, u32)>,
    studio_opened: Millis,
    studio_closing: Option<Millis>,
    studio_shelf_face: Option<TexId>,
    studio_shelf_under: [Option<TexId>; 2],
    studio_from: [f32; 4],
    studio_liner: Option<TexId>,
    studio_disc: Option<TexId>,
    studio_printer: Option<TexId>,
    studio_scene: Option<crate::studio::ApplyScene>,
    studio_slips: Vec<crate::studio::Slip>,
    studio_prints: u32,
    studio_shuffled: f64,
    studio_picked: Option<(crate::studio::LabelKind, f64)>,
    studio_repeat: (Option<Btn>, Millis, u32),
    studio_applied: bool,
    studio: Option<crate::studio::Studio>,
    server: Option<ServerSession>,
    server_net: ServerNet,
    print_cues: Vec<Cue>,
    rescanned: bool,
    stale_labels: Vec<Cart>,
    fetch_landed: Vec<ArtLanded>,
    fetch_relabeled: usize,
    art_pending: usize,
    fetch_faces_required: bool,
    fetch_tex: Vec<(Platform, String, FetchTex)>,
    fetch_example_tex: Option<[(TexId, u32, u32); 2]>,
    fetch_legends: Option<[[(TexId, u32); 5]; 2]>,
    fetch_text: Vec<(TexId, u32, u32)>,
    fetch_paper: Option<TexId>,
    update: Option<UpdateState>,
    update_busy: bool,
    update_at: Millis,
    update_sounded: bool,
    update_faces: Vec<(TexId, u32, u32)>,
    update_legends: Option<[(TexId, u32); 3]>,
    update_scene: Option<UpdateScene>,
    update_picker: Option<CorePicker>,
    update_seat_at: Option<Millis>,
    update_offers: Vec<Release>,
    update_back: bool,
    update_sockets: Vec<TexId>,
    update_chips: Vec<TexId>,
    state_writer: crate::state_writer::StateWriter,
    shot_writer: crate::shot_writer::ShotWriter,
    shot_wanted: bool,
    last_shot: Option<i64>,
    shot_listing: Option<std::sync::mpsc::Receiver<Vec<ShotEntry>>>,
    has_states: bool,
    has_shots: bool,
    shot_viewer: Option<ShotViewer>,
    delete_shot_faces: Vec<(TexId, u32, u32)>,
    shot_hint_faces: Vec<TexId>,
    shot_opens: u64,
    shot_tick_face: Option<(TexId, u32, u32)>,
    delete_shot_text: Option<[(TexId, u32, u32); 2]>,
    has_guide: bool,
    guide: Option<(Guide, PathBuf)>,
    guide_hint_faces: Vec<TexId>,
    guide_help_face: Option<((TexId, u32, u32), TexId)>,
}

const FACE_AHEAD: i32 = 8;
const LETTER_REPEAT_MS: Option<Millis> = None;
const FACE_KEEP: i32 = 12;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FaceWant {
    pub shelf: usize,
    pub index: usize,
    pub on_screen: bool,
}

fn shelves_of(carts: Vec<Cart>) -> Vec<(Rack, Shelf)> {
    let mut rows: Vec<(Rack, Vec<Cart>)> = [Rack::Recents, Rack::Favorites]
        .into_iter()
        .chain(Platform::ALL.iter().map(|p| Rack::Of(*p)))
        .map(|r| (r, Vec::new()))
        .collect();
    for cart in carts {
        if let Some((_, row)) = rows.iter_mut().find(|(r, _)| *r == Rack::Of(cart.platform)) {
            row.push(cart);
        }
    }
    rows.into_iter()
        .map(|(rack, carts)| (rack, Shelf::new(carts)))
        .collect()
}

fn wallpaper_of_rack(rack: Rack, files: &[String]) -> Option<String> {
    std::iter::once(rack)
        .chain(rack.platform().map(|p| Rack::Of(p.hardware())))
        .find_map(|rack| {
            files
                .iter()
                .find(|file| {
                    file.rsplit_once('.')
                        .is_some_and(|(stem, _)| stem.eq_ignore_ascii_case(&rack.key()))
                })
                .cloned()
        })
}

pub(crate) fn shift_down(draws: &mut [Draw], dy: f32) {
    if dy == 0.0 {
        return;
    }
    for draw in draws {
        match draw {
            Draw::Rect { y, .. }
            | Draw::Tex { y, .. }
            | Draw::Part { y, .. }
            | Draw::Turned { y, .. } => *y += dy,
            Draw::Quad { corners, .. } => {
                for c in corners {
                    c[1] += dy;
                }
            }
            Draw::Paper { pivot, .. } => pivot[1] += dy,
            Draw::Game | Draw::Shot { .. } | Draw::Capture => {}
        }
    }
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        let shelves = shelves_of(carts);
        let shelf_at = shelves
            .iter()
            .position(|(_, s)| !s.carts.is_empty())
            .unwrap_or(0);
        let state_writer = crate::state_writer::StateWriter::spawn();
        let card_writes = state_writer.card_writes();
        App {
            radio: radio_jobs(),
            update_jobs: no_update_jobs(),
            art_jobs: no_art_jobs(),
            fetch_art: None,
            wifi_held: false,
            wifi_boot_blocked: false,
            wifi_probe: None,
            core_stale: false,
            studio_saving: None,
            studio_write: None,
            studio_hex: None,
            wifi_wanted_at: None,
            wifi: None,
            net_time: crate::net_time::NetTime::new(std::sync::Arc::new(crate::sntp::fetch)),
            joined_hidden: None,
            wifi_bars: 3,
            wifi_faces: None,
            wifi_sampled: 0,
            wifi_scanning: false,
            keyboard: None,
            keyboard_tex: None,
            net_lines: Vec::new(),
            net_message: None,
            net_icons: None,
            net_legends: None,
            forget_prompt: false,
            forget_faces: Vec::new(),
            forgotten: None,
            studio_art: None,
            studio_fetched: false,
            studio_stale: 0,
            studio_session: 0,
            studio_tex: None,
            studio_chips: None,
            studio_text: Default::default(),
            studio_legends: Default::default(),
            studio_opened: 0,
            studio_closing: None,
            studio_shelf_face: None,
            studio_shelf_under: [None; 2],
            studio_from: [0.0; 4],
            studio_liner: None,
            studio_disc: None,
            studio_printer: None,
            studio_scene: None,
            studio_slips: Vec::new(),
            studio_prints: 0,
            studio_shuffled: f64::NEG_INFINITY,
            studio_picked: None,
            studio_repeat: (None, 0, 0),
            studio_applied: false,
            studio: None,
            server: None,
            server_net: ServerNet::default(),
            print_cues: Vec::new(),
            rescanned: false,
            stale_labels: Vec::new(),
            fetch_landed: Vec::new(),
            fetch_relabeled: 0,
            art_pending: 0,
            fetch_faces_required: false,
            fetch_tex: Vec::new(),
            fetch_example_tex: None,
            fetch_legends: None,
            fetch_text: Vec::new(),
            fetch_paper: None,
            update: None,
            update_busy: false,
            update_at: 0,
            update_sounded: false,
            update_faces: Vec::new(),
            update_legends: None,
            update_scene: None,
            update_picker: None,
            update_seat_at: None,
            update_offers: Vec::new(),
            update_back: false,
            update_sockets: Vec::new(),
            update_chips: Vec::new(),
            state_writer,
            shot_writer: crate::shot_writer::ShotWriter::spawn(card_writes.clone()),
            shot_wanted: false,
            last_shot: None,
            shot_listing: None,
            has_states: false,
            has_shots: false,
            shot_viewer: None,
            delete_shot_faces: Vec::new(),
            shot_hint_faces: Vec::new(),
            has_guide: false,
            guide: None,
            guide_hint_faces: Vec::new(),
            guide_help_face: None,
            shot_opens: 0,
            shot_tick_face: None,
            delete_shot_text: None,
            phase: Phase::Shelf,
            shelves,
            shelf_at,
            orphaned: Vec::new(),
            play_held: None,
            held_dir: None,
            refusal: None,
            refused_from: None,
            alert_face: None,
            shutdown_faces: Vec::new(),
            repair_faces: Vec::new(),
            core_picker: None,
            core_board_face: None,
            core_interior_face: None,
            core_lid_face: None,
            core_faces_stem: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_chip_shadow_face: None,
            core_legend_faces: [Vec::new(), Vec::new()],
            game_menu: None,
            link_sprites: None,
            link_shot: None,
            link_hardware: LinkKind::Cable,
            last_role: LinkRow::Host,
            link_choices: HashMap::new(),
            link_loaded: None,
            link_reload: None,
            reload: None,
            link_menu_faces: Vec::new(),
            link_linked_face: None,
            link_step_faces: Vec::new(),
            link_fail_faces: Vec::new(),
            link_legend_faces: Vec::new(),
            starting: None,
            link_transport: None,
            restarting: false,
            relaunching: false,
            repairing: false,
            repair_prompt: false,
            reset_prompt: None,
            reset_faces: Vec::new(),
            delete_prompt: false,
            delete_faces: Vec::new(),
            act_at: 0,
            root: None,
            state: SlotState::default(),
            left_stick: false,
            controls: Controls::default(),
            capture: None,
            plugged_face: None,
            binding_faces: HashMap::new(),
            carousel: CarouselKeys::default(),
            shortcuts: ShortcutKeys::default(),
            vol_before: Vec::new(),
            snapshot: None,
            core: Core::default(),
            colour_pending: None,
            blending_pending: None,
            palette_live: false,
            palette_pending: None,
            sunlight_manual: None,
            link_player: None,
            platform: Platform::default(),
            named_core: false,
            video_mode: VideoMode::default(),
            link: None,
            sfx: None,
            polaroids: None,
            pending: None,
            legend_faces: Vec::new(),
            undo_face: None,
            clock_faces: None,
            clock_legends: None,
            clock_title: None,
            quick_menu_faces: None,
            quick_clock_faces: None,
            quick_wallpaper_faces: None,
            quick_network_faces: None,
            quick_palette_faces: None,
            quick_bezel_faces: [None, None, None],
            game_bezel: None,
            bezel_column: Vec::new(),
            in_game_menu: None,
            footer: 0.0,
            footer_row: None,
            states_from_menu: false,
            menu_tap: None,
            sticker_face: None,
            ground: None,
            blur: None,
            wallpaper: None,
            random_wallpaper: None,
            shelf_wallpaper: None,
            battery_percent: slot_ui::Printed::default(),
            bolt: None,
            hud_bolt: None,
            slot_faces: None,
            shelf_platform: slot_ui::Printed::default(),
            shelf_title: slot_ui::Printed::default(),
            name_pending: false,
            slot_letter: None,
            shelf_named: None,
            shelf_clock: slot_ui::Printed::default(),
            hud: Hud::new(),
            screen: 0.0,
            blank_s: 0.0,
            lift: 1.0,
            sp_strip: None,
            eject_overlay: 0.0,
            panel: (0, 0),
            game_ready: false,
            clock: 0.0,
            power: None,
            dozed_at: 0,
            woke_on_press: false,
            autosave_at: AUTOSAVE_MS,
            resume_at: RESUME_MS,
            loading: None,
            marked: false,
            playing_since: None,
            pending_save: None,
            card_writes,
            resume_unread: false,
            card_read_only: false,
            card_check_at: 0,
            battery_at: BATTERY_POLL_MS,
            charge_at: CHARGE_POLL_MS,
            headphones: false,
            headphones_at: 0,
            battery: None,
            last_led: None,
            powering_off: false,
        }
    }

    pub fn boot(root: &Path) -> Self {
        Self::boot_marked(root, LoadingMark::from_env())
    }

    pub fn boot_marked(root: &Path, loading: Option<LoadingMark>) -> Self {
        crate::root::ensure(root);
        slot_store::header_index::load(root);
        crate::boot_time::mark("root");
        let carts = scan(root).unwrap_or_default();
        crate::boot_time::mark("scan");
        let mut app = App::new(carts);
        app.root = Some(root.to_path_buf());
        app.loading = loading;
        app.state = read_slot_state(root);
        app.carousel = read_carousel(root);
        app.shortcuts = read_shortcuts(root);
        app.controls = read_controls(root);
        app.restock_recents();
        app.restock_favorites();
        if let Some((at, i)) = app.remembered_seat() {
            app.shelf_at = at;
            app.shelves[at].1.select(i);
        } else if !app.shelves[RECENTS_AT].1.carts.is_empty() {
            app.shelf_at = RECENTS_AT;
        }
        if app.state.clock_set {
            app.start();
        } else {
            app.phase = clock_screen(system_secs(), &app.state, app.has_saved(), false);
        }
        app
    }

    fn start(&mut self) {
        if let Some(stem) = self.loading.as_ref().and_then(LoadingMark::left) {
            return self.start_after_a_crash(&stem);
        }
        let seated = if self.single_cart() {
            Some((self.shelf_at, 0))
        } else {
            let stem = self.state.cart.clone();
            let platform = self.state.cart_platform;
            stem.and_then(|stem| {
                let shelf = &self.shelf().carts;
                let here = shelf
                    .iter()
                    .position(|c| c.stem == stem && platform.is_none_or(|p| c.platform == p));
                here.map(|i| (self.shelf_at, i))
                    .or_else(|| self.seat_of(&stem, platform))
            })
        };
        self.phase = Phase::Shelf;
        match seated {
            Some((at, i)) => {
                self.shelf_at = at;
                self.shelf_mut().select(i);
                self.insert(!self.state.auto_load);
                if let Phase::Inserting { resumed, t, .. } = &mut self.phase {
                    *resumed = true;
                    *t = INSERT_S;
                }
            }
            None => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.name_pending = true;
            }
        }
    }

    fn start_after_a_crash(&mut self, stem: &str) {
        eprintln!("slot: {stem} took slot down while it was loading, starting on the shelf");
        if let Some(mark) = &self.loading {
            mark.clear();
        }
        self.phase = Phase::Shelf;
        let platform = self
            .state
            .cart_platform
            .or_else(|| self.carts().find(|c| c.stem == stem).map(|c| c.platform));
        self.state.cart = None;
        self.state.cart_platform = None;
        self.name_pending = true;
        self.persist_soon();
        let (Some(root), Some(platform)) = (&self.root, platform) else {
            return;
        };
        let core = slot_store::core_for_platform(root, stem, platform);
        let ring = StateRing::new(root, platform, core, stem);
        match ring.retire_resume(&format_stamp(self.wall_secs())) {
            Ok(Some(to)) => eprintln!("slot: resume: moved {stem}'s state to {}", to.display()),
            Ok(None) => {}
            Err(e) => eprintln!("slot: resume: could not move {stem}'s state aside: {e}"),
        }
    }

    fn settle_loading_mark(&mut self) {
        if !self.marked {
            return;
        }
        let settled = match self.phase {
            Phase::Playing { .. } => {
                let since = *self.playing_since.get_or_insert(self.now());
                self.now().saturating_sub(since) >= SETTLED_MS
            }
            Phase::Shelf => true,
            _ => false,
        };
        if settled {
            self.clear_loading_mark();
        }
    }

    pub fn clear_loading_mark(&mut self) {
        if let Some(mark) = &self.loading {
            mark.clear();
        }
        self.marked = false;
        self.playing_since = None;
    }

    fn shelf(&self) -> &Shelf {
        &self.shelves[self.shelf_at].1
    }

    fn shelf_mut(&mut self) -> &mut Shelf {
        &mut self.shelves[self.shelf_at].1
    }

    fn switch_shelf(&mut self, by: i32) {
        let Some(to) = self.next_shelf(by) else {
            return;
        };
        self.shelf_mut().release_hold();
        self.play_held = None;
        self.shelf_at = to;
        self.shelf_platform = slot_ui::Printed::default();
        self.slot_letter = None;
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn track_held_dir(&mut self, action: Action) {
        match action {
            Action::GbaDown(b @ (Btn::Up | Btn::Down | Btn::Left | Btn::Right)) => {
                if self.held_dir.map(|(held, ..)| held) != Some(b) {
                    self.held_dir = Some((b, self.now() + REPEAT_DELAY_MS, 0));
                }
            }
            Action::GbaDown(b @ (Btn::A | Btn::B | Btn::L1 | Btn::R1)) if self.keyboard.is_some() => {
                if self.held_dir.map(|(held, ..)| held) != Some(b) {
                    self.held_dir = Some((b, self.now() + REPEAT_DELAY_MS, 0));
                }
            }
            Action::GbaUp(b) if self.held_dir.is_some_and(|(held, ..)| held == b) => {
                self.held_dir = None;
            }
            _ => {}
        }
    }

    fn repeat_held_dir(&mut self) {
        let Some((btn, due, fired)) = self.held_dir else {
            return;
        };
        if self.now() < due {
            return;
        }
        if !self.repeats(btn) {
            self.held_dir = None;
            return;
        }
        let gap = match LETTER_REPEAT_MS {
            Some(ms) if self.jumps_letters(btn) => ms,
            _ => repeat_gap(fired),
        };
        self.held_dir = Some((btn, self.now() + gap, fired + 1));
        self.apply(Action::GbaDown(btn));
    }

    fn jumps_letters(&self, btn: Btn) -> bool {
        matches!(btn, Btn::Up | Btn::Down)
            && matches!(self.phase, Phase::Shelf)
            && self.core_picker.is_none()
            && self.studio.is_none()
            && self.keyboard.is_none()
            && self.game_menu.is_none()
    }

    fn repeats(&self, btn: Btn) -> bool {
        let vertical = matches!(btn, Btn::Up | Btn::Down);
        let direction = vertical || matches!(btn, Btn::Left | Btn::Right);
        if let Some((kb, _)) = &self.keyboard {
            return !kb.help() && (btn != Btn::A || kb.on_delete());
        }
        if !direction {
            return false;
        }
        if self.game_menu.is_some() {
            return false;
        }
        if self.studio.is_some() {
            return true;
        }
        match self.phase {
            Phase::SetClock { .. } => true,
            Phase::QuickMenu { .. } => vertical,
            Phase::Shelf => vertical && self.core_picker.is_none(),
            Phase::Polaroids { .. } if self.guide.is_some() => true,
            Phase::Polaroids { .. } => match self.shot_viewer.as_ref().map(ShotViewer::mode) {
                Some(ShotMode::Grid) => true,
                Some(ShotMode::Zoomed) => false,
                _ => !vertical,
            },
            Phase::Playing { .. } => vertical && self.in_game_menu.is_some(),
            _ => false,
        }
    }

    fn jump_letter(&mut self, dir: i32) {
        let before = self.shelf().index;
        match dir > 0 {
            true => self.shelf_mut().jump_next_letter(),
            false => self.shelf_mut().jump_prev_letter(),
        }
        if self.shelf().index == before {
            return;
        }
        let Some(letter) = self.selected_stem().map(slot_store::initial) else {
            return;
        };
        if self.slot_letter != Some(letter) {
            self.shelf_platform = slot_ui::Printed::default();
        }
        self.slot_letter = Some(letter);
        self.shelf_named = None;
        self.name_pending = true;
    }

    fn slot_name_alpha(&self) -> f32 {
        let Some(at) = self.shelf_named else {
            return 0.0;
        };
        let t = self.now().saturating_sub(at);
        let level = if t < SLOT_NAME_IN_MS {
            t as f32 / SLOT_NAME_IN_MS as f32
        } else if t < SLOT_NAME_IN_MS + SLOT_NAME_HOLD_MS {
            1.0
        } else {
            let out = t - SLOT_NAME_IN_MS - SLOT_NAME_HOLD_MS;
            1.0 - (out as f32 / SLOT_NAME_OUT_MS as f32).min(1.0)
        };
        SLOT_NAME_ALPHA * ease(level)
    }

    fn next_shelf(&self, by: i32) -> Option<usize> {
        let n = self.shelves.len() as i32;
        (1..n)
            .map(|step| (self.shelf_at as i32 + by * step).rem_euclid(n) as usize)
            .find(|at| !self.shelves[*at].1.carts.is_empty())
    }

    fn remembered_seat(&self) -> Option<(usize, usize)> {
        let key = self.state.shelf.as_deref()?;
        let cart = self.state.shelf_cart.as_ref()?;
        let at = self.shelves.iter().position(|(r, _)| r.key() == key)?;
        let i = self.shelves[at]
            .1
            .carts
            .iter()
            .position(|c| c.platform == cart.platform && c.stem == cart.stem)?;
        Some((at, i))
    }

    fn remember_shelf(&mut self) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        self.state.shelf_cart = Some(Recent {
            platform: cart.platform,
            stem: cart.stem.clone(),
        });
        self.state.shelf = Some(self.shelves[self.shelf_at].0.key());
        self.persist();
    }

    fn seat_of(&self, stem: &str, platform: Option<Platform>) -> Option<(usize, usize)> {
        self.shelves
            .iter()
            .enumerate()
            .filter(|(_, (r, _))| {
                platform.is_none_or(|want| r.platform().is_none_or(|p| p == want))
            })
            .find_map(|(at, (_, shelf))| {
                shelf
                    .carts
                    .iter()
                    .position(|c| c.stem == stem && platform.is_none_or(|p| c.platform == p))
                    .map(|i| (at, i))
            })
    }

    pub fn confirm_clock(&mut self) {
        let Phase::SetClock {
            picker,
            seed,
            from_menu,
        } = &self.phase
        else {
            return;
        };
        let network_time = picker.network_time();
        let moved = match network_time {
            Some(true) => 0,
            _ => picker.secs() - *seed,
        };
        let (zone, clock_24h, from_menu) = (picker.zone(), picker.clock_24h(), *from_menu);
        let utc = self.utc_secs() + moved;
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
        }
        self.state.zone = zone;
        self.state.clock_24h = clock_24h;
        self.state.network_time = network_time.unwrap_or(self.state.network_time);
        self.state.clock_set = true;
        self.persist();
        if from_menu {
            self.phase = Phase::QuickMenu {
                row: QuickRow::DateTime,
            };
        } else {
            self.start();
        }
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }

    pub fn wall_secs(&self) -> i64 {
        self.state.zone.local(self.utc_secs())
    }

    pub fn clock_24h(&self) -> bool {
        self.state.clock_24h
    }

    fn utc_secs(&self) -> i64 {
        self.power.as_ref().map_or_else(system_secs, |p| p.now())
    }

    pub fn picker(&self) -> Option<&ClockPicker> {
        match &self.phase {
            Phase::SetClock { picker, .. } => Some(picker),
            _ => None,
        }
    }

    pub fn quick_menu(&self) -> Option<QuickRow> {
        match self.phase {
            Phase::QuickMenu { row } => Some(row),
            _ => None,
        }
    }

    pub fn in_game_menu(&self) -> Option<QuickRow> {
        self.in_game_menu
    }

    pub fn in_game_hidden(&self) -> Vec<QuickRow> {
        let no_bezel = self.playing_platform().hardware() == Platform::Gba && !self.framed_panel();
        let mut hidden = Vec::new();
        if !self.palette_live {
            hidden.push(QuickRow::Palette);
        }
        if no_bezel {
            hidden.push(QuickRow::Bezel);
        }
        if self.scale_choices(self.playing_platform()).len() < 2 {
            hidden.push(QuickRow::Scale);
        }
        if self.stretch_choices(self.playing_platform()).len() < 2 {
            hidden.push(QuickRow::Stretch);
        }
        if self.single_cart() {
            hidden.push(QuickRow::ExitGame);
        }
        if self.playing_platform().hardware() != Platform::Gba {
            hidden.extend([QuickRow::GameL, QuickRow::GameR]);
        }
        if !self.left_stick {
            hidden.push(QuickRow::LeftStick);
        }
        if !self.state.turbo {
            hidden.extend([QuickRow::GameTurboA, QuickRow::GameTurboB]);
        }
        if !self.has_states || self.link_active() {
            hidden.push(QuickRow::States);
        }
        if !self.has_shots || self.link_active() {
            hidden.push(QuickRow::Screenshots);
        }
        if !self.has_guide || self.link_active() {
            hidden.push(QuickRow::Guide);
        }
        let offer = match self.undo_available(self.now()) {
            true => self.pending.as_ref().map(|(what, _)| what),
            false => None,
        };
        if !matches!(offer, Some(PendingUndo::Save { .. })) {
            hidden.push(QuickRow::UndoSave);
        }
        if !matches!(offer, Some(PendingUndo::Load { .. })) || !self.may_load_state() {
            hidden.push(QuickRow::UndoLoad);
        }
        if !matches!(offer, Some(PendingUndo::Reset { .. })) || !self.may_load_state() {
            hidden.push(QuickRow::UndoReset);
        }
        hidden
    }

    fn refresh_albums(&mut self) {
        self.has_states = !self.entries().is_empty();
        let removing = self.shot_writer.removing();
        self.has_shots = self.shot_dir().is_some_and(|s| s.any(&removing));
        self.has_guide = self.guide_file().is_some();
    }

    fn list_shots_later(&mut self) {
        let (Some(dir), (tx, rx)) = (self.shot_dir(), std::sync::mpsc::channel()) else {
            return;
        };
        let removing = self.shot_writer.removing();
        let spawned = std::thread::Builder::new()
            .name("slot-shot-list".into())
            .spawn(move || {
                let at = std::time::Instant::now();
                let mut shots = dir.list().unwrap_or_default();
                shots.retain(|e| !removing.contains(&e.path));
                eprintln!(
                    "slot: shots: listed {} in {} ms",
                    shots.len(),
                    at.elapsed().as_millis()
                );
                let _ = tx.send(shots);
            });
        match spawned {
            Ok(_) => self.shot_listing = Some(rx),
            Err(e) => eprintln!("slot: shots: no list thread: {e}"),
        }
    }

    fn take_shot_list(&mut self) {
        let Some(rx) = &self.shot_listing else {
            return;
        };
        let shots = match rx.try_recv() {
            Ok(shots) => shots,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Vec::new(),
        };
        self.shot_listing = None;
        let Some(v) = &mut self.shot_viewer else {
            return;
        };
        if shots.is_empty() {
            self.has_shots = false;
            return self.close_polaroids();
        }
        v.fill(shots);
        self.push_hint_faces();
    }

    fn shot_dir(&self) -> Option<ShotDir> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        Some(ShotDir::new(root, self.platform, cart))
    }

    pub fn gb_colors_label(&self) -> &'static str {
        self.state.gb_colors().label()
    }

    pub fn set_quick_palette_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_palette_faces = Some([dim, lit]);
    }

    fn playing_row(&self, row: QuickRow) -> QuickRow {
        match (row, self.playing_platform().hardware()) {
            (QuickRow::Shader, Platform::Gba) => QuickRow::GbaShader,
            (QuickRow::Shader, Platform::Gb) => QuickRow::GbShader,
            (QuickRow::Shader, Platform::Gbc) => QuickRow::GbcShader,
            (QuickRow::ColourCorrection, Platform::Gba) => QuickRow::GbaColourCorrection,
            (QuickRow::ColourCorrection, Platform::Gb) => QuickRow::GbColourCorrection,
            (QuickRow::ColourCorrection, Platform::Gbc) => QuickRow::GbcColourCorrection,
            (QuickRow::Bezel, Platform::Gba) => QuickRow::GbaBezel,
            (QuickRow::Bezel, Platform::Gb) => QuickRow::GbBezel,
            (QuickRow::Bezel, Platform::Gbc) => QuickRow::GbcBezel,
            (QuickRow::Scale, Platform::Gba) => QuickRow::GbaScale,
            (QuickRow::Scale, Platform::Gb) => QuickRow::GbScale,
            (QuickRow::Scale, Platform::Gbc) => QuickRow::GbcScale,
            (QuickRow::Stretch, Platform::Gb) => QuickRow::GbStretch,
            (QuickRow::Stretch, Platform::Gbc) => QuickRow::GbcStretch,
            (QuickRow::PlayBrightness, _) => QuickRow::Brightness,
            (QuickRow::PlayBlueLight, _) => QuickRow::BlueLight,
            (QuickRow::PlaySaturation, _) => QuickRow::Saturation,
            (QuickRow::PlayContrast, _) => QuickRow::Contrast,
            (row, _) => row,
        }
    }

    pub fn quick_value(&self, row: QuickRow) -> Option<QuickValue> {
        match row {
            QuickRow::Shader
            | QuickRow::ColourCorrection
            | QuickRow::PlayBrightness
            | QuickRow::PlayBlueLight
            | QuickRow::PlaySaturation
            | QuickRow::PlayContrast
            | QuickRow::Scale => self.quick_value(self.playing_row(row)),
            QuickRow::Stretch => match self.playing_row(row) {
                QuickRow::Stretch => None,
                row => self.quick_value(row),
            },
            QuickRow::GbaScale => Some(scale_value(self.resolved_scale(Platform::Gba))),
            QuickRow::GbScale => Some(scale_value(self.resolved_scale(Platform::Gb))),
            QuickRow::GbcScale => Some(scale_value(self.resolved_scale(Platform::Gbc))),
            QuickRow::GbStretch => Some(stretch_value(self.resolved_stretch(Platform::Gb))),
            QuickRow::GbcStretch => Some(stretch_value(self.resolved_stretch(Platform::Gbc))),
            QuickRow::GbaBlending => Some(blending_value(self.state.blending_gba)),
            QuickRow::GbcBlending => Some(blending_value(self.state.blending_gbc)),
            QuickRow::GbBlending => Some(blending_value(self.state.blending_gb)),
            QuickRow::Blending => Some(blending_value(self.blending_in_play())),
            QuickRow::Brightness => Some(QuickValue::level(self.state.brightness)),
            QuickRow::BlueLight => Some(match self.state.blue_light {
                0 => QuickValue::Off,
                n => QuickValue::level(n),
            }),
            QuickRow::Saturation => Some(match self.state.saturation {
                0 => QuickValue::Off,
                n => QuickValue::level(n),
            }),
            QuickRow::Contrast => Some(match self.state.contrast {
                0 => QuickValue::Off,
                n => QuickValue::level(n),
            }),
            QuickRow::ScreenGeneral => None,
            QuickRow::FastForward => QuickValue::speed(self.state.ff_speed),
            QuickRow::FastForwardSound => Some(QuickValue::flag(self.state.ff_sound)),
            QuickRow::GbaColourCorrection => {
                Some(QuickValue::flag(self.state.colour_correction_gba))
            }
            QuickRow::GbColourCorrection => Some(QuickValue::flag(self.state.colour_correction_gb)),
            QuickRow::GbcColourCorrection => {
                Some(QuickValue::flag(self.state.colour_correction_gbc))
            }
            QuickRow::GbaShader => Some(shader_value(self.gba_shader())),
            QuickRow::GbShader => Some(shader_value(self.state.shader_gb)),
            QuickRow::GbcShader => Some(shader_value(self.state.shader_gbc)),
            QuickRow::Rumble => Some(QuickValue::flag(self.state.rumble)),
            QuickRow::LeftStick => Some(QuickValue::flag(self.state.left_stick_dpad)),
            QuickRow::AutoLoad => Some(QuickValue::flag(self.state.auto_load)),
            QuickRow::EjectSave => Some(QuickValue::flag(self.state.eject_save)),
            QuickRow::Turbo => Some(QuickValue::flag(self.state.turbo)),
            QuickRow::Rewind => Some(QuickValue::flag(self.state.rewind)),
            QuickRow::Recents => Some(if self.state.show_recents {
                QuickValue::Show
            } else {
                QuickValue::Hide
            }),
            QuickRow::ShowTitles => Some(if self.state.show_titles {
                QuickValue::Show
            } else {
                QuickValue::Hide
            }),
            QuickRow::WallpaperTint => Some(match self.state.wallpaper_tint {
                WallpaperTint::Off => QuickValue::Off,
                WallpaperTint::Low => QuickValue::Low,
                WallpaperTint::Medium => QuickValue::Medium,
                WallpaperTint::High => QuickValue::High,
            }),
            QuickRow::LidOff => QuickValue::LID_OFF
                .into_iter()
                .find(|v| v.minutes() == Some(self.lid_off_min())),
            QuickRow::Theme => Some(if self.light() {
                QuickValue::Light
            } else {
                QuickValue::Dark
            }),
            QuickRow::WifiPower => Some(QuickValue::flag(!self.state.wifi_off)),
            QuickRow::DateTime
            | QuickRow::FetchArt
            | QuickRow::Network
            | QuickRow::Networks
            | QuickRow::CheckUpdates
            | QuickRow::Server
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::Wallpaper
            | QuickRow::Resume
            | QuickRow::UndoSave
            | QuickRow::UndoLoad
            | QuickRow::UndoReset
            | QuickRow::States
            | QuickRow::Screenshots
            | QuickRow::Guide
            | QuickRow::Settings
            | QuickRow::ExitGame
            | QuickRow::PlayScreen
            | QuickRow::PlayGameplay
            | QuickRow::Palette
            | QuickRow::GbPalette
            | QuickRow::ScreenGba
            | QuickRow::ScreenGbc
            | QuickRow::ScreenGb
            | QuickRow::GbBezel
            | QuickRow::GbcBezel
            | QuickRow::GbaBezel
            | QuickRow::Bezel
            | QuickRow::General
            | QuickRow::ShelfPrev
            | QuickRow::ShelfNext
            | QuickRow::ShelfFavorite
            | QuickRow::ShelfForget
            | QuickRow::PlayControls
            | QuickRow::Shortcuts
            | QuickRow::GameUp
            | QuickRow::GameDown
            | QuickRow::GameLeft
            | QuickRow::GameRight
            | QuickRow::GameA
            | QuickRow::GameB
            | QuickRow::GameSelect
            | QuickRow::GameStart
            | QuickRow::GameL
            | QuickRow::GameR
            | QuickRow::GameTurboA
            | QuickRow::GameTurboB
            | QuickRow::ScSave
            | QuickRow::ScLoad
            | QuickRow::ScScreenshot
            | QuickRow::ScBrightUp
            | QuickRow::ScBrightDown
            | QuickRow::ScBlueDown
            | QuickRow::ScBlueUp
            | QuickRow::ScColour
            | QuickRow::ScPaletteBack
            | QuickRow::ScPaletteNext
            | QuickRow::ScShaderNext
            | QuickRow::ScShaderBack
            | QuickRow::ScFastForward
            | QuickRow::ScFastForwardToggle
            | QuickRow::ScRewind
            | QuickRow::ScSunlightUp
            | QuickRow::ScSunlightDown
            | QuickRow::ScReset
            | QuickRow::ScGuide
            | QuickRow::Controls
            | QuickRow::ControlsShelf
            | QuickRow::ControlsInGame => None,
        }
    }

    pub fn set_quick_menu_faces(&mut self, faces: QuickMenuFaces) {
        self.quick_menu_faces = Some(faces);
    }

    pub fn set_quick_clock_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_clock_faces = Some([dim, lit]);
    }

    pub fn set_quick_wallpaper_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_wallpaper_faces = Some([dim, lit]);
    }

    pub fn set_quick_network_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_network_faces = Some([dim, lit]);
    }

    pub fn set_quick_bezel_faces(
        &mut self,
        platform: Platform,
        dim: (TexId, u32, u32),
        lit: (TexId, u32, u32),
    ) {
        self.quick_bezel_faces[bezel_slot(platform)] = Some([dim, lit]);
    }

    pub fn bezel_files(&self, platform: Platform) -> Vec<String> {
        match platform.hardware() == Platform::Gba && !self.framed_panel() {
            true => Vec::new(),
            false => self
                .root()
                .map(|root| crate::bezel::list(root, platform))
                .unwrap_or_default(),
        }
    }

    fn bezel_choices(&self, platform: Platform) -> Vec<BezelPick> {
        let mut choices = vec![BezelPick::None];
        if platform.hardware() != Platform::Gba {
            choices.push(BezelPick::Default);
        }
        if self.framed_panel() {
            choices.push(BezelPick::Ags102);
        }
        let scale = self.resolved_scale(platform);
        choices.extend(
            self.bezel_files(platform)
                .into_iter()
                .filter(|name| slot_store::bezel_scale(name).is_none_or(|s| s == scale))
                .map(BezelPick::File),
        );
        choices
    }

    fn bezel_pick(&mut self, platform: Platform) -> &mut BezelPick {
        match platform {
            Platform::Gba | Platform::Gbav => &mut self.state.bezel_gba,
            Platform::Gb => &mut self.state.bezel_gb,
            Platform::Gbc => &mut self.state.bezel_gbc,
        }
    }

    pub fn resolved_bezel(&self, platform: Platform) -> BezelPick {
        let pick = match platform {
            Platform::Gba | Platform::Gbav => &self.state.bezel_gba,
            Platform::Gb => &self.state.bezel_gb,
            Platform::Gbc => &self.state.bezel_gbc,
        };
        let choices = self.bezel_choices(platform);
        if choices.contains(pick) {
            return pick.clone();
        }
        let BezelPick::File(name) = pick else {
            return BezelPick::None;
        };
        slot_store::bezel_for_scale(name, self.resolved_scale(platform))
            .map(BezelPick::File)
            .filter(|swapped| choices.contains(swapped))
            .unwrap_or(BezelPick::None)
    }

    fn cycle_bezel(&mut self, platform: Platform, right: bool) {
        let choices = self.bezel_choices(platform);
        let key = self.resolved_bezel(platform).key();
        let at = choices.iter().position(|c| c.key() == key).unwrap_or(0);
        let n = choices.len();
        *self.bezel_pick(platform) = choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
        .clone();
    }

    pub fn sp_frame(&self) -> bool {
        self.resolved_bezel(self.playing_platform()) == BezelPick::Ags102
            && !self.game_layout().panel
    }

    pub fn picture_on_top(&self) -> bool {
        if self.sp_frame() {
            return true;
        }
        let layout = self.game_layout();
        matches!(
            self.resolved_bezel(self.playing_platform()),
            BezelPick::File(_)
        ) && self.shown_game_bezel().is_some()
            && !layout.panel
            && slot_gfx::hole_on_top(&self.bezel_column, self.panel, self.picture_in_band(layout))
    }

    fn picture_in_band(&self, layout: GameLayout) -> (f32, f32) {
        let [_, y, _, h] = layout.rect;
        let (top, tall) = match self.source_rect() == WHOLE_TEXTURE {
            true => {
                let rows = self.playing_platform().picture().1 as f32;
                let src = slot_gfx::SRC_H as f32;
                ((src - rows) / 2.0 / src, rows / src)
            }
            false => (0.0, 1.0),
        };
        ((y + h * top) / OUT_H as f32, h * tall / OUT_H as f32)
    }

    pub fn set_game_bezel(&mut self, tex: Option<TexId>) {
        self.game_bezel = tex;
    }

    pub fn set_bezel_column(&mut self, column: Vec<u8>) {
        self.bezel_column = column;
    }

    pub fn shown_game_bezel(&self) -> Option<TexId> {
        if self.picture_fills() {
            return None;
        }
        if self.resolved_bezel(self.playing_platform()) == BezelPick::Default
            && self.bezel_art().is_none()
        {
            return None;
        }
        self.game_bezel
    }

    pub fn bezel_art(&self) -> Option<BezelArt> {
        if self.picture_fills() {
            return None;
        }
        let gba = self.playing_platform().hardware() == Platform::Gba;
        match (self.picture().0, self.framed_panel()) {
            (Sizing::Band, _) => Some(BezelArt::Default),
            _ if gba => None,
            (Sizing::Integer, false) => Some(BezelArt::Default),
            (Sizing::Integer, true) => Some(BezelArt::Integer4x3),
            (Sizing::Fit, false) => Some(BezelArt::Fit3x2),
            (Sizing::Fit, true) => Some(BezelArt::Fit4x3),
            (Sizing::Panel, _) => None,
        }
    }

    pub fn wallpaper_files(&self) -> Vec<String> {
        self.root().map(crate::wallpaper::list).unwrap_or_default()
    }

    pub fn wallpaper(&self) -> WallpaperPick {
        self.state.wallpaper.clone()
    }

    pub fn resolved_wallpaper(&self) -> WallpaperPick {
        match &self.state.wallpaper {
            WallpaperPick::File(name) if !self.wallpaper_files().iter().any(|n| n == name) => {
                WallpaperPick::Default
            }
            other => other.clone(),
        }
    }

    pub fn shown_wallpaper(&mut self) -> Option<String> {
        match self.resolved_wallpaper() {
            WallpaperPick::File(name) => Some(name),
            WallpaperPick::Random => self.random_wallpaper(),
            WallpaperPick::Shelf => {
                self.shelf_wallpaper = self.wallpaper_for_shelf();
                self.shelf_wallpaper.clone()
            }
            _ => None,
        }
    }

    fn wallpaper_for_shelf(&self) -> Option<String> {
        wallpaper_of_rack(self.shelves[self.shelf_at].0, &self.wallpaper_files())
    }

    pub fn shelf_wallpapers(&self) -> Vec<String> {
        let files = self.wallpaper_files();
        let mut names: Vec<String> = self
            .shelves
            .iter()
            .filter_map(|(rack, _)| wallpaper_of_rack(*rack, &files))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    fn random_wallpaper(&mut self) -> Option<String> {
        if self.random_wallpaper.is_none() {
            let seed = self.wall_secs().unsigned_abs();
            let pick = self
                .root()
                .and_then(|root| crate::wallpaper::pick(root, seed))
                .and_then(|path| Some(path.file_name()?.to_str()?.to_owned()));
            self.random_wallpaper = Some(pick);
        }
        self.random_wallpaper.clone().flatten()
    }

    fn cycle_wallpaper(&mut self, right: bool) {
        let mut choices = vec![
            WallpaperPick::None,
            WallpaperPick::Random,
            WallpaperPick::Shelf,
            WallpaperPick::Default,
            WallpaperPick::Blur,
        ];
        choices.extend(self.wallpaper_files().into_iter().map(WallpaperPick::File));
        let key = self.resolved_wallpaper().key();
        let at = choices.iter().position(|c| c.key() == key).unwrap_or(3);
        let n = choices.len();
        let next = if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        };
        self.state.wallpaper = choices[next].clone();
    }

    fn grounds(&self) -> [Option<(TexId, u32)>; 3] {
        let ground = self.ground.map(|t| (t, OUT_W));
        let blur = self.blur.map(|t| (t, OUT_W)).or(ground);
        let own = self
            .wallpaper
            .map(|t| (t, self.wallpaper_size().0))
            .or(ground);
        [ground, blur, own]
    }

    fn menu_ground(&self) -> Option<(TexId, u32)> {
        let [_, blur, own] = self.grounds();
        match &self.state.wallpaper {
            WallpaperPick::None => None,
            WallpaperPick::Default | WallpaperPick::Blur => blur,
            WallpaperPick::Random if !self.random_drawn() => blur,
            WallpaperPick::Shelf if self.shelf_wallpaper.is_none() => blur,
            WallpaperPick::File(_) | WallpaperPick::Random | WallpaperPick::Shelf => own,
        }
    }

    fn draw_background(&self, out: &mut Vec<Draw>) {
        let [ground, blur, own] = self.grounds();
        let face = match &self.state.wallpaper {
            WallpaperPick::None => None,
            WallpaperPick::Default => ground,
            WallpaperPick::Random if !self.random_drawn() => ground,
            WallpaperPick::Shelf if self.shelf_wallpaper.is_none() => ground,
            WallpaperPick::Blur => blur,
            WallpaperPick::File(_) | WallpaperPick::Random | WallpaperPick::Shelf => own,
        };
        draw_ground(self.canvas(), face, out);
        let alpha = tint_alpha(self.state.wallpaper_tint);
        if alpha > 0.0 && self.own_wallpaper_shown() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: self.canvas().w,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, alpha],
            });
        }
    }

    fn own_wallpaper_shown(&self) -> bool {
        match self.state.wallpaper {
            WallpaperPick::File(_) => self.wallpaper.is_some(),
            WallpaperPick::Random => self.random_drawn() && self.wallpaper.is_some(),
            WallpaperPick::Shelf => self.shelf_wallpaper.is_some() && self.wallpaper.is_some(),
            _ => false,
        }
    }

    pub fn hidden_rows(&self) -> Vec<QuickRow> {
        let own = matches!(
            self.state.wallpaper,
            WallpaperPick::File(_) | WallpaperPick::Random | WallpaperPick::Shelf
        );
        let mut hidden = Vec::new();
        if !own {
            hidden.push(QuickRow::WallpaperTint);
        }
        if !self.framed_panel() {
            hidden.push(QuickRow::GbaBezel);
        }
        for (row, platform) in [
            (QuickRow::GbaScale, Platform::Gba),
            (QuickRow::GbScale, Platform::Gb),
            (QuickRow::GbcScale, Platform::Gbc),
        ] {
            if self.scale_choices(platform).len() < 2 {
                hidden.push(row);
            }
        }
        for (row, platform) in [
            (QuickRow::GbStretch, Platform::Gb),
            (QuickRow::GbcStretch, Platform::Gbc),
        ] {
            if self.stretch_choices(platform).len() < 2 {
                hidden.push(row);
            }
        }
        if !self.left_stick {
            hidden.push(QuickRow::LeftStick);
        }
        if !self.state.turbo {
            hidden.extend([QuickRow::GameTurboA, QuickRow::GameTurboB]);
        }
        if !self.server_allowed() {
            hidden.push(QuickRow::Server);
        }
        if self.wifi_probe.is_some() {
            hidden.push(QuickRow::Network);
        }
        if self.state.wifi_off {
            hidden.extend(QuickRow::NETWORK.into_iter().filter(|r| *r != QuickRow::WifiPower));
        } else if !self.wifi_up() {
            hidden.extend(NET_TOOLS);
        }
        hidden
    }

    pub fn lid_off_min(&self) -> u8 {
        match (self.state.lid_off_min, &self.power) {
            (Some(min), _) => min,
            (None, Some(power)) => (power.timeout().as_secs() / 60) as u8,
            (None, None) => 3,
        }
    }

    fn random_drawn(&self) -> bool {
        matches!(self.random_wallpaper, Some(Some(_)))
    }

    pub fn set_sticker_face(&mut self, face: TexId) {
        self.sticker_face = Some(face);
    }

    pub fn set_clock_faces(&mut self, faces: ClockFaces) {
        self.clock_faces = Some(faces);
    }

    pub fn set_clock_legend_faces(&mut self, legends: [[(TexId, u32); 6]; 2]) {
        self.clock_legends = Some(legends);
    }

    pub fn set_clock_title_face(&mut self, title: (TexId, u32, u32)) {
        self.clock_title = Some(title);
    }

    pub fn set_heart_face(&mut self, face: TexId, w: u32, h: u32) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_heart(face, w, h);
        }
    }

    fn mark_hearts(&mut self) {
        let state = &self.state;
        for (_, shelf) in &mut self.shelves {
            shelf.mark_hearted(|c| state.is_favorite(c.platform, &c.stem));
        }
    }

    pub fn set_cart_shadow(&mut self, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_shadow(face);
        }
    }

    pub fn set_gb_cart_shadows(&mut self, notched: TexId, rounded: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_gb_shadow(GbShell::Notched, notched);
            shelf.set_gb_shadow(GbShell::Rounded, rounded);
        }
    }

    pub fn set_cart_backing(&mut self, face: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_backing(face);
        }
    }

    pub fn set_gb_cart_backings(&mut self, notched: TexId, rounded: TexId) {
        for (_, shelf) in &mut self.shelves {
            shelf.set_gb_backing(GbShell::Notched, notched);
            shelf.set_gb_backing(GbShell::Rounded, rounded);
        }
    }

    pub fn set_ground(&mut self, face: TexId) {
        self.ground = Some(face);
    }

    pub fn set_blur(&mut self, face: TexId) {
        self.blur = Some(face);
    }

    pub fn set_wallpaper(&mut self, face: TexId) {
        self.wallpaper = Some(face);
    }

    pub fn set_wifi_faces(&mut self, faces: [[slot_ui::NetFace; 4]; 2]) {
        self.wifi_faces = Some(faces);
    }

    fn wifi_face(&self) -> Option<slot_ui::NetFace> {
        let faces = self.wifi_faces?;
        Some(faces[usize::from(self.light())][usize::from(self.wifi_bars.min(3))])
    }

    pub fn set_bolt_face(&mut self, bolt: TexId) {
        self.bolt = Some(bolt);
    }

    pub fn set_hud_bolt_face(&mut self, bolt: TexId) {
        self.hud_bolt = Some(bolt);
    }

    pub fn set_slot_faces(&mut self, back: TexId, front: TexId) {
        self.slot_faces = Some((back, front));
    }

    pub fn set_shelf_platform_face(&mut self, face: TexId, w: u32) {
        self.shelf_platform = slot_ui::Printed::new(face, w);
    }

    pub fn clear_shelf_platform(&mut self) {
        self.shelf_platform = slot_ui::Printed::default();
    }

    pub fn slot_text(&self) -> Option<String> {
        match self.slot_letter {
            Some(letter) => Some(letter.to_string()),
            None => self.shelf_platform_name().map(str::to_string),
        }
    }

    pub fn title_text(&self) -> Option<String> {
        if !self.state.show_titles {
            return None;
        }
        let shelf = self.shelf();
        shelf.carts.get(shelf.index).map(slot_ui::label_text)
    }

    pub fn set_shelf_title_face(&mut self, face: TexId, w: u32) {
        self.shelf_title = slot_ui::Printed::new(face, w);
    }

    pub fn clear_shelf_title(&mut self) {
        self.shelf_title = slot_ui::Printed::default();
    }

    fn title_alpha(&self) -> f32 {
        if !self.state.show_titles || (self.name_pending && self.slot_text().is_some()) {
            return 0.0;
        }
        let named = self.shelf_named.map(|at| self.now().saturating_sub(at));
        if named.is_some_and(|t| t < SLOT_NAME_IN_MS + SLOT_NAME_HOLD_MS) {
            return 0.0;
        }
        1.0 - self.slot_name_alpha() / SLOT_NAME_ALPHA
    }

    pub fn shelf_platform_name(&self) -> Option<&'static str> {
        self.next_shelf(1)?;
        Some(self.shelves[self.shelf_at].0.name())
    }

    pub fn set_battery_percent_face(&mut self, face: TexId, w: u32) {
        self.battery_percent = slot_ui::Printed::new(face, w);
    }

    pub fn set_shelf_clock_face(&mut self, face: TexId, w: u32) {
        self.shelf_clock = slot_ui::Printed::new(face, w);
    }

    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    pub fn carts(&self) -> impl Iterator<Item = &Cart> {
        self.shelves
            .iter()
            .filter(|(r, _)| r.platform().is_some())
            .flat_map(|(_, shelf)| shelf.carts.iter())
    }

    pub fn seated_cart(&self) -> Option<&Cart> {
        let stem = match &self.phase {
            Phase::Inserting { cart, .. }
            | Phase::Playing { cart }
            | Phase::Ejecting { cart, .. }
            | Phase::Polaroids { cart } => cart,
            Phase::Doze { cart: Some(cart) } => cart,
            _ => return None,
        };
        self.shelf().carts.iter().find(|c| c.stem == *stem)
    }

    pub fn single_cart(&self) -> bool {
        self.carts().count() == 1
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        let mut faces = faces.into_iter();
        for (_, shelf) in &mut self.shelves {
            let n = shelf.carts.len();
            shelf.set_faces(faces.by_ref().take(n).collect());
        }
    }

    pub fn face_wants(&self) -> Vec<FaceWant> {
        let here = self.shelf();
        let screen = here.on_screen();
        let mut wants: Vec<(usize, usize)> = screen.iter().map(|i| (self.shelf_at, *i)).collect();
        for at in self.neighbour_shelves() {
            wants.extend(self.shelves[at].1.on_screen().into_iter().map(|i| (at, i)));
        }
        wants.extend(
            here.around(FACE_AHEAD)
                .into_iter()
                .filter(|i| !screen.contains(i))
                .map(|i| (self.shelf_at, i)),
        );
        wants
            .into_iter()
            .filter(|(at, i)| self.shelves[*at].1.face(*i).is_none())
            .map(|(shelf, index)| FaceWant {
                shelf,
                index,
                on_screen: shelf == self.shelf_at && screen.contains(&index),
            })
            .collect()
    }

    pub fn face_cart(&self, shelf: usize, index: usize) -> Option<&Cart> {
        self.shelves.get(shelf)?.1.carts.get(index)
    }

    pub fn set_cart_face(&mut self, shelf: usize, index: usize, tex: TexId) -> Option<TexId> {
        self.shelves.get_mut(shelf)?.1.set_face(index, tex)
    }

    pub fn set_cart_interior(&mut self, shelf: usize, index: usize, tex: TexId) -> Option<TexId> {
        self.shelves.get_mut(shelf)?.1.set_interior(index, tex)
    }

    pub fn shed_faces(&mut self) -> Vec<TexId> {
        let neighbours = self.neighbour_shelves();
        let mut shed = std::mem::take(&mut self.orphaned);
        for (at, (_, shelf)) in self.shelves.iter_mut().enumerate() {
            let keep = if at == self.shelf_at {
                shelf.around(FACE_KEEP)
            } else if neighbours.contains(&at) {
                shelf.on_screen()
            } else {
                Vec::new()
            };
            for i in 0..shelf.carts.len() {
                if !keep.contains(&i) {
                    shed.extend(shelf.take_face(i));
                    shed.extend(shelf.take_interior(i));
                }
            }
        }
        shed
    }

    fn neighbour_shelves(&self) -> Vec<usize> {
        let mut out = Vec::new();
        for by in [1, -1] {
            if let Some(at) = self.next_shelf(by) {
                if !out.contains(&at) {
                    out.push(at);
                }
            }
        }
        out
    }

    pub fn set_snapshot(&mut self, snapshot: Box<dyn Snapshot>) {
        self.snapshot = Some(snapshot);
    }

    pub fn set_core(&mut self, core: Core) {
        self.core = core;
    }

    pub fn set_platform(&mut self, platform: Platform) {
        self.platform = platform;
    }

    pub fn set_named_core(&mut self, named: bool) {
        self.named_core = named;
    }

    pub fn set_video_mode(&mut self, mode: VideoMode) {
        self.video_mode = mode;
    }

    pub fn source_rect(&self) -> [f32; 4] {
        self.picture().1
    }

    fn picture(&self) -> (Sizing, [f32; 4]) {
        let platform = self.playing_platform();
        video_mode::picture_for(
            platform,
            self.video_mode,
            self.drawn_scale(platform),
            self.resolved_stretch(platform),
        )
    }

    fn scale_of(&self, platform: Platform) -> Scale {
        match platform {
            Platform::Gba | Platform::Gbav => self.state.scale_gba,
            Platform::Gb => self.state.scale_gb,
            Platform::Gbc => self.state.scale_gbc,
        }
    }

    fn stretch_of(&self, platform: Platform) -> Stretch {
        match platform {
            Platform::Gb => self.state.stretch_gb,
            Platform::Gbc => self.state.stretch_gbc,
            Platform::Gba | Platform::Gbav => Stretch::Classic,
        }
    }

    pub fn game_layout(&self) -> GameLayout {
        let (sizing, source) = self.picture();
        self.layout_of(self.playing_platform(), sizing, source)
    }

    fn layout_of(&self, platform: Platform, sizing: Sizing, source: [f32; 4]) -> GameLayout {
        let picture = match source == WHOLE_TEXTURE {
            true => (slot_gfx::SRC_W, slot_gfx::SRC_H),
            false => platform.picture(),
        };
        slot_gfx::game_layout(self.panel, picture, sizing)
    }

    fn shown_as(
        &self,
        platform: Platform,
        mode: VideoMode,
        scale: Scale,
        stretch: Stretch,
    ) -> (bool, [i32; 4]) {
        let (sizing, source) = video_mode::picture_for(platform, mode, scale, stretch);
        let layout = self.layout_of(platform, sizing, source);
        let (_, crop) =
            video_mode::picture_for(platform, VideoMode::Actual, Scale::Integer, stretch);
        let [rx, ry, rw, rh] = layout.rect;
        let at = |from: f32, len: f32, off: f32, src: f32, size: f32| {
            ((from + (off - src) / len * size) * 100.0).round() as i32
        };
        (
            layout.panel,
            [
                at(rx, source[2], crop[0], source[0], rw),
                at(ry, source[3], crop[1], source[1], rh),
                (crop[2] / source[2] * rw * 100.0).round() as i32,
                (crop[3] / source[3] * rh * 100.0).round() as i32,
            ],
        )
    }

    fn scale_candidates(platform: Platform) -> &'static [Scale] {
        match platform.hardware() {
            Platform::Gba => &[Scale::Fit, Scale::Integer, Scale::FullScreen],
            _ => &[Scale::SpFrame, Scale::Integer, Scale::Fit],
        }
    }

    pub fn scale_choices(&self, platform: Platform) -> Vec<Scale> {
        let mut seen = Vec::new();
        let mut choices = Vec::new();
        for &scale in Self::scale_candidates(platform) {
            let shown = self.shown_as(platform, VideoMode::Actual, scale, Stretch::Classic);
            match seen.iter().position(|s| *s == shown) {
                Some(at) if choices[at] == Scale::SpFrame => choices[at] = scale,
                Some(_) => {}
                None => {
                    seen.push(shown);
                    choices.push(scale);
                }
            }
        }
        choices
    }

    pub fn stretch_choices(&self, platform: Platform) -> Vec<Stretch> {
        if platform.hardware() == Platform::Gba {
            return Vec::new();
        }
        let mut seen = Vec::new();
        let mut choices = Vec::new();
        for stretch in [Stretch::Classic, Stretch::FullScreen] {
            let shown = self.shown_as(platform, VideoMode::Stretch, Scale::SpFrame, stretch);
            if !seen.contains(&shown) {
                seen.push(shown);
                choices.push(stretch);
            }
        }
        choices
    }

    pub fn resolved_scale(&self, platform: Platform) -> Scale {
        let stored = self.scale_of(platform);
        let shown = self.shown_as(platform, VideoMode::Actual, stored, Stretch::Classic);
        self.scale_choices(platform)
            .into_iter()
            .find(|&c| {
                c == stored
                    || self.shown_as(platform, VideoMode::Actual, c, Stretch::Classic) == shown
            })
            .or_else(|| self.scale_choices(platform).first().copied())
            .unwrap_or_default()
    }

    fn drawn_scale(&self, platform: Platform) -> Scale {
        let stored = self.scale_of(platform);
        let shown = self.shown_as(platform, VideoMode::Actual, stored, Stretch::Classic);
        Self::scale_candidates(platform)
            .iter()
            .copied()
            .find(|&c| self.shown_as(platform, VideoMode::Actual, c, Stretch::Classic) == shown)
            .unwrap_or(stored)
    }

    pub fn resolved_stretch(&self, platform: Platform) -> Stretch {
        let stored = self.stretch_of(platform);
        let shown = self.shown_as(platform, VideoMode::Stretch, Scale::SpFrame, stored);
        self.stretch_choices(platform)
            .into_iter()
            .find(|&c| {
                c == stored
                    || self.shown_as(platform, VideoMode::Stretch, Scale::SpFrame, c) == shown
            })
            .unwrap_or_default()
    }

    fn cycle_scale(&mut self, platform: Platform, right: bool) {
        let choices = self.scale_choices(platform);
        let at = choices
            .iter()
            .position(|&c| c == self.resolved_scale(platform))
            .unwrap_or(0);
        let n = choices.len();
        let to = choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }];
        match platform {
            Platform::Gba | Platform::Gbav => self.state.scale_gba = to,
            Platform::Gb => self.state.scale_gb = to,
            Platform::Gbc => self.state.scale_gbc = to,
        }
    }

    fn cycle_stretch(&mut self, platform: Platform, right: bool) {
        let choices = self.stretch_choices(platform);
        let Some(at) = choices
            .iter()
            .position(|&c| c == self.resolved_stretch(platform))
        else {
            return;
        };
        let n = choices.len();
        let to = choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }];
        match platform {
            Platform::Gbc => self.state.stretch_gbc = to,
            _ => self.state.stretch_gb = to,
        }
    }

    pub fn picture_fills(&self) -> bool {
        match self.playing_platform().hardware() {
            Platform::Gba => self.picture().0 == Sizing::Panel && self.game_layout().panel,
            _ => self.video_mode == VideoMode::Stretch,
        }
    }

    fn slot_owns_the_shoulders(&self) -> bool {
        matches!(self.phase, Phase::Playing { .. }) && self.platform.hardware() != Platform::Gba
    }

    pub fn taken_buttons(&self) -> Vec<Btn> {
        if !self.slot_owns_the_shoulders() {
            return Vec::new();
        }
        [Btn::L1, Btn::R1]
            .into_iter()
            .filter(|b| !self.bound_to_game(*b))
            .collect()
    }

    fn bound_to_game(&self, b: Btn) -> bool {
        GameButton::of(self.platform).iter().any(|g| {
            self.controls
                .game(*g)
                .is_some_and(|x| x.modifier.is_none() && crate::input::device_btn(x.button) == b)
        })
    }

    pub fn controls(&self) -> Controls {
        self.controls
    }

    pub fn shortcuts(&self) -> ShortcutKeys {
        self.shortcuts
    }

    pub fn set_printer_face(&mut self, tex: TexId) {
        self.studio_printer = Some(tex);
    }

    pub fn set_plugged_face(&mut self, tex: TexId) {
        self.plugged_face = Some(tex);
    }

    pub fn capturing(&self) -> bool {
        self.capture.is_some()
    }

    pub fn capture_row(&self) -> Option<QuickRow> {
        self.capture.map(|c| c.row)
    }

    fn start_capture(&mut self, row: QuickRow) {
        self.capture = Some(BindCapture {
            row,
            menu: false,
            first: None,
        });
    }

    pub fn capture_input(&mut self, ev: RawEvent) {
        let Some(mut c) = self.capture else {
            return;
        };
        let pairs = c.row.game_button().is_none();
        match ev {
            RawEvent::Down(Btn::Menu) if c.first.is_none() => c.menu = true,
            RawEvent::Up(Btn::Menu) => c.menu = false,
            RawEvent::Down(b) => {
                if let Some(button) = crate::input::btn_device(b) {
                    let binding = match (c.menu, c.first) {
                        (true, _) => Binding::with(Modifier::Menu, button),
                        (false, Some(first)) => Binding::pair(first, button),
                        (false, None) if pairs => {
                            c.first = Some(button);
                            self.capture = Some(c);
                            return;
                        }
                        (false, None) => Binding::plain(button),
                    };
                    return self.bind(c.row, binding);
                }
            }
            RawEvent::Up(b) => {
                if let Some(first) = c.first.filter(|f| crate::input::btn_device(b) == Some(*f)) {
                    return self.bind(c.row, Binding::plain(first));
                }
            }
        }
        self.capture = Some(c);
    }

    fn bind(&mut self, row: QuickRow, binding: Option<Binding>) {
        self.capture = None;
        self.set_binding(row, binding);
        if let Some(other) = self.clash(row) {
            let now = self.now();
            self.hud.toast(Toast::Duplicate(other), now);
        }
    }

    pub fn clash(&self, row: QuickRow) -> Option<QuickRow> {
        self.binding_table().clash(row)
    }

    pub fn binding_table(&self) -> crate::input::Table {
        crate::input::Table::new(
            &self.controls,
            &self.shortcuts,
            &self.carousel,
            self.state.turbo,
        )
    }

    pub fn input_context(&self) -> slot_input::Context {
        let overlaid = self.game_menu_open()
            || self.in_game_menu().is_some()
            || self.repair_prompt()
            || self.shutting_down();
        let shelf = matches!(self.phase, Phase::Shelf)
            && self.core_picker.is_none()
            && !self.repair_prompt
            && self.reset_prompt.is_none()
            && self.studio.is_none()
            && self.fetch_art.is_none();
        match self.phase {
            Phase::Playing { .. } if !overlaid => slot_input::Context::Game,
            _ if shelf => slot_input::Context::Shelf,
            _ => slot_input::Context::Menu,
        }
    }

    fn binding_of(&self, row: QuickRow) -> Option<Binding> {
        if let Some(g) = row.game_button() {
            return self.controls.game(g);
        }
        if let Some(s) = row.shortcut() {
            return self.shortcuts.get(s);
        }
        row.carousel_action().and_then(|a| self.carousel.get(a))
    }

    fn reset_page(&mut self, row: QuickRow) {
        self.reset_prompt = None;
        let (what, written) = if row.game_button().is_some() {
            self.controls = Controls::default();
            let Some(root) = &self.root else {
                return;
            };
            ("controls", write_controls(root, &self.controls))
        } else if row.shortcut().is_some() {
            self.shortcuts = ShortcutKeys::default();
            let Some(root) = &self.root else {
                return;
            };
            ("shortcuts", write_shortcuts(root, &self.shortcuts))
        } else {
            self.carousel = CarouselKeys::default();
            let Some(root) = &self.root else {
                return;
            };
            ("carousel", write_carousel(root, &self.carousel))
        };
        if let Err(e) = &written {
            eprintln!("slot: {what}: {e}");
        }
        self.card_writes.wrote(what, &written);
    }

    fn page_is_default(&self, row: QuickRow) -> bool {
        if row.game_button().is_some() {
            self.controls == Controls::default()
        } else if row.shortcut().is_some() {
            self.shortcuts == ShortcutKeys::default()
        } else {
            self.carousel == CarouselKeys::default()
        }
    }

    pub fn reset_prompt(&self) -> bool {
        self.reset_prompt.is_some()
    }

    fn reset_prompt_faces(&self, row: QuickRow) -> Vec<(TexId, u32, u32)> {
        let at = if row.game_button().is_some() {
            0
        } else if row.shortcut().is_some() {
            2
        } else {
            4
        };
        match self.reset_faces[..] {
            [_, _, _, _, _, _, cancel, reset] => {
                vec![
                    self.reset_faces[at],
                    self.reset_faces[at + 1],
                    cancel,
                    reset,
                ]
            }
            _ => Vec::new(),
        }
    }

    fn set_binding(&mut self, row: QuickRow, binding: Option<Binding>) {
        if let Some(g) = row.game_button() {
            self.controls.set_game(g, binding);
            if let Some(root) = &self.root {
                let written = write_controls(root, &self.controls);
                if let Err(e) = &written {
                    eprintln!("slot: controls: {e}");
                }
                self.card_writes.wrote("controls", &written);
            }
        } else if let Some(s) = row.shortcut() {
            self.shortcuts.set(s, binding);
            if let Some(root) = &self.root {
                let written = write_shortcuts(root, &self.shortcuts);
                if let Err(e) = &written {
                    eprintln!("slot: shortcuts: {e}");
                }
                self.card_writes.wrote("shortcuts", &written);
            }
        } else if let Some(a) = row.carousel_action() {
            self.carousel.set(a, binding);
            if let Some(root) = &self.root {
                let written = write_carousel(root, &self.carousel);
                if let Err(e) = &written {
                    eprintln!("slot: carousel: {e}");
                }
                self.card_writes.wrote("carousel", &written);
            }
        }
    }

    pub fn binding_texts(&self) -> Vec<String> {
        let mut texts: Vec<String> = QuickRow::ALL
            .iter()
            .filter(|r| r.binds())
            .map(|r| self.bound_text(*r))
            .collect();
        texts.push(RECORDING.to_string());
        texts.sort();
        texts.dedup();
        texts
    }

    pub fn has_binding_face(&self, text: &str) -> bool {
        self.binding_faces.contains_key(text)
    }

    pub fn add_binding_face(&mut self, text: String, faces: [(TexId, u32, u32); 2]) {
        self.binding_faces.insert(text, faces);
    }

    fn bound_faces(&self) -> Vec<Option<[(TexId, u32, u32); 2]>> {
        QuickRow::ALL
            .iter()
            .map(|r| {
                if !r.binds() {
                    return None;
                }
                let text = match self.capture_row() == Some(*r) {
                    true => RECORDING.to_string(),
                    false => self.bound_text(*r),
                };
                self.binding_faces.get(&text).copied()
            })
            .collect()
    }

    fn bound_text(&self, row: QuickRow) -> String {
        let text = Binding::text(self.binding_of(row));
        match self.clash(row) {
            Some(_) => format!("{CLASH_MARK}{text}"),
            None => text,
        }
    }

    fn description(&self, row: QuickRow) -> Option<(TexId, u32, u32)> {
        let faces = self.quick_menu_faces.as_ref()?;
        if row.shortcut().is_some() || row.carousel_action().is_some() {
            Some(faces.descriptions[1])
        } else if row.binds() {
            Some(faces.descriptions[0])
        } else {
            None
        }
    }

    pub fn carousel(&self) -> CarouselKeys {
        self.carousel
    }

    pub fn takes_from_the_game(&self, action: Action) -> bool {
        match action {
            Action::GbaDown(btn) | Action::GbaUp(btn) => self.taken_buttons().contains(&btn),
            _ => false,
        }
    }

    fn set_picture(&mut self, mode: VideoMode) {
        if self.video_mode == mode {
            return;
        }
        self.video_mode = mode;
        let (Some(root), Phase::Playing { cart }) = (self.root.clone(), &self.phase) else {
            return;
        };
        let written = video_mode::write_video_mode(&root, cart, mode);
        if let Err(e) = &written {
            eprintln!("slot: video: could not write video_mode.txt: {e}");
        }
        self.card_writes.wrote("video_mode.txt", &written);
    }

    pub fn set_link_loaded(&mut self, serial: &'static str) {
        self.link_loaded = Some(serial);
    }

    pub fn link_mode(&self, stem: &str) -> (LinkKind, &'static str) {
        let Some((cart, auto)) = self.auto_link(stem) else {
            return (LinkKind::Cable, "auto");
        };
        let chosen = self.link_choices.get(stem).copied().unwrap_or(auto);
        (chosen, serial_option(chosen, auto, &cart.code, &cart.title))
    }

    pub fn link_active(&self) -> bool {
        self.link.is_some()
    }

    pub fn begin_link(&mut self, client_id: u16) {
        self.link = Some(LinkSession {
            client_id,
            lost_at: None,
        });
        self.sync_link_badge();
    }

    pub fn link_client_id(&self) -> Option<u16> {
        self.link.as_ref().map(|s| s.client_id)
    }

    pub fn may_rewind(&self) -> bool {
        self.state.rewind && !self.link_active()
    }

    pub fn turbo(&self) -> bool {
        self.state.turbo
    }

    pub fn records_rewind(&self) -> bool {
        self.state.rewind
    }

    pub fn picture_rect(&self) -> [f32; 4] {
        let (w, h) = self.platform.picture();
        let (sw, sh) = (slot_gfx::SRC_W as f32, slot_gfx::SRC_H as f32);
        let (x, y) = ((sw - w as f32) / 2.0, (sh - h as f32) / 2.0);
        [x / sw, y / sh, (x + w as f32) / sw, (y + h as f32) / sh]
    }

    pub fn may_load_state(&self) -> bool {
        !self.link_active()
    }

    pub fn may_fast_forward(&self) -> bool {
        self.state.ff_speed != 0 && !self.link_active()
    }

    pub fn end_link(&mut self) {
        self.link = None;
        self.link_player = None;
        self.sync_link_badge();
        self.radio.ask(RadioJob::Down);
        self.radio.ask(RadioJob::Cool);
        if self.state.wifi_off {
            self.wifi_send(crate::wifi::Cmd::PowerOff);
        }
    }

    pub fn peer_lost(&mut self) {
        if matches!(self.game_menu, Some(GameMenu::Linked { .. })) {
            self.game_menu = None;
        }
        let now = self.now();
        if let Some(session) = &mut self.link {
            session.lost_at.get_or_insert(now);
        }
        self.sync_link_badge();
    }

    pub fn bios_mismatch(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::BiosMismatch, self.now());
        self.unplug(role);
    }

    pub fn peer_ended(&mut self) {
        if !self.link_active() {
            return;
        }
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::PeerEnded, self.now());
        self.unplug(role);
    }

    pub fn link_badge(&self) -> LinkBadge {
        self.hud.link()
    }

    pub fn set_link_badge_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_link_faces(faces);
    }

    fn sync_link_badge(&mut self) {
        let badge = match &self.link {
            None => LinkBadge::Off,
            Some(s) => match (s.client_id == 0, s.lost_at.is_some()) {
                (true, false) => LinkBadge::Hosting,
                (false, false) => LinkBadge::Joined,
                (true, true) => LinkBadge::HostingLost,
                (false, true) => LinkBadge::JoinedLost,
            },
        };
        self.hud.set_link(badge);
    }

    pub fn take_sfx(&mut self) -> Option<Sfx> {
        self.sfx.take()
    }

    pub fn set_power(&mut self, mut power: Power) {
        power.set_backlight(self.state.brightness + 1);
        self.headphones = power.headphones();
        self.hud.set_headphones(self.headphones);
        let secs = power.now();
        if matches!(self.phase, Phase::SetClock { .. }) || secs < CLOCK_FLOOR {
            self.phase = clock_screen(secs, &self.state, self.has_saved(), false);
        }
        self.copy_card_log(&power);
        if let Some(result) = power.take_repair_result() {
            let toast = match result {
                RepairResult::Repaired | RepairResult::Clean => Toast::CardRepaired,
                RepairResult::Failed => Toast::CardRepairFailed,
            };
            self.hud.toast(toast, self.now());
        }
        self.power = Some(power);
        self.battery_at = self.now();
        self.charge_at = self.now();
        self.last_led = None;
    }

    fn copy_card_log(&self, power: &Power) {
        let (Some(log), Some(root)) = (power.card_log(), &self.root) else {
            return;
        };
        if let Err(e) = crate::card_log::copy_to_card(&log, root) {
            eprintln!("slot: card log: {e}");
        }
    }

    fn log_card(&self, alarm: bool) {
        let Some(power) = &self.power else {
            return;
        };
        let Some(log) = power.card_log() else {
            return;
        };
        let kernel = if alarm {
            power.card_kernel_lines()
        } else {
            Vec::new()
        };
        let write_error = self.card_writes.last_error();
        let entry = crate::card_log::entry(&crate::card_log::CardEvent {
            secs: power.now(),
            alarm,
            read_only: self.card_read_only,
            write_error: write_error.as_deref(),
            cart: self.seated(),
            kernel: &kernel,
        });
        if let Err(e) = crate::card_log::append(&log, &entry) {
            eprintln!("slot: card log: {e}");
        }
    }

    pub fn set_rumble(&mut self, strength: u16) {
        if let Some(power) = &mut self.power {
            power.set_rumble(strength);
        }
    }

    pub fn rumble_enabled(&self) -> bool {
        self.state.rumble
    }

    pub fn set_left_stick(&mut self, present: bool) {
        self.left_stick = present;
    }

    pub fn left_stick_dpad(&self) -> bool {
        self.left_stick && self.state.left_stick_dpad
    }

    pub fn ff_speed(&self) -> u8 {
        self.state.ff_speed
    }

    pub fn ff_sound(&self) -> bool {
        self.state.ff_sound
    }

    fn gba_shaders(&self) -> &'static [Shader] {
        let (w, h) = self.panel;
        if w > 0 && w * 2 == h * 3 {
            &Shader::GBA_3_2
        } else {
            &Shader::GBA
        }
    }

    fn gba_shader(&self) -> Shader {
        match self.gba_shaders().contains(&self.state.shader_gba) {
            true => self.state.shader_gba,
            false => Shader::Off,
        }
    }

    pub fn screen_shader(&self) -> Shader {
        match self.playing_platform() {
            Platform::Gba | Platform::Gbav => self.gba_shader(),
            Platform::Gb => self.state.shader_gb,
            Platform::Gbc => self.state.shader_gbc,
        }
    }

    pub fn colour_correction(&self) -> bool {
        self.colour_correction_for(self.playing_platform())
    }

    pub fn playing_platform(&self) -> Platform {
        self.seated_cart().map_or(self.platform, |c| c.platform)
    }

    pub fn colour_correction_for(&self, platform: Platform) -> bool {
        match platform {
            Platform::Gba | Platform::Gbav => self.state.colour_correction_gba,
            Platform::Gb => self.state.colour_correction_gb,
            Platform::Gbc => self.state.colour_correction_gbc,
        }
    }

    pub fn blending_for(&self, platform: Platform) -> Blending {
        match platform {
            Platform::Gba | Platform::Gbav => self.state.blending_gba,
            Platform::Gb => self.state.blending_gb,
            Platform::Gbc => self.state.blending_gbc,
        }
    }

    pub fn blending_in_play(&self) -> Blending {
        self.blending_for(self.playing_platform()).on(self.core)
    }

    pub fn light(&self) -> bool {
        self.state.appearance.light()
    }

    pub fn status_ink(&self) -> [u8; 3] {
        match self.phase {
            Phase::Playing { .. } | Phase::Polaroids { .. } => footer_ink(false),
            _ => footer_ink(self.light()),
        }
    }

    pub fn powering_off(&self) -> bool {
        self.powering_off
    }

    pub fn ready_to_power_off(&self) -> bool {
        self.powering_off && self.server.is_none() && self.now() >= self.act_at
    }

    pub fn ready_to_repair(&self) -> bool {
        self.repairing && self.now() >= self.act_at
    }

    pub fn repair_offered(&self) -> bool {
        self.repair_possible() && self.charged_to_repair()
    }

    fn repair_possible(&self) -> bool {
        self.card_read_only
            && !self.repair_prompt
            && !self.shutting_down()
            && matches!(self.phase, Phase::Shelf | Phase::Playing { .. })
    }

    fn charged_to_repair(&self) -> bool {
        self.battery.is_none_or(|b| {
            b.percent >= REPAIR_BATTERY_MIN || matches!(b.charge, Charge::Charging | Charge::Full)
        })
    }

    pub fn repair_prompt(&self) -> bool {
        self.repair_prompt
    }

    pub fn repair(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.repair_card();
        }
    }

    pub fn ready_to_restart(&self) -> bool {
        self.restarting && self.now() >= self.act_at
    }

    pub fn shutting_down(&self) -> bool {
        self.powering_off || self.restarting || self.repairing
    }

    pub fn restarting(&self) -> bool {
        self.restarting
    }

    pub fn relaunching(&self) -> bool {
        self.relaunching
    }

    pub fn restart(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.restart();
        }
    }

    pub fn core_picker(&self) -> Option<Core> {
        self.core_picker.map(|p| p.seat())
    }

    pub fn core_picker_chip(&self) -> Option<Chip> {
        self.core_picker.map(|p| p.chip(self.now()))
    }

    pub fn game_menu_open(&self) -> bool {
        self.game_menu.is_some()
    }

    pub fn game_menu(&self) -> Option<GameMenu> {
        self.game_menu
    }

    pub fn take_link_transport(&mut self) -> Option<(u16, Box<dyn LinkChannel>)> {
        self.link_transport.take()
    }

    pub fn take_link_reload(&mut self) -> Option<(String, &'static str)> {
        self.link_reload.take()
    }

    pub fn link_player(&self) -> Option<u8> {
        self.link_player
    }

    pub fn take_colour_correction(&mut self) -> Option<bool> {
        self.colour_pending.take()
    }

    pub fn take_blending(&mut self) -> Option<Blending> {
        self.blending_pending.take()
    }

    pub fn sunlight(&self) -> Option<u8> {
        if self.core != Core::Mgba
            || self.seated_cart().is_none()
            || self.playing_platform() != Platform::Gba
        {
            return None;
        }
        Some(
            self.sunlight_manual
                .unwrap_or_else(|| crate::core::sunlight_at(self.wall_secs())),
        )
    }

    fn step_sunlight(&mut self, brighter: bool) {
        if self.seated_cart().is_none() || self.playing_platform() != Platform::Gba {
            return;
        }
        let Some(now) = self.sunlight() else {
            self.hud.toast(Toast::NeedsMgba, self.now());
            return;
        };
        let to = match brighter {
            true => up(now, 1, SUNLIGHT_MAX),
            false => now.saturating_sub(1),
        };
        self.sunlight_manual = Some(to);
        self.hud.show(HudKind::Sunlight, to, false, self.now());
    }

    pub fn gb_colors(&self) -> GbColors {
        self.state.gb_colors()
    }

    pub fn card_read(
        &self,
        what: &str,
        read: std::io::Result<Option<Vec<u8>>>,
    ) -> Option<Option<Vec<u8>>> {
        match read {
            Ok(bytes) => Some(bytes),
            Err(e) => {
                eprintln!("slot: {what} read: {e}");
                self.card_writes.failed(what, &e);
                None
            }
        }
    }

    pub fn set_resume_unread(&mut self, unread: bool) {
        self.resume_unread = unread;
    }

    pub fn set_palette_live(&mut self, live: bool) {
        self.palette_live = live;
    }

    pub fn palette_live(&self) -> bool {
        self.palette_live
    }

    pub fn take_gb_colors(&mut self) -> Option<GbColors> {
        self.palette_pending.take()
    }

    pub fn core(&self) -> Core {
        self.core
    }

    pub fn link_reload_done(&mut self) {
        let Some(reload) = self.reload.take() else {
            return;
        };
        if reload.fallback {
            self.link_choices.insert(reload.stem, reload.from);
            self.close_game_menu();
            return self.refuse();
        }
        if reload.cancelled {
            return self.close_game_menu();
        }
        let since = match self.game_menu {
            Some(GameMenu::Working { since, .. }) => since,
            _ => self.now(),
        };
        self.start_link_from(
            LinkStarter::spawn(reload.role.role(), link_port()),
            reload.role.client_id(),
            since,
        );
    }

    pub fn link_reload_failed(&mut self) {
        let Some(mut reload) = self.reload.take() else {
            return;
        };
        if !reload.fallback {
            self.link_reload = Some((reload.stem.clone(), reload.from_serial));
            reload.fallback = true;
            self.reload = Some(reload);
            return;
        }
        self.link_choices.insert(reload.stem, reload.from);
        self.refuse_seated();
    }

    pub fn selected_stem(&self) -> Option<&str> {
        self.shelf()
            .carts
            .get(self.shelf().index)
            .map(|c| c.stem.as_str())
    }

    pub fn battery(&self) -> Option<Battery> {
        self.battery
    }

    pub fn poweroff(&mut self) {
        self.settle_saves();
        if let Some(power) = &mut self.power {
            power.poweroff();
        }
    }

    pub fn apply(&mut self, action: Action) {
        self.track_held_dir(action);
        match action {
            Action::LidClose => return self.doze(),
            Action::LidOpen => return self.wake(),
            Action::PowerPress => {
                if self.link_active() {
                    self.end_link();
                    return;
                }
                self.woke_on_press = matches!(self.phase, Phase::Doze { .. });
                if self.woke_on_press {
                    self.wake();
                }
                return self.flush_resume();
            }
            Action::PowerTap => return self.power_press(),
            Action::PowerHold => return self.hold_power(),
            Action::PowerOff => return,
            _ => {}
        }
        if let Phase::SetClock { picker, .. } = &mut self.phase {
            match action {
                Action::GbaDown(Btn::Left) | Action::ShelfLeft => picker.left(),
                Action::GbaDown(Btn::Right) | Action::ShelfRight => picker.right(),
                Action::GbaDown(Btn::Up) => picker.up(),
                Action::GbaDown(Btn::Down) => picker.down(),
                Action::GbaDown(Btn::A) | Action::Insert => picker.a(),
                Action::GbaDown(Btn::B) if !picker.b() => self.confirm_clock(),
                _ => {}
            }
            return;
        }
        let server_adjusts = matches!(
            action,
            Action::VolumeUp
                | Action::VolumeDown
                | Action::MuteToggle
                | Action::BrightnessUp
                | Action::BrightnessDown
        );
        let adjusts = self.fetch_art.is_some()
            || self.studio.is_some()
            || (self.server.is_some() && server_adjusts);
        if adjusts && self.adjust(action) {
            return;
        }
        if self.fetch_art.is_some() {
            return self.fetch_art_input(action);
        }
        if self.studio.is_some() {
            return self.studio_input(action);
        }
        if self.server.is_some() {
            return self.server_input(action);
        }
        if self.update.is_some() {
            return self.update_input(action);
        }
        if self.forget_prompt {
            return self.forget_input(action);
        }
        if self.keyboard.is_some() {
            return self.keyboard_input(action);
        }
        if self.repair_prompt {
            match action {
                Action::GbaDown(Btn::A) => self.begin_repair(),
                Action::GbaDown(Btn::B) => self.repair_prompt = false,
                _ => {}
            }
            return;
        }
        if let Some(row) = self.reset_prompt {
            match action {
                Action::GbaDown(Btn::A) => self.reset_page(row),
                Action::GbaDown(Btn::B) => self.reset_prompt = None,
                _ => {}
            }
            return;
        }
        if self.delete_prompt {
            match action {
                Action::GbaDown(Btn::X) => {
                    self.delete_prompt = false;
                    match self.shot_viewer {
                        Some(_) => self.delete_shot(),
                        None => self.delete_selected(),
                    }
                }
                Action::GbaDown(Btn::B) => self.delete_prompt = false,
                _ => {}
            }
            return;
        }
        if action == Action::RepairCard {
            self.repair_prompt = self.repair_offered();
            return;
        }
        if self.adjust(action) {
            return;
        }
        match action {
            Action::GbaUp(Btn::Left) => self.shelf_mut().release_left(),
            Action::GbaUp(Btn::Right) => self.shelf_mut().release_right(),
            _ => {}
        }
        if self.game_menu.is_some() {
            return self.game_menu_input(action);
        }
        if let (Some(row), Phase::Playing { .. }) = (self.in_game_menu, &self.phase) {
            if action != Action::Eject {
                return self.in_game_menu_input(row, action);
            }
        }
        let now = self.now();
        match self.phase {
            Phase::Shelf => match action {
                Action::SelectTap if self.core_picker.is_none() => self.open_studio(),
                Action::GbaDown(Btn::Start) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::PreviousShelf => self.switch_shelf(-1),
                Action::NextShelf => self.switch_shelf(1),
                Action::Favorite => self.toggle_favorite(),
                Action::ForgetRecent if self.shelf_at == RECENTS_AT => self.forget_recent(),
                Action::GbaDown(Btn::Up) => self.jump_letter(-1),
                Action::GbaDown(Btn::Down) => self.jump_letter(1),
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf_mut().hold_left(now),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => {
                    self.shelf_mut().hold_right(now)
                }
                Action::QuickMenu => self.open_quick_menu(),
                Action::GbaDown(Btn::A) => self.play_held = Some(now),
                Action::GbaUp(Btn::A) => {
                    if self.play_held.take().is_some() {
                        self.insert(!self.state.auto_load);
                    }
                }
                Action::Insert => self.insert(!self.state.auto_load),
                _ => {}
            },
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::GbaDown(Btn::L1)
                    if self.slot_owns_the_shoulders() && !self.bound_to_game(Btn::L1) =>
                {
                    self.set_picture(VideoMode::Stretch)
                }
                Action::GbaDown(Btn::R1)
                    if self.slot_owns_the_shoulders() && !self.bound_to_game(Btn::R1) =>
                {
                    self.set_picture(VideoMode::Actual)
                }
                Action::Eject => self.eject(),
                Action::GameMenu => self.game_menu_shortcut(),
                Action::QuickMenu => self.tap_in_game_menu(Some(QuickRow::PLAY[0])),
                Action::Polaroids => self.double_tap_states(),
                Action::SaveState => self.save_state(),
                Action::Screenshot if self.screen >= 1.0 && self.game_visible() => {
                    self.shot_wanted = true
                }
                Action::LoadState => self.load_newest(),
                Action::Reset => self.reset_game(),
                Action::Guide => self.open_guide(),
                Action::RewindStart if !self.may_rewind() => self.refuse(),
                Action::FfStart if !self.may_fast_forward() => self.refuse(),
                _ => {}
            },
            Phase::Polaroids { .. } if self.guide.is_some() => self.guide_input(action),
            Phase::Polaroids { .. } if self.shot_viewer.is_some() => self.shot_input(action),
            Phase::Polaroids { .. } => match action {
                Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.flick(Polaroids::left),
                Action::ShelfRight | Action::GbaDown(Btn::Right) => self.flick(Polaroids::right),
                Action::GbaDown(Btn::A) => self.load_selected(),
                Action::GbaDown(Btn::B) | Action::Polaroids => self.close_polaroids(),
                Action::GbaDown(Btn::X) => self.undo(self.now()),
                Action::GbaDown(Btn::Y) => {
                    self.delete_prompt =
                        self.polaroids.as_ref().and_then(|p| p.selected()).is_some()
                }
                _ => {}
            },
            Phase::About if action == Action::GbaDown(Btn::B) || action == Action::QuickMenu => {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::About,
                }
            }
            Phase::About if action == Action::Restart || action == Action::GbaDown(Btn::X) => {
                self.begin_restart()
            }
            Phase::QuickMenu { row } => self.quick_menu_input(row, action),
            Phase::Networks { .. } => self.networks_input(action),
            Phase::NetworkScan { .. } => self.network_scan_input(action),
            Phase::NetworkPage { .. } => self.network_page_input(action),
            _ => {}
        }
    }

    fn open_quick_menu(&mut self) {
        self.phase = Phase::QuickMenu {
            row: QuickRow::MAIN[0],
        };
    }

    fn quick_menu_input(&mut self, row: QuickRow, action: Action) {
        if row.binds() {
            match action {
                Action::GbaDown(Btn::A) => return self.start_capture(row),
                Action::GbaDown(Btn::X) => return self.set_binding(row, None),
                Action::GbaDown(Btn::Y) if !self.page_is_default(row) => {
                    self.reset_prompt = Some(row);
                    return;
                }
                Action::GbaDown(Btn::Left | Btn::Right) => return,
                _ => {}
            }
        }
        let row = match action {
            Action::GbaDown(Btn::Up) => row.up(&self.hidden_rows()),
            Action::GbaDown(Btn::Down) => row.down(&self.hidden_rows()),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) => return self.open_quick_row(row),
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = match row.parent() {
                    Some(row) => Phase::QuickMenu { row },
                    None => Phase::Shelf,
                };
                return;
            }
            _ => return,
        };
        self.phase = Phase::QuickMenu { row };
    }

    fn in_game_menu_input(&mut self, row: QuickRow, action: Action) {
        let hidden = self.in_game_hidden();
        let hidden = &hidden[..];
        if row.binds() {
            match action {
                Action::GbaDown(Btn::A) => return self.start_capture(row),
                Action::GbaDown(Btn::X) => return self.set_binding(row, None),
                Action::GbaDown(Btn::Y) if !self.page_is_default(row) => {
                    self.reset_prompt = Some(row);
                    return;
                }
                Action::GbaDown(Btn::Left | Btn::Right) => return,
                _ => {}
            }
        }
        self.in_game_menu = match action {
            Action::GbaDown(Btn::Up) => Some(row.up(hidden)),
            Action::GbaDown(Btn::Down) => Some(row.down(hidden)),
            Action::GbaDown(Btn::Left) => return self.change_setting(row, false),
            Action::GbaDown(Btn::Right) => return self.change_setting(row, true),
            Action::GbaDown(Btn::A) => return self.open_in_game_row(row),
            Action::GbaDown(Btn::B) => row.parent_in_game(),
            Action::QuickMenu => return self.tap_in_game_menu(None),
            Action::Polaroids => return self.double_tap_states(),
            _ => return,
        };
    }

    fn open_in_game_row(&mut self, row: QuickRow) {
        match row {
            QuickRow::Resume => self.in_game_menu = None,
            QuickRow::UndoSave | QuickRow::UndoLoad | QuickRow::UndoReset => {
                self.in_game_menu = None;
                self.undo(self.now());
            }
            QuickRow::States => self.open_states(),
            QuickRow::Screenshots => self.open_shots(),
            QuickRow::Guide => self.open_guide(),
            QuickRow::ExitGame => self.eject(),
            _ => {
                if let Some(child) = row.child() {
                    self.in_game_menu = Some(child);
                }
            }
        }
    }

    fn tap_in_game_menu(&mut self, to: Option<QuickRow>) {
        if to.is_some() && self.in_game_menu.is_none() {
            self.refresh_albums();
            self.hud.hush();
        }
        self.menu_tap = Some((self.now(), self.in_game_menu));
        self.in_game_menu = to;
    }

    fn in_game_menu_shown(&self) -> Option<QuickRow> {
        match self.menu_tap {
            Some((at, None)) if self.now().saturating_sub(at) < MENU_REVEAL_MS => None,
            _ => self.in_game_menu,
        }
    }

    fn double_tap_states(&mut self) {
        if let Some((at, before)) = self.menu_tap.take() {
            if self.now().saturating_sub(at) <= MENU_DOUBLE_TAP_MS {
                self.in_game_menu = before;
            }
        }
        match self.in_game_menu {
            Some(_) => self.open_states(),
            None => self.open_polaroids(),
        }
    }

    fn open_states(&mut self) {
        let before = self.in_game_menu;
        self.in_game_menu = None;
        self.open_polaroids();
        match self.phase {
            Phase::Polaroids { .. } => self.states_from_menu = true,
            _ => self.in_game_menu = before,
        }
    }

    fn open_quick_row(&mut self, row: QuickRow) {
        match row {
            QuickRow::DateTime => {
                self.phase = clock_screen(self.utc_secs(), &self.state, self.has_saved(), true);
            }
            QuickRow::About => self.phase = Phase::About,
            QuickRow::FetchArt => self.open_fetch_missing_art(),
            QuickRow::CheckUpdates => self.open_check_for_updates(),
            QuickRow::Server => self.open_server(),
            QuickRow::WifiPower => self.set_wifi_off(!self.state.wifi_off),
            QuickRow::Networks => self.open_networks(),
            QuickRow::Screen
            | QuickRow::ScreenGeneral
            | QuickRow::Game
            | QuickRow::ScreenGba
            | QuickRow::ScreenGbc
            | QuickRow::ScreenGb
            | QuickRow::Shortcuts
            | QuickRow::General
            | QuickRow::Network
            | QuickRow::Controls
            | QuickRow::ControlsShelf
            | QuickRow::ControlsInGame => {
                if let Some(row) = row.child() {
                    let row = row.shown(&self.hidden_rows())[0];
                    self.phase = Phase::QuickMenu { row };
                }
            }
            QuickRow::Brightness
            | QuickRow::BlueLight
            | QuickRow::Saturation
            | QuickRow::Contrast
            | QuickRow::PlayBrightness
            | QuickRow::PlayBlueLight
            | QuickRow::PlaySaturation
            | QuickRow::PlayContrast
            | QuickRow::AutoLoad
            | QuickRow::EjectSave
            | QuickRow::Turbo
            | QuickRow::Rewind
            | QuickRow::FastForward
            | QuickRow::FastForwardSound
            | QuickRow::GbaColourCorrection
            | QuickRow::GbColourCorrection
            | QuickRow::GbcColourCorrection
            | QuickRow::GbaShader
            | QuickRow::GbShader
            | QuickRow::GbcShader
            | QuickRow::GbPalette
            | QuickRow::Rumble
            | QuickRow::LeftStick
            | QuickRow::Theme
            | QuickRow::Recents
            | QuickRow::ShowTitles
            | QuickRow::Wallpaper
            | QuickRow::WallpaperTint
            | QuickRow::Resume
            | QuickRow::UndoSave
            | QuickRow::UndoLoad
            | QuickRow::UndoReset
            | QuickRow::States
            | QuickRow::Screenshots
            | QuickRow::Guide
            | QuickRow::Settings
            | QuickRow::ExitGame
            | QuickRow::PlayScreen
            | QuickRow::PlayGameplay
            | QuickRow::Shader
            | QuickRow::ColourCorrection
            | QuickRow::Palette
            | QuickRow::GbBezel
            | QuickRow::GbcBezel
            | QuickRow::GbaBezel
            | QuickRow::GbaScale
            | QuickRow::GbScale
            | QuickRow::GbcScale
            | QuickRow::GbStretch
            | QuickRow::GbcStretch
            | QuickRow::GbaBlending
            | QuickRow::GbcBlending
            | QuickRow::GbBlending
            | QuickRow::Blending
            | QuickRow::Scale
            | QuickRow::Stretch
            | QuickRow::Bezel
            | QuickRow::ShelfPrev
            | QuickRow::ShelfNext
            | QuickRow::ShelfFavorite
            | QuickRow::ShelfForget
            | QuickRow::PlayControls
            | QuickRow::GameUp
            | QuickRow::GameDown
            | QuickRow::GameLeft
            | QuickRow::GameRight
            | QuickRow::GameA
            | QuickRow::GameB
            | QuickRow::GameSelect
            | QuickRow::GameStart
            | QuickRow::GameL
            | QuickRow::GameR
            | QuickRow::GameTurboA
            | QuickRow::GameTurboB
            | QuickRow::ScSave
            | QuickRow::ScLoad
            | QuickRow::ScScreenshot
            | QuickRow::ScBrightUp
            | QuickRow::ScBrightDown
            | QuickRow::ScBlueDown
            | QuickRow::ScBlueUp
            | QuickRow::ScColour
            | QuickRow::ScPaletteBack
            | QuickRow::ScPaletteNext
            | QuickRow::ScShaderNext
            | QuickRow::ScShaderBack
            | QuickRow::ScFastForward
            | QuickRow::ScFastForwardToggle
            | QuickRow::ScRewind
            | QuickRow::ScSunlightUp
            | QuickRow::ScSunlightDown
            | QuickRow::ScReset
            | QuickRow::ScGuide
            | QuickRow::LidOff => {}
        }
    }

    fn change_setting(&mut self, row: QuickRow, right: bool) {
        if row == QuickRow::WifiPower {
            return self.set_wifi_off(!self.state.wifi_off);
        }
        if row == QuickRow::Blending {
            let to = self
                .blending_in_play()
                .step(Blending::choices(self.core), right);
            match self.playing_platform().hardware() {
                Platform::Gb => self.state.blending_gb = to,
                Platform::Gbc => self.state.blending_gbc = to,
                _ => self.state.blending_gba = to,
            }
            self.blending_pending = Some(self.blending_in_play());
            self.persist();
            return;
        }
        let row = self.playing_row(row);
        if let QuickRow::Saturation | QuickRow::Contrast = row {
            let (level, max) = match row {
                QuickRow::Saturation => (&mut self.state.saturation, SATURATION_MAX),
                _ => (&mut self.state.contrast, CONTRAST_MAX),
            };
            *level = match right {
                true => up(*level, 1, max),
                false => level.saturating_sub(1),
            };
            self.persist_soon();
            return;
        }
        if let QuickRow::Brightness | QuickRow::BlueLight = row {
            let (kind, now, max) = match row {
                QuickRow::Brightness => {
                    (HudKind::Brightness, self.state.brightness, BRIGHTNESS_MAX)
                }
                _ => (HudKind::BlueLight, self.state.blue_light, BLUE_LIGHT_MAX),
            };
            let to = match right {
                true => up(now, 1, max),
                false => now.saturating_sub(1),
            };
            self.set_level(kind, to);
            return;
        }
        if let QuickRow::Palette | QuickRow::GbPalette = row {
            let to = match right {
                true => self.state.gb_colors().next(),
                false => self.state.gb_colors().prev(),
            };
            self.state.set_gb_colors(to);
            self.palette_pending = Some(to);
            self.persist();
            return;
        }
        if let QuickRow::GbaScale | QuickRow::GbScale | QuickRow::GbcScale = row {
            let platform = match row {
                QuickRow::GbaScale => Platform::Gba,
                QuickRow::GbScale => Platform::Gb,
                _ => Platform::Gbc,
            };
            self.cycle_scale(platform, right);
            self.persist();
            return;
        }
        if let QuickRow::GbStretch | QuickRow::GbcStretch = row {
            let platform = match row {
                QuickRow::GbStretch => Platform::Gb,
                _ => Platform::Gbc,
            };
            self.cycle_stretch(platform, right);
            self.persist();
            return;
        }
        if let QuickRow::GbaBezel | QuickRow::GbBezel | QuickRow::GbcBezel = row {
            let platform = match row {
                QuickRow::GbaBezel => Platform::Gba,
                QuickRow::GbBezel => Platform::Gb,
                _ => Platform::Gbc,
            };
            self.cycle_bezel(platform, right);
            self.persist();
            return;
        }
        if row == QuickRow::Wallpaper {
            self.cycle_wallpaper(right);
            self.persist();
            return;
        }
        if row == QuickRow::Recents {
            self.state.show_recents = !self.state.show_recents;
            self.restock_recents();
            self.persist();
            return;
        }
        if row == QuickRow::ShowTitles {
            self.state.show_titles = !self.state.show_titles;
            self.persist();
            return;
        }
        if row == QuickRow::LidOff {
            let n = LID_OFF_MIN.len();
            let at = LID_OFF_MIN
                .iter()
                .position(|m| *m == self.lid_off_min())
                .unwrap_or(1);
            self.state.lid_off_min = Some(
                LID_OFF_MIN[if right {
                    (at + 1) % n
                } else {
                    (at + n - 1) % n
                }],
            );
            self.persist();
            return;
        }
        let gba_shaders = self.gba_shaders();
        let s = &mut self.state;
        match row {
            QuickRow::FastForward => {
                let to = ff_next(s.ff_speed, right);
                if to == s.ff_speed {
                    return;
                }
                s.ff_speed = to;
            }
            QuickRow::FastForwardSound => s.ff_sound = !s.ff_sound,
            QuickRow::GbaColourCorrection => {
                s.colour_correction_gba = !s.colour_correction_gba;
                self.colour_pending = Some(self.colour_correction());
            }
            QuickRow::GbColourCorrection => {
                s.colour_correction_gb = !s.colour_correction_gb;
                self.colour_pending = Some(self.colour_correction());
            }
            QuickRow::GbcColourCorrection => {
                s.colour_correction_gbc = !s.colour_correction_gbc;
                self.colour_pending = Some(self.colour_correction());
            }
            QuickRow::GbaShader => {
                let to = s.shader_gba.step(gba_shaders, right);
                if to == s.shader_gba {
                    return;
                }
                s.shader_gba = to;
            }
            QuickRow::GbShader => {
                let to = s.shader_gb.step(&Shader::GB, right);
                if to == s.shader_gb {
                    return;
                }
                s.shader_gb = to;
            }
            QuickRow::GbcShader => {
                let to = s.shader_gbc.step(&Shader::GB, right);
                if to == s.shader_gbc {
                    return;
                }
                s.shader_gbc = to;
            }
            QuickRow::Rumble => s.rumble = !s.rumble,
            QuickRow::LeftStick => s.left_stick_dpad = !s.left_stick_dpad,
            QuickRow::AutoLoad => s.auto_load = !s.auto_load,
            QuickRow::EjectSave => s.eject_save = !s.eject_save,
            QuickRow::Turbo => s.turbo = !s.turbo,
            QuickRow::Rewind => s.rewind = !s.rewind,
            QuickRow::GbaBlending => {
                s.blending_gba = s.blending_gba.step(&Blending::MGBA, right);
                self.blending_pending = Some(self.blending_in_play());
            }
            QuickRow::GbcBlending => {
                s.blending_gbc = s.blending_gbc.step(&Blending::MGBA, right);
                self.blending_pending = Some(self.blending_in_play());
            }
            QuickRow::GbBlending => {
                s.blending_gb = s.blending_gb.step(&Blending::MGBA, right);
                self.blending_pending = Some(self.blending_in_play());
            }
            QuickRow::Theme => s.appearance = s.appearance.toggle(),
            QuickRow::WallpaperTint => s.wallpaper_tint = s.wallpaper_tint.step(right),
            QuickRow::ScreenGeneral
            | QuickRow::Brightness
            | QuickRow::BlueLight
            | QuickRow::Saturation
            | QuickRow::Contrast
            | QuickRow::PlayBrightness
            | QuickRow::PlayBlueLight
            | QuickRow::PlaySaturation
            | QuickRow::PlayContrast
            | QuickRow::Wallpaper
            | QuickRow::Recents
            | QuickRow::ShowTitles
            | QuickRow::DateTime
            | QuickRow::About
            | QuickRow::Screen
            | QuickRow::Game
            | QuickRow::Resume
            | QuickRow::UndoSave
            | QuickRow::UndoLoad
            | QuickRow::UndoReset
            | QuickRow::States
            | QuickRow::Screenshots
            | QuickRow::Guide
            | QuickRow::Settings
            | QuickRow::ExitGame
            | QuickRow::PlayScreen
            | QuickRow::PlayGameplay
            | QuickRow::Shader
            | QuickRow::ColourCorrection
            | QuickRow::Palette
            | QuickRow::GbPalette
            | QuickRow::ScreenGba
            | QuickRow::ScreenGbc
            | QuickRow::ScreenGb
            | QuickRow::GbBezel
            | QuickRow::GbcBezel
            | QuickRow::GbaBezel
            | QuickRow::GbaScale
            | QuickRow::GbScale
            | QuickRow::GbcScale
            | QuickRow::GbStretch
            | QuickRow::GbcStretch
            | QuickRow::Blending
            | QuickRow::Scale
            | QuickRow::Stretch
            | QuickRow::Bezel
            | QuickRow::General
            | QuickRow::ShelfPrev
            | QuickRow::ShelfNext
            | QuickRow::ShelfFavorite
            | QuickRow::ShelfForget
            | QuickRow::PlayControls
            | QuickRow::Shortcuts
            | QuickRow::GameUp
            | QuickRow::GameDown
            | QuickRow::GameLeft
            | QuickRow::GameRight
            | QuickRow::GameA
            | QuickRow::GameB
            | QuickRow::GameSelect
            | QuickRow::GameStart
            | QuickRow::GameL
            | QuickRow::GameR
            | QuickRow::GameTurboA
            | QuickRow::GameTurboB
            | QuickRow::ScSave
            | QuickRow::ScLoad
            | QuickRow::ScScreenshot
            | QuickRow::ScBrightUp
            | QuickRow::ScBrightDown
            | QuickRow::ScBlueDown
            | QuickRow::ScBlueUp
            | QuickRow::ScColour
            | QuickRow::ScPaletteBack
            | QuickRow::ScPaletteNext
            | QuickRow::ScShaderNext
            | QuickRow::ScShaderBack
            | QuickRow::ScFastForward
            | QuickRow::ScFastForwardToggle
            | QuickRow::ScRewind
            | QuickRow::ScSunlightUp
            | QuickRow::ScSunlightDown
            | QuickRow::ScReset
            | QuickRow::ScGuide
            | QuickRow::Controls
            | QuickRow::ControlsShelf
            | QuickRow::ControlsInGame
            | QuickRow::LidOff
            | QuickRow::FetchArt
            | QuickRow::Network
            | QuickRow::WifiPower
            | QuickRow::Networks
            | QuickRow::CheckUpdates
            | QuickRow::Server => return,
        }
        self.persist();
    }

    pub fn apply_at(&mut self, action: Action, now: Millis) {
        self.clock = now as f64;
        self.apply(action);
    }

    fn step_shader(&mut self, right: bool) {
        if !matches!(self.phase, Phase::Inserting { .. } | Phase::Playing { .. }) {
            return;
        }
        let gba_shaders = self.gba_shaders();
        let (shader, choices) = match self.playing_platform() {
            Platform::Gba | Platform::Gbav => (&mut self.state.shader_gba, gba_shaders),
            Platform::Gb => (&mut self.state.shader_gb, &Shader::GB[..]),
            Platform::Gbc => (&mut self.state.shader_gbc, &Shader::GB[..]),
        };
        *shader = shader.step(choices, right);
        let said = match *shader {
            Shader::Off => Toast::ShaderOff,
            Shader::Lcd3x => Toast::ShaderLcd3x,
            Shader::Grid => Toast::ShaderGrid,
            Shader::Dot => Toast::ShaderDot,
            Shader::Simpletex => Toast::ShaderSimpletex,
            Shader::Shimmerless => Toast::ShaderShimmerless,
        };
        self.persist();
        self.hud.toast(said, self.now());
    }

    fn adjust(&mut self, action: Action) -> bool {
        if action == Action::MuteToggle {
            self.mute_toggle();
            return true;
        }
        if self.in_game_menu.is_some()
            && matches!(
                action,
                Action::ShaderNext
                    | Action::ShaderPrev
                    | Action::PaletteNext
                    | Action::PalettePrev
                    | Action::ColourCorrectionToggle
            )
        {
            return true;
        }
        if let Action::ShaderNext | Action::ShaderPrev = action {
            self.step_shader(action == Action::ShaderNext);
            return true;
        }
        if let Action::PaletteNext | Action::PalettePrev = action {
            if self.palette_live && matches!(self.phase, Phase::Playing { .. }) {
                let to = match action {
                    Action::PalettePrev => self.state.gb_colors().prev(),
                    _ => self.state.gb_colors().next(),
                };
                self.state.set_gb_colors(to);
                self.palette_pending = Some(to);
                self.persist();
                self.hud.toast(Toast::Palette(to), self.now());
            }
            return true;
        }
        if action == Action::ColourCorrectionToggle {
            let row = match self.playing_platform() {
                Platform::Gba | Platform::Gbav => QuickRow::GbaColourCorrection,
                Platform::Gb => QuickRow::GbColourCorrection,
                Platform::Gbc => QuickRow::GbcColourCorrection,
            };
            self.change_setting(row, true);
            let said = match self.colour_correction() {
                true => Toast::ColourOn,
                false => Toast::ColourOff,
            };
            self.hud.toast(said, self.now());
            return true;
        }
        if let Action::SunlightUp | Action::SunlightDown = action {
            self.step_sunlight(action == Action::SunlightUp);
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
            Action::BrightnessUp => (HudKind::Brightness, up(s.brightness, 1, BRIGHTNESS_MAX)),
            Action::BrightnessDown => (HudKind::Brightness, s.brightness.saturating_sub(1)),
            Action::BlueLightUp => (HudKind::BlueLight, up(s.blue_light, 1, BLUE_LIGHT_MAX)),
            Action::BlueLightDown => (HudKind::BlueLight, s.blue_light.saturating_sub(1)),
            Action::VolumeUp => (HudKind::Volume, up(self.level(), VOLUME_STEP, VOLUME_MAX)),
            Action::VolumeDown => (HudKind::Volume, self.level().saturating_sub(VOLUME_STEP)),
            _ => return false,
        };
        if kind == HudKind::Volume {
            self.remember_volume();
        }
        let unmuted = kind == HudKind::Volume && std::mem::take(self.muted_mut());
        let moved = self.set_level(kind, value);
        let (shown, now) = (self.hud_value(kind, value), self.now());
        self.hud.show(kind, shown, self.muted(), now);
        if unmuted && !moved {
            self.persist_soon();
        }
        true
    }

    fn set_level(&mut self, kind: HudKind, value: u8) -> bool {
        let level = match kind {
            HudKind::Brightness => &mut self.state.brightness,
            HudKind::BlueLight => &mut self.state.blue_light,
            HudKind::Volume if self.headphones => &mut self.state.volume_hp,
            HudKind::Volume => &mut self.state.volume,
            HudKind::Rewind | HudKind::Sunlight => return false,
        };
        let moved = *level != value;
        *level = value;
        if let (HudKind::Brightness, Some(power)) = (kind, &mut self.power) {
            power.set_backlight(value + 1);
        }
        if moved {
            self.persist_soon();
        }
        moved
    }

    fn remember_volume(&mut self) {
        if self.vol_before.len() == 2 {
            self.vol_before.remove(0);
        }
        self.vol_before
            .push((self.level(), self.muted(), self.now()));
    }

    fn mute_toggle(&mut self) {
        let now = self.now();
        if let Some((volume, muted, _)) = self
            .vol_before
            .iter()
            .find(|(_, _, at)| now.saturating_sub(*at) <= MUTE_CHORD_MS)
            .copied()
        {
            *self.level_mut() = volume;
            *self.muted_mut() = muted;
        }
        self.vol_before.clear();
        let muted = !self.muted();
        *self.muted_mut() = muted;
        self.hud
            .show(HudKind::Volume, self.output_volume(), muted, now);
        self.persist_soon();
    }

    fn hud_value(&self, kind: HudKind, value: u8) -> u8 {
        match kind {
            HudKind::Volume => self.output_volume(),
            _ => value,
        }
    }

    pub fn show_rewind(&mut self, fill: u8) {
        let now = self.now();
        self.hud.show(HudKind::Rewind, fill, false, now);
    }

    pub fn hide_rewind(&mut self) {
        self.hud.release_rewind();
    }

    pub fn set_ff(&mut self, ff: FfState) {
        self.hud.set_ff(ff);
    }

    pub fn ff_badge(&self) -> Option<Icon> {
        self.hud.badge()
    }

    pub fn blue_light(&self) -> u8 {
        self.state.blue_light
    }

    pub fn saturation(&self) -> u8 {
        self.state.saturation
    }

    pub fn contrast(&self) -> u8 {
        self.state.contrast
    }

    pub fn brightness(&self) -> u8 {
        self.state.brightness
    }

    pub fn volume(&self) -> u8 {
        self.level()
    }

    fn level(&self) -> u8 {
        if self.headphones {
            self.state.volume_hp
        } else {
            self.state.volume
        }
    }

    fn level_mut(&mut self) -> &mut u8 {
        if self.headphones {
            &mut self.state.volume_hp
        } else {
            &mut self.state.volume
        }
    }

    pub fn headphones(&self) -> bool {
        self.headphones
    }

    pub fn muted(&self) -> bool {
        if self.headphones {
            self.state.muted_hp
        } else {
            self.state.muted
        }
    }

    fn muted_mut(&mut self) -> &mut bool {
        if self.headphones {
            &mut self.state.muted_hp
        } else {
            &mut self.state.muted
        }
    }

    pub fn output_volume(&self) -> u8 {
        if self.muted() {
            0
        } else {
            self.level()
        }
    }

    pub fn hud_icon(&self) -> Icon {
        self.hud.glyph()
    }

    pub fn now(&self) -> Millis {
        self.clock as Millis
    }

    fn flick(&mut self, step: fn(&mut Polaroids)) {
        if let Some(p) = &mut self.polaroids {
            step(p);
        }
    }

    pub fn update(&mut self, dt: f32) {
        self.clock += dt as f64 * 1000.0;
        if self.name_pending && self.shelf_platform.face.is_some() {
            self.name_pending = false;
            self.shelf_named = Some(self.now());
        }
        self.timers();
        self.poll_link();
        let now = self.now();
        let ready = self.core_faces_ready();
        if let Some(picker) = &mut self.core_picker {
            if picker.waiting() && (ready || picker.waited(now) >= FACES_WAIT_MS) {
                picker.start(now);
            }
        }
        if self.core_picker.is_some_and(|p| p.finished(now)) {
            self.core_picker = None;
        }
        if !self.on_shelf() {
            self.shelf_mut().release_hold();
        }
        let mut touched = false;
        let at = self.shelf_at;
        let next = match &mut self.phase {
            Phase::Shelf => {
                let shelf = &mut self.shelves[at].1;
                shelf.tick(now);
                shelf.update(dt);
                None
            }
            Phase::Inserting {
                cart,
                t,
                core_ready,
                resumed,
                ..
            } => {
                let was = *t;
                *t += dt;
                let at = SEATED_AT - Sfx::Insert.lead();
                touched = !*resumed && was < at && *t >= at;
                (*t >= INSERT_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += dt;
                    touched = was < 0.0 && *t >= 0.0;
                }
                (*t >= EJECT_S).then_some(Phase::Shelf)
            }
            _ => None,
        };
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => Sfx::Eject,
                _ => Sfx::Insert,
            });
        }
        if let Some(phase) = next {
            self.phase = phase;
            self.refused_from = None;
            let seated = match &self.phase {
                Phase::Playing { cart } => Some(cart.clone()),
                _ => None,
            };
            if seated.is_none() {
                self.link_loaded = None;
            }
            self.record_cart(seated);
        }
        self.step_screen(dt);
        self.step_footer(dt);
    }

    fn step_screen(&mut self, dt: f32) {
        self.blank_s = match self.blank_panel() {
            Some(_) => self.blank_s + dt,
            None => 0.0,
        };
        let lit = matches!(self.phase, Phase::Playing { .. } | Phase::Polaroids { .. })
            || self.powering_up();
        let step = if lit {
            dt / POWER_ON_S
        } else {
            -dt / POWER_OFF_S
        };
        self.screen = (self.screen + step).clamp(0.0, 1.0);
        let docked = matches!(
            self.phase,
            Phase::Shelf | Phase::Inserting { .. } | Phase::Ejecting { .. }
        );
        let sliding = matches!(
            self.phase,
            Phase::Inserting { .. }
                | Phase::Playing { .. }
                | Phase::Polaroids { .. }
                | Phase::Ejecting { .. }
        );
        let reach = if sliding { dt / POWER_ON_S } else { 1.0 };
        self.lift = match docked {
            true => (self.lift - reach).max(0.0),
            false => (self.lift + reach).min(1.0),
        };
    }

    fn step_footer(&mut self, dt: f32) {
        let shown = match &self.phase {
            Phase::Playing { .. } => self.in_game_menu_shown(),
            // The carousel already holds this strip, so the menu does not slide it.
            Phase::QuickMenu { row } => {
                self.footer = 1.0;
                self.footer_row = Some(*row);
                return;
            }
            Phase::Ejecting { .. } => {
                self.footer = match self.screen > 0.0 {
                    true => (self.footer + dt / POWER_OFF_S).min(1.0),
                    false => 1.0,
                };
                self.footer_row = None;
                return;
            }
            _ => {
                self.footer = 0.0;
                self.footer_row = None;
                return;
            }
        };
        if let Some(row) = shown {
            self.footer_row = Some(row);
        }
        let target = if shown.is_some() { 1.0 } else { 0.0 };
        let step = dt / FOOTER_SLIDE_S;
        self.footer = if target > self.footer {
            (self.footer + step).min(target)
        } else {
            (self.footer - step).max(target)
        };
        if self.footer == 0.0 {
            self.footer_row = None;
        }
    }

    fn menu_footer_offset(&self) -> f32 {
        (1.0 - ease(self.footer)) * MOUTH_H
    }

    /// The carousel strip shares this canvas, so the picture scales in the space above it.
    /// A 4:3 game frame is a different canvas; its close keeps the full-frame scale.
    fn footer_on_carousel_canvas(&self) -> bool {
        !self.framed_panel() || !self.shows_game()
    }

    pub fn screen_footer(&self) -> f32 {
        if matches!(self.phase, Phase::Ejecting { .. })
            && self.screen > 0.0
            && self.footer_on_carousel_canvas()
        {
            MOUTH_H * ease(self.footer)
        } else {
            0.0
        }
    }

    pub fn set_panel(&mut self, panel: (u32, u32)) {
        self.panel = panel;
    }

    pub fn wallpaper_size(&self) -> (u32, u32) {
        (Canvas::of(self.panel).w as u32, OUT_H)
    }

    pub fn game_bezel_size(&self) -> (u32, u32) {
        match slot_gfx::framed(self.panel) {
            true => self.panel,
            false => (OUT_W, OUT_H),
        }
    }

    pub fn shows_game(&self) -> bool {
        if matches!(self.phase, Phase::Playing { .. }) && self.in_game_menu_shown().is_some() {
            return false;
        }
        if self.powering_up() {
            return true;
        }
        match &self.phase {
            Phase::Playing { .. } | Phase::Polaroids { .. } => true,
            Phase::Ejecting { .. } => self.overlay_slide() > 0.0,
            Phase::Doze { cart } => cart.is_some(),
            _ => false,
        }
    }

    pub fn canvas(&self) -> Canvas {
        Canvas::of(self.panel)
    }

    pub fn set_sp_strip(&mut self, tex: Option<TexId>) {
        self.sp_strip = tex;
    }

    fn draw_sp_strip(&self, out: &mut Vec<Draw>) {
        let slide = self.overlay_slide();
        let shown = self.shows_game() && self.sp_frame() && self.framed_panel() && slide > 0.0;
        let Some(tex) = self.sp_strip.filter(|_| shown) else {
            return;
        };
        let canvas = self.canvas();
        let drop =
            slot_gfx::lifted_rect(self.panel, slide).1 as f32 * canvas.h / self.panel.1 as f32;
        out.push(Draw::Tex {
            x: 0.0,
            y: drop,
            w: canvas.w,
            h: canvas.h,
            tex,
            alpha: 1.0,
        });
    }

    pub fn overlay_slide(&self) -> f32 {
        match &self.phase {
            Phase::Ejecting { t, .. } => ((-t - (EJECT_HOLD_S - POWER_ON_S)) / POWER_ON_S)
                .clamp(0.0, 1.0)
                .min(self.eject_overlay),
            Phase::Playing { .. } | Phase::Polaroids { .. } | Phase::Doze { cart: Some(_) } => {
                self.lift
            }
            _ => 0.0,
        }
    }

    fn framed_panel(&self) -> bool {
        slot_gfx::framed(self.panel)
    }

    fn draw_powering(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas().w,
            h: OUT_H as f32,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        if self.panel_lit() && self.screen > 0.0 {
            out.push(Draw::Game);
        }
    }

    fn powering_up(&self) -> bool {
        matches!(self.phase, Phase::Inserting { t, resumed: false, .. } if t >= INSERT_S)
    }

    pub fn blank_panel(&self) -> Option<[u8; 3]> {
        let waiting = self.powering_up()
            || (matches!(self.phase, Phase::Playing { .. }) && self.blank_s > 0.0);
        if self.game_ready || !waiting {
            return None;
        }
        let lit = match self.playing_platform() {
            Platform::Gb => [0x9b, 0xa5, 0x82],
            _ => [0xe6, 0xe4, 0xda],
        };
        let k = (self.blank_s / BLANK_WARM_S).min(1.0);
        let level = BLANK_DIM + (1.0 - BLANK_DIM) * (1.0 - (1.0 - k) * (1.0 - k));
        Some(lit.map(|c| (c as f32 * level) as u8))
    }

    fn panel_lit(&self) -> bool {
        self.game_ready || self.blank_panel().is_some()
    }

    pub fn screen_power(&self) -> f32 {
        self.screen
    }

    pub fn set_game_ready(&mut self, ready: bool) {
        self.game_ready = ready;
    }

    pub fn game_ready(&self) -> bool {
        self.game_ready
    }

    pub fn game_visible(&self) -> bool {
        self.panel_lit() && self.screen > 0.0
    }

    pub fn tick_ms(&mut self, now: Millis) {
        self.clock = self.clock.max(now as f64);
        self.timers();
    }

    pub fn set_net_time_source(&mut self, source: crate::net_time::Source) {
        self.net_time = crate::net_time::NetTime::new(source);
    }

    fn step_net_time(&mut self) {
        let ready = self.state.network_time && self.wifi_up();
        let now = self.now();
        let Some(utc) = self.net_time.step(ready, now) else {
            return;
        };
        if let Some(power) = &mut self.power {
            if (utc - power.now()).abs() >= CLOCK_DRIFT_SECS {
                power.set_clock_in_background(utc);
            }
        }
    }

    fn timers(&mut self) {
        self.step_server();
        self.step_net_time();
        self.take_art_events();
        self.take_update_events();
        self.play_hold();
        self.repeat_held_dir();
        self.take_shot_list();
        if self.shot_writer.take_failures() > 0 {
            self.hud.toast(Toast::ShotFailed, self.now());
        }
        let (now, canvas) = (self.now(), self.canvas());
        if let Some(v) = &mut self.shot_viewer {
            v.tick(now, canvas);
        }
        let offer = self.undo_label();
        if let Some(p) = &mut self.polaroids {
            p.set_undo(offer);
        }
        if let Some(row @ (QuickRow::UndoSave | QuickRow::UndoLoad | QuickRow::UndoReset)) =
            self.in_game_menu
        {
            if self.in_game_hidden().contains(&row) {
                self.in_game_menu = Some(QuickRow::Resume);
            }
        }
        if self.doze_expired() {
            self.on_doze_timeout();
        }
        self.settle_loading_mark();
        if self.now() >= self.autosave_at.min(self.resume_at) {
            self.autosave();
        }
        if self.now() >= self.card_check_at {
            self.card_check_at = self.now() + CARD_CHECK_MS;
            self.card_read_only = self.power.as_ref().is_some_and(|p| p.card_read_only());
        }
        let failing = self.card_writes.failing() || self.card_read_only;
        if failing != self.hud.alarmed() {
            eprintln!(
                "slot: card: alarm {} (read-only {})",
                if failing { "on" } else { "off" },
                self.card_read_only
            );
            self.log_card(failing);
        }
        self.hud.set_alarm(failing, self.now());
        self.hud
            .set_alarm_toast(match (self.repair_possible(), self.charged_to_repair()) {
                (true, true) => Toast::CardRepair,
                (true, false) => Toast::CardRepairCharge,
                (false, _) => Toast::CardUnwritable,
            });
        if self.now() >= self.battery_at {
            self.battery_at = self.now() + BATTERY_POLL_MS;
            self.battery = self.power.as_ref().and_then(|p| p.battery());
            if let Some(b) = self.battery {
                self.on_battery(b);
            }
        }
        if self.now() >= self.headphones_at {
            self.headphones_at = self.now() + HEADPHONES_POLL_MS;
            let on = self.power.as_ref().is_some_and(|p| p.headphones());
            if on != self.headphones {
                self.headphones = on;
                self.hud.set_headphones(on);
                let now = self.now();
                self.hud
                    .show(HudKind::Volume, self.output_volume(), self.muted(), now);
            }
        }
        if self.now() >= self.charge_at {
            self.charge_at = self.now() + CHARGE_POLL_MS;
            if let (Some(power), Some(b)) = (self.power.as_ref(), self.battery.as_mut()) {
                b.charge = power.charge();
            }
            let state = self.led_state();
            self.set_led(state);
        }
        if let Some(GameMenu::Linked {
            since,
            opened: false,
            ..
        }) = self.game_menu
        {
            if self.now().saturating_sub(since) >= LINKED_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(GameMenu::Unplug { since, .. }) = self.game_menu {
            if self.now().saturating_sub(since) >= UNPLUG_HOLD_MS {
                self.game_menu = None;
            }
        }
        if let Some(at) = self.link.as_ref().and_then(|s| s.lost_at) {
            if self.now().saturating_sub(at) >= LINK_LOST_MS {
                self.end_link();
            }
        }
    }

    pub fn led_state(&self) -> LedState {
        let Some(b) = self.battery else {
            return LedState::Running;
        };
        match b.charge {
            Charge::Charging => LedState::Charging,
            Charge::Full => LedState::Charged,
            _ if b.percent <= BATTERY_LOW => LedState::Low,
            _ => LedState::Running,
        }
    }

    fn set_led(&mut self, state: LedState) {
        if self.shutting_down() && state != LedState::Off {
            return;
        }
        if self.last_led == Some(state) {
            return;
        }
        self.last_led = Some(state);
        if let Some(power) = self.power.as_mut() {
            power.set_led(state);
        }
    }

    fn restock_recents(&mut self) {
        let carts: Vec<Cart> = match self.state.show_recents {
            true => self
                .state
                .recents
                .iter()
                .filter_map(|r| {
                    self.carts()
                        .find(|c| c.platform == r.platform && c.stem == r.stem)
                })
                .cloned()
                .collect(),
            false => Vec::new(),
        };
        self.restock(RECENTS_AT, carts);
        self.mark_hearts();
    }

    fn restock_favorites(&mut self) {
        let mut carts: Vec<Cart> = self
            .carts()
            .filter(|c| self.state.is_favorite(c.platform, &c.stem))
            .cloned()
            .collect();
        carts.sort_by(|a, b| {
            (slot_store::sort_key(&a.stem), a.platform as u8)
                .cmp(&(slot_store::sort_key(&b.stem), b.platform as u8))
        });
        let index = self.shelves[FAVORITES_AT].1.index;
        self.restock(FAVORITES_AT, carts);
        let shelf = &mut self.shelves[FAVORITES_AT].1;
        if self.shelf_at == FAVORITES_AT && !shelf.carts.is_empty() {
            shelf.select(index.min(shelf.carts.len() - 1));
        }
        self.mark_hearts();
    }

    fn restock(&mut self, at: usize, carts: Vec<Cart>) {
        let stocked = &self.shelves[at].1.carts;
        if stocked.len() == carts.len()
            && stocked
                .iter()
                .zip(&carts)
                .all(|(a, b)| a.platform == b.platform && a.stem == b.stem)
        {
            return;
        }
        let old = self.shelves[at].1.restock(carts);
        self.orphaned.extend(old);
        let empty = self.shelves[at].1.carts.is_empty();
        if self.shelf_at == at && empty {
            self.shelf_at = self.next_shelf(1).unwrap_or(0);
        }
    }

    fn toggle_favorite(&mut self) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        let (platform, stem) = (cart.platform, cart.stem.clone());
        self.state.toggle_favorite(platform, &stem);
        let shelf_at = self.shelf_at;
        self.restock_favorites();
        if self.shelf_at != shelf_at {
            self.shelf_platform = slot_ui::Printed::default();
            self.slot_letter = None;
            self.shelf_named = None;
            self.name_pending = true;
        }
        self.persist();
    }

    fn forget_recent(&mut self) {
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        let (platform, stem) = (cart.platform, cart.stem.clone());
        self.state.forget(platform, &stem);
        let index = self.shelves[RECENTS_AT].1.index;
        self.restock_recents();
        let shelf = &mut self.shelves[RECENTS_AT].1;
        if self.shelf_at == RECENTS_AT && !shelf.carts.is_empty() {
            shelf.select(index.min(shelf.carts.len() - 1));
        } else {
            self.shelf_platform = slot_ui::Printed::default();
            self.slot_letter = None;
            self.shelf_named = None;
            self.name_pending = true;
        }
        self.persist();
    }

    fn record_cart(&mut self, cart: Option<String>) {
        let platform = self.seated_cart().map(|c| c.platform);
        if let (Some(stem), Some(platform)) = (&cart, platform) {
            let first = self.state.recents.first();
            if first.is_none_or(|r| r.platform != platform || r.stem != *stem) {
                let stem = stem.clone();
                self.state.played(platform, &stem);
                self.restock_recents();
                self.persist();
            }
        }
        if self.state.cart == cart && self.state.cart_platform == platform {
            return;
        }
        self.state.cart = cart;
        self.state.cart_platform = platform;
        self.persist();
    }

    fn persist(&self) {
        let Some(root) = self.root.as_ref().filter(|_| self.server.is_none()) else {
            return;
        };
        self.state_writer.flush();
        let written = write_slot_state(root, &self.state);
        if let Err(e) = &written {
            eprintln!("slot: slot.state: {e}");
        }
        self.card_writes.wrote("slot.state", &written);
    }

    fn persist_soon(&self) {
        if let Some(root) = self.root.as_ref().filter(|_| self.server.is_none()) {
            self.state_writer.save(root, &self.state);
        }
    }

    pub fn flush_state(&self) {
        self.state_writer.flush();
    }

    pub fn on_core_ready(&mut self) {
        let Phase::Inserting {
            cart, core_ready, ..
        } = &mut self.phase
        else {
            return;
        };
        *core_ready = true;
        let cart = cart.clone();
        self.retire_refused_resume(&cart);
    }

    fn retire_refused_resume(&mut self, stem: &str) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        if snapshot.resume_trusted() || self.resume_unread {
            return;
        }
        if !self.named_core {
            return;
        }
        let Some(root) = &self.root else {
            return;
        };
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
        let ring = StateRing::new(root, self.platform, self.core, stem);
        match ring.retire_resume(&format_stamp(self.wall_secs())) {
            Ok(Some(to)) => eprintln!(
                "slot: resume: {} refused this state, moved it to {}",
                self.core.as_str(),
                to.display()
            ),
            Ok(None) => {}
            Err(e) => eprintln!("slot: resume: could not move the refused state aside: {e}"),
        }
    }

    pub fn on_core_failed(&mut self) {
        let caught = self.seat();
        let Phase::Inserting { cart, .. } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.refuse_out(cart, caught);
    }

    fn refuse_out(&mut self, cart: String, caught: f32) {
        let t = (1.0 - caught) * EJECT_S;
        self.phase = Phase::Ejecting { cart, t };
        self.refused_from = Some(t);
    }

    pub fn refuse(&mut self) {
        self.refusal = Some(Refusal::started(self.now()));
    }

    pub fn refusal_active(&self, now: Millis) -> bool {
        self.refusal.is_some_and(|r| r.active(now))
    }

    pub fn seat(&self) -> f32 {
        match &self.phase {
            Phase::Shelf => 0.0,
            Phase::Inserting { t, resumed, .. } => {
                if *resumed {
                    1.0
                } else {
                    (t / SEATED_AT).clamp(0.0, 1.0)
                }
            }
            Phase::Ejecting { t, .. } => 1.0 - (t / EJECT_S).clamp(0.0, 1.0),
            _ => 1.0,
        }
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        match self.server_frame() {
            Some((frame, tex)) => self.draw_server(&frame, &tex, out),
            None => self.draw_app(out),
        }
    }

    fn draw_app(&self, out: &mut Vec<Draw>) {
        if self.shutting_down() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: self.canvas().w,
                h: OUT_H as f32,
                colour: [0.0, 0.0, 0.0, 1.0],
            });
            let which = if self.repairing {
                PowerChoice::Repair
            } else if self.restarting {
                PowerChoice::Restart
            } else {
                PowerChoice::PowerOff
            };
            if let Some((tex, w, h)) = self.shutdown_faces.get(which.index()).copied() {
                out.push(Draw::Tex {
                    x: self.canvas().centred(w as f32).floor(),
                    y: ((OUT_H - h) / 2) as f32,
                    w: w as f32,
                    h: h as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        if self.fetch_art.is_some() && !matches!(self.phase, Phase::Doze { .. }) {
            self.draw_fetch_art(out);
            return self.hud.draw(self.canvas(), self.now(), out);
        }
        if self.studio.is_some() && !matches!(self.phase, Phase::Doze { .. }) {
            self.draw_studio(out);
            return self.hud.draw(self.canvas(), self.now(), out);
        }
        if self.update.is_some() {
            return self.draw_update(out);
        }
        if self.repair_prompt {
            return self.draw_prompt(&self.repair_faces, out);
        }
        if let Some(row) = self.reset_prompt {
            return self.draw_prompt(&self.reset_prompt_faces(row), out);
        }
        if self.forget_prompt {
            return self.draw_prompt(&self.forget_faces, out);
        }
        if self.delete_prompt {
            return match self.shot_viewer {
                Some(_) => {
                    let mut faces = self.delete_shot_faces.clone();
                    if let Some(text) = self.delete_shot_text {
                        for (face, line) in faces.iter_mut().zip(text) {
                            *face = line;
                        }
                    }
                    self.draw_prompt(&faces, out)
                }
                None => self.draw_prompt(&self.delete_faces, out),
            };
        }
        match &self.phase {
            Phase::SetClock {
                picker, from_menu, ..
            } => {
                ClockScreen {
                    picker,
                    faces: self.clock_faces.as_ref(),
                    title: self.clock_title.filter(|_| !from_menu),
                    pointer: self.quick_menu_faces.as_ref().map(|q| q.pointer),
                    ground: self.menu_ground(),
                    canvas: self.canvas(),
                }
                .draw(out);
                slot_ui::draw_plugged_slot(self.canvas(), self.plugged_face, 1.0, out);
                self.draw_clock_legends(picker, *from_menu, out);
                return;
            }
            Phase::QuickMenu { row } => QuickMenu {
                row: *row,
                hidden: &self.hidden_rows(),
                bound: Some(&self.bound_faces()),
                description: self.description(*row),
                values: QuickRow::ALL.map(|r| self.quick_value(r)),
                clock: self.quick_clock_faces,
                wallpaper: self.quick_wallpaper_faces,
                networks: self.quick_network_faces,
                faces: self.quick_menu_faces.as_ref(),
                ground: self.menu_ground(),
                palette: self.quick_palette_faces,
                gba_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gba)],
                gb_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gb)],
                gbc_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gbc)],
                bezel: None,
                over_game: false,
                footer_top: OUT_H as f32,
                canvas: self.canvas(),
            }
            .draw(out),
            Phase::Shelf => {
                self.draw_background(out);
                match (self.core_picker_shown(), self.selected_stem()) {
                    (Some(picker), Some(stem)) => {
                        let open = ease(picker.openness(self.now()));
                        let dim = 1.0 + (CORE_PICKER_DIM - 1.0) * open;
                        self.shelf().draw_row(
                            self.canvas(),
                            Some(stem),
                            0.0,
                            CORE_PICKER_RECEDE * open,
                            dim,
                            out,
                        );
                        draw_empty_slot(self.canvas(), self.slot_faces, out);
                    }
                    _ => {
                        self.shelf()
                            .draw(self.canvas(), self.shelf_shake(), self.slot_faces, out);
                        slot_ui::draw_slot_title(
                            self.canvas(),
                            self.shelf_platform,
                            self.slot_name_alpha(),
                            out,
                        );
                        slot_ui::draw_slot_title(
                            self.canvas(),
                            self.shelf_title,
                            self.title_alpha(),
                            out,
                        );
                    }
                }
                draw_footer(
                    self.canvas(),
                    self.battery,
                    self.battery_percent,
                    self.bolt,
                    self.shelf_clock,
                    footer_ink(self.light()),
                    out,
                );
                if self.wifi_up() {
                    slot_ui::draw_wifi_icon(self.battery_percent, self.wifi_face(), out);
                }
                if let Some(picker) = self.core_picker_shown() {
                    let lift = lift_of(picker.openness(self.now()));
                    slot_ui::draw_plugged_slot(self.canvas(), self.plugged_face, lift, out);
                }
            }
            Phase::About => {
                slot_ui::draw_menu_ground(self.canvas(), self.menu_ground(), out);
                draw_sticker(self.canvas(), self.sticker_face, out);
                self.draw_menu_footer(out);
                return;
            }
            Phase::Networks { .. } | Phase::NetworkScan { .. } | Phase::NetworkPage { .. } => {
                self.draw_networks(out);
                return self.hud.draw(self.canvas(), self.now(), out);
            }
            Phase::Inserting { .. } if self.powering_up() && self.framed_panel() => {
                self.draw_powering(out)
            }
            Phase::Inserting { cart, .. } if self.powering_up() => {
                self.draw_background(out);
                self.chrome(cart, 0.0, out);
            }
            Phase::Inserting { cart, resumed, .. } => {
                if !resumed {
                    self.draw_background(out);
                    self.shelf()
                        .draw_row(self.canvas(), Some(cart), 0.0, self.seat(), 1.0, out);
                }
                self.chrome(cart, 0.0, out);
            }
            Phase::Ejecting { .. }
                if self.framed_panel() && (self.shows_game() || self.screen > 0.0) =>
            {
                self.draw_powering(out)
            }
            Phase::Playing { .. } if self.framed_panel() && self.screen < 1.0 => {
                self.draw_powering(out)
            }
            Phase::Ejecting { cart, .. } => {
                self.draw_background(out);
                self.shelf()
                    .draw_row(self.canvas(), Some(cart), 0.0, self.seat(), 1.0, out);
                self.chrome(cart, 0.0, out);
            }
            Phase::Playing { cart } if self.screen < 1.0 => {
                self.draw_background(out);
                self.chrome(cart, 0.0, out);
            }
            Phase::Playing { .. } => self.push_game(out),
            Phase::Polaroids { .. } if self.guide.is_some() => {
                if let Some((g, _)) = &self.guide {
                    g.draw(
                        self.canvas(),
                        self.battery,
                        self.battery_percent,
                        self.hud_bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            Phase::Polaroids { .. } if self.shot_viewer.is_some() => {
                if let Some(v) = &self.shot_viewer {
                    v.draw(
                        self.canvas(),
                        self.battery,
                        self.battery_percent,
                        self.hud_bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            Phase::Polaroids { .. } => {
                self.push_game(out);
                if let Some(p) = &self.polaroids {
                    p.draw_photo(self.canvas(), out);
                    self.draw_sp_strip(out);
                    p.draw_chrome(
                        self.canvas(),
                        self.battery,
                        self.battery_percent,
                        self.hud_bolt,
                        self.shelf_clock,
                        out,
                    );
                }
            }
            Phase::Doze { .. } => {
                out.push(Draw::Rect {
                    x: 0.0,
                    y: 0.0,
                    w: self.canvas().w,
                    h: OUT_H as f32,
                    colour: [0.0, 0.0, 0.0, 1.0],
                });
                self.draw_sp_strip(out);
                return;
            }
        }
        if !matches!(self.phase, Phase::Polaroids { .. }) {
            self.draw_sp_strip(out);
        }
        if self.shot_wanted && matches!(self.phase, Phase::Playing { .. }) && self.game_visible() {
            out.push(Draw::Capture);
        }
        if let (Some(row), Phase::Playing { .. }, true) =
            (self.in_game_menu_shown(), &self.phase, self.screen >= 1.0)
        {
            let hidden = self.in_game_hidden();
            let bound = self.bound_faces();
            QuickMenu {
                row,
                bound: Some(&bound),
                description: self.description(row),
                hidden: &hidden,
                values: QuickRow::ALL.map(|r| self.quick_value(r)),
                clock: None,
                wallpaper: None,
                networks: None,
                faces: self.quick_menu_faces.as_ref(),
                ground: None,
                palette: self.quick_palette_faces,
                gba_bezel: None,
                gb_bezel: None,
                gbc_bezel: None,
                bezel: self.quick_bezel_faces[bezel_slot(self.playing_platform())],
                over_game: true,
                footer_top: OUT_H as f32 - MOUTH_H + self.menu_footer_offset(),
                canvas: self.canvas(),
            }
            .draw(out);
            if row.page() != QuickRow::PLAY_SCREEN {
                slot_ui::draw_corners(
                    self.canvas(),
                    self.battery,
                    self.battery_percent,
                    self.hud_bolt,
                    self.shelf_clock,
                    out,
                );
            }
        }
        self.draw_menu_footer(out);
        self.draw_close_footer(out);
        if let Some(picker) = self.core_picker_shown() {
            self.draw_core_picker(&picker, out);
        }
        if let Some(menu) = self.game_menu {
            self.draw_game_menu(menu, out);
        }
        self.hud.draw(self.canvas(), self.now(), out);
    }

    pub fn screen_shake(&self) -> f32 {
        self.shake_at(self.now())
    }

    pub fn shake_at(&self, now: Millis) -> f32 {
        self.shake_when(matches!(self.phase, Phase::Playing { .. }), now)
    }

    pub fn shelf_shake(&self) -> f32 {
        self.shake_when(self.on_shelf() && self.core_picker.is_none(), self.now())
    }

    fn shake_when(&self, mine: bool, now: Millis) -> f32 {
        if !mine {
            return 0.0;
        }
        self.refusal.map_or(0.0, |r| r.offset(now))
    }

    fn push_game(&self, out: &mut Vec<Draw>) {
        if self.game_visible() {
            out.push(Draw::Game);
        }
    }

    fn draw_menu_footer(&self, out: &mut Vec<Draw>) {
        let carousel = matches!(self.phase, Phase::QuickMenu { .. } | Phase::About);
        let dy = if carousel {
            0.0
        } else {
            self.menu_footer_offset()
        };
        let start = out.len();
        let row = match &self.phase {
            Phase::Playing { cart } if self.footer > 0.0 => {
                let row = self.in_game_menu_shown().or(self.footer_row);
                match row.is_some_and(QuickRow::binds) {
                    true => slot_ui::draw_plugged_slot(self.canvas(), self.plugged_face, 1.0, out),
                    false => self.draw_seated_mouth(cart, out),
                }
                row
            }
            // The carousel menu is not a game, so the mouth stays empty, in the
            // same place the shelf already had it.
            Phase::QuickMenu { row } if row.binds() => {
                slot_ui::draw_plugged_slot(self.canvas(), self.plugged_face, 1.0, out);
                Some(*row)
            }
            Phase::QuickMenu { row } => {
                draw_empty_slot(self.canvas(), self.slot_faces, out);
                Some(*row)
            }
            Phase::About => {
                draw_empty_slot(self.canvas(), self.slot_faces, out);
                self.draw_about_legend(out);
                return;
            }
            _ => return,
        };
        shift_down(&mut out[start..], dy);
        if let Some(row) = row {
            self.draw_menu_legends(row, LINK_LEGEND_Y + dy, out);
        }
    }

    fn draw_close_footer(&self, out: &mut Vec<Draw>) {
        let Phase::Ejecting { cart, .. } = &self.phase else {
            return;
        };
        if !self.footer_on_carousel_canvas() {
            return;
        }
        let start = out.len();
        // The framed power-off paints black and the picture, and never the slot. The other
        // close already draws the seated cart through the chrome.
        if self.framed_panel() && self.screen > 0.0 {
            self.draw_seated_mouth(cart, out);
        }
        draw_footer(
            self.canvas(),
            self.battery,
            self.battery_percent,
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
        if self.wifi_up() {
            slot_ui::draw_wifi_icon(self.battery_percent, self.wifi_face(), out);
        }
        shift_down(&mut out[start..], self.menu_footer_offset());
    }

    fn draw_seated_mouth(&self, stem: &str, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf().find(stem) else {
            return;
        };
        let (rest, scale) = self.shelf().selected_at(self.canvas());
        SlotChrome {
            cart,
            face,
            rest,
            scale,
            seat: 1.0,
            alert: None,
            dim: 0.0,
            screen: 0.0,
            game: false,
            over: true,
            behind: self.shelf().behind(stem),
            slot: self.slot_faces,
            canvas: self.canvas(),
        }
        .draw(out);
    }

    fn draw_prompt(&self, faces: &[(TexId, u32, u32)], out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas().w,
            h: OUT_H as f32,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let [lines @ .., no, yes] = faces else {
            return;
        };
        let gaps = REPAIR_GAP * lines.len().saturating_sub(1) as u32;
        let total = lines.iter().map(|l| l.2).sum::<u32>() + gaps;
        let mut y = (OUT_H.saturating_sub(total) / 2) as f32;
        for &(tex, w, h) in lines {
            out.push(Draw::Tex {
                x: self.canvas().centred(w as f32).floor(),
                y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
            y += (h + REPAIR_GAP) as f32;
        }
        self.draw_side_legend((no.0, no.1), false, LINK_LEGEND_Y, out);
        self.draw_side_legend((yes.0, yes.1), true, LINK_LEGEND_Y, out);
    }

    fn draw_about_legend(&self, out: &mut Vec<Draw>) {
        let Some(faces) = &self.quick_menu_faces else {
            return;
        };
        let [back, _, _, _, restart, ..] = match self.light() {
            true => faces.legend_light,
            false => faces.legend,
        };
        self.draw_side_legend(back, false, LINK_LEGEND_Y, out);
        self.draw_side_legend(restart, true, LINK_LEGEND_Y, out);
    }

    fn draw_menu_legends(&self, row: QuickRow, y: f32, out: &mut Vec<Draw>) {
        let Some(faces) = &self.quick_menu_faces else {
            return;
        };
        let [back, change, open, resume, _, set, clear, reset] = match self.light() {
            true => faces.legend_light,
            false => faces.legend,
        };
        if row.binds() {
            let legends = match self.page_is_default(row) {
                true => vec![back, clear, set],
                false => vec![back, reset, clear, set],
            };
            self.draw_spread_legends(&legends, y, 1.0, out);
            return;
        }
        let left = match row.page() == QuickRow::PLAY {
            true => resume,
            false => back,
        };
        self.draw_side_legend(left, false, y, out);
        let right = if row.opens() { open } else { change };
        self.draw_side_legend(right, true, y, out);
    }

    fn draw_clock_legends(&self, picker: &ClockPicker, from_menu: bool, out: &mut Vec<Draw>) {
        let (Some(faces), Some(clock)) = (&self.quick_menu_faces, self.clock_legends) else {
            return;
        };
        let [back, change, open, ..] = match self.light() {
            true => faces.legend_light,
            false => faces.legend,
        };
        let [done, turn, page, edit, select, finish] = clock[self.light() as usize];
        let back = if from_menu { back } else { finish };
        let legends = match (picker.highlighted(), picker.editing(), picker.row()) {
            (Some(_), _, _) => vec![back, page, select],
            (None, true, _) => vec![back, turn, done],
            (None, false, ClockRow::Date | ClockRow::Time) => vec![back, edit],
            (None, false, ClockRow::Zone) => vec![back, open],
            (None, false, ClockRow::Clock | ClockRow::NetworkTime) => vec![back, change],
        };
        self.draw_spread_legends(&legends, LINK_LEGEND_Y, 1.0, out);
    }

    fn draw_spread_legends(
        &self,
        legends: &[(TexId, u32)],
        y: f32,
        alpha: f32,
        out: &mut Vec<Draw>,
    ) {
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let room = self.canvas().w - 2.0 * LINK_LEGEND_EDGE;
        let gap =
            (room - legends.iter().map(|l| seen(l.1)).sum::<f32>()) / (legends.len() - 1) as f32;
        let mut x = LINK_LEGEND_EDGE;
        for &(tex, w) in legends {
            out.push(Draw::Tex {
                x: x.round(),
                y,
                w: w as f32,
                h: MOUTH_H,
                tex,
                alpha,
            });
            x += seen(w) + gap;
        }
    }

    fn draw_side_legend(&self, face: (TexId, u32), right: bool, y: f32, out: &mut Vec<Draw>) {
        let seen = face.1.saturating_sub(HINT_EDGE) as f32;
        let x = if right {
            (self.canvas().w - LINK_LEGEND_EDGE - seen).round()
        } else {
            LINK_LEGEND_EDGE
        };
        out.push(Draw::Tex {
            x,
            y,
            w: face.1 as f32,
            h: MOUTH_H,
            tex: face.0,
            alpha: 1.0,
        });
    }

    fn chrome(&self, stem: &str, dim: f32, out: &mut Vec<Draw>) {
        let Some((cart, face)) = self.shelf().find(stem) else {
            return;
        };
        let alpha = self.alert_alpha();
        let (rest, scale) = self.shelf().selected_at(self.canvas());
        SlotChrome {
            cart,
            face,
            rest,
            scale,
            seat: self.seat(),
            alert: self.alert_face.filter(|_| alpha > 0.0).map(|t| (t, alpha)),
            dim,
            screen: self.screen,
            game: self.panel_lit(),
            over: !self.powering_up()
                && !matches!(
                    &self.phase,
                    Phase::Inserting { resumed: true, .. }
                        | Phase::Playing { .. }
                        | Phase::Ejecting { .. }
                ),
            behind: self.shelf().behind(stem),
            slot: self.slot_faces,
            canvas: self.canvas(),
        }
        .draw(out);
    }

    pub fn alert_visible(&self) -> bool {
        self.alert_alpha() > 0.0
    }

    pub fn alert_alpha(&self) -> f32 {
        let (Some(from), Phase::Ejecting { t, .. }) = (self.refused_from, &self.phase) else {
            return 0.0;
        };
        let span = EJECT_S - from;
        if span <= 0.0 {
            return 0.0;
        }
        let u = ((t - from) / span).clamp(0.0, 1.0);
        ((ALERT_GONE - u) / (ALERT_GONE - ALERT_HOLD)).clamp(0.0, 1.0)
    }

    pub fn set_alert_face(&mut self, face: TexId) {
        self.alert_face = Some(face);
    }

    pub fn set_repair_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.repair_faces = faces;
    }

    pub fn set_delete_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.delete_faces = faces;
    }

    pub fn delete_prompt(&self) -> bool {
        self.delete_prompt
    }

    pub fn set_reset_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.reset_faces = faces;
    }

    pub fn set_shutdown_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.shutdown_faces = faces;
    }

    pub fn set_core_interior_face(&mut self, interior: TexId) {
        self.core_interior_face = Some(interior);
    }

    pub fn set_core_board_faces(&mut self, board: TexId, lid: TexId) {
        self.core_board_face = Some(board);
        self.core_lid_face = Some(lid);
        self.core_faces_stem = self.selected_stem().map(str::to_string);
    }

    pub fn set_core_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
        shadow: TexId,
    ) {
        self.core_socket_faces = sockets;
        self.core_chip_faces = chips;
        self.core_blank_chip_face = Some(blank);
        self.core_chip_shadow_face = Some(shadow);
    }

    pub fn set_core_legend_faces(
        &mut self,
        dark: Vec<(TexId, u32)>,
        light: Vec<(TexId, u32)>,
    ) {
        self.core_legend_faces = [dark, light];
    }

    pub fn set_link_menu_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_menu_faces = faces;
    }

    pub fn set_link_linked_face(&mut self, face: (TexId, u32, u32)) {
        self.link_linked_face = Some(face);
    }

    pub fn set_link_step_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_step_faces = faces;
    }

    pub fn set_link_fail_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.link_fail_faces = faces;
    }

    pub fn set_link_legend_faces(&mut self, faces: Vec<(TexId, u32)>) {
        self.link_legend_faces = faces;
    }

    pub fn set_link_sprites(&mut self, sprites: LinkSprites) {
        self.link_sprites = Some(sprites);
    }

    pub fn set_link_shot(&mut self, tex: Option<TexId>) {
        self.link_shot = tex;
    }

    pub fn set_link_port(&mut self, tex: TexId) {
        if let Some(sprites) = &mut self.link_sprites {
            sprites.port.tex = tex;
        }
    }

    pub fn link_sprites_ready(&self) -> bool {
        self.link_sprites.is_some()
    }

    fn core_picker_shown(&self) -> Option<CorePicker> {
        self.core_picker.filter(|p| !p.waiting())
    }

    fn draw_lid_shadow(&self, shelf: Placed, progress: f32, out: &mut Vec<Draw>) {
        let Some(tex) = self.core_chip_shadow_face else {
            return;
        };
        let lift = lift_of(progress);
        let (lid, _) = lid_from(self.canvas(), shelf, progress);
        let k = lid.w / lid_at(self.canvas(), 1.0).0.w;
        let (w, h) = (LID_SHADOW_W * k, LID_SHADOW_H * k);
        out.push(Draw::Tex {
            x: lid.x + (lid.w - w) / 2.0,
            y: lid.y + lid.h + LID_SHADOW_DROP * k - h / 2.0,
            w,
            h,
            tex,
            alpha: LID_SHADOW_ALPHA * lift,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_board_chip(
        &self,
        board: Placed,
        sockets: &[TexId],
        chip: Chip,
        faces: &[TexId],
        blank: Option<TexId>,
        lift: f32,
        out: &mut Vec<Draw>,
    ) {
        let zoom = board_zoom(board);
        for (i, tex) in sockets.iter().copied().enumerate() {
            let (x, y) = on_board(board, SOCKET_U[i], SOCKET_V);
            out.push(Draw::Tex {
                x: x.round(),
                y: y.round(),
                w: SOCKET_W as f32 * zoom,
                h: SOCKET_H as f32 * zoom,
                tex,
                alpha: 1.0,
            });
        }
        let u = CHIP_U[0] + (CHIP_U[1] - CHIP_U[0]) * chip.across;
        if chip.lift > 0.0 {
            if let Some(tex) = self.core_chip_shadow_face {
                let (cx, cy) = on_board(board, u + 19.0, CHIP_V + 29.4);
                let (w, h) = (SHADOW_W as f32 * zoom, SHADOW_H as f32 * zoom);
                out.push(Draw::Tex {
                    x: cx - w / 2.0,
                    y: cy - h / 2.0,
                    w,
                    h,
                    tex,
                    alpha: 0.6 * chip.lift * lift,
                });
            }
        }
        let face = match chip.seated {
            Some(core) => faces.get(core.index()).copied(),
            None => blank,
        };
        if let Some(tex) = face {
            let (x, y) = on_board(board, u, CHIP_V - HOP_LIFT * chip.lift);
            let body = Placed {
                x: x + chip.shake,
                y,
                w: CHIP_W as f32 * zoom,
                h: CHIP_H as f32 * zoom,
            };
            let at = grown(body, TURN_PAD as f32 * zoom);
            out.push(Draw::Turned {
                x: at.x.round(),
                y: at.y.round(),
                w: at.w,
                h: at.h,
                tex,
                alpha: 1.0,
                turn: chip.tip,
            });
        }
    }

    fn draw_core_picker(&self, picker: &CorePicker, out: &mut Vec<Draw>) {
        let now = self.now();
        let progress = picker.openness(now);
        let lift = lift_of(progress);
        let (rest, scale) = self.shelf().selected_at(self.canvas());
        let shelf = shelf_cart_at(rest, scale);
        let board = board_from(self.canvas(), shelf, progress);
        let ready = self.core_faces_ready();

        if ready {
            if let Some(tex) = self.core_interior_face {
                out.push(Draw::Tex {
                    x: board.x,
                    y: board.y,
                    w: board.w,
                    h: board.h,
                    tex,
                    alpha: 1.0,
                });
            }
            if let Some(tex) = self.core_board_face {
                out.push(Draw::Tex {
                    x: board.x,
                    y: board.y,
                    w: board.w,
                    h: board.h,
                    tex,
                    alpha: 1.0,
                });
            }
            self.draw_board_chip(
                board,
                &self.core_socket_faces,
                picker.chip(now),
                &self.core_chip_faces,
                self.core_blank_chip_face,
                lift,
                out,
            );
        }

        self.draw_lid_shadow(shelf, progress, out);

        if ready {
            if let Some(tex) = self.core_lid_face {
                let (lid, turn) = lid_from(self.canvas(), shelf, progress);
                let at = grown(lid, TURN_PAD as f32 * lid.w / CART_W as f32);
                out.push(Draw::Turned {
                    x: at.x,
                    y: at.y,
                    w: at.w,
                    h: at.h,
                    tex,
                    alpha: 1.0,
                    turn,
                });
            }
        } else if let Some((_, Some(tex))) = self
            .selected_stem()
            .and_then(|stem| self.shelf().find(stem))
        {
            let (lid, turn) = lid_from(self.canvas(), shelf, progress);
            out.push(Draw::Turned {
                x: lid.x,
                y: lid.y,
                w: lid.w,
                h: lid.h,
                tex,
                alpha: 1.0,
                turn,
            });
        }

        let legends = &self.core_legend_faces[self.light() as usize];
        if !legends.is_empty() {
            self.draw_spread_legends(legends, LINK_LEGEND_Y, lift, out);
        }
    }

    fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        if let Some(tex) = self.link_shot {
            out.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: canvas.w,
                h: OUT_H as f32,
                tex,
                alpha: 1.0,
            });
        }
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: OUT_H as f32,
            colour: LINK_SCRIM,
        });
        if let Some(sprites) = &self.link_sprites {
            crate::link_screen::draw_link_art(
                self.canvas(),
                menu,
                self.link_hardware,
                self.now(),
                sprites,
                out,
            );
        }
        let line = match menu {
            GameMenu::Pick(role) => self.link_menu_faces.get(role.index()).copied(),
            GameMenu::Working { step, .. } => self
                .link_step_faces
                .get(step.shown(self.radio.warmed()).index())
                .copied(),
            GameMenu::Linked { .. } => self.link_linked_face,
            GameMenu::Failed { fail, .. } => fail
                .shown()
                .and_then(|i| self.link_fail_faces.get(i).copied()),
            GameMenu::Unplug { .. } => None,
        };
        if let Some((tex, w, h)) = line {
            out.push(Draw::Tex {
                x: canvas.centred(w as f32).round(),
                y: LINK_TEXT_Y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
        let switchable = self.seated().is_some_and(|stem| self.link_switchable(stem));
        let (left, right): (&[LinkLegend], &[LinkLegend]) = match menu {
            GameMenu::Pick(_) if switchable => (&[LinkLegend::Cancel], &[LinkLegend::Mode]),
            GameMenu::Pick(_) => (&[LinkLegend::Cancel], &[LinkLegend::Link]),
            GameMenu::Working { .. } => (&[LinkLegend::Cancel], &[]),
            GameMenu::Linked { opened: true, .. } => (&[LinkLegend::Back], &[LinkLegend::EndLink]),
            GameMenu::Linked { .. } => (&[], &[]),
            GameMenu::Failed { .. } => (&[], &[LinkLegend::Ok]),
            GameMenu::Unplug { .. } => (&[], &[]),
        };
        let (left, right) = (self.link_faces(left), self.link_faces(right));
        let room = self.canvas().w - 2.0 * LINK_LEGEND_EDGE;
        if link_keys_w(&left) + link_keys_w(&right) + LINK_LEGEND_GAP <= room {
            self.draw_link_keys(&left, false, out);
            self.draw_link_keys(&right, true, out);
            return;
        }
        let all: Vec<(TexId, u32)> = left.into_iter().chain(right).collect();
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let gap = (room - all.iter().map(|(_, w)| seen(*w)).sum::<f32>())
            / all.len().saturating_sub(1).max(1) as f32;
        let mut x = LINK_LEGEND_EDGE;
        for (tex, w) in all {
            out.push(Draw::Tex {
                x: x.round(),
                y: LINK_LEGEND_Y,
                w: w as f32,
                h: MOUTH_H,
                tex,
                alpha: 1.0,
            });
            x += seen(w) + gap;
        }
    }

    fn link_faces(&self, keys: &[LinkLegend]) -> Vec<(TexId, u32)> {
        keys.iter()
            .filter_map(|k| self.link_legend_faces.get(k.index()).copied())
            .collect()
    }

    fn draw_link_keys(&self, faces: &[(TexId, u32)], right: bool, out: &mut Vec<Draw>) {
        if faces.is_empty() {
            return;
        }
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let total = link_keys_w(faces);
        let mut x = if right {
            (self.canvas().w - LINK_LEGEND_EDGE - total).round()
        } else {
            LINK_LEGEND_EDGE
        };
        for &(tex, w) in faces {
            out.push(Draw::Tex {
                x,
                y: LINK_LEGEND_Y,
                w: w as f32,
                h: MOUTH_H,
                tex,
                alpha: 1.0,
            });
            x += seen(w) + LINK_LEGEND_GAP;
        }
    }

    fn auto_link(&self, stem: &str) -> Option<(&Cart, LinkKind)> {
        let cart = self.shelf().carts.iter().find(|c| c.stem == stem)?;
        let auto = link_kind(&cart.code, &cart.title, slot_store::header_clean(&cart.rom));
        Some((cart, auto))
    }

    fn on_shelf(&self) -> bool {
        matches!(self.phase, Phase::Shelf)
    }

    pub fn browsed_rom(&self) -> Option<&std::path::Path> {
        if !self.on_shelf() {
            return None;
        }
        let shelf = self.shelf();
        shelf.carts.get(shelf.index).map(|c| c.rom.as_path())
    }

    fn insert(&mut self, clean: bool) {
        if !self.on_shelf() {
            return;
        }
        let at = self.shelf().index;
        let Some(cart) = self.shelf_mut().carts.get_mut(at).map(|c| {
            c.read_header();
            c.stem.clone()
        }) else {
            return;
        };
        eprintln!(
            "slot: launch: insert {cart} at {:.1} ms",
            crate::boot_time::ms()
        );
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        self.sunlight_manual = None;
        if let Some(mark) = &self.loading {
            mark.set(&cart);
            self.marked = true;
            self.playing_since = None;
        }
        self.phase = Phase::Inserting {
            cart,
            t: 0.0,
            core_ready: false,
            resumed: false,
            clean,
        };
    }

    pub fn starting_clean(&self) -> bool {
        matches!(self.phase, Phase::Inserting { clean: true, .. })
    }

    fn play_hold(&mut self) {
        let Some(at) = self.play_held else {
            return;
        };
        if !self.on_shelf() {
            self.play_held = None;
            return;
        }
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS {
            self.insert(true);
        }
    }

    fn eject(&mut self) {
        if self.single_cart() {
            return self.refuse();
        }
        self.shot_wanted = false;
        self.eject_overlay = match self.shows_game() {
            true => self.overlay_slide(),
            false => 0.0,
        };
        self.in_game_menu = None;
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Inserting { cart, .. } => std::mem::take(cart),
            _ => return,
        };
        self.end_link();
        self.close_game_menu();
        self.flush_eject(&cart);
        self.pending = None;
        self.refusal = None;
        self.refused_from = None;
        self.phase = Phase::Ejecting {
            cart,
            t: -EJECT_HOLD_S,
        };
    }

    fn flush_eject(&mut self, stem: &str) {
        self.settle_saves();
        let (Some(root), Some(snapshot)) = (&self.root, &self.snapshot) else {
            return;
        };
        let (state, sav) = match snapshot.state() {
            Some(state) => trusted_write(snapshot.as_ref(), state, "eject"),
            None => {
                eprintln!("slot: eject: the core gave up no state");
                (None, None)
            }
        };
        let state = state.filter(|_| self.state.eject_save);
        self.state_writer.flush();
        match persist::eject(
            root,
            self.platform,
            self.core,
            stem,
            state.as_deref(),
            sav.as_deref(),
            &self.state,
        ) {
            Ok(()) => {
                self.state.cart = None;
                self.state.cart_platform = None;
                self.card_writes.ok("eject");
            }
            Err(e) => {
                eprintln!("slot: eject: {e}");
                self.card_writes.failed("eject", &e);
            }
        }
    }

    fn updating(&self) -> bool {
        matches!(
            self.update,
            Some(
                UpdateState::Downloading(..)
                    | UpdateState::Cancelling(..)
                    | UpdateState::Closing(_)
            )
        )
    }

    fn doze(&mut self) {
        self.cancel_fetch_art();
        self.abandon_server();
        if !self.updating() {
            self.release_radio();
        }
        if self.link_active() {
            self.end_link();
        }
        if !self.updating() {
            self.update = None;
        }
        self.close_game_menu();
        self.core_picker = None;
        self.keyboard = None;
        self.forget_prompt = false;
        self.shot_wanted = false;
        if matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.state_writer.flush();
        self.flush_resume();
        let cart = match &mut self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(std::mem::take(cart)),
            _ => None,
        };
        let row = self.album_row();
        self.shut_guide();
        self.shot_viewer = None;
        if std::mem::take(&mut self.states_from_menu) {
            self.in_game_menu = Some(row);
        }
        self.polaroids = None;
        self.phase = Phase::Doze { cart };
        self.dozed_at = self.now();
        self.radio.ask(RadioJob::Cool);
        if let Some(power) = &mut self.power {
            power.on_close();
        }
    }

    fn wake(&mut self) {
        let Phase::Doze { cart } = &mut self.phase else {
            return;
        };
        self.phase = match cart.take() {
            Some(cart) => Phase::Playing { cart },
            None => Phase::Shelf,
        };
        if let Some(power) = &mut self.power {
            power.on_open();
        }
    }

    pub fn on_doze_timeout(&mut self) {
        if !matches!(self.phase, Phase::Doze { .. }) {
            return;
        }
        self.begin_power_off();
    }

    fn power_press(&mut self) {
        if std::mem::take(&mut self.woke_on_press) {
            return;
        }
        match self.phase {
            Phase::Doze { .. } => self.wake(),
            _ => self.doze(),
        }
    }

    fn hold_power(&mut self) {
        self.flush_resume();
        self.begin_power_off();
    }

    fn begin_repair(&mut self) {
        self.repair_prompt = false;
        if self.shutting_down() {
            return;
        }
        self.repairing = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    fn begin_restart(&mut self) {
        self.release_radio();
        if self.shutting_down() {
            return;
        }
        self.remember_shelf();
        self.restarting = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    fn open_core_picker(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        if cart.platform.hardware() != Platform::Gba {
            return;
        }
        let seat = slot_store::core_for(&root, &cart.stem);
        let now = self.now();
        let mut picker = CorePicker::open(seat, now);
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    fn core_faces_ready(&self) -> bool {
        self.core_faces_stem
            .as_deref()
            .is_some_and(|stem| self.selected_stem() == Some(stem))
    }

    fn core_picker_input(&mut self, action: Action) {
        let press = match action {
            Action::GbaDown(Btn::Left) | Action::ShelfLeft => Press::Left,
            Action::GbaDown(Btn::Right) | Action::ShelfRight => Press::Right,
            Action::GbaDown(Btn::A) => Press::Keep,
            Action::GbaDown(Btn::B) => Press::Back,
            _ => return,
        };
        let now = self.now();
        let Some(picker) = &mut self.core_picker else {
            return;
        };
        let outcome = picker.press(press, now);
        if let Outcome::Write(core) = outcome {
            self.write_core(core);
        }
    }

    fn open_game_menu(&mut self) {
        if self.game_menu.is_some() {
            return;
        }
        if self.link_active() {
            return self.refuse();
        }
        if !self.has_wifi_hardware() {
            self.hud.toast(Toast::NoWifi, self.now());
            return;
        }
        if self.core == Core::Mgba {
            let wireless = self
                .seated()
                .is_some_and(|stem| self.link_mode(stem).0 == LinkKind::Wireless);
            if wireless {
                self.hud.toast(Toast::NeedsGpsp, self.now());
                return;
            }
            self.link_hardware = LinkKind::Cable;
            self.radio.ask(RadioJob::Warm);
            self.game_menu = Some(GameMenu::Pick(self.last_role));
            return;
        }
        if self.platform != Platform::Gba {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        let carried = self
            .seated()
            .and_then(|stem| self.auto_link(stem))
            .is_some_and(|(cart, _)| link_carried(&cart.code, &cart.title));
        if !carried {
            self.hud.toast(Toast::NoLink, self.now());
            return;
        }
        if self.core != Core::Gpsp {
            self.hud.toast(Toast::NeedsGpsp, self.now());
            return;
        }
        let hardware = self
            .seated()
            .map_or(LinkKind::Cable, |stem| self.link_mode(stem).0);
        self.link_hardware = hardware;
        self.radio.ask(RadioJob::Warm);
        self.game_menu = Some(GameMenu::Pick(self.last_role));
    }

    fn game_menu_shortcut(&mut self) {
        let Some(client_id) = self.link_client_id() else {
            return self.open_game_menu();
        };
        let now = self.now();
        self.game_menu = Some(GameMenu::Linked {
            role: LinkRow::from_client_id(client_id),
            worked: now,
            since: now,
            opened: true,
        });
    }

    fn end_link_from_menu(&mut self) {
        let role = self
            .link_client_id()
            .map_or(self.last_role, LinkRow::from_client_id);
        self.end_link();
        self.hud.toast(Toast::LinkEnded, self.now());
        self.unplug(role);
    }

    fn unplug(&mut self, role: LinkRow) {
        self.game_menu = Some(GameMenu::Unplug {
            role,
            since: self.now(),
        });
    }

    pub fn set_radio_jobs(&mut self, jobs: Box<dyn RadioJobs>) {
        self.radio = jobs;
    }

    pub fn set_update_jobs(&mut self, jobs: Box<dyn UpdateJobs>) {
        self.update_jobs = jobs;
    }

    pub fn studio(&self) -> Option<&crate::studio::Studio> {
        self.studio.as_ref()
    }

    fn open_studio(&mut self) {
        let wifi = self.wifi_up();
        let shelf = self.shelf();
        let picked = shelf.carts.get(shelf.index).cloned();
        if picked.as_ref().is_some_and(|c| self.studio_saving.as_deref() == Some(c.stem.as_str())) {
            return;
        }
        let face = shelf.face(shelf.index);
        let under = picked.as_ref().map_or([None; 2], |cart| shelf.behind(&cart.stem));
        if let Some(cart) = picked {
            self.studio_from = crate::studio_view::shelf_rect(self.canvas().w, &cart);
            self.studio_shelf_face = face;
            self.studio_shelf_under = under;
            self.studio_opened = self.now();
            use slot_ui::house_label::LabelTag;
            let mut studio = crate::studio::Studio::new(cart, wifi);
            let tag = self.root.as_deref().and_then(|root| {
                let label = crate::art::make::label_path(root, &studio.cart);
                slot_ui::house_label::label_tag(&std::fs::read(label).ok()?)
            });
            match tag {
                Some(LabelTag::Logo(ground)) => studio.adopt_logo(ground),
                Some(LabelTag::Text(ground)) => studio.adopt_text(ground),
                None => {}
            }
            self.studio = Some(studio);
            self.studio_art = None;
            self.studio_session += 1;
            self.studio_tex = None;
            self.studio_chips = None;
            self.studio_scene = None;
            self.studio_slips.clear();
            self.studio_prints = 0;
            self.studio_shuffled = f64::NEG_INFINITY;
            self.shelf_mut().release_hold();
            self.play_held = None;
            self.studio_fetched = false;
            self.studio_applied = false;
            if wifi {
                self.fetch_studio_art();
            }
        }
    }

    fn fetch_studio_art(&mut self) {
        let Some(cart) = self.studio.as_ref().map(|s| s.cart.clone()) else {
            return;
        };
        self.studio_fetched = true;
        self.art_pending += 1;
        self.hold_wifi();
        self.art_jobs.ask(ArtJob::StudioFetch(cart));
    }

    pub fn set_wifi(&mut self, wifi: crate::wifi::Wifi) {
        self.wifi = Some((wifi, std::time::Instant::now(), self.now()));
        self.wifi_held = false;
        self.wifi_scanning = false;
        self.wifi_boot_blocked = false;
    }

    pub fn wifi(&self) -> &crate::wifi::Snapshot {
        static NONE: std::sync::OnceLock<crate::wifi::Snapshot> = std::sync::OnceLock::new();
        match &self.wifi {
            Some((w, _, _)) => w.snapshot(),
            None => NONE.get_or_init(crate::wifi::Snapshot::default),
        }
    }

    pub fn wifi_send(&mut self, cmd: crate::wifi::Cmd) {
        if let Some((w, _, _)) = &mut self.wifi {
            w.send(cmd);
        }
    }

    fn hold_wifi(&mut self) {
        if !std::mem::replace(&mut self.wifi_held, true) {
            self.wifi_send(crate::wifi::Cmd::Hold);
        }
    }

    fn poll_wifi(&mut self) {
        let now = self.now();
        let Some((w, at, from)) = &mut self.wifi else {
            return;
        };
        w.poll(*at + std::time::Duration::from_millis(now.saturating_sub(*from)));
        if self.wifi_held && !w.busy() && !w.snapshot().held {
            self.wifi_held = false;
            self.wifi_wanted_at = Some(now + WIFI_RETRY_MS);
        }
        if let Phase::QuickMenu { row } = self.phase {
            if NET_TOOLS.contains(&row) && self.hidden_rows().contains(&row) {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::Networks,
                };
            }
        }
    }

    pub fn take_core_stale(&mut self) -> bool {
        std::mem::take(&mut self.core_stale)
    }

    pub fn release_radio(&mut self) {
        self.wifi_wanted_at = None;
        if std::mem::take(&mut self.wifi_held) {
            self.wifi_send(if self.state.wifi_off {
                crate::wifi::Cmd::PowerOff
            } else {
                crate::wifi::Cmd::Release
            });
        }
    }

    pub fn wifi_up(&self) -> bool {
        let snap = self.wifi();
        snap.status.state == crate::wifi::State::Completed && snap.status.ip.is_some()
    }

    fn wifi_wanted(&self) -> bool {
        matches!(
            self.phase,
            Phase::Shelf
                | Phase::QuickMenu { .. }
                | Phase::About
                | Phase::SetClock { .. }
                | Phase::Networks { .. }
                | Phase::NetworkScan { .. }
                | Phase::NetworkPage { .. }
        ) && self.seated_cart().is_none()
            && !self.link_active()
            && !self.shutting_down()
            && !self.state.wifi_off
            && (self.has_saved()
                || matches!(
                    self.phase,
                    Phase::Networks { .. } | Phase::NetworkScan { .. } | Phase::NetworkPage { .. }
                ))
    }

    pub fn wifi_is_off(&self) -> bool {
        self.state.wifi_off
    }

    pub fn set_wifi_off(&mut self, off: bool) {
        if self.state.wifi_off == off {
            return;
        }
        self.state.wifi_off = off;
        self.persist();
        if off {
            self.wifi_wanted_at = None;
            self.wifi_held = false;
            self.wifi_boot_blocked = true;
            self.wifi_send(crate::wifi::Cmd::PowerOff);
        } else if self.wifi_wanted() {
            self.hold_wifi();
        }
    }

    pub fn wifi_configured(&self) -> bool {
        self.root.as_deref().is_some_and(crate::update::wifi_conf)
    }

    pub fn connect_wifi(&mut self) {
        if self.wifi_wanted() {
            self.hold_wifi();
        }
    }

    pub fn set_wifi_probe(&mut self, wlan0: PathBuf) {
        self.wifi_probe = Some((wlan0, 0));
    }

    fn has_wifi_hardware(&mut self) -> bool {
        if self.wifi_probe.as_ref().is_some_and(|(p, _)| p.exists()) {
            self.wifi_probe = None;
        }
        self.wifi_probe.is_none()
    }

    fn probe_wifi(&mut self) {
        let now = self.now();
        let Some((path, next)) = &mut self.wifi_probe else {
            return;
        };
        if now < *next {
            return;
        }
        match path.exists() {
            true => self.wifi_probe = None,
            false => *next = now + WIFI_PROBE_MS,
        }
    }

    fn steer_wifi(&mut self) {
        self.probe_wifi();
        let scan = matches!(self.phase, Phase::NetworkScan { .. })
            && self.keyboard.is_none()
            && !self.state.wifi_off;
        if scan != self.wifi_scanning {
            self.wifi_scanning = scan;
            if let Some((w, _, _)) = &mut self.wifi {
                w.want_scans(scan);
            }
        }
        self.poll_wifi();
        if self.state.wifi_off
            && !self.wifi_boot_blocked
            && self.wifi.is_some()
            && matches!(self.phase, Phase::Shelf)
        {
            self.wifi_boot_blocked = true;
            self.wifi_send(crate::wifi::Cmd::PowerOff);
        }
        if !self.wifi_wanted() {
            if !self.updating() {
                self.release_radio();
            }
            return;
        }
        let now = self.now();
        if self.wifi_up() && now.saturating_sub(self.wifi_sampled) >= WIFI_SAMPLE_MS {
            self.wifi_sampled = now;
            if let Some(rssi) = crate::update::wifi_rssi() {
                self.wifi_bars = crate::update::signal_bars(rssi);
            }
        }
        if self.wifi_held {
            return;
        }
        let now = self.now();
        let since = *self.wifi_wanted_at.get_or_insert(now);
        if now.saturating_sub(since) >= WIFI_SETTLE_MS {
            self.hold_wifi();
        }
    }

    fn reface(&mut self, stem: &str) {
        for (_, shelf) in self.shelves.iter_mut() {
            for i in 0..shelf.carts.len() {
                if shelf.carts[i].stem == stem {
                    self.orphaned.extend(shelf.take_face(i));
                }
            }
        }
        self.core_stale = true;
    }

    fn tick_tray(&mut self, now: f64) {
        use crate::studio::LabelKind;
        if self.studio_scene.is_some() {
            return;
        }
        let Some(studio) = &self.studio else {
            return;
        };
        if !matches!(studio.screen, crate::studio::Screen::Sheet { .. }) {
            self.studio_slips.clear();
            return;
        }
        let picked = match studio.screen {
            crate::studio::Screen::Sheet { at } => studio.sheet.get(at).copied(),
            _ => None,
        };
        if picked != self.studio_picked.map(|(kind, _)| kind) {
            self.studio_picked = picked.map(|kind| (kind, now));
        }
        let Some(studio) = &self.studio else {
            return;
        };
        let cart = &studio.cart;
        let t = crate::studio_view::tray(self.canvas().w, cart);
        let sheet = studio.sheet.clone();
        let ready: Vec<LabelKind> = [
            LabelKind::Existing,
            LabelKind::Real,
            LabelKind::Logo,
            LabelKind::Title,
        ]
        .into_iter()
        .filter(|k| self.studio_face(*k).is_some())
        .collect();
        let before = self.studio_slips.len();
        crate::studio::tray_step(
            &mut self.studio_slips,
            &sheet,
            &|k| ready.contains(&k),
            now,
            t.slip_h,
            t.bottom,
        );
        if self.studio_slips.len() > before {
            self.studio_prints += 1;
        }
    }

    pub fn studio_slip_count(&self) -> usize {
        self.studio_slips.len()
    }

    pub fn studio_session(&self) -> u64 {
        self.studio_session
    }

    pub fn studio_preview(&self) -> Option<crate::studio_faces::PreviewJob> {
        let studio = self.studio.as_ref()?;
        let mut cart = studio.cart.clone();
        cart.shell = studio.shell;
        cart.label = None;
        let art = self.studio_art.as_ref();
        Some(crate::studio_faces::PreviewJob {
            session: self.studio_session,
            gen: 0,
            cart,
            ground: studio.preview_ground(),
            text: studio.wearing_text(),
            real: art.and_then(|a| a.real.clone()),
            logo: art.and_then(|a| a.logo.clone()),
            wheel: art.and_then(|a| a.wheel.clone()),
            existing: studio.cart.label.clone(),
            look: art.and_then(|a| a.look.clone()),
        })
    }

    pub fn set_studio_tex(&mut self, tex: StudioTex) {
        self.studio_tex = Some(tex);
    }

    pub fn studio_tex(&self) -> Option<&StudioTex> {
        self.studio_tex.as_ref()
    }

    pub fn set_studio_chips(&mut self, chips: (TexId, u32, u32)) {
        self.studio_chips = Some(chips);
    }

    pub fn set_studio_paper(&mut self, liner: TexId, disc: TexId) {
        self.studio_liner = Some(liner);
        self.studio_disc = Some(disc);
    }

    pub fn set_studio_legend(
        &mut self,
        key: (crate::studio::Legend, bool, bool),
        face: (TexId, u32),
    ) {
        self.studio_legends.insert(key, face);
    }

    pub fn studio_wanted_text(&self) -> Vec<(String, crate::studio_view::Style)> {
        use crate::studio_view::{view, Item};
        let Some(studio) = &self.studio else {
            return Vec::new();
        };
        let mut wanted: Vec<_> = view(studio, self.canvas().w)
            .items
            .into_iter()
            .filter_map(|i| match i {
                Item::Text { text, style, .. } => Some((text, style)),
                _ => None,
            })
            .collect();
        wanted.retain(|k| !self.studio_text.contains_key(k));
        wanted
    }

    pub fn set_studio_text(&mut self, key: (String, crate::studio_view::Style), face: (TexId, u32, u32)) {
        if key.0.len() == 7 && key.0.starts_with('#') {
            if let Some(old) = self.studio_hex.replace(key.clone()) {
                self.orphaned.extend(self.studio_text.remove(&old).map(|f| f.0));
            }
        }
        self.studio_text.insert(key, face);
    }

    fn studio_face(&self, kind: crate::studio::LabelKind) -> Option<TexId> {
        use crate::studio::LabelKind;
        let tex = self.studio_tex.as_ref()?;
        let find = |k| tex.faces.iter().find(|(f, _)| *f == k).map(|(_, t)| *t);
        match kind {
            LabelKind::Existing => find(LabelKind::Existing).or(find(LabelKind::Title)),
            _ => find(kind),
        }
    }

    fn studio_shown(&self) -> crate::studio::LabelKind {
        self.studio
            .as_ref()
            .map_or(crate::studio::LabelKind::Title, |s| s.shown_label())
    }

    fn studio_cart_rect(&self) -> [f32; 4] {
        let Some(studio) = &self.studio else {
            return [0.0; 4];
        };
        let l = crate::studio_view::layout(self.canvas().w, studio.cart.platform);
        [l.cart_x, l.cart_y, l.cart_w, l.cart_h]
    }

    #[allow(clippy::too_many_arguments)]
    fn studio_put_text(
        &self,
        text: &str,
        style: crate::studio_view::Style,
        align: crate::studio_view::Align,
        x: f32,
        y: f32,
        alpha: f32,
        scale: f32,
        out: &mut Vec<Draw>,
    ) {
        use crate::studio_view::Align;
        let Some(&(tex, w, h)) = self.studio_text.get(&(text.to_string(), style)) else {
            return;
        };
        let (w, h) = (w as f32 * scale, h as f32 * scale);
        let pad = slot_ui::MENU_PAD as f32 * scale;
        let x = match align {
            Align::Left => x - pad,
            Align::Center => x - w / 2.0,
            Align::Right => x - w + pad,
        };
        out.push(Draw::Tex { x: x.floor(), y, w, h, tex, alpha });
    }

    fn draw_studio(&self, out: &mut Vec<Draw>) {
        use crate::studio_view::{view, Item};
        let Some(studio) = &self.studio else {
            return;
        };
        let canvas = self.canvas();
        self.draw_studio_ground(0.0, out);
        let shown = view(studio, canvas.w);
        let since = self.studio_closing.unwrap_or(self.studio_opened);
        let t = (self.now().saturating_sub(since) as f32 / STUDIO_SLIDE_MS).min(1.0);
        if self.studio_closing.is_some() || t < 1.0 {
            let e = if self.studio_closing.is_some() { 1.0 - ease(t) } else { ease(t) };
            self.draw_background(out);
            self.shelf().draw_row(canvas, Some(&studio.cart.stem), 0.0, 0.0, 1.0, out);
            self.draw_studio_ground((e - 1.0) * OUT_H as f32, out);
            self.draw_studio_cart(out);
            return draw_empty_slot(canvas, self.slot_faces, out);
        }
        for item in &shown.items {
            match item {
                Item::Cart => self.draw_studio_cart(out),
                Item::Sheet => self.draw_studio_sheet(out),
                Item::Text { text, style, align, x, y, alpha, scale } => {
                    self.studio_put_text(text, *style, *align, *x, *y, *alpha, *scale, out)
                }
                Item::Rect { x, y, w, h, color } => {
                    out.push(Draw::Rect { x: *x, y: *y, w: *w, h: *h, colour: *color })
                }
                Item::Chip { cell, row, x, y, selected } => {
                    self.draw_studio_chip(*cell, *row, *x, *y, *selected, out)
                }
                Item::Dot { x, y, size, alpha } => {
                    if let Some(disc) = self.studio_disc {
                        out.push(Draw::Tex { x: *x, y: *y, w: *size, h: *size, tex: disc, alpha: *alpha });
                    }
                }
            }
        }
        if shown.closed_bar {
            slot_ui::draw_plugged_slot(canvas, self.plugged_face, 1.0, out);
        } else {
            draw_empty_slot(canvas, self.slot_faces, out);
        }
        if let (Some(scene), Some(tex)) = (&self.studio_scene, &self.studio_tex) {
            self.draw_apply_cart(scene, tex, true, out);
        }
        self.draw_studio_legends(studio, out);
    }

    fn draw_studio_chip(&self, cell: u32, row: u32, x: f32, y: f32, selected: bool, out: &mut Vec<Draw>) {
        let Some((tex, w, h)) = self.studio_chips else {
            return;
        };
        let size = crate::studio_faces::CHIP as f32;
        let grow = if selected { 3.0 } else { 0.0 };
        if let (true, Some(disc)) = (selected, self.studio_disc) {
            let ring = size + 2.0 * grow + 6.0;
            out.push(Draw::Tex { x: x - grow - 3.0, y: y - grow - 3.0, w: ring, h: ring, tex: disc, alpha: 1.0 });
        }
        let cell_w = crate::studio_faces::CHIP as f32 / w as f32;
        let cell_h = crate::studio_faces::CHIP as f32 / h as f32;
        out.push(Draw::Part {
            x: x - grow,
            y: y - grow,
            w: size + 2.0 * grow,
            h: size + 2.0 * grow,
            tex,
            alpha: 1.0,
            uv: [cell as f32 * cell_w, row as f32 * cell_h, cell_w, cell_h],
        });
    }

    fn draw_studio_legends(&self, studio: &crate::studio::Studio, out: &mut Vec<Draw>) {
        use crate::studio::{Legend, Screen};
        let mut legends = studio.legends();
        if let Screen::Sheet { at } = studio.screen {
            let now = self.now() as f64;
            let picked = studio.sheet.get(at).copied();
            let ready = self
                .studio_slips
                .iter()
                .any(|s| Some(s.kind) == picked && !s.moving(now));
            if !ready || self.studio_scene.is_some() {
                legends.retain(|l| *l != Legend::Apply);
            }
        }
        let n = legends.len();
        let light = self.light();
        if n <= 2 {
            for (i, legend) in legends.iter().enumerate() {
                if let Some(face) = self.studio_legends.get(&(*legend, i == 0, light)) {
                    self.draw_side_legend(*face, i == 1, LINK_LEGEND_Y, out);
                }
            }
            return;
        }
        let faces: Vec<_> = legends
            .iter()
            .enumerate()
            .filter_map(|(i, l)| self.studio_legends.get(&(*l, i < n / 2, light)).copied())
            .collect();
        let total: f32 = faces.iter().map(|f| f.1 as f32).sum();
        let gap = (self.canvas().w - total) / (faces.len() + 1) as f32;
        let mut x = gap;
        for (tex, w) in faces {
            out.push(Draw::Tex { x: x.floor(), y: LINK_LEGEND_Y, w: w as f32, h: MOUTH_H, tex, alpha: 1.0 });
            x += w as f32 + gap;
        }
    }

    fn draw_studio_cart(&self, out: &mut Vec<Draw>) {
        let since = self.studio_closing.unwrap_or(self.studio_opened);
        let t = (self.now().saturating_sub(since) as f32 / STUDIO_SLIDE_MS).min(1.0);
        let e = if self.studio_closing.is_some() { 1.0 - ease(t) } else { ease(t) };
        let (from, to) = (self.studio_from, self.studio_cart_rect());
        let at = |i: usize| from[i] + (to[i] - from[i]) * e;
        let rect = [at(0), at(1), at(2), at(3)];
        let centre = [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0];
        let corners =
            crate::studio_view::turned(rect, centre, crate::studio_view::CART_TILT * e);
        let quad = |tex: TexId| Draw::Quad { corners, tex, uv: [0.0, 0.0, 1.0, 1.0], alpha: 1.0 };
        let Some(tex) = &self.studio_tex else {
            for under in self.studio_shelf_under.into_iter().flatten() {
                out.push(quad(under));
            }
            if let Some(face) = self.studio_shelf_face {
                out.push(quad(face));
            }
            return;
        };
        for under in tex.under.into_iter().flatten() {
            out.push(quad(under));
        }
        if let Some(scene) = &self.studio_scene {
            return self.draw_apply_cart(scene, tex, false, out);
        }
        let shown = self.studio_face(self.studio_shown());
        if let Some(face) = shown.or(self.studio_face(crate::studio::LabelKind::Existing)) {
            out.push(quad(face));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn studio_sticker(
        &self,
        face: TexId,
        tex: &StudioTex,
        well: [u32; 4],
        rect: [f32; 4],
        flat: f32,
        curl: f32,
        pose: slot_ui::sticker_curl::Pose,
        out: &mut Vec<Draw>,
    ) {
        let (fw, fh) = (tex.w as f32, tex.h as f32);
        let focus = [self.canvas().w / 2.0, 240.0];
        for strip in slot_ui::sticker_curl::strips(rect, flat, curl, &pose, focus) {
            let (t, uv) = match (strip.back, self.fetch_paper) {
                (true, Some(paper)) => (paper, [0.0, 0.0, 1.0, 1.0]),
                _ => (
                    face,
                    [
                        (well[0] as f32 + strip.uv[0] * well[2] as f32) / fw,
                        well[1] as f32 / fh,
                        strip.uv[2] * well[2] as f32 / fw,
                        well[3] as f32 / fh,
                    ],
                ),
            };
            out.push(Draw::Quad { corners: strip.corners, tex: t, uv, alpha: 1.0 });
        }
    }

    fn draw_apply_cart(
        &self,
        scene: &crate::studio::ApplyScene,
        tex: &StudioTex,
        top: bool,
        out: &mut Vec<Draw>,
    ) {
        use crate::studio::{apply_stage, ApplyStage};
        use crate::studio_view::{tray, well_of, SLIP_PAD, TRAY_TOP};
        use slot_ui::sticker_curl::Pose;
        let Some(studio) = &self.studio else {
            return;
        };
        let [x, y, w, h] = self.studio_cart_rect();
        let well = well_of(&studio.cart);
        let (sx, sy) = (w / tex.w as f32, h / tex.h as f32);
        let rect = [
            x + well[0] as f32 * sx,
            y + well[1] as f32 * sy,
            well[2] as f32 * sx,
            well[3] as f32 * sy,
        ];
        let t = tray(self.canvas().w, &studio.cart);
        let spot = [t.x + SLIP_PAD, TRAY_TOP + scene.used_y + SLIP_PAD];
        let centre = [x + w / 2.0, y + h / 2.0];
        let tilt = crate::studio_view::CART_TILT;
        let cart = crate::studio_view::turned([x, y, w, h], centre, tilt);
        let seat = crate::studio_view::turned(rect, centre, tilt)[0];
        let (dx, dy) = (seat[0] - rect[0], seat[1] - rect[1]);
        let seated = Pose { x: dx, y: dy, z: 0.0, turn_deg: tilt, scale: 1.0 };
        let pose_at = |p: f32| {
            let arc = (p * std::f32::consts::PI).sin();
            Pose {
                x: dx + (spot[0] - seat[0]) * p,
                y: dy + (spot[1] - seat[1]) * p - 30.0 * arc,
                z: 90.0 * arc,
                turn_deg: tilt * (1.0 - p),
                scale: 1.0 + (t.sticker_w / rect[2] - 1.0) * p,
            }
        };
        let (stage, p) = apply_stage(self.now() as f64 - scene.started);
        let (Some(old), Some(new)) = (self.studio_face(scene.was), self.studio_face(scene.chosen))
        else {
            return;
        };
        if !top {
            out.push(Draw::Quad { corners: cart, tex: tex.bare, uv: [0.0, 0.0, 1.0, 1.0], alpha: 1.0 });
            return;
        }
        let stick = |t: TexId, flat: f32, curl: f32, pose: Pose, out: &mut Vec<Draw>| {
            self.studio_sticker(t, tex, well, rect, flat, curl, pose, out)
        };
        match stage {
            ApplyStage::Peel => {
                let q = p * p;
                stick(old, 1.0 - q, 150.0 + 270.0 * q, seated, out);
            }
            ApplyStage::Toss => {
                let (q, arc) = (p * p, (p * std::f32::consts::PI).sin());
                let pose = Pose {
                    x: dx - 520.0 * q,
                    y: dy - 60.0 * q - 50.0 * arc,
                    z: 90.0 * arc,
                    turn_deg: tilt - 24.0 * q,
                    scale: 1.0,
                };
                stick(old, 0.0, 420.0, pose, out);
            }
            ApplyStage::Lift => stick(new, 1.0 - p, 150.0, pose_at(1.0), out),
            ApplyStage::ToCart => stick(new, 0.0, 150.0, pose_at(1.0 - p), out),
            ApplyStage::Roll => stick(new, p, 150.0, seated, out),
        }
    }

    fn draw_studio_ground(&self, top: f32, out: &mut Vec<Draw>) {
        let w = self.canvas().w;
        let h = OUT_H as f32;
        out.push(Draw::Rect { x: 0.0, y: top, w, h, colour: [0.122, 0.267, 0.216, 1.0] });
        let line = |x: f32, y: f32, w: f32, h: f32, a: f32| Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [0.82, 0.92, 0.78, a],
        };
        for i in 0..(w as usize / 24 + 1) {
            let a = if i % 5 == 0 { 0.34 } else { 0.13 };
            out.push(line(i as f32 * 24.0, top, 1.0, h, a));
        }
        for i in 0..(h as usize / 24 + 1) {
            let a = if i % 5 == 0 { 0.34 } else { 0.13 };
            out.push(line(0.0, top + i as f32 * 24.0, w, 1.0, a));
        }
    }

    fn draw_slip(
        &self,
        kind: Option<crate::studio::LabelKind>,
        y: f32,
        shift: f32,
        zoom: f32,
        out: &mut Vec<Draw>,
    ) {
        use crate::studio_view::{tray, well_of, SLIP_PAD};
        let (Some(studio), Some(liner)) = (&self.studio, self.studio_liner) else {
            return;
        };
        let t = tray(self.canvas().w, &studio.cart);
        let (w, h) = (t.slip_w * zoom, t.slip_h * zoom);
        let (x, y) = (t.x + shift - (w - t.slip_w) / 2.0, y - (h - t.slip_h) / 2.0);
        out.push(Draw::Rect {
            x: x + 1.0,
            y: y + 3.0,
            w,
            h,
            colour: [0.0, 0.0, 0.0, 0.4],
        });
        out.push(Draw::Tex { x, y, w, h, tex: liner, alpha: 1.0 });
        let (Some(kind), Some(tex)) = (kind, &self.studio_tex) else {
            return;
        };
        let Some(face) = self.studio_face(kind) else {
            return;
        };
        let well = well_of(&studio.cart);
        let (fw, fh) = (tex.w as f32, tex.h as f32);
        out.push(Draw::Part {
            x: x + SLIP_PAD * zoom,
            y: y + SLIP_PAD * zoom,
            w: t.sticker_w * zoom,
            h: t.sticker_h * zoom,
            tex: face,
            alpha: 1.0,
            uv: [
                well[0] as f32 / fw,
                well[1] as f32 / fh,
                well[2] as f32 / fw,
                well[3] as f32 / fh,
            ],
        });
    }

    pub fn studio_apart(&self) -> f32 {
        let Some(studio) = &self.studio else {
            return 0.0;
        };
        let t = crate::studio_view::tray(self.canvas().w, &studio.cart);
        if t.bottom >= t.slip_h {
            return 0.0;
        }
        crate::studio::slip_shuffle(self.now() as f64 - self.studio_shuffled) * SLIP_SHUFFLE_PX
    }

    fn draw_studio_sheet(&self, out: &mut Vec<Draw>) {
        use crate::studio::{apply_stage, ApplyStage, Screen};
        use crate::studio_view::{tray, TRAY_TOP};
        let Some(studio) = &self.studio else {
            return;
        };
        let t = tray(self.canvas().w, &studio.cart);
        let now = self.now() as f64;
        let picked = match studio.screen {
            Screen::Sheet { at } => studio.sheet.get(at).copied(),
            _ => None,
        };
        let used = self.studio_scene.as_ref().map(|scene| scene.chosen);
        let held = |s: &crate::studio::Slip| {
            Some(s.kind) == picked && !s.moving(now) && self.studio_scene.is_none()
        };
        let slips = || self.studio_slips.iter().filter(|s| Some(s.kind) != used);
        let piled = t.bottom < t.slip_h;
        let since = self.studio_picked.map_or(now, |(_, at)| at);
        let elapsed = now - since;
        let apart = self.studio_apart();
        let dim = self.studio_scene.is_some() || slips().any(held);
        let rest = |out: &mut Vec<Draw>| {
            for slip in slips().filter(|s| !held(s)) {
                let y = TRAY_TOP + slip.y(now);
                self.draw_slip(Some(slip.kind), y, apart, 1.0, out);
                if dim && !slip.moving(now) {
                    out.push(Draw::Rect {
                        x: t.x + apart,
                        y,
                        w: t.slip_w,
                        h: t.slip_h,
                        colour: [0.0, 0.0, 0.0, 0.5],
                    });
                }
            }
        };
        let swapped = !piled || now - self.studio_shuffled >= crate::studio::SLIP_SHUFFLE_MS / 2.0;
        if swapped {
            rest(out);
        }
        for slip in slips().filter(|s| held(s)) {
            let zoom = if piled { 1.0 } else { crate::studio::slip_zoom(elapsed) };
            let (w, h) = (t.slip_w * zoom, t.slip_h * zoom);
            let x = t.x - apart - (w - t.slip_w) / 2.0;
            let y = TRAY_TOP + slip.y(now) - (h - t.slip_h) / 2.0;
            for (grow, colour) in [(6.0, [0.0, 0.0, 0.0, 0.85]), (4.0, [1.0, 1.0, 1.0, 1.0])] {
                out.push(Draw::Rect {
                    x: x - grow,
                    y: y - grow,
                    w: w + 2.0 * grow,
                    h: h + 2.0 * grow,
                    colour,
                });
            }
            self.draw_slip(Some(slip.kind), TRAY_TOP + slip.y(now), -apart, zoom, out);
        }
        if !swapped {
            rest(out);
        }
        if let Some(scene) = &self.studio_scene {
            let (stage, p) = apply_stage(now - scene.started);
            let lifted = !matches!(stage, ApplyStage::Peel | ApplyStage::Toss);
            let kind = (!lifted).then_some(scene.chosen);
            let shift = match stage {
                ApplyStage::Roll => crate::studio::backing_shift(p, self.canvas().w - t.x),
                _ => 0.0,
            };
            self.draw_slip(kind, TRAY_TOP + scene.used_y, shift, 1.0, out);
        }
        let printing = self.studio_slips.iter().find(|s| s.printing(now));
        let travel = self.studio_prints as f32 * t.slip_h
            + printing.map_or(0.0, |s| s.y(now) - s.y1);
        if let Some(liner) = self.studio_liner {
            let tile = 60.0 / 121.0;
            out.push(Draw::Part {
                x: t.x,
                y: 0.0,
                w: t.slip_w,
                h: 24.0,
                tex: liner,
                alpha: 1.0,
                uv: [0.0, 1.0 - tile - (travel.rem_euclid(60.0) / 121.0), 1.0, 24.0 / 121.0],
            });
        }
        let (px, pw) = (t.printer_x, t.printer_w);
        out.push(Draw::Rect { x: px, y: 20.0, w: pw, h: 34.0, colour: [0.0, 0.0, 0.0, 0.45] });
        if let Some(body) = self.studio_printer {
            out.push(Draw::Part {
                x: px,
                y: 16.0,
                w: pw,
                h: 32.0,
                tex: body,
                alpha: 1.0,
                uv: [0.1, 0.1, pw / 720.0, 32.0 / 58.0],
            });
        }
        let waiting = !studio.art_ready() && !studio.art_failed;
        let lit = self.studio_slips.iter().any(|s| s.lit(now));
        let colour = match crate::studio::printer_light(lit, waiting, now) {
            crate::studio::Light::Printing => [0.37, 0.88, 0.48, 1.0],
            crate::studio::Light::Waiting => [0.25, 0.55, 1.0, 1.0],
            crate::studio::Light::Off => [0.25, 0.25, 0.25, 1.0],
        };
        out.push(Draw::Rect {
            x: px + pw - 18.0,
            y: 28.0,
            w: 7.0,
            h: 7.0,
            colour,
        });
    }

    fn set_cart_label(&mut self, platform: Platform, stem: &str, label: Option<PathBuf>) {
        for (_, shelf) in self.shelves.iter_mut() {
            for i in 0..shelf.carts.len() {
                let cart = &mut shelf.carts[i];
                if cart.platform != platform || cart.stem != stem {
                    continue;
                }
                cart.label = label.clone();
                self.orphaned.extend(shelf.take_face(i));
            }
        }
        self.core_stale = true;
    }

    fn studio_apply(&mut self, kind: crate::studio::LabelKind) -> bool {
        use crate::studio::LabelKind;
        self.finish_studio_write(true);
        let (Some(root), Some(studio)) = (self.root.clone(), self.studio.as_mut()) else {
            return false;
        };
        let cart = studio.cart.clone();
        let art = self.studio_art.as_ref();
        let src = match kind {
            LabelKind::Real => art.and_then(|a| a.real.clone()),
            LabelKind::Logo => art.and_then(|a| a.logo.clone()),
            LabelKind::Existing => match crate::art::make::kept_label(&root, &cart) {
                Ok(kept) => kept,
                Err(_) => return false,
            },
            LabelKind::Title => None,
        };
        if matches!(kind, LabelKind::Real | LabelKind::Logo) && src.is_none() {
            return false;
        }
        let undo = studio.clone();
        studio.applied(kind);
        self.studio_applied = true;
        let write = std::thread::spawn(move || {
            (cart.label.is_none() || crate::art::make::keep_existing(&root, &cart).is_ok())
                && crate::art::make::apply_label(&root, &cart, src.as_deref()).is_ok()
        });
        self.studio_write = Some((write, Box::new(undo)));
        true
    }

    fn finish_studio_write(&mut self, wait: bool) {
        if !wait && self.studio_write.as_ref().is_some_and(|w| !w.0.is_finished()) {
            return;
        }
        let Some((write, undo)) = self.studio_write.take() else {
            return;
        };
        if !write.join().unwrap_or(false) {
            self.studio = Some(*undo);
            self.studio_scene = None;
        }
    }

    pub fn close_studio(&mut self) {
        self.finish_studio_write(true);
        let Some(studio) = self.studio.take() else {
            return;
        };
        self.studio_closing = None;
        if self.studio_fetched && self.studio_art.is_none() {
            self.studio_stale += 1;
        }
        let (platform, stem) = (studio.cart.platform, studio.cart.stem.clone());
        if let Some(root) = self.root.clone() {
            if studio.shell != studio.cart.shell {
                let _ = slot_store::cart_shell::write_choice(&root, &stem, studio.shell.as_ref());
            }
            if self.studio_applied {
                let path = crate::art::make::label_path(&root, &studio.cart);
                self.set_cart_label(platform, &stem, path.is_file().then_some(path));
            }
            if studio.recolored() && studio.wearing_text() {
                let mut edited = studio.cart.clone();
                edited.shell = studio.shell;
                let png = studio.ground.map(|c| crate::art::make::text_label(&edited, c));
                let path = crate::art::make::label_path(&root, &studio.cart);
                let _ = match &png {
                    Some(png) => crate::art::make::relabel(&root, &studio.cart, png),
                    None => crate::art::make::apply_label(&root, &studio.cart, None),
                };
                self.set_cart_label(platform, &stem, path.is_file().then_some(path));
                let _ = std::fs::remove_dir_all(root.join(crate::art::make::STUDIO_CACHE));
            } else if studio.recolored() {
                self.art_pending += 1;
                self.studio_saving = Some(stem.clone());
                self.art_jobs.ask(ArtJob::StudioClose {
                    cart: studio.cart.clone(),
                    ground: studio.ground,
                    look: self.studio_art.as_ref().and_then(|a| a.look.clone()),
                });
            } else {
                let _ = std::fs::remove_dir_all(root.join(crate::art::make::STUDIO_CACHE));
            }
        }
        if studio.shell != studio.cart.shell {
            self.set_cart_shell(platform, &stem, studio.shell);
        }
    }

    fn studio_input(&mut self, action: Action) {
        let Action::GbaDown(btn) = action else {
            return;
        };
        if self.studio_scene.is_some() || self.studio_closing.is_some() {
            return;
        }
        let now = self.now();
        let (last, at, run) = self.studio_repeat;
        let run = if last == Some(btn) && now.saturating_sub(at) < 300 { run + 1 } else { 0 };
        self.studio_repeat = (Some(btn), now, run);
        let Some(studio) = &mut self.studio else {
            return;
        };
        if matches!(studio.screen, crate::studio::Screen::Custom { .. })
            && matches!(btn, Btn::Left | Btn::Right)
        {
            let step = if run > 8 { 5 } else { 1 };
            return studio.nudge(if btn == Btn::Left { -step } else { step });
        }
        let was = studio.wearing;
        let used_y = match studio.screen {
            crate::studio::Screen::Sheet { at } => {
                let picked = studio.sheet.get(at).copied();
                let slip = self.studio_slips.iter().find(|s| Some(s.kind) == picked);
                let resting = slip.filter(|s| !s.moving(now as f64));
                if btn == Btn::A && resting.is_none() {
                    return;
                }
                resting.map_or(0.0, |s| s.y1)
            }
            _ => 0.0,
        };
        let from = studio.screen;
        let effect = studio.press(btn);
        if let (crate::studio::Screen::Sheet { at: a }, crate::studio::Screen::Sheet { at: b }) =
            (from, studio.screen)
        {
            if a != b && effect.is_none() {
                self.studio_shuffled = now as f64;
            }
        }
        match effect {
            Some(crate::studio::Effect::Close) => self.studio_closing = Some(now),
            Some(crate::studio::Effect::Fetch) if !self.studio_fetched => self.fetch_studio_art(),
            Some(crate::studio::Effect::Apply(kind)) if self.studio_apply(kind) => {
                self.studio_scene = Some(crate::studio::ApplyScene {
                    chosen: kind,
                    was,
                    used_y,
                    started: now as f64,
                });
            }
            _ => {}
        }
    }

    pub fn set_art_jobs(&mut self, jobs: Box<dyn ArtJobs>) {
        self.art_jobs = jobs;
    }

    pub fn fetch_art(&self) -> Option<&FetchArt> {
        self.fetch_art.as_ref()
    }

    pub fn fetch_landed(&self) -> &[ArtLanded] {
        &self.fetch_landed
    }

    pub fn open_fetch_art(&mut self) {
        let missing = self
            .root
            .as_deref()
            .map_or(0, |r| crate::art::rom::missing_art(r).len());
        self.fetch_art = Some(FetchArt::Choice {
            pick: ArtKind::Real,
            missing,
        });
    }

    fn cancel_fetch_art(&mut self) {
        if let Some(FetchArt::Running(run)) = &mut self.fetch_art {
            if !run.stopping {
                self.art_jobs.ask(ArtJob::Cancel);
                let rest = run.stop();
                self.fetch_landed.extend(rest);
            }
        }
    }

    fn fetch_art_input(&mut self, action: Action) {
        let Action::GbaDown(btn) = action else {
            return;
        };
        match (&mut self.fetch_art, btn) {
            (Some(FetchArt::Choice { pick, .. }), Btn::Left | Btn::Right) => {
                *pick = match pick {
                    ArtKind::Real => ArtKind::Logo,
                    ArtKind::Logo => ArtKind::Real,
                };
            }
            (Some(FetchArt::Choice { pick, .. }), Btn::A) => {
                let kind = *pick;
                self.art_pending += 1;
                self.hold_wifi();
                self.art_jobs.ask(ArtJob::FetchMissing(kind));
                self.fetch_art = Some(FetchArt::Running(ArtRun::new(kind)));
            }
            (Some(FetchArt::Choice { .. }), Btn::B) => self.fetch_art = None,
            (Some(FetchArt::Running(_)), Btn::B) => self.cancel_fetch_art(),
            (Some(FetchArt::Running(run)), Btn::X)
                if run.scene.is_some() && !run.stopping && !run.skipping =>
            {
                let rest = run.skip();
                self.fetch_landed.extend(rest);
            }
            (Some(FetchArt::Summary { .. }), Btn::A | Btn::B) => self.fetch_art = None,
            _ => {}
        }
    }

    pub fn art_busy(&self) -> bool {
        self.art_pending > 0
    }

    pub fn require_fetch_faces(&mut self) {
        self.fetch_faces_required = true;
    }

    pub fn fetch_wanted(&self) -> Vec<ArtLanded> {
        if let Some(FetchArt::Summary { featured, .. }) = &self.fetch_art {
            return featured.iter().cloned().collect();
        }
        let Some(FetchArt::Running(run)) = &self.fetch_art else {
            return Vec::new();
        };
        if let Some((cart, _)) = &run.closing {
            return vec![cart.clone()];
        }
        if run.skipping {
            return Vec::new();
        }
        run.scene
            .iter()
            .map(|s| s.cart.clone())
            .chain(run.queue.iter().cloned())
            .take(2)
            .collect()
    }

    pub fn fetch_cart(&self, platform: Platform, stem: &str) -> Option<Cart> {
        self.carts()
            .find(|c| c.platform == platform && c.stem == stem)
            .cloned()
    }

    pub fn set_fetch_tex(&mut self, platform: Platform, stem: &str, tex: FetchTex) {
        let wanted = self.fetch_wanted();
        self.fetch_tex
            .retain(|(p, s, _)| wanted.iter().any(|l| l.platform == *p && l.stem == *s));
        self.fetch_tex
            .retain(|(p, s, _)| !(*p == platform && s == stem));
        self.fetch_tex.push((platform, stem.to_string(), tex));
    }

    pub fn set_fetch_example_tex(&mut self, tex: [(TexId, u32, u32); 2]) {
        self.fetch_example_tex = Some(tex);
    }

    pub fn fetch_can_skip(&self) -> bool {
        matches!(
            &self.fetch_art,
            Some(FetchArt::Running(r)) if r.scene.is_some() && !r.skipping && !r.stopping
        )
    }

    pub fn fetch_featured_ready(&self) -> bool {
        match &self.fetch_art {
            Some(FetchArt::Summary { featured: Some(cart), .. }) => self
                .fetch_tex
                .iter()
                .any(|(p, s, _)| *p == cart.platform && *s == cart.stem),
            _ => false,
        }
    }

    pub fn fetch_examples_ready(&self) -> bool {
        self.fetch_example_tex.is_some()
    }

    pub fn set_fetch_legends(&mut self, legends: [[(TexId, u32); 5]; 2]) {
        self.fetch_legends = Some(legends);
    }

    pub fn set_fetch_text(&mut self, text: Vec<(TexId, u32, u32)>) {
        self.fetch_text = text;
    }

    pub fn set_fetch_paper(&mut self, tex: TexId) {
        self.fetch_paper = Some(tex);
    }

    pub fn fetch_art_lines(&self) -> Vec<String> {
        let carts = |n: usize, one: &str, many: &str| match n {
            1 => format!("1 cart {one}"),
            n => format!("{n} carts {many}"),
        };
        match &self.fetch_art {
            None => Vec::new(),
            Some(FetchArt::Choice { missing, .. }) => vec![
                "Fetch Missing Art".into(),
                carts(*missing, "has no art", "have no art"),
                "Real Label".into(),
                "Logo Only".into(),
            ],
            Some(FetchArt::Running(run)) => match &run.scene {
                None if (run.skipping || run.closing.is_some()) && !run.stopping => vec![
                    "Applying Labels".into(),
                    format!("{} of {}", run.landed + run.missed.len(), run.total),
                ],
                Some(scene) => vec![
                    "Applying Labels".into(),
                    slot_ui::clean_label(&scene.cart.stem),
                    format!("{} of {}", scene.number, run.total.max(scene.number)),
                ],
                None if run.stopping => vec!["Stopping".into()],
                None if !run.counted && !self.wifi_up() => vec!["Connecting to Wi-Fi".into()],
                None => vec!["Fetching Art".into()],
            },
            Some(FetchArt::Summary {
                stopped,
                landed,
                missed,
                failed,
                ..
            }) => {
                let heading = match failed {
                    Some(UpdateFail::Offline | UpdateFail::NoWifi) => "Wi-Fi Lost",
                    Some(UpdateFail::Clock) => "Clock Not Set",
                    Some(_) => "Stopped",
                    None if *stopped => "Stopped",
                    None => "All Done",
                };
                let mut lines = vec![heading.into(), "Added".into(), landed.to_string()];
                if !missed.is_empty() {
                    lines.push("No Art Found".into());
                    lines.push(missed.len().to_string());
                }
                lines
            }
        }
    }

    fn draw_fetch_text(&self, line: usize, cx: f32, y: f32, out: &mut Vec<Draw>) {
        let Some(&(tex, w, h)) = self.fetch_text.get(line) else {
            return;
        };
        out.push(Draw::Tex {
            x: (cx - w as f32 / 2.0).floor(),
            y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha: 1.0,
        });
    }

    fn draw_fetch_progress(&self, done: usize, total: usize, out: &mut Vec<Draw>) {
        const BAR_W: f32 = 420.0;
        const BAR_H: f32 = 14.0;
        const LINE_H: f32 = 40.0;
        const GAP: f32 = 18.0;
        let mid = self.canvas().w / 2.0;
        let room = OUT_H as f32 - MOUTH_H;
        let top = ((room - (2.0 * LINE_H + BAR_H + 2.0 * GAP)) / 2.0).floor();
        let (x, y) = ((mid - BAR_W / 2.0).floor(), top + LINE_H + GAP);
        self.draw_fetch_text(0, mid, top, out);
        out.push(Draw::Rect { x, y, w: BAR_W, h: BAR_H, colour: [1.0, 1.0, 1.0, 0.2] });
        let filled = done.min(total) as f32 / total.max(1) as f32;
        if filled > 0.0 {
            out.push(Draw::Rect { x, y, w: BAR_W * filled, h: BAR_H, colour: [1.0, 1.0, 1.0, 1.0] });
        }
        self.draw_fetch_text(1, mid, y + BAR_H + GAP, out);
    }

    fn draw_fetch_art(&self, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        slot_ui::draw_menu_ground(canvas, self.menu_ground(), out);
        let mid = canvas.w / 2.0;
        let legend = match &self.fetch_art {
            Some(FetchArt::Choice { pick, .. }) => {
                self.draw_fetch_text(0, mid, 26.0, out);
                self.draw_fetch_choice(*pick, out);
                0
            }
            Some(FetchArt::Running(run)) => {
                match run.scene.as_ref() {
                    None if (run.skipping || run.closing.is_some()) && !run.stopping => {
                        let done = run.landed + run.missed.len();
                        self.draw_fetch_progress(done, run.total, out);
                    }
                    Some(scene) => {
                        self.draw_fetch_scene(scene, out);
                        self.draw_fetch_text(0, mid, 26.0, out);
                        self.draw_fetch_text(1, mid, slot_ui::slot_name_y() - 30.0, out);
                        self.draw_fetch_text(2, mid, slot_ui::slot_name_y(), out);
                    }
                    None => {
                        let lines = self.fetch_text.len().min(2) as f32;
                        let top = ((OUT_H as f32 - MOUTH_H - 40.0 * lines) / 2.0).floor();
                        self.draw_fetch_text(0, mid, top, out);
                        self.draw_fetch_text(1, mid, top + 40.0, out);
                    }
                }
                2
            }
            Some(FetchArt::Summary { featured, .. }) => {
                self.draw_fetch_summary(featured.as_ref(), out);
                3
            }
            None => return,
        };
        draw_empty_slot(canvas, self.slot_faces, out);
        if let Some(legends) = self.fetch_legends {
            let legends = legends[usize::from(self.light())];
            self.draw_side_legend(legends[legend], false, LINK_LEGEND_Y, out);
            if legend == 0 {
                self.draw_side_legend(legends[1], true, LINK_LEGEND_Y, out);
            }
            if self.fetch_can_skip() {
                self.draw_side_legend(legends[4], true, LINK_LEGEND_Y, out);
            }
        }
    }

    fn draw_fetch_summary(&self, featured: Option<&ArtLanded>, out: &mut Vec<Draw>) {
        const MARGIN: f32 = 44.0;
        const COLUMN: f32 = 288.0;
        const ROW: f32 = 40.0;
        let ground = OUT_H as f32 - MOUTH_H;
        let canvas_w = self.canvas().w;
        let (left, right) = match featured {
            Some(_) => (MARGIN + COLUMN + 40.0, canvas_w - MARGIN),
            None => (canvas_w / 2.0 - 152.0, canvas_w / 2.0 + 152.0),
        };
        let tex = featured.and_then(|cart| {
            self.fetch_tex
                .iter()
                .find(|(p, s, _)| *p == cart.platform && *s == cart.stem)
        });
        if let Some((_, _, tex)) = tex {
            let scale = (COLUMN / tex.w as f32).min(300.0 / tex.h as f32);
            let (w, h) = (tex.w as f32 * scale, tex.h as f32 * scale);
            let (x, y) = (MARGIN + (COLUMN - w) / 2.0, ((ground - h) / 2.0).floor());
            for under in tex.under.into_iter().flatten() {
                out.push(Draw::Tex { x, y, w, h, tex: under, alpha: 1.0 });
            }
            out.push(Draw::Tex { x, y, w, h, tex: tex.done, alpha: 1.0 });
        }
        let rows = (self.fetch_text.len().saturating_sub(1) / 2) as f32;
        let block = 64.0 + rows * ROW;
        let top = ((ground - block) / 2.0).floor();
        self.draw_fetch_text(0, (left + right) / 2.0, top, out);
        for row in 0..rows as usize {
            let y = top + 64.0 + row as f32 * ROW;
            if let Some(&(tex, w, h)) = self.fetch_text.get(1 + row * 2) {
                out.push(Draw::Tex { x: left, y, w: w as f32, h: h as f32, tex, alpha: 1.0 });
            }
            if let Some(&(tex, w, h)) = self.fetch_text.get(2 + row * 2) {
                out.push(Draw::Tex { x: right - w as f32, y, w: w as f32, h: h as f32, tex, alpha: 1.0 });
            }
        }
    }

    fn draw_fetch_choice(&self, pick: ArtKind, out: &mut Vec<Draw>) {
        const SCALE: f32 = 0.8;
        const TOP: f32 = 150.0;
        let Some(faces) = self.fetch_example_tex else {
            return;
        };
        self.draw_fetch_text(1, self.canvas().w / 2.0, 72.0, out);
        let [left, right] = crate::fetch_art::choice_xs(self.canvas().w);
        for (i, (x, kind)) in [(left, ArtKind::Real), (right, ArtKind::Logo)].into_iter().enumerate() {
            let (tex, w, h) = faces[i];
            let (w, h) = (w as f32 * SCALE, h as f32 * SCALE);
            let on = kind == pick;
            let y = if on { TOP - 10.0 } else { TOP };
            out.push(Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha: if on { 1.0 } else { 0.6 },
            });
            self.draw_fetch_text(2 + i, x + w / 2.0, TOP + h + 16.0, out);
        }
    }

    fn draw_fetch_scene(&self, scene: &ArtScene, out: &mut Vec<Draw>) {
        let Some((_, _, tex)) = self
            .fetch_tex
            .iter()
            .find(|(p, s, _)| *p == scene.cart.platform && *s == scene.cart.stem)
        else {
            return;
        };
        let (scale, well) = match scene.cart.platform {
            Platform::Gba | Platform::Gbav => (
                1.0,
                [slot_ui::LABEL_X, slot_ui::LABEL_Y, slot_ui::LABEL_W, slot_ui::LABEL_H],
            ),
            Platform::Gb => (
                0.72,
                [slot_ui::GB_LABEL_X, slot_ui::GB_LABEL_Y, slot_ui::GB_LABEL_W, slot_ui::GB_LABEL_H],
            ),
            Platform::Gbc => (
                0.72,
                [slot_ui::GBC_LABEL_X, slot_ui::GBC_LABEL_Y, slot_ui::GBC_LABEL_W, slot_ui::GBC_LABEL_H],
            ),
        };
        let (fw, fh) = (tex.w as f32, tex.h as f32);
        let (w, h) = (fw * scale, fh * scale);
        let x = ((self.canvas().w - w) / 2.0).floor();
        let y = ((OUT_H as f32 - MOUTH_H - h) / 2.0).floor();
        let rect = [
            x + well[0] as f32 * scale,
            y + well[1] as f32 * scale,
            well[2] as f32 * scale,
            well[3] as f32 * scale,
        ];
        let under = tex.under;
        let face = |t: TexId, dx: f32, alpha: f32, out: &mut Vec<Draw>| {
            for u in under.into_iter().flatten() {
                out.push(Draw::Tex {
                    x: x + dx,
                    y,
                    w,
                    h,
                    tex: u,
                    alpha,
                });
            }
            out.push(Draw::Tex {
                x: x + dx,
                y,
                w,
                h,
                tex: t,
                alpha,
            })
        };
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let (stage, p) = stage_at(self.now() as f64 - scene.started_ms);
        let sticker = |t: TexId, flat: f32, curl: f32, pose: slot_ui::sticker_curl::Pose, out: &mut Vec<Draw>| {
            let focus = [self.canvas().w / 2.0, 240.0];
            for strip in slot_ui::sticker_curl::strips(rect, flat, curl, &pose, focus) {
                let (tex, uv) = match (strip.back, self.fetch_paper) {
                    (true, Some(paper)) => (paper, [0.0, 0.0, 1.0, 1.0]),
                    _ => (
                        t,
                        [
                            (well[0] as f32 + strip.uv[0] * well[2] as f32) / fw,
                            well[1] as f32 / fh,
                            strip.uv[2] * well[2] as f32 / fw,
                            well[3] as f32 / fh,
                        ],
                    ),
                };
                out.push(Draw::Quad {
                    corners: strip.corners,
                    tex,
                    uv,
                    alpha: 1.0,
                });
            }
        };
        let rest = slot_ui::sticker_curl::Pose::REST;
        match stage {
            Stage::Arrive => face(tex.title, -40.0 * (1.0 - p), p, out),
            Stage::Hold => face(tex.title, 0.0, 1.0, out),
            Stage::Peel => {
                face(tex.bare, 0.0, 1.0, out);
                let q = p * p;
                sticker(tex.title, 1.0 - q, lerp(150.0, 420.0, q), rest, out);
            }
            Stage::Toss => {
                face(tex.bare, 0.0, 1.0, out);
                let (q, arc) = (p * p, (p * std::f32::consts::PI).sin());
                let pose = slot_ui::sticker_curl::Pose {
                    x: -600.0 * q,
                    y: -70.0 * q - 60.0 * arc,
                    z: 140.0 * arc,
                    turn_deg: -28.0 * q,
                    scale: 1.0,
                };
                sticker(tex.title, 0.0, 420.0, pose, out);
            }
            Stage::Fly => {
                face(tex.bare, 0.0, 1.0, out);
                let e = 1.0 - (1.0 - p).powi(3);
                let pose = slot_ui::sticker_curl::Pose {
                    x: lerp(680.0, 0.0, e),
                    y: lerp(-120.0, 0.0, e),
                    z: lerp(160.0, 0.0, e),
                    turn_deg: lerp(12.0, 0.0, e),
                    scale: 1.0,
                };
                sticker(tex.done, 0.0, 150.0, pose, out);
            }
            Stage::Roll => {
                face(tex.bare, 0.0, 1.0, out);
                sticker(tex.done, p, 150.0, rest, out);
            }
            Stage::Rest => face(tex.done, 0.0, 1.0, out),
            Stage::Leave => face(tex.done, 0.0, 1.0 - p, out),
        }
    }

    pub fn cart_shells(&self, platform: Platform, stem: &str) -> Vec<Option<slot_store::ShellChoice>> {
        self.shelves
            .iter()
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .filter(|c| c.platform == platform && c.stem == stem)
            .map(|c| c.shell)
            .collect()
    }

    pub fn set_cart_shell(
        &mut self,
        platform: Platform,
        stem: &str,
        shell: Option<slot_store::ShellChoice>,
    ) {
        for (_, shelf) in self.shelves.iter_mut() {
            for i in 0..shelf.carts.len() {
                let cart = &mut shelf.carts[i];
                if cart.platform != platform || cart.stem != stem {
                    continue;
                }
                cart.shell = shell;
                self.orphaned.extend(shelf.take_face(i));
                self.orphaned.extend(shelf.take_interior(i));
            }
        }
        self.core_stale = true;
    }

    pub fn cart_labels(&self, platform: Platform, stem: &str) -> Vec<Option<PathBuf>> {
        self.shelves
            .iter()
            .flat_map(|(_, shelf)| shelf.carts.iter())
            .filter(|c| c.platform == platform && c.stem == stem)
            .map(|c| c.label.clone())
            .collect()
    }

    fn relabel_landed(&mut self) {
        while self.fetch_relabeled < self.fetch_landed.len() {
            let landed = self.fetch_landed[self.fetch_relabeled].clone();
            self.fetch_relabeled += 1;
            for (_, shelf) in self.shelves.iter_mut() {
                for i in 0..shelf.carts.len() {
                    let cart = &mut shelf.carts[i];
                    if cart.platform != landed.platform || cart.stem != landed.stem {
                        continue;
                    }
                    cart.label = Some(landed.path.clone());
                    self.orphaned.extend(shelf.take_face(i));
                }
            }
            self.core_stale = true;
        }
    }

    fn take_art_events(&mut self) {
        self.steer_wifi();
        let now = self.now() as f64;
        if self
            .studio_scene
            .as_ref()
            .is_some_and(|s| now - s.started >= crate::studio::APPLY_MS)
        {
            self.studio_scene = None;
        }
        if self
            .studio_closing
            .is_some_and(|at| now - at as f64 >= STUDIO_SLIDE_MS as f64)
        {
            self.close_studio();
        }
        self.finish_studio_write(false);
        self.tick_tray(now);
        if self.fetch_art.is_none() {
            self.relabel_landed();
        }
        while let Some(event) = self.art_jobs.take() {
            if matches!(
                event,
                ArtEvent::Done { .. }
                    | ArtEvent::Failed(_)
                    | ArtEvent::StudioArt(_)
                    | ArtEvent::StudioSaved(_)
            ) {
                self.art_pending = self.art_pending.saturating_sub(1);
            }
            match (&mut self.fetch_art, event) {
                (_, ArtEvent::StudioSaved(stem)) => {
                    self.studio_saving = None;
                    self.reface(&stem);
                }
                (_, ArtEvent::StudioArt(_)) if self.studio_stale > 0 => self.studio_stale -= 1,
                (_, ArtEvent::StudioArt(art)) => {
                    if let Some(studio) = &mut self.studio {
                        if studio.cart.stem == art.stem {
                            studio.art_arrived(art.real.is_some(), art.logo.is_some());
                            studio.art_failed = art.failed.is_some();
                            studio.duplicates = art.duplicates;
                            if let Some(ground) = art.logo_worn {
                                studio.adopt_logo(ground);
                            }
                            if art.wheel.is_some() {
                                studio.wheel_arrived();
                            }
                            self.studio_art = Some(art);
                        }
                    }
                }
                (Some(FetchArt::Running(run)), event) => run.event(event),
                _ => {}
            }
        }
        let now = self.now() as f64;
        let playable: Vec<(Platform, String)> = match &self.fetch_art {
            Some(FetchArt::Running(run)) if !run.queue.is_empty() => self
                .carts()
                .map(|c| (c.platform, c.stem.clone()))
                .collect(),
            _ => Vec::new(),
        };
        let Some(FetchArt::Running(run)) = &mut self.fetch_art else {
            return;
        };
        let unplayable = run.drop_unplayable(&|l: &ArtLanded| {
            playable.iter().any(|(p, s)| *p == l.platform && *s == l.stem)
        });
        self.fetch_landed.extend(unplayable);
        let Some(FetchArt::Running(run)) = &mut self.fetch_art else {
            return;
        };
        let required = self.fetch_faces_required;
        let ready: Vec<(Platform, String)> = self
            .fetch_tex
            .iter()
            .map(|(p, s, _)| (*p, s.clone()))
            .collect();
        let done = run.tick(now, &|l: &ArtLanded| {
            !required || ready.iter().any(|(p, s)| *p == l.platform && *s == l.stem)
        });
        let built = |l: &ArtLanded| ready.iter().any(|(p, s)| *p == l.platform && *s == l.stem);
        if run.over() && run.closing.is_none() {
            let shown: Vec<&ArtLanded> = run.carts.iter().filter(|l| built(l)).collect();
            let picked = match shown.is_empty() {
                true => run.carts.get(now as usize % run.carts.len().max(1)),
                false => shown.get(now as usize % shown.len()).copied(),
            };
            run.closing = picked.cloned().map(|cart| (cart, now));
        }
        let waiting = run.closing.as_ref().is_some_and(|(cart, since)| {
            required && !built(cart) && now - since < SUMMARY_WAIT_MS
        });
        let over = (run.over() && !waiting).then(|| FetchArt::Summary {
            featured: run.closing.as_ref().map(|(cart, _)| cart.clone()),
            stopped: run.stopping,
            landed: run.landed,
            missed: run.missed.clone(),
            failed: run.failed.clone(),
        });
        self.fetch_landed.extend(done);
        if let Some(summary) = over {
            self.fetch_art = Some(summary);
        }
    }

    pub fn restart_chord(&self) -> bool {
        matches!(self.phase, Phase::About) && self.update.is_none()
    }

    pub fn update_offered(&self) -> bool {
        !self.update_busy && !self.art_busy() && !self.link_active()
    }

    fn has_saved(&self) -> bool {
        match &self.wifi {
            Some((w, _, _)) => !w.snapshot().saved.is_empty(),
            None => self.wifi_configured(),
        }
    }

    fn server_allowed(&self) -> bool {
        self.root.is_some() && self.seated_cart().is_none() && !self.link_active()
    }

    fn open_server(&mut self) {
        let Some(root) = self.root.clone().filter(|_| self.server_allowed()) else {
            return;
        };
        self.persist();
        self.server = Some(ServerSession::open(root, self.server_net.clone()));
    }

    fn server_input(&mut self, action: Action) {
        let now = self.now();
        if let (Action::GbaDown(Btn::B), Some(server)) = (action, &mut self.server) {
            server.stop(now);
        }
    }

    fn step_server(&mut self) {
        let now = self.now();
        let Some(server) = &mut self.server else {
            return;
        };
        server.poll(now);
        self.print_cues.extend(server.cues(now));
        if !server.finished(now) {
            return;
        }
        let failed = server.failed();
        let hurried = server.hurried();
        let rescan = server.take_rescan();
        self.server = None;
        self.reload_server_edits();
        match rescan {
            Some(rescan) => {
                self.adopt_rescan(rescan.carts);
                if !hurried {
                    self.stale_labels = rescan.stale;
                }
            }
            None => {
                self.restock_recents();
                self.restock_favorites();
            }
        }
        if self.powering_off {
            self.remember_shelf();
        }
        self.persist();
        if failed {
            self.set_update(Some(UpdateState::Notice(NetNotice::ServerFailed)));
        }
    }

    fn abandon_server(&mut self) {
        let now = self.now();
        if let Some(server) = &mut self.server {
            server.abandon(now);
        }
    }

    pub fn end_server(&mut self) {
        self.abandon_server();
        while self.server.is_some() {
            self.step_server();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn reload_server_edits(&mut self) {
        let Some(root) = &self.root else {
            return;
        };
        let disk = read_slot_state(root);
        self.state.recents = disk.recents;
        self.state.favorites = disk.favorites;
        self.state.cart = disk.cart;
        self.state.cart_platform = disk.cart_platform;
        self.state.shelf_cart = disk.shelf_cart;
    }

    fn adopt_rescan(&mut self, carts: Vec<Cart>) {
        let seats: Vec<(usize, Option<(Platform, String)>)> = self
            .shelves
            .iter()
            .map(|(_, shelf)| {
                let cart = shelf.carts.get(shelf.index);
                (shelf.index, cart.map(|c| (c.platform, c.stem.clone())))
            })
            .collect();
        for (at, (_, fresh)) in shelves_of(carts).into_iter().enumerate() {
            let old = self.shelves[at].1.restock(fresh.carts);
            self.orphaned.extend(old);
        }
        self.restock_recents();
        self.restock_favorites();
        for ((_, shelf), (index, seat)) in self.shelves.iter_mut().zip(seats) {
            let found = shelf
                .carts
                .iter()
                .position(|c| seat.as_ref().is_some_and(|(p, s)| c.platform == *p && c.stem == *s));
            if !shelf.carts.is_empty() {
                shelf.select(found.unwrap_or(index.min(shelf.carts.len() - 1)));
            }
        }
        if self.shelf().carts.is_empty() {
            self.shelf_at = self.next_shelf(1).unwrap_or(0);
        }
        self.shelf_platform = slot_ui::Printed::default();
        self.slot_letter = None;
        self.shelf_named = None;
        self.name_pending = true;
        self.core_stale = true;
        self.rescanned = true;
    }

    fn server_frame(&self) -> Option<(PrintFrame, PrintTextures)> {
        match self.shutting_down() {
            true => None,
            false => self.server.as_ref()?.frame(self.now()),
        }
    }

    fn draw_server(&self, frame: &PrintFrame, tex: &PrintTextures, out: &mut Vec<Draw>) {
        if frame.slide > 0.0 {
            self.draw_app(out);
            return print_draws(frame, tex, self.canvas(), out);
        }
        print_draws(frame, tex, self.canvas(), out);
        self.hud.draw(self.canvas(), self.now(), out);
    }

    pub fn server_open(&self) -> bool {
        self.server.is_some()
    }

    pub fn server_scene_running(&self) -> bool {
        self.server.as_ref().is_some_and(ServerSession::scene_running)
    }

    pub fn server_paper(&self) -> Option<&ServerPaper> {
        self.server.as_ref()?.paper()
    }

    pub fn set_server_net(&mut self, net: ServerNet) {
        self.server_net = net;
    }

    pub fn take_server_art(&mut self) -> PrintArtState {
        match &mut self.server {
            Some(server) => server.take_art(),
            None => PrintArtState::Pending,
        }
    }

    pub fn set_server_tex(&mut self, tex: PrintTextures) {
        if let Some(server) = &mut self.server {
            server.set_tex(tex);
        }
    }

    pub fn server_paper_ready(&self) -> bool {
        self.server.as_ref().is_some_and(ServerSession::paper_up)
    }

    pub fn server_paper_up(&mut self) {
        if let Some(server) = &mut self.server {
            server.set_paper_up();
        }
    }

    pub fn server_art_failed(&mut self) {
        let now = self.now();
        if let Some(server) = &mut self.server {
            server.fail(now);
        }
    }

    pub fn take_print_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.print_cues)
    }

    pub fn take_rescanned(&mut self) -> bool {
        std::mem::take(&mut self.rescanned)
    }

    pub fn take_stale_labels(&mut self) -> Vec<Cart> {
        std::mem::take(&mut self.stale_labels)
    }

    pub fn is_favorite(&self, platform: Platform, stem: &str) -> bool {
        self.state.is_favorite(platform, stem)
    }

    fn open_check_for_updates(&mut self) {
        if self.update_offered() {
            self.check_for_update();
        }
    }

    fn open_fetch_missing_art(&mut self) {
        if !self.link_active() {
            self.open_fetch_art();
        }
    }

    fn net_rows(&self) -> Vec<crate::networks::NetRow> {
        crate::networks::list_rows(self.wifi(), self.state.wifi_off)
    }

    fn open_networks(&mut self) {
        let joined = self.wifi().status.ssid.clone().filter(|_| self.wifi_up());
        self.phase = Phase::Networks {
            pick: joined.map_or(crate::networks::NetPick::Top, crate::networks::NetPick::Name),
        };
        if !self.state.wifi_off {
            self.hold_wifi();
        }
    }

    fn networks_input(&mut self, action: Action) {
        let rows = self.net_rows();
        let Phase::Networks { pick } = &self.phase else {
            return;
        };
        let at = crate::networks::selected(&rows, pick);
        let n = rows.len();
        let to = match action {
            Action::GbaDown(Btn::Up) if n > 0 => (at + n - 1) % n,
            Action::GbaDown(Btn::Down) if n > 0 => (at + 1) % n,
            Action::GbaDown(Btn::A) => {
                if let Some(row) = rows.get(at).cloned() {
                    self.pick_network(row);
                }
                return;
            }
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = Phase::QuickMenu {
                    row: QuickRow::Networks,
                };
                return;
            }
            _ => return,
        };
        self.phase = Phase::Networks {
            pick: crate::networks::NetPick::of(&rows[to]),
        };
    }

    fn network_scan_input(&mut self, action: Action) {
        use crate::networks::NetPick;
        let rows = crate::networks::scan_rows(self.wifi());
        let Phase::NetworkScan { pick } = &self.phase else {
            return;
        };
        let at = crate::networks::selected(&rows, pick);
        let n = rows.len();
        let to = match action {
            Action::GbaDown(Btn::Up) if n > 0 => (at + n - 1) % n,
            Action::GbaDown(Btn::Down) if n > 0 => (at + 1) % n,
            Action::GbaDown(Btn::A) => {
                if let Some(row) = rows.get(at).cloned() {
                    self.pick_network(row);
                }
                return;
            }
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = Phase::Networks { pick: NetPick::Scan };
                return;
            }
            _ => return,
        };
        self.phase = Phase::NetworkScan {
            pick: NetPick::of(&rows[to]),
        };
    }

    fn pick_network(&mut self, row: crate::networks::NetRow) {
        use crate::networks::{NetPick, NetRow, NAME_TITLE, PASSWORD_TITLE};
        match row {
            NetRow::Scan => {
                self.wifi_send(crate::wifi::Cmd::ClearScan);
                self.phase = Phase::NetworkScan { pick: NetPick::Top };
            }
            NetRow::Saved(s) => {
                self.phase = Phase::NetworkPage {
                    ssid: s.ssid,
                    row: 0,
                }
            }
            NetRow::Nearby(n) if n.secured => self.open_keyboard(
                PASSWORD_TITLE,
                Some(&n.ssid),
                KeyboardFor::Password {
                    ssid: n.ssid.clone(),
                    hidden: false,
                },
            ),
            NetRow::Nearby(n) => self.join_network(n.ssid, String::new(), false),
            NetRow::Other => self.open_keyboard(NAME_TITLE, None, KeyboardFor::NetworkName),
        }
    }

    fn open_keyboard(&mut self, title: &str, subtitle: Option<&str>, task: KeyboardFor) {
        let kb = slot_ui::Keyboard::new(slot_ui::Layout::General, title, subtitle, "");
        self.keyboard = Some((kb, task));
    }

    fn join_network(&mut self, ssid: String, password: String, hidden: bool) {
        self.joined_hidden = Some((ssid.clone(), hidden));
        self.wifi_send(crate::wifi::Cmd::Join {
            ssid: ssid.clone(),
            password,
            hidden,
        });
        self.phase = Phase::NetworkPage { ssid, row: 0 };
    }

    fn keyboard_input(&mut self, action: Action) {
        let btn = match action {
            Action::GbaDown(b) => b,
            Action::QuickMenu => Btn::Menu,
            _ => return,
        };
        let Some((kb, _)) = self.keyboard.as_mut() else {
            return;
        };
        if kb.help() {
            self.held_dir = None;
        }
        match kb.press(btn) {
            slot_ui::Typed::Cancelled => self.keyboard = None,
            slot_ui::Typed::Done(text) => self.keyboard_done(text),
            _ => {}
        }
    }

    fn keyboard_done(&mut self, text: String) {
        use crate::networks::{OPEN_HINT, PASSWORD_TITLE};
        use crate::wifi::wpa::password_ok;
        let Some(task) = self.keyboard.as_ref().map(|k| k.1.clone()) else {
            return;
        };
        let refuse = |a: &mut App| a.set_update(Some(UpdateState::Notice(NetNotice::Password)));
        match task {
            KeyboardFor::NetworkName if text.is_empty() => {}
            KeyboardFor::NetworkName => self.open_keyboard(
                PASSWORD_TITLE,
                Some(OPEN_HINT),
                KeyboardFor::Password {
                    ssid: text,
                    hidden: true,
                },
            ),
            KeyboardFor::Password { ssid, hidden } => {
                if !password_ok(&text, !hidden) {
                    return refuse(self);
                }
                self.keyboard = None;
                self.join_network(ssid, text, hidden);
            }
        }
    }

    fn network_page_input(&mut self, action: Action) {
        use crate::networks::{page_rows, PageRow};
        use crate::wifi::Cmd;
        let Phase::NetworkPage { ssid, row } = &self.phase else {
            return;
        };
        let (ssid, row) = (ssid.clone(), *row);
        let snap = self.wifi();
        let saved = snap.saved.iter().find(|s| s.ssid == ssid).cloned();
        let rows = page_rows(snap, &ssid, self.state.wifi_off);
        let n = rows.len();
        let at = row.min(n - 1);
        let to = match action {
            Action::GbaDown(Btn::Up) => (at + n - 1) % n,
            Action::GbaDown(Btn::Down) => (at + 1) % n,
            Action::GbaDown(Btn::B) | Action::QuickMenu => {
                self.phase = Phase::Networks {
                    pick: crate::networks::NetPick::Name(ssid),
                };
                return;
            }
            Action::GbaDown(Btn::A | Btn::Left | Btn::Right) => {
                let a = action == Action::GbaDown(Btn::A);
                if a && rows[at] == PageRow::Password {
                    let hidden = self
                        .joined_hidden
                        .as_ref()
                        .is_some_and(|(s, hidden)| *s == ssid && *hidden);
                    let task = KeyboardFor::Password {
                        ssid: ssid.clone(),
                        hidden,
                    };
                    return self.open_keyboard(crate::networks::PASSWORD_TITLE, Some(&ssid), task);
                }
                let Some(saved) = saved else {
                    return;
                };
                match rows[at] {
                    PageRow::AutoJoin => self.wifi_send(Cmd::AutoJoin {
                        id: saved.id,
                        on: !saved.auto_join,
                    }),
                    PageRow::Connect if a => self.wifi_send(Cmd::Connect(saved.id)),
                    PageRow::Disconnect if a => self.wifi_send(Cmd::Disconnect),
                    PageRow::Forget if a => self.forget_prompt = true,
                    _ => {}
                }
                return;
            }
            _ => return,
        };
        self.phase = Phase::NetworkPage { ssid, row: to };
    }

    fn forget_input(&mut self, action: Action) {
        match action {
            Action::GbaDown(Btn::A) => {
                self.forget_prompt = false;
                let Phase::NetworkPage { ssid, .. } = &self.phase else {
                    return;
                };
                let ssid = ssid.clone();
                let mut ids: Vec<u32> = self
                    .wifi()
                    .saved
                    .iter()
                    .filter(|s| s.ssid == ssid)
                    .map(|s| s.id)
                    .collect();
                ids.sort_unstable_by(|a, b| b.cmp(a));
                for id in ids {
                    self.wifi_send(crate::wifi::Cmd::Forget(id));
                }
                self.forgotten = Some(ssid.clone());
                self.hud.toast(Toast::Forgotten, self.now());
                self.phase = Phase::Networks {
                    pick: crate::networks::NetPick::Name(ssid),
                };
            }
            Action::GbaDown(Btn::B) => self.forget_prompt = false,
            _ => {}
        }
    }

    pub fn forgotten_network(&self) -> Option<&str> {
        self.forgotten.as_deref()
    }

    pub fn toast_tex(&self, toast: Toast) -> Option<TexId> {
        self.hud.toast_tex(toast)
    }

    pub fn forget_prompt(&self) -> Option<String> {
        match (&self.phase, self.forget_prompt) {
            (Phase::NetworkPage { ssid, .. }, true) => Some(format!("Forget {ssid}?")),
            _ => None,
        }
    }

    pub fn network_screen(&self) -> Option<crate::networks::NetScreen> {
        use crate::networks::{
            list_line, list_message, list_rows, page_line, page_rows, scan_message, scan_rows,
            selected, NetScreen,
        };
        let (snap, off) = (self.wifi(), self.state.wifi_off);
        match &self.phase {
            Phase::Networks { pick } => {
                let rows = list_rows(snap, off);
                Some(NetScreen {
                    selected: selected(&rows, pick),
                    lines: rows.iter().map(|r| list_line(r, snap)).collect(),
                    message: list_message(off),
                })
            }
            Phase::NetworkScan { pick } => {
                let rows = scan_rows(snap);
                Some(NetScreen {
                    selected: selected(&rows, pick),
                    lines: rows.iter().map(|r| list_line(r, snap)).collect(),
                    message: scan_message(snap),
                })
            }
            Phase::NetworkPage { ssid, row } => {
                let rows = page_rows(snap, ssid, off);
                Some(NetScreen {
                    selected: (*row).min(rows.len() - 1),
                    lines: rows.iter().map(|r| page_line(*r, snap, ssid, off)).collect(),
                    message: None,
                })
            }
            _ => None,
        }
    }

    pub fn network_action(&self) -> Option<&'static str> {
        use crate::networks::{list_rows, page_rows, scan_rows, selected, NetRow, PageRow};
        let (snap, off) = (self.wifi(), self.state.wifi_off);
        let pick_action = |row: &NetRow| match row {
            NetRow::Saved(_) => "Open",
            NetRow::Nearby(_) | NetRow::Other => "Join",
            NetRow::Scan => "Scan",
        };
        match &self.phase {
            Phase::Networks { pick } => {
                let rows = list_rows(snap, off);
                rows.get(selected(&rows, pick)).map(pick_action)
            }
            Phase::NetworkScan { pick } => {
                let rows = scan_rows(snap);
                rows.get(selected(&rows, pick)).map(pick_action)
            }
            Phase::NetworkPage { ssid, row } => {
                let rows = page_rows(snap, ssid, off);
                match rows[(*row).min(rows.len() - 1)] {
                    PageRow::Status | PageRow::Ip => None,
                    PageRow::Password => Some("Open"),
                    PageRow::Connect => Some("Connect"),
                    PageRow::Disconnect => Some("Disconnect"),
                    PageRow::AutoJoin => Some("Change"),
                    PageRow::Forget => Some("Forget"),
                }
            }
            _ => None,
        }
    }

    pub fn network_back(&self) -> &'static str {
        let scanning = matches!(self.phase, Phase::NetworkScan { .. }) && self.wifi().scan.is_none();
        if scanning {
            "Cancel"
        } else {
            "Back"
        }
    }

    pub fn keyboard(&self) -> Option<&slot_ui::Keyboard> {
        self.keyboard.as_ref().map(|k| &k.0)
    }

    pub fn keyboard_for(&self) -> Option<&KeyboardFor> {
        self.keyboard.as_ref().map(|k| &k.1)
    }

    pub fn take_keyboard_tex(&mut self) -> Option<slot_ui::KeyboardTex> {
        self.keyboard_tex.take()
    }

    pub fn set_keyboard_tex(&mut self, tex: Option<slot_ui::KeyboardTex>) {
        self.keyboard_tex = tex;
    }

    pub fn set_network_faces(
        &mut self,
        lines: Vec<slot_ui::NetLine>,
        message: Option<slot_ui::NetFace>,
    ) {
        self.net_lines = lines;
        self.net_message = message;
    }

    pub fn set_network_icons(&mut self, icons: slot_ui::NetIcons) {
        self.net_icons = Some(icons);
    }

    pub fn set_network_legends(&mut self, legends: NetLegends) {
        self.net_legends = Some(legends);
    }

    pub fn set_forget_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.forget_faces = faces;
    }

    fn draw_networks(&self, out: &mut Vec<Draw>) {
        if let (Some((kb, _)), Some(tex)) = (&self.keyboard, &self.keyboard_tex) {
            slot_ui::draw_keyboard(self.canvas(), self.menu_ground(), tex, kb, out);
            if kb.help() {
                return;
            }
            draw_empty_slot(self.canvas(), self.slot_faces, out);
            slot_ui::draw_keyboard_legends(self.canvas(), tex, out);
            return;
        }
        slot_ui::NetworkList {
            lines: &self.net_lines,
            icons: self.net_icons,
            selected: self.network_screen().map_or(0, |s| s.selected),
            message: self.net_message,
            pointer: self.quick_menu_faces.as_ref().map(|f| f.pointer),
            ground: self.menu_ground(),
            canvas: self.canvas(),
        }
        .draw(out);
        draw_empty_slot(self.canvas(), self.slot_faces, out);
        let Some(faces) = &self.quick_menu_faces else {
            return;
        };
        let light = self.light();
        let [back, change, open, ..] = match light {
            true => faces.legend_light,
            false => faces.legend,
        };
        let net = self.net_legends.as_ref();
        let left = match self.network_back() {
            "Cancel" => net.map_or(back, |n| n.cancel[light as usize]),
            _ => back,
        };
        self.draw_side_legend(left, false, LINK_LEGEND_Y, out);
        let right = match self.network_action() {
            Some("Open") => Some(open),
            Some("Change") => Some(change),
            Some("Join") => net.map(|n| n.join[light as usize]),
            Some("Scan") => net.map(|n| n.scan[light as usize]),
            Some("Connect") => net.map(|n| n.connect[light as usize]),
            Some("Disconnect") => net.map(|n| n.disconnect[light as usize]),
            Some("Forget") => net.map(|n| n.forget[light as usize]),
            _ => None,
        };
        if let Some(right) = right {
            self.draw_side_legend(right, true, LINK_LEGEND_Y, out);
        }
    }

    pub fn networks_value(&self) -> String {
        if self.state.wifi_off {
            return "Off".into();
        }
        match &self.wifi().status.ssid {
            Some(ssid) if self.wifi_up() => ssid.clone(),
            _ => "Not Connected".into(),
        }
    }

    pub fn update_shown(&self) -> bool {
        self.update.is_some()
    }

    pub fn update_progress(&self) -> Option<f32> {
        match self.update.as_ref()? {
            UpdateState::Downloading(r, got) | UpdateState::Cancelling(r, got) => {
                Some((*got as f32 / r.size.max(1) as f32).clamp(0.0, 1.0))
            }
            UpdateState::Closing(_) => Some(1.0),
            _ => None,
        }
    }

    fn set_update(&mut self, state: Option<UpdateState>) {
        self.update = state;
        self.update_at = self.now();
        self.update_back = false;
    }

    pub fn update_text(&self) -> Option<(String, String)> {
        let running = crate::build_info::Build::current().version;
        let retry = "Nothing was changed. Try again later.";
        let (title, detail) = match self.update.as_ref()? {
            UpdateState::Checking => ("Checking for updates".into(), String::new()),
            UpdateState::Available(offers, _) => (
                match offers.len() {
                    1 => "An update is available",
                    _ => "Updates are available",
                }
                .into(),
                String::new(),
            ),
            UpdateState::Battery(_) => (
                "Battery is low".into(),
                "Plug in before updating BaseOS. A installs anyway.".into(),
            ),
            UpdateState::UpToDate => (
                "slot is up to date".into(),
                format!("{running} is the latest version."),
            ),
            UpdateState::Notice(NetNotice::Password) => ("Invalid Password".into(), String::new()),
            UpdateState::Notice(NetNotice::ServerFailed) => {
                ("Gamebit failed".into(), "Gamebit has stopped.".into())
            }
            UpdateState::Downloading(..)
            | UpdateState::Cancelling(..)
            | UpdateState::Closing(_) => return None,
            UpdateState::Failed(f) => {
                let (title, detail) = match f {
                    UpdateFail::NoWifi => (
                        "Could not join Wi-Fi",
                        "Check Network in the Wireless menu.",
                    ),
                    UpdateFail::Offline => (
                        "Could not connect",
                        "Check the network has internet access.",
                    ),
                    UpdateFail::Clock => (
                        "Could not check for updates",
                        "Set the date and time, then try again.",
                    ),
                    UpdateFail::Download => ("Download failed", retry),
                    UpdateFail::Checksum => ("The download was damaged", retry),
                    UpdateFail::Bad => ("The update could not be read", retry),
                    UpdateFail::Space => (
                        "Not enough space on the card",
                        "Free some space, then try again.",
                    ),
                    UpdateFail::Rejected(why) => ("BaseOS refused the update", why.as_str()),
                };
                (title.into(), detail.into())
            }
        };
        Some((title, detail))
    }

    fn check_for_update(&mut self) {
        self.hold_wifi();
        self.update_jobs.ask(UpdateJob::Check);
        self.update_busy = true;
        self.set_update(Some(UpdateState::Checking));
    }

    fn update_input(&mut self, action: Action) {
        if let (Some(UpdateState::Downloading(r, got)), Action::GbaDown(Btn::B)) =
            (&self.update, action)
        {
            let (r, got) = (r.clone(), *got);
            if self.update_jobs.cancel() {
                self.cancel_update(r, got);
            }
            return;
        }
        match (&mut self.update, action) {
            (Some(UpdateState::Available(offers, i)), Action::GbaDown(Btn::Left)) => {
                *i = i.saturating_sub(1).min(offers.len() - 1)
            }
            (Some(UpdateState::Available(offers, i)), Action::GbaDown(Btn::Right)) => {
                *i = (*i + 1).min(offers.len() - 1)
            }
            (Some(UpdateState::Available(offers, i)), Action::GbaDown(Btn::A)) => {
                let r = offers[*i].clone();
                match r.kind == Kind::BaseOs && !self.charged_to_repair() {
                    true => self.set_update(Some(UpdateState::Battery(r))),
                    false => self.install_update(r),
                }
            }
            (Some(UpdateState::Battery(r)), Action::GbaDown(Btn::A)) => {
                let r = r.clone();
                self.install_update(r);
            }
            (
                Some(
                    UpdateState::Downloading(..)
                    | UpdateState::Cancelling(..)
                    | UpdateState::Closing(_),
                ),
                _,
            ) => {}
            (_, Action::GbaDown(Btn::B)) => self.set_update(None),
            _ => {}
        }
    }

    fn install_update(&mut self, r: Release) {
        self.hold_wifi();
        self.update_jobs.ask(UpdateJob::Install(r.clone()));
        self.update_busy = true;
        let mut picker = CorePicker::open(Core::Mgba, self.now());
        picker.start(self.now());
        self.update_picker = Some(picker);
        self.update_seat_at = None;
        self.set_update(Some(UpdateState::Downloading(r, 0)));
    }

    fn cancel_update(&mut self, r: Release, got: u64) {
        let now = self.now();
        if let Some(picker) = &mut self.update_picker {
            picker.press(Press::Back, now);
        }
        self.update_busy = false;
        self.set_update(Some(UpdateState::Cancelling(r, got)));
        let led = self.led_state();
        self.set_led(led);
    }

    fn take_update_events(&mut self) {
        while let Some(event) = self.update_jobs.take() {
            if event == UpdateEvent::Cancelled {
                continue;
            }
            self.update_busy &= matches!(event, UpdateEvent::Progress(_));
            match (&mut self.update, event) {
                (Some(UpdateState::Downloading(_, got)), UpdateEvent::Progress(n)) => *got = n,
                (Some(UpdateState::Downloading(r, _)), UpdateEvent::Staged) => {
                    let r = r.clone();
                    self.update_sounded = false;
                    self.set_update(Some(UpdateState::Closing(r)));
                    let led = self.led_state();
                    self.set_led(led);
                }
                (Some(UpdateState::Downloading(..)), UpdateEvent::Failed(f)) => {
                    self.set_update(Some(UpdateState::Failed(f)));
                    let led = self.led_state();
                    self.set_led(led);
                }
                (Some(UpdateState::Checking), UpdateEvent::Available(offers)) => {
                    self.update_offers = offers.clone();
                    self.set_update(Some(UpdateState::Available(offers, 0)))
                }
                (Some(UpdateState::Checking), UpdateEvent::UpToDate) => {
                    self.set_update(Some(UpdateState::UpToDate))
                }
                (Some(UpdateState::Checking), UpdateEvent::Failed(f)) => {
                    self.set_update(Some(UpdateState::Failed(f)))
                }
                _ => {}
            }
        }
        self.step_update();
    }

    fn step_update(&mut self) {
        let now = self.now();
        let t = now.saturating_sub(self.update_at);
        let closing = matches!(self.update, Some(UpdateState::Closing(_)));
        if let Some(picker) = &mut self.update_picker {
            if !picker.closing()
                && picker.seat() == Core::Mgba
                && t >= crate::core_picker::OPEN_MS + UPDATE_SWAP_PAUSE_MS
            {
                picker.press(Press::Right, now);
            }
            if closing && !picker.closing() && picker.chip(now).seated == Some(Core::Gpsp) {
                picker.press(Press::Back, now);
                self.update_seat_at = Some(now + crate::core_picker::CLOSE_MS);
            }
        }
        match self.update {
            Some(UpdateState::Downloading(..)) => {
                let led = match (t / UPDATE_BLINK_MS) % 2 {
                    0 => self.led_state(),
                    _ => LedState::Off,
                };
                self.set_led(led);
            }
            Some(UpdateState::Cancelling(ref r, _)) => {
                if self.update_picker.as_ref().is_none_or(|p| p.finished(now)) {
                    let offers = self.update_offers.clone();
                    let i = offers.iter().position(|o| o.kind == r.kind).unwrap_or(0);
                    self.update_picker = None;
                    self.set_update(Some(UpdateState::Available(offers, i)));
                    self.update_at = now.saturating_sub(UPDATE_DROP_MS);
                    self.update_back = true;
                }
            }
            Some(UpdateState::Closing(_)) => {
                let Some(inserting) = self.update_inserting() else {
                    return;
                };
                if !self.update_sounded && inserting >= SEATED_AT - Sfx::Insert.lead() {
                    self.update_sounded = true;
                    self.sfx = Some(Sfx::Insert);
                }
                if inserting >= INSERT_S + POWER_ON_S + UPDATE_HOLD_MS as f32 / 1000.0 {
                    match self.update_chosen() {
                        Some(Kind::BaseOs) => self.begin_restart(),
                        _ => {
                            self.settle_saves();
                            self.relaunching = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn update_inserting(&self) -> Option<f32> {
        let at = self.update_seat_at?;
        let now = self.now();
        (now >= at).then(|| (now - at) as f32 / 1000.0)
    }

    fn update_openness(&self) -> f32 {
        self.update_picker
            .as_ref()
            .map_or(0.0, |p| p.openness(self.now()))
    }

    fn draw_update(&self, out: &mut Vec<Draw>) {
        if let (
            Some(scene),
            Some(
                UpdateState::Checking
                | UpdateState::Available(..)
                | UpdateState::Downloading(..)
                | UpdateState::Cancelling(..)
                | UpdateState::Closing(_),
            ),
        ) = (self.update_scene, &self.update)
        {
            return self.draw_update_scene(scene, out);
        }
        self.draw_update_prompt(out);
    }

    fn draw_update_scene(&self, scene: UpdateScene, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        let t = self.now().saturating_sub(self.update_at) as f32;
        slot_ui::draw_menu_ground(canvas, self.menu_ground(), out);
        if let (Some(UpdateState::Closing(_)), Some(inserting)) =
            (&self.update, self.update_inserting())
        {
            let screen = ((inserting - INSERT_S) / POWER_ON_S).clamp(0.0, 1.0);
            if screen > 0.0 {
                self.draw_update_logo(scene, screen, out);
            }
            let kind = self.update_chosen().unwrap_or(Kind::Slot);
            let cart = cart_for(kind);
            SlotChrome {
                cart: &cart,
                face: Some(scene.carts[kind.index()].face),
                rest: canvas.centred(CART_W as f32),
                scale: 1.0,
                seat: (inserting / SEATED_AT).clamp(0.0, 1.0),
                alert: None,
                dim: 0.0,
                screen,
                game: false,
                over: true,
                behind: [None, None],
                slot: self.slot_faces,
                canvas,
            }
            .draw(out);
            return;
        }
        slot_ui::draw_slot_back(canvas, self.slot_faces, 1.0, out);
        let rest = slot_ui::shelf_cart(canvas);
        match &self.update {
            Some(UpdateState::Available(offers, chosen)) => {
                let u = (t / UPDATE_DROP_MS as f32).min(1.0) - 1.0;
                let k = UPDATE_OVERSHOOT;
                let drop = 1.0 + (k + 1.0) * u * u * u + k * u * u;
                let phase =
                    (t - UPDATE_DROP_MS as f32).max(0.0) / UPDATE_BOB_MS * std::f32::consts::TAU;
                let rests = match offers.len() {
                    1 => vec![rest],
                    n => {
                        let scales: Vec<f32> = (0..n)
                            .map(|i| match i == *chosen {
                                true => UPDATE_PAIR_SCALE,
                                false => UPDATE_PAIR_SCALE * UPDATE_PAIR_SIDE,
                            })
                            .collect();
                        let total = scales.iter().sum::<f32>() * CART_W as f32
                            + UPDATE_PAIR_GAP * (n - 1) as f32;
                        let mut x = canvas.centred(total);
                        scales
                            .iter()
                            .map(|&scale| {
                                let at = slot_ui::shelf_cart_at(x, scale);
                                x += at.w + UPDATE_PAIR_GAP;
                                at
                            })
                            .collect()
                    }
                };
                let back = match self.update_back {
                    true => ease(((t - UPDATE_DROP_MS as f32) / UPDATE_BACK_MS).clamp(0.0, 1.0)),
                    false => 1.0,
                };
                for (i, (offer, at)) in offers.iter().zip(rests).enumerate() {
                    let at = match i == *chosen {
                        true => Placed {
                            x: rest.x + (at.x - rest.x) * back,
                            y: rest.y + (at.y - rest.y) * back,
                            w: rest.w + (at.w - rest.w) * back,
                            h: rest.h + (at.h - rest.h) * back,
                        },
                        false => at,
                    };
                    let alpha = match i == *chosen {
                        true => 1.0,
                        false => back,
                    };
                    let (bob, sway) = match i == *chosen {
                        true => (
                            (1.0 - phase.cos()) / 2.0 * UPDATE_BOB_PX,
                            phase.sin() * UPDATE_SWAY,
                        ),
                        false => (0.0, 0.0),
                    };
                    let y = at.y - (at.y + at.h) * (1.0 - drop) - bob;
                    let cart = scene.carts[offer.kind.index()];
                    self.draw_update_cart(cart, Placed { y, ..at }, sway, alpha, out);
                }
            }
            Some(
                UpdateState::Downloading(..)
                | UpdateState::Cancelling(..)
                | UpdateState::Closing(_),
            ) => self.draw_update_open(scene, self.update_openness(), out),
            _ => {}
        }
        slot_ui::draw_slot_front(canvas, self.slot_faces, 1.0, out);
        if let Some(&(tex, w, h)) = self.update_faces.first() {
            let y = match self.update {
                Some(UpdateState::Checking) => UPDATE_CHECK_Y,
                _ => UPDATE_TITLE_Y,
            };
            out.push(Draw::Tex {
                x: canvas.centred(w as f32).floor(),
                y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
        }
        let Some([cancel, install, _]) = self.update_legends else {
            return;
        };
        match self.update {
            Some(UpdateState::Checking | UpdateState::Downloading(..)) => {
                self.draw_side_legend(cancel, false, LINK_LEGEND_Y, out)
            }
            Some(UpdateState::Available(..)) => {
                self.draw_side_legend(cancel, false, LINK_LEGEND_Y, out);
                self.draw_side_legend(install, true, LINK_LEGEND_Y, out);
            }
            _ => {}
        }
    }

    fn draw_update_logo(&self, scene: UpdateScene, alpha: f32, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        let [r, g, b, _] = scene.logo_ground;
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: OUT_H as f32,
            colour: [r, g, b, alpha],
        });
        let (tex, w, h) = scene.logo;
        out.push(Draw::Tex {
            x: canvas.centred(w as f32).floor(),
            y: ((OUT_H - h) / 2) as f32,
            w: w as f32,
            h: h as f32,
            tex,
            alpha,
        });
    }

    fn draw_update_cart(
        &self,
        cart: UpdateCart,
        at: Placed,
        turn: f32,
        alpha: f32,
        out: &mut Vec<Draw>,
    ) {
        let at = grown(at, TURN_PAD as f32 * at.w / CART_W as f32);
        out.push(Draw::Turned {
            x: at.x,
            y: at.y,
            w: at.w,
            h: at.h,
            tex: cart.cart,
            alpha,
            turn,
        });
    }

    fn draw_update_open(&self, scene: UpdateScene, open: f32, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        let board = slot_ui::board_at(canvas, open);
        let cart = scene.carts[self.update_chosen().unwrap_or(Kind::Slot).index()];
        for tex in [cart.interior, scene.board] {
            out.push(Draw::Tex {
                x: board.x,
                y: board.y,
                w: board.w,
                h: board.h,
                tex,
                alpha: 1.0,
            });
        }
        if let Some(picker) = &self.update_picker {
            self.draw_board_chip(
                board,
                &self.update_sockets,
                picker.chip(self.now()),
                &self.update_chips,
                Some(scene.chip),
                lift_of(open),
                out,
            );
        }
        let lit = self.update_clean().unwrap_or(0.0) * slot_ui::FINGERS.len() as f32;
        let k = board.w / slot_ui::BOARD_W as f32;
        for (i, (x0, x1)) in slot_ui::FINGERS.iter().enumerate() {
            let clean = (lit - i as f32).clamp(0.0, 1.0);
            if clean >= 1.0 {
                continue;
            }
            let [r, g, b] = TARNISH;
            out.push(Draw::Rect {
                x: board.x + x0 * k,
                y: board.y + slot_ui::FINGER_Y * k,
                w: (x1 - x0 + 1.0) * k,
                h: slot_ui::FINGER_H * k,
                colour: [r, g, b, TARNISH_ALPHA * (1.0 - clean)],
            });
        }
        self.draw_lid_shadow(slot_ui::shelf_cart(canvas), open, out);
        let (lid, turn) = lid_at(canvas, open);
        self.draw_update_cart(cart, lid, turn, 1.0, out);
    }

    fn draw_update_prompt(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas().w,
            h: OUT_H as f32,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let lines = &self.update_faces;
        let gaps = REPAIR_GAP * lines.len().saturating_sub(1) as u32;
        let total = lines.iter().map(|l| l.2).sum::<u32>() + gaps;
        let mut y = (OUT_H.saturating_sub(total) / 2) as f32;
        for &(tex, w, h) in lines {
            out.push(Draw::Tex {
                x: self.canvas().centred(w as f32).floor(),
                y,
                w: w as f32,
                h: h as f32,
                tex,
                alpha: 1.0,
            });
            y += (h + REPAIR_GAP) as f32;
        }
        let Some([cancel, install, back]) = self.update_legends else {
            return;
        };
        match self.update {
            Some(UpdateState::Available(..)) => {
                self.draw_side_legend(cancel, false, LINK_LEGEND_Y, out);
                self.draw_side_legend(install, true, LINK_LEGEND_Y, out);
            }
            Some(UpdateState::Battery(_)) => {
                self.draw_side_legend(cancel, false, LINK_LEGEND_Y, out);
                self.draw_side_legend(install, true, LINK_LEGEND_Y, out);
            }
            Some(UpdateState::UpToDate | UpdateState::Failed(_) | UpdateState::Notice(_)) => {
                self.draw_side_legend(back, false, LINK_LEGEND_Y, out)
            }
            _ => {}
        }
    }

    pub fn set_update_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.update_faces = faces;
    }

    pub fn set_update_scene(&mut self, scene: UpdateScene) {
        self.update_scene = Some(scene);
    }

    pub fn update_chip(&self) -> Option<usize> {
        let picker = self.update_picker.as_ref()?;
        picker.chip(self.now()).seated.map(Core::index)
    }

    pub fn update_clean(&self) -> Option<f32> {
        match self.update_chip() {
            Some(1) => self.update_progress(),
            _ => self.update_progress().map(|_| 0.0),
        }
    }

    pub fn update_versions(&self) -> Option<(String, String)> {
        match self.update.as_ref()? {
            UpdateState::Available(offers, i) => offers.get(*i),
            UpdateState::Downloading(r, _)
            | UpdateState::Cancelling(r, _)
            | UpdateState::Closing(r) => Some(r),
            _ => None,
        }
        .map(|r| (format!("v{}", r.running), format!("v{}", r.version)))
    }

    pub fn update_chosen(&self) -> Option<Kind> {
        match self.update.as_ref()? {
            UpdateState::Available(offers, i) => offers.get(*i).map(|r| r.kind),
            UpdateState::Downloading(r, _)
            | UpdateState::Cancelling(r, _)
            | UpdateState::Closing(r) => Some(r.kind),
            _ => None,
        }
    }

    pub fn set_update_chips(&mut self, sockets: Vec<TexId>, chips: Vec<TexId>) {
        self.update_sockets = sockets;
        self.update_chips = chips;
    }

    pub fn update_headline(&self) -> bool {
        matches!(self.update, Some(UpdateState::Available(..)))
    }

    pub fn set_update_legends(&mut self, legends: [(TexId, u32); 3]) {
        self.update_legends = Some(legends);
    }

    fn game_menu_input(&mut self, action: Action) {
        let Some(menu) = self.game_menu else {
            return;
        };
        match menu {
            GameMenu::Pick(role) => match action {
                Action::GbaDown(Btn::Left) | Action::GbaDown(Btn::Right) => {
                    self.last_role = role.other();
                    self.game_menu = Some(GameMenu::Pick(role.other()));
                }
                Action::GbaDown(Btn::Select) => self.switch_hardware(),
                Action::GbaDown(Btn::A) => self.pick_link(role),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Working { .. } => {
                if action == Action::GbaDown(Btn::B) {
                    if let Some(starting) = &mut self.starting {
                        starting.starter.cancel();
                    }
                    if let Some(reload) = &mut self.reload {
                        reload.cancelled = true;
                    }
                }
            }
            GameMenu::Linked { opened: true, .. } => match action {
                Action::GbaDown(Btn::A) => self.end_link_from_menu(),
                Action::GbaDown(Btn::B) | Action::GameMenu => self.close_game_menu(),
                _ => {}
            },
            GameMenu::Linked { .. } => {}
            GameMenu::Failed { .. } => {
                if matches!(
                    action,
                    Action::GbaDown(Btn::A) | Action::GbaDown(Btn::B) | Action::GameMenu
                ) {
                    self.close_game_menu();
                }
            }
            GameMenu::Unplug { .. } => {}
        }
    }

    pub fn start_link(&mut self, starter: LinkStarter, client_id: u16) {
        self.start_link_from(starter, client_id, self.now());
    }

    fn start_link_from(&mut self, starter: LinkStarter, client_id: u16, since: Millis) {
        if self.art_busy() {
            return self.refuse();
        }
        self.release_radio();
        if let Some(mut old) = self.starting.replace(LinkStarting { starter, client_id }) {
            old.starter.cancel();
        }
        self.game_menu = Some(GameMenu::Working {
            role: LinkRow::from_client_id(client_id),
            step: LinkStep::Radio,
            since,
        });
    }

    fn switch_hardware(&mut self) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        if !self.link_switchable(&stem) {
            return self.refuse();
        }
        let other = self.link_hardware.other();
        self.link_hardware = other;
        self.link_choices.insert(stem, other);
    }

    fn link_switchable(&self, stem: &str) -> bool {
        self.core == Core::Gpsp
            && self
                .auto_link(stem)
                .is_some_and(|(cart, _)| link_dual(&cart.code, &cart.title))
    }

    fn pick_link(&mut self, role: LinkRow) {
        let Some(stem) = self.seated().map(str::to_string) else {
            return;
        };
        let loaded = self.link_loaded.unwrap_or("auto");
        if self.core == Core::Mgba {
            let player = role.client_id() as u8;
            if self.link_player == Some(player) {
                return self.start_link(
                    LinkStarter::spawn(role.role(), link_port()),
                    role.client_id(),
                );
            }
            if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
                return self.refuse();
            }
            self.link_player = Some(player);
            self.link_reload = Some((stem.clone(), loaded));
            self.reload = Some(Reload {
                stem,
                role,
                cancelled: false,
                from: self.link_hardware,
                from_serial: loaded,
                fallback: false,
            });
            self.game_menu = Some(GameMenu::Working {
                role,
                step: LinkStep::Radio,
                since: self.now(),
            });
            return;
        }
        let (_, serial) = self.link_mode(&stem);
        if serial == loaded {
            return self.start_link(
                LinkStarter::spawn(role.role(), link_port()),
                role.client_id(),
            );
        }
        if self.snapshot.as_ref().is_some_and(|s| !s.resume_trusted()) {
            return self.refuse();
        }
        self.link_reload = Some((stem.clone(), serial));
        self.reload = Some(Reload {
            stem,
            role,
            cancelled: false,
            from: self.link_hardware.other(),
            from_serial: loaded,
            fallback: false,
        });
        self.game_menu = Some(GameMenu::Working {
            role,
            step: LinkStep::Radio,
            since: self.now(),
        });
    }

    fn refuse_seated(&mut self) {
        self.close_game_menu();
        self.pending = None;
        let caught = self.seat();
        let cart = match &mut self.phase {
            Phase::Playing { cart } => Some(std::mem::take(cart)),
            Phase::Doze { cart } => {
                *cart = None;
                None
            }
            _ => None,
        };
        if let Some(cart) = cart {
            self.refuse_out(cart, caught);
        }
    }

    fn close_game_menu(&mut self) {
        self.game_menu = None;
        if let Some(mut starting) = self.starting.take() {
            starting.starter.cancel();
        }
        if !self.link_active() {
            self.radio.ask(RadioJob::Cool);
        }
        let uncollected =
            self.link_reload.is_some() && self.reload.as_ref().is_some_and(|r| !r.fallback);
        if uncollected {
            self.link_reload = None;
            self.reload = None;
        } else if let Some(reload) = &mut self.reload {
            reload.cancelled = true;
        }
    }

    fn working_role(&self, client_id: u16) -> (LinkRow, Millis) {
        match self.game_menu {
            Some(GameMenu::Working { role, since, .. }) => (role, since),
            _ => (LinkRow::from_client_id(client_id), self.now()),
        }
    }

    fn poll_link(&mut self) {
        let Some(mut starting) = self.starting.take() else {
            return;
        };
        match starting.starter.poll() {
            None => self.starting = Some(starting),
            Some(LinkProgress::At(step)) => {
                if let Some(GameMenu::Working { role, since, .. }) = self.game_menu {
                    self.game_menu = Some(GameMenu::Working { role, step, since });
                }
                self.starting = Some(starting);
            }
            Some(LinkProgress::Ready(link)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Linked {
                    role,
                    worked,
                    since: self.now(),
                    opened: false,
                });
                self.begin_link(starting.client_id);
                self.link_transport = Some((starting.client_id, Box::new(link)));
            }
            Some(LinkProgress::Failed(LinkFail::Cancelled)) => self.game_menu = None,
            Some(LinkProgress::Failed(fail)) => {
                let (role, worked) = self.working_role(starting.client_id);
                self.game_menu = Some(GameMenu::Failed {
                    role,
                    fail,
                    worked,
                    since: self.now(),
                });
            }
        }
    }

    fn write_core(&self, core: Core) {
        let (Some(root), Some(cart)) = (
            self.root.clone(),
            self.shelf().carts.get(self.shelf().index),
        ) else {
            return;
        };
        if let Err(e) = slot_store::write_selected_core(&root, &cart.stem, core) {
            eprintln!("slot: core: could not write selected_core.txt: {e}");
        }
    }

    fn begin_power_off(&mut self) {
        self.abandon_server();
        self.close_studio();
        self.release_radio();
        if self.powering_off {
            return;
        }
        if self.link_active() {
            self.end_link();
        }
        self.close_game_menu();
        self.save_guide_spot();
        self.remember_shelf();
        self.powering_off = true;
        self.act_at = self.now() + SHUTDOWN_SHOW_MS;
        self.set_led(LedState::Off);
    }

    pub fn on_battery(&mut self, b: Battery) {
        if b.percent > BATTERY_CRITICAL || self.powering_off {
            return;
        }
        if matches!(b.charge, Charge::Charging | Charge::Full) {
            return;
        }
        self.flush_resume();
        self.begin_power_off();
    }

    fn doze_expired(&self) -> bool {
        if self.link_active() {
            return false;
        }
        let (Phase::Doze { .. }, Some(power)) = (&self.phase, &self.power) else {
            return false;
        };
        let timeout = match self.state.lid_off_min {
            Some(min) => min as Millis * 60_000,
            None => power.timeout().as_millis() as Millis,
        };
        self.now().saturating_sub(self.dozed_at) >= timeout
    }

    pub fn settle_saves(&mut self) {
        self.state_writer.flush();
        self.shot_writer.flush();
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
    }

    fn autosave(&mut self) {
        self.autosave_at = self.now() + AUTOSAVE_MS;
        self.state_writer.flush();
        if let Some(h) = self.pending_save.take() {
            let _ = h.join();
        }
        let resume = self.now() >= self.resume_at;
        if resume {
            self.resume_at = self.now() + RESUME_MS;
        }
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let (state, sav, what) = if resume {
            let Some(state) = snapshot.state() else {
                eprintln!("slot: autosave: the core gave up no state");
                return;
            };
            let (state, sav) = trusted_write(snapshot.as_ref(), state, "autosave");
            (state, sav, "autosave")
        } else {
            match trusted_sav(snapshot.as_ref(), "autosave") {
                Some(sav) => (None, Some(sav), "in-game save"),
                None => return,
            }
        };
        let (root, platform, core, cart) =
            (root.clone(), self.platform, self.core, cart.to_owned());
        let failing = self.card_writes.clone();
        let write = move || {
            let written = persist::flush(
                &root,
                platform,
                core,
                &cart,
                state.as_deref(),
                sav.as_deref(),
            );
            if let Err(e) = &written {
                eprintln!("slot: {what}: {e}");
            }
            failing.wrote(what, &written);
        };
        match std::thread::Builder::new()
            .name("slot-autosave".into())
            .spawn(write)
        {
            Ok(h) => self.pending_save = Some(h),
            Err(e) => eprintln!("slot: autosave: no writer thread: {e}"),
        }
    }

    pub fn flush_resume(&mut self) {
        self.settle_saves();
        self.autosave_at = self.now() + AUTOSAVE_MS;
        self.resume_at = self.now() + RESUME_MS;
        let (Some(root), Some(snapshot), Some(cart)) = (&self.root, &self.snapshot, self.seated())
        else {
            return;
        };
        let Some(state) = snapshot.state() else {
            eprintln!("slot: flush: the core gave up no state");
            return;
        };
        let (state, sav) = trusted_write(snapshot.as_ref(), state, "flush");
        let written = persist::flush(
            root,
            self.platform,
            self.core,
            cart,
            state.as_deref(),
            sav.as_deref(),
        );
        if let Err(e) = &written {
            eprintln!("slot: flush: {e}");
        }
        self.card_writes.wrote("flush", &written);
    }

    fn ring(&self) -> Option<StateRing> {
        let (Some(root), Some(cart)) = (&self.root, self.seated()) else {
            return None;
        };
        Some(StateRing::new(root, self.platform, self.core, cart))
    }

    fn seated(&self) -> Option<&str> {
        match &self.phase {
            Phase::Playing { cart } | Phase::Polaroids { cart } => Some(cart),
            _ => None,
        }
    }

    fn entries(&self) -> Vec<StateEntry> {
        self.ring().and_then(|r| r.list().ok()).unwrap_or_default()
    }

    fn open_polaroids(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let entries = self.entries();
        if entries.is_empty() {
            return self.refuse();
        }
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        let mut p = Polaroids::new(entries);
        p.set_undo(self.undo_label());
        self.polaroids = Some(p);
        self.phase = Phase::Polaroids { cart };
        self.push_hint_faces();
    }

    fn close_polaroids(&mut self) {
        let Phase::Polaroids { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        let row = self.album_row();
        self.shut_guide();
        self.polaroids = None;
        self.shot_viewer = None;
        self.delete_prompt = false;
        self.phase = Phase::Playing { cart };
        if std::mem::take(&mut self.states_from_menu) {
            let hidden = self.in_game_hidden();
            self.in_game_menu = Some(match hidden.contains(&row) {
                true => row.shown(&hidden)[0],
                false => row,
            });
        }
    }

    fn shot_input(&mut self, action: Action) {
        let canvas = self.canvas();
        let Some(mode) = self.shot_viewer.as_ref().map(ShotViewer::mode) else {
            return;
        };
        let pressed = match action {
            Action::Polaroids => return self.close_polaroids(),
            Action::ShelfLeft if mode != ShotMode::Zoomed => Btn::Left,
            Action::ShelfRight if mode != ShotMode::Zoomed => Btn::Right,
            Action::GbaDown(b) => b,
            Action::GbaUp(b) => {
                if let (Some((dx, dy)), Some(v)) = (pan_step(b), &mut self.shot_viewer) {
                    v.pan_hold(dx, dy, false);
                }
                return;
            }
            _ => return,
        };
        let Some(v) = &mut self.shot_viewer else {
            return;
        };
        if mode == ShotMode::Grid && v.selecting() {
            match (pressed, pan_step(pressed)) {
                (_, Some((dx, dy))) => v.grid_move(dx, dy, canvas),
                (Btn::A, _) => v.tick_selected(),
                (Btn::B | Btn::Select, _) => v.toggle_selecting(),
                (Btn::Y, _) if v.ticked_count() > 0 => self.delete_prompt = true,
                (Btn::Y, _) => self.refuse(),
                _ => {}
            }
            return;
        }
        match (mode, pressed, pan_step(pressed)) {
            (ShotMode::Grid, Btn::Select, _) => v.toggle_selecting(),
            (ShotMode::Grid, _, Some((dx, dy))) => v.grid_move(dx, dy, canvas),
            (ShotMode::Grid, Btn::A, _) => v.open(),
            (ShotMode::Grid, Btn::B, _) => self.close_polaroids(),
            (ShotMode::Grid | ShotMode::Full, Btn::Y, _) => {
                self.delete_prompt = v.selected().is_some()
            }
            (ShotMode::Full, Btn::Left, _) => v.step(-1),
            (ShotMode::Full, Btn::Right, _) => v.step(1),
            (ShotMode::Full | ShotMode::Zoomed, Btn::A, _) => v.zoom_in(canvas),
            (ShotMode::Full, Btn::B, _) => v.to_grid(canvas),
            (ShotMode::Zoomed, Btn::B, _) => v.zoom_out(canvas),
            (ShotMode::Zoomed, _, Some((dx, dy))) => {
                v.pan_tap(dx, dy, canvas);
                v.pan_hold(dx, dy, true);
            }
            _ => {}
        }
        if self.shot_viewer.as_ref().map(ShotViewer::mode) != Some(mode) {
            self.held_dir = None;
        }
    }

    fn guide_file(&self) -> Option<PathBuf> {
        let (Some(root), Some(cart)) = (&self.root, self.seated_cart()) else {
            return None;
        };
        slot_store::find_guide(root, cart.platform, &cart.stem)
    }

    fn open_guide(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let (Some(root), Some(path)) = (self.root.clone(), self.guide_file()) else {
            self.has_guide = false;
            return self.refuse();
        };
        let lines = match slot_store::read_guide(&path) {
            Ok(lines) => lines,
            Err(e) => {
                eprintln!("slot: guide: {}: {e}", path.display());
                return self.refuse();
            }
        };
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.states_from_menu = self.in_game_menu.take().is_some();
        let spot = slot_store::read_guide_spot(&root, &path);
        let mut guide = Guide::new(lines, spot, self.canvas());
        guide.set_hint_faces(self.guide_hint_faces.clone());
        if let Some((face, title)) = self.guide_help_face {
            guide.set_help_face(face, title);
        }
        self.guide = Some((guide, path));
        self.phase = Phase::Polaroids { cart };
    }

    fn shut_guide(&mut self) {
        self.save_guide_spot();
        self.guide = None;
    }

    fn save_guide_spot(&self) {
        let (Some((guide, path)), Some(root)) = (&self.guide, &self.root) else {
            return;
        };
        if let Err(e) = slot_store::write_guide_spot(root, path, guide.spot()) {
            eprintln!("slot: guide: {}: {e}", path.display());
        }
    }

    fn guide_input(&mut self, action: Action) {
        let canvas = self.canvas();
        if action == Action::Guide {
            return self.close_polaroids();
        }
        let Some((g, _)) = &mut self.guide else {
            return;
        };
        let pressed = match action {
            Action::QuickMenu => return g.toggle_help(),
            Action::GbaDown(b) => b,
            _ => return,
        };
        if g.help() {
            if pressed == Btn::B {
                g.toggle_help();
            }
            return;
        }
        match pressed {
            Btn::Up => g.scroll(-1, canvas),
            Btn::Down => g.scroll(1, canvas),
            Btn::Left => g.pan(-1, canvas),
            Btn::Right => g.pan(1, canvas),
            Btn::L1 => g.page(-1, canvas),
            Btn::R1 => g.page(1, canvas),
            Btn::A => {
                g.zoom_by(1, canvas);
            }
            Btn::B => {
                g.zoom_by(-1, canvas);
            }
            Btn::X => g.toggle_wrap(canvas),
            Btn::Y => self.close_polaroids(),
            _ => {}
        }
    }

    pub fn guide(&self) -> Option<&Guide> {
        self.guide.as_ref().map(|(g, _)| g)
    }

    pub fn set_guide_page_face(&mut self, tex: TexId, view: slot_ui::GuideView) {
        if let Some((g, _)) = &mut self.guide {
            g.set_page_face(tex, view);
        }
    }

    pub fn set_guide_hint_faces(&mut self, faces: Vec<TexId>) {
        self.guide_hint_faces = faces;
    }

    pub fn set_guide_help_face(&mut self, face: (TexId, u32, u32), title: TexId) {
        self.guide_help_face = Some((face, title));
    }

    fn album_row(&self) -> QuickRow {
        if self.guide.is_some() {
            return QuickRow::Guide;
        }
        match self.shot_viewer {
            Some(_) => QuickRow::Screenshots,
            None => QuickRow::States,
        }
    }

    fn open_shots(&mut self) {
        if self.link_active() {
            return self.refuse();
        }
        let removing = self.shot_writer.removing();
        if !self.shot_dir().is_some_and(|s| s.any(&removing)) {
            self.has_shots = false;
            let hidden = self.in_game_hidden();
            if let Some(row) = self.in_game_menu.filter(|r| hidden.contains(r)) {
                self.in_game_menu = Some(row.shown(&hidden)[0]);
            }
            return self.refuse();
        }
        self.shot_opens += 1;
        let Phase::Playing { cart } = &mut self.phase else {
            return;
        };
        let cart = std::mem::take(cart);
        self.in_game_menu = None;
        self.shot_viewer = Some(ShotViewer::new(Vec::new()));
        self.states_from_menu = true;
        self.phase = Phase::Polaroids { cart };
        self.list_shots_later();
        self.push_hint_faces();
    }

    fn delete_shot(&mut self) {
        let canvas = self.canvas();
        let Some(v) = &mut self.shot_viewer else {
            return;
        };
        let gone = match v.selecting() {
            true => v.remove_ticked(canvas),
            false => {
                let one = v.selected().cloned();
                v.remove_selected(canvas);
                one.into_iter().collect()
            }
        };
        let empty = v.is_empty();
        for e in gone {
            self.shot_writer.remove(e.path);
        }
        self.has_shots = !empty;
        if empty {
            self.close_polaroids();
        }
    }

    pub fn shots_to_delete(&self) -> usize {
        match &self.shot_viewer {
            Some(v) if v.selecting() => v.ticked_count(),
            Some(_) => 1,
            None => 0,
        }
    }

    pub fn shot_opens(&self) -> u64 {
        self.shot_opens
    }

    fn load_selected(&mut self) {
        let state = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.state.clone());
        let refused = state.map(|state| self.load_file(&state)) == Some(false);
        if !refused {
            self.states_from_menu = false;
            self.close_polaroids();
        }
    }

    fn delete_selected(&mut self) {
        let stamp = self
            .polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.clone());
        let (Some(stamp), Some(ring)) = (stamp, self.ring()) else {
            return;
        };
        if let Err(e) = ring.remove(&stamp) {
            eprintln!("slot: delete: {e}");
            return;
        }
        if self.undo_targets(&stamp) {
            self.pending = None;
        }
        self.refresh_albums();
        let Some(p) = &mut self.polaroids else {
            return;
        };
        p.remove_selected();
        if p.is_empty() {
            self.close_polaroids();
        }
    }

    fn undo_targets(&self, stamp: &str) -> bool {
        match self.pending.as_ref() {
            Some((PendingUndo::Save { stamp: pending, .. }, _)) => pending == stamp,
            _ => false,
        }
    }

    fn load_newest(&mut self) {
        let Some(newest) = self.entries().first().map(|e| e.state.clone()) else {
            return self.refuse();
        };
        self.load_file(&newest);
    }

    fn reset_game(&mut self) {
        if !self.may_load_state() {
            return self.refuse();
        }
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let prior = snapshot.state();
        snapshot.reset();
        self.hud.toast(Toast::GameReset, self.now());
        if let Some(prior) = prior {
            self.pending = Some((PendingUndo::Reset { prior }, self.now()));
        }
    }

    fn load_file(&mut self, state: &Path) -> bool {
        if !self.may_load_state() {
            self.refuse();
            return false;
        }
        let Some(snapshot) = &self.snapshot else {
            return false;
        };
        let bytes = match std::fs::read(state) {
            Ok(bytes) => bytes,
            Err(e) => {
                eprintln!("slot: load: {e}");
                return false;
            }
        };
        let prior = snapshot.state();
        snapshot.load(bytes);
        self.hud.toast(Toast::StateLoaded, self.now());
        if let Some(prior) = prior {
            self.pending = Some((PendingUndo::Load { prior }, self.now()));
        }
        true
    }

    pub fn shot_requested(&self) -> bool {
        self.shot_wanted
    }

    pub fn shot_taken(&mut self, w: u32, h: u32, rgba: Vec<u8>) {
        self.shot_wanted = false;
        let Some(shots) = self.shot_dir() else {
            return;
        };
        let mut ms = self.wall_secs() * 1000 + i64::from(system_subsec_ms());
        if let Some(last) = self.last_shot.filter(|&last| ms <= last) {
            ms = last + 1;
        }
        self.shot_writer.write(crate::shot_writer::Shot {
            path: shots.path(&slot_store::shot_stamp(ms)),
            w,
            h,
            rgba,
        });
        self.last_shot = Some(ms);
        self.has_shots = true;
        self.hud.toast(Toast::ShotCaptured, self.now());
    }

    pub fn flush_shots(&self) {
        self.shot_writer.flush();
    }

    fn save_state(&mut self) {
        let (Some(ring), Some(snapshot)) = (self.ring(), &self.snapshot) else {
            return;
        };
        if !snapshot.resume_trusted() {
            eprintln!("slot: save: the core refused the resume it was given, not pushing a state");
            return self.refuse();
        }
        let Some(state) = snapshot.state() else {
            eprintln!("slot: save: the core gave up no state");
            return;
        };
        let thumb = snapshot.thumb().unwrap_or_default();
        let stamp = free_stamp(&stamps_of(&ring), self.wall_secs());
        let evicted = doomed(&ring);
        let pushed = ring.push(&state, &thumb, &stamp);
        self.card_writes.wrote("save state", &pushed);
        if let Err(e) = pushed {
            eprintln!("slot: save: {e}");
            return;
        }
        self.has_states = true;
        self.hud.toast(Toast::StateSaved, self.now());
        self.pending = Some((PendingUndo::Save { stamp, evicted }, self.now()));
    }

    pub fn undo_available(&self, now: Millis) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|(_, at)| now.saturating_sub(*at) <= UNDO_GRACE_MS)
    }

    pub fn undo_label(&self) -> Option<&'static str> {
        if !self.undo_available(self.now()) {
            return None;
        }
        match self.pending.as_ref()?.0 {
            PendingUndo::Save { .. } => Some("undo save"),
            PendingUndo::Load { .. } => Some("undo load"),
            PendingUndo::Reset { .. } => Some("undo reset"),
        }
    }

    pub fn set_legend_faces(&mut self, faces: Vec<TexId>) {
        self.legend_faces = faces;
        self.push_hint_faces();
    }

    pub fn set_undo_face(&mut self, face: Option<TexId>) {
        self.undo_face = face;
        self.push_hint_faces();
    }

    fn push_hint_faces(&mut self) {
        let mut faces = self.legend_faces.clone();
        faces.extend(self.undo_face);
        if let Some(v) = &mut self.shot_viewer {
            v.set_hint_faces(self.shot_hint_faces.clone());
            v.set_tick_face(self.shot_tick_face);
        }
        if let Some(p) = &mut self.polaroids {
            p.set_hint_faces(faces);
        }
    }

    pub fn shot_viewer(&self) -> Option<&ShotViewer> {
        self.shot_viewer.as_ref()
    }

    pub fn set_shot_face(&mut self, face: Option<(TexId, u32, u32)>) {
        if let Some(v) = &mut self.shot_viewer {
            v.set_face(face);
        }
    }

    pub fn set_shot_thumb(&mut self, path: &std::path::Path, face: Option<(TexId, u32, u32)>) {
        if let Some(v) = &mut self.shot_viewer {
            v.set_thumb(path, face);
        }
    }

    pub fn set_shot_count_face(&mut self, face: Option<(TexId, u32)>) {
        if let Some(v) = &mut self.shot_viewer {
            v.set_count_face(face);
        }
    }

    pub fn set_shot_tick_face(&mut self, face: (TexId, u32, u32)) {
        self.shot_tick_face = Some(face);
        self.push_hint_faces();
    }

    pub fn set_delete_shot_text(&mut self, title: (TexId, u32, u32), detail: (TexId, u32, u32)) {
        self.delete_shot_text = Some([title, detail]);
    }

    pub fn delete_shot_lines(&self) -> Option<(String, &'static str)> {
        if !self.delete_prompt || self.shot_viewer.is_none() {
            return None;
        }
        Some(match self.shots_to_delete() {
            1 => (
                "Delete this screenshot?".to_string(),
                "It cannot be brought back.",
            ),
            n => (
                format!("Delete {n} screenshots?"),
                "They cannot be brought back.",
            ),
        })
    }

    pub fn set_shot_hint_faces(&mut self, faces: Vec<TexId>) {
        self.shot_hint_faces = faces;
        self.push_hint_faces();
    }

    pub fn shot_wants_full(&self) -> Option<&std::path::Path> {
        self.shot_viewer
            .as_ref()
            .filter(|v| v.mode() != ShotMode::Grid)
            .and_then(|v| v.selected())
            .map(|e| e.path.as_path())
    }

    pub fn set_delete_shot_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.delete_shot_faces = faces;
    }

    pub fn set_icon_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_icons(faces);
    }

    pub fn set_toast_faces(&mut self, faces: Vec<TexId>) {
        self.hud.set_toasts(faces);
    }

    pub fn toast(&self) -> Option<Toast> {
        self.hud.said(self.now())
    }

    pub fn card_alarm(&self) -> bool {
        self.hud.alarmed()
    }

    pub fn alarm_toast(&self) -> Option<Toast> {
        self.hud.alarm_toast()
    }

    pub fn undo(&mut self, now: Millis) {
        if !self.undo_available(now) {
            self.pending = None;
            return;
        }
        if matches!(
            &self.pending,
            Some((PendingUndo::Load { .. } | PendingUndo::Reset { .. }, _))
        ) && !self.may_load_state()
        {
            return self.refuse();
        }
        let Some((what, _)) = self.pending.take() else {
            return;
        };
        match what {
            PendingUndo::Save { stamp, evicted } => self.undo_save(&stamp, evicted),
            PendingUndo::Load { prior } | PendingUndo::Reset { prior } => {
                if let Some(snapshot) = &self.snapshot {
                    snapshot.load(prior);
                }
                self.states_from_menu = false;
            }
        }
        self.refresh_albums();
        self.close_polaroids();
    }

    fn undo_save(&self, stamp: &str, evicted: Option<(String, Vec<u8>, Vec<u8>)>) {
        let Some(ring) = self.ring() else {
            return;
        };
        if let Err(e) = ring.remove(stamp) {
            eprintln!("slot: undo: {e}");
            return;
        }
        let Some((stamp, state, thumb)) = evicted else {
            return;
        };
        if let Err(e) = ring.push(&state, &thumb, &stamp) {
            eprintln!("slot: undo: {e}");
        }
    }

    pub fn polaroid_entries(&self) -> &[StateEntry] {
        match &self.polaroids {
            Some(p) => &p.entries,
            None => &[],
        }
    }

    pub fn set_polaroid_faces(&mut self, faces: Vec<TexId>) {
        if let Some(p) = &mut self.polaroids {
            p.set_faces(faces);
        }
    }

    pub fn polaroid_stamp(&self) -> Option<&str> {
        if let Some(v) = &self.shot_viewer {
            return v.selected().map(|e| e.stamp.as_str());
        }
        self.polaroids
            .as_ref()
            .and_then(|p| p.selected())
            .map(|e| e.stamp.as_str())
    }

    pub fn polaroid_title(&self, now: &str) -> String {
        if let Some(v) = &self.shot_viewer {
            return v.title(now, self.state.clock_24h);
        }
        self.polaroids
            .as_ref()
            .map_or_else(String::new, |p| p.title(now, self.state.clock_24h))
    }

    pub fn set_polaroid_title_face(&mut self, face: TexId) {
        if let Some(v) = &mut self.shot_viewer {
            v.set_title_face(Some(face));
        }
        if let Some(p) = &mut self.polaroids {
            p.set_title_face(Some(face));
        }
    }
}

fn system_subsec_ms() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_millis())
}

fn pan_step(b: Btn) -> Option<(i32, i32)> {
    match b {
        Btn::Left => Some((-1, 0)),
        Btn::Right => Some((1, 0)),
        Btn::Up => Some((0, -1)),
        Btn::Down => Some((0, 1)),
        _ => None,
    }
}

fn trusted_write(
    snapshot: &dyn Snapshot,
    state: Vec<u8>,
    verb: &str,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let state = if snapshot.resume_trusted() {
        Some(state)
    } else {
        eprintln!(
            "slot: {verb}: the core refused the resume it was given, not overwriting the saved one"
        );
        None
    };
    (state, trusted_sav(snapshot, verb))
}

fn trusted_sav(snapshot: &dyn Snapshot, verb: &str) -> Option<Vec<u8>> {
    let sav = snapshot.save_ram();
    if snapshot.save_ram_trusted() {
        return sav;
    }
    if sav.is_some() {
        eprintln!(
            "slot: {verb}: the core refused the save ram it was given, not overwriting the saved one"
        );
    }
    None
}

fn up(level: u8, step: u8, max: u8) -> u8 {
    level.saturating_add(step).min(max)
}

fn blending_value(blending: Blending) -> QuickValue {
    match blending {
        Blending::Off => QuickValue::Off,
        Blending::Mix => QuickValue::Mix,
        Blending::Ghosting => QuickValue::Ghosting,
    }
}

fn scale_value(scale: Scale) -> QuickValue {
    match scale {
        Scale::SpFrame => QuickValue::SpFrame,
        Scale::Integer => QuickValue::Integer,
        Scale::Fit => QuickValue::Fit,
        Scale::FullScreen => QuickValue::FullScreen,
    }
}

fn stretch_value(stretch: Stretch) -> QuickValue {
    match stretch {
        Stretch::Classic => QuickValue::Classic,
        Stretch::FullScreen => QuickValue::FullScreen,
    }
}

fn shader_value(shader: Shader) -> QuickValue {
    match shader {
        Shader::Off => QuickValue::Off,
        Shader::Lcd3x => QuickValue::Lcd3x,
        Shader::Grid => QuickValue::Grid,
        Shader::Dot => QuickValue::Dot,
        Shader::Simpletex => QuickValue::Simpletex,
        Shader::Shimmerless => QuickValue::Shimmerless,
    }
}

fn tint_alpha(tint: WallpaperTint) -> f32 {
    match tint {
        WallpaperTint::Off => 0.0,
        WallpaperTint::Low => 0.25,
        WallpaperTint::Medium => 0.45,
        WallpaperTint::High => 0.65,
    }
}

fn ff_next(from: u8, right: bool) -> u8 {
    let n = FF_SPEEDS.len();
    let at = FF_SPEEDS.iter().position(|&v| v == from).unwrap_or(0);
    FF_SPEEDS[if right {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    }]
}

fn clock_screen(utc: i64, state: &SlotState, networked: bool, from_menu: bool) -> Phase {
    Phase::SetClock {
        picker: ClockPicker::new(utc, state.zone, state.clock_24h)
            .with_network_time(networked.then_some(state.network_time)),
        seed: utc - utc.rem_euclid(60),
        from_menu,
    }
}

fn system_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn doomed(ring: &StateRing) -> Option<(String, Vec<u8>, Vec<u8>)> {
    let entries = ring.list().ok()?;
    let oldest = entries.get(RING_MAX - 1)?;
    let (state, thumb) = ring.read(&oldest.stamp).ok()?;
    Some((oldest.stamp.clone(), state, thumb))
}

fn stamps_of(ring: &StateRing) -> Vec<String> {
    ring.list()
        .map(|l| l.into_iter().map(|e| e.stamp).collect())
        .unwrap_or_default()
}

fn free_stamp(taken: &[String], now: i64) -> String {
    let newest = taken.iter().filter_map(|s| parse_stamp(s)).max();
    let mut secs = newest.map_or(now, |n| now.max(n + 1));
    let mut stamp = format_stamp(secs);
    while taken.contains(&stamp) {
        secs += 1;
        stamp = format_stamp(secs);
    }
    stamp
}

fn bezel_slot(platform: Platform) -> usize {
    match platform {
        Platform::Gba | Platform::Gbav => 0,
        Platform::Gb => 1,
        Platform::Gbc => 2,
    }
}
