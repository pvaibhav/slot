use std::path::{Path, PathBuf};

use crate::atomic::atomic_write;
use crate::core::Core;
use crate::gb_palette::{GbColors, GbPalette};
use crate::platform::Platform;
use crate::zone::Zone;

pub const BRIGHTNESS_MAX: u8 = 9;
pub const DIM_MAX: u8 = 6;
pub const BLUE_LIGHT_MAX: u8 = 9;
pub const SATURATION_MAX: u8 = 9;
pub const CONTRAST_MAX: u8 = 9;
pub const SUNLIGHT_MAX: u8 = 10;
pub const VOLUME_MAX: u8 = 100;

const UTC_OFFSET_MIN: i16 = -720;
const UTC_OFFSET_MAX: i16 = 840;

pub const FF_SPEEDS: [u8; 6] = [0, 6, 8, 12, 16, 24];

const FF_SPEED_TEXT: [&str; 6] = ["0", "1.5", "2", "3", "4", "6"];

pub const LID_OFF_MIN: [u8; 6] = [1, 3, 5, 10, 15, 30];

pub const FF_SPEED_DEFAULT: u8 = 24;

pub const RECENTS_MAX: usize = 10;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Recent {
    pub platform: Platform,
    pub stem: String,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum ShelfMode {
    #[default]
    All,
    Collections,
}

impl ShelfMode {
    pub fn key(self) -> &'static str {
        match self {
            ShelfMode::All => "all",
            ShelfMode::Collections => "collections",
        }
    }

    pub fn parse(value: &str) -> Option<ShelfMode> {
        match value {
            "all" => Some(ShelfMode::All),
            "collections" => Some(ShelfMode::Collections),
            _ => None,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum KidMode {
    #[default]
    Off,
    OneShelf,
    Shelves,
}

impl KidMode {
    pub fn key(self) -> &'static str {
        match self {
            KidMode::Off => "off",
            KidMode::OneShelf => "one",
            KidMode::Shelves => "shelves",
        }
    }

    pub fn parse(value: &str) -> Option<KidMode> {
        match value {
            "off" => Some(KidMode::Off),
            "one" => Some(KidMode::OneShelf),
            "shelves" => Some(KidMode::Shelves),
            _ => None,
        }
    }
}

pub const KID_SHELF_ALL: &str = "all";

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Appearance {
    #[default]
    Dark,
    Light,
}

impl Appearance {
    pub fn light(self) -> bool {
        matches!(self, Appearance::Light)
    }

    pub fn name(self) -> &'static str {
        match self {
            Appearance::Dark => "dark",
            Appearance::Light => "light",
        }
    }

    pub fn parse(name: &str) -> Option<Appearance> {
        match name {
            "dark" => Some(Appearance::Dark),
            "light" => Some(Appearance::Light),
            _ => None,
        }
    }

    pub fn toggle(self) -> Appearance {
        match self {
            Appearance::Dark => Appearance::Light,
            Appearance::Light => Appearance::Dark,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum WallpaperTint {
    #[default]
    Off,
    Low,
    Medium,
    High,
}

impl WallpaperTint {
    pub const ALL: [WallpaperTint; 4] = [
        WallpaperTint::Off,
        WallpaperTint::Low,
        WallpaperTint::Medium,
        WallpaperTint::High,
    ];

    pub fn key(self) -> &'static str {
        match self {
            WallpaperTint::Off => "off",
            WallpaperTint::Low => "low",
            WallpaperTint::Medium => "medium",
            WallpaperTint::High => "high",
        }
    }

    pub fn parse(value: &str) -> Option<WallpaperTint> {
        WallpaperTint::ALL.into_iter().find(|t| t.key() == value)
    }

    pub fn step(self, right: bool) -> WallpaperTint {
        let n = WallpaperTint::ALL.len();
        let at = WallpaperTint::ALL
            .iter()
            .position(|t| *t == self)
            .unwrap_or(0);
        WallpaperTint::ALL[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum BatteryDisplay {
    #[default]
    Both,
    Icon,
    Percent,
}

impl BatteryDisplay {
    pub const ALL: [BatteryDisplay; 3] = [
        BatteryDisplay::Both,
        BatteryDisplay::Icon,
        BatteryDisplay::Percent,
    ];

    pub fn key(self) -> &'static str {
        match self {
            BatteryDisplay::Both => "both",
            BatteryDisplay::Icon => "icon",
            BatteryDisplay::Percent => "percent",
        }
    }

    pub fn parse(value: &str) -> Option<BatteryDisplay> {
        BatteryDisplay::ALL.into_iter().find(|d| d.key() == value)
    }

    pub fn icon(self) -> bool {
        self != BatteryDisplay::Percent
    }

    pub fn percent(self) -> bool {
        self != BatteryDisplay::Icon
    }

    pub fn step(self, right: bool) -> BatteryDisplay {
        let n = BatteryDisplay::ALL.len();
        let at = BatteryDisplay::ALL
            .iter()
            .position(|d| *d == self)
            .unwrap_or(0);
        BatteryDisplay::ALL[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum WallpaperPick {
    None,
    Random,
    Shelf,
    #[default]
    Default,
    Blur,
    File(String),
}

impl WallpaperPick {
    pub fn key(&self) -> String {
        match self {
            WallpaperPick::None => "none".to_string(),
            WallpaperPick::Random => "random".to_string(),
            WallpaperPick::Shelf => "shelf".to_string(),
            WallpaperPick::Default => "default".to_string(),
            WallpaperPick::Blur => "default_blur".to_string(),
            WallpaperPick::File(name) => name.clone(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            WallpaperPick::None => "None".to_string(),
            WallpaperPick::Random => "Random".to_string(),
            WallpaperPick::Shelf => "Per Shelf".to_string(),
            WallpaperPick::Default => "Slot".to_string(),
            WallpaperPick::Blur => "Slot Blur".to_string(),
            WallpaperPick::File(name) => name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(name)
                .to_string(),
        }
    }

    pub fn parse(value: &str) -> WallpaperPick {
        match value {
            "none" => WallpaperPick::None,
            "random" => WallpaperPick::Random,
            "shelf" => WallpaperPick::Shelf,
            "" | "default" => WallpaperPick::Default,
            "default_blur" => WallpaperPick::Blur,
            other => WallpaperPick::File(other.to_string()),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum BezelPick {
    None,
    Default,
    #[default]
    Ags102,
    File(String),
}

impl BezelPick {
    pub fn key(&self) -> String {
        match self {
            BezelPick::None => "none".to_string(),
            BezelPick::Default => "default".to_string(),
            BezelPick::Ags102 => "ags-102".to_string(),
            BezelPick::File(name) => name.clone(),
        }
    }

    pub fn label(&self) -> String {
        match self {
            BezelPick::None => "None".to_string(),
            BezelPick::Default => "Default".to_string(),
            BezelPick::Ags102 => "AGS-102".to_string(),
            BezelPick::File(name) => {
                let stem = name
                    .rsplit_once('.')
                    .map_or(name.as_str(), |(stem, _)| stem);
                match bezel_scale(name) {
                    Some(scale) => stem[..stem.len() - scale.bezel_suffix().len()].to_string(),
                    None => stem.to_string(),
                }
            }
        }
    }

    pub fn parse(value: &str) -> BezelPick {
        match value {
            "none" => BezelPick::None,
            "default" => BezelPick::Default,
            "" | "ags-102" => BezelPick::Ags102,
            other => BezelPick::File(other.to_string()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Scale {
    #[default]
    SpFrame,
    Integer,
    Fit,
    FullScreen,
}

pub fn bezel_scale(name: &str) -> Option<Scale> {
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let stem = stem.to_ascii_lowercase();
    [Scale::SpFrame, Scale::Integer, Scale::Fit]
        .into_iter()
        .find(|scale| stem.ends_with(scale.bezel_suffix()))
}

pub fn bezel_for_scale(name: &str, scale: Scale) -> Option<String> {
    let from = bezel_scale(name)?;
    let (stem, ext) = name.rsplit_once('.')?;
    let base = &stem[..stem.len() - from.bezel_suffix().len()];
    Some(format!("{base}{}.{ext}", scale.bezel_suffix()))
}

impl Scale {
    pub fn bezel_suffix(self) -> &'static str {
        match self {
            Scale::SpFrame => "_spscreen",
            Scale::Integer => "_integer",
            Scale::Fit => "_fit",
            Scale::FullScreen => "_fullscreen",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Scale::SpFrame => "sp-frame",
            Scale::Integer => "integer",
            Scale::Fit => "fit",
            Scale::FullScreen => "full-screen",
        }
    }

    pub fn parse(name: &str) -> Option<Scale> {
        match name {
            "sp-frame" => Some(Scale::SpFrame),
            "integer" => Some(Scale::Integer),
            "fit" => Some(Scale::Fit),
            "full-screen" => Some(Scale::FullScreen),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Stretch {
    #[default]
    Classic,
    FullScreen,
}

impl Stretch {
    pub fn name(self) -> &'static str {
        match self {
            Stretch::Classic => "classic",
            Stretch::FullScreen => "full-screen",
        }
    }

    pub fn parse(name: &str) -> Option<Stretch> {
        match name {
            "classic" => Some(Stretch::Classic),
            "full-screen" => Some(Stretch::FullScreen),
            _ => None,
        }
    }
}

/// The Game Boy the Authentic shader draws a Game Boy game on.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum GbModel {
    Dmg,
    #[default]
    Pocket,
    Light,
}

impl GbModel {
    pub const ALL: [GbModel; 3] = [GbModel::Dmg, GbModel::Pocket, GbModel::Light];

    pub fn key(self) -> &'static str {
        match self {
            GbModel::Dmg => "dmg",
            GbModel::Pocket => "pocket",
            GbModel::Light => "light",
        }
    }

    pub fn parse(value: &str) -> Option<GbModel> {
        GbModel::ALL.into_iter().find(|m| m.key() == value)
    }

    pub fn step(self, right: bool) -> GbModel {
        let at = GbModel::ALL.iter().position(|&m| m == self).unwrap_or(0);
        GbModel::ALL[(at + if right { 1 } else { 2 }) % 3]
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Blending {
    #[default]
    Off,
    Mix,
    Ghosting,
}

impl Blending {
    pub const ALL: [Blending; 3] = [Blending::Off, Blending::Mix, Blending::Ghosting];

    /// Slot blends frames on the GPU, so every Game Boy and GBA core offers every choice.
    pub fn choices(core: Core) -> &'static [Blending] {
        match core {
            Core::Mgba | Core::Gpsp | Core::Gambatte => &Blending::ALL,
            #[cfg(feature = "nds")]
            Core::Dsperate => &[Blending::Off],
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Blending::Off => "off",
            Blending::Mix => "mix",
            Blending::Ghosting => "ghosting",
        }
    }

    pub fn parse(value: &str) -> Option<Blending> {
        Blending::ALL.into_iter().find(|b| b.key() == value)
    }

    pub fn step(self, choices: &[Blending], right: bool) -> Blending {
        let n = choices.len();
        let at = choices.iter().position(|&b| b == self).unwrap_or(0);
        choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SlotState {
    pub cart: Option<String>,
    pub cart_platform: Option<Platform>,
    pub brightness: u8,
    pub dim: u8,
    pub blue_light: u8,
    pub saturation: u8,
    pub contrast: u8,
    pub volume: u8,
    pub volume_hp: u8,
    pub muted: bool,
    pub muted_hp: bool,
    pub clock_set: bool,
    pub zone: Zone,
    pub clock_24h: bool,
    pub network_time: bool,
    pub rumble: bool,
    pub ff_speed: u8,
    pub ff_sound: bool,
    pub colour_correction_gba: bool,
    pub colour_correction_gb: bool,
    pub colour_correction_gbc: bool,
    pub shader_gba: Shader,
    pub shader_gb: Shader,
    pub shader_gbc: Shader,
    pub resume_insert: bool,
    pub resume_power_on: bool,
    pub turbo: bool,
    pub rewind: bool,
    pub high_refresh: bool,
    pub gb_palettes: bool,
    pub gb_palette: GbPalette,
    pub appearance: Appearance,
    pub wallpaper: WallpaperPick,
    pub wallpaper_tint: WallpaperTint,
    pub wallpapers_given: bool,
    pub show_recents: bool,
    pub show_titles: bool,
    pub battery_display: BatteryDisplay,
    pub wifi_off: bool,
    pub gb_core: Core,
    pub left_stick_dpad: bool,
    pub lid_off_min: Option<u8>,
    pub bezel_gba: BezelPick,
    pub bezel_gb: BezelPick,
    pub bezel_gbc: BezelPick,
    /// Whether Authentic lays the glass's reflection over each console's screen.
    pub reflection_gba: bool,
    pub reflection_gb: bool,
    pub reflection_gbc: bool,
    pub overlay_gba: Option<String>,
    pub overlay_gb: Option<String>,
    pub overlay_gbc: Option<String>,
    #[cfg(feature = "nds")]
    pub overlay_nds: Option<String>,
    pub scale_gba: Scale,
    pub scale_gb: Scale,
    pub scale_gbc: Scale,
    pub stretch_gb: Stretch,
    pub stretch_gbc: Stretch,
    pub blending_gba: Blending,
    pub blending_gb: Blending,
    pub gb_model: GbModel,
    pub blending_gbc: Blending,
    pub recents: Vec<Recent>,
    pub favorites: Vec<Recent>,
    pub shelf: Option<String>,
    pub shelf_cart: Option<Recent>,
    pub shelf_mode: ShelfMode,
    pub collection: Option<String>,
    pub gba_video: bool,
    pub kid_mode: KidMode,
    pub kid_shelf: String,
}

impl SlotState {
    pub fn gb_colors(&self) -> GbColors {
        match self.gb_palettes {
            true => GbColors::Palette(self.gb_palette),
            false => GbColors::Auto,
        }
    }

    pub fn set_gb_colors(&mut self, to: GbColors) {
        match to {
            GbColors::Auto => self.gb_palettes = false,
            GbColors::Palette(p) => {
                self.gb_palettes = true;
                self.gb_palette = p;
            }
        }
    }

    pub fn played(&mut self, platform: Platform, stem: &str) {
        self.recents
            .retain(|r| !(r.platform == platform && r.stem == stem));
        self.recents.insert(
            0,
            Recent {
                platform,
                stem: stem.to_string(),
            },
        );
        self.recents.truncate(RECENTS_MAX);
    }

    pub fn forget(&mut self, platform: Platform, stem: &str) {
        self.recents
            .retain(|r| !(r.platform == platform && r.stem == stem));
    }

    pub fn is_favorite(&self, platform: Platform, stem: &str) -> bool {
        self.favorites
            .iter()
            .any(|f| f.platform == platform && f.stem == stem)
    }

    pub fn toggle_favorite(&mut self, platform: Platform, stem: &str) -> bool {
        if self.is_favorite(platform, stem) {
            self.favorites
                .retain(|f| !(f.platform == platform && f.stem == stem));
            return false;
        }
        self.favorites.push(Recent {
            platform,
            stem: stem.to_string(),
        });
        true
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shader {
    Off,
    Lcd3x,
    Grid,
    Dot,
    Simpletex,
    Shimmerless,
    Authentic,
}

impl Shader {
    pub const GBA: [Shader; 6] = [
        Shader::Off,
        Shader::Lcd3x,
        Shader::Grid,
        Shader::Dot,
        Shader::Shimmerless,
        Shader::Authentic,
    ];
    pub const GBA_3_2: [Shader; 5] = [
        Shader::Off,
        Shader::Lcd3x,
        Shader::Grid,
        Shader::Dot,
        Shader::Authentic,
    ];
    pub const GB: [Shader; 6] = [
        Shader::Off,
        Shader::Lcd3x,
        Shader::Grid,
        Shader::Simpletex,
        Shader::Shimmerless,
        Shader::Authentic,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Shader::Off => "off",
            Shader::Lcd3x => "lcd3x",
            Shader::Grid => "grid",
            Shader::Dot => "dot",
            Shader::Simpletex => "simpletex",
            Shader::Shimmerless => "shimmerless",
            Shader::Authentic => "authentic",
        }
    }

    pub fn parse(name: &str) -> Option<Shader> {
        [
            Shader::Off,
            Shader::Lcd3x,
            Shader::Grid,
            Shader::Dot,
            Shader::Simpletex,
            Shader::Shimmerless,
            Shader::Authentic,
        ]
        .into_iter()
        .find(|s| s.name() == name)
        // Authentic's name in test builds before it was named.
        .or((name == "transparent").then_some(Shader::Authentic))
    }

    pub fn step(self, choices: &[Shader], right: bool) -> Shader {
        let n = choices.len();
        let at = choices.iter().position(|&s| s == self).unwrap_or(0);
        choices[if right {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
}

impl Default for SlotState {
    fn default() -> Self {
        SlotState {
            cart: None,
            cart_platform: None,
            brightness: 5,
            dim: 0,
            blue_light: 0,
            saturation: 0,
            contrast: 0,
            volume: 60,
            volume_hp: 60,
            muted: false,
            muted_hp: false,
            clock_set: false,
            zone: Zone::default(),
            clock_24h: true,
            network_time: false,
            rumble: true,
            ff_speed: FF_SPEED_DEFAULT,
            ff_sound: false,
            colour_correction_gba: false,
            colour_correction_gb: false,
            colour_correction_gbc: false,
            shader_gba: Shader::Grid,
            shader_gb: Shader::Simpletex,
            shader_gbc: Shader::Simpletex,
            resume_insert: true,
            resume_power_on: true,
            turbo: true,
            rewind: true,
            high_refresh: true,
            gb_palettes: false,
            gb_palette: GbPalette::DEFAULT,
            appearance: Appearance::Dark,
            wallpaper: WallpaperPick::Default,
            wallpaper_tint: WallpaperTint::Off,
            wallpapers_given: false,
            show_recents: false,
            show_titles: false,
            battery_display: BatteryDisplay::Both,
            wifi_off: false,
            gb_core: Core::Gambatte,
            left_stick_dpad: true,
            lid_off_min: None,
            bezel_gba: BezelPick::Ags102,
            bezel_gb: BezelPick::Ags102,
            bezel_gbc: BezelPick::Ags102,
            reflection_gba: true,
            reflection_gb: true,
            reflection_gbc: true,
            overlay_gba: None,
            overlay_gb: None,
            overlay_gbc: None,
            #[cfg(feature = "nds")]
            overlay_nds: None,
            scale_gba: Scale::Fit,
            scale_gb: Scale::SpFrame,
            scale_gbc: Scale::SpFrame,
            stretch_gb: Stretch::Classic,
            stretch_gbc: Stretch::Classic,
            blending_gba: Blending::Off,
            blending_gb: Blending::Off,
            gb_model: GbModel::Pocket,
            blending_gbc: Blending::Off,
            recents: Vec::new(),
            favorites: Vec::new(),
            shelf: None,
            shelf_cart: None,
            shelf_mode: ShelfMode::All,
            collection: None,
            gba_video: false,
            kid_mode: KidMode::Off,
            kid_shelf: KID_SHELF_ALL.to_string(),
        }
    }
}

fn state_path(root: &Path) -> PathBuf {
    root.join("Config").join("slot.state")
}

const RECENTS_FILE: &str = "recents.txt";
const FAVORITES_FILE: &str = "favorites.txt";

fn list_path(root: &Path, name: &str) -> PathBuf {
    root.join("Config").join(name)
}

pub fn read_slot_state(root: &Path) -> SlotState {
    let mut s: SlotState = std::fs::read(state_path(root))
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| parse(&s))
        .unwrap_or_default();
    s.recents = read_list(root, RECENTS_FILE);
    s.recents.truncate(RECENTS_MAX);
    s.favorites = read_list(root, FAVORITES_FILE);
    s
}

fn read_list(root: &Path, name: &str) -> Vec<Recent> {
    let text = std::fs::read_to_string(list_path(root, name)).unwrap_or_default();
    let mut list: Vec<Recent> = Vec::new();
    for r in text.lines().filter_map(recent_value) {
        if !list.contains(&r) {
            list.push(r);
        }
    }
    list
}

fn write_list(root: &Path, name: &str, list: &[Recent]) -> std::io::Result<()> {
    let text: String = list
        .iter()
        .map(|r| format!("{}/{}\n", platform_key(r.platform), r.stem))
        .collect();
    let path = list_path(root, name);
    if std::fs::read(&path).map_or(text.is_empty(), |old| old == text.as_bytes()) {
        return Ok(());
    }
    atomic_write(&path, text.as_bytes())
}

pub fn write_slot_state(root: &Path, s: &SlotState) -> std::io::Result<()> {
    let mut text = format!(
        "cart={}\ncart_platform={}\nbrightness={}\nblue_light={}\nvolume={}\nvolume_hp={}\nmuted={}\nmuted_hp={}\nclock_set={}\nzone={}\nclock_24h={}\nnetwork_time={}\nrumble={}\nff_speed={}\nff_sound={}\ncolour_correction_gba={}\ncolour_correction_gb={}\nshader_gba={}\nshader_gb={}\nturbo={}\nrewind={}\nhigh_refresh={}\ngb_palettes={}\ngb_palette={}\nappearance={}\nwallpaper={}\n",
        s.cart.as_deref().unwrap_or(""),
        s.cart_platform.map_or(String::new(), platform_key),
        s.brightness,
        s.blue_light,
        s.volume,
        s.volume_hp,
        s.muted as u8,
        s.muted_hp as u8,
        s.clock_set as u8,
        s.zone.id(),
        s.clock_24h as u8,
        s.network_time as u8,
        s.rumble as u8,
        ff_speed_text(s.ff_speed),
        s.ff_sound as u8,
        s.colour_correction_gba as u8,
        s.colour_correction_gb as u8,
        s.shader_gba.name(),
        s.shader_gb.name(),
        s.turbo as u8,
        s.rewind as u8,
        s.high_refresh as u8,
        s.gb_palettes as u8,
        s.gb_palette.core_name(),
        s.appearance.name(),
        s.wallpaper.key()
    );
    text.push_str(&format!(
        "colour_correction_gbc={}\n",
        s.colour_correction_gbc as u8
    ));
    text.push_str(&format!("shader_gbc={}\n", s.shader_gbc.name()));
    text.push_str(&format!("dim={}\n", s.dim));
    text.push_str(&format!("saturation={}\n", s.saturation));
    text.push_str(&format!("contrast={}\n", s.contrast));
    text.push_str(&format!("wallpaper_tint={}\n", s.wallpaper_tint.key()));
    text.push_str(&format!("wallpapers_given={}\n", s.wallpapers_given as u8));
    text.push_str(&format!("show_recents={}\n", s.show_recents as u8));
    text.push_str(&format!("show_titles={}\n", s.show_titles as u8));
    text.push_str(&format!("battery_display={}\n", s.battery_display.key()));
    if s.wifi_off {
        text.push_str("wifi_off=1\n");
    }
    text.push_str(&format!("gb_core={}\n", s.gb_core.as_str()));
    text.push_str(&format!("left_stick_dpad={}\n", s.left_stick_dpad as u8));
    text.push_str(&format!("resume_insert={}\n", s.resume_insert as u8));
    text.push_str(&format!("resume_power_on={}\n", s.resume_power_on as u8));
    if let Some(min) = s.lid_off_min {
        text.push_str(&format!("lid_off_min={min}\n"));
    }
    text.push_str(&format!("bezel_gba={}\n", s.bezel_gba.key()));
    text.push_str(&format!("bezel_gb={}\n", s.bezel_gb.key()));
    text.push_str(&format!("bezel_gbc={}\n", s.bezel_gbc.key()));
    text.push_str(&format!("reflection_gba={}\n", s.reflection_gba as u8));
    text.push_str(&format!("reflection_gb={}\n", s.reflection_gb as u8));
    text.push_str(&format!("reflection_gbc={}\n", s.reflection_gbc as u8));
    text.push_str(&format!(
        "overlay_gba={}\n",
        s.overlay_gba.as_deref().unwrap_or_default()
    ));
    text.push_str(&format!(
        "overlay_gb={}\n",
        s.overlay_gb.as_deref().unwrap_or_default()
    ));
    text.push_str(&format!(
        "overlay_gbc={}\n",
        s.overlay_gbc.as_deref().unwrap_or_default()
    ));
    #[cfg(feature = "nds")]
    text.push_str(&format!(
        "overlay_nds={}\n",
        s.overlay_nds.as_deref().unwrap_or_default()
    ));
    text.push_str(&format!("scale_gba={}\n", s.scale_gba.name()));
    text.push_str(&format!("scale_gb={}\n", s.scale_gb.name()));
    text.push_str(&format!("scale_gbc={}\n", s.scale_gbc.name()));
    text.push_str(&format!("stretch_gb={}\n", s.stretch_gb.name()));
    text.push_str(&format!("stretch_gbc={}\n", s.stretch_gbc.name()));
    text.push_str(&format!("blending_gba={}\n", s.blending_gba.key()));
    text.push_str(&format!("blending_gb={}\n", s.blending_gb.key()));
    text.push_str(&format!("gb_model={}\n", s.gb_model.key()));
    text.push_str(&format!("blending_gbc={}\n", s.blending_gbc.key()));
    text.push_str(&format!("shelf={}\n", s.shelf.as_deref().unwrap_or("")));
    text.push_str(&format!(
        "shelf_cart={}\n",
        s.shelf_cart.as_ref().map_or(String::new(), |r| format!(
            "{}/{}",
            platform_key(r.platform),
            r.stem
        ))
    ));
    text.push_str(&format!("shelf_mode={}\n", s.shelf_mode.key()));
    text.push_str(&format!("kid_mode={}\n", s.kid_mode.key()));
    text.push_str(&format!("kid_shelf={}\n", s.kid_shelf));
    text.push_str(&format!(
        "collection={}\n",
        s.collection.as_deref().unwrap_or("")
    ));
    text.push_str(&format!("gba_video={}\n", s.gba_video as u8));
    atomic_write(&state_path(root), text.as_bytes())?;
    write_list(root, RECENTS_FILE, &s.recents)?;
    write_list(root, FAVORITES_FILE, &s.favorites)
}

fn auto_load_value(value: &str) -> Option<(bool, bool)> {
    match value {
        "1" => Some((true, true)),
        "power" => Some((false, true)),
        "0" => Some((false, false)),
        _ => None,
    }
}

fn parse(text: &str) -> Option<SlotState> {
    let mut cart = None;
    let mut cart_platform = None;
    let mut brightness = None;
    let mut blue_light = None;
    let mut dim = None;
    let mut saturation = None;
    let mut contrast = None;
    let mut volume = None;
    let mut volume_hp = None;
    let mut muted = None;
    let mut muted_hp = None;
    let mut clock_set = None;
    let mut utc_offset_min = None;
    let mut zone = None;
    let mut clock_24h = None;
    let mut network_time = None;
    let mut rumble = None;
    let mut ff_speed = None;
    let mut ff_sound = None;
    let mut colour_correction = None;
    let mut colour_correction_gba = None;
    let mut colour_correction_gb = None;
    let mut colour_correction_gbc = None;
    let mut ghosting_gba = None;
    let mut ghosting_gb = None;
    let mut ghosting_gbc = None;
    let mut shader_gba = None;
    let mut shader_gb = None;
    let mut shader_gbc = None;
    let mut auto_load = None;
    let mut resume_insert = None;
    let mut resume_power_on = None;
    let mut wallpapers_given = None;
    let mut turbo = None;
    let mut rewind = None;
    let mut high_refresh = None;
    let mut gb_palettes = None;
    let mut gb_palette = None;
    let mut appearance = None;
    let mut wallpaper = None;
    let mut wallpaper_tint = None;
    let mut show_recents = None;
    let mut show_titles = None;
    let mut battery_display = None;
    let mut wifi_off = None;
    let mut gb_core = None;
    let mut left_stick_dpad = None;
    let mut lid_off_min = None;
    let mut bezel_gba = None;
    let mut bezel_gb = None;
    let mut bezel_gbc = None;
    let mut reflection_gba = None;
    let mut reflection_gb = None;
    let mut reflection_gbc = None;
    let mut overlay_gba = None;
    let mut overlay_gb = None;
    let mut overlay_gbc = None;
    #[cfg(feature = "nds")]
    let mut overlay_nds = None;
    let mut scale_gba = None;
    let mut scale_gb = None;
    let mut scale_gbc = None;
    let mut stretch_gb = None;
    let mut stretch_gbc = None;
    let mut blending_gba = None;
    let mut blending_gb = None;
    let mut gb_model = None;
    let mut blending_gbc = None;
    let mut shelf = None;
    let mut shelf_cart = None;
    let mut shelf_mode = None;
    let mut kid_mode = None;
    let mut kid_shelf = None;
    let mut collection = None;
    let mut gba_video = None;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "cart" => cart = Some(value.to_string()),
            "cart_platform" => cart_platform = platform_value(value),
            "brightness" => brightness = Some(level(value, BRIGHTNESS_MAX)?),
            "blue_light" => blue_light = Some(level(value, BLUE_LIGHT_MAX)?),
            "dim" => dim = level(value, DIM_MAX),
            "saturation" => saturation = level(value, SATURATION_MAX),
            "contrast" => contrast = level(value, CONTRAST_MAX),
            "volume" => volume = Some(level(value, VOLUME_MAX)?),
            "volume_hp" => volume_hp = Some(level(value, VOLUME_MAX)?),
            "muted" => muted = Some(level(value, 1)? == 1),
            "muted_hp" => muted_hp = Some(level(value, 1)? == 1),
            "clock_set" => clock_set = Some(level(value, 1)? == 1),
            "utc_offset_min" => utc_offset_min = Some(offset(value)?),
            "zone" => zone = Zone::parse(value),
            "clock_24h" => clock_24h = flag(value),
            "network_time" => network_time = flag(value),
            "rumble" => rumble = flag(value),
            "ff_speed" => ff_speed = ff_speed_value(value),
            "ff_sound" => ff_sound = flag(value),
            "colour_correction" => colour_correction = flag(value),
            "colour_correction_gba" => colour_correction_gba = flag(value),
            "colour_correction_gb" => colour_correction_gb = flag(value),
            "colour_correction_gbc" => colour_correction_gbc = flag(value),
            "ghosting_gba" => ghosting_gba = flag(value),
            "ghosting_gb" => ghosting_gb = flag(value),
            "ghosting_gbc" => ghosting_gbc = flag(value),
            "shader_gba" => shader_gba = Shader::parse(value).filter(|s| Shader::GBA.contains(s)),
            "auto_load" => auto_load = auto_load_value(value),
            "resume_insert" => resume_insert = flag(value),
            "resume_power_on" => resume_power_on = flag(value),
            "wallpapers_given" => wallpapers_given = flag(value),
            "turbo" => turbo = flag(value),
            "rewind" => rewind = flag(value),
            "high_refresh" => high_refresh = Some(flag(value).unwrap_or(false)),
            "gb_palettes" => gb_palettes = flag(value),
            "gb_palette" => gb_palette = GbPalette::parse(value),
            "appearance" => appearance = Appearance::parse(value),
            "wallpaper" => wallpaper = Some(WallpaperPick::parse(value)),
            "wallpaper_tint" => wallpaper_tint = WallpaperTint::parse(value),
            "show_recents" => show_recents = flag(value),
            "show_titles" => show_titles = flag(value),
            "battery_display" => battery_display = BatteryDisplay::parse(value),
            "wifi_off" => wifi_off = flag(value),
            "gb_core" => gb_core = Core::parse(value).filter(|c| c.runs(Platform::Gb)),
            "left_stick_dpad" => left_stick_dpad = flag(value),
            "lid_off_min" => lid_off_min = value.parse().ok().filter(|m| LID_OFF_MIN.contains(m)),
            "bezel_gba" => bezel_gba = Some(BezelPick::parse(value)),
            "bezel_gb" => bezel_gb = Some(BezelPick::parse(value)),
            "bezel_gbc" => bezel_gbc = Some(BezelPick::parse(value)),
            "reflection_gba" => reflection_gba = flag(value),
            "reflection_gb" => reflection_gb = flag(value),
            "reflection_gbc" => reflection_gbc = flag(value),
            "overlay_gba" => overlay_gba = overlay_name(value),
            "overlay_gb" => overlay_gb = overlay_name(value),
            "overlay_gbc" => overlay_gbc = overlay_name(value),
            #[cfg(feature = "nds")]
            "overlay_nds" => overlay_nds = overlay_name(value),
            "scale_gba" => scale_gba = Scale::parse(value),
            "scale_gb" => scale_gb = Scale::parse(value),
            "scale_gbc" => scale_gbc = Scale::parse(value),
            "stretch_gb" => stretch_gb = Stretch::parse(value),
            "stretch_gbc" => stretch_gbc = Stretch::parse(value),
            "blending_gba" => blending_gba = Some(Blending::parse(value).unwrap_or_default()),
            "blending_gb" => blending_gb = Some(Blending::parse(value).unwrap_or_default()),
            "gb_model" => gb_model = GbModel::parse(value),
            "blending_gbc" => blending_gbc = Some(Blending::parse(value).unwrap_or_default()),
            "shelf" => shelf = Some(value.to_string()).filter(|v| !v.is_empty()),
            "shelf_cart" => shelf_cart = recent_value(value),
            "shelf_mode" => shelf_mode = ShelfMode::parse(value),
            "kid_mode" => kid_mode = KidMode::parse(value),
            "kid_shelf" => kid_shelf = Some(value.to_string()).filter(|v| !v.is_empty()),
            "collection" => collection = Some(value.to_string()).filter(|v| !v.is_empty()),
            "gba_video" => gba_video = flag(value),
            "shader_gb" => shader_gb = Shader::parse(value).filter(|s| Shader::GB.contains(s)),
            "shader_gbc" => shader_gbc = Shader::parse(value).filter(|s| Shader::GB.contains(s)),
            _ => {}
        }
    }
    let cart = cart?;
    let fallback = SlotState::default();
    let legacy_blending = |on: Option<bool>| {
        on.or(high_refresh.map(|_| true))
            .map(|on| if on { Blending::Mix } else { Blending::Off })
    };
    Some(SlotState {
        cart: (!cart.is_empty()).then_some(cart),
        cart_platform,
        brightness: brightness?,
        dim: dim.unwrap_or(fallback.dim),
        blue_light: blue_light?,
        saturation: saturation.unwrap_or(fallback.saturation),
        contrast: contrast.unwrap_or(fallback.contrast),
        volume: volume?,
        volume_hp: volume_hp.or(volume)?,
        muted: muted?,
        muted_hp: muted_hp.or(muted)?,
        clock_set: clock_set?,
        zone: zone.or_else(|| utc_offset_min.map(|m| Zone::for_offset(i64::from(m), now())))?,
        clock_24h: clock_24h.unwrap_or(fallback.clock_24h),
        network_time: network_time.unwrap_or(fallback.network_time),
        rumble: rumble.unwrap_or(fallback.rumble),
        ff_speed: ff_speed.unwrap_or(fallback.ff_speed),
        ff_sound: ff_sound.unwrap_or(fallback.ff_sound),
        colour_correction_gba: colour_correction_gba
            .or(colour_correction)
            .unwrap_or(fallback.colour_correction_gba),
        colour_correction_gb: colour_correction_gb
            .or(colour_correction)
            .unwrap_or(fallback.colour_correction_gb),
        colour_correction_gbc: colour_correction_gbc
            .or(colour_correction_gb)
            .or(colour_correction)
            .unwrap_or(fallback.colour_correction_gbc),
        shader_gba: shader_gba.unwrap_or(fallback.shader_gba),
        shader_gb: shader_gb.unwrap_or(fallback.shader_gb),
        shader_gbc: shader_gbc.or(shader_gb).unwrap_or(fallback.shader_gbc),
        resume_insert: resume_insert
            .or(auto_load.map(|(insert, _)| insert))
            .unwrap_or(fallback.resume_insert),
        resume_power_on: resume_power_on
            .or(auto_load.map(|(_, power_on)| power_on))
            .unwrap_or(fallback.resume_power_on),
        turbo: turbo.unwrap_or(fallback.turbo),
        rewind: rewind.unwrap_or(fallback.rewind),
        high_refresh: high_refresh.unwrap_or(fallback.high_refresh),
        gb_palettes: gb_palettes.unwrap_or(fallback.gb_palettes),
        gb_palette: gb_palette.unwrap_or(fallback.gb_palette),
        appearance: appearance.unwrap_or(fallback.appearance),
        wallpaper: wallpaper.unwrap_or(fallback.wallpaper),
        wallpaper_tint: wallpaper_tint.unwrap_or(fallback.wallpaper_tint),
        wallpapers_given: wallpapers_given.unwrap_or(fallback.wallpapers_given),
        show_recents: show_recents.unwrap_or(fallback.show_recents),
        show_titles: show_titles.unwrap_or(fallback.show_titles),
        battery_display: battery_display.unwrap_or(fallback.battery_display),
        wifi_off: wifi_off.unwrap_or(fallback.wifi_off),
        gb_core: gb_core.unwrap_or(Core::Mgba),
        left_stick_dpad: left_stick_dpad.unwrap_or(fallback.left_stick_dpad),
        lid_off_min,
        bezel_gba: bezel_gba.unwrap_or(fallback.bezel_gba),
        bezel_gb: bezel_gb.unwrap_or(fallback.bezel_gb),
        bezel_gbc: bezel_gbc.unwrap_or(fallback.bezel_gbc),
        reflection_gba: reflection_gba.unwrap_or(fallback.reflection_gba),
        reflection_gb: reflection_gb.unwrap_or(fallback.reflection_gb),
        reflection_gbc: reflection_gbc.unwrap_or(fallback.reflection_gbc),
        overlay_gba,
        overlay_gb,
        overlay_gbc,
        #[cfg(feature = "nds")]
        overlay_nds,
        scale_gba: scale_gba.unwrap_or(fallback.scale_gba),
        scale_gb: scale_gb.unwrap_or(fallback.scale_gb),
        scale_gbc: scale_gbc.unwrap_or(fallback.scale_gbc),
        stretch_gb: stretch_gb.unwrap_or(fallback.stretch_gb),
        stretch_gbc: stretch_gbc.unwrap_or(fallback.stretch_gbc),
        blending_gba: blending_gba
            .or_else(|| legacy_blending(ghosting_gba))
            .unwrap_or(fallback.blending_gba),
        blending_gb: blending_gb
            .or_else(|| legacy_blending(ghosting_gb))
            .unwrap_or(fallback.blending_gb),
        gb_model: gb_model.unwrap_or(fallback.gb_model),
        blending_gbc: blending_gbc
            .or_else(|| legacy_blending(ghosting_gbc))
            .unwrap_or(fallback.blending_gbc),
        recents: Vec::new(),
        favorites: Vec::new(),
        shelf,
        shelf_cart,
        shelf_mode: shelf_mode.unwrap_or(fallback.shelf_mode),
        kid_mode: kid_mode.unwrap_or(KidMode::Off),
        kid_shelf: kid_shelf.unwrap_or_else(|| KID_SHELF_ALL.to_string()),
        collection,
        gba_video: gba_video.unwrap_or(false),
    })
}

pub(crate) fn platform_key(platform: Platform) -> String {
    platform.dir_name().to_ascii_lowercase()
}

pub(crate) fn recent_value(value: &str) -> Option<Recent> {
    let (platform, stem) = value.split_once('/')?;
    Some(Recent {
        platform: platform_value(platform)?,
        stem: stem.to_string(),
    })
    .filter(|r| !r.stem.is_empty())
}

fn platform_value(value: &str) -> Option<Platform> {
    Platform::ALL
        .into_iter()
        .find(|p| value.eq_ignore_ascii_case(p.dir_name()))
}

fn offset(value: &str) -> Option<i16> {
    value
        .parse()
        .ok()
        .filter(|n| (UTC_OFFSET_MIN..=UTC_OFFSET_MAX).contains(n))
}

fn ff_speed_value(value: &str) -> Option<u8> {
    FF_SPEED_TEXT
        .iter()
        .position(|t| *t == value)
        .map(|i| FF_SPEEDS[i])
}

fn ff_speed_text(speed: u8) -> &'static str {
    let at = |speed| FF_SPEEDS.iter().position(|s| *s == speed);
    FF_SPEED_TEXT[at(speed).or(at(FF_SPEED_DEFAULT)).unwrap_or(0)]
}

fn level(value: &str, max: u8) -> Option<u8> {
    value.parse().ok().filter(|n| *n <= max)
}

fn overlay_name(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

fn flag(value: &str) -> Option<bool> {
    level(value, 1).map(|n| n == 1)
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
