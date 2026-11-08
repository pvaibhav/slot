use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slot::audio::{AudioSink, StubSink};
use slot::emu::{CoreState, EmuHandle, Speed};
use slot_retro::{AvInfo, ButtonMask, CoreError, MockCore, RetroCore};

fn wait_for(cond: impl Fn() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    false
}

fn spawn(core: Box<dyn RetroCore>) -> EmuHandle {
    let mut sink = StubSink::new();
    sink.open(32_768).expect("the stub refused to open");
    let drain = sink.clone();
    std::thread::spawn(move || loop {
        drain.device_drain();
        std::thread::sleep(Duration::from_millis(2));
    });
    let emu = EmuHandle::spawn(core, PathBuf::from("mock"), sink.ring(), None, None);
    emu.set_speed(Speed::Normal);
    assert!(wait_for(|| emu.state() == CoreState::Ready));
    emu
}

#[test]
fn a_ds_game_publishes_256_by_384_frames() {
    let emu = spawn(Box::new(MockCore::with_size(256, 384)));
    assert!(wait_for(|| emu.has_published()));
    let frame = emu.latest_frame().expect("a frame");
    assert_eq!(frame.len(), 256 * 384 * 4);
}

#[derive(Debug, Clone, PartialEq)]
enum Seen {
    Touch(Option<(u8, u8)>),
    Lid(bool),
    Frame(u16),
}

struct Recorder {
    inner: MockCore,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl RetroCore for Recorder {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.inner.load(rom)
    }
    fn run_frame(&mut self, input: ButtonMask) {
        self.seen.lock().unwrap().push(Seen::Frame(input.0));
        self.inner.run_frame(input)
    }
    fn video_size(&self) -> (u32, u32) {
        self.inner.video_size()
    }
    fn set_touch(&mut self, at: Option<(u8, u8)>) {
        self.seen.lock().unwrap().push(Seen::Touch(at));
    }
    fn set_lid(&mut self, shut: bool) {
        self.seen.lock().unwrap().push(Seen::Lid(shut));
    }
    fn takes_xy(&self) -> bool {
        true
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.inner.video_xrgb8888()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        self.inner.take_audio()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        self.inner.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.inner.unserialize(data)
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.inner.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.inner.load_save_ram(data)
    }
    fn av_info(&self) -> AvInfo {
        self.inner.av_info()
    }
}

fn recorder() -> (EmuHandle, Arc<Mutex<Vec<Seen>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let emu = spawn(Box::new(Recorder {
        inner: MockCore::with_size(256, 384),
        seen: seen.clone(),
    }));
    (emu, seen)
}

#[test]
fn touch_and_lid_reach_the_core_before_the_frame() {
    let (emu, seen) = recorder();
    emu.set_touch(Some((10, 20)));
    emu.set_lid(true);
    assert!(wait_for(|| {
        let s = seen.lock().unwrap();
        let touch = s.iter().position(|e| *e == Seen::Touch(Some((10, 20))));
        let lid = s.iter().position(|e| *e == Seen::Lid(true));
        match (touch, lid) {
            (Some(t), Some(l)) => s[t.max(l)..].iter().any(|e| matches!(e, Seen::Frame(_))),
            _ => false,
        }
    }));
}

#[test]
fn x_and_y_reach_a_ds_core_as_x_and_y() {
    let (emu, seen) = recorder();
    let held = ButtonMask::X | ButtonMask::Y;
    emu.set_input(ButtonMask(held));
    assert!(wait_for(|| seen
        .lock()
        .unwrap()
        .iter()
        .filter(|e| **e == Seen::Frame(held))
        .count()
        >= 6));
    assert!(!seen
        .lock()
        .unwrap()
        .iter()
        .any(|e| matches!(e, Seen::Frame(m) if m & ButtonMask::A != 0)));
}

struct Slow(MockCore);

impl RetroCore for Slow {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.0.load(rom)
    }
    fn run_frame(&mut self, input: ButtonMask) {
        std::thread::sleep(Duration::from_millis(10));
        self.0.run_frame(input)
    }
    fn video_size(&self) -> (u32, u32) {
        self.0.video_size()
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.0.video_xrgb8888()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        self.0.take_audio()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        self.0.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.0.unserialize(data)
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.0.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.0.load_save_ram(data)
    }
    fn av_info(&self) -> AvInfo {
        self.0.av_info()
    }
}

#[test]
fn stepping_ahead_overlaps_the_core_with_the_caller() {
    let emu = spawn(Box::new(Slow(MockCore::with_size(256, 384))));
    emu.set_driven(true);
    for _ in 0..5 {
        emu.step_ahead(Duration::from_millis(40));
        std::thread::sleep(Duration::from_millis(12));
    }
    let mut waited = Duration::ZERO;
    let mut late = 0;
    for _ in 0..20 {
        let began = Instant::now();
        late += u32::from(emu.step_ahead(Duration::from_millis(40)));
        waited += began.elapsed();
        std::thread::sleep(Duration::from_millis(12));
    }
    assert_eq!(late, 0);
    assert!(
        waited / 20 < Duration::from_millis(4),
        "step_ahead waited {:?} a frame: the core is not running while the caller renders",
        waited / 20
    );
}

struct Counted(MockCore, Arc<Mutex<u32>>);

impl RetroCore for Counted {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.0.load(rom)
    }
    fn run_frame(&mut self, input: ButtonMask) {
        let mut n = self.1.lock().unwrap();
        *n += 1;
        let slow = (*n).is_multiple_of(5);
        drop(n);
        std::thread::sleep(Duration::from_millis(if slow { 40 } else { 2 }));
        self.0.run_frame(input)
    }
    fn video_size(&self) -> (u32, u32) {
        self.0.video_size()
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.0.video_xrgb8888()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        self.0.take_audio()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        self.0.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.0.unserialize(data)
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.0.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.0.load_save_ram(data)
    }
    fn av_info(&self) -> AvInfo {
        self.0.av_info()
    }
}

#[test]
fn a_locked_core_that_falls_behind_runs_every_frame_it_was_ticked_for() {
    let frames = Arc::new(Mutex::new(0u32));
    let emu = spawn(Box::new(Counted(
        MockCore::with_size(256, 384),
        frames.clone(),
    )));
    emu.set_driven(true);
    assert!(wait_for(|| emu.locked()));
    emu.step_ahead(Duration::from_millis(100));
    assert!(emu.wait_frame(Duration::from_millis(200)));
    let before = *frames.lock().unwrap();
    let ticks = 60;
    for _ in 0..ticks {
        emu.tick();
        std::thread::sleep(Duration::from_millis(16));
    }
    assert!(emu.wait_frame(Duration::from_millis(300)));
    let ran = *frames.lock().unwrap() - before;
    assert!(ran + 1 >= ticks, "{ticks} ticks ran {ran} frames: the ones it fell behind on were dropped, and their sound with them");
}
