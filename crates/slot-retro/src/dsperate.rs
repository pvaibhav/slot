use std::ffi::{c_char, CString};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use libloading::Library;

use crate::core::{AvInfo, ButtonMask, CoreError, RetroCore};

pub const DS_W: u32 = 256;
pub const DS_H: u32 = 384;
const ABI: u32 = 1;
const AUDIO_CHUNK: usize = 2048;

static LIVE: AtomicBool = AtomicBool::new(false);

type Handle = *mut u8;

struct Api {
    abi: unsafe extern "C" fn() -> u32,
    fps: unsafe extern "C" fn() -> f64,
    sample_rate: unsafe extern "C" fn() -> f64,
    open: unsafe extern "C" fn(*const c_char) -> Handle,
    load: unsafe extern "C" fn(Handle, *const c_char) -> i32,
    reset: unsafe extern "C" fn(Handle),
    run_frame: unsafe extern "C" fn(Handle, u16, u8, u8, i32, i32),
    skip_draw: unsafe extern "C" fn(Handle, i32),
    screens: unsafe extern "C" fn(Handle) -> *const u32,
    take_audio: unsafe extern "C" fn(Handle, *mut i16, usize) -> usize,
    save_size: unsafe extern "C" fn(Handle) -> usize,
    save_read: unsafe extern "C" fn(Handle, *mut u8, usize) -> usize,
    save_write: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    state_save: unsafe extern "C" fn(Handle, *mut *const u8) -> usize,
    state_load: unsafe extern "C" fn(Handle, *const u8, usize) -> i32,
    close: unsafe extern "C" fn(Handle),
}

impl Api {
    fn load(lib: &Library) -> Result<Api, CoreError> {
        unsafe fn sym<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, CoreError> {
            lib.get::<T>(name)
                .map(|s| *s)
                .map_err(|e| CoreError::Load(format!("{}: {e}", String::from_utf8_lossy(name))))
        }
        unsafe {
            Ok(Api {
                abi: sym(lib, b"slot_ds_abi")?,
                fps: sym(lib, b"slot_ds_fps")?,
                sample_rate: sym(lib, b"slot_ds_sample_rate")?,
                open: sym(lib, b"slot_ds_open")?,
                load: sym(lib, b"slot_ds_load")?,
                reset: sym(lib, b"slot_ds_reset")?,
                run_frame: sym(lib, b"slot_ds_run_frame")?,
                skip_draw: sym(lib, b"slot_ds_skip_draw")?,
                screens: sym(lib, b"slot_ds_screens")?,
                take_audio: sym(lib, b"slot_ds_take_audio")?,
                save_size: sym(lib, b"slot_ds_save_size")?,
                save_read: sym(lib, b"slot_ds_save_read")?,
                save_write: sym(lib, b"slot_ds_save_write")?,
                state_save: sym(lib, b"slot_ds_state_save")?,
                state_load: sym(lib, b"slot_ds_state_load")?,
                close: sym(lib, b"slot_ds_close")?,
            })
        }
    }
}

pub struct DsperateCore {
    api: Api,
    ds: Handle,
    video: Vec<u8>,
    touch: Option<(u8, u8)>,
    lid: bool,
    _lib: Library,
}

unsafe impl Send for DsperateCore {}

fn c_path(path: &Path) -> Result<CString, CoreError> {
    CString::new(path.as_os_str().to_string_lossy().as_bytes())
        .map_err(|_| CoreError::Load(format!("{}: has a NUL byte", path.display())))
}

pub fn ds_buttons(mask: ButtonMask) -> u16 {
    const MAP: [(u16, u16); 12] = [
        (ButtonMask::A, 0),
        (ButtonMask::B, 1),
        (ButtonMask::SELECT, 2),
        (ButtonMask::START, 3),
        (ButtonMask::RIGHT, 4),
        (ButtonMask::LEFT, 5),
        (ButtonMask::UP, 6),
        (ButtonMask::DOWN, 7),
        (ButtonMask::R, 8),
        (ButtonMask::L, 9),
        (ButtonMask::X, 10),
        (ButtonMask::Y, 11),
    ];
    MAP.iter()
        .filter(|(bit, _)| mask.0 & bit != 0)
        .fold(0, |out, (_, ds)| out | 1 << ds)
}

impl DsperateCore {
    pub fn open(shim: &Path, bios: &Path) -> Result<DsperateCore, CoreError> {
        if LIVE.swap(true, Ordering::AcqRel) {
            return Err(CoreError::Unsupported(
                "a DSperate core is already running in this process".into(),
            ));
        }
        let opened = Self::open_unguarded(shim, bios);
        if opened.is_err() {
            LIVE.store(false, Ordering::Release);
        }
        opened
    }

    fn open_unguarded(shim: &Path, bios: &Path) -> Result<DsperateCore, CoreError> {
        let lib = unsafe { Library::new(shim) }
            .map_err(|e| CoreError::Load(format!("{}: {e}", shim.display())))?;
        let api = Api::load(&lib)?;
        let abi = unsafe { (api.abi)() };
        if abi != ABI {
            return Err(CoreError::Load(format!(
                "{}: shim ABI {abi}, slot wants {ABI}",
                shim.display()
            )));
        }
        let dir = c_path(bios)?;
        let ds = unsafe { (api.open)(dir.as_ptr()) };
        if ds.is_null() {
            return Err(CoreError::Load("slot_ds_open failed".into()));
        }
        Ok(DsperateCore {
            api,
            ds,
            video: vec![0; (DS_W * DS_H * 4) as usize],
            touch: None,
            lid: false,
            _lib: lib,
        })
    }

    fn copy_video(&mut self) {
        let px = unsafe {
            std::slice::from_raw_parts((self.api.screens)(self.ds), (DS_W * DS_H) as usize)
        };
        for (out, p) in self.video.chunks_exact_mut(4).zip(px) {
            out.copy_from_slice(&p.to_le_bytes());
        }
    }
}

impl Drop for DsperateCore {
    fn drop(&mut self) {
        unsafe { (self.api.close)(self.ds) };
        LIVE.store(false, Ordering::Release);
    }
}

impl RetroCore for DsperateCore {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        let path = c_path(rom)?;
        match unsafe { (self.api.load)(self.ds, path.as_ptr()) } {
            0 => {
                self.copy_video();
                Ok(())
            }
            _ => Err(CoreError::Load(format!(
                "{}: DSperate could not load it",
                rom.display()
            ))),
        }
    }

    fn run_frame(&mut self, input: ButtonMask) {
        let (x, y) = self.touch.unwrap_or((0, 0));
        unsafe {
            (self.api.run_frame)(
                self.ds,
                ds_buttons(input),
                x,
                y,
                self.touch.is_some() as i32,
                self.lid as i32,
            )
        };
        self.copy_video();
    }

    fn reset(&mut self) {
        unsafe { (self.api.reset)(self.ds) };
        self.copy_video();
    }

    fn set_frame_skip(&mut self, skip: bool) {
        unsafe { (self.api.skip_draw)(self.ds, skip as i32) };
    }

    fn video_size(&self) -> (u32, u32) {
        (DS_W, DS_H)
    }

    fn set_touch(&mut self, at: Option<(u8, u8)>) {
        self.touch = at;
    }

    fn set_lid(&mut self, shut: bool) {
        self.lid = shut;
    }

    fn takes_xy(&self) -> bool {
        true
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.video
    }

    fn take_audio(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        let mut chunk = vec![0i16; AUDIO_CHUNK * 2];
        loop {
            let got = unsafe { (self.api.take_audio)(self.ds, chunk.as_mut_ptr(), AUDIO_CHUNK) };
            out.extend_from_slice(&chunk[..got * 2]);
            if got < AUDIO_CHUNK {
                return out;
            }
        }
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        let mut at: *const u8 = std::ptr::null();
        let n = unsafe { (self.api.state_save)(self.ds, &mut at) };
        if n == 0 || at.is_null() {
            return Err(CoreError::State("DSperate could not save a state".into()));
        }
        Ok(unsafe { std::slice::from_raw_parts(at, n) }.to_vec())
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let refused = unsafe { (self.api.state_load)(self.ds, data.as_ptr(), data.len()) };
        self.copy_video();
        match refused {
            0 => Ok(()),
            _ => Err(CoreError::State("DSperate refused the state".into())),
        }
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        let n = unsafe { (self.api.save_size)(self.ds) };
        if n == 0 {
            return None;
        }
        let mut out = vec![0u8; n];
        let got = unsafe { (self.api.save_read)(self.ds, out.as_mut_ptr(), n) };
        out.truncate(got);
        Some(out)
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        match unsafe { (self.api.save_write)(self.ds, data.as_ptr(), data.len()) } {
            0 => Ok(()),
            _ => Err(CoreError::State("no DS game is loaded".into())),
        }
    }

    fn av_info(&self) -> AvInfo {
        unsafe {
            AvInfo {
                fps: (self.api.fps)(),
                sample_rate: (self.api.sample_rate)(),
            }
        }
    }
}
