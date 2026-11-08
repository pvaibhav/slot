use std::path::Path;

use crate::core::{AvInfo, ButtonMask, CoreError, RetroCore, GBA_H, GBA_W};

const FPS: f64 = 59.7275;
const SAMPLE_RATE: f64 = 32768.0;
const TONE_HZ: f64 = 440.0;
const AMPLITUDE: f64 = 6000.0;
const SRAM_LEN: usize = 8 * 1024;

fn samples_at(frame: u64) -> u64 {
    (frame as f64 * SAMPLE_RATE / FPS) as u64
}

pub struct MockCore {
    frame: u64,
    video: Vec<u8>,
    audio: Vec<i16>,
    sram: Option<Vec<u8>>,
    skip_next: bool,
    size: (u32, u32),
}

impl Default for MockCore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockCore {
    pub fn new() -> Self {
        Self::with_size(GBA_W, GBA_H)
    }

    pub fn with_size(w: u32, h: u32) -> Self {
        let mut c = MockCore {
            frame: 0,
            video: vec![0; (w * h * 4) as usize],
            audio: Vec::new(),
            sram: None,
            skip_next: false,
            size: (w, h),
        };
        c.render();
        c
    }

    fn render(&mut self) {
        let t = self.frame as u32;
        let (w, h) = self.size;
        let mut i = 0;
        for y in 0..h {
            let g = (y * 255 / (h - 1)) as u8;
            for x in 0..w {
                let r = x.wrapping_add(t) as u8;
                let b = if (x.wrapping_add(y).wrapping_add(t) / 8) % 2 == 0 {
                    0
                } else {
                    255
                };
                self.video[i] = b;
                self.video[i + 1] = g;
                self.video[i + 2] = r;
                self.video[i + 3] = 0;
                i += 4;
            }
        }
    }
}

impl RetroCore for MockCore {
    fn load(&mut self, _rom: &Path) -> Result<(), CoreError> {
        self.frame = 0;
        self.audio.clear();
        self.sram = Some(vec![0; SRAM_LEN]);
        self.render();
        Ok(())
    }

    fn run_frame(&mut self, _input: ButtonMask) {
        let start = samples_at(self.frame);
        self.frame += 1;
        let end = samples_at(self.frame);
        self.audio.reserve(((end - start) * 2) as usize);
        for n in start..end {
            let v = (std::f64::consts::TAU * TONE_HZ * n as f64 / SAMPLE_RATE).sin() * AMPLITUDE;
            let s = v as i16;
            self.audio.push(s);
            self.audio.push(s);
        }
        if !self.skip_next {
            self.render();
        }
    }

    fn reset(&mut self) {
        self.frame = 0;
        self.audio.clear();
        self.render();
    }

    fn set_frame_skip(&mut self, skip: bool) {
        self.skip_next = skip;
    }

    fn video_size(&self) -> (u32, u32) {
        self.size
    }

    fn video_xrgb8888(&self) -> &[u8] {
        &self.video
    }

    fn take_audio(&mut self) -> Vec<i16> {
        std::mem::take(&mut self.audio)
    }

    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        Ok(self.frame.to_le_bytes().to_vec())
    }

    fn unserialize(&mut self, data: &[u8]) -> Result<(), CoreError> {
        let bytes: [u8; 8] = data
            .try_into()
            .map_err(|_| CoreError::State(format!("expected 8 bytes, got {}", data.len())))?;
        self.frame = u64::from_le_bytes(bytes);
        self.audio.clear();
        self.render();
        Ok(())
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        self.sram.clone()
    }

    fn load_save_ram(&mut self, data: &[u8]) -> Result<(), CoreError> {
        match &mut self.sram {
            Some(s) if s.len() == data.len() => {
                s.copy_from_slice(data);
                Ok(())
            }
            Some(s) => Err(CoreError::State(format!(
                "save ram is {} bytes, got {}",
                s.len(),
                data.len()
            ))),
            None => Err(CoreError::State("no rom loaded".into())),
        }
    }

    fn av_info(&self) -> AvInfo {
        AvInfo {
            fps: FPS,
            sample_rate: SAMPLE_RATE,
        }
    }
}
