use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use slot_gfx::{Canvas, GameLayout, Sizing, OUT_H, OUT_W, WHOLE_TEXTURE};
use slot_input::{Action, Btn, RawEvent, MENU_DOUBLE_TAP_MS, MUTE_CHORD_MS};
use slot_power::{Battery, Charge, LedState, LidPolicy, Power, RepairResult};
use slot_retro::LinkChannel;
use slot_store::{
    format_stamp, parse_stamp, read_slot_state, scan, write_slot_state, BatteryDisplay, BezelPick, Blending, Cart,
    Collection, Core, GbColors, KidMode, Platform, Recent, Scale, Shader, ShelfMode, ShotDir, ShotEntry,
    SlotState, StateEntry,
    StateRing, Stretch, WallpaperPick, WallpaperTint, BLUE_LIGHT_MAX, BRIGHTNESS_MAX, CONTRAST_MAX,
    DIM_MAX, FF_SPEEDS, RING_MAX, SATURATION_MAX, SUNLIGHT_MAX, VOLUME_MAX,
};
use slot_ui::{
    board_from_shaped, draw_empty_slot, draw_footer, draw_ground, draw_sticker, ease, footer_ink,
    grown, lid_at, lid_from_shaped, lift_of, repeat_gap, seats, shelf_cart_shaped, BezelArt,
    ClockFaces, ClockPicker, ClockRow, ClockScreen, Draw, FfState, GbShell, Guide, Hud, HudKind,
    Icon, LinkBadge, Millis, Placed, Polaroids, PowerChoice, QuickMenu, QuickMenuFaces, QuickRow,
    QuickValue, Refusal, Seats, Shelf, ShotMode, ShotViewer, SlotChrome, TexId, Toast, CART_W,
    GBA_SEATS, HINT_EDGE, HOP_LIFT, MOUTH_H, REPEAT_DELAY_MS, SHADOW_H, SHADOW_W, TURN_PAD,
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
use crate::unwrap_scene::{unwrap_draws, UnwrapScene, UnwrapTextures};
use crate::server::{ServerNet, ServerPaper, ServerSession};
use crate::art::jobs::{no_art_jobs, ArtEvent, ArtJob, ArtJobs};
use crate::romm::{no_romm_jobs, PairFail, RommEvent, RommJob, RommJobs};
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
const DS_INSERT_FLY_S: f32 = 1.8;
const DS_PART_S: f32 = 0.3;
const DS_EJECT_FLY_S: f32 = 1.8;
const DS_CLICK_GAP_S: f32 = 0.53;

pub const POWER_ON_S: f32 = 0.22;

const POWER_OFF_S: f32 = 0.16;

const FOOTER_SLIDE_S: f32 = POWER_ON_S;
const GIFT_REACH: i32 = 4;

const BLANK_WARM_S: f32 = 1.5;
const BLANK_DIM: f32 = 0.1;

const VOLUME_STEP: u8 = 5;

const AUTOSAVE_MS: Millis = 60_000;
const PEEK_IN_MS: Millis = 120;
const PEEK_HOLD_MS: Millis = 680;
const PEEK_OUT_MS: Millis = 200;
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
const EDIT_DIM: [f32; 4] = [0.043, 0.043, 0.059, 0.38];
const PART_PUSH: f32 = 300.0;
const SIDE_SCALE: f32 = 0.78;
const SIDE_PITCH: f32 = 368.0;
const GLIDE_MS: Millis = 200;
const ENTER_RISE: f32 = 24.0;
const HEADING_Y: f32 = 12.0;
const STACK_BASE: f32 = 344.0;
const STACK_PICKED: f32 = 0.5;
const STACK_SIDE: f32 = 0.38;
const STACK_SIDE_ALPHA: f32 = 0.75;
const STACK_SPREAD: f32 = 16.0;
const STACK_RAISE: f32 = 6.0;
const STACK_TILT: f32 = 6.0;
const NEW_W: f32 = 155.0;
const NEW_H: f32 = 170.0;
const PAIR_GAP: f32 = 120.0;
const TRIO_GAP: f32 = 230.0;
const ROW_GAP: f32 = 262.0;
const ROW_SIDE: f32 = 0.78;
const ROW_SIDE_ALPHA: f32 = 0.8;
const ROW_HELD: f32 = 0.6;
const ROW_BASE: f32 = 350.0;
const ROW_LIFT: f32 = -16.0;
const ROW_NAME_Y: f32 = 364.0;
const PICKED_NAME_Y: f32 = 362.0;
const SIDE_NAME_Y: f32 = 356.0;
const ADD_FLOAT: f32 = 0.34;
const ADD_FLOAT_TOP: f32 = 46.0;
const REMOVE_FLOAT: f32 = 0.42;
const REMOVE_FLOAT_TOP: f32 = 56.0;
const REMOVE_BASE: f32 = 398.0;
const LAST_SCALE: f32 = 0.82;
const LAST_DROP: f32 = 38.0;
const PICK_DIM_ALPHA: f32 = 0.4;
const BUNDLE_MAX: usize = 4;
const BUNDLE_SCALE: f32 = 0.29;
const BUNDLE_SPREAD: f32 = 30.0;
const BUNDLE_BADGE_DX: f32 = 56.0;
const BUNDLE_BADGE_LIFT: f32 = 6.0;

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

const ROMM_HOST_SHOWN: usize = 28;
const ROMM_POLL_MS: Millis = 30_000;

pub const LINKED_HOLD_MS: Millis = 1000;

pub const UNPLUG_HOLD_MS: Millis = 420;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BoxArtFaces {
    pub boxed: Option<(TexId, u32, u32)>,
    pub logo: Option<(TexId, u32, u32)>,
    pub stem: String,
    pub platform: Platform,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomSource {
    DsScreen,
    Compositor,
    Black,
}

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
    RommPair,
    Unwrap {
        stem: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum PairScreen {
    Starting,
    Show { url: String, code: String },
    Failed(PairFail),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PairView {
    pub url: Option<String>,
    pub lines: Vec<(slot_ui::PairSlot, String)>,
}

pub struct PairFaces {
    pub qr: Option<slot_ui::NetFace>,
    pub lines: Vec<(slot_ui::PairSlot, slot_ui::NetFace)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardFor {
    Password { ssid: String, hidden: bool },
    CollectionName,
    NetworkName,
    RommServer,
    RommPort,
}

pub struct NetLegends {
    pub join: [(TexId, u32); 2],
    pub scan: [(TexId, u32); 2],
    pub cancel: [(TexId, u32); 2],
    pub connect: [(TexId, u32); 2],
    pub retry: [(TexId, u32); 2],
    pub disconnect: [(TexId, u32); 2],
    pub forget: [(TexId, u32); 2],
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Rack {
    Recents,
    Favorites,
    Collection(usize),
    Of(Platform),
    AllGames,
}

impl Rack {
    fn platform(self) -> Option<Platform> {
        match self {
            Rack::Of(p) => Some(p),
            Rack::Recents | Rack::Favorites | Rack::Collection(_) | Rack::AllGames => None,
        }
    }

    fn key(self) -> String {
        match self {
            Rack::Recents => "recents".into(),
            Rack::Favorites => "favorites".into(),
            Rack::Collection(i) => format!("collection/{i}"),
            Rack::Of(p) => p.dir_name().to_ascii_lowercase(),
            Rack::AllGames => slot_store::KID_SHELF_ALL.into(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Rack::Recents => "Recents",
            Rack::Favorites => "Favorites",
            Rack::Collection(_) => "",
            Rack::Of(p) => p.name(),
            Rack::AllGames => "All Games",
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
    CheckFailed,
    Notice(NetNotice),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NetNotice {
    Password,
    ServerFailed,
    NoNetwork,
    RommNoServer,
    RommPort,
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
    collections: Vec<Collection>,
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
    edit: Option<crate::editing::Edit>,
    picking_faces: Option<[[(TexId, u32); 8]; 2]>,
    heading_face: Option<(TexId, u32, u32)>,
    glide: Option<(f32, Millis)>,
    leaving: Option<(Leaving, Millis)>,
    press: Option<(Btn, Millis)>,
    name_faces: Vec<(String, [(TexId, u32, u32); 2])>,
    add_faces: Option<([(TexId, u32); 2], usize)>,
    count_badge: Option<((TexId, f32, f32), usize)>,
    core_board_face: Option<TexId>,
    core_interior_face: Option<TexId>,
    core_lid_face: Option<TexId>,
    core_faces_for: Option<(Platform, String)>,
    core_socket_faces: Vec<TexId>,
    core_chip_faces: Vec<TexId>,
    core_blank_chip_face: Option<TexId>,
    core_gb_socket_faces: Vec<TexId>,
    core_gb_chip_faces: Vec<TexId>,
    core_gb_blank_chip_face: Option<TexId>,
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
    quick_romm_faces: [Option<[(TexId, u32, u32); 2]>; 4],
    quick_kid_shelf_faces: Option<[(TexId, u32, u32); 2]>,
    kid_faces: Vec<(TexId, u32, u32)>,
    kid_prompt: Option<KidMode>,
    kid_unlocked: bool,
    seated_platform: Option<Platform>,
    kid_icon_faces: Option<[(TexId, u32, u32); 2]>,
    pub romm: crate::romm::RommConf,
    quick_palette_faces: Option<[(TexId, u32, u32); 2]>,
    quick_bezel_faces: [Option<[(TexId, u32, u32); 2]>; 3],
    quick_overlay_faces: [Option<[(TexId, u32, u32); 2]>; 4],
    overlay_cache: std::sync::Mutex<Vec<OverlayScan>>,
    game_bezel: Option<TexId>,
    bezel_column: Vec<u8>,
    in_game_menu: Option<QuickRow>,
    footer: f32,
    footer_row: Option<QuickRow>,
    states_from_menu: bool,
    menu_tap: Option<(Millis, Option<QuickRow>)>,
    tint_peek: Option<Millis>,
    sticker_face: Option<(TexId, u32, u32)>,
    ground: Option<TexId>,
    ground_w: u32,
    blur: Option<TexId>,
    wallpaper: Option<TexId>,
    random_wallpaper: Option<Option<String>>,
    shelf_wallpaper: Option<String>,
    collection_wallpaper: Option<String>,
    collection_pngs: Vec<String>,
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
    eject_write: Option<std::thread::JoinHandle<bool>>,
    two_panels: bool,
    ds_slot_faces: Option<(TexId, TexId)>,
    box_art: Option<BoxArtFaces>,
    ds_landed: Option<f64>,
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
    romm_jobs: Box<dyn RommJobs>,
    romm_poll_at: Millis,
    romm_open: bool,
    romm_carts: Option<Vec<Cart>>,
    romm_gifts: Vec<(Platform, String)>,
    gifts: crate::romm::Gifts,
    unwrap_scene: Option<UnwrapScene>,
    unwrap_tex: Option<UnwrapTextures>,
    romm_note: Option<String>,
    romm_online: bool,
    pair_screen: PairScreen,
    pair_faces: Option<PairFaces>,
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
    collection_said: Option<(u64, String)>,
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

fn retain_headers(carts: &[Cart]) {
    let roms: Vec<std::path::PathBuf> = carts.iter().map(|c| c.rom.clone()).collect();
    slot_store::header_index::retain(&roms);
}

fn shelves_of(carts: Vec<Cart>, collections: usize) -> Vec<(Rack, Shelf)> {
    let mut rows: Vec<(Rack, Vec<Cart>)> = [Rack::Recents, Rack::Favorites]
        .into_iter()
        .chain((0..collections).map(Rack::Collection))
        .chain(Platform::ALL.iter().map(|p| Rack::Of(*p)))
        .chain(std::iter::once(Rack::AllGames))
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
    let rack = match rack {
        Rack::Collection(_) => Rack::Favorites,
        rack => rack,
    };
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

const PLACEHOLDER_BEZEL: [f32; 4] = [50.0 / 255.0, 205.0 / 255.0, 50.0 / 255.0, 1.0];

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

#[derive(Clone)]
enum Leaving {
    Added {
        snap: crate::editing::Edit,
        game: Recent,
        stack_x: f32,
        multi: bool,
        before: (usize, Vec<Recent>),
    },
    Backed {
        snap: crate::editing::Edit,
        game: Recent,
        float_x: f32,
    },
    Removed {
        game: Recent,
        around: Vec<Recent>,
    },
    Kept {
        game: Recent,
        around: Vec<Recent>,
        last: bool,
    },
    LastRemoved {
        game: Recent,
    },
}

impl App {
    pub fn new(carts: Vec<Cart>) -> Self {
        let shelves = shelves_of(carts, 0);
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
            romm_jobs: no_romm_jobs(),
            romm_poll_at: 0,
            romm_open: false,
            romm_carts: None,
            romm_gifts: Vec::new(),
            gifts: crate::romm::Gifts::default(),
            unwrap_scene: None,
            unwrap_tex: None,
            romm_note: None,
            romm_online: false,
            pair_screen: PairScreen::Starting,
            pair_faces: None,
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
            collection_said: None,
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
            collections: Vec::new(),
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
            edit: None,
            picking_faces: None,
            heading_face: None,
            glide: None,
            leaving: None,
            press: None,
            name_faces: Vec::new(),
            add_faces: None,
            count_badge: None,
            core_board_face: None,
            core_interior_face: None,
            core_lid_face: None,
            core_faces_for: None,
            core_socket_faces: Vec::new(),
            core_chip_faces: Vec::new(),
            core_blank_chip_face: None,
            core_gb_socket_faces: Vec::new(),
            core_gb_chip_faces: Vec::new(),
            core_gb_blank_chip_face: None,
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
            quick_romm_faces: [None; 4],
            quick_kid_shelf_faces: None,
            kid_faces: Vec::new(),
            kid_prompt: None,
            kid_unlocked: false,
            seated_platform: None,
            kid_icon_faces: None,
            romm: crate::romm::RommConf::default(),
            quick_palette_faces: None,
            quick_bezel_faces: [None, None, None],
            quick_overlay_faces: [None; 4],
            overlay_cache: std::sync::Mutex::new(Vec::new()),
            game_bezel: None,
            bezel_column: Vec::new(),
            in_game_menu: None,
            footer: 0.0,
            footer_row: None,
            states_from_menu: false,
            menu_tap: None,
            tint_peek: None,
            sticker_face: None,
            ground: None,
            ground_w: OUT_W,
            blur: None,
            wallpaper: None,
            random_wallpaper: None,
            shelf_wallpaper: None,
            collection_wallpaper: None,
            collection_pngs: Vec::new(),
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
            eject_write: None,
            two_panels: false,
            ds_slot_faces: None,
            box_art: None,
            ds_landed: None,
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
        crate::server::sweep_names(root);
        slot_store::header_index::load(root);
        crate::boot_time::mark("root");
        let carts = scan(root).unwrap_or_default();
        retain_headers(&carts);
        crate::boot_time::mark("scan");
        let mut app = App::new(carts);
        app.root = Some(root.to_path_buf());
        app.loading = loading;
        app.state = read_slot_state(root);
        tell_libc_the_zone(app.state.zone);
        app.romm = crate::romm::RommConf::load(root);
        app.gifts = crate::romm::Gifts::load(root);
        app.carousel = read_carousel(root);
        app.shortcuts = read_shortcuts(root);
        app.controls = read_controls(root);
        app.install_collections(slot_store::read_collections(root));
        app.restock_recents();
        app.restock_favorites();
        app.restock_all_games();
        app.restock_collections();
        if let Some((at, i)) = app.remembered_seat() {
            app.shelf_at = at;
            app.shelves[at].1.select(i);
        } else if !app.shelves[RECENTS_AT].1.carts.is_empty() {
            app.shelf_at = RECENTS_AT;
        }
        let hidden = matches!(
            (app.state.shelf_mode, app.shelves[app.shelf_at].0),
            (ShelfMode::Collections, Rack::Of(_))
        );
        if hidden {
            if let Some(to) = (0..app.shelves.len()).find_map(|at| app.row_target(at)) {
                app.shelf_at = to;
            }
        }
        if app.kid_one_shelf() {
            app.shelf_at = app.kid_shelf_at();
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
                self.insert(!self.state.resume_power_on);
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
        let core = slot_store::core_for_platform(root, stem, platform, self.state.gb_core);
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
        self.land_on(to);
        self.note_shelf();
    }

    fn land_on(&mut self, to: usize) {
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
        let from = self.row_seat(self.shelf_at) as i32;
        (1..n)
            .map(|step| (from + by * step).rem_euclid(n) as usize)
            .find_map(|at| self.row_target(at))
    }

    fn row_seat(&self, at: usize) -> usize {
        match (self.state.shelf_mode, self.shelves[at].0) {
            (ShelfMode::All, Rack::Collection(_)) => FAVORITES_AT,
            (_, Rack::Of(Platform::Gbav)) => self.rack_at(Rack::Of(Platform::Gba)).unwrap_or(at),
            _ => at,
        }
    }

    fn row_target(&self, at: usize) -> Option<usize> {
        let (rack, shelf) = &self.shelves[at];
        if self.kid_one_shelf() {
            return (at == self.kid_shelf_at()).then_some(at);
        }
        match (self.state.shelf_mode, rack) {
            (_, Rack::AllGames) => None,
            (ShelfMode::All, Rack::Collection(_)) => None,
            (ShelfMode::All, Rack::Favorites) => self.stack_landing(),
            (ShelfMode::Collections, Rack::Of(_)) if !self.shows_systems() => None,
            (_, Rack::Of(Platform::Gbav)) => None,
            (_, Rack::Of(Platform::Gba)) => self.video_landing(),
            _ => (!shelf.carts.is_empty()).then_some(at),
        }
    }

    fn rack_at(&self, rack: Rack) -> Option<usize> {
        self.shelves.iter().position(|(r, _)| *r == rack)
    }

    fn video_stack(&self) -> Vec<usize> {
        [Platform::Gba, Platform::Gbav]
            .into_iter()
            .filter_map(|p| self.rack_at(Rack::Of(p)))
            .filter(|&at| !self.shelves[at].1.carts.is_empty())
            .collect()
    }

    fn video_landing(&self) -> Option<usize> {
        let stack = self.video_stack();
        let want = Rack::Of(match self.state.gba_video {
            true => Platform::Gbav,
            false => Platform::Gba,
        });
        stack
            .iter()
            .copied()
            .find(|&at| self.shelves[at].0 == want)
            .or(stack.first().copied())
    }

    fn shows_systems(&self) -> bool {
        matches!(
            self.edit,
            Some(crate::editing::Edit::Selecting { .. })
                | Some(crate::editing::Edit::Choosing { multi: true, .. })
        )
    }

    fn rack_key(&self, rack: Rack) -> String {
        match rack {
            Rack::Collection(i) => format!("collection/{}", self.collections[i].name),
            rack => rack.key(),
        }
    }

    fn rack_name(&self, rack: Rack) -> &str {
        match rack {
            Rack::Collection(i) => &self.collections[i].name,
            rack => rack.name(),
        }
    }

    fn stack(&self) -> Vec<usize> {
        self.shelves
            .iter()
            .enumerate()
            .filter(|(_, (r, s))| {
                matches!(r, Rack::Favorites | Rack::Collection(_)) && !s.carts.is_empty()
            })
            .map(|(at, _)| at)
            .collect()
    }

    fn stack_landing(&self) -> Option<usize> {
        let stack = self.stack();
        let want = self.state.collection.as_deref();
        stack
            .iter()
            .copied()
            .find(|&at| match self.shelves[at].0 {
                Rack::Collection(i) => want == Some(self.collections[i].name.as_str()),
                _ => want.is_none(),
            })
            .or(stack.first().copied())
    }

    fn flip_stack(&mut self, by: i32) {
        let stack = match self.shelves[self.shelf_at].0 {
            Rack::Of(Platform::Gba | Platform::Gbav) => self.video_stack(),
            _ if self.state.shelf_mode != ShelfMode::All => return,
            _ => self.stack(),
        };
        let Some(pos) = stack.iter().position(|&at| at == self.shelf_at) else {
            return;
        };
        if stack.len() < 2 {
            return;
        }
        let to = stack[(pos as i32 + by).rem_euclid(stack.len() as i32) as usize];
        self.land_on(to);
        self.note_shelf();
    }

    fn fallback_shelf(&self) -> usize {
        let on_stack = matches!(
            self.shelves[self.shelf_at].0,
            Rack::Favorites | Rack::Collection(_)
        );
        if on_stack && self.state.shelf_mode == ShelfMode::All {
            if let Some(at) = self.stack_landing() {
                return at;
            }
        }
        self.next_shelf(1).unwrap_or(0)
    }

    fn set_shelf_mode(&mut self, mode: ShelfMode) {
        self.state.shelf_mode = mode;
        let here = self.shelf_at;
        let stays = match (mode, self.shelves[here].0) {
            (ShelfMode::Collections, Rack::Of(_)) => false,
            _ => !self.shelves[here].1.carts.is_empty(),
        };
        if !stays {
            if let Some(to) = (0..self.shelves.len()).find_map(|at| self.row_target(at)) {
                self.land_on(to);
            }
        }
        self.remember_shelf();
        self.persist();
    }

    fn install_collections(&mut self, collections: Vec<Collection>) {
        self.collection_pngs = self
            .root
            .as_deref()
            .map(crate::wallpaper::collection_list)
            .unwrap_or_default();
        let keep = self.rack_key(self.shelves[self.shelf_at].0);
        let mut kept = Vec::new();
        let mut old: Vec<(String, Shelf)> = Vec::new();
        for (rack, shelf) in std::mem::take(&mut self.shelves) {
            match rack {
                Rack::Collection(i) => old.push((self.collections[i].name.clone(), shelf)),
                _ => kept.push((rack, shelf)),
            }
        }
        let tail = kept.split_off(FAVORITES_AT + 1);
        let fresh: Vec<(Rack, Shelf)> = collections
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let shelf = match old.iter().position(|(n, _)| *n == c.name) {
                    Some(at) => old.remove(at).1,
                    None => kept[FAVORITES_AT].1.empty_like(),
                };
                (Rack::Collection(i), shelf)
            })
            .collect();
        for (_, mut shelf) in old {
            self.orphaned.extend(shelf.restock(Vec::new()));
        }
        kept.extend(fresh);
        kept.extend(tail);
        self.shelves = kept;
        self.collections = collections;
        self.shelf_at = (0..self.shelves.len())
            .find(|&at| self.rack_key(self.shelves[at].0) == keep)
            .unwrap_or(0);
    }

    fn restock_collections(&mut self) {
        for i in 0..self.collections.len() {
            let carts: Vec<Cart> = self.collections[i]
                .games
                .iter()
                .filter_map(|g| {
                    self.carts()
                        .find(|c| c.platform == g.platform && c.stem == g.stem)
                })
                .cloned()
                .collect();
            let at = self
                .shelves
                .iter()
                .position(|(r, _)| *r == Rack::Collection(i))
                .expect("a rack per collection");
            self.restock(at, carts);
        }
        self.mark_hearts();
    }

    fn on_collection(&self) -> Option<usize> {
        match self.shelves[self.shelf_at].0 {
            Rack::Collection(i) => Some(i),
            _ => None,
        }
    }

    fn collection_at(&self, i: usize) -> usize {
        self.shelves
            .iter()
            .position(|(r, _)| *r == Rack::Collection(i))
            .expect("a rack per collection")
    }

    fn save_collection(&self, i: usize) {
        let Some(root) = &self.root else {
            return;
        };
        let written = slot_store::write_collection(root, &self.collections[i]);
        if let Err(e) = &written {
            eprintln!("slot: collection: {e}");
        }
        self.card_writes.wrote("collection", &written);
    }

    fn delete_collection(&mut self, i: usize) {
        let mut collections = self.collections.clone();
        let gone = collections.remove(i);
        self.install_collections(collections);
        self.restock_collections();
        self.save_order();
        let Some(root) = &self.root else {
            return;
        };
        let deleted = slot_store::delete_collection(root, &gone.name);
        if let Err(e) = &deleted {
            eprintln!("slot: collection: {e}");
        }
        self.card_writes.wrote("collection", &deleted);
    }

    fn save_order(&self) {
        let Some(root) = &self.root else {
            return;
        };
        let names: Vec<String> = self.collection_names();
        let written = slot_store::write_order(root, &names);
        if let Err(e) = &written {
            eprintln!("slot: collection order: {e}");
        }
        self.card_writes.wrote("collection order", &written);
    }

    fn say_collection(&mut self, text: String) {
        let generation = self.collection_said.as_ref().map_or(0, |(g, _)| *g) + 1;
        let now = self.now();
        self.hud.toast_saying(Toast::Collection, &text, now);
        self.collection_said = Some((generation, text));
    }

    pub fn collection_said(&self) -> Option<(u64, &str)> {
        self.collection_said
            .as_ref()
            .map(|(g, text)| (*g, text.as_str()))
    }

    fn selected_game(&self) -> Option<Recent> {
        self.shelf().carts.get(self.shelf().index).map(|c| Recent {
            platform: c.platform,
            stem: c.stem.clone(),
        })
    }

    fn remove_from_collection(&mut self) {
        let Some(crate::editing::Edit::Removing {
            game, collection: i, ..
        }) = self.edit.take()
        else {
            return;
        };
        let around = crate::editing::remove_stack(&self.shelf_games(self.shelf_at), self.shelf().index);
        if !self.collections[i].remove(&game) {
            return;
        }
        let leaving = match around.is_empty() {
            true => Leaving::LastRemoved { game: game.clone() },
            false => Leaving::Removed {
                game: game.clone(),
                around,
            },
        };
        self.leaving = Some((leaving, self.now()));
        self.save_collection(i);
        let index = self.shelf().index;
        let at = self.shelf_at;
        self.restock_collections();
        let left = self.shelves[at].1.carts.len();
        if left > 0 {
            let shelf = &mut self.shelves[at].1;
            shelf.select(if index < left { index } else { 0 });
        } else {
            let to = self.after_emptied(at);
            self.land_on(to);
        }
        let name = self.collections[i].name.clone();
        if self.collections[i].games.is_empty() {
            self.delete_collection(i);
        }
        self.say_collection(format!("Removed from {name}"));
        self.note_shelf();
    }

    fn after_emptied(&self, at: usize) -> usize {
        if self.state.shelf_mode == ShelfMode::All {
            let stack = self.stack();
            if let Some(&to) = stack
                .iter()
                .find(|&&s| s > at)
                .or_else(|| stack.first())
            {
                return to;
            }
        }
        let n = self.shelves.len();
        (1..=n)
            .map(|k| (at + k) % n)
            .find_map(|s| self.row_target(s))
            .unwrap_or(0)
    }

    pub fn edit(&self) -> Option<&crate::editing::Edit> {
        self.edit.as_ref()
    }

    fn choosing(&self) -> Option<(&[Recent], &[crate::editing::Choice], usize)> {
        match self.edit.as_ref()? {
            crate::editing::Edit::Choosing {
                games, choices, at, ..
            } => Some((games, choices, *at)),
            _ => None,
        }
    }

    fn picked(&self) -> Option<crate::editing::Choice> {
        let (_, choices, at) = self.choosing()?;
        choices.get(at).copied()
    }

    fn choice_name(&self, c: crate::editing::Choice) -> &str {
        match c {
            crate::editing::Choice::Collection(i) => &self.collections[i].name,
            crate::editing::Choice::New => "New Collection",
        }
    }

    pub fn picked_choice(&self) -> Option<&str> {
        self.picked().map(|c| self.choice_name(c))
    }

    pub fn choice_names(&self) -> Vec<String> {
        self.choosing()
            .map(|(_, choices, _)| {
                choices
                    .iter()
                    .map(|c| self.choice_name(*c).to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn here(&self) -> crate::editing::Seat {
        crate::editing::Seat {
            shelf: self.shelf_at,
            index: self.shelf().index,
        }
    }

    fn sit(&mut self, seat: crate::editing::Seat) {
        if self.shelf_at != seat.shelf {
            self.land_on(seat.shelf);
        }
        let shelf = self.shelf_mut();
        if seat.index < shelf.carts.len() {
            shelf.select(seat.index);
        }
    }

    fn edit_input(&mut self, action: Action, now: Millis) {
        use crate::editing::Edit;
        match self.edit {
            Some(Edit::Selecting { .. }) => self.selecting_input(action, now),
            Some(Edit::Choosing { .. }) => self.choosing_input(action),
            Some(Edit::MovingGame { .. } | Edit::MovingCollection { .. }) => {
                self.moving_input(action, now)
            }
            Some(Edit::Removing { .. }) => match action {
                Action::GbaDown(Btn::A) => self.remove_from_collection(),
                Action::GbaDown(Btn::B) => {
                    if let Some(crate::editing::Edit::Removing { game, last, .. }) = self.edit.take() {
                        let around = crate::editing::remove_stack(&self.shelf_games(self.shelf_at), self.shelf().index);
                        self.leaving = Some((Leaving::Kept { game, around, last }, now));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn open_moving(&mut self, now: Millis) {
        let Some(collection) = self.on_collection() else {
            return;
        };
        self.edit = Some(crate::editing::Edit::MovingGame {
            collection,
            before: self.collections[collection].games.clone(),
            from: self.shelf().index,
            hold: None,
            done: 0,
            since: now,
        });
    }

    fn moving_input(&mut self, action: Action, now: Millis) {
        use crate::editing::{Edit, Hold};
        let dir = match action {
            Action::GbaDown(Btn::Left) | Action::GbaUp(Btn::Left) => -1,
            Action::GbaDown(Btn::Right) | Action::GbaUp(Btn::Right) => 1,
            Action::GbaDown(Btn::A) => return self.drop_moved(),
            Action::GbaDown(Btn::B) => return self.cancel_move(),
            Action::ForgetRecentHold => return self.lift_collection(now),
            _ => return,
        };
        let pressed = matches!(action, Action::GbaDown(_));
        let room = match &self.edit {
            Some(Edit::MovingCollection { row, .. }) => row.len(),
            _ => self.shelf().carts.len(),
        };
        if pressed && room < 2 {
            return;
        }
        self.tick_move(now);
        let (Some(Edit::MovingGame { hold, done, .. }) | Some(Edit::MovingCollection { hold, done, .. })) =
            self.edit.as_mut()
        else {
            return;
        };
        match pressed {
            true => {
                *hold = Some(Hold {
                    down: now,
                    up: None,
                    dir,
                });
                *done = 0;
            }
            false => {
                if let Some(h) = hold.as_mut().filter(|h| h.dir == dir && h.up.is_none()) {
                    h.up = Some(now);
                }
            }
        }
    }

    fn tick_move(&mut self, now: Millis) {
        use crate::editing::Edit;
        let (target, dir) = match &self.edit {
            Some(Edit::MovingGame {
                hold: Some(h),
                done,
                ..
            })
            | Some(Edit::MovingCollection {
                hold: Some(h),
                done,
                ..
            }) => {
                let target = crate::editing::travel(h, now, 10_000).floor() as usize;
                (target.saturating_sub(*done), h.dir)
            }
            _ => return,
        };
        for _ in 0..target {
            self.carry(dir);
        }
    }

    fn lift_collection(&mut self, now: Millis) {
        use crate::editing::Edit;
        let Some(Edit::MovingGame {
            collection,
            before,
            from,
            hold: None,
            done: 0,
            ..
        }) = self.edit.clone()
        else {
            return;
        };
        self.collections[collection].games = before;
        self.restock_collections();
        let at = self.collection_at(collection);
        self.shelves[at].1.select(from);
        let row: Vec<String> = (0..self.collections.len())
            .filter(|&i| !self.shelves[self.collection_at(i)].1.carts.is_empty())
            .map(|i| self.collections[i].name.clone())
            .collect();
        let name = self.collections[collection].name.clone();
        let Some(pos) = row.iter().position(|n| *n == name) else {
            return;
        };
        self.edit = Some(Edit::MovingCollection {
            before: self.collection_names(),
            row,
            at: pos,
            hold: None,
            done: 0,
            since: now,
        });
    }

    fn carry(&mut self, dir: i32) {
        use crate::editing::Edit;
        if let Some(Edit::MovingCollection { row, at, done, .. }) = self.edit.as_mut() {
            *done += 1;
            let n = row.len();
            if n > 1 {
                let to = (*at as i32 + dir).rem_euclid(n as i32) as usize;
                row.swap(*at, to);
                *at = to;
            }
            return;
        }
        let Some(Edit::MovingGame {
            collection, done, ..
        }) = self.edit.as_mut()
        else {
            return;
        };
        *done += 1;
        let i = *collection;
        let at = self.collection_at(i);
        let Some((from, to)) = self.shelves[at].1.carry(dir) else {
            return;
        };
        let shelf = &self.shelves[at].1;
        let [a, b] = [from, to].map(|k| Recent {
            platform: shelf.carts[k].platform,
            stem: shelf.carts[k].stem.clone(),
        });
        let games = &mut self.collections[i].games;
        if let (Some(p), Some(q)) = (
            games.iter().position(|g| *g == a),
            games.iter().position(|g| *g == b),
        ) {
            games.swap(p, q);
        }
    }

    fn drop_moved(&mut self) {
        use crate::editing::Edit;
        match self.edit.take() {
            Some(Edit::MovingGame { collection, .. }) => {
                self.save_collection(collection);
                self.note_shelf();
            }
            Some(Edit::MovingCollection { before, row, .. }) => {
                let order = crate::editing::reorder_with_hidden(&before, &row);
                let mut collections = self.collections.clone();
                collections.sort_by_key(|c| order.iter().position(|n| *n == c.name));
                self.install_collections(collections);
                self.restock_collections();
                self.save_order();
                self.note_shelf();
            }
            _ => {}
        }
    }

    fn cancel_move(&mut self) {
        if let Some(crate::editing::Edit::MovingGame {
            collection,
            before,
            from,
            ..
        }) = self.edit.take()
        {
            self.collections[collection].games = before;
            self.restock_collections();
            let at = self.collection_at(collection);
            self.shelves[at].1.select(from);
        }
    }

    pub fn moving(&self) -> Option<String> {
        match self.edit.as_ref()? {
            crate::editing::Edit::MovingGame { .. } => Some("Move".to_string()),
            crate::editing::Edit::MovingCollection { row, at, .. } => {
                Some(format!("Move {}", row[*at]))
            }
            _ => None,
        }
    }

    fn open_removing(&mut self, now: Millis) {
        let (Some(collection), Some(game)) = (self.on_collection(), self.selected_game()) else {
            return;
        };
        let last = self.shelf().carts.len() == 1;
        self.edit = Some(crate::editing::Edit::Removing {
            game,
            collection,
            last,
            since: now,
        });
    }

    pub fn removing(&self) -> Option<(&str, bool)> {
        match self.edit.as_ref()? {
            crate::editing::Edit::Removing {
                collection, last, ..
            } => Some((self.collections[*collection].name.as_str(), *last)),
            _ => None,
        }
    }

    fn selecting_input(&mut self, action: Action, now: Millis) {
        use crate::editing::Edit;
        let all = self.state.shelf_mode == ShelfMode::All;
        match action {
            Action::GbaDown(Btn::A) => {
                if let (Some(game), Some(edit)) = (self.selected_game(), self.edit.as_mut()) {
                    edit.toggle(game);
                }
            }
            Action::GbaDown(Btn::Start) => {
                let checked = match &self.edit {
                    Some(Edit::Selecting { checked, .. }) => checked.clone(),
                    _ => return,
                };
                if !checked.is_empty() {
                    let from = self.here();
                    self.open_choosing(checked, from, true, now);
                }
            }
            Action::GbaDown(Btn::B) => {
                if all || !matches!(self.shelves[self.shelf_at].0, Rack::Of(_)) {
                    self.end_edit();
                }
            }
            Action::PreviousShelf => self.switch_shelf(-1),
            Action::NextShelf => self.switch_shelf(1),
            Action::PreviousCollection => self.flip_stack(-1),
            Action::NextCollection => self.flip_stack(1),
            Action::GbaDown(Btn::Up) => self.jump_letter(-1),
            Action::GbaDown(Btn::Down) => self.jump_letter(1),
            Action::ShelfLeft | Action::GbaDown(Btn::Left) => self.shelf_mut().hold_left(now),
            Action::ShelfRight | Action::GbaDown(Btn::Right) => self.shelf_mut().hold_right(now),
            _ => {}
        }
    }

    fn open_choosing(
        &mut self,
        games: Vec<Recent>,
        from: crate::editing::Seat,
        multi: bool,
        now: Millis,
    ) {
        let choices = crate::editing::choices(&self.collections, &games);
        let at = crate::editing::opening_pick(&choices);
        self.edit = Some(crate::editing::Edit::Choosing {
            games,
            from,
            multi,
            choices,
            at,
            since: now,
        });
    }

    fn choosing_input(&mut self, action: Action) {
        use crate::editing::{Choice, Edit};
        let step = match action {
            Action::ShelfLeft | Action::GbaDown(Btn::Left) => -1,
            Action::ShelfRight | Action::GbaDown(Btn::Right) => 1,
            Action::GbaDown(Btn::A) => {
                return match self.picked() {
                    Some(Choice::New) => self.open_keyboard(
                        "New Collection",
                        None,
                        KeyboardFor::CollectionName,
                        slot_ui::Layout::General,
                        "",
                    ),
                    Some(Choice::Collection(i)) => self.add_to_collection(i),
                    None => {}
                };
            }
            Action::GbaDown(Btn::B) => return self.back_out(),
            _ => return,
        };
        let from = self.choosing().and_then(|(_, choices, at)| match crate::editing::layout(choices.len()) {
            crate::editing::Layout::Carousel => Some(self.canvas().w / 2.0 + step as f32 * ROW_GAP),
            layout => self.choice_x(layout, at, at, choices.len()),
        });
        if let Some(Edit::Choosing { choices, at, .. }) = self.edit.as_mut() {
            *at = crate::editing::step_pick(*at, step, choices.len());
        }
        let now = self.now();
        self.glide = from.map(|x| (x, now));
    }

    fn picked_x(&self) -> f32 {
        self.choosing()
            .and_then(|(_, choices, at)| {
                self.choice_x(crate::editing::layout(choices.len()), at, at, choices.len())
            })
            .unwrap_or(self.canvas().w / 2.0)
    }

    fn back_out(&mut self) {
        use crate::editing::Edit;
        let float_x = self.picked_x();
        let snap = self.edit.clone();
        let Some(Edit::Choosing {
            games, from, multi, ..
        }) = self.edit.take()
        else {
            return;
        };
        if let (false, Some(snap), Some(game)) = (multi, snap, games.first()) {
            self.leaving = Some((
                Leaving::Backed {
                    snap,
                    game: game.clone(),
                    float_x,
                },
                self.now(),
            ));
        }
        if multi {
            self.edit = Some(Edit::Selecting {
                checked: games,
                seat: Some(from),
            });
        }
        self.sit(from);
    }

    fn add_to_collection(&mut self, i: usize) {
        let Some((games, _, _)) = self.choosing() else {
            return;
        };
        let fresh = crate::editing::missing(&self.collections[i], games);
        let before = self.stack_of(i);
        self.collections[i].add(&fresh);
        if !fresh.is_empty() {
            self.save_collection(i);
            self.restock_collections();
        }
        self.finish_adding(i, &fresh, before);
    }

    fn finish_adding(&mut self, i: usize, added: &[Recent], before: Vec<Recent>) {
        use crate::editing::Edit;
        let stack_x = self.picked_x();
        let snap = self.edit.clone();
        let Some(Edit::Choosing {
            from, games, multi, ..
        }) = self.edit.take()
        else {
            return;
        };
        if let (Some(snap), Some(game)) = (snap, games.first()) {
            self.leaving = Some((
                Leaving::Added {
                    snap,
                    game: game.clone(),
                    stack_x,
                    multi,
                    before: (i, before),
                },
                self.now(),
            ));
        }
        let name = self.collections[i].name.clone();
        let hidden = self.state.shelf_mode == ShelfMode::Collections
            && matches!(self.shelves[from.shelf].0, Rack::Of(_));
        match hidden {
            false => self.sit(from),
            true => {
                self.land_on(self.collection_at(i));
                if let Some(at) = added.first().and_then(|g| {
                    self.shelf()
                        .carts
                        .iter()
                        .position(|c| c.platform == g.platform && c.stem == g.stem)
                }) {
                    self.shelf_mut().select(at);
                }
            }
        }
        self.say_collection(match added.len() {
            1 => format!("Added to {name}"),
            n => format!("Added {n} to {name}"),
        });
        self.note_shelf();
    }

    fn end_edit(&mut self) {
        self.edit = None;
        let hidden = matches!(
            (self.state.shelf_mode, self.shelves[self.shelf_at].0),
            (ShelfMode::Collections, Rack::Of(_))
        );
        if hidden {
            let to = (0..self.shelves.len()).find_map(|at| self.row_target(at));
            if let Some(to) = to {
                self.land_on(to);
            }
        }
    }

    fn name_collection(&mut self, text: String) {
        let name = text.trim().to_string();
        if !slot_store::name_ok(&name, &self.collections) {
            let taken = self
                .collections
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(&name));
            let why = match taken {
                true => "Name already exists, use a different name",
                false => "Name not allowed, use a different name",
            };
            match self.keyboard.as_mut() {
                Some((kb, _)) => kb.refuse(why),
                None => self.open_keyboard(
                    "New Collection",
                    Some(why),
                    KeyboardFor::CollectionName,
                    slot_ui::Layout::General,
                    &text,
                ),
            }
            return;
        }
        let Some((games, _, _)) = self.choosing() else {
            return;
        };
        let games = games.to_vec();
        self.keyboard = None;
        let from_key = match &self.edit {
            Some(crate::editing::Edit::Choosing { from, .. }) => {
                Some(self.rack_key(self.shelves[from.shelf].0))
            }
            _ => None,
        };
        let mut collections = self.collections.clone();
        collections.push(Collection {
            name: name.clone(),
            games: games.clone(),
        });
        self.install_collections(collections);
        let moved = from_key.and_then(|key| {
            (0..self.shelves.len()).find(|&at| self.rack_key(self.shelves[at].0) == key)
        });
        if let (Some(at), Some(crate::editing::Edit::Choosing { from, .. })) =
            (moved, self.edit.as_mut())
        {
            from.shelf = at;
        }
        let i = self.collections.len() - 1;
        self.save_collection(i);
        self.save_order();
        self.restock_collections();
        self.finish_adding(i, &games, Vec::new());
    }

    pub fn collection_names(&self) -> Vec<String> {
        self.collections.iter().map(|c| c.name.clone()).collect()
    }

    pub fn shelf_key(&self) -> String {
        self.rack_key(self.shelves[self.shelf_at].0)
    }

    pub fn shelf_stems(&self) -> Vec<&str> {
        self.shelf().carts.iter().map(|c| c.stem.as_str()).collect()
    }

    fn remembered_seat(&self) -> Option<(usize, usize)> {
        let key = self.state.shelf.as_deref()?;
        let cart = self.state.shelf_cart.as_ref()?;
        let at = self
            .shelves
            .iter()
            .position(|(r, _)| self.rack_key(*r) == key)?;
        let i = self.shelves[at]
            .1
            .carts
            .iter()
            .position(|c| c.platform == cart.platform && c.stem == cart.stem)?;
        Some((at, i))
    }

    fn note_shelf(&mut self) {
        self.note_collection();
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return self.persist_soon();
        };
        self.state.shelf_cart = Some(Recent {
            platform: cart.platform,
            stem: cart.stem.clone(),
        });
        self.state.shelf = Some(self.rack_key(self.shelves[self.shelf_at].0));
        self.persist_soon();
    }

    fn note_collection(&mut self) {
        self.state.gba_video = match self.shelves[self.shelf_at].0 {
            Rack::Of(Platform::Gba) => false,
            Rack::Of(Platform::Gbav) => true,
            _ => self.state.gba_video,
        };
        self.state.collection = match self.shelves[self.shelf_at].0 {
            Rack::Collection(i) => Some(self.collections[i].name.clone()),
            Rack::Favorites => None,
            _ => return,
        };
    }

    fn remember_shelf(&mut self) {
        self.note_collection();
        let Some(cart) = self.shelf().carts.get(self.shelf().index) else {
            return;
        };
        self.state.shelf_cart = Some(Recent {
            platform: cart.platform,
            stem: cart.stem.clone(),
        });
        self.state.shelf = Some(self.rack_key(self.shelves[self.shelf_at].0));
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
        let network_time = network_time.unwrap_or(self.state.network_time);
        if let Some(power) = &mut self.power {
            power.set_clock(utc);
            if network_time != self.state.network_time {
                power.set_network_time(network_time);
            }
        }
        self.state.zone = zone;
        tell_libc_the_zone(zone);
        self.state.clock_24h = clock_24h;
        self.state.network_time = network_time;
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
        if self.overlay_entries(self.playing_platform()).is_empty() {
            hidden.push(QuickRow::Overlay);
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
        #[cfg(feature = "nds")]
        if self.playing_platform() == Platform::Nds {
            hidden.extend([
                QuickRow::FastForward,
                QuickRow::FastForwardSound,
                QuickRow::Rewind,
                QuickRow::Shader,
            ]);
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
        let hardware = match self.playing_platform().hardware() {
            #[cfg(feature = "nds")]
            Platform::Nds => Platform::Gba,
            p => p,
        };
        match (row, hardware) {
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
            QuickRow::Brightness => Some(QuickValue::level(self.brightness_level())),
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
            QuickRow::ResumeInsert => Some(QuickValue::flag(self.state.resume_insert)),
            QuickRow::ResumePowerOn => Some(QuickValue::flag(self.state.resume_power_on)),
            QuickRow::Turbo => Some(QuickValue::flag(self.state.turbo)),
            QuickRow::Rewind => Some(QuickValue::flag(self.state.rewind)),
            QuickRow::Recents => Some(if self.state.show_recents {
                QuickValue::Show
            } else {
                QuickValue::Hide
            }),
            QuickRow::ShelfMode => Some(match self.state.shelf_mode {
                ShelfMode::All => QuickValue::All,
                ShelfMode::Collections => QuickValue::Collections,
            }),
            QuickRow::KidMode => Some(match self.state.kid_mode {
                KidMode::Off => QuickValue::Off,
                KidMode::OneShelf => QuickValue::OneShelf,
                KidMode::Shelves => QuickValue::Shelves,
            }),
            QuickRow::ShowTitles => Some(if self.state.show_titles {
                QuickValue::Show
            } else {
                QuickValue::Hide
            }),
            QuickRow::BatteryDisplay => Some(match self.state.battery_display {
                BatteryDisplay::Both => QuickValue::Both,
                BatteryDisplay::Icon => QuickValue::Icon,
                BatteryDisplay::Percent => QuickValue::Percentage,
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
            QuickRow::RommProtocol => Some(match self.romm.protocol {
                crate::romm::Protocol::Https => QuickValue::Https,
                crate::romm::Protocol::Http => QuickValue::Http,
            }),
            QuickRow::RommSelfSigned => Some(QuickValue::flag(self.romm.self_signed)),
            QuickRow::Romm
            | QuickRow::RommServer
            | QuickRow::KidShelf
            | QuickRow::RommPort
            | QuickRow::RommPair
            | QuickRow::RommStatus
            | QuickRow::RommPairedServer
            | QuickRow::RommDisconnect
            | QuickRow::DateTime
            | QuickRow::FetchArt
            | QuickRow::Network
            | QuickRow::Wifi
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
            | QuickRow::GbaOverlay
            | QuickRow::GbOverlay
            | QuickRow::GbcOverlay
            | QuickRow::Overlay
            | QuickRow::GbaBezel
            | QuickRow::Bezel
            | QuickRow::General
            | QuickRow::System
            | QuickRow::ShelfPrev
            | QuickRow::ShelfNext
            | QuickRow::ShelfFavorite
            | QuickRow::ShelfForget
            | QuickRow::CollPrev
            | QuickRow::CollNext
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
            | QuickRow::ControlsCollections
            | QuickRow::ControlsInGame => None,
            #[cfg(feature = "nds")]
            QuickRow::FetchBoxArt => None,
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

    pub fn set_quick_romm_faces(&mut self, faces: [Option<[(TexId, u32, u32); 2]>; 4]) {
        self.quick_romm_faces = faces;
    }

    pub fn set_quick_kid_shelf_faces(&mut self, dim: (TexId, u32, u32), lit: (TexId, u32, u32)) {
        self.quick_kid_shelf_faces = Some([dim, lit]);
    }

    pub fn set_kid_icon_faces(&mut self, faces: [(TexId, u32, u32); 2]) {
        self.kid_icon_faces = Some(faces);
    }

    fn draw_kid_icon(&self, out: &mut Vec<Draw>) {
        if !self.kid_locked() {
            return;
        }
        let wifi = self.wifi_up().then(|| self.wifi_face()).flatten();
        let face = self.kid_icon_faces.map(|f| f[usize::from(self.light())]);
        slot_ui::draw_kid_icon(self.canvas(), self.gauge(), wifi, face, out);
    }

    pub fn set_kid_faces(&mut self, faces: Vec<(TexId, u32, u32)>) {
        self.kid_faces = faces;
    }

    pub fn kid_locked(&self) -> bool {
        self.state.kid_mode != KidMode::Off && !self.kid_unlocked
    }

    pub fn kid_prompt(&self) -> Option<KidMode> {
        self.kid_prompt
    }

    pub fn unlock_kid_mode(&mut self) {
        if self.state.kid_mode == KidMode::Off {
            return;
        }
        self.kid_unlocked = true;
        self.hud.toast(Toast::KidUnlocked, self.now());
        let seated = match &self.phase {
            Phase::Inserting { cart, .. }
            | Phase::Playing { cart }
            | Phase::Ejecting { cart, .. } => Some(cart.clone()),
            _ => None,
        };
        if let Some(stem) = seated {
            if let Some((at, i)) = self.seat_of(&stem, self.state.cart_platform) {
                self.shelf_at = at;
                self.shelves[at].1.select(i);
                return;
            }
        }
        if self.shelves[self.shelf_at].0 == Rack::AllGames {
            if let Some(to) = (0..self.shelves.len()).find_map(|at| self.row_target(at)) {
                self.land_on(to);
            }
        }
    }

    fn kid_one_shelf(&self) -> bool {
        self.kid_locked() && self.state.kid_mode == KidMode::OneShelf
    }

    fn kid_shelf_at(&self) -> usize {
        let all = self.rack_at(Rack::AllGames).unwrap_or(0);
        (0..self.shelves.len())
            .find(|&at| {
                at != all
                    && self.rack_key(self.shelves[at].0) == self.state.kid_shelf
                    && !self.shelves[at].1.carts.is_empty()
            })
            .unwrap_or(all)
    }

    fn land_for_kid(&mut self) {
        if !self.kid_one_shelf() {
            return;
        }
        let to = self.kid_shelf_at();
        if to != self.shelf_at {
            self.land_on(to);
        }
    }

    fn restock_all_games(&mut self) {
        let Some(at) = self.rack_at(Rack::AllGames) else {
            return;
        };
        let mut carts: Vec<Cart> = self.carts().cloned().collect();
        carts.sort_by(|a, b| {
            (slot_store::sort_key(&a.stem), a.platform as u8)
                .cmp(&(slot_store::sort_key(&b.stem), b.platform as u8))
        });
        self.restock(at, carts);
    }

    pub fn kid_shelf_choices(&self) -> Vec<String> {
        std::iter::once(slot_store::KID_SHELF_ALL.to_string())
            .chain(
                self.shelves
                    .iter()
                    .filter(|(r, s)| {
                        !matches!(r, Rack::Of(Platform::Gbav) | Rack::AllGames)
                            && !s.carts.is_empty()
                    })
                    .map(|(r, _)| self.rack_key(*r)),
            )
            .collect()
    }

    pub fn kid_shelf_text(&self) -> String {
        self.shelves
            .iter()
            .find(|(r, _)| self.rack_key(*r) == self.state.kid_shelf)
            .map(|(r, _)| self.rack_name(*r).to_string())
            .unwrap_or_else(|| "All Games".to_string())
    }

    fn step_kid_mode(&mut self, right: bool) {
        let order = [KidMode::Off, KidMode::OneShelf, KidMode::Shelves];
        let at = order
            .iter()
            .position(|m| *m == self.state.kid_mode)
            .unwrap_or(0);
        let to = order[match right {
            true => (at + 1) % order.len(),
            false => (at + order.len() - 1) % order.len(),
        }];
        if self.state.kid_mode == KidMode::Off {
            self.kid_prompt = Some(to);
            return;
        }
        self.state.kid_mode = to;
        self.persist();
    }

    fn step_kid_shelf(&mut self, right: bool) {
        let choices = self.kid_shelf_choices();
        let n = choices.len();
        let at = choices
            .iter()
            .position(|c| *c == self.state.kid_shelf)
            .unwrap_or(0);
        self.state.kid_shelf = choices[match right {
            true => (at + 1) % n,
            false => (at + n - 1) % n,
        }]
        .clone();
        self.persist();
    }

    fn confirm_kid_mode(&mut self, mode: KidMode) {
        self.kid_prompt = None;
        self.state.kid_mode = mode;
        self.kid_unlocked = false;
        self.persist();
        self.phase = Phase::Shelf;
        self.land_for_kid();
    }

    pub fn romm_texts(&self) -> [String; 4] {
        let host = self.romm.host();
        let server = match host.is_empty() {
            true => "Not Set".to_string(),
            false if host.chars().count() > ROMM_HOST_SHOWN => {
                let head: String = host.chars().take(ROMM_HOST_SHOWN - 3).collect();
                format!("{head}...")
            }
            false => host.to_string(),
        };
        let row = match (self.romm.paired(), self.romm_online) {
            (false, _) => "SETUP",
            (true, true) => "ONLINE",
            (true, false) => "OFFLINE",
        }
        .to_string();
        let port = self.romm.port.map(|p| p.to_string()).unwrap_or_default();
        let status = self.romm_note.clone().unwrap_or("Paired".to_string());
        [row, server, port, status]
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
            #[cfg(feature = "nds")]
            Platform::Nds => &mut self.state.bezel_gba,
            Platform::Gb => &mut self.state.bezel_gb,
            Platform::Gbc => &mut self.state.bezel_gbc,
        }
    }

    pub fn resolved_bezel(&self, platform: Platform) -> BezelPick {
        let pick = match platform {
            Platform::Gba | Platform::Gbav => &self.state.bezel_gba,
            #[cfg(feature = "nds")]
            Platform::Nds => &self.state.bezel_gba,
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

    pub fn overlay_frame(&self) -> (u32, u32) {
        match slot_gfx::fills(self.panel) {
            true => slot_gfx::panel_pixels(self.panel),
            false => (slot_gfx::OUT_W, slot_gfx::OUT_H),
        }
    }

    pub fn overlay_entries(&self, platform: Platform) -> Vec<slot_store::OverlayEntry> {
        let frame = self.overlay_frame();
        let Some(root) = self.root() else {
            return Vec::new();
        };
        let now = std::time::Instant::now();
        let mut cache = self
            .overlay_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(scan) = cache.iter().find(|s| {
            s.platform == platform.hardware()
                && s.frame == frame
                && now.duration_since(s.at) < OVERLAY_RESCAN
        }) {
            return scan.entries.clone();
        }
        let entries = crate::overlay::usable(root, platform, frame, frame);
        cache.retain(|s| s.platform != platform.hardware());
        cache.push(OverlayScan {
            platform: platform.hardware(),
            frame,
            at: now,
            entries: entries.clone(),
        });
        entries
    }

    fn overlay_pick(&mut self, platform: Platform) -> &mut Option<String> {
        match platform {
            Platform::Gba | Platform::Gbav => &mut self.state.overlay_gba,
            #[cfg(feature = "nds")]
            Platform::Nds => &mut self.state.overlay_nds,
            Platform::Gb => &mut self.state.overlay_gb,
            Platform::Gbc => &mut self.state.overlay_gbc,
        }
    }

    pub fn resolved_overlay(&self, platform: Platform) -> Option<slot_store::OverlayEntry> {
        let pick = match platform {
            Platform::Gba | Platform::Gbav => &self.state.overlay_gba,
            #[cfg(feature = "nds")]
            Platform::Nds => &self.state.overlay_nds,
            Platform::Gb => &self.state.overlay_gb,
            Platform::Gbc => &self.state.overlay_gbc,
        };
        let name = pick.as_deref()?;
        self.overlay_entries(platform)
            .into_iter()
            .find(|e| e.name == name)
    }

    pub fn overlay_label(&self, platform: Platform) -> String {
        self.resolved_overlay(platform)
            .map_or_else(|| "Off".to_string(), |e| e.name)
    }

    fn cycle_overlay(&mut self, platform: Platform, right: bool) {
        let mut choices: Vec<Option<String>> = vec![None];
        choices.extend(self.overlay_entries(platform).into_iter().map(|e| Some(e.name)));
        let now = self.resolved_overlay(platform).map(|e| e.name);
        let at = choices.iter().position(|c| *c == now).unwrap_or(0);
        let n = choices.len();
        *self.overlay_pick(platform) = choices[if right { (at + 1) % n } else { (at + n - 1) % n }].clone();
    }

    pub fn set_quick_overlay_faces(
        &mut self,
        platform: Platform,
        dim: (TexId, u32, u32),
        lit: (TexId, u32, u32),
    ) {
        self.quick_overlay_faces[overlay_slot(platform)] = Some([dim, lit]);
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
        if self.picture_fills() || self.table_panel() {
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
        if self.picture_fills() || self.table_panel() {
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
        self.collection_wallpaper = self.collection_wallpaper_here();
        if let Some(name) = &self.collection_wallpaper {
            return Some(name.clone());
        }
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

    fn shown_rack(&self) -> Option<Rack> {
        match self.picked() {
            Some(crate::editing::Choice::Collection(i)) => Some(Rack::Collection(i)),
            Some(crate::editing::Choice::New) => None,
            None => Some(self.shelves[self.shelf_at].0),
        }
    }

    fn wallpaper_for_shelf(&self) -> Option<String> {
        wallpaper_of_rack(self.shown_rack()?, &self.wallpaper_files())
    }

    fn collection_wallpaper_here(&self) -> Option<String> {
        let Some(Rack::Collection(i)) = self.shown_rack() else {
            return None;
        };
        let name = &self.collections[i].name;
        self.collection_pngs
            .iter()
            .cloned()
            .find(|file| {
                file.rsplit_once('.')
                    .is_some_and(|(stem, _)| stem.eq_ignore_ascii_case(name))
            })
            .map(|file| format!("Collections/{file}"))
    }

    pub fn wallpapers_preload(&self) -> bool {
        self.state.wallpaper == WallpaperPick::Shelf || !self.collection_pngs.is_empty()
    }

    pub fn shelf_wallpapers(&self) -> Vec<String> {
        let files = self.wallpaper_files();
        let mut names: Vec<String> = match self.state.wallpaper {
            WallpaperPick::Shelf => self
                .shelves
                .iter()
                .filter(|(rack, _)| *rack != Rack::AllGames || self.kid_one_shelf())
                .filter_map(|(rack, _)| wallpaper_of_rack(*rack, &files))
                .collect(),
            WallpaperPick::File(_) => match self.resolved_wallpaper() {
                WallpaperPick::File(name) => vec![name],
                _ => Vec::new(),
            },
            _ => Vec::new(),
        };
        names.extend(self.collection_pngs.iter().filter_map(|file| {
            let (stem, _) = file.rsplit_once('.')?;
            self.collections
                .iter()
                .any(|c| c.name.eq_ignore_ascii_case(stem))
                .then(|| format!("Collections/{file}"))
        }));
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
        let ground = self.ground.map(|t| (t, self.ground_w));
        let blur = self.blur.map(|t| (t, self.ground_w)).or(ground);
        let own = self
            .wallpaper
            .map(|t| (t, self.wallpaper_size().0))
            .or(ground);
        [ground, blur, own]
    }

    fn menu_ground(&self) -> Option<slot_ui::MenuGround> {
        let tint = match self.own_wallpaper_shown() {
            true => tint_alpha(self.state.wallpaper_tint),
            false => 0.0,
        };
        self.menu_backdrop()
            .map(|(tex, w)| slot_ui::MenuGround { tex, w, tint })
    }

    fn menu_backdrop(&self) -> Option<(TexId, u32)> {
        let [_, blur, own] = self.grounds();
        if self.collection_wallpaper.is_some() {
            return own;
        }
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
            _ if self.collection_wallpaper.is_some() => own,
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
                h: self.canvas().h,
                colour: [0.0, 0.0, 0.0, alpha],
            });
        }
    }

    fn own_wallpaper_shown(&self) -> bool {
        if self.collection_wallpaper.is_some() {
            return self.wallpaper.is_some();
        }
        match self.state.wallpaper {
            WallpaperPick::File(_) => self.wallpaper.is_some(),
            WallpaperPick::Random => self.random_drawn() && self.wallpaper.is_some(),
            WallpaperPick::Shelf => self.shelf_wallpaper.is_some() && self.wallpaper.is_some(),
            _ => false,
        }
    }

    pub fn hidden_rows(&self) -> Vec<QuickRow> {
        let hidden = self.hidden_rows_any_panel();
        #[cfg(feature = "nds")]
        let hidden = {
            let mut hidden = hidden;
            if !self.two_panels {
                hidden.push(QuickRow::FetchBoxArt);
            }
            hidden
        };
        hidden
    }

    fn hidden_rows_any_panel(&self) -> Vec<QuickRow> {
        let own = matches!(
            self.state.wallpaper,
            WallpaperPick::File(_) | WallpaperPick::Random | WallpaperPick::Shelf
        );
        let mut hidden = Vec::new();
        if self.state.kid_mode != KidMode::OneShelf {
            hidden.push(QuickRow::KidShelf);
        }
        if !own {
            hidden.push(QuickRow::WallpaperTint);
        }
        if !self.framed_panel() {
            hidden.push(QuickRow::GbaBezel);
        }
        for (row, platform) in [
            (QuickRow::GbaOverlay, Platform::Gba),
            (QuickRow::GbOverlay, Platform::Gb),
            (QuickRow::GbcOverlay, Platform::Gbc),
        ] {
            if self.overlay_entries(platform).is_empty() {
                hidden.push(row);
            }
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
            hidden.extend([QuickRow::Network, QuickRow::Wifi]);
        }
        if self.state.wifi_off {
            hidden.push(QuickRow::Networks);
        }
        if !cfg!(feature = "romm") {
            hidden.push(QuickRow::Romm);
        }
        match self.romm.paired() {
            true => hidden.extend(QuickRow::ROMM_SETUP),
            false => hidden.extend(QuickRow::ROMM_PAIRED),
        }
        if self.romm.protocol == crate::romm::Protocol::Http {
            hidden.push(QuickRow::RommSelfSigned);
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

    pub fn set_sticker_face(&mut self, face: TexId, w: u32, h: u32) {
        self.sticker_face = Some((face, w, h));
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

    pub fn set_ground_width(&mut self, w: u32) {
        self.ground_w = w;
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

    pub fn set_ds_slot_faces(&mut self, back: TexId, front: TexId) {
        self.ds_slot_faces = Some((back, front));
    }

    fn top_slot_faces(&self) -> Option<(TexId, TexId)> {
        match self.two_panels {
            true => self.ds_slot_faces.or(self.slot_faces),
            false => self.slot_faces,
        }
    }

    fn slot_faces_for(&self, stem: &str) -> Option<(TexId, TexId)> {
        #[cfg(feature = "nds")]
        if self.shelf().find(stem).is_some_and(|(c, _)| c.platform == Platform::Nds) {
            return self.top_slot_faces();
        }
        let _ = stem;
        self.slot_faces
    }

    pub fn set_shelf_platform_face(&mut self, face: TexId, w: u32) {
        self.shelf_platform = slot_ui::Printed::new(face, w);
    }

    pub fn shelf_platform_missing(&self) -> bool {
        self.shelf_platform.face.is_none()
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
        (1.0 - self.slot_name_alpha() / SLOT_NAME_ALPHA) * (1.0 - self.edit_dim())
    }

    fn edit_dim(&self) -> f32 {
        use crate::editing::{ease_in_out, progress, Edit, ENTER_MOVE_MS};
        match &self.edit {
            Some(Edit::Choosing { since, .. }) | Some(Edit::Removing { since, .. }) => {
                ease_in_out(progress(self.now(), *since, ENTER_MOVE_MS))
            }
            _ => 0.0,
        }
    }

    pub fn shelf_platform_name(&self) -> Option<&str> {
        self.next_shelf(1)?;
        Some(self.rack_name(self.shelves[self.shelf_at].0))
    }

    pub fn set_battery_percent_face(&mut self, face: TexId, w: u32) {
        self.battery_percent = slot_ui::Printed::new(face, w);
    }

    fn gauge(&self) -> slot_ui::Gauge {
        slot_ui::Gauge {
            battery: self.battery,
            percent: self.battery_percent,
            display: self.state.battery_display,
        }
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
        self.shelf()
            .carts
            .iter()
            .find(|c| c.stem == *stem && self.seated_platform.is_none_or(|p| c.platform == p))
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
        wants.extend(self.bundle_seats());
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
        let bundle = self.bundle_seats();
        let mut shed = std::mem::take(&mut self.orphaned);
        for (at, (_, shelf)) in self.shelves.iter_mut().enumerate() {
            let mut keep = if at == self.shelf_at {
                shelf.around(FACE_KEEP)
            } else if neighbours.contains(&at) {
                shelf.on_screen()
            } else {
                Vec::new()
            };
            keep.extend(bundle.iter().filter(|(s, _)| *s == at).map(|(_, i)| *i));
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
            #[cfg(feature = "nds")]
            Platform::Nds => self.state.scale_gba,
            Platform::Gb => self.state.scale_gb,
            Platform::Gbc => self.state.scale_gbc,
        }
    }

    fn stretch_of(&self, platform: Platform) -> Stretch {
        match platform {
            Platform::Gb => self.state.stretch_gb,
            Platform::Gbc => self.state.stretch_gbc,
            Platform::Gba | Platform::Gbav => Stretch::Classic,
            #[cfg(feature = "nds")]
            Platform::Nds => Stretch::Classic,
        }
    }

    pub fn game_layout(&self) -> GameLayout {
        let (sizing, source) = self.picture();
        self.layout_of(self.playing_platform(), sizing, source)
    }

    fn layout_of(&self, platform: Platform, sizing: Sizing, source: [f32; 4]) -> GameLayout {
        #[cfg(feature = "nds")]
        if platform == Platform::Nds {
            return slot_gfx::game_layout(self.panel, platform.picture(), sizing);
        }
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
            #[cfg(feature = "nds")]
            Platform::Nds => self.state.scale_gba = to,
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
        if self.kid_locked() {
            return crate::input::Table::kid(
                &self.controls,
                &self.shortcuts,
                &self.carousel,
                self.state.kid_mode,
                self.state.turbo,
            );
        }
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
            let fresh = CarouselKeys::default();
            for &a in Self::carousel_page(row) {
                self.carousel.set(a, fresh.get(a));
            }
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
            let fresh = CarouselKeys::default();
            Self::carousel_page(row)
                .iter()
                .all(|a| self.carousel.get(*a) == fresh.get(*a))
        }
    }

    fn carousel_page(row: QuickRow) -> &'static [slot_store::CarouselAction] {
        match row.page() == &QuickRow::COLLECTION_KEYS[..] {
            true => &slot_store::COLLECTION_ACTIONS,
            false => &slot_store::SHELF_ACTIONS,
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
        #[cfg(feature = "nds")]
        if self.playing_platform() == Platform::Nds {
            return false;
        }
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
        #[cfg(feature = "nds")]
        if self.playing_platform() == Platform::Nds {
            return false;
        }
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
        power.set_network_time(self.state.network_time);
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
            #[cfg(feature = "nds")]
            Platform::Nds => Shader::Off,
            Platform::Gb => self.state.shader_gb,
            Platform::Gbc => self.state.shader_gbc,
        }
    }

    pub fn screen_effect(&self) -> slot_gfx::ScreenEffect {
        use slot_gfx::ScreenEffect;
        match self.screen_shader() {
            Shader::Off => ScreenEffect::None,
            Shader::Lcd3x => ScreenEffect::lcd_for(self.panel),
            Shader::Grid => ScreenEffect::Grid,
            Shader::Dot => ScreenEffect::Dot,
            Shader::Simpletex => ScreenEffect::Simpletex,
            Shader::Shimmerless => ScreenEffect::Shimmerless,
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
            #[cfg(feature = "nds")]
            Platform::Nds => false,
            Platform::Gb => self.state.colour_correction_gb,
            Platform::Gbc => self.state.colour_correction_gbc,
        }
    }

    pub fn blending_for(&self, platform: Platform) -> Blending {
        match platform {
            Platform::Gba | Platform::Gbav => self.state.blending_gba,
            #[cfg(feature = "nds")]
            Platform::Nds => Blending::Off,
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
        !self.kid_locked() && self.repair_possible() && self.charged_to_repair()
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

    pub fn selected_cart(&self) -> Option<&Cart> {
        self.shelf().carts.get(self.shelf().index)
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
        if matches!(self.phase, Phase::SetClock { .. }) && self.kid_locked() {
            if matches!(action, Action::GbaDown(Btn::A | Btn::B)) {
                self.confirm_clock();
            }
            return;
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
        let action = match self.kid_locked() {
            true => {
                let at = match self.phase {
                    Phase::Shelf => crate::kid::KidWhere::Shelf,
                    Phase::Playing { .. } => crate::kid::KidWhere::Playing,
                    _ => crate::kid::KidWhere::Other,
                };
                match crate::kid::allowed(at, self.state.kid_mode, action) {
                    Some(Action::Eject) if self.single_cart() => return,
                    Some(kept) => kept,
                    None => return,
                }
            }
            false => action,
        };
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
        if let Some(mode) = self.kid_prompt {
            match action {
                Action::GbaDown(Btn::A) => self.confirm_kid_mode(mode),
                Action::GbaDown(Btn::B) => self.kid_prompt = None,
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
        if matches!(self.phase, Phase::Shelf) {
            if matches!(action, Action::GbaDown(_)) {
                self.leaving = None;
            }
            match action {
                Action::GbaDown(Btn::Y) if self.edit.is_none() => self.press = Some((Btn::Y, now)),
                Action::GbaUp(Btn::Y) | Action::FavoriteHold => self.press = None,
                _ => {}
            }
        }
        match self.phase {
            Phase::Shelf => match action {
                Action::SelectHold if self.core_picker.is_none() && self.studio.is_none() => {
                    if self.edit.is_none() {
                        let mut edit = crate::editing::Edit::selecting();
                        if let Some(game) = self.selected_game() {
                            edit.toggle(game);
                        }
                        self.edit = Some(edit);
                    }
                }
                Action::FavoriteHold
                    if self.edit.is_none()
                        && self.core_picker.is_none()
                        && self.studio.is_none()
                        && self.on_collection().is_none() =>
                {
                    if let Some(game) = self.selected_game() {
                        let from = self.here();
                        self.open_choosing(vec![game], from, false, now);
                    }
                }
                Action::FavoriteHold
                    if self.edit.is_none()
                        && self.core_picker.is_none()
                        && self.studio.is_none() =>
                {
                    self.open_removing(now)
                }
                Action::ForgetRecent
                    if self.edit.is_none()
                        && self.core_picker.is_none()
                        && self.studio.is_none()
                        && self.on_collection().is_some() =>
                {
                    self.open_moving(now)
                }
                _ if self.edit.is_some() && self.core_picker.is_none() => self.edit_input(action, now),
                Action::SelectTap if self.core_picker.is_none() => self.open_studio(),
                Action::GbaDown(Btn::Start) if self.core_picker.is_none() => {
                    self.open_core_picker()
                }
                _ if self.core_picker.is_some() => self.core_picker_input(action),
                Action::PreviousShelf => self.switch_shelf(-1),
                Action::NextShelf => self.switch_shelf(1),
                Action::PreviousCollection => self.flip_stack(-1),
                Action::NextCollection => self.flip_stack(1),
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
                    if self.play_held.take().is_some() && !self.unwrap_seated() {
                        self.insert(!self.state.resume_insert);
                    }
                }
                Action::Insert if !self.unwrap_seated() => {
                    self.insert(!self.state.resume_insert);
                }
                _ => {}
            },
            Phase::Inserting { .. } if action == Action::Eject => self.eject(),
            Phase::Playing { .. } => match action {
                Action::GbaDown(Btn::L1)
                    if self.slot_owns_the_shoulders()
                        && !self.bound_to_game(Btn::L1)
                        && !self.kid_locked() =>
                {
                    self.set_picture(VideoMode::Stretch)
                }
                Action::GbaDown(Btn::R1)
                    if self.slot_owns_the_shoulders()
                        && !self.bound_to_game(Btn::R1)
                        && !self.kid_locked() =>
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
            Phase::RommPair => self.pair_input(action),
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
        match (row, action) {
            (QuickRow::WallpaperTint, Action::GbaDown(btn @ (Btn::Left | Btn::Right))) => {
                self.change_setting(row, btn == Btn::Right);
                return self.peek_tint();
            }
            (_, Action::GbaDown(_)) if self.tint_peek.take().is_some() => self.hud.hush(),
            _ => {}
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

    fn peek_tint(&mut self) {
        let now = self.now();
        let into = (self.tint_peek_alpha(now) * PEEK_IN_MS as f32) as Millis;
        self.tint_peek = Some(now.saturating_sub(into));
        let said = match self.state.wallpaper_tint {
            WallpaperTint::Off => Toast::TintOff,
            WallpaperTint::Low => Toast::TintLow,
            WallpaperTint::Medium => Toast::TintMedium,
            WallpaperTint::High => Toast::TintHigh,
        };
        self.hud.toast(said, now);
    }

    pub fn tint_peek_alpha(&self, now: Millis) -> f32 {
        let Some(at) = self.tint_peek else {
            return 0.0;
        };
        let t = now.saturating_sub(at);
        if t < PEEK_IN_MS {
            t as f32 / PEEK_IN_MS as f32
        } else if t < PEEK_IN_MS + PEEK_HOLD_MS {
            1.0
        } else {
            let out = t - PEEK_IN_MS - PEEK_HOLD_MS;
            (1.0 - out as f32 / PEEK_OUT_MS as f32).max(0.0)
        }
    }

    fn draw_tint_peek(&self, out: &mut Vec<Draw>) {
        let alpha = self.tint_peek_alpha(self.now());
        let Some((tex, tex_w)) = self.menu_backdrop().filter(|_| alpha > 0.0) else {
            return;
        };
        let canvas = self.canvas();
        out.push(Draw::Part {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            tex,
            alpha,
            uv: canvas.cover(tex_w as f32),
        });
        let tint = tint_alpha(self.state.wallpaper_tint);
        if tint > 0.0 && self.own_wallpaper_shown() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: canvas.w,
                h: canvas.h,
                colour: [0.0, 0.0, 0.0, tint * alpha],
            });
        }
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
                    let first = child.shown(&self.in_game_hidden()).first().copied();
                    self.in_game_menu = Some(first.unwrap_or(child));
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
            QuickRow::Server | QuickRow::Romm | QuickRow::FetchArt if !self.wifi_up() => {
                self.set_update(Some(UpdateState::Notice(NetNotice::NoNetwork)));
            }
            #[cfg(feature = "nds")]
            QuickRow::FetchBoxArt if !self.wifi_up() => {
                self.set_update(Some(UpdateState::Notice(NetNotice::NoNetwork)));
            }
            QuickRow::DateTime => {
                self.phase = clock_screen(self.utc_secs(), &self.state, self.has_saved(), true);
            }
            QuickRow::About => self.phase = Phase::About,
            QuickRow::FetchArt => self.open_fetch_missing_art(),
            #[cfg(feature = "nds")]
            QuickRow::FetchBoxArt => self.open_fetch_box_art(),
            QuickRow::CheckUpdates => self.open_check_for_updates(),
            QuickRow::Server => self.open_server(),
            QuickRow::WifiPower => self.set_wifi_off(!self.state.wifi_off),
            QuickRow::Networks => self.open_networks(),
            QuickRow::RommProtocol | QuickRow::RommSelfSigned => self.change_setting(row, true),
            QuickRow::RommServer => {
                let text = self.romm.server.clone();
                self.open_keyboard(
                    "Server",
                    None,
                    KeyboardFor::RommServer,
                    slot_ui::Layout::Host,
                    &text,
                );
            }
            QuickRow::RommPort => {
                let text = self.romm.port.map(|p| p.to_string()).unwrap_or_default();
                self.open_keyboard(
                    "Port",
                    None,
                    KeyboardFor::RommPort,
                    slot_ui::Layout::Numeric,
                    &text,
                );
            }
            QuickRow::RommPair => self.pair_romm(),
            QuickRow::RommDisconnect => {
                self.romm_jobs.ask(RommJob::Disconnect(self.romm.clone()));
            }
            QuickRow::RommStatus | QuickRow::RommPairedServer => {}
            QuickRow::Screen
            | QuickRow::ScreenGeneral
            | QuickRow::Game
            | QuickRow::ScreenGba
            | QuickRow::ScreenGbc
            | QuickRow::ScreenGb
            | QuickRow::Shortcuts
            | QuickRow::General
            | QuickRow::System
            | QuickRow::Network
            | QuickRow::Romm
            | QuickRow::Wifi
            | QuickRow::Controls
            | QuickRow::ControlsShelf
            | QuickRow::ControlsCollections
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
            | QuickRow::ResumeInsert
            | QuickRow::ResumePowerOn
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
            | QuickRow::ShelfMode
            | QuickRow::ShowTitles
            | QuickRow::BatteryDisplay
            | QuickRow::KidMode
            | QuickRow::KidShelf
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
            | QuickRow::GbaOverlay
            | QuickRow::GbOverlay
            | QuickRow::GbcOverlay
            | QuickRow::Overlay
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
            | QuickRow::CollPrev
            | QuickRow::CollNext
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
        match row {
            QuickRow::RommProtocol => {
                self.romm.protocol = match self.romm.protocol {
                    crate::romm::Protocol::Https => crate::romm::Protocol::Http,
                    crate::romm::Protocol::Http => crate::romm::Protocol::Https,
                };
                return self.save_romm();
            }
            QuickRow::RommSelfSigned => {
                self.romm.self_signed = !self.romm.self_signed;
                return self.save_romm();
            }
            _ => {}
        }
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
        if row == QuickRow::Overlay {
            self.cycle_overlay(self.playing_platform(), right);
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
        if row == QuickRow::Brightness {
            self.step_brightness(right);
            return;
        }
        if row == QuickRow::BlueLight {
            let now = self.state.blue_light;
            let to = match right {
                true => up(now, 1, BLUE_LIGHT_MAX),
                false => now.saturating_sub(1),
            };
            self.set_level(HudKind::BlueLight, to);
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
        if let QuickRow::GbaOverlay | QuickRow::GbOverlay | QuickRow::GbcOverlay = row {
            let platform = match row {
                QuickRow::GbaOverlay => Platform::Gba,
                QuickRow::GbOverlay => Platform::Gb,
                _ => Platform::Gbc,
            };
            self.cycle_overlay(platform, right);
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
        if row == QuickRow::ShelfMode {
            self.set_shelf_mode(match self.state.shelf_mode {
                ShelfMode::All => ShelfMode::Collections,
                ShelfMode::Collections => ShelfMode::All,
            });
            return;
        }
        if row == QuickRow::KidMode {
            return self.step_kid_mode(right);
        }
        if row == QuickRow::KidShelf {
            return self.step_kid_shelf(right);
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
            QuickRow::ResumeInsert => s.resume_insert = !s.resume_insert,
            QuickRow::ResumePowerOn => s.resume_power_on = !s.resume_power_on,
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
            QuickRow::BatteryDisplay => s.battery_display = s.battery_display.step(right),
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
            | QuickRow::ShelfMode
            | QuickRow::ShowTitles
            | QuickRow::KidMode
            | QuickRow::KidShelf
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
            | QuickRow::GbaOverlay
            | QuickRow::GbOverlay
            | QuickRow::GbcOverlay
            | QuickRow::Overlay
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
            | QuickRow::System
            | QuickRow::ShelfPrev
            | QuickRow::ShelfNext
            | QuickRow::ShelfFavorite
            | QuickRow::ShelfForget
            | QuickRow::CollPrev
            | QuickRow::CollNext
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
            | QuickRow::ControlsCollections
            | QuickRow::ControlsInGame
            | QuickRow::LidOff
            | QuickRow::FetchArt
            | QuickRow::Network
            | QuickRow::Wifi
            | QuickRow::WifiPower
            | QuickRow::Networks
            | QuickRow::CheckUpdates
            | QuickRow::Romm
            | QuickRow::RommProtocol
            | QuickRow::RommServer
            | QuickRow::RommPort
            | QuickRow::RommSelfSigned
            | QuickRow::RommPair
            | QuickRow::RommStatus
            | QuickRow::RommPairedServer
            | QuickRow::RommDisconnect
            | QuickRow::Server => return,
            #[cfg(feature = "nds")]
            QuickRow::FetchBoxArt => return,
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
            #[cfg(feature = "nds")]
            Platform::Nds => (&mut self.state.shader_gba, gba_shaders),
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
                #[cfg(feature = "nds")]
                Platform::Nds => QuickRow::GbaColourCorrection,
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
        if let Action::BrightnessUp | Action::BrightnessDown = action {
            self.step_brightness(action == Action::BrightnessUp);
            let now = self.now();
            let muted = self.muted();
            self.hud
                .show(HudKind::Brightness, self.brightness_level(), muted, now);
            return true;
        }
        let s = &self.state;
        let (kind, value) = match action {
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

    fn step_brightness(&mut self, brighter: bool) {
        let s = &self.state;
        let dim = match brighter {
            true => s.dim.saturating_sub(1),
            false if s.brightness == 0 => up(s.dim, 1, DIM_MAX),
            false => s.dim,
        };
        if dim != s.dim {
            self.state.dim = dim;
            self.persist_soon();
            return;
        }
        let to = match brighter {
            true => up(s.brightness, 1, BRIGHTNESS_MAX),
            false => s.brightness.saturating_sub(1),
        };
        self.set_level(HudKind::Brightness, to);
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

    pub fn dim(&self) -> u8 {
        self.state.dim
    }

    pub fn brightness_level(&self) -> u8 {
        DIM_MAX - self.state.dim.min(DIM_MAX) + self.state.brightness
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
        self.take_eject_write(false);
        if self.name_pending && self.shelf_platform.face.is_some() {
            self.name_pending = false;
            self.shelf_named = Some(self.now());
        }
        self.timers();
        self.poll_link();
        let now = self.now();
        self.tick_move(now);
        let lead = match (&self.edit, self.move_view(now)) {
            (Some(crate::editing::Edit::MovingGame { .. }), Some((_, cam))) => {
                self.shelf().index as f32 - cam
            }
            _ => 0.0,
        };
        self.shelf_mut().set_lead(lead);
        if let Some((l, start)) = &self.leaving {
            if now.saturating_sub(*start) > Self::leaving_ms(l) {
                self.leaving = None;
            }
        }
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
            self.press = None;
        }
        if let Phase::Unwrap { stem } = &self.phase {
            if self.unwrap_scene.is_none_or(|s| s.done(now)) {
                let stem = stem.clone();
                self.finish_unwrap(&stem);
            }
        }
        let mut touched = false;
        let at = self.shelf_at;
        let fly = self.ds_fly_rates();
        let clicks = self.cart_clicks();
        let next = match &mut self.phase {
            Phase::Shelf => {
                let shelf = &mut self.shelves[at].1;
                shelf.tick(now);
                shelf.update(dt);
                None
            }
            Phase::Unwrap { .. } => {
                self.shelves[at].1.update(dt);
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
                *t += match *t < SEATED_AT {
                    true => dt * fly.0,
                    false => dt,
                };
                let at = SEATED_AT - clicks.0.lead() * fly.0;
                touched = !*resumed && was < at && *t >= at;
                (*t >= INSERT_S && *core_ready).then(|| Phase::Playing {
                    cart: std::mem::take(cart),
                })
            }
            Phase::Ejecting { t, .. } => {
                if self.screen <= 0.0 {
                    let was = *t;
                    *t += match *t >= 0.0 {
                        true => dt * fly.1,
                        false => dt,
                    };
                    let at = clicks.2 - clicks.1.lead() * fly.1;
                    touched = was < at && *t >= at;
                }
                (*t >= EJECT_S).then_some(Phase::Shelf)
            }
            _ => None,
        };
        if touched {
            self.sfx = Some(match self.phase {
                Phase::Ejecting { .. } => clicks.1,
                _ => clicks.0,
            });
        }
        if let Some(phase) = next {
            if matches!(self.phase, Phase::Ejecting { .. }) && self.two_panels && self.ds_seated() {
                self.ds_landed = Some(self.clock);
            }
            self.phase = phase;
            self.refused_from = None;
            if matches!(self.phase, Phase::Shelf) {
                self.land_for_kid();
            }
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

    fn legend_y(&self) -> f32 {
        self.canvas().h - MOUTH_H
    }

    pub fn wallpaper_size(&self) -> (u32, u32) {
        let c = Canvas::of(self.panel);
        (c.w as u32, c.h as u32)
    }

    pub fn game_bezel_size(&self) -> (u32, u32) {
        let c = self.canvas();
        match slot_gfx::framed(self.panel) {
            true => (c.w as u32, c.h as u32),
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
            h: self.canvas().h,
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

    fn timers(&mut self) {
        self.step_server();
        self.take_art_events();
        self.take_romm_events();
        self.romm_tick(self.now());
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
        self.hud.set_alarm_toast(
            match (
                self.repair_possible() && !self.kid_locked(),
                self.charged_to_repair(),
            ) {
                (true, true) => Toast::CardRepair,
                (true, false) => Toast::CardRepairCharge,
                (false, _) => Toast::CardUnwritable,
            },
        );
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
            self.shelf_at = self.fallback_shelf();
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
        self.restock_all_games();
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

    pub fn expects_sound(&self) -> bool {
        matches!(
            self.phase,
            Phase::Inserting { resumed: false, .. } | Phase::Ejecting { .. }
        )
    }

    pub fn set_two_panels(&mut self, on: bool) {
        self.two_panels = on && cfg!(feature = "nds");
    }

    pub fn two_panels(&self) -> bool {
        self.two_panels
    }

    pub fn draw(&self, out: &mut Vec<Draw>) {
        self.draw_screen(crate::screen::Screen::Top, out);
    }

    pub fn draw_screen(&self, screen: crate::screen::Screen, out: &mut Vec<Draw>) {
        use crate::screen::Screen;
        let below = self.menu_on_bottom();
        let single = |out: &mut Vec<Draw>| match self.server_frame() {
            Some((frame, tex)) => self.draw_server(&frame, &tex, out),
            None => self.draw_app(below, out),
        };
        if !self.two_panels {
            if screen == Screen::Top {
                single(out);
            }
            return;
        }
        let ds = self.ds_seated();
        let gaming = matches!(
            self.phase,
            Phase::Inserting { .. } | Phase::Playing { .. } | Phase::Ejecting { .. }
        );
        match (screen, below) {
            (Screen::Top, true) => self.draw_top_under_menu(out),
            (Screen::Bottom, true) => single(out),
            (Screen::Top, false) if self.ds_flying() => self.draw_ds_fling_top(out),
            (Screen::Bottom, false) if self.ds_flying() => self.draw_ds_fling_bottom(out),
            (Screen::Bottom, false) if self.ds_powering_on() => self.draw_ds_power_bottom(out),
            (Screen::Top, false) if ds => single(out),
            (Screen::Top, false) if gaming && self.screen > 0.0 && self.panel_lit() => out.push(Draw::Game),
            (Screen::Top, false) => self.draw_top_idle(out),
            (Screen::Bottom, false) if ds => {}
            (Screen::Bottom, false) if matches!(self.phase, Phase::Playing { .. }) => self.draw_bottom_slot(out),
            (Screen::Bottom, false) => {
                let from = out.len();
                single(out);
                let mut i = from;
                while i < out.len() {
                    if matches!(out[i], Draw::Game) {
                        out.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }

    fn ds_fly_rates(&self) -> (f32, f32) {
        match self.two_panels && self.ds_seated() {
            true => (SEATED_AT / DS_INSERT_FLY_S, EJECT_S / DS_EJECT_FLY_S),
            false => (1.0, 1.0),
        }
    }

    fn cart_clicks(&self) -> (Sfx, Sfx, f32) {
        #[cfg(feature = "nds")]
        if self.two_panels && self.ds_seated() {
            return (Sfx::DsInsert, Sfx::DsEject, DS_PRESS_END * EJECT_S);
        }
        (Sfx::Insert, Sfx::Eject, Sfx::Eject.lead())
    }

    fn ds_flying(&self) -> bool {
        self.two_panels
            && self.ds_seated()
            && matches!(self.phase, Phase::Inserting { .. } | Phase::Ejecting { .. })
            && self.screen <= 0.0
            && self.seat() < 1.0
    }

    fn flying_stem(&self) -> Option<&str> {
        match &self.phase {
            Phase::Inserting { cart, .. } | Phase::Ejecting { cart, .. } => Some(cart),
            _ => None,
        }
    }

    fn ds_powering_on(&self) -> bool {
        self.two_panels
            && self.ds_seated()
            && matches!(
                self.phase,
                Phase::Inserting { .. } | Phase::Playing { .. } | Phase::Ejecting { .. }
            )
            && self.seat() >= 1.0
            && self.screen < 1.0
    }

    fn draw_ds_power_bottom(&self, out: &mut Vec<Draw>) {
        self.draw_background(out);
        draw_empty_slot(self.canvas(), self.slot_faces, out);
        self.draw_status_footer(out);
        if self.screen > 0.0 && self.panel_lit() {
            out.push(Draw::Game);
        }
    }

    fn ds_inserting(&self) -> bool {
        matches!(self.phase, Phase::Inserting { .. })
    }

    fn draw_ds_fling_top(&self, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        self.draw_background(out);
        slot_ui::draw_slot_back(canvas, self.top_slot_faces(), 1.0, out);
        let seat = self.seat();
        let placed = match self.ds_inserting() {
            true => ds_insert_top(seat),
            false => ds_eject_top(1.0 - seat),
        };
        if let (Some([dy, sx, sy]), Some(stem)) = (placed, self.flying_stem()) {
            if let Some((cart, face)) = self.shelf().find(stem) {
                let [x, y, w, h] = slot_ui::seated_rect(canvas, cart.platform);
                let (w2, h2) = (w * sx, h * sy);
                let foot = y + h + dy;
                self.draw_moving_cart(stem, face, [x + (w - w2) / 2.0, foot - h2, w2, h2], out);
            }
        }
        slot_ui::draw_slot_front(canvas, self.top_slot_faces(), 1.0, out);
        self.draw_status_footer(out);
    }

    fn ds_parted(&self) -> Option<f32> {
        let at = self.ds_landed?;
        let k = ((self.clock - at) / 1000.0) as f32 / DS_PART_S;
        (k < 1.0).then(|| 1.0 - ease(k.max(0.0)))
    }

    fn draw_ds_fling_bottom(&self, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        self.draw_background(out);
        let Some(stem) = self.flying_stem() else {
            return;
        };
        let seat = self.seat();
        let part = match self.ds_inserting() {
            true => ease((seat * DS_INSERT_FLY_S / DS_PART_S).clamp(0.0, 1.0)),
            false => 1.0,
        };
        self.shelf().draw_row_parted(canvas, Some(stem), part, out);
        let placed = match self.ds_inserting() {
            true => ds_insert_bottom(seat),
            false => ds_eject_bottom(1.0 - seat),
        };
        if let Some([lift, sx, sy]) = placed {
            if let Some((cart, face)) = self.shelf().find(stem) {
                let (rest, scale) = self.shelf().selected_at(canvas);
                let (cw, ch) = slot_ui::cart_box(cart.platform);
                let (w, h) = (cw as f32 * scale, ch as f32 * scale);
                let foot = slot_ui::foot_y(canvas, ch as f32);
                let (w2, h2) = (w * sx, h * sy);
                let at = foot - lift * (foot + 8.0);
                self.draw_moving_cart(stem, face, [rest + (w - w2) / 2.0, at - h2, w2, h2], out);
            }
        }
        draw_empty_slot(canvas, self.slot_faces, out);
        self.draw_status_footer(out);
    }

    fn draw_moving_cart(&self, stem: &str, face: Option<TexId>, rect: [f32; 4], out: &mut Vec<Draw>) {
        let [x, y, w, h] = rect;
        for tex in self.shelf().behind(stem).into_iter().flatten() {
            out.push(Draw::Tex { x, y, w, h, tex, alpha: 1.0 });
        }
        match face {
            Some(tex) => out.push(Draw::Tex { x, y, w, h, tex, alpha: 1.0 }),
            None => {
                let c = slot_ui::label_colour(&slot_ui::clean_label(stem));
                let colour = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
                out.push(Draw::Rect { x, y, w, h, colour });
            }
        }
    }

    fn draw_status_footer(&self, out: &mut Vec<Draw>) {
        draw_footer(
            self.canvas(),
            self.gauge(),
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
    }

    fn draw_top_idle(&self, out: &mut Vec<Draw>) {
        self.draw_background(out);
        draw_empty_slot(self.canvas(), self.top_slot_faces(), out);
        draw_footer(
            self.canvas(),
            self.gauge(),
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
    }

    fn draw_bottom_slot(&self, out: &mut Vec<Draw>) {
        let Phase::Playing { cart } = &self.phase else {
            return;
        };
        self.draw_background(out);
        self.draw_seated_mouth(cart, out);
        draw_footer(
            self.canvas(),
            self.gauge(),
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
    }

    pub fn bottom_source(&self) -> BottomSource {
        if !self.two_panels {
            return BottomSource::Black;
        }
        if self.menu_on_bottom() {
            return BottomSource::Compositor;
        }
        if self.ds_seated() && !self.ds_flying() && !self.ds_powering_on() {
            return BottomSource::DsScreen;
        }
        BottomSource::Compositor
    }

    fn ds_seated(&self) -> bool {
        #[cfg(feature = "nds")]
        {
            self.seated_cart().is_some_and(|c| c.platform == Platform::Nds)
        }
        #[cfg(not(feature = "nds"))]
        {
            false
        }
    }

    fn menu_on_bottom(&self) -> bool {
        if !self.two_panels {
            return false;
        }
        let menu_phase = matches!(
            self.phase,
            Phase::QuickMenu { .. }
                | Phase::About
                | Phase::Networks { .. }
                | Phase::NetworkScan { .. }
                | Phase::NetworkPage { .. }
                | Phase::RommPair
                | Phase::Polaroids { .. }
                | Phase::SetClock { .. }
        );
        let in_game = matches!(self.phase, Phase::Playing { .. }) && self.in_game_menu_shown().is_some();
        menu_phase
            || in_game
            || self.game_menu.is_some()
            || self.keyboard.is_some()
            || self.fetch_art.is_some()
            || self.studio.is_some()
            || self.update.is_some()
            || self.repair_prompt
            || self.kid_prompt.is_some()
            || self.reset_prompt.is_some()
            || self.forget_prompt
            || self.delete_prompt
            || self.server_frame().is_some()
    }

    pub fn set_box_art(&mut self, faces: Option<BoxArtFaces>) {
        self.box_art = faces;
    }

    pub fn box_art(&self) -> Option<&BoxArtFaces> {
        self.box_art.as_ref()
    }

    pub fn top_previews_game(&self) -> bool {
        !self.ds_seated()
            && matches!(self.phase, Phase::Playing { .. })
            && self
                .in_game_menu_shown()
                .is_some_and(|r| r == QuickRow::PlayScreen || r.page() == &QuickRow::PLAY_SCREEN[..])
    }

    fn draw_top_under_menu(&self, out: &mut Vec<Draw>) {
        if self.top_previews_game() {
            self.push_game(out);
            return;
        }
        let over_game = matches!(self.phase, Phase::Playing { .. }) && self.game_visible();
        match over_game {
            true => out.push(Draw::Game),
            false => self.draw_background(out),
        }
        #[cfg(feature = "nds")]
        {
            let art = self.box_art.as_ref().filter(|a| {
                self.seated_cart().is_some_and(|c| c.stem == a.stem && c.platform == a.platform)
            });
            slot_ui::BoxScreen {
                canvas: self.canvas(),
                boxed: art.and_then(|a| a.boxed),
                logo: art.and_then(|a| a.logo),
                over_game,
            }
            .draw(out);
        }
        match self.seated_cart().filter(|_| self.ds_seated()) {
            Some(cart) => self.draw_seated_mouth(&cart.stem, out),
            None => draw_empty_slot(self.canvas(), self.top_slot_faces(), out),
        }
        draw_footer(
            self.canvas(),
            self.gauge(),
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
    }

    fn draw_app(&self, below: bool, out: &mut Vec<Draw>) {
        if self.shutting_down() {
            out.push(Draw::Rect {
                x: 0.0,
                y: 0.0,
                w: self.canvas().w,
                h: self.canvas().h,
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
                    y: ((self.canvas().h as u32 - h) / 2) as f32,
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
        if self.kid_prompt.is_some() {
            return self.draw_prompt(&self.kid_faces, out);
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
            Phase::QuickMenu { .. } if self.keyboard.is_some() => self.draw_networks(out),
            Phase::QuickMenu { row } => {
                QuickMenu {
                    row: *row,
                    hidden: &self.hidden_rows(),
                    bound: Some(&self.bound_faces()),
                    description: self.description(*row),
                    values: QuickRow::ALL.map(|r| self.quick_value(r)),
                    clock: self.quick_clock_faces,
                    wallpaper: self.quick_wallpaper_faces,
                    networks: self.quick_network_faces,
                    romm: self.quick_romm_faces,
                    kid_shelf: self.quick_kid_shelf_faces,
                    faces: self.quick_menu_faces.as_ref(),
                    ground: self.menu_ground(),
                    palette: self.quick_palette_faces,
                    gba_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gba)],
                    gb_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gb)],
                    gbc_bezel: self.quick_bezel_faces[bezel_slot(Platform::Gbc)],
                    bezel: None,
                    gba_overlay: self.quick_overlay_faces[overlay_slot(Platform::Gba)],
                    gb_overlay: self.quick_overlay_faces[overlay_slot(Platform::Gb)],
                    gbc_overlay: self.quick_overlay_faces[overlay_slot(Platform::Gbc)],
                    overlay: None,
                    over_game: false,
                    footer_top: self.canvas().h,
                    canvas: self.canvas(),
                }
                .draw(out);
                if !self.two_panels {
                    slot_ui::draw_corners(
                        self.canvas(),
                        self.gauge(),
                        self.hud_bolt,
                        self.shelf_clock,
                        out,
                    );
                }
                self.draw_tint_peek(out);
            }
            Phase::Shelf if self.keyboard.is_some() => self.draw_networks(out),
            Phase::Shelf | Phase::Unwrap { .. } => {
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
                        if self.edit.is_some() {
                            self.draw_picking(out);
                        } else if self.draw_leaving(out) {
                        } else if self.draw_pressing(out) {
                        } else if let Some(part) = self.ds_parted() {
                            self.shelf().draw_row_parted(self.canvas(), None, part, out);
                            draw_empty_slot(self.canvas(), self.slot_faces, out);
                        } else if !self.draw_unwrap(out) {
                            self.shelf()
                                .draw(self.canvas(), self.shelf_shake(), self.slot_faces, out);
                        }
                        let name_alpha = match &self.edit {
                            None | Some(crate::editing::Edit::Selecting { .. }) => {
                                self.slot_name_alpha()
                            }
                            Some(_) => 0.0,
                        };
                        slot_ui::draw_slot_title(
                            self.canvas(),
                            self.shelf_platform,
                            name_alpha,
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
                if self.edit.is_some() && self.core_picker_shown().is_none() {
                    slot_ui::draw_plugged_slot(self.canvas(), self.plugged_face, 1.0, out);
                    self.draw_picking_legends(out);
                    self.draw_heading(out);
                } else {
                    draw_footer(
                        self.canvas(),
                        self.gauge(),
                        self.bolt,
                        self.shelf_clock,
                        footer_ink(self.light()),
                        out,
                    );
                    if self.wifi_up() {
                        slot_ui::draw_wifi_icon(self.canvas(), self.gauge(), self.wifi_face(), out);
                    }
                    self.draw_kid_icon(out);
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
            Phase::RommPair => {
                self.draw_pair(out);
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
            Phase::Playing { .. } if below && !self.ds_seated() => self.draw_background(out),
            Phase::Playing { .. } => self.push_game(out),
            Phase::Polaroids { .. } if self.guide.is_some() => {
                if let Some((g, _)) = &self.guide {
                    g.draw(
                        self.canvas(),
                        self.gauge(),
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
                        self.gauge(),
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
                        self.gauge(),
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
                    h: self.canvas().h,
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
                romm: [None; 4],
                kid_shelf: None,
                faces: self.quick_menu_faces.as_ref(),
                ground: None,
                palette: self.quick_palette_faces,
                gba_bezel: None,
                gb_bezel: None,
                gbc_bezel: None,
                bezel: self.quick_bezel_faces[bezel_slot(self.playing_platform())],
                gba_overlay: None,
                gb_overlay: None,
                gbc_overlay: None,
                overlay: self.quick_overlay_faces[overlay_slot(self.playing_platform())],
                over_game: true,
                footer_top: self.canvas().h - MOUTH_H + self.menu_footer_offset(),
                canvas: self.canvas(),
            }
            .draw(out);
            if row.page() != QuickRow::PLAY_SCREEN && !self.two_panels {
                slot_ui::draw_corners(
                    self.canvas(),
                    self.gauge(),
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

    pub fn placeholder_bezel(&self) -> Option<[f32; 4]> {
        self.table_panel().then_some(PLACEHOLDER_BEZEL)
    }

    fn table_panel(&self) -> bool {
        let c = self.canvas();
        (c.w as u32, c.h as u32) != (OUT_W, OUT_H) && !slot_gfx::framed(self.panel)
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
            self.draw_menu_legends(row, self.legend_y() + dy, out);
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
        if self.framed_panel() && self.screen > 0.0 {
            self.draw_seated_mouth(cart, out);
        }
        draw_footer(
            self.canvas(),
            self.gauge(),
            self.bolt,
            self.shelf_clock,
            footer_ink(self.light()),
            out,
        );
        if self.wifi_up() {
            slot_ui::draw_wifi_icon(self.canvas(), self.gauge(), self.wifi_face(), out);
        }
        self.draw_kid_icon(out);
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
            slot: self.slot_faces_for(stem),
            canvas: self.canvas(),
        }
        .draw(out);
    }

    fn draw_prompt(&self, faces: &[(TexId, u32, u32)], out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas().w,
            h: self.canvas().h,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let [lines @ .., no, yes] = faces else {
            return;
        };
        let gaps = REPAIR_GAP * lines.len().saturating_sub(1) as u32;
        let total = lines.iter().map(|l| l.2).sum::<u32>() + gaps;
        let mut y = ((self.canvas().h as u32).saturating_sub(total) / 2) as f32;
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
        self.draw_side_legend((no.0, no.1), false, self.legend_y(), out);
        self.draw_side_legend((yes.0, yes.1), true, self.legend_y(), out);
    }

    fn draw_about_legend(&self, out: &mut Vec<Draw>) {
        let Some(faces) = &self.quick_menu_faces else {
            return;
        };
        let [back, _, _, _, restart, ..] = match self.light() {
            true => faces.legend_light,
            false => faces.legend,
        };
        self.draw_side_legend(back, false, self.legend_y(), out);
        self.draw_side_legend(restart, true, self.legend_y(), out);
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
        self.draw_spread_legends(&legends, self.legend_y(), 1.0, out);
    }

    fn draw_spread_legends(
        &self,
        legends: &[(TexId, u32)],
        y: f32,
        alpha: f32,
        out: &mut Vec<Draw>,
    ) {
        let alphas = vec![alpha; legends.len()];
        self.draw_spread_legends_each(legends, y, &alphas, out);
    }

    fn draw_spread_legends_each(
        &self,
        legends: &[(TexId, u32)],
        y: f32,
        alphas: &[f32],
        out: &mut Vec<Draw>,
    ) {
        let seen = |w: u32| w.saturating_sub(HINT_EDGE) as f32;
        let room = self.canvas().w - 2.0 * LINK_LEGEND_EDGE;
        let gap =
            (room - legends.iter().map(|l| seen(l.1)).sum::<f32>()) / (legends.len() - 1) as f32;
        let mut x = LINK_LEGEND_EDGE;
        for (&(tex, w), &alpha) in legends.iter().zip(alphas) {
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

    fn draw_picking(&self, out: &mut Vec<Draw>) {
        use crate::editing::Edit;
        let Some(edit) = &self.edit else {
            return;
        };
        let canvas = self.canvas();
        let now = self.now();
        match edit {
            Edit::Selecting { .. } => {
                let shelf = self.shelf();
                let turn = |i: usize| {
                    let c = &shelf.carts[i];
                    match edit.is_checked(c.platform, &c.stem) {
                        true => crate::editing::soft_jiggle(now, (i % 2) as f32 * 0.45),
                        false => 0.0,
                    }
                };
                shelf.draw_row_turned(canvas, None, 1.0, &turn, &|_| (0.0, 0.0, 1.0), out);
            }
            Edit::Choosing {
                games,
                at,
                choices,
                since,
                multi,
                ..
            } => {
                let slide = crate::editing::ease_in(crate::editing::progress(now, *since, crate::editing::ENTER_SLIDE_MS));
                let move_ = crate::editing::ease_in_out(crate::editing::progress(now, *since, crate::editing::ENTER_MOVE_MS));
                let fade = self.edit_fade();
                if move_ < 1.0 {
                    let hidden = match (multi, games.first()) {
                        (false, Some(g)) => Some(g.stem.as_str()),
                        _ => None,
                    };
                    self.draw_parted(hidden, slide * PART_PUSH, 1.0 - slide, out);
                }
                self.draw_dim_at(move_, out);
                let target = self.draw_choices(choices, *at, fade, out);
                let carousel = crate::editing::layout(choices.len()) == crate::editing::Layout::Carousel;
                let picked_x = match self.glide {
                    Some((from, t)) if !carousel => {
                        from + (target - from) * crate::editing::ease_out(crate::editing::progress(now, t, GLIDE_MS))
                    }
                    _ => target,
                };
                match (games.as_slice(), multi) {
                    ([one], false) => {
                        let float = self.float_rect(one, picked_x, ADD_FLOAT_TOP + self.canvas().inset_y(), ADD_FLOAT);
                        let rect = Self::lerp_rect(self.home_rect(one), float, move_);
                        self.draw_game_at(one, rect, 1.0, 0.0, out);
                    }
                    _ => self.draw_bundle_at(games, picked_x, out),
                }
            }
            Edit::Removing { last: true, since, .. } => {
                let e = crate::editing::ease_in_out(crate::editing::progress(now, *since, crate::editing::ENTER_MOVE_MS));
                self.draw_dim_at(e, out);
                let shelf = self.shelf();
                let Some(cart) = shelf.carts.get(shelf.index) else {
                    return;
                };
                let (cw, ch) = slot_ui::cart_box(cart.platform);
                let s = 1.0 + (LAST_SCALE - 1.0) * e;
                let (w, h) = (cw as f32 * s, ch as f32 * s);
                let foot = slot_ui::foot_y(self.canvas(), ch as f32);
                let y = foot - ch as f32 + (ch as f32 - h) / 2.0 + LAST_DROP * e;
                shelf.draw_cart_at(shelf.index, [canvas.w / 2.0 - w / 2.0, y, w, h], out);
            }
            Edit::Removing { game, since, .. } => {
                let slide = crate::editing::ease_in(crate::editing::progress(now, *since, crate::editing::ENTER_SLIDE_MS));
                let move_ = crate::editing::ease_in_out(crate::editing::progress(now, *since, crate::editing::ENTER_MOVE_MS));
                let fade = self.edit_fade();
                if move_ < 1.0 {
                    self.draw_parted(Some(game.stem.as_str()), slide * PART_PUSH, 1.0 - slide, out);
                }
                self.draw_dim_at(move_, out);
                let shelf_at = self.shelf_at;
                let around = crate::editing::remove_stack(&self.shelf_games(shelf_at), self.shelf().index);
                let rise = (1.0 - fade) * ENTER_RISE;
                self.draw_stack(&around, canvas.w / 2.0, REMOVE_BASE + rise, STACK_PICKED, fade, 0.0, out);
                let float = self.float_rect(game, canvas.w / 2.0, REMOVE_FLOAT_TOP + self.canvas().inset_y(), REMOVE_FLOAT);
                self.draw_game_at(game, Self::lerp_rect(self.home_rect(game), float, move_), 1.0, 0.0, out);
            }
            Edit::MovingGame { hold, .. } => {
                let shelf = self.shelf();
                let held = shelf.index;
                let n = shelf.carts.len();
                let still = crate::editing::move_still(hold.as_ref(), now);
                let turn = |i: usize| match i == held {
                    true => crate::editing::soft_jiggle(now, 0.0) * still,
                    false => 0.0,
                };
                let (ahead, dir) = self.move_pass(now);
                let passed = (held as i32 + dir).rem_euclid(n.max(1) as i32) as usize;
                let (slide, dip, grow) = crate::editing::pass(ahead);
                let d = dir as f32;
                let shift = |i: usize| match i {
                    _ if dir == 0 || n < 2 => (0.0, 0.0, 1.0),
                    i if i == held => (d * ahead, 0.0, 1.0),
                    i if i == passed => (-d * slide, dip, grow),
                    _ => (0.0, 0.0, 1.0),
                };
                shelf.draw_row_turned(canvas, None, 1.0, &turn, &shift, out);
            }
            Edit::MovingCollection { row, at, since, .. } => {
                let gather = crate::editing::ease_in_out(crate::editing::progress(now, *since, crate::editing::GATHER_MS));
                if gather < 1.0 {
                    self.draw_parted(None, 0.0, 1.0 - gather, out);
                }
                self.draw_dim_at(gather, out);
                let (held, cam) = self.move_view(now).unwrap_or((*at as f32, *at as f32));
                let (ahead, dir) = self.move_pass(now);
                let n = row.len() as i32;
                let passed = (*at as i32 + dir).rem_euclid(n.max(1));
                let (slide, dip, grow) = crate::editing::pass(ahead);
                let order = (0..row.len())
                    .filter(|&k| k != *at)
                    .chain(std::iter::once(*at));
                for k in order {
                    let name = &row[k];
                    let off = k as i32 - *at as i32;
                    let passing = dir != 0 && n > 1 && k as i32 == passed;
                    let Some(i) = self.collections.iter().position(|c| c.name == *name) else {
                        continue;
                    };
                    let games = self.stack_of(i);
                    let raw = match (off, passing) {
                        (0, _) => held - cam,
                        (_, true) => k as f32 - dir as f32 * slide - cam,
                        _ => k as f32 - cam,
                    };
                    let place = (raw + n as f32 / 2.0).rem_euclid(n as f32) - n as f32 / 2.0;
                    if place.abs() > 1.6 {
                        continue;
                    }
                    let cx = canvas.w / 2.0 + place * ROW_GAP;
                    let (scale, alpha, lift) = match off {
                        0 => (ROW_HELD, 1.0, ROW_LIFT),
                        _ => {
                            let a = place.abs().min(1.0);
                            let (dip, grow) = match passing {
                                true => (dip, grow),
                                false => (0.0, 1.0),
                            };
                            (
                                ROW_HELD * (1.0 + (ROW_SIDE - 1.0) * a) * grow,
                                1.0 + (ROW_SIDE_ALPHA - 1.0) * a,
                                dip,
                            )
                        }
                    };
                    self.draw_stack(&games, cx, (ROW_BASE + self.canvas().inset_y()) + lift, scale, alpha * gather, 0.0, out);
                    if off != 0 {
                        self.draw_name(name, false, cx, ROW_NAME_Y + self.canvas().inset_y(), gather, out);
                    }
                }
            }
        }
    }

    pub fn move_offset(&self) -> Option<f32> {
        self.move_view(self.now()).map(|(held, cam)| held - cam)
    }

    pub fn pressing(&self) -> bool {
        self.press.is_some()
    }

    fn draw_pressing(&self, out: &mut Vec<Draw>) -> bool {
        let Some((Btn::Y, since)) = self.press else {
            return false;
        };
        let t = self.now().saturating_sub(since);
        if t < crate::editing::JIGGLE_DELAY_MS || self.core_picker.is_some() || self.studio.is_some() {
            return false;
        }
        let shelf = self.shelf();
        let Some(cart) = shelf.carts.get(shelf.index) else {
            return false;
        };
        let canvas = self.canvas();
        let dim = crate::editing::neighbor_dim(t);
        shelf.draw_row(canvas, Some(cart.stem.as_str()), self.shelf_shake(), 0.0, dim, out);
        let (x, scale) = shelf.selected_at(canvas);
        let (cw, ch) = slot_ui::cart_box(cart.platform);
        let (w, h) = (cw as f32 * scale, ch as f32 * scale);
        let rect = [x, slot_ui::foot_y(self.canvas(), ch as f32) - h, w, h];
        let start = out.len();
        shelf.draw_cart_at(shelf.index, rect, out);
        slot_ui::turn_about(&mut out[start..], [x + w / 2.0, rect[1] + h / 2.0], crate::editing::hold_jiggle(t));
        draw_empty_slot(canvas, self.slot_faces, out);
        true
    }

    fn draw_dim_at(&self, amount: f32, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        let [r, g, b, a] = EDIT_DIM;
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            colour: [r, g, b, a * amount],
        });
    }

    fn edit_fade(&self) -> f32 {
        use crate::editing::{ease_out, progress, Edit, ENTER_FADE_MS, ENTER_MOVE_MS};
        let since = match &self.edit {
            Some(Edit::Choosing { since, .. }) | Some(Edit::Removing { since, .. }) => *since,
            Some(Edit::MovingCollection { since, .. }) => {
                return ease_out(progress(self.now(), *since, crate::editing::GATHER_MS));
            }
            _ => return 1.0,
        };
        ease_out(progress(self.now(), since + ENTER_MOVE_MS - 40, ENTER_FADE_MS))
    }

    fn move_pass(&self, now: Millis) -> (f32, i32) {
        use crate::editing::Edit;
        let (hold, done) = match &self.edit {
            Some(Edit::MovingCollection { hold, done, .. }) | Some(Edit::MovingGame { hold, done, .. }) => (hold, *done),
            _ => return (0.0, 0),
        };
        let Some(h) = hold else {
            return (0.0, 0);
        };
        let ahead = crate::editing::travel(h, now, 10_000) - done as f32;
        (ahead.clamp(0.0, 1.0), h.dir)
    }

    fn move_view(&self, now: Millis) -> Option<(f32, f32)> {
        use crate::editing::Edit;
        let (at, hold, done) = match &self.edit {
            Some(Edit::MovingCollection { at, hold, done, .. }) => (*at as f32, hold, *done),
            Some(Edit::MovingGame { hold, done, .. }) => (self.shelf().index as f32, hold, *done),
            _ => return None,
        };
        let Some(h) = hold else {
            return Some((at, at));
        };
        let dir = h.dir as f32;
        let start = at - dir * done as f32;
        let moved = crate::editing::travel(h, now, 10_000);
        let held = start + dir * moved;
        let trail = |m: f32| start + dir * (m - 1.0).max(0.0);
        let cam = match h.up {
            None => trail(moved),
            Some(up) => {
                let from = trail(crate::editing::travel(h, up, 10_000));
                let to = start + dir * crate::editing::travel(h, up + 60_000, 10_000);
                let settle = crate::editing::ease_in_out(crate::editing::progress(now, up, crate::editing::CAMERA_MS));
                from + (to - from) * settle
            }
        };
        Some((held, cam))
    }

    fn shelf_games(&self, at: usize) -> Vec<Recent> {
        self.shelves[at]
            .1
            .carts
            .iter()
            .map(|c| Recent {
                platform: c.platform,
                stem: c.stem.clone(),
            })
            .collect()
    }

    fn stack_of(&self, i: usize) -> Vec<Recent> {
        crate::editing::stack_games(&self.collections[i], &self.shelf_games(self.collection_at(i)))
    }

    fn choice_x(&self, layout: crate::editing::Layout, k: usize, at: usize, n: usize) -> Option<f32> {
        use crate::editing::Layout;
        let mid = self.canvas().w / 2.0;
        let off = match layout {
            Layout::Single => 0.0,
            Layout::Pair => [-PAIR_GAP, PAIR_GAP][k],
            Layout::Trio => [-TRIO_GAP, 0.0, TRIO_GAP][k],
            Layout::Carousel => {
                let n = n as i32;
                let mut off = k as i32 - at as i32;
                if off > n / 2 {
                    off -= n;
                }
                if off < -(n / 2) {
                    off += n;
                }
                if off.abs() > 2 {
                    return None;
                }
                off as f32 * ROW_GAP
            }
        };
        Some(mid + off)
    }

    fn draw_choices(
        &self,
        choices: &[crate::editing::Choice],
        at: usize,
        fade: f32,
        out: &mut Vec<Draw>,
    ) -> f32 {
        self.draw_choices_as(choices, at, fade, None, None, out)
    }

    fn draw_choices_as(
        &self,
        choices: &[crate::editing::Choice],
        at: usize,
        fade: f32,
        swap: Option<&(usize, Vec<Recent>)>,
        fresh: Option<(&str, f32)>,
        out: &mut Vec<Draw>,
    ) -> f32 {
        use crate::editing::{Choice, Layout};
        let layout = crate::editing::layout(choices.len());
        let rise = (1.0 - fade) * ENTER_RISE;
        let mid = self.canvas().w / 2.0;
        let slide = match (layout, self.glide) {
            (Layout::Carousel, Some((from, t))) => {
                (from - mid) * (1.0 - crate::editing::ease_out(crate::editing::progress(self.now(), t, GLIDE_MS)))
            }
            _ => 0.0,
        };
        let mut picked_x = mid;
        for (k, choice) in choices.iter().enumerate() {
            let Some(cx) = self.choice_x(layout, k, at, choices.len()) else {
                continue;
            };
            let cx = cx + slide;
            if cx < -ROW_GAP || cx > self.canvas().w + ROW_GAP {
                continue;
            }
            let picked = k == at;
            let (scale, alpha) = match (picked, layout) {
                (true, _) => (STACK_PICKED, 1.0),
                (false, Layout::Carousel) => (STACK_PICKED * ROW_SIDE, ROW_SIDE_ALPHA),
                (false, _) => (STACK_SIDE, STACK_SIDE_ALPHA),
            };
            if fade > 0.0 {
                match choice {
                    Choice::Collection(i) => {
                        let stack = match swap {
                            Some((k, before)) if k == i => before.clone(),
                            _ => self.stack_of(*i),
                        };
                        self.draw_stack(&stack, cx, (STACK_BASE + self.canvas().inset_y()) + rise, scale, alpha * fade, 0.0, out);
                    }
                    Choice::New => {
                        let solid = fresh.map_or(1.0, |(_, a)| a);
                        self.draw_new_stack(cx, (STACK_BASE + self.canvas().inset_y()) + rise, scale, alpha * fade * solid, out)
                    }
                }
                let y = match picked {
                    true => PICKED_NAME_Y + self.canvas().inset_y(),
                    false => SIDE_NAME_Y + self.canvas().inset_y(),
                };
                let name = match (choice, fresh) {
                    (Choice::New, Some((name, _))) => name,
                    _ => self.choice_name(*choice),
                };
                self.draw_name(name, picked, cx, y + rise, fade, out);
            }
            if picked {
                picked_x = cx - slide;
            }
        }
        picked_x
    }

    fn float_rect(&self, g: &Recent, cx: f32, top: f32, scale: f32) -> [f32; 4] {
        let (cw, ch) = slot_ui::cart_box(g.platform);
        let (w, h) = (cw as f32 * scale, ch as f32 * scale);
        [cx - w / 2.0, top, w, h]
    }

    fn home_rect(&self, g: &Recent) -> [f32; 4] {
        let (cw, ch) = slot_ui::cart_box(g.platform);
        let (w, h) = (cw as f32, ch as f32);
        [self.canvas().w / 2.0 - w / 2.0, slot_ui::foot_y(self.canvas(), h) - h, w, h]
    }

    fn fresh_rect(&self, g: &Recent, cx: f32) -> [f32; 4] {
        let (cw, ch) = slot_ui::cart_box(g.platform);
        let (w, h) = (cw as f32 * STACK_PICKED, ch as f32 * STACK_PICKED);
        [cx - w / 2.0, (STACK_BASE + self.canvas().inset_y()) - (NEW_H + h) / 2.0, w, h]
    }

    fn front_rect(&self, g: &Recent, cx: f32) -> [f32; 4] {
        let (cw, ch) = slot_ui::cart_box(g.platform);
        let (w, h) = (cw as f32 * STACK_PICKED, ch as f32 * STACK_PICKED);
        [cx + STACK_SPREAD - w / 2.0, (STACK_BASE + self.canvas().inset_y()) - h, w, h]
    }

    fn draw_game_at(&self, g: &Recent, rect: [f32; 4], alpha: f32, turn: f32, out: &mut Vec<Draw>) {
        let Some((at, i)) = self.bundle_seat(g) else {
            return;
        };
        let start = out.len();
        self.shelves[at].1.draw_cart_faded(i, rect, alpha, out);
        slot_ui::turn_about(&mut out[start..], [rect[0] + rect[2] / 2.0, rect[1] + rect[3] / 2.0], turn);
    }

    fn draw_parted(&self, hidden: Option<&str>, push: f32, alpha: f32, out: &mut Vec<Draw>) {
        if alpha <= 0.0 {
            return;
        }
        let start = out.len();
        let canvas = self.canvas();
        self.shelf().draw_row(canvas, hidden, 0.0, 0.0, alpha, out);
        let mid = canvas.w / 2.0;
        for draw in &mut out[start..] {
            let dx = |x: f32, w: f32| if x + w / 2.0 < mid { -push } else { push };
            match draw {
                Draw::Rect { x, w, .. } | Draw::Tex { x, w, .. } | Draw::Part { x, w, .. } | Draw::Turned { x, w, .. } => {
                    *x += dx(*x, *w);
                }
                Draw::Quad { corners, .. } => {
                    let cx = corners.iter().map(|c| c[0]).sum::<f32>() / 4.0;
                    let d = if cx < mid { -push } else { push };
                    for c in corners {
                        c[0] += d;
                    }
                }
                _ => {}
            }
        }
    }

    fn lerp_rect(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
        [0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * t)
    }

    fn stack_rects(&self, games: &[Recent], cx: f32, base: f32, scale: f32) -> Vec<([f32; 4], f32)> {
        let s = scale / STACK_PICKED;
        let fan = [(-1.0, -STACK_TILT), (0.0, 0.0), (1.0, STACK_TILT)];
        games
            .iter()
            .take(3)
            .enumerate()
            .map(|(k, g)| {
                let slot = match games.len().min(3) {
                    1 => 1,
                    2 => [2, 0][k],
                    _ => [2, 1, 0][k],
                };
                let (side, tilt) = fan[slot];
                let (cw, ch) = slot_ui::cart_box(g.platform);
                let (w, h) = (cw as f32 * scale, ch as f32 * scale);
                let x = cx + side * STACK_SPREAD * s - w / 2.0;
                let dy = if side == 0.0 { -STACK_RAISE * s } else { 0.0 };
                ([x, base - h + dy, w, h], tilt)
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_stack(
        &self,
        games: &[Recent],
        cx: f32,
        base: f32,
        scale: f32,
        alpha: f32,
        wob: f32,
        out: &mut Vec<Draw>,
    ) {
        let rects = self.stack_rects(games, cx, base, scale);
        for (g, (rect, tilt)) in games.iter().zip(rects).rev() {
            self.draw_game_at(g, rect, alpha, tilt + wob, out);
        }
    }

    fn side_rect(&self, g: &Recent, side: f32) -> [f32; 4] {
        let (cw, ch) = slot_ui::cart_box(g.platform);
        let (w, h) = (cw as f32 * SIDE_SCALE, ch as f32 * SIDE_SCALE);
        let cx = self.canvas().w / 2.0 + side * SIDE_PITCH;
        [cx - w / 2.0, slot_ui::foot_y(self.canvas(), ch as f32) - h, w, h]
    }

    fn leaving_ms(l: &Leaving) -> Millis {
        use crate::editing::*;
        match l {
            Leaving::Added { .. } => KEYS_OUT_MS + ADD_SLOT_MS + ADD_PAUSE_MS + HOME_MS + NEIGHBORS_IN_MS,
            Leaving::Backed { .. } => BACK_HOME_MS + NEIGHBORS_IN_MS,
            Leaving::Removed { .. } => REMOVE_RISE_MS + FAN_MS,
            Leaving::Kept { .. } => FAN_MS,
            Leaving::LastRemoved { .. } => REMOVE_RISE_MS + LAST_RISE_MS,
        }
    }

    fn draw_snap(
        &self,
        snap: &crate::editing::Edit,
        fade: f32,
        swap: Option<&(usize, Vec<Recent>)>,
        fresh: Option<(&str, f32)>,
        out: &mut Vec<Draw>,
    ) {
        if let crate::editing::Edit::Choosing { choices, at, .. } = snap {
            self.draw_choices_as(choices, *at, fade, swap, fresh, out);
        }
    }

    fn fresh_name(&self, snap: &crate::editing::Edit, i: usize) -> Option<&str> {
        match snap {
            crate::editing::Edit::Choosing { choices, at, .. } => match choices.get(*at) {
                Some(crate::editing::Choice::New) => self.collections.get(i).map(|c| c.name.as_str()),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn exit_stack(&self) -> Option<Vec<String>> {
        match &self.leaving {
            Some((Leaving::Added { before, .. }, _)) => {
                Some(before.1.iter().map(|g| g.stem.clone()).collect())
            }
            _ => None,
        }
    }

    pub fn exit_cart(&self) -> Option<String> {
        match &self.leaving {
            Some((Leaving::Added { multi: true, .. }, _)) => None,
            Some((Leaving::Added { game, .. } | Leaving::Backed { game, .. }, _)) => {
                Some(game.stem.clone())
            }
            _ => None,
        }
    }

    fn draw_leaving(&self, out: &mut Vec<Draw>) -> bool {
        use crate::editing::*;
        let Some((leaving, start)) = &self.leaving else {
            return false;
        };
        let now = self.now();
        let t = now.saturating_sub(*start);
        let mid = self.canvas().w / 2.0;
        match leaving {
            Leaving::Added {
                snap,
                game,
                stack_x,
                multi,
                before,
            } => {
                let fresh = self.fresh_name(snap, before.0);
                let lead = match fresh {
                    Some(_) => KEYS_OUT_MS,
                    None => 0,
                };
                let start = &(*start + lead);
                let t = t.saturating_sub(lead);
                let float = self.float_rect(game, *stack_x, ADD_FLOAT_TOP + self.canvas().inset_y(), ADD_FLOAT);
                let front = match fresh {
                    Some(_) => self.fresh_rect(game, *stack_x),
                    None => self.front_rect(game, *stack_x),
                };
                let pause_end = ADD_SLOT_MS + ADD_PAUSE_MS;
                let u = ease_in_out(progress(now, *start, ADD_SLOT_MS));
                let fresh = fresh.map(|n| (n, 1.0 - u));
                if t < pause_end {
                    self.draw_dim_at(1.0, out);
                    self.draw_snap(snap, 1.0, Some(before), fresh, out);
                    match (multi, snap) {
                        (true, crate::editing::Edit::Choosing { games, .. }) => {
                            let begin = out.len();
                            self.draw_bundle_at(games, *stack_x, out);
                            let (_, base) = self.bundle_frame(games);
                            let mid_y = ((ADD_FLOAT_TOP + self.canvas().inset_y()) + base) / 2.0;
                            let drop = u * (front[1] + front[3] / 2.0 - mid_y);
                            for d in &mut out[begin..] {
                                if let Draw::Tex { y, alpha, .. } = d {
                                    *y += drop;
                                    *alpha *= 1.0 - u.powi(3);
                                }
                            }
                        }
                        _ => self.draw_game_at(game, Self::lerp_rect(float, front, u), 1.0, 0.0, out),
                    }
                    if fresh.is_some() && now < *start {
                        let keys = 1.0 - ease_out(progress(now, *start - lead, lead));
                        let begin = out.len();
                        slot_ui::draw_menu_ground(self.canvas(), self.menu_ground(), out);
                        for d in &mut out[begin..] {
                            match d {
                                Draw::Part { alpha, .. } | Draw::Tex { alpha, .. } => *alpha *= keys,
                                Draw::Rect { colour, .. } => colour[3] *= keys,
                                _ => {}
                            }
                        }
                    }
                    return true;
                }
                let begin = *start + pause_end;
                let o = ease_out(progress(now, begin, OUT_MS));
                let c = ease_in_out(progress(now, begin, HOME_MS));
                self.draw_dim_at(1.0 - c, out);
                self.draw_snap(snap, 1.0 - o, Some(before), fresh, out);
                if *multi {
                    let n = ease_out(progress(now, begin, HOME_MS));
                    self.draw_parted(None, (1.0 - n) * PART_PUSH, 1.0, out);
                } else {
                    self.draw_neighbours_in(game, begin + HOME_MS - NEIGHBORS_LEAD_MS, out);
                    self.draw_game_at(game, Self::lerp_rect(front, self.home_rect(game), c), 1.0, 0.0, out);
                }
            }
            Leaving::Backed { snap, game, float_x } => {
                let o = ease_out(progress(now, *start, OUT_MS));
                let c = ease_in_out(progress(now, *start, BACK_HOME_MS));
                self.draw_dim_at(1.0 - c, out);
                self.draw_snap(snap, 1.0 - o, None, None, out);
                self.draw_neighbours_in(game, *start + BACK_HOME_MS - NEIGHBORS_LEAD_MS, out);
                let float = self.float_rect(game, *float_x, ADD_FLOAT_TOP + self.canvas().inset_y(), ADD_FLOAT);
                self.draw_game_at(game, Self::lerp_rect(float, self.home_rect(game), c), 1.0, 0.0, out);
            }
            Leaving::Removed { game, around } => {
                let g = progress(now, *start, REMOVE_RISE_MS);
                let f = ease_in_out(progress(now, *start + REMOVE_RISE_MS - 40, FAN_MS));
                self.draw_dim_at(1.0 - f, out);
                self.draw_fan(around, mid, REMOVE_BASE, f, &[0.0, -1.0, 1.0], out);
                let mut float = self.float_rect(game, mid, REMOVE_FLOAT_TOP + self.canvas().inset_y(), REMOVE_FLOAT);
                float[1] -= 150.0 * g * g;
                self.draw_game_at(game, float, 1.0 - ease_out(g), 0.0, out);
            }
            Leaving::Kept { game, around, last } => {
                let c = ease_in_out(progress(now, *start, FAN_MS));
                self.draw_dim_at(1.0 - c, out);
                self.draw_fan(around, mid, REMOVE_BASE, c, &[1.0, -1.0, f32::NAN], out);
                let home = self.home_rect(game);
                let from = match last {
                    true => {
                        let (w, h) = (home[2] * LAST_SCALE, home[3] * LAST_SCALE);
                        [mid - w / 2.0, home[1] + (home[3] - h) / 2.0 + LAST_DROP, w, h]
                    }
                    false => self.float_rect(game, mid, REMOVE_FLOAT_TOP + self.canvas().inset_y(), REMOVE_FLOAT),
                };
                self.draw_game_at(game, Self::lerp_rect(from, home, c), 1.0, 0.0, out);
            }
            Leaving::LastRemoved { game } => {
                let g = progress(now, *start, REMOVE_RISE_MS);
                let c = ease_in_out(progress(now, *start + REMOVE_RISE_MS - 80, LAST_RISE_MS));
                self.draw_dim_at(1.0 - c, out);
                self.draw_parted(None, 0.0, c, out);
                let home = self.home_rect(game);
                let (w, h) = (home[2] * LAST_SCALE, home[3] * LAST_SCALE);
                let rect = [
                    mid - w / 2.0,
                    home[1] + (home[3] - h) / 2.0 + LAST_DROP - 470.0 * g * g,
                    w,
                    h,
                ];
                self.draw_game_at(game, rect, 1.0 - ease_out(g), 0.0, out);
            }
        }
        true
    }

    fn draw_neighbours_in(&self, game: &Recent, from: Millis, out: &mut Vec<Draw>) {
        let now = self.now();
        if now < from {
            return;
        }
        let n = crate::editing::ease_out(crate::editing::progress(now, from, crate::editing::NEIGHBORS_IN_MS));
        self.draw_parted(Some(game.stem.as_str()), (1.0 - n) * PART_PUSH, 1.0, out);
    }

    fn draw_fan(&self, around: &[Recent], cx: f32, base: f32, f: f32, sides: &[f32], out: &mut Vec<Draw>) {
        let rects = self.stack_rects(around, cx, base, STACK_PICKED);
        for (k, (g, (rect, tilt))) in around.iter().zip(rects).enumerate().rev() {
            let side = sides.get(k).copied().unwrap_or(f32::NAN);
            if side.is_nan() {
                self.draw_game_at(g, rect, crate::editing::fan_fade(f), tilt, out);
                continue;
            }
            let to = match side == 0.0 {
                true => self.home_rect(g),
                false => self.side_rect(g, side),
            };
            self.draw_game_at(g, Self::lerp_rect(rect, to, f), 1.0, tilt * (1.0 - f), out);
        }
    }

    fn draw_new_stack(&self, cx: f32, base: f32, scale: f32, alpha: f32, out: &mut Vec<Draw>) {
        let s = scale / STACK_PICKED;
        let (w, h) = (NEW_W * s, NEW_H * s);
        let (x, y) = (cx - w / 2.0, base - h);
        let ink = [1.0, 1.0, 1.0, 0.66 * alpha];
        out.push(Draw::Rect {
            x,
            y,
            w,
            h,
            colour: [1.0, 1.0, 1.0, 0.06 * alpha],
        });
        let t = 3.0;
        let dash = 12.0 * s;
        let gap = 9.0 * s;
        let mut d = 0.0;
        while d < w {
            let len = dash.min(w - d);
            for yy in [y, y + h - t] {
                out.push(Draw::Rect { x: x + d, y: yy, w: len, h: t, colour: ink });
            }
            d += dash + gap;
        }
        let mut d = 0.0;
        while d < h {
            let len = dash.min(h - d);
            for xx in [x, x + w - t] {
                out.push(Draw::Rect { x: xx, y: y + d, w: t, h: len, colour: ink });
            }
            d += dash + gap;
        }
        let arm = 22.0 * s;
        out.push(Draw::Rect { x: cx - arm / 2.0, y: y + h / 2.0 - 2.0, w: arm, h: 4.0, colour: ink });
        out.push(Draw::Rect { x: cx - 2.0, y: y + h / 2.0 - arm / 2.0, w: 4.0, h: arm, colour: ink });
    }

    fn draw_name(&self, name: &str, picked: bool, cx: f32, y: f32, alpha: f32, out: &mut Vec<Draw>) {
        let Some(&(_, faces)) = self.name_faces.iter().find(|(n, _)| n == name) else {
            return;
        };
        let (tex, w, h) = faces[picked as usize];
        out.push(Draw::Tex {
            x: (cx - w as f32 / 2.0).round(),
            y,
            w: w as f32,
            h: h as f32,
            tex,
            alpha,
        });
    }

    fn draw_heading(&self, out: &mut Vec<Draw>) {
        let Some((tex, w, h)) = self.heading_face else {
            return;
        };
        out.push(Draw::Tex {
            x: (self.canvas().w / 2.0 - w as f32 / 2.0).round(),
            y: (HEADING_Y + self.canvas().inset_y()),
            w: w as f32,
            h: h as f32,
            tex,
            alpha: self.edit_fade(),
        });
    }

    pub fn edit_heading(&self) -> Option<(String, bool)> {
        use crate::editing::Edit;
        match self.edit.as_ref()? {
            Edit::Selecting { .. } => None,
            Edit::Choosing { .. } => Some(("Add to which collection?".to_string(), false)),
            Edit::Removing { collection, last, .. } => {
                let name = &self.collections[*collection].name;
                Some(match last {
                    true => (format!("Remove last game from {name}?"), true),
                    false => (format!("Remove from {name}?"), false),
                })
            }
            Edit::MovingGame { .. } | Edit::MovingCollection { .. } => {
                self.moving().map(|m| (m, true))
            }
        }
    }

    pub fn set_heading_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.heading_face = face;
    }

    pub fn set_name_faces(&mut self, faces: Vec<(String, [(TexId, u32, u32); 2])>) {
        self.name_faces = faces;
    }

    pub fn name_face_names(&self) -> Vec<String> {
        self.collections
            .iter()
            .map(|c| c.name.clone())
            .chain(std::iter::once("New Collection".to_string()))
            .collect()
    }

    fn bundle_seat(&self, g: &Recent) -> Option<(usize, usize)> {
        let here = self
            .shelf()
            .carts
            .iter()
            .position(|c| c.platform == g.platform && c.stem == g.stem);
        if let Some(i) = here {
            return Some((self.shelf_at, i));
        }
        let at = self
            .shelves
            .iter()
            .position(|(r, _)| *r == Rack::Of(g.platform))?;
        let i = self.shelves[at]
            .1
            .carts
            .iter()
            .position(|c| c.stem == g.stem)?;
        Some((at, i))
    }

    fn bundle_seats(&self) -> Vec<(usize, usize)> {
        let mut games = self.edit.as_ref().map_or(Vec::new(), |e| self.edit_games(e));
        match self.leaving.as_ref().map(|(l, _)| l) {
            Some(Leaving::Added { snap, game, before, .. }) => {
                games.extend(self.edit_games(snap));
                games.push(game.clone());
                games.extend(before.1.iter().cloned());
            }
            Some(Leaving::Backed { snap, game, .. }) => {
                games.extend(self.edit_games(snap));
                games.push(game.clone());
            }
            Some(Leaving::Removed { game, around }) | Some(Leaving::Kept { game, around, .. }) => {
                games.push(game.clone());
                games.extend(around.iter().cloned());
            }
            Some(Leaving::LastRemoved { game }) => games.push(game.clone()),
            None => {}
        }
        let mut seats: Vec<(usize, usize)> = games.iter().filter_map(|g| self.bundle_seat(g)).collect();
        seats.sort();
        seats.dedup();
        seats
    }

    fn edit_games(&self, edit: &crate::editing::Edit) -> Vec<Recent> {
        use crate::editing::{Choice, Edit};
        let mut games: Vec<Recent> = Vec::new();
        match edit {
            Edit::Choosing { games: carried, choices, .. } => {
                games.extend(carried.iter().take(BUNDLE_MAX).cloned());
                for c in choices {
                    if let Choice::Collection(i) = c {
                        games.extend(self.stack_of(*i));
                    }
                }
            }
            Edit::Removing { game, .. } => {
                games.push(game.clone());
                games.extend(crate::editing::remove_stack(
                    &self.shelf_games(self.shelf_at),
                    self.shelf().index,
                ));
            }
            Edit::MovingCollection { row, .. } => {
                for name in row {
                    if let Some(i) = self.collections.iter().position(|c| c.name == *name) {
                        games.extend(self.stack_of(i));
                    }
                }
            }
            _ => {}
        }
        games
    }

    fn bundle_frame(&self, checked: &[Recent]) -> (f32, f32) {
        let shown = &checked[..checked.len().min(BUNDLE_MAX)];
        let room = slot_ui::cart_box(Platform::Gba).1 as f32 * BUNDLE_SCALE;
        let tallest = shown
            .iter()
            .map(|g| slot_ui::cart_box(g.platform).1 as f32)
            .fold(0.0, f32::max);
        let scale = match tallest > 0.0 {
            true => BUNDLE_SCALE.min(room / tallest),
            false => BUNDLE_SCALE,
        };
        (scale, (ADD_FLOAT_TOP + self.canvas().inset_y()) + tallest * scale)
    }

    fn draw_bundle_at(&self, checked: &[Recent], mid: f32, out: &mut Vec<Draw>) {
        let shown = &checked[..checked.len().min(BUNDLE_MAX)];
        let n = shown.len();
        let (scale, base) = self.bundle_frame(checked);
        for (k, g) in shown.iter().enumerate() {
            let Some((at, i)) = self.bundle_seat(g) else {
                continue;
            };
            let shelf = &self.shelves[at].1;
            let (cw, ch) = slot_ui::cart_box(g.platform);
            let (w, h) = (cw as f32 * scale, ch as f32 * scale);
            let cx = mid + (k as f32 - (n as f32 - 1.0) / 2.0) * BUNDLE_SPREAD;
            shelf.draw_cart_at(i, [cx - w / 2.0, base - h, w, h], out);
        }
        if checked.len() > 1 {
            if let Some(((tex, w, h), _)) = self.count_badge {
                let x = mid + (n as f32 - 1.0) / 2.0 * BUNDLE_SPREAD + BUNDLE_BADGE_DX;
                out.push(Draw::Tex {
                    x: (x - w / 2.0).round(),
                    y: ((ADD_FLOAT_TOP + self.canvas().inset_y()) - BUNDLE_BADGE_LIFT).round(),
                    w,
                    h,
                    tex,
                    alpha: 1.0,
                });
            }
        }
    }

    fn draw_picking_legends(&self, out: &mut Vec<Draw>) {
        use crate::editing::Edit;
        let (Some(edit), Some(faces)) = (&self.edit, self.picking_faces) else {
            return;
        };
        let ink = self.light() as usize;
        let [cancel, select, unselect, back, add, keep, remove, drop] = faces[ink];
        match edit {
            Edit::Selecting { checked, .. } => {
                let shelf = self.shelf();
                let check = match shelf.carts.get(shelf.index) {
                    Some(c) if edit.is_checked(c.platform, &c.stem) => unselect,
                    _ => select,
                };
                let Some((add, _)) = self.add_faces else {
                    return;
                };
                let dim = match checked.is_empty() {
                    true => PICK_DIM_ALPHA,
                    false => 1.0,
                };
                self.draw_spread_legends_each(
                    &[cancel, check, add[ink]],
                    self.legend_y(),
                    &[1.0, 1.0, dim],
                    out,
                );
            }
            Edit::Choosing { .. } => {
                self.draw_spread_legends(&[back, add], self.legend_y(), self.edit_fade(), out)
            }
            Edit::Removing { .. } => {
                self.draw_spread_legends(&[keep, remove], self.legend_y(), self.edit_fade(), out)
            }
            Edit::MovingGame { .. } | Edit::MovingCollection { .. } => {
                self.draw_spread_legends(&[cancel, drop], self.legend_y(), 1.0, out)
            }
        }
    }

    pub fn picked_count(&self) -> Option<usize> {
        match self.edit.as_ref()? {
            crate::editing::Edit::Selecting { checked, .. } => Some(checked.len()),
            crate::editing::Edit::Choosing { games, .. } => Some(games.len()),
            _ => None,
        }
    }

    pub fn set_picking_faces(&mut self, faces: [[(TexId, u32); 8]; 2]) {
        self.picking_faces = Some(faces);
    }

    pub fn set_add_faces(&mut self, faces: [(TexId, u32); 2], n: usize) {
        self.add_faces = Some((faces, n));
    }

    pub fn add_faces_count(&self) -> Option<usize> {
        self.add_faces.map(|(_, n)| n)
    }


    pub fn set_count_badge(&mut self, face: (TexId, u32, u32), n: usize) {
        self.count_badge = Some(((face.0, face.1 as f32, face.2 as f32), n));
    }

    pub fn count_badge_count(&self) -> Option<usize> {
        self.count_badge.map(|(_, n)| n)
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
            slot: self.slot_faces_for(stem),
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
        self.core_faces_for = self.selected_cart().map(|c| (c.platform, c.stem.clone()));
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

    pub fn set_core_gb_part_faces(
        &mut self,
        sockets: Vec<TexId>,
        chips: Vec<TexId>,
        blank: TexId,
    ) {
        self.core_gb_socket_faces = sockets;
        self.core_gb_chip_faces = chips;
        self.core_gb_blank_chip_face = Some(blank);
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

    fn draw_lid_shadow(
        &self,
        shell: Option<GbShell>,
        shelf: Placed,
        progress: f32,
        out: &mut Vec<Draw>,
    ) {
        let Some(tex) = self.core_chip_shadow_face else {
            return;
        };
        let lift = lift_of(progress);
        let (lid, _) = lid_from_shaped(self.canvas(), shell, shelf, progress);
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
        seats: Seats,
        pair: [Core; 2],
        sockets: &[TexId],
        chip: Chip,
        faces: &[TexId],
        blank: Option<TexId>,
        lift: f32,
        out: &mut Vec<Draw>,
    ) {
        let zoom = seats.zoom(board);
        for (place, core) in pair.into_iter().enumerate() {
            let Some(tex) = sockets.get(core.index()).copied() else {
                continue;
            };
            let (x, y) = seats.at(board, seats.socket_u[place], seats.socket_v);
            out.push(Draw::Tex {
                x: x.round(),
                y: y.round(),
                w: seats.socket_size.0 * zoom,
                h: seats.socket_size.1 * zoom,
                tex,
                alpha: 1.0,
            });
        }
        let u = seats.chip_u[0] + (seats.chip_u[1] - seats.chip_u[0]) * chip.across;
        if chip.lift > 0.0 {
            if let Some(tex) = self.core_chip_shadow_face {
                let (du, dv) = seats.shadow_uv;
                let (cx, cy) = seats.at(board, u + du, seats.chip_v + dv);
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
            let (x, y) = seats.at(board, u, seats.chip_v - HOP_LIFT * chip.lift);
            let body = Placed {
                x: x + chip.shake,
                y,
                w: seats.chip_size.0 * zoom,
                h: seats.chip_size.1 * zoom,
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
        let shell = self.shelf().selected_shell();
        let shelf = shelf_cart_shaped(self.canvas(), shell, rest, scale);
        let board = board_from_shaped(self.canvas(), shell, shelf, progress);
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
            let (sockets, chips, blank) = match shell {
                Some(_) => (
                    &self.core_gb_socket_faces,
                    &self.core_gb_chip_faces,
                    self.core_gb_blank_chip_face,
                ),
                None => (
                    &self.core_socket_faces,
                    &self.core_chip_faces,
                    self.core_blank_chip_face,
                ),
            };
            self.draw_board_chip(
                board,
                seats(shell),
                picker.pair(),
                sockets,
                picker.chip(now),
                chips,
                blank,
                lift,
                out,
            );
        }

        self.draw_lid_shadow(shell, shelf, progress, out);

        if ready {
            if let Some(tex) = self.core_lid_face {
                let (lid, turn) = lid_from_shaped(self.canvas(), shell, shelf, progress);
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
            let (lid, turn) = lid_from_shaped(self.canvas(), shell, shelf, progress);
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
            self.draw_spread_legends(legends, self.legend_y(), lift, out);
        }
    }

    fn draw_game_menu(&self, menu: GameMenu, out: &mut Vec<Draw>) {
        let canvas = self.canvas();
        if let Some(tex) = self.link_shot {
            out.push(Draw::Tex {
                x: 0.0,
                y: 0.0,
                w: canvas.w,
                h: self.canvas().h,
                tex,
                alpha: 1.0,
            });
        }
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: self.canvas().h,
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
                y: self.legend_y(),
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
                y: self.legend_y(),
                w: w as f32,
                h: MOUTH_H,
                tex,
                alpha: 1.0,
            });
            x += seen(w) + LINK_LEGEND_GAP;
        }
    }

    fn auto_link(&self, stem: &str) -> Option<(&Cart, LinkKind)> {
        let cart = self
            .shelf()
            .carts
            .iter()
            .find(|c| c.stem == stem && self.seated_platform.is_none_or(|p| c.platform == p))?;
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
        self.settle_saves();
        let at = self.shelf().index;
        let Some((cart, platform)) = self.shelf_mut().carts.get_mut(at).map(|c| {
            c.read_header();
            (c.stem.clone(), c.platform)
        }) else {
            return;
        };
        self.seated_platform = Some(platform);
        eprintln!(
            "slot: launch: insert {cart} at {:.1} ms",
            crate::boot_time::ms()
        );
        self.play_held = None;
        self.refusal = None;
        self.refused_from = None;
        self.sunlight_manual = None;
        self.stop_romm();
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
        if self.now().saturating_sub(at) >= PLAY_HOLD_MS && !self.unwrap_seated() {
            self.insert(!self.kid_locked() || !self.state.resume_insert);
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
        let began = std::time::Instant::now();
        let (state, sav) = match snapshot.state() {
            Some(state) => trusted_write(snapshot.as_ref(), state, "eject"),
            None => {
                eprintln!("slot: eject: the core gave up no state");
                (None, None)
            }
        };
        let got = began.elapsed();
        self.state_writer.flush();
        let flushed = began.elapsed();
        let (root, platform, core, stem, slot) = (
            root.clone(),
            self.platform,
            self.core,
            stem.to_owned(),
            self.state.clone(),
        );
        let card_writes = self.card_writes.clone();
        let write = move || {
            let written = persist::eject(
                &root,
                platform,
                core,
                &stem,
                state.as_deref(),
                sav.as_deref(),
                &slot,
            );
            eprintln!(
                "slot: eject: state {} bytes in {:.0} ms, writer flushed at {:.0} ms, written at {:.0} ms",
                state.as_ref().map_or(0, |s| s.len()),
                got.as_secs_f64() * 1e3,
                flushed.as_secs_f64() * 1e3,
                began.elapsed().as_secs_f64() * 1e3
            );
            if let Err(e) = &written {
                eprintln!("slot: eject: {e}");
            }
            card_writes.wrote("eject", &written);
            written.is_ok()
        };
        match std::thread::Builder::new().name("slot-eject".into()).spawn(write) {
            Ok(h) => self.eject_write = Some(h),
            Err(e) => eprintln!("slot: eject: no writer thread: {e}"),
        }
    }

    fn take_eject_write(&mut self, wait: bool) {
        let done = self.eject_write.as_ref().is_some_and(|h| wait || h.is_finished());
        if !done {
            return;
        }
        if let Some(h) = self.eject_write.take() {
            if h.join().unwrap_or(false) {
                self.state.cart = None;
                self.state.cart_platform = None;
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
        self.stop_romm();
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
        let seat =
            slot_store::core_for_platform(&root, &cart.stem, cart.platform, self.state.gb_core);
        let now = self.now();
        let mut picker = CorePicker::open(Core::pair(cart.platform), seat, now);
        if self.core_faces_ready() {
            picker.start(now);
        }
        self.core_picker = Some(picker);
        self.shelf_mut().release_hold();
        self.play_held = None;
    }

    fn core_faces_ready(&self) -> bool {
        let (Some((platform, stem)), Some(cart)) = (&self.core_faces_for, self.selected_cart()) else {
            return false;
        };
        cart.platform == *platform && cart.stem == *stem
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
        if self.core == Core::Gambatte {
            self.hud.toast(Toast::NeedsMgba, self.now());
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
            self.studio_from = crate::studio_view::shelf_rect(self.canvas(), &cart);
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
            if row.page() == QuickRow::ROMM && self.update.is_none() && !self.wifi_up() {
                self.keyboard = None;
                self.phase = Phase::QuickMenu {
                    row: QuickRow::Romm,
                };
                self.set_update(Some(UpdateState::Notice(NetNotice::NoNetwork)));
            }
        }
        if matches!(self.phase, Phase::RommPair) && self.update.is_none() && !self.wifi_up() {
            self.romm_jobs.ask(RommJob::CancelPair);
            self.phase = Phase::QuickMenu {
                row: QuickRow::Romm,
            };
            self.set_update(Some(UpdateState::Notice(NetNotice::NoNetwork)));
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
                | Phase::RommPair
                | Phase::Unwrap { .. }
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

    pub fn gb_core(&self) -> Core {
        self.state.gb_core
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
        [l.cart_x, l.cart_y + self.canvas().inset_y(), l.cart_w, l.cart_h]
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
        let since = self.studio_closing.unwrap_or(self.studio_opened);
        let t = (self.now().saturating_sub(since) as f32 / STUDIO_SLIDE_MS).min(1.0);
        if self.studio_closing.is_some() || t < 1.0 {
            let e = if self.studio_closing.is_some() { 1.0 - ease(t) } else { ease(t) };
            self.draw_background(out);
            self.shelf().draw_row(canvas, Some(&studio.cart.stem), 0.0, 0.0, 1.0, out);
            self.draw_studio_ground((e - 1.0) * self.canvas().h, out);
            self.draw_studio_cart(out);
            return draw_empty_slot(canvas, self.slot_faces, out);
        }
        self.draw_studio_ground(0.0, out);
        let shown = view(studio, canvas.w);
        for item in &shown.items {
            let start = out.len();
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
            if !matches!(item, Item::Cart) {
                shift_down(&mut out[start..], canvas.inset_y());
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
                    self.draw_side_legend(*face, i == 1, self.legend_y(), out);
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
            out.push(Draw::Tex { x: x.floor(), y: self.legend_y(), w: w as f32, h: MOUTH_H, tex, alpha: 1.0 });
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
        let spot = [t.x + SLIP_PAD, TRAY_TOP + self.canvas().inset_y() + scene.used_y + SLIP_PAD];
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
        let h = self.canvas().h;
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

    pub fn set_romm_jobs(&mut self, jobs: Box<dyn RommJobs>) {
        self.romm_jobs = jobs;
        if self.romm.paired() {
            self.romm_jobs.ask(RommJob::Start(self.romm.clone()));
        }
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

    #[cfg(feature = "nds")]
    fn open_fetch_box_art(&mut self) {
        if self.link_active() {
            return;
        }
        self.art_pending += 1;
        self.hold_wifi();
        self.art_jobs.ask(ArtJob::FetchBoxArt);
        self.fetch_art = Some(FetchArt::Running(ArtRun::boxes()));
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
            Some(FetchArt::Running(run)) if run.boxes => match (run.stopping, run.counted) {
                (true, _) => vec!["Stopping".into()],
                (false, true) => vec![
                    "Fetching Box Art".into(),
                    format!("{} of {}", run.landed + run.missed.len(), run.total),
                ],
                (false, false) if !self.wifi_up() => vec!["Connecting to Wi-Fi".into()],
                (false, false) => vec!["Fetching Box Art".into()],
            },
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
        let room = self.canvas().h - MOUTH_H;
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
                    None if run.boxes && run.counted && !run.stopping => {
                        let done = run.landed + run.missed.len();
                        self.draw_fetch_progress(done, run.total, out);
                    }
                    None if (run.skipping || run.closing.is_some()) && !run.stopping => {
                        let done = run.landed + run.missed.len();
                        self.draw_fetch_progress(done, run.total, out);
                    }
                    Some(scene) => {
                        self.draw_fetch_scene(scene, out);
                        self.draw_fetch_text(0, mid, 26.0, out);
                        self.draw_fetch_text(1, mid, slot_ui::slot_name_y(self.canvas()) - 30.0, out);
                        self.draw_fetch_text(2, mid, slot_ui::slot_name_y(self.canvas()), out);
                    }
                    None => {
                        let lines = self.fetch_text.len().min(2) as f32;
                        let top = ((self.canvas().h - MOUTH_H - 40.0 * lines) / 2.0).floor();
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
            self.draw_side_legend(legends[legend], false, self.legend_y(), out);
            if legend == 0 {
                self.draw_side_legend(legends[1], true, self.legend_y(), out);
            }
            if self.fetch_can_skip() {
                self.draw_side_legend(legends[4], true, self.legend_y(), out);
            }
        }
    }

    fn draw_fetch_summary(&self, featured: Option<&ArtLanded>, out: &mut Vec<Draw>) {
        const MARGIN: f32 = 44.0;
        const COLUMN: f32 = 288.0;
        const ROW: f32 = 40.0;
        let ground = self.canvas().h - MOUTH_H;
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
            #[cfg(feature = "nds")]
            Platform::Nds => (
                1.0,
                [slot_ui::DS_LABEL_X, slot_ui::DS_LABEL_Y, slot_ui::DS_LABEL_W, slot_ui::DS_LABEL_H],
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
        let y = ((self.canvas().h - MOUTH_H - h) / 2.0).floor();
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
        match (action, &mut self.server) {
            (Action::GbaDown(Btn::B), Some(server)) => server.stop(now),
            (Action::GbaDown(Btn::A), Some(server)) => server.skip(now),
            _ => {}
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
        if let Some(root) = &self.root {
            self.gifts = crate::romm::Gifts::load(root);
        }
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
                self.restock_all_games();
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
        let collections = slot_store::read_collections(root);
        self.install_collections(collections);
        self.restock_collections();
    }

    fn adopt_rescan(&mut self, carts: Vec<Cart>) {
        retain_headers(&carts);
        if let Some(root) = self.root.clone() {
            self.gifts.retain_games(&root, &carts);
            self.collection_pngs = crate::wallpaper::collection_list(&root);
        }
        let seats: Vec<(usize, Option<(Platform, String)>)> = self
            .shelves
            .iter()
            .map(|(_, shelf)| {
                let cart = shelf.carts.get(shelf.index);
                (shelf.index, cart.map(|c| (c.platform, c.stem.clone())))
            })
            .collect();
        for (at, (_, fresh)) in shelves_of(carts, self.collections.len())
            .into_iter()
            .enumerate()
        {
            let old = self.shelves[at].1.restock(fresh.carts);
            self.orphaned.extend(old);
        }
        self.restock_recents();
        self.restock_favorites();
        self.restock_all_games();
        self.restock_collections();
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
            self.shelf_at = self.fallback_shelf();
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
            self.draw_app(self.menu_on_bottom(), out);
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
                slot_ui::Layout::General,
                "",
            ),
            NetRow::Nearby(n) => self.join_network(n.ssid, String::new(), false),
            NetRow::Other => self.open_keyboard(
                NAME_TITLE,
                None,
                KeyboardFor::NetworkName,
                slot_ui::Layout::General,
                "",
            ),
        }
    }

    fn open_keyboard(
        &mut self,
        title: &str,
        subtitle: Option<&str>,
        task: KeyboardFor,
        layout: slot_ui::Layout,
        text: &str,
    ) {
        let kb = slot_ui::Keyboard::new(layout, title, subtitle, text);
        self.keyboard = Some((kb, task));
    }

    fn save_romm(&self) {
        if let Some(root) = &self.root {
            let _ = self.romm.save(root);
        }
    }

    fn pair_romm(&mut self) {
        if self.romm.server.trim().is_empty() {
            self.set_update(Some(UpdateState::Notice(NetNotice::RommNoServer)));
            return;
        }
        self.start_pairing();
    }

    fn start_pairing(&mut self) {
        if let Some(root) = &self.root {
            if self.romm.save(root).is_ok() {
                self.romm = crate::romm::RommConf::load(root);
            }
        }
        self.pair_screen = PairScreen::Starting;
        self.phase = Phase::RommPair;
        self.romm_jobs.ask(RommJob::Pair(self.romm.clone()));
    }

    fn leave_pairing(&mut self) {
        self.romm_jobs.ask(RommJob::CancelPair);
        self.phase = Phase::QuickMenu {
            row: QuickRow::RommPair,
        };
    }

    fn pair_retryable(&self) -> bool {
        matches!(
            self.pair_screen,
            PairScreen::Failed(PairFail::Offline | PairFail::Denied | PairFail::Expired)
        )
    }

    fn pair_input(&mut self, action: Action) {
        match action {
            Action::GbaDown(Btn::B) | Action::QuickMenu => self.leave_pairing(),
            Action::GbaDown(Btn::A) if self.pair_retryable() => self.start_pairing(),
            _ => {}
        }
    }

    fn romm_busy(&self) -> bool {
        self.romm.paired()
    }

    fn stop_romm(&mut self) {
        self.romm_open = false;
        if self.romm_busy() {
            self.romm_jobs.ask(RommJob::Stop);
        }
    }

    fn romm_quiet(&self) -> bool {
        self.seated_cart().is_none() && !matches!(self.phase, Phase::Doze { .. })
    }

    fn romm_tick(&mut self, now: Millis) {
        if !self.romm_quiet() {
            if self.romm_open {
                self.stop_romm();
            }
            return;
        }
        for (platform, stem) in std::mem::take(&mut self.romm_gifts) {
            self.gift_arrived(platform, &stem);
        }
        if !matches!(self.phase, Phase::Unwrap { .. }) {
            if let Some(carts) = self.romm_carts.take() {
                self.adopt_rescan(carts);
            }
        }
        if !self.romm.paired() || !self.wifi_up() {
            if self.romm_open {
                self.stop_romm();
            }
            return;
        }
        if !self.romm_open {
            self.romm_open = true;
            self.romm_poll_at = now;
        }
        if now >= self.romm_poll_at {
            self.romm_poll_at = now + ROMM_POLL_MS;
            self.romm_jobs.ask(RommJob::Claim);
        }
    }

    fn gift_arrived(&mut self, platform: Platform, stem: &str) {
        if let Some(root) = self.root.clone() {
            self.gifts.add(&root, platform, stem);
        }
    }

    fn seated_gift(&self) -> Option<(Platform, String)> {
        let shelf = self.shelf();
        let cart = shelf.carts.get(shelf.index)?;
        self.gifts
            .holds(cart.platform, &cart.stem)
            .then(|| (cart.platform, cart.stem.clone()))
    }

    fn unwrap_seated(&mut self) -> bool {
        if !self.on_shelf() {
            return false;
        }
        let Some((_, stem)) = self.seated_gift() else {
            return false;
        };
        self.play_held = None;
        self.unwrap_scene = self.unwrap_tex.map(|_| UnwrapScene::new(self.now()));
        self.phase = Phase::Unwrap { stem };
        true
    }

    fn draw_unwrap(&self, out: &mut Vec<Draw>) -> bool {
        let (Phase::Unwrap { stem }, Some(scene), Some(tex)) =
            (&self.phase, &self.unwrap_scene, &self.unwrap_tex)
        else {
            return false;
        };
        let shelf = self.shelf();
        let Some(cart) = shelf.carts.get(shelf.index) else {
            return false;
        };
        let canvas = self.canvas();
        shelf.draw_row(canvas, Some(stem), 0.0, 0.0, 1.0, out);
        unwrap_draws(
            &scene.frame(self.now()),
            tex,
            (cart.platform, stem),
            |rect, out| shelf.draw_cart_at(shelf.index, rect, out),
            canvas,
            out,
        );
        draw_empty_slot(canvas, self.slot_faces, out);
        true
    }

    pub fn set_unwrap_tex(&mut self, tex: UnwrapTextures) {
        self.unwrap_tex = Some(tex);
    }

    pub fn has_gifts(&self) -> bool {
        !self.gifts.is_empty()
    }

    pub fn gift_held(&self, platform: Platform, stem: &str) -> bool {
        self.gifts.holds(platform, stem)
    }

    pub fn gift_wants(&self) -> Vec<(usize, (Platform, String))> {
        let shelf = self.shelf();
        shelf
            .around(GIFT_REACH)
            .into_iter()
            .filter(|i| shelf.wrapped(*i).is_none())
            .filter_map(|i| {
                let cart = shelf.carts.get(i)?;
                self.gifts
                    .holds(cart.platform, &cart.stem)
                    .then(|| (i, (cart.platform, cart.stem.clone())))
            })
            .collect()
    }

    pub fn set_gift_box(&mut self, i: usize, tex: TexId) {
        self.shelf_mut().set_wrapped(i, Some(tex));
    }

    fn finish_unwrap(&mut self, stem: &str) {
        let platform = self
            .shelf()
            .carts
            .get(self.shelf().index)
            .filter(|c| c.stem == stem)
            .map(|c| c.platform);
        if let (Some(root), Some(platform)) = (self.root.clone(), platform) {
            self.gifts.remove(&root, platform, stem);
        }
        for (_, shelf) in &mut self.shelves {
            for i in 0..shelf.carts.len() {
                if shelf.carts[i].stem == stem && platform.is_none_or(|p| shelf.carts[i].platform == p) {
                    shelf.set_wrapped(i, None);
                }
            }
        }
        self.unwrap_scene = None;
        self.phase = Phase::Shelf;
    }

    fn romm_unpaired(&mut self) {
        self.romm.unpair();
        self.romm_note = None;
        self.romm_online = false;
        self.save_romm();
        if let Phase::QuickMenu { row } = self.phase {
            if row.page() == QuickRow::ROMM {
                let hidden = self.hidden_rows();
                self.phase = Phase::QuickMenu {
                    row: QuickRow::ROMM[0].shown(&hidden)[0],
                };
            }
        }
    }

    fn take_romm_events(&mut self) {
        while let Some(event) = self.romm_jobs.take() {
            match event {
                RommEvent::Paired { token, device_id } => {
                    self.romm.token = Some(token);
                    self.romm.device_id = Some(device_id);
                    self.romm_online = false;
                    self.save_romm();
                    self.romm_jobs.ask(RommJob::Start(self.romm.clone()));
                    if matches!(self.phase, Phase::RommPair) {
                        let hidden = self.hidden_rows();
                        self.phase = Phase::QuickMenu {
                            row: QuickRow::ROMM[0].shown(&hidden)[0],
                        };
                    }
                }
                RommEvent::Unpaired => self.romm_unpaired(),
                RommEvent::Installed {
                    carts,
                    platform,
                    stem,
                } => {
                    self.romm_carts = Some(carts);
                    self.romm_gifts.push((platform, stem));
                }
                RommEvent::Note(note) => self.romm_note = Some(note),
                RommEvent::NoteCleared => self.romm_note = None,
                RommEvent::Online(up) => self.romm_online = up,
                RommEvent::PairShow { url, code } if matches!(self.phase, Phase::RommPair) => {
                    self.pair_screen = PairScreen::Show { url, code };
                }
                RommEvent::PairFailed(fail) if matches!(self.phase, Phase::RommPair) => {
                    self.pair_screen = PairScreen::Failed(fail);
                }
                RommEvent::PairShow { .. } | RommEvent::PairFailed(_) => {}
            }
        }
    }

    pub fn pair_view(&self) -> Option<PairView> {
        use slot_ui::PairSlot::*;
        if !matches!(self.phase, Phase::RommPair) {
            return None;
        }
        let words = |slots: [(slot_ui::PairSlot, &str); 2]| {
            slots.into_iter().map(|(s, t)| (s, t.to_string())).collect()
        };
        Some(match &self.pair_screen {
            PairScreen::Starting => PairView {
                url: None,
                lines: vec![(Scan, "Connecting".into())],
            },
            PairScreen::Show { url, code } => PairView {
                url: Some(url.clone()),
                lines: vec![
                    (Scan, "Scan with".into()),
                    (Phone, "Your phone".into()),
                    (Enter, "Or enter".into()),
                    (Code, code.clone()),
                    (Status, "Waiting".into()),
                ],
            },
            PairScreen::Failed(fail) => PairView {
                url: None,
                lines: match fail {
                    PairFail::Offline => words([(Scan, "Can't reach"), (Phone, "the server")]),
                    PairFail::TooOld => words([(Scan, "RomM 5.4.0"), (Phone, "or later needed")]),
                    PairFail::Denied => words([(Scan, "Pairing"), (Phone, "was denied")]),
                    PairFail::Expired => words([(Scan, "Pairing"), (Phone, "expired")]),
                },
            },
        })
    }

    pub fn set_pair_faces(&mut self, faces: Option<PairFaces>) {
        self.pair_faces = faces;
    }

    fn draw_pair(&self, out: &mut Vec<Draw>) {
        let empty = [];
        let (qr, lines) = match &self.pair_faces {
            Some(f) => (f.qr, f.lines.as_slice()),
            None => (None, empty.as_slice()),
        };
        slot_ui::PairScreen {
            qr,
            lines,
            ground: self.menu_ground(),
            canvas: self.canvas(),
        }
        .draw(out);
        draw_empty_slot(self.canvas(), self.slot_faces, out);
        let Some(net) = self.net_legends.as_ref() else {
            return;
        };
        let light = self.light() as usize;
        self.draw_side_legend(net.cancel[light], false, self.legend_y(), out);
        if self.pair_retryable() {
            self.draw_side_legend(net.retry[light], true, self.legend_y(), out);
        }
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
                slot_ui::Layout::General,
                "",
            ),
            KeyboardFor::CollectionName => self.name_collection(text),
            KeyboardFor::RommServer => {
                self.romm.server = text.trim().to_string();
                self.keyboard = None;
                self.save_romm();
            }
            KeyboardFor::RommPort => {
                let port = match text.trim() {
                    "" => None,
                    t => match t.parse::<u16>() {
                        Ok(p) if p > 0 => Some(p),
                        _ => {
                            return self.set_update(Some(UpdateState::Notice(NetNotice::RommPort)))
                        }
                    },
                };
                self.romm.port = port;
                self.keyboard = None;
                self.save_romm();
            }
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
                    return self.open_keyboard(
                        crate::networks::PASSWORD_TITLE,
                        Some(&ssid),
                        task,
                        slot_ui::Layout::General,
                        "",
                    );
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
                let now = self.now();
                self.hud.toast_saying(Toast::Forgotten, &format!("{ssid} was forgotten"), now);
                self.forgotten = Some(ssid.clone());
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
        self.draw_side_legend(left, false, self.legend_y(), out);
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
            self.draw_side_legend(right, true, self.legend_y(), out);
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
            UpdateState::CheckFailed => (
                "Could not fetch update info".into(),
                "Try again later.".into(),
            ),
            UpdateState::Notice(NetNotice::Password) => ("Invalid Password".into(), String::new()),
            UpdateState::Notice(NetNotice::RommPort) => ("Invalid Port".into(), String::new()),
            UpdateState::Notice(NetNotice::NoNetwork) => (
                "Not connected".into(),
                "Join a Wi-Fi network first.".into(),
            ),
            UpdateState::Notice(NetNotice::RommNoServer) => (
                "Enter a server first".into(),
                "Set Server before pairing.".into(),
            ),
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
                        "Check Wi-Fi in the System menu.",
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
        let mut picker = CorePicker::open(Core::pair(Platform::Gba), Core::Mgba, self.now());
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
                    self.set_update(Some(match f {
                        UpdateFail::Clock | UpdateFail::NoWifi => UpdateState::Failed(f),
                        _ => UpdateState::CheckFailed,
                    }))
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
                                let at = slot_ui::shelf_cart_at(self.canvas(), x, scale);
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
                Some(UpdateState::Checking) => UPDATE_CHECK_Y + self.canvas().inset_y(),
                _ => UPDATE_TITLE_Y + self.canvas().inset_y(),
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
                self.draw_side_legend(cancel, false, self.legend_y(), out)
            }
            Some(UpdateState::Available(..)) => {
                self.draw_side_legend(cancel, false, self.legend_y(), out);
                self.draw_side_legend(install, true, self.legend_y(), out);
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
            h: self.canvas().h,
            colour: [r, g, b, alpha],
        });
        let (tex, w, h) = scene.logo;
        out.push(Draw::Tex {
            x: canvas.centred(w as f32).floor(),
            y: ((self.canvas().h as u32 - h) / 2) as f32,
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
                GBA_SEATS,
                picker.pair(),
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
        self.draw_lid_shadow(None, slot_ui::shelf_cart(canvas), open, out);
        let (lid, turn) = lid_at(canvas, open);
        self.draw_update_cart(cart, lid, turn, 1.0, out);
    }

    fn draw_update_prompt(&self, out: &mut Vec<Draw>) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: self.canvas().w,
            h: self.canvas().h,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let lines = &self.update_faces;
        let gaps = REPAIR_GAP * lines.len().saturating_sub(1) as u32;
        let total = lines.iter().map(|l| l.2).sum::<u32>() + gaps;
        let mut y = ((self.canvas().h as u32).saturating_sub(total) / 2) as f32;
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
                self.draw_side_legend(cancel, false, self.legend_y(), out);
                self.draw_side_legend(install, true, self.legend_y(), out);
            }
            Some(UpdateState::Battery(_)) => {
                self.draw_side_legend(cancel, false, self.legend_y(), out);
                self.draw_side_legend(install, true, self.legend_y(), out);
            }
            Some(
                UpdateState::UpToDate
                | UpdateState::Failed(_)
                | UpdateState::CheckFailed
                | UpdateState::Notice(_),
            ) => {
                self.draw_side_legend(back, false, self.legend_y(), out)
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
        self.stop_romm();
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
        if let Err(e) = slot_store::write_selected_core(&root, cart.platform, &cart.stem, core) {
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
        self.take_eject_write(true);
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

extern "C" {
    fn tzset();
}

fn tell_libc_the_zone(zone: slot_store::Zone) {
    std::env::set_var("TZ", zone.posix());
    unsafe { tzset() };
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

const OVERLAY_RESCAN: std::time::Duration = std::time::Duration::from_secs(1);

struct OverlayScan {
    platform: Platform,
    frame: (u32, u32),
    at: std::time::Instant,
    entries: Vec<slot_store::OverlayEntry>,
}

pub fn overlay_slot(platform: Platform) -> usize {
    match platform {
        Platform::Gba | Platform::Gbav => 0,
        Platform::Gb => 1,
        Platform::Gbc => 2,
        #[cfg(feature = "nds")]
        Platform::Nds => 3,
    }
}

fn bezel_slot(platform: Platform) -> usize {
    match platform {
        Platform::Gba | Platform::Gbav => 0,
        #[cfg(feature = "nds")]
        Platform::Nds => 0,
        Platform::Gb => 1,
        Platform::Gbc => 2,
    }
}

fn keyed(u: f32, keys: &[(f32, [f32; 3])]) -> [f32; 3] {
    if u <= keys[0].0 {
        return keys[0].1;
    }
    for pair in keys.windows(2) {
        let ((a, va), (b, vb)) = (pair[0], pair[1]);
        if u <= b {
            let k = slot_ui::ease(((u - a) / (b - a)).clamp(0.0, 1.0));
            return [0, 1, 2].map(|i| va[i] + (vb[i] - va[i]) * k);
        }
    }
    keys[keys.len() - 1].1
}

const DS_RISE_END: f32 = 0.26;
const DS_GLIDE_FROM: f32 = 0.33;
const DS_GLIDE_END: f32 = 0.52;
const DS_PUSH_FROM: f32 = 0.60;
const DS_PUSH_END: f32 = 1.0 - DS_CLICK_GAP_S / DS_INSERT_FLY_S;
const DS_RELEASE_FROM: f32 = 0.95;
const DS_LINE_UP: f32 = -180.0;
const DS_OFF_TOP: f32 = -720.0;
const DS_PRESS: f32 = 10.0;
const DS_PRESS_END: f32 = 0.25 / DS_EJECT_FLY_S;
const DS_LATCH_END: f32 = DS_PRESS_END + DS_CLICK_GAP_S / DS_EJECT_FLY_S;
const DS_SPRUNG: f32 = DS_LATCH_END + 0.3 / DS_EJECT_FLY_S;

fn ds_insert_bottom(seat: f32) -> Option<[f32; 3]> {
    (seat < DS_RISE_END).then(|| [(seat / DS_RISE_END).powf(2.2), 1.0, 1.0])
}

fn ds_insert_top(seat: f32) -> Option<[f32; 3]> {
    let span = |a: f32, b: f32| ((seat - a) / (b - a)).clamp(0.0, 1.0);
    if seat < DS_GLIDE_FROM {
        return None;
    }
    if seat < DS_GLIDE_END {
        let k = 1.0 - (1.0 - span(DS_GLIDE_FROM, DS_GLIDE_END)).powi(3);
        let s = 1.1 - 0.05 * k;
        return Some([DS_OFF_TOP + (DS_LINE_UP - DS_OFF_TOP) * k, s, s]);
    }
    if seat < DS_PUSH_FROM {
        return Some([DS_LINE_UP, 1.05, 1.05]);
    }
    if seat < DS_PUSH_END {
        let k = span(DS_PUSH_FROM, DS_PUSH_END).powi(2);
        let s = 1.05 - 0.05 * k;
        return Some([DS_LINE_UP + (DS_PRESS - DS_LINE_UP) * k, s, s]);
    }
    if seat < DS_RELEASE_FROM {
        return Some([DS_PRESS, 1.0, 1.0]);
    }
    let k = 1.0 - (1.0 - span(DS_RELEASE_FROM, 1.0)).powi(2);
    Some([DS_PRESS * (1.0 - k), 1.0, 1.0])
}

fn ds_eject_top(u: f32) -> Option<[f32; 3]> {
    if u >= DS_SPRUNG {
        return None;
    }
    if u < DS_PRESS_END {
        return Some([DS_PRESS * slot_ui::ease(u / DS_PRESS_END), 1.0, 1.0]);
    }
    if u < DS_LATCH_END {
        return Some([DS_PRESS, 1.0, 1.0]);
    }
    let k = (u - DS_LATCH_END) / (DS_SPRUNG - DS_LATCH_END);
    let e = 1.0 - (1.0 - k).powi(3);
    Some([DS_PRESS + (DS_OFF_TOP - DS_PRESS) * e, 1.0, 1.0])
}

fn ds_eject_bottom(u: f32) -> Option<[f32; 3]> {
    (u >= 0.68).then(|| {
        if u < 0.88 {
            let k = 1.0 - (1.0 - (u - 0.68) / 0.2).powi(3);
            return [1.0 - k, 1.0, 1.0];
        }
        keyed(
            u,
            &[
                (0.88, [0.0, 1.0, 1.0]),
                (0.92, [0.0, 1.02, 0.97]),
                (0.96, [0.016, 1.0, 1.0]),
                (1.0, [0.0, 1.0, 1.0]),
            ],
        )
    })
}
