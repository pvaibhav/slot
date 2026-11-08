use std::path::{Path, PathBuf};

use slot_input::{Btn, RawEvent};

pub const EVENT_BYTES: usize = 24;

pub const EV_SYN: u16 = 0x00;
pub const EV_KEY: u16 = 0x01;
pub const EV_SW: u16 = 0x05;
pub const SW_LID: u16 = 0x00;

pub const EV_ABS: u16 = 0x03;
pub const ABS_HAT0X: u16 = 0x10;
pub const ABS_HAT0Y: u16 = 0x11;
pub const ABS_LSX: u16 = 0x02;
pub const ABS_LSY: u16 = 0x03;
pub const ABS_X: u16 = 0x00;
pub const ABS_Y: u16 = 0x01;

pub const STICK_DEADZONE: i32 = 2048;
pub const DSOS_STICK_DEADZONE: i32 = 900;

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum Keymap {
    #[default]
    BaseOs,
    DsOs,
}

impl Keymap {
    pub fn from_is_dsos(flag: Option<&str>) -> Keymap {
        match flag {
            Some("1") => Keymap::DsOs,
            _ => Keymap::BaseOs,
        }
    }

    pub fn of_system() -> Keymap {
        Keymap::from_is_dsos(crate::baseos::var(IS_DSOS).as_deref())
    }

    pub fn of_release(env: Option<String>, release: &Path) -> Keymap {
        Keymap::from_is_dsos(crate::baseos::var_in(IS_DSOS, env, release).as_deref())
    }

    pub fn btn(self, code: u16) -> Option<Btn> {
        match self {
            Keymap::BaseOs => code_to_btn(code),
            Keymap::DsOs => dsos_code_to_btn(code),
        }
    }

    pub fn to_raw(self, ev: Ev) -> Option<RawEvent> {
        let btn = match ev.kind {
            EV_KEY => self.btn(ev.code)?,
            EV_SW if ev.code == SW_LID => Btn::Lid,
            _ => return None,
        };
        match ev.value {
            0 => Some(RawEvent::Up(btn)),
            1 => Some(RawEvent::Down(btn)),
            _ => None,
        }
    }

    pub fn held_from_bits(self, bits: &[u8]) -> Vec<Btn> {
        bits.iter()
            .enumerate()
            .flat_map(|(i, byte)| {
                (0..8)
                    .filter(move |b| byte & (1 << b) != 0)
                    .map(move |b| i * 8 + b)
            })
            .filter_map(|code| u16::try_from(code).ok().and_then(|c| self.btn(c)))
            .collect()
    }

    pub fn held_keys(self, node: &Path) -> Vec<Btn> {
        use std::os::unix::io::AsRawFd;
        let Ok(file) = std::fs::File::open(node) else {
            return Vec::new();
        };
        let mut bits = [0u8; KEY_MAX / 8 + 1];
        let len = bits.len() as u64;
        let request = (2u64 << 30) | (len << 16) | ((b'E' as u64) << 8) | EVIOCGKEY_NR;
        let read = unsafe { libc::ioctl(file.as_raw_fd(), request as _, bits.as_mut_ptr()) };
        match read < 0 {
            true => Vec::new(),
            false => self.held_from_bits(&bits),
        }
    }

    pub fn pick_devices(self, dev: &Path, sys: &Path) -> Vec<PathBuf> {
        let Ok(entries) = std::fs::read_dir(dev) else {
            return Vec::new();
        };
        let mut nodes: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("event"))
            })
            .collect();
        nodes.sort();
        nodes.retain(|node| {
            let Some(name) = node.file_name() else {
                return false;
            };
            self.wanted_node(&sys.join(name).join("device"))
        });
        nodes
    }

    fn wanted_node(self, device: &Path) -> bool {
        let cap = |file: &str| std::fs::read_to_string(device.join("capabilities").join(file));
        let keys = cap("key").unwrap_or_default();
        let lid = cap("sw").is_ok_and(|sw| has_bit(&sw, SW_LID));
        let wanted: &[u16] = match self {
            Keymap::BaseOs => &WANTED,
            Keymap::DsOs => &DSOS_WANTED,
        };
        lid || wanted.iter().any(|bit| has_bit(&keys, *bit))
    }

    pub fn has_stick(self, target: &str) -> bool {
        self == Keymap::DsOs || has_left_stick(target)
    }

    pub fn stick_axes(self) -> [u16; 2] {
        match self {
            Keymap::BaseOs => [ABS_LSX, ABS_LSY],
            Keymap::DsOs => [ABS_X, ABS_Y],
        }
    }

    fn stick_deadzone(self) -> i32 {
        match self {
            Keymap::BaseOs => STICK_DEADZONE,
            Keymap::DsOs => DSOS_STICK_DEADZONE,
        }
    }
}

const IS_DSOS: &str = "IS_DSOS";

const LEFT_STICK_TARGETS: [&str; 6] = [
    "rg34xxsp",
    "rg35xxh",
    "rg35xxpro",
    "rg40xxh",
    "rg40xxv",
    "rgcubexx",
];

pub fn has_left_stick(target: &str) -> bool {
    LEFT_STICK_TARGETS.contains(&target)
}

#[derive(Default)]
pub struct Hat {
    keymap: Keymap,
    hat: [i32; 2],
    stick: [i32; 2],
    steers: bool,
}

impl Hat {
    pub fn new(keymap: Keymap) -> Hat {
        Hat {
            keymap,
            ..Hat::default()
        }
    }

    pub fn takes(&self, ev: Ev) -> bool {
        match ev.kind {
            EV_ABS => true,
            EV_KEY => self.key_axis(ev.code).is_some(),
            _ => false,
        }
    }

    fn key_axis(&self, code: u16) -> Option<(usize, i32)> {
        match self.keymap.btn(code)? {
            Btn::Up => Some((1, -1)),
            Btn::Down => Some((1, 1)),
            Btn::Left => Some((0, -1)),
            Btn::Right => Some((0, 1)),
            _ => None,
        }
    }

    pub fn steer_with_stick(&mut self, on: bool) {
        self.steers = on;
    }

    pub fn feed(&mut self, ev: Ev) -> Vec<RawEvent> {
        let deadzone = self.keymap.stick_deadzone();
        let tilt = |v: i32| match v {
            v if v > deadzone => 1,
            v if v < -deadzone => -1,
            _ => 0,
        };
        let steer = |v: i32| if self.steers { tilt(v) } else { 0 };
        let [stick_x, stick_y] = self.keymap.stick_axes();
        let (axis, from_stick, value) = match ev.kind {
            EV_KEY => {
                let Some((axis, way)) = self.key_axis(ev.code) else {
                    return Vec::new();
                };
                match ev.value {
                    1 => (axis, false, way),
                    0 if self.hat[axis] == way => (axis, false, 0),
                    _ => return Vec::new(),
                }
            }
            EV_ABS => match ev.code {
                ABS_HAT0X => (0, false, ev.value.signum()),
                ABS_HAT0Y => (1, false, ev.value.signum()),
                c if c == stick_x => (0, true, steer(ev.value)),
                c if c == stick_y => (1, true, steer(ev.value)),
                _ => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        let was = self.held(axis);
        match from_stick {
            true => self.stick[axis] = value,
            false => self.hat[axis] = value,
        }
        let now = self.held(axis);
        if now == was {
            return Vec::new();
        }
        let ends = [[Btn::Left, Btn::Right], [Btn::Up, Btn::Down]][axis];
        let btn = |v: i32| ends[usize::from(v > 0)];
        let mut out = Vec::new();
        if was != 0 {
            out.push(RawEvent::Up(btn(was)));
        }
        if now != 0 {
            out.push(RawEvent::Down(btn(now)));
        }
        out
    }

    fn held(&self, axis: usize) -> i32 {
        match self.hat[axis] {
            0 => self.stick[axis],
            hat => hat,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Ev {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}

pub fn decode(bytes: &[u8]) -> Option<Ev> {
    let packet: &[u8; EVENT_BYTES] = bytes.get(..EVENT_BYTES)?.try_into().ok()?;
    Some(Ev {
        kind: u16::from_le_bytes([packet[16], packet[17]]),
        code: u16::from_le_bytes([packet[18], packet[19]]),
        value: i32::from_le_bytes([packet[20], packet[21], packet[22], packet[23]]),
    })
}

const KEY_MAX: usize = 0x2ff;
const EVIOCGKEY_NR: u64 = 0x18;

pub fn dsos_code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        0x131 => Btn::A,
        0x130 => Btn::B,
        0x133 => Btn::X,
        0x134 => Btn::Y,
        0x136 => Btn::L1,
        0x137 => Btn::R1,
        0x138 => Btn::L2,
        0x139 => Btn::R2,
        0x13a => Btn::Select,
        0x13b => Btn::Start,
        0x135 => Btn::Menu,
        0x220 => Btn::Up,
        0x221 => Btn::Down,
        0x222 => Btn::Left,
        0x223 => Btn::Right,
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

const DSOS_WANTED: [u16; 18] = [
    0x130, 0x131, 0x133, 0x134, 0x135, 0x136, 0x137, 0x138, 0x139, 0x13a, 0x13b, 0x220, 0x221,
    0x222, 0x223, 115, 114, 116,
];

pub fn code_to_btn(code: u16) -> Option<Btn> {
    Some(match code {
        0x130 => Btn::A,
        0x131 => Btn::B,
        0x132 => Btn::Y,
        0x133 => Btn::X,
        0x134 => Btn::L1,
        0x135 => Btn::R1,
        0x136 => Btn::Select,
        0x137 => Btn::Start,
        0x138 => Btn::Menu,
        0x13a => Btn::L2,
        0x13b => Btn::R2,
        115 => Btn::VolUp,
        114 => Btn::VolDown,
        116 => Btn::Power,
        _ => return None,
    })
}

const WANTED: [u16; 15] = [
    0x130, 0x131, 0x132, 0x133, 0x134, 0x135, 0x136, 0x137, 0x138, 0x13a, 0x13b, 0x162, 115, 114,
    116,
];

pub use slot_power::has_bit;

pub fn device_name(sys: &Path, node: &Path) -> String {
    node.file_name()
        .and_then(|n| std::fs::read_to_string(sys.join(n).join("device/name")).ok())
        .map(|n| n.trim().to_string())
        .unwrap_or_else(|| node.display().to_string())
}
