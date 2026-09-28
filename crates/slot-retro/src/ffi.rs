use std::ffi::{c_char, c_int, c_uint, c_void};

use libloading::Library;

use crate::core::CoreError;

pub const API_VERSION: c_uint = 1;

pub const GET_CAN_DUPE: c_uint = 3;
pub const GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const SET_PIXEL_FORMAT: c_uint = 10;
pub const GET_VARIABLE: c_uint = 15;
pub const SET_VARIABLES: c_uint = 16;
pub const GET_VARIABLE_UPDATE: c_uint = 17;
pub const GET_RUMBLE_INTERFACE: c_uint = 23;
pub const GET_LOG_INTERFACE: c_uint = 27;
pub const GET_SAVE_DIRECTORY: c_uint = 31;
pub const SET_MEMORY_MAPS: c_uint = 36 | 0x10000;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryDescriptor {
    pub flags: u64,
    pub ptr: *mut c_void,
    pub offset: usize,
    pub start: usize,
    pub select: usize,
    pub disconnect: usize,
    pub len: usize,
    pub addrspace: *const c_char,
}

// Descriptors move with the exclusively owned LibretroCore. They are read only between
// calls on its emulator thread and are discarded before the core's memory is freed.
unsafe impl Send for MemoryDescriptor {}

#[repr(C)]
pub struct MemoryMap {
    pub descriptors: *const MemoryDescriptor,
    pub num_descriptors: c_uint,
}
pub const SET_AUDIO_BUFFER_STATUS_CALLBACK: c_uint = 62;
pub const SET_NETPACKET_INTERFACE: c_uint = 78;

pub const RUMBLE_STRONG: c_uint = 0;
pub const RUMBLE_WEAK: c_uint = 1;

pub const PIXEL_FORMAT_XRGB8888: c_uint = 1;
pub const PIXEL_FORMAT_RGB565: c_uint = 2;
pub const DEVICE_JOYPAD: c_uint = 1;
pub const MEMORY_SAVE_RAM: c_uint = 0;
pub const JOYPAD_MASK: c_uint = 256;

pub const NETPACKET_UNRELIABLE: i32 = 0;
pub const NETPACKET_RELIABLE: i32 = 1 << 0;
pub const NETPACKET_UNSEQUENCED: i32 = 1 << 1;
pub const NETPACKET_FLUSH_HINT: i32 = 1 << 2;
pub const NETPACKET_BROADCAST: u16 = 0xFFFF;

#[repr(C)]
pub struct GameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

#[repr(C)]
#[derive(Default)]
pub struct GameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Default)]
pub struct SystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Default)]
pub struct SystemAvInfo {
    pub geometry: GameGeometry,
    pub timing: SystemTiming,
}

#[repr(C)]
pub struct Variable {
    pub key: *const c_char,
    pub value: *const c_char,
}

#[repr(C)]
pub struct LogCallback {
    pub log: *const c_void,
}

pub type SetRumbleStateFn = unsafe extern "C" fn(c_uint, c_uint, u16) -> bool;

#[repr(C)]
pub struct RumbleInterface {
    pub set_rumble_state: SetRumbleStateFn,
}

pub type AudioBufferStatusFn =
    unsafe extern "C" fn(active: bool, occupancy: c_uint, underrun_likely: bool);

#[repr(C)]
pub struct AudioBufferStatusCallback {
    pub callback: Option<AudioBufferStatusFn>,
}

pub type NetpacketSend =
    unsafe extern "C" fn(flags: c_int, buf: *const c_void, len: usize, client_id: u16);
pub type NetpacketPollReceive = unsafe extern "C" fn();

pub type NetpacketStart =
    unsafe extern "C" fn(client_id: u16, send: NetpacketSend, poll_receive: NetpacketPollReceive);
pub type NetpacketReceive = unsafe extern "C" fn(buf: *const c_void, len: usize, client_id: u16);
pub type NetpacketStop = unsafe extern "C" fn();
pub type NetpacketPoll = unsafe extern "C" fn();
pub type NetpacketConnected = unsafe extern "C" fn(client_id: u16) -> bool;
pub type NetpacketDisconnected = unsafe extern "C" fn(client_id: u16);

#[repr(C)]
pub struct NetpacketCallback {
    pub start: Option<NetpacketStart>,
    pub receive: Option<NetpacketReceive>,
    pub stop: Option<NetpacketStop>,
    pub poll: Option<NetpacketPoll>,
    pub connected: Option<NetpacketConnected>,
    pub disconnected: Option<NetpacketDisconnected>,
    pub protocol_version: *const c_char,
}

// SAFETY: besides function pointers, `protocol_version` points at a string literal in the
unsafe impl Send for NetpacketCallback {}

pub type EnvironmentFn = unsafe extern "C" fn(c_uint, *mut c_void) -> bool;
pub type VideoRefreshFn = unsafe extern "C" fn(*const c_void, c_uint, c_uint, usize);
pub type AudioSampleFn = unsafe extern "C" fn(i16, i16);
pub type AudioBatchFn = unsafe extern "C" fn(*const i16, usize) -> usize;
pub type InputPollFn = unsafe extern "C" fn();
pub type InputStateFn = unsafe extern "C" fn(c_uint, c_uint, c_uint, c_uint) -> i16;

pub struct Api {
    pub init: unsafe extern "C" fn(),
    pub deinit: unsafe extern "C" fn(),
    pub api_version: unsafe extern "C" fn() -> c_uint,
    pub get_system_av_info: unsafe extern "C" fn(*mut SystemAvInfo),
    pub set_environment: unsafe extern "C" fn(EnvironmentFn),
    pub set_video_refresh: unsafe extern "C" fn(VideoRefreshFn),
    pub set_audio_sample: unsafe extern "C" fn(AudioSampleFn),
    pub set_audio_sample_batch: unsafe extern "C" fn(AudioBatchFn),
    pub set_input_poll: unsafe extern "C" fn(InputPollFn),
    pub set_input_state: unsafe extern "C" fn(InputStateFn),
    pub set_controller_port_device: unsafe extern "C" fn(c_uint, c_uint),
    pub load_game: unsafe extern "C" fn(*const GameInfo) -> bool,
    pub unload_game: unsafe extern "C" fn(),
    pub run: unsafe extern "C" fn(),
    pub serialize_size: unsafe extern "C" fn() -> usize,
    pub serialize: unsafe extern "C" fn(*mut c_void, usize) -> bool,
    pub unserialize: unsafe extern "C" fn(*const c_void, usize) -> bool,
    pub get_memory_data: unsafe extern "C" fn(c_uint) -> *mut c_void,
    pub get_memory_size: unsafe extern "C" fn(c_uint) -> usize,
}

impl Api {
    pub unsafe fn load(lib: &Library) -> Result<Self, CoreError> {
        macro_rules! get {
            ($name:literal) => {
                *lib.get(concat!($name, "\0").as_bytes())
                    .map_err(|e| CoreError::Load(format!("{}: {e}", $name)))?
            };
        }
        Ok(Api {
            init: get!("retro_init"),
            deinit: get!("retro_deinit"),
            api_version: get!("retro_api_version"),
            get_system_av_info: get!("retro_get_system_av_info"),
            set_environment: get!("retro_set_environment"),
            set_video_refresh: get!("retro_set_video_refresh"),
            set_audio_sample: get!("retro_set_audio_sample"),
            set_audio_sample_batch: get!("retro_set_audio_sample_batch"),
            set_input_poll: get!("retro_set_input_poll"),
            set_input_state: get!("retro_set_input_state"),
            set_controller_port_device: get!("retro_set_controller_port_device"),
            load_game: get!("retro_load_game"),
            unload_game: get!("retro_unload_game"),
            run: get!("retro_run"),
            serialize_size: get!("retro_serialize_size"),
            serialize: get!("retro_serialize"),
            unserialize: get!("retro_unserialize"),
            get_memory_data: get!("retro_get_memory_data"),
            get_memory_size: get!("retro_get_memory_size"),
        })
    }
}
