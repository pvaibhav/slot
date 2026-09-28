use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};

use slot_retro::{AvInfo, ButtonMask, CoreError, Link, RetroCore, Rumble};

use crate::{network, Control, Frame, Service, QUEUE_SIZE, RAM_SIZE};

static NEXT_GAME: AtomicU64 = AtomicU64::new(1);

pub(crate) struct Tracked {
    core: Box<dyn RetroCore>,
    controls: mpsc::Sender<Control>,
    frames: mpsc::SyncSender<Frame>,
    recycled: mpsc::Receiver<Vec<u8>>,
    recycle: mpsc::SyncSender<Vec<u8>>,
    enabled: Arc<AtomicBool>,
    current: Arc<AtomicU64>,
    generation: u64,
    sequence: u64,
    timeline: u64,
    allocated: usize,
}

impl Tracked {
    pub fn new(core: Box<dyn RetroCore>, service: &Service) -> Self {
        let (recycle, recycled) = mpsc::sync_channel(QUEUE_SIZE + 1);
        Self {
            core,
            controls: service.controls.clone(),
            frames: service.frames.clone(),
            recycled,
            recycle,
            enabled: service.enabled.clone(),
            current: service.current.clone(),
            generation: 0,
            sequence: 0,
            timeline: 0,
            allocated: 0,
        }
    }

    fn snapshot(&mut self) {
        self.sequence += 1;
        if !self.enabled.load(Ordering::Acquire) {
            return;
        }
        let mut ram = match self.recycled.try_recv() {
            Ok(ram) => ram,
            Err(_) if self.allocated < QUEUE_SIZE + 1 => {
                self.allocated += 1;
                vec![0; RAM_SIZE]
            }
            Err(_) => return,
        };
        let valid = self.core.achievement_memory(&mut ram);
        let frame = Frame {
            generation: self.generation,
            sequence: self.sequence,
            timeline: self.timeline,
            earned_at: network::now(),
            ram,
            valid,
            recycle: self.recycle.clone(),
        };
        // In particular, never use send(): HTTPS, fsync, or evaluation must not backpressure
        // gameplay. Sequence gaps reset the worker's hit/delta state before it proceeds.
        if let Err(error) = self.frames.try_send(frame) {
            let (mpsc::TrySendError::Full(frame) | mpsc::TrySendError::Disconnected(frame)) = error;
            let _ = self.recycle.try_send(frame.ram);
        }
    }
}

impl RetroCore for Tracked {
    fn load(&mut self, rom: &Path) -> Result<(), CoreError> {
        self.core.load(rom)?;
        self.generation = NEXT_GAME.fetch_add(1, Ordering::Relaxed);
        self.current.store(self.generation, Ordering::Release);
        let _ = self.controls.send(Control::Load(network::Load {
            generation: self.generation,
            path: rom.to_path_buf(),
        }));
        Ok(())
    }
    fn run_frame(&mut self, input: ButtonMask) {
        self.core.run_frame(input);
        self.snapshot();
    }
    fn run_frame_linked(&mut self, p1: ButtonMask, p2: ButtonMask) {
        self.core.run_frame_linked(p1, p2);
        self.timeline += 1;
    }
    fn set_option(&mut self, key: &str, value: &str) {
        self.core.set_option(key, value);
    }
    fn set_frame_skip(&mut self, skip: bool) {
        self.core.set_frame_skip(skip);
    }
    fn video_xrgb8888(&self) -> &[u8] {
        self.core.video_xrgb8888()
    }
    fn take_audio(&mut self) -> Vec<i16> {
        self.core.take_audio()
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        self.core.serialize()
    }
    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.core.unserialize(data)?;
        self.timeline += 1;
        Ok(())
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        self.core.save_ram()
    }
    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        self.core.load_save_ram(data)
    }
    fn av_info(&self) -> AvInfo {
        self.core.av_info()
    }
    fn rumble(&self) -> Rumble {
        self.core.rumble()
    }
    fn net(&self) -> Link {
        self.core.net()
    }
    fn start_link(&mut self, client_id: u16) {
        self.timeline += 1;
        self.core.start_link(client_id);
    }
    fn pump_link(&mut self) {
        self.core.pump_link();
    }
    fn stop_link(&mut self) {
        self.core.stop_link();
        self.timeline += 1;
    }
    fn achievement_memory(&self, ram: &mut [u8]) -> [usize; 3] {
        self.core.achievement_memory(ram)
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        let _ = self.controls.send(Control::Unload(self.generation));
        let _ =
            self.current
                .compare_exchange(self.generation, 0, Ordering::AcqRel, Ordering::Relaxed);
    }
}
