use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::cable::{self, Cable};
use slot_retro::{
    ButtonMask, Link, LinkChannel, RetroCore, Rumble, GBA_H, GBA_W, NETPACKET_RELIABLE,
};

use crate::audio::Ring;
use crate::drc::{drc_ratio, drc_target};
use crate::frames::{FrameRef, Frames};
use crate::link_state;
use crate::persist::Snapshot;
use crate::resample::Resampler;
use crate::rewind::{RewindThread, REWIND_BYTES};

const PRESENT: Duration = Duration::from_nanos(16_666_667);

pub const FAST_STEPS: u32 = 6;

pub const FAST_STEPS_MAX: u32 = 6;

const FAST_TARGET: Duration = Duration::from_micros(14_000);

const COST_BLEND: u32 = 4;

const SNAPSHOT_EVERY: u32 = 2;

const STALL: Duration = Duration::from_millis(25);

const RATE_WINDOW: u32 = 300;

const TRACE_EVERY: u64 = 300;

const MAX_LINK_PACKETS_PER_PRESENT: u32 = 256;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Speed {
    Paused,
    Normal,
    Fast,
}

impl Speed {
    fn from_u8(v: u8) -> Speed {
        match v {
            0 => Speed::Paused,
            1 => Speed::Normal,
            _ => Speed::Fast,
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum CoreState {
    Loading,
    Ready,
    Failed,
}

pub struct EmuHandle {
    frames: Arc<Frames>,
    shared: Arc<Shared>,
    cmds: Sender<Cmd>,
    join: Option<JoinHandle<()>>,
    rumble: Rumble,
    link: Link,
}

enum Cmd {
    Load(Vec<u8>),
    Save(Sender<Vec<u8>>),
    Sav(Sender<Option<Vec<u8>>>),
    Thumb(Sender<Option<Vec<u8>>>),
    BeginLink(u16, Box<dyn LinkChannel>),
    EndLink,
    BeginCable(u8, Box<dyn LinkChannel>),
    SetOption(String, String),
}

struct Shared {
    input: AtomicU16,
    speed: AtomicU8,
    observed: AtomicU8,
    state: AtomicU8,
    rewind: AtomicBool,
    rewind_fill: AtomicU8,
    stop: AtomicBool,
    volume: AtomicU8,
    fast_steps: AtomicU32,
    ff_sound: AtomicBool,
    published: AtomicU64,
    resume_refused: AtomicBool,
    sav_refused: AtomicBool,
    link_lost: AtomicBool,
    linked: AtomicU64,
    peer_ended: AtomicBool,
    bios_mismatch: AtomicBool,
    driven: AtomicBool,
    locked: AtomicBool,
    present_ns: AtomicU64,
    requested: AtomicBool,
    clock: Mutex<Clock>,
    clocked: Condvar,
}

#[derive(Default)]
struct Clock {
    tick: u64,
    done: u64,
}

impl EmuHandle {
    pub fn spawn(
        core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
    ) -> Self {
        Self::spawn_as(core, rom, ring, sav, resume, None)
    }

    pub fn spawn_linked(
        core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
        player: u8,
    ) -> Self {
        Self::spawn_as(core, rom, ring, sav, resume, Some(player))
    }

    fn spawn_as(
        core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
        player: Option<u8>,
    ) -> Self {
        let rumble = core.rumble();
        let link = core.net();
        let frames = Frames::new((GBA_W * GBA_H * 4) as usize);
        let shared = Arc::new(Shared {
            input: AtomicU16::new(0),
            speed: AtomicU8::new(Speed::Paused as u8),
            observed: AtomicU8::new(Speed::Paused as u8),
            state: AtomicU8::new(CoreState::Loading as u8),
            rewind: AtomicBool::new(false),
            rewind_fill: AtomicU8::new(0),
            stop: AtomicBool::new(false),
            volume: AtomicU8::new(100),
            fast_steps: AtomicU32::new(FAST_STEPS),
            ff_sound: AtomicBool::new(false),
            published: AtomicU64::new(0),
            resume_refused: AtomicBool::new(false),
            sav_refused: AtomicBool::new(false),
            link_lost: AtomicBool::new(false),
            linked: AtomicU64::new(0),
            peer_ended: AtomicBool::new(false),
            bios_mismatch: AtomicBool::new(false),
            driven: AtomicBool::new(false),
            locked: AtomicBool::new(false),
            present_ns: AtomicU64::new(PRESENT.as_nanos() as u64),
            requested: AtomicBool::new(false),
            clock: Mutex::new(Clock::default()),
            clocked: Condvar::new(),
        });
        let (tx, rx) = channel();
        let worker = Worker {
            frames: frames.clone(),
            shared: shared.clone(),
            cmds: rx,
            player,
        };
        let worker_link = link.clone();
        let join = std::thread::Builder::new()
            .name("slot-emu".into())
            .spawn(move || worker.run(core, rom, ring, sav, resume, worker_link))
            .ok();
        if join.is_none() {
            shared
                .state
                .store(CoreState::Failed as u8, Ordering::Release);
        }
        EmuHandle {
            frames,
            shared,
            cmds: tx,
            join,
            rumble,
            link,
        }
    }

    pub fn rumble(&self) -> &Rumble {
        &self.rumble
    }

    pub fn net(&self) -> &Link {
        &self.link
    }

    pub fn linked_frames(&self) -> u64 {
        self.shared.linked.load(Ordering::Relaxed)
    }

    pub fn link_lost(&self) -> bool {
        self.shared.link_lost.load(Ordering::Relaxed)
    }

    pub fn peer_ended(&self) -> bool {
        self.shared.peer_ended.load(Ordering::Relaxed)
    }

    pub fn bios_mismatch(&self) -> bool {
        self.shared.bios_mismatch.load(Ordering::Relaxed)
    }

    pub fn begin_link(&self, client_id: u16, transport: Box<dyn LinkChannel>) {
        request(
            &self.cmds,
            &self.shared,
            Cmd::BeginLink(client_id, transport),
        );
    }

    pub fn begin_cable(&self, player: u8, transport: Box<dyn LinkChannel>) {
        request(&self.cmds, &self.shared, Cmd::BeginCable(player, transport));
    }

    pub fn end_link(&self) {
        request(&self.cmds, &self.shared, Cmd::EndLink);
    }

    pub fn set_option(&self, key: &str, value: &str) {
        request(
            &self.cmds,
            &self.shared,
            Cmd::SetOption(key.to_owned(), value.to_owned()),
        );
    }

    pub fn set_driven(&self, driven: bool) {
        self.shared.driven.store(driven, Ordering::Relaxed);
        self.shared.clocked.notify_all();
    }

    pub fn locked(&self) -> bool {
        self.shared.locked.load(Ordering::Acquire)
    }

    pub fn tick(&self, present: Duration) {
        self.shared
            .present_ns
            .store(present.as_nanos() as u64, Ordering::Relaxed);
        let mut clock = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner());
        clock.tick += 1;
        self.shared.clocked.notify_all();
    }

    pub fn wait_frame(&self, timeout: Duration) -> bool {
        let until = Instant::now() + timeout;
        let mut clock = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !self.locked() {
                return false;
            }
            if clock.done >= clock.tick {
                return true;
            }
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                return false;
            };
            clock = match self.shared.clocked.wait_timeout(clock, left) {
                Ok((c, _)) => c,
                Err(e) => e.into_inner().0,
            };
        }
    }

    pub fn set_input(&self, mask: ButtonMask) {
        let before = self.shared.input.swap(mask.0, Ordering::Relaxed);
        crate::latency::applied(before, mask.0);
    }

    pub fn input(&self) -> ButtonMask {
        ButtonMask(self.shared.input.load(Ordering::Relaxed))
    }

    pub fn latest_frame(&self) -> Option<FrameRef> {
        self.frames.latest()
    }

    pub fn set_speed(&self, speed: Speed) {
        self.shared.speed.store(speed as u8, Ordering::Relaxed);
    }

    pub fn has_published(&self) -> bool {
        self.shared.published.load(Ordering::Relaxed) > 0
    }

    pub fn frame_ready(&self) -> bool {
        self.frames.is_ready()
    }

    pub fn frames_taken(&self) -> u64 {
        self.frames.taken()
    }

    pub fn published_count(&self) -> u64 {
        self.shared.published.load(Ordering::Relaxed)
    }

    pub fn observed_speed(&self) -> Speed {
        Speed::from_u8(self.shared.observed.load(Ordering::Acquire))
    }

    pub fn set_volume(&self, level: u8) {
        self.shared.volume.store(level.min(100), Ordering::Relaxed);
    }

    pub fn set_fast_steps(&self, steps: u32) {
        self.shared
            .fast_steps
            .store(steps.clamp(1, FAST_STEPS_MAX), Ordering::Relaxed);
    }

    pub fn fast_steps(&self) -> u32 {
        self.shared.fast_steps.load(Ordering::Relaxed)
    }

    pub fn set_ff_sound(&self, on: bool) {
        self.shared.ff_sound.store(on, Ordering::Relaxed);
    }

    pub fn ff_sound(&self) -> bool {
        self.shared.ff_sound.load(Ordering::Relaxed)
    }

    pub fn set_rewinding(&self, on: bool) {
        self.shared.rewind.store(on, Ordering::Relaxed);
    }

    pub fn rewind_fill(&self) -> u8 {
        self.shared.rewind_fill.load(Ordering::Relaxed)
    }

    pub fn state(&self) -> CoreState {
        match self.shared.state.load(Ordering::Acquire) {
            0 => CoreState::Loading,
            1 => CoreState::Ready,
            _ => CoreState::Failed,
        }
    }

    pub fn request_state(&self) -> Receiver<Vec<u8>> {
        let (tx, rx) = channel();
        request(&self.cmds, &self.shared, Cmd::Save(tx));
        rx
    }

    pub fn request_load(&self, state: Vec<u8>) {
        request(&self.cmds, &self.shared, Cmd::Load(state));
    }

    pub fn snapshot(&self) -> EmuSnapshot {
        EmuSnapshot {
            cmds: self.cmds.clone(),
            shared: self.shared.clone(),
        }
    }
}

#[derive(Clone)]
pub struct EmuSnapshot {
    cmds: Sender<Cmd>,
    shared: Arc<Shared>,
}

impl Snapshot for EmuSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        request(&self.cmds, &self.shared, Cmd::Save(tx)).then_some(())?;
        rx.recv().ok()
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        request(&self.cmds, &self.shared, Cmd::Sav(tx)).then_some(())?;
        rx.recv().ok().flatten()
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        let (tx, rx) = channel();
        request(&self.cmds, &self.shared, Cmd::Thumb(tx)).then_some(())?;
        rx.recv().ok().flatten()
    }

    fn load(&self, state: Vec<u8>) {
        request(&self.cmds, &self.shared, Cmd::Load(state));
    }

    fn resume_trusted(&self) -> bool {
        !self.shared.resume_refused.load(Ordering::Acquire)
    }

    fn save_ram_trusted(&self) -> bool {
        !self.shared.sav_refused.load(Ordering::Acquire)
    }
}

fn request(cmds: &Sender<Cmd>, shared: &Shared, cmd: Cmd) -> bool {
    if cmds.send(cmd).is_err() {
        return false;
    }
    shared.requested.store(true, Ordering::Release);
    let _clock = shared.clock.lock().unwrap_or_else(|e| e.into_inner());
    shared.clocked.notify_all();
    true
}

enum Wait {
    Tick(u64),
    Request,
    Late,
}

impl Drop for EmuHandle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

struct Worker {
    frames: Arc<Frames>,
    shared: Arc<Shared>,
    cmds: Receiver<Cmd>,
    player: Option<u8>,
}

impl Worker {
    fn for_core(&self, state: Vec<u8>) -> Vec<u8> {
        match self.player {
            Some(_) if !link_state::is_pair(&state) => link_state::pair(&state),
            _ => state,
        }
    }

    fn for_card(&self, state: Vec<u8>) -> Vec<u8> {
        self.player
            .and_then(|p| link_state::local(&state, p))
            .unwrap_or(state)
    }

    fn run(
        self,
        mut core: Box<dyn RetroCore>,
        rom: PathBuf,
        ring: Arc<Ring>,
        sav: Option<Vec<u8>>,
        resume: Option<Vec<u8>>,
        link: Link,
    ) {
        if let Err(e) = core.load(&rom) {
            eprintln!("slot: {e}");
            self.shared
                .state
                .store(CoreState::Failed as u8, Ordering::Release);
            return;
        }
        if let Some(sav) = sav {
            if let Err(e) = core.load_save_ram(&sav) {
                eprintln!("slot: save ram: {e}");
                self.shared.sav_refused.store(true, Ordering::Release);
            }
        }
        if let Some(resume) = resume {
            if let Err(e) = core.unserialize(&self.for_core(resume)) {
                eprintln!("slot: resume: {e}");
                self.shared.resume_refused.store(true, Ordering::Release);
            }
        }
        let av = core.av_info();
        let device_hz = match ring.sample_rate() {
            0 => av.sample_rate,
            hz => hz as f64,
        };
        let core_hz = match av.fps {
            fps if fps > 0.0 => av.sample_rate / (fps * PRESENT.as_secs_f64()),
            _ => av.sample_rate,
        };
        let mut resampler = Resampler::new(core_hz, device_hz);
        ring.clear_faults();
        self.shared
            .state
            .store(CoreState::Ready as u8, Ordering::Release);

        let mut out = Vec::new();
        let mut gated = (false, false);
        let rewind = RewindThread::spawn(REWIND_BYTES);
        let mut since_snapshot = 0;
        let mut cost = FrameCost::seed();
        let mut post_cost = PRESENT - FAST_TARGET;
        let mut fast_span: Option<(Instant, Duration)> = None;
        let mut deadline = Instant::now();
        let mut served = 0u64;
        let mut ticked = false;
        let mut paced = 0u64;
        let mut scale = 0.0f64;
        let mut rate = TickRate::new(Instant::now(), 0);
        let mut stalled = false;
        let mut turbo_frame = 0u32;
        let mut transport: Option<Box<dyn LinkChannel>> = None;
        let mut cable: Option<Cable> = None;
        let mut cable_presents = 0u32;
        let mut cable_stalls = 0u32;
        let mut cable_said = Instant::now();
        let mut cable_core = Duration::ZERO;
        let mut cable_wait = Duration::ZERO;
        let mut ff = (0u32, 0u32, Duration::ZERO, Instant::now());
        while !self.shared.stop.load(Ordering::Relaxed) {
            for cmd in self.cmds.try_iter() {
                self.apply(cmd, core.as_mut(), &mut transport, &mut cable, &link);
            }

            if let Some(t) = transport.as_mut() {
                match cable.as_mut() {
                    Some(c) => {
                        while let Some(buf) = t.try_recv() {
                            c.accept(&buf);
                        }
                        if let Some(state) = c.take_state() {
                            let own = core.serialize().unwrap_or_default();
                            if !link_state::same_bios(&state, &own) {
                                eprintln!("slot: cable: the host runs another GBA BIOS");
                                self.shared.bios_mismatch.store(true, Ordering::Relaxed);
                            } else {
                                match core.unserialize(&state) {
                                    Ok(()) => {
                                        eprintln!(
                                        "slot: cable: restored {} bytes, running the host's game",
                                        state.len()
                                    );
                                        c.prime();
                                        t.send(NETPACKET_RELIABLE, &cable::ready_packet());
                                    }
                                    Err(e) => eprintln!("slot: cable: the state was refused: {e}"),
                                }
                            }
                        }
                    }
                    None => drain_transport(t.as_mut(), &link, MAX_LINK_PACKETS_PER_PRESENT),
                }
                if t.peer_ended() {
                    self.shared.peer_ended.store(true, Ordering::Relaxed);
                }
                if t.is_closed() {
                    self.shared.link_lost.store(true, Ordering::Relaxed);
                }
            }
            core.pump_link();
            flush_outbound(&mut transport, &link);

            let speed = self.speed();
            self.shared.observed.store(speed as u8, Ordering::Release);
            let ff_sound = self.shared.ff_sound.load(Ordering::Relaxed);
            let rewinding = speed != Speed::Paused && self.shared.rewind.load(Ordering::Relaxed);
            let gate = (
                speed == Speed::Fast && !ff_sound,
                speed == Speed::Paused || rewinding,
            );
            if gate != gated {
                ring.set_muted(gate.0);
                ring.set_idle(gate.1);
                gated = gate;
            }
            let lock = self.shared.driven.load(Ordering::Relaxed)
                && speed == Speed::Normal
                && !rewinding
                && transport.is_none();
            if lock != self.shared.locked.load(Ordering::Relaxed) {
                self.shared.locked.store(lock, Ordering::Release);
                deadline = Instant::now();
                rate.restart(deadline, self.current_tick());
                self.shared.clocked.notify_all();
            }
            let input = ButtonMask(self.shared.input.load(Ordering::Relaxed));
            crate::latency::emu_frame(input.0);
            let mut span = 1u32;
            let ceiling = match speed {
                Speed::Paused => 0,
                Speed::Normal if lock && !ticked => 0,
                Speed::Normal => 1,
                Speed::Fast => self.shared.fast_steps.load(Ordering::Relaxed),
            };
            if rewinding {
                if let Some(state) = rewind.pop() {
                    if let Err(e) = core.unserialize(&state) {
                        eprintln!("slot: rewind: {e}");
                    }
                    core.set_frame_skip(false);
                    core.run_frame(ButtonMask(0));
                    self.publish(core.video_xrgb8888());
                }
                self.shared
                    .rewind_fill
                    .store(rewind.fill(), Ordering::Relaxed);
                let dropped = core.take_audio();
                core.recycle_audio(dropped);
            } else if ceiling > 0 {
                let one = FAST_TARGET.saturating_sub(post_cost);
                if speed == Speed::Fast {
                    span = cost.span(one, ceiling);
                }
                let ceiling = ceiling * span;
                let budget = one + PRESENT * (span - 1);
                let began = Instant::now();
                let mut ran = 0u32;
                let (mut skipped, mut drawn) = (None::<Duration>, Duration::ZERO);
                loop {
                    ran += 1;
                    let last = ran >= ceiling || cost.last(began.elapsed(), budget);
                    let input = input.turbo(turbo_frame);
                    turbo_frame = turbo_frame.wrapping_add(1);
                    core.set_frame_skip(!last);
                    let frame_began = Instant::now();
                    match cable.as_mut() {
                        Some(c) => {
                            if let Some(t) = transport.as_deref_mut() {
                                t.send(NETPACKET_RELIABLE, &c.sample(input));
                            }
                            let waited = Instant::now();
                            let ready = loop {
                                if let Some(pair) = c.ready() {
                                    break Some(pair);
                                }
                                if began.elapsed() + cost.draw > budget {
                                    break None;
                                }
                                if let Some(t) = transport.as_deref_mut() {
                                    while let Some(buf) = t.try_recv() {
                                        c.accept(&buf);
                                    }
                                }
                                std::thread::sleep(Duration::from_micros(250));
                            };
                            cable_wait += waited.elapsed();
                            match ready {
                                Some((p0, p1)) => {
                                    core.run_frame_linked(p0, p1);
                                    c.advance();
                                    self.shared.linked.fetch_add(1, Ordering::Relaxed);
                                }
                                None => {
                                    cable_stalls += 1;
                                    c.stall();
                                    self.shared.link_lost.store(
                                        c.stalled() >= cable::QUIET_FRAMES,
                                        Ordering::Relaxed,
                                    );
                                    break;
                                }
                            }
                        }
                        None => core.run_frame(input),
                    }
                    let took = frame_began.elapsed();
                    if last {
                        drawn = took;
                    } else {
                        skipped = Some(skipped.map_or(took, |w| w.max(took)));
                    }
                    if last {
                        break;
                    }
                }
                let core_time = began.elapsed();
                cost.measured(skipped, drawn);
                if cable.is_some() {
                    cable_presents += 1;
                    cable_core += core_time;
                    if cable_said.elapsed() >= Duration::from_secs(5) {
                        let secs = cable_said.elapsed().as_secs_f32();
                        eprintln!(
                            "slot: cable: {} presents, {} stalled, {:.1} fps, {:.1} ms core ({:.1} ms waiting) of {:.1} ms present",
                            cable_presents,
                            cable_stalls,
                            (cable_presents - cable_stalls) as f32 / secs,
                            cable_core.as_secs_f32() * 1000.0 / cable_presents as f32,
                            cable_wait.as_secs_f32() * 1000.0 / cable_presents as f32,
                            secs * 1000.0 / cable_presents as f32,
                        );
                        cable_presents = 0;
                        cable_stalls = 0;
                        cable_core = Duration::ZERO;
                        cable_wait = Duration::ZERO;
                        cable_said = Instant::now();
                    }
                }
                fast_span = Some((began, core_time));
                if speed != Speed::Fast {
                    ff = (0, 0, Duration::ZERO, Instant::now());
                } else if crate::session::trace() {
                    ff = (ff.0 + 1, ff.1 + ran, ff.2 + core_time, ff.3);
                    if ff.0 == 300 {
                        let secs = ff.3.elapsed().as_secs_f32();
                        eprintln!(
                            "slot: ff: {:.1} frames a present, {:.0} fps, {:.1} ms core of {:.1} ms present, {:.1} ms after",
                            ff.1 as f32 / 300.0,
                            ff.1 as f32 / secs,
                            ff.2.as_secs_f32() * 1000.0 / 300.0,
                            secs * 1000.0 / 300.0,
                            post_cost.as_secs_f32() * 1000.0,
                        );
                        ff = (0, 0, Duration::ZERO, Instant::now());
                    }
                }
                flush_outbound(&mut transport, &link);
                self.publish(core.video_xrgb8888());
                self.frame_done(served);

                since_snapshot += 1;
                if cable.is_some() || speed == Speed::Fast {
                    since_snapshot = 0;
                }
                if since_snapshot >= SNAPSHOT_EVERY {
                    since_snapshot = 0;
                    if let Ok(state) = core.serialize() {
                        rewind.push(state);
                        self.shared
                            .rewind_fill
                            .store(rewind.fill(), Ordering::Relaxed);
                    }
                }

                let mut audio = core.take_audio();
                if speed == Speed::Normal || ff_sound {
                    let target = drc_target(ring.capacity_frames());
                    let queued = ring.queued_frames();
                    if lock && scale == 0.0 {
                        scale = self.shared.present_ns.load(Ordering::Relaxed) as f64
                            / PRESENT.as_nanos() as f64;
                    }
                    let base = if lock { scale } else { 1.0 };
                    resampler.set_ratio(
                        drc_ratio(queued, target) * base * f64::from(span) / f64::from(ran),
                    );
                    resampler.process(&audio, &mut out);
                    crate::audio::volume::apply(
                        &mut out,
                        self.shared.volume.load(Ordering::Relaxed),
                    );
                    ring.push_blocking(&out);
                    paced += 1;
                    if crate::session::trace() && paced.is_multiple_of(TRACE_EVERY) {
                        let (dropped, starved) = (ring.overruns(), ring.underruns());
                        eprintln!(
                            "slot: audio: {queued}/{target} queued, {dropped} dropped, {starved} starved, locked {lock} at {scale:.5}"
                        );
                    }
                }
                core.recycle_audio(std::mem::take(&mut audio));
            }

            if let Some((began, core_time)) = fast_span.take() {
                post_cost = blend(post_cost, began.elapsed().saturating_sub(core_time));
            }
            self.frame_done(served);
            if lock {
                let patience = if stalled { PRESENT } else { STALL };
                let until = Instant::now() + patience;
                loop {
                    match self.next_tick(served, until) {
                        Wait::Tick(tick) => {
                            served = tick;
                            stalled = false;
                            if let Some(measured) = rate.served(Instant::now(), tick) {
                                scale = measured;
                            }
                        }
                        Wait::Request => {
                            for cmd in self.cmds.try_iter() {
                                self.apply(cmd, core.as_mut(), &mut transport, &mut cable, &link);
                            }
                            continue;
                        }
                        Wait::Late => {
                            rate.stalled();
                            stalled = true;
                        }
                    }
                    break;
                }
                ticked = true;
                continue;
            }
            ticked = false;
            deadline += PRESENT * span;
            let now = Instant::now();
            match deadline.checked_duration_since(now) {
                Some(wait) => std::thread::sleep(wait),
                None => deadline = now,
            }
        }
        for cmd in self.cmds.try_iter() {
            self.apply(cmd, core.as_mut(), &mut transport, &mut cable, &link);
        }
        ring.set_muted(false);
        ring.set_idle(false);
        let (dropped, starved) = (ring.overruns(), ring.underruns());
        if dropped > 0 || starved > 0 || crate::session::trace() {
            eprintln!("slot: audio: {dropped} samples dropped, {starved} starved");
        }
    }

    fn apply(
        &self,
        cmd: Cmd,
        core: &mut dyn RetroCore,
        transport: &mut Option<Box<dyn LinkChannel>>,
        cable: &mut Option<Cable>,
        link: &Link,
    ) {
        match cmd {
            Cmd::Save(reply) => match core.serialize() {
                Ok(state) => {
                    let _ = reply.send(self.for_card(state));
                }
                Err(e) => eprintln!("slot: {e}"),
            },
            Cmd::Load(state) => {
                if let Err(e) = core.unserialize(&self.for_core(state)) {
                    eprintln!("slot: {e}");
                }
            }
            Cmd::Sav(reply) => {
                let _ = reply.send(core.save_ram());
            }
            Cmd::Thumb(reply) => {
                let _ = reply.send(crate::thumb::png(core.video_xrgb8888()));
            }
            Cmd::BeginLink(client_id, t) => {
                self.shared.link_lost.store(false, Ordering::Relaxed);
                self.shared.peer_ended.store(false, Ordering::Relaxed);
                self.shared.bios_mismatch.store(false, Ordering::Relaxed);
                link.clear();
                core.start_link(client_id);
                link.set_active(true);
                *transport = Some(t);
            }
            Cmd::BeginCable(player, t) => {
                self.shared.link_lost.store(false, Ordering::Relaxed);
                self.shared.peer_ended.store(false, Ordering::Relaxed);
                self.shared.bios_mismatch.store(false, Ordering::Relaxed);
                let mut c = Cable::new(player);
                let mut wire = t;
                if player == 0 {
                    match core.serialize() {
                        Ok(state) => {
                            eprintln!("slot: cable: sending {} bytes of state", state.len());
                            for p in cable::state_packets(&state) {
                                wire.send(NETPACKET_RELIABLE, &p);
                            }
                        }
                        Err(e) => eprintln!("slot: cable: the core would not serialize: {e}"),
                    }
                    c.prime();
                }
                *cable = Some(c);
                *transport = Some(wire);
            }
            Cmd::SetOption(key, value) => {
                core.set_option(&key, &value);
            }
            Cmd::EndLink => {
                self.shared.link_lost.store(false, Ordering::Relaxed);
                self.shared.peer_ended.store(false, Ordering::Relaxed);
                self.shared.bios_mismatch.store(false, Ordering::Relaxed);
                *cable = None;
                core.stop_link();
                if let Some(t) = transport.as_mut() {
                    t.send_end();
                }
                *transport = None;
                link.clear();
                link.set_active(false);
            }
        }
    }

    fn frame_done(&self, served: u64) {
        let mut clock = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner());
        if clock.done < served {
            clock.done = served;
            self.shared.clocked.notify_all();
        }
    }

    fn current_tick(&self) -> u64 {
        self.shared
            .clock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .tick
    }

    fn next_tick(&self, served: u64, until: Instant) -> Wait {
        let mut clock = self.shared.clock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if clock.tick > served {
                return Wait::Tick(clock.tick);
            }
            if self.shared.requested.swap(false, Ordering::AcqRel) {
                return Wait::Request;
            }
            let Some(left) = until.checked_duration_since(Instant::now()) else {
                return Wait::Late;
            };
            if self.shared.stop.load(Ordering::Relaxed)
                || !self.shared.driven.load(Ordering::Relaxed)
            {
                return Wait::Late;
            }
            clock = match self.shared.clocked.wait_timeout(clock, left) {
                Ok((c, _)) => c,
                Err(e) => e.into_inner().0,
            };
        }
    }

    fn publish(&self, video: &[u8]) {
        let mut buf = self.frames.take_write();
        buf.clear();
        buf.extend_from_slice(video);
        self.frames.publish(buf);
        self.shared.published.fetch_add(1, Ordering::Relaxed);
        crate::latency::published();
    }

    fn speed(&self) -> Speed {
        Speed::from_u8(self.shared.speed.load(Ordering::Relaxed))
    }
}

fn flush_outbound(transport: &mut Option<Box<dyn LinkChannel>>, link: &Link) {
    let Some(t) = transport.as_deref_mut() else {
        return;
    };
    while let Some(packet) = link.take_outbound() {
        t.send(NETPACKET_RELIABLE, &packet);
    }
}

fn blend(estimate: Duration, measured: Duration) -> Duration {
    (estimate * (COST_BLEND - 1) + measured) / COST_BLEND
}

struct FrameCost {
    skip: Duration,
    draw: Duration,
    seeded: bool,
}

impl FrameCost {
    fn seed() -> Self {
        FrameCost {
            skip: PRESENT,
            draw: PRESENT,
            seeded: true,
        }
    }

    fn last(&self, elapsed: Duration, budget: Duration) -> bool {
        elapsed + self.skip + self.draw > budget
    }

    fn frames(&self, budget: Duration, ceiling: u32) -> u32 {
        let mut elapsed = Duration::ZERO;
        let mut ran = 0;
        loop {
            ran += 1;
            let last = ran >= ceiling || self.last(elapsed, budget);
            elapsed += if last { self.draw } else { self.skip };
            if last {
                return ran;
            }
        }
    }

    fn span(&self, budget: Duration, ceiling: u32) -> u32 {
        if self.seeded {
            return 1;
        }
        if self.frames(budget, ceiling) < ceiling {
            2
        } else {
            1
        }
    }

    fn measured(&mut self, skip: Option<Duration>, draw: Duration) {
        if self.seeded {
            self.seeded = false;
            self.draw = draw;
            self.skip = skip.unwrap_or(draw).min(draw);
            return;
        }
        let follow = |estimate: Duration, seen: Duration| {
            if seen > estimate {
                seen
            } else {
                blend(estimate, seen)
            }
        };
        self.draw = follow(self.draw, draw);
        self.skip = match skip {
            Some(skip) => follow(self.skip, skip),
            None => self.skip.min(self.draw),
        };
    }
}

struct TickRate {
    began: Instant,
    start_tick: u64,
    clean: bool,
}

impl TickRate {
    fn new(now: Instant, tick: u64) -> Self {
        TickRate {
            began: now,
            start_tick: tick,
            clean: true,
        }
    }

    fn restart(&mut self, now: Instant, tick: u64) {
        *self = TickRate::new(now, tick);
    }

    fn stalled(&mut self) {
        self.clean = false;
    }

    fn served(&mut self, now: Instant, tick: u64) -> Option<f64> {
        let ticks = tick.saturating_sub(self.start_tick);
        if ticks < u64::from(RATE_WINDOW) {
            return None;
        }
        let present = (now - self.began).as_secs_f64() / ticks as f64;
        let clean = self.clean;
        self.restart(now, tick);
        clean.then(|| present / PRESENT.as_secs_f64())
    }
}

fn drain_transport(transport: &mut dyn LinkChannel, link: &Link, cap: u32) {
    for _ in 0..cap {
        let Some(packet) = transport.try_recv() else {
            break;
        };
        link.push_inbound(packet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use slot_retro::LoopbackLink;

    const PANEL: Duration = Duration::from_micros(16_760);

    fn frames_in(cost: &FrameCost, budget: Duration, ceiling: u32) -> u32 {
        cost.frames(budget, ceiling)
    }

    #[test]
    fn a_core_that_cannot_reach_its_ceiling_in_one_refresh_spans_two() {
        let cost = FrameCost {
            skip: ms(4.8),
            draw: ms(5.7),
            seeded: false,
        };
        assert_eq!(cost.span(ms(11.9), 6), 2);
        assert!(frames_in(&cost, ms(11.9) + PRESENT, 12) >= 5);
    }

    #[test]
    fn an_unmeasured_core_keeps_one_refresh() {
        let cost = FrameCost::seed();
        assert_eq!(cost.span(ms(11.9), 6), 1);
    }

    #[test]
    fn a_core_that_reaches_its_ceiling_keeps_one_refresh() {
        let cost = FrameCost {
            skip: ms(1.2),
            draw: ms(2.0),
            seeded: false,
        };
        assert_eq!(cost.span(ms(11.9), 6), 1);
    }

    fn ms(ms: f64) -> Duration {
        Duration::from_secs_f64(ms / 1000.0)
    }

    #[test]
    fn a_core_whose_skipped_frames_fit_runs_more_than_one_a_present() {
        let cost = FrameCost {
            skip: ms(4.8),
            draw: ms(5.7),
            seeded: false,
        };
        assert_eq!(frames_in(&cost, ms(11.9), 6), 2);
    }

    #[test]
    fn cheap_frames_run_up_to_the_ceiling() {
        let cost = FrameCost {
            skip: ms(1.2),
            draw: ms(2.0),
            seeded: false,
        };
        assert_eq!(frames_in(&cost, ms(11.9), 6), 6);
    }

    #[test]
    fn a_present_never_plans_past_its_budget() {
        let cost = FrameCost {
            skip: ms(4.8),
            draw: ms(5.7),
            seeded: false,
        };
        for budget in [5.0, 8.0, 11.9, 14.0, 20.0] {
            let n = frames_in(&cost, ms(budget), 6);
            let planned = cost.skip * (n - 1) + cost.draw;
            assert!(
                n == 1 || planned <= ms(budget),
                "{n} frames plan {planned:?} into {budget} ms"
            );
        }
    }

    #[test]
    fn with_no_skipped_frame_yet_the_skip_estimate_follows_the_drawn_one() {
        let mut cost = FrameCost::seed();
        for _ in 0..12 {
            cost.measured(None, ms(3.6));
        }
        assert_eq!(cost.skip, cost.draw);
        assert!(frames_in(&cost, ms(13.6), 6) > 1);
    }

    #[test]
    fn a_slow_frame_raises_its_estimate_at_once_and_a_fast_one_lowers_it_slowly() {
        let mut cost = FrameCost {
            skip: ms(4.0),
            draw: ms(5.0),
            seeded: false,
        };
        cost.measured(Some(ms(9.0)), ms(5.0));
        assert_eq!(cost.skip, ms(9.0));
        cost.measured(Some(ms(1.0)), ms(5.0));
        assert!(cost.skip > ms(6.0) && cost.skip < ms(9.0));
        cost.measured(None, ms(5.0));
        assert!(
            cost.skip <= cost.draw,
            "a skipped frame was costed above a drawn one"
        );
    }

    fn run_window(rate: &mut TickRate, start: Instant, ticks_per_frame: u64) -> Option<f64> {
        let mut scale = None;
        let mut tick = rate.start_tick;
        while scale.is_none() {
            tick += ticks_per_frame;
            scale = rate.served(start + PANEL * (tick - rate.start_tick) as u32, tick);
        }
        scale
    }

    fn near_panel(scale: Option<f64>) -> bool {
        scale.is_some_and(|s| (s - PANEL.as_secs_f64() / PRESENT.as_secs_f64()).abs() < 1e-3)
    }

    #[test]
    fn a_worker_serving_two_ticks_a_frame_still_measures_the_panel() {
        let start = Instant::now();
        let mut rate = TickRate::new(start, 0);
        let scale = run_window(&mut rate, start, 2);
        assert!(near_panel(scale), "measured {scale:?}");
    }

    #[test]
    fn a_pause_before_the_lock_returns_is_not_measured() {
        let start = Instant::now();
        let mut rate = TickRate::new(start, 0);
        rate.served(start + PANEL * 10, 10);
        let back = start + PANEL * 10 + Duration::from_secs(6);
        rate.restart(back, 370);
        let scale = run_window(&mut rate, back, 1);
        assert!(near_panel(scale), "measured {scale:?}");
    }

    #[test]
    fn a_window_with_a_stall_is_dropped() {
        let start = Instant::now();
        let mut rate = TickRate::new(start, 0);
        rate.stalled();
        let first: Vec<_> = (1..=u64::from(RATE_WINDOW))
            .filter_map(|t| rate.served(start + PANEL * t as u32, t))
            .collect();
        assert!(first.is_empty(), "a stalled window gave {first:?}");
        let next = start + PANEL * RATE_WINDOW;
        assert!(near_panel(run_window(&mut rate, next, 1)));
    }

    #[test]
    fn drain_transport_stops_at_the_cap_and_leaves_the_rest_queued() {
        let mut transport = LoopbackLink::default();
        for i in 0..10u8 {
            transport.send(0, &[i]);
        }
        let link = Link::default();

        drain_transport(&mut transport, &link, 4);

        let mut got = Vec::new();
        while let Some(p) = link.take_inbound() {
            got.push(p[0]);
        }
        assert_eq!(
            got,
            vec![0, 1, 2, 3],
            "the cap must stop the drain, not just slow it"
        );
        assert_eq!(
            transport.try_recv(),
            Some(vec![4]),
            "packets past the cap must stay queued in the transport, not be dropped"
        );
    }

    #[test]
    fn drain_transport_moves_everything_under_the_cap() {
        let mut transport = LoopbackLink::default();
        transport.send(0, b"one");
        transport.send(0, b"two");
        let link = Link::default();

        drain_transport(&mut transport, &link, MAX_LINK_PACKETS_PER_PRESENT);

        assert_eq!(link.take_inbound().as_deref(), Some(&b"one"[..]));
        assert_eq!(link.take_inbound().as_deref(), Some(&b"two"[..]));
        assert_eq!(link.take_inbound(), None);
    }
}
