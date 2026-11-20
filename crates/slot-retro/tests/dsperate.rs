use std::path::{Path, PathBuf};

use libloading::Library;

const W: usize = 256;
const SCREEN: usize = W * 192;

type Open = unsafe extern "C" fn(*const std::ffi::c_char) -> *mut u8;
type Load = unsafe extern "C" fn(*mut u8, *const std::ffi::c_char) -> i32;
type Run = unsafe extern "C" fn(*mut u8, u16, u8, u8, i32, i32);
type Screens = unsafe extern "C" fn(*mut u8) -> *const u32;
type StateLoad = unsafe extern "C" fn(*mut u8, *const u8, usize) -> i32;
type Close = unsafe extern "C" fn(*mut u8);

fn shim() -> Option<PathBuf> {
    let name = format!("slot_ds.{}", std::env::consts::DLL_EXTENSION);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../vendor")
        .join(name);
    if !path.exists() {
        eprintln!("skipped: {} is not built", path.display());
        return None;
    }
    Some(path)
}

fn crc16(data: &[u8]) -> u16 {
    let mut v: u16 = 0xFFFF;
    for &b in data {
        v ^= b as u16;
        for _ in 0..8 {
            let carry = v & 1;
            v >>= 1;
            if carry != 0 {
                v ^= 0xA001;
            }
        }
    }
    v
}

pub fn backdrop_rom() -> Vec<u8> {
    rom_with(
        &[
            0xE59F0034, 0xE59F1034, 0xE5801000, 0xE59F0030, 0xE59F1030, 0xE5801000, 0xE59F002C,
            0xE5801000, 0xE59F0028, 0xE59F1028, 0xE1C010B0, 0xE59F0024, 0xE59F1024, 0xE1C010B0,
            0xEAFFFFFE, 0x04000304, 0x00008203, 0x04000000, 0x00010000, 0x04001000, 0x05000000,
            0x0000001F, 0x05000400, 0x000003E0,
        ],
        &[0xEAFFFFFE],
    )
}

pub fn arm7_tick_rom() -> Vec<u8> {
    rom_with(
        &[
            0xE59F0048, 0xE59F1048, 0xE5801000, 0xE3A00301, 0xE3A01801, 0xE5801000, 0xE3A02622,
            0xE3A01000, 0xE5821000, 0xE5923000, 0xE3530000, 0x0AFFFFFC, 0xE3A01E3E, 0xE3530001,
            0x13A0101F, 0xE3A00405, 0xE1C010B0, 0xE59F000C, 0xE1C010B0, 0xEAFFFFFE, 0x04000304,
            0x00008203, 0x05000400,
        ],
        &[
            0xE59F0080, 0xE3A01000, 0xE5801000, 0xE59F4078, 0xE3A01000, 0xE1C410B0, 0xE3A010C0,
            0xE1C410B2, 0xE59F5068, 0xE3A06000, 0xE3A07000, 0xE3A08001, 0xE59FA05C, 0xE1D420B0,
            0xE5953000, 0xE1A09006, 0xE3130008, 0x0A000001, 0xE3120902, 0x02899001, 0xE082B809,
            0xE15B0007, 0x33A08002, 0xE1A0700B, 0xE3130008, 0x0A000002, 0xE3A01008, 0xE5851000,
            0xE2866001, 0xE25AA001, 0x1AFFFFED, 0xE3A00622, 0xE5808000, 0xEAFFFFFE, 0x04000208,
            0x04000100, 0x04000214, 0x000493E0,
        ],
    )
}

fn rom_with(arm9: &[u32], arm7: &[u32]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x10000];
    let put =
        |rom: &mut Vec<u8>, at: usize, v: u32| rom[at..at + 4].copy_from_slice(&v.to_le_bytes());
    rom[..8].copy_from_slice(b"SLOTTEST");
    rom[0x0C..0x10].copy_from_slice(b"SLTT");
    put(&mut rom, 0x20, 0x8000);
    put(&mut rom, 0x24, 0x0200_0000);
    put(&mut rom, 0x28, 0x0200_0000);
    put(&mut rom, 0x2C, 0x100);
    put(&mut rom, 0x30, 0x9000);
    put(&mut rom, 0x34, 0x037F_8000);
    put(&mut rom, 0x38, 0x037F_8000);
    put(&mut rom, 0x3C, (arm7.len() * 4).next_multiple_of(16) as u32);
    put(&mut rom, 0x80, 0x10000);
    put(&mut rom, 0x84, 0x4000);
    rom[0x15C..0x15E].copy_from_slice(&0xCF56u16.to_le_bytes());
    let crc = crc16(&rom[..0x15E]);
    rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
    for (i, w) in arm9.iter().enumerate() {
        put(&mut rom, 0x8000 + i * 4, *w);
    }
    for (i, w) in arm7.iter().enumerate() {
        put(&mut rom, 0x9000 + i * 4, *w);
    }
    rom
}

static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Ds {
    _one: std::sync::MutexGuard<'static, ()>,
    lib: Library,
    ds: *mut u8,
    _dir: tempfile::TempDir,
}

impl Ds {
    fn boot(path: &Path) -> Ds {
        Ds::boot_rom(path, backdrop_rom())
    }

    fn boot_rom(path: &Path, image: Vec<u8>) -> Ds {
        let one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("backdrop.nds");
        std::fs::write(&rom, image).unwrap();
        let lib = unsafe { Library::new(path) }.unwrap();
        let ds = unsafe {
            let open: libloading::Symbol<Open> = lib.get(b"slot_ds_open").unwrap();
            let load: libloading::Symbol<Load> = lib.get(b"slot_ds_load").unwrap();
            let bios = std::ffi::CString::new(dir.path().to_str().unwrap()).unwrap();
            let ds = open(bios.as_ptr());
            assert!(!ds.is_null(), "slot_ds_open failed");
            let rom = std::ffi::CString::new(rom.to_str().unwrap()).unwrap();
            assert_eq!(load(ds, rom.as_ptr()), 0, "slot_ds_load failed");
            ds
        };
        Ds {
            _one: one,
            lib,
            ds,
            _dir: dir,
        }
    }

    fn run(&self, frames: usize) {
        unsafe {
            let run: libloading::Symbol<Run> = self.lib.get(b"slot_ds_run_frame").unwrap();
            for _ in 0..frames {
                run(self.ds, 0, 0, 0, 0, 0);
            }
        }
    }

    fn screens(&self) -> Vec<u32> {
        unsafe {
            let screens: libloading::Symbol<Screens> = self.lib.get(b"slot_ds_screens").unwrap();
            std::slice::from_raw_parts(screens(self.ds), SCREEN * 2).to_vec()
        }
    }
}

impl Drop for Ds {
    fn drop(&mut self) {
        unsafe {
            let close: libloading::Symbol<Close> = self.lib.get(b"slot_ds_close").unwrap();
            close(self.ds);
        }
    }
}

fn uniform(px: &[u32]) -> Option<u32> {
    let first = px[0] & 0x00FF_FFFF;
    px.iter().all(|p| p & 0x00FF_FFFF == first).then_some(first)
}

fn red(c: u32) -> bool {
    (c >> 16) & 0xFF > 0xC0 && (c >> 8) & 0xFF < 0x20 && c & 0xFF < 0x20
}

fn green(c: u32) -> bool {
    (c >> 16) & 0xFF < 0x20 && (c >> 8) & 0xFF > 0xC0 && c & 0xFF < 0x20
}

#[test]
fn shim_boots_a_rom_and_draws_both_screens() {
    let Some(path) = shim() else { return };
    let ds = Ds::boot(&path);
    ds.run(120);
    let px = ds.screens();
    let top = uniform(&px[..SCREEN]).expect("top screen is one backdrop color");
    let bottom = uniform(&px[SCREEN..]).expect("bottom screen is one backdrop color");
    let (r, g) = (red(top) || red(bottom), green(top) || green(bottom));
    assert!(r && g, "top {top:06x} bottom {bottom:06x}");
}

#[test]
fn a_bad_state_leaves_the_game_running() {
    let Some(path) = shim() else { return };
    let ds = Ds::boot(&path);
    ds.run(10);
    let junk: Vec<u8> = (0..64u32)
        .map(|i| (i.wrapping_mul(2654435761) >> 7) as u8)
        .collect();
    let refused = unsafe {
        let load: libloading::Symbol<StateLoad> = ds.lib.get(b"slot_ds_state_load").unwrap();
        load(ds.ds, junk.as_ptr(), junk.len())
    };
    assert_ne!(refused, 0);
    ds.run(120);
    let px = ds.screens();
    assert!(uniform(&px[..SCREEN]).is_some_and(|c| c != 0));
    assert!(uniform(&px[SCREEN..]).is_some_and(|c| c != 0));
}

fn core_on_rom(
    path: &Path,
) -> (
    std::sync::MutexGuard<'static, ()>,
    tempfile::TempDir,
    slot_retro::DsperateCore,
) {
    use slot_retro::RetroCore;
    let one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let rom = dir.path().join("backdrop.nds");
    std::fs::write(&rom, backdrop_rom()).unwrap();
    let mut core = slot_retro::DsperateCore::open(path, dir.path()).unwrap();
    core.load(&rom).unwrap();
    (one, dir, core)
}

#[test]
fn dsperate_core_draws_both_screens_into_one_stacked_frame() {
    use slot_retro::RetroCore;
    let Some(path) = shim() else { return };
    let (_one, _dir, mut core) = core_on_rom(&path);
    assert_eq!(core.video_size(), (256, 384));
    assert!(core.takes_xy());
    for _ in 0..60 {
        core.run_frame(slot_retro::ButtonMask(0));
    }
    let v = core.video_xrgb8888();
    assert_eq!(v.len(), 256 * 384 * 4);
    let px = |i: usize| u32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap());
    let (top, bottom) = (px(0), px(SCREEN));
    assert!(
        (red(top) && green(bottom)) || (green(top) && red(bottom)),
        "{top:06x} {bottom:06x}"
    );
    assert!(core.take_audio().len() % 2 == 0);
}

#[test]
fn dsperate_core_round_trips_a_state() {
    use slot_retro::RetroCore;
    let Some(path) = shim() else { return };
    let (_one, _dir, mut core) = core_on_rom(&path);
    core.run_frame(slot_retro::ButtonMask(0));
    let state = core.serialize().unwrap();
    assert!(!state.is_empty());
    core.unserialize(&state).unwrap();
    core.run_frame(slot_retro::ButtonMask(0));
}

#[test]
fn dsperate_core_refuses_a_bad_state() {
    use slot_retro::RetroCore;
    let Some(path) = shim() else { return };
    let (_one, _dir, mut core) = core_on_rom(&path);
    core.run_frame(slot_retro::ButtonMask(0));
    assert!(core.unserialize(&[1, 2, 3, 4]).is_err());
    for _ in 0..30 {
        core.run_frame(slot_retro::ButtonMask(0));
    }
    assert_ne!(core.video_xrgb8888()[..4], [0, 0, 0, 0]);
}

#[test]
fn dsperate_core_plays_48_khz() {
    use slot_retro::RetroCore;
    let Some(path) = shim() else { return };
    let (_one, _dir, mut core) = core_on_rom(&path);
    let av = core.av_info();
    assert_eq!(av.sample_rate, 48000.0);
    for _ in 0..10 {
        core.run_frame(slot_retro::ButtonMask(0));
        core.take_audio();
    }
    let mut frames = 0;
    for _ in 0..120 {
        core.run_frame(slot_retro::ButtonMask(0));
        frames += core.take_audio().len() / 2;
    }
    let want = 120.0 * 48000.0 / av.fps;
    assert!(
        (frames as f64 - want).abs() < want * 0.02,
        "{frames} frames, want about {want:.0}"
    );
}

#[test]
fn a_second_live_dsperate_core_is_refused() {
    let Some(path) = shim() else { return };
    let (_one, dir, _core) = core_on_rom(&path);
    assert!(slot_retro::DsperateCore::open(&path, dir.path()).is_err());
}

#[test]
fn a_timer_read_never_runs_ahead_of_its_overflow_flag() {
    let Some(path) = shim() else { return };
    let ds = Ds::boot_rom(&path, arm7_tick_rom());
    ds.run(600);
    let px = ds.screens();
    let c = uniform(&px[..SCREEN]).expect("the test program did not finish");
    assert!(
        green(c),
        "{c:06x}: a timer read returned a wrapped counter before its overflow flag, so an SDK tick went backwards"
    );
}
