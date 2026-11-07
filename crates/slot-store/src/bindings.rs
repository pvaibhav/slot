use std::path::Path;

use crate::atomic::atomic_write;
use crate::platform::Platform;

const CONTROLS_FILE: &str = "controls.txt";
const CAROUSEL_FILE: &str = "carousel.txt";
const SHORTCUTS_FILE: &str = "shortcuts.txt";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Device {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    Select,
    Start,
}

impl Device {
    pub fn leads(self) -> bool {
        matches!(self, Device::Select | Device::Start)
    }

    pub const ALL: [Device; 14] = [
        Device::Up,
        Device::Down,
        Device::Left,
        Device::Right,
        Device::A,
        Device::B,
        Device::X,
        Device::Y,
        Device::L1,
        Device::R1,
        Device::L2,
        Device::R2,
        Device::Select,
        Device::Start,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Device::Up => "up",
            Device::Down => "down",
            Device::Left => "left",
            Device::Right => "right",
            Device::A => "a",
            Device::B => "b",
            Device::X => "x",
            Device::Y => "y",
            Device::L1 => "l1",
            Device::R1 => "r1",
            Device::L2 => "l2",
            Device::R2 => "r2",
            Device::Select => "select",
            Device::Start => "start",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Device::Up => "UP",
            Device::Down => "DOWN",
            Device::Left => "LEFT",
            Device::Right => "RIGHT",
            Device::A => "A",
            Device::B => "B",
            Device::X => "X",
            Device::Y => "Y",
            Device::L1 => "L1",
            Device::R1 => "R1",
            Device::L2 => "L2",
            Device::R2 => "R2",
            Device::Select => "SELECT",
            Device::Start => "START",
        }
    }

    fn parse(key: &str) -> Option<Device> {
        Device::ALL
            .into_iter()
            .find(|d| d.key().eq_ignore_ascii_case(key.trim()))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Modifier {
    Select,
    Menu,
    Button(Device),
}

impl Modifier {
    fn key(self) -> &'static str {
        match self {
            Modifier::Select => "select",
            Modifier::Menu => "menu",
            Modifier::Button(d) => d.key(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Modifier::Select => "SEL",
            Modifier::Menu => "MENU",
            Modifier::Button(d) => d.label(),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Binding {
    pub modifier: Option<Modifier>,
    pub button: Device,
}

impl Binding {
    pub fn plain(button: Device) -> Option<Binding> {
        Some(Binding {
            modifier: None,
            button,
        })
    }

    pub fn with(modifier: Modifier, button: Device) -> Option<Binding> {
        Some(Binding {
            modifier: Some(modifier),
            button,
        })
    }

    pub fn pair(first: Device, second: Device) -> Option<Binding> {
        match (first, second) {
            _ if first == second => Binding::plain(first),
            (Device::Select, other) | (other, Device::Select) => {
                Binding::with(Modifier::Select, other)
            }
            (Device::Start, other) | (other, Device::Start) => {
                Binding::with(Modifier::Button(Device::Start), other)
            }
            _ if (first as usize) < (second as usize) => {
                Binding::with(Modifier::Button(first), second)
            }
            _ => Binding::with(Modifier::Button(second), first),
        }
    }

    pub fn text(binding: Option<Binding>) -> String {
        match binding {
            None => "NONE".to_string(),
            Some(Binding {
                modifier: None,
                button,
            }) => button.label().to_string(),
            Some(Binding {
                modifier: Some(m),
                button,
            }) => format!("{}+{}", m.label(), button.label()),
        }
    }

    fn key(binding: Option<Binding>) -> String {
        match binding {
            None => "none".to_string(),
            Some(Binding {
                modifier: None,
                button,
            }) => button.key().to_string(),
            Some(Binding {
                modifier: Some(m),
                button,
            }) => format!("{}+{}", m.key(), button.key()),
        }
    }

    fn parse(value: &str) -> Option<Option<Binding>> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("none") {
            return Some(None);
        }
        let Some((m, b)) = value.split_once('+') else {
            return Some(Binding::plain(Device::parse(value)?));
        };
        let button = Device::parse(b)?;
        if m.trim().eq_ignore_ascii_case(Modifier::Menu.key()) {
            return Some(Binding::with(Modifier::Menu, button));
        }
        Some(Binding::pair(Device::parse(m)?, button))
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GameButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Select,
    Start,
    L,
    R,
    TurboA,
    TurboB,
}

impl GameButton {
    pub const ALL: [GameButton; 12] = [
        GameButton::Up,
        GameButton::Down,
        GameButton::Left,
        GameButton::Right,
        GameButton::A,
        GameButton::B,
        GameButton::Select,
        GameButton::Start,
        GameButton::L,
        GameButton::R,
        GameButton::TurboA,
        GameButton::TurboB,
    ];

    const GAME_BOY: [GameButton; 10] = [
        GameButton::Up,
        GameButton::Down,
        GameButton::Left,
        GameButton::Right,
        GameButton::A,
        GameButton::B,
        GameButton::Select,
        GameButton::Start,
        GameButton::TurboA,
        GameButton::TurboB,
    ];

    pub fn of(platform: Platform) -> &'static [GameButton] {
        match platform {
            Platform::Gba | Platform::Gbav => &GameButton::ALL,
            Platform::Gb | Platform::Gbc => &GameButton::GAME_BOY,
            #[cfg(feature = "nds")]
            Platform::Nds => &GameButton::ALL,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GameButton::Up => "Up",
            GameButton::Down => "Down",
            GameButton::Left => "Left",
            GameButton::Right => "Right",
            GameButton::A => "A Button",
            GameButton::B => "B Button",
            GameButton::Select => "Select",
            GameButton::Start => "Start",
            GameButton::L => "L Button",
            GameButton::R => "R Button",
            GameButton::TurboA => "Turbo A",
            GameButton::TurboB => "Turbo B",
        }
    }

    fn key(self) -> &'static str {
        match self {
            GameButton::Up => "up",
            GameButton::Down => "down",
            GameButton::Left => "left",
            GameButton::Right => "right",
            GameButton::A => "a",
            GameButton::B => "b",
            GameButton::Select => "select",
            GameButton::Start => "start",
            GameButton::L => "l",
            GameButton::R => "r",
            GameButton::TurboA => "turbo_a",
            GameButton::TurboB => "turbo_b",
        }
    }

    fn default_device(self) -> Device {
        match self {
            GameButton::Up => Device::Up,
            GameButton::Down => Device::Down,
            GameButton::Left => Device::Left,
            GameButton::Right => Device::Right,
            GameButton::A => Device::A,
            GameButton::B => Device::B,
            GameButton::Select => Device::Select,
            GameButton::Start => Device::Start,
            GameButton::L => Device::L1,
            GameButton::R => Device::R1,
            GameButton::TurboA => Device::X,
            GameButton::TurboB => Device::Y,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Shortcut {
    SaveState,
    LoadState,
    Screenshot,
    BrightnessUp,
    BrightnessDown,
    BlueLightDown,
    BlueLightUp,
    ColorCorrection,
    PaletteBack,
    PaletteNext,
    ShaderNext,
    ShaderBack,
    FastForward,
    FastForwardToggle,
    Rewind,
    SunlightUp,
    SunlightDown,
    Reset,
    Guide,
}

impl Shortcut {
    pub const ALL: [Shortcut; 19] = [
        Shortcut::SaveState,
        Shortcut::LoadState,
        Shortcut::Screenshot,
        Shortcut::BrightnessUp,
        Shortcut::BrightnessDown,
        Shortcut::BlueLightDown,
        Shortcut::BlueLightUp,
        Shortcut::ColorCorrection,
        Shortcut::PaletteBack,
        Shortcut::PaletteNext,
        Shortcut::ShaderNext,
        Shortcut::ShaderBack,
        Shortcut::FastForward,
        Shortcut::FastForwardToggle,
        Shortcut::Rewind,
        Shortcut::SunlightUp,
        Shortcut::SunlightDown,
        Shortcut::Reset,
        Shortcut::Guide,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Shortcut::SaveState => "Save State",
            Shortcut::LoadState => "Load State",
            Shortcut::Screenshot => "Screenshot",
            Shortcut::BrightnessUp => "Brightness Up",
            Shortcut::BrightnessDown => "Brightness Down",
            Shortcut::BlueLightDown => "Blue Light Down",
            Shortcut::BlueLightUp => "Blue Light Up",
            Shortcut::ColorCorrection => "Color Correction",
            Shortcut::PaletteBack => "Palette Back",
            Shortcut::PaletteNext => "Palette Next",
            Shortcut::ShaderNext => "Shader Next",
            Shortcut::ShaderBack => "Shader Back",
            Shortcut::FastForward => "Fast Forward",
            Shortcut::FastForwardToggle => "FF Toggle",
            Shortcut::Rewind => "Rewind",
            Shortcut::SunlightUp => "Sunlight Up",
            Shortcut::SunlightDown => "Sunlight Down",
            Shortcut::Reset => "Reset Game",
            Shortcut::Guide => "Guide",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Shortcut::SaveState => "save_state",
            Shortcut::LoadState => "load_state",
            Shortcut::Screenshot => "screenshot",
            Shortcut::BrightnessUp => "brightness_up",
            Shortcut::BrightnessDown => "brightness_down",
            Shortcut::BlueLightDown => "blue_light_down",
            Shortcut::BlueLightUp => "blue_light_up",
            Shortcut::ColorCorrection => "color_correction",
            Shortcut::PaletteBack => "palette_back",
            Shortcut::PaletteNext => "palette_next",
            Shortcut::ShaderNext => "shader_next",
            Shortcut::ShaderBack => "shader_back",
            Shortcut::FastForward => "fast_forward",
            Shortcut::FastForwardToggle => "fast_forward_toggle",
            Shortcut::Rewind => "rewind",
            Shortcut::SunlightUp => "sunlight_up",
            Shortcut::SunlightDown => "sunlight_down",
            Shortcut::Reset => "reset",
            Shortcut::Guide => "guide",
        }
    }

    fn default_binding(self) -> Option<Binding> {
        let select = |b| Binding::with(Modifier::Select, b);
        match self {
            Shortcut::SaveState => select(Device::R1),
            Shortcut::LoadState => select(Device::L1),
            Shortcut::Screenshot => select(Device::X),
            Shortcut::BrightnessUp => select(Device::Up),
            Shortcut::BrightnessDown => select(Device::Down),
            Shortcut::BlueLightDown => select(Device::Left),
            Shortcut::BlueLightUp => select(Device::Right),
            Shortcut::ColorCorrection => select(Device::Y),
            Shortcut::PaletteBack => select(Device::L2),
            Shortcut::PaletteNext => select(Device::R2),
            Shortcut::ShaderNext => select(Device::A),
            Shortcut::ShaderBack => select(Device::B),
            Shortcut::FastForward => Binding::plain(Device::R2),
            Shortcut::FastForwardToggle => None,
            Shortcut::Rewind => Binding::plain(Device::L2),
            Shortcut::SunlightUp | Shortcut::SunlightDown | Shortcut::Reset | Shortcut::Guide => {
                None
            }
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Controls {
    game: [Option<Binding>; 12],
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            game: GameButton::ALL.map(|g| Binding::plain(g.default_device())),
        }
    }
}

impl Controls {
    pub fn game(&self, button: GameButton) -> Option<Binding> {
        self.game[button.index()]
    }

    pub fn set_game(&mut self, button: GameButton, binding: Option<Binding>) {
        self.game[button.index()] = binding;
    }

    fn text(&self) -> String {
        GameButton::ALL
            .iter()
            .map(|g| format!("{}={}\n", g.key(), Binding::key(self.game(*g))))
            .collect()
    }

    fn parse(text: &str) -> Controls {
        let mut c = Controls::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            let game = GameButton::ALL
                .into_iter()
                .find(|g| g.key().eq_ignore_ascii_case(key.trim()));
            if let (Some(g), Some(binding)) = (game, Binding::parse(value)) {
                c.set_game(g, binding);
            }
        }
        c
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ShortcutKeys {
    keys: [Option<Binding>; 19],
}

impl Default for ShortcutKeys {
    fn default() -> Self {
        ShortcutKeys {
            keys: Shortcut::ALL.map(Shortcut::default_binding),
        }
    }
}

impl ShortcutKeys {
    pub fn get(&self, shortcut: Shortcut) -> Option<Binding> {
        self.keys[shortcut.index()]
    }

    pub fn set(&mut self, shortcut: Shortcut, binding: Option<Binding>) {
        self.keys[shortcut.index()] = binding;
        let rival = match shortcut {
            Shortcut::FastForward => Shortcut::FastForwardToggle,
            Shortcut::FastForwardToggle => Shortcut::FastForward,
            _ => return,
        };
        if binding.is_some() && self.get(rival) == binding {
            self.keys[rival.index()] = None;
        }
    }
}

pub fn read_shortcuts(root: &Path) -> ShortcutKeys {
    let mut keys = ShortcutKeys::default();
    let Ok(text) = std::fs::read_to_string(root.join("Config").join(SHORTCUTS_FILE)) else {
        return keys;
    };
    for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
        let shortcut = Shortcut::ALL
            .into_iter()
            .find(|s| s.key().eq_ignore_ascii_case(key.trim()));
        if let (Some(shortcut), Some(binding)) = (shortcut, Binding::parse(value)) {
            keys.set(shortcut, binding);
        }
    }
    keys
}

pub fn write_shortcuts(root: &Path, keys: &ShortcutKeys) -> std::io::Result<()> {
    let text: String = Shortcut::ALL
        .iter()
        .map(|s| format!("{}={}\n", s.key(), Binding::key(keys.get(*s))))
        .collect();
    write(&root.join("Config").join(SHORTCUTS_FILE), &text)
}

fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    atomic_write(path, text.as_bytes())
}

pub fn read_controls(root: &Path) -> Controls {
    std::fs::read_to_string(root.join("Config").join(CONTROLS_FILE))
        .map_or_else(|_| Controls::default(), |text| Controls::parse(&text))
}

pub fn write_controls(root: &Path, c: &Controls) -> std::io::Result<()> {
    write(&root.join("Config").join(CONTROLS_FILE), &c.text())
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CarouselAction {
    PreviousShelf,
    NextShelf,
    Favorite,
    ForgetRecent,
    PreviousCollection,
    NextCollection,
}

impl CarouselAction {
    pub const ALL: [CarouselAction; 6] = [
        CarouselAction::PreviousShelf,
        CarouselAction::NextShelf,
        CarouselAction::Favorite,
        CarouselAction::ForgetRecent,
        CarouselAction::PreviousCollection,
        CarouselAction::NextCollection,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CarouselAction::PreviousShelf => "Previous Shelf",
            CarouselAction::NextShelf => "Next Shelf",
            CarouselAction::Favorite => "Favorite",
            CarouselAction::ForgetRecent => "Remove Recent",
            CarouselAction::PreviousCollection => "Previous",
            CarouselAction::NextCollection => "Next",
        }
    }

    fn key(self) -> &'static str {
        match self {
            CarouselAction::PreviousShelf => "previous_shelf",
            CarouselAction::NextShelf => "next_shelf",
            CarouselAction::Favorite => "favorite",
            CarouselAction::ForgetRecent => "remove_recent",
            CarouselAction::PreviousCollection => "previous_collection",
            CarouselAction::NextCollection => "next_collection",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct CarouselKeys {
    keys: [Option<Binding>; 6],
}

impl Default for CarouselKeys {
    fn default() -> Self {
        CarouselKeys {
            keys: [
                Binding::plain(Device::L1),
                Binding::plain(Device::R1),
                Binding::plain(Device::Y),
                Binding::plain(Device::X),
                Binding::plain(Device::L2),
                Binding::plain(Device::R2),
            ],
        }
    }
}

impl CarouselKeys {
    pub fn get(&self, action: CarouselAction) -> Option<Binding> {
        self.keys[action.index()]
    }

    pub fn set(&mut self, action: CarouselAction, binding: Option<Binding>) {
        self.keys[action.index()] = binding;
    }
}

pub const SHELF_ACTIONS: [CarouselAction; 4] = [
    CarouselAction::PreviousShelf,
    CarouselAction::NextShelf,
    CarouselAction::Favorite,
    CarouselAction::ForgetRecent,
];

pub const COLLECTION_ACTIONS: [CarouselAction; 2] = [
    CarouselAction::PreviousCollection,
    CarouselAction::NextCollection,
];

pub fn read_carousel(root: &Path) -> CarouselKeys {
    let mut keys = CarouselKeys::default();
    let Ok(text) = std::fs::read_to_string(root.join("Config").join(CAROUSEL_FILE)) else {
        return keys;
    };
    for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
        let action = CarouselAction::ALL
            .into_iter()
            .find(|a| a.key().eq_ignore_ascii_case(key.trim()));
        if let (Some(action), Some(binding)) = (action, Binding::parse(value)) {
            keys.set(action, binding);
        }
    }
    keys
}

pub fn write_carousel(root: &Path, keys: &CarouselKeys) -> std::io::Result<()> {
    let text: String = CarouselAction::ALL
        .iter()
        .map(|a| format!("{}={}\n", a.key(), Binding::key(keys.get(*a))))
        .collect();
    write(&root.join("Config").join(CAROUSEL_FILE), &text)
}
