use std::path::PathBuf;
use std::time::{Duration, Instant};

use slot::audio::{ring_capacity, AudioSink, Ring, StubSink, OUTPUT_HZ};
use slot::emu::{CoreState, EmuHandle, Speed};
use slot_retro::MockCore;

fn ramp(frames: usize, from: i16) -> Vec<i16> {
    (0..frames * 2)
        .map(|i| from.wrapping_add(i as i16))
        .collect()
}

#[test]
fn queued_frames_tracks_the_device_backlog() {
    let mut s = StubSink::new();
    s.open(32768).unwrap();
    let r = s.ring();
    let primed = r.queued_frames();
    r.push_blocking(&ramp(1000, 0));
    assert_eq!(r.queued_frames(), primed + 1000);
    s.device_read(600);
    assert_eq!(r.queued_frames(), primed + 400);
}

#[test]
fn opening_primes_the_ring_to_the_drc_target() {
    let mut s = StubSink::new();
    s.open(48_000).unwrap();
    let r = s.ring();
    assert_eq!(r.queued_frames(), r.capacity_frames() / 2);
    s.device_read(64);
    assert_eq!(
        (r.overruns(), r.underruns()),
        (0, 0),
        "the device was starved at open"
    );
}

#[test]
fn a_steady_producer_and_consumer_neither_drop_nor_starve() {
    let r = Ring::new(ring_capacity(48_000));
    let mut out = vec![0i16; 1024];
    for i in 0..2_000 {
        let n = match i % 5 {
            0 => 2048,
            4 => 0,
            _ => 1024,
        };
        let block = vec![1i16; n];
        r.push_blocking(&block);
        r.fill(&mut out);
    }
    assert_eq!(
        r.overruns(),
        0,
        "dropped audio: the producer was never made to wait"
    );
    assert_eq!(r.underruns(), 0, "starved the device");
}

#[test]
fn push_blocking_waits_instead_of_discarding() {
    let r = std::sync::Arc::new(Ring::new(64));
    let w = r.clone();
    let h = std::thread::spawn(move || w.push_blocking(&[7i16; 256]));
    let mut out = vec![0i16; 64];
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        r.fill(&mut out);
    }
    h.join().unwrap();
    assert_eq!(r.overruns(), 0, "push_blocking dropped rather than waited");
}

#[test]
fn the_ring_holds_enough_to_ride_out_a_late_frame() {
    let ms = ring_capacity(48_000) as f64 / 48_000.0 * 1000.0;
    assert!(
        ms > 100.0,
        "only {ms:.0} ms of slack, a single late frame empties it"
    );
}

#[test]
fn a_muted_or_paused_core_does_not_block_on_audio() {
    let r = std::sync::Arc::new(Ring::new(64));
    r.set_muted(true);
    let w = r.clone();
    let h = std::thread::spawn(move || w.push_blocking(&[7i16; 4096]));
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(h.is_finished(), "a muted sink blocked the emulator thread");
}

#[test]
fn a_device_that_stopped_draining_does_not_freeze_the_emulator() {
    let r = Ring::new(8);
    r.push_blocking(&[1i16; 32]);
    let start = std::time::Instant::now();
    for _ in 0..20 {
        r.push_blocking(&[1i16; 32]);
    }
    assert!(
        start.elapsed() < std::time::Duration::from_millis(500),
        "twenty frames took {:?} against a dead device",
        start.elapsed()
    );
    assert!(r.overruns() > 0, "the drops went uncounted");
}

#[test]
fn samples_come_back_in_order_across_the_wrap() {
    let r = Ring::new(8);
    r.push(&ramp(6, 1));

    let mut out = vec![0i16; 8];
    r.fill(&mut out);
    assert_eq!(out, ramp(6, 1)[..8]);

    r.push(&ramp(5, 100));
    let mut rest = vec![0i16; 14];
    r.fill(&mut rest);
    let mut want = ramp(6, 1)[8..].to_vec();
    want.extend(ramp(5, 100));
    assert_eq!(rest, want);
}

#[test]
fn a_full_ring_drops_new_samples_instead_of_evicting_queued_audio() {
    let r = Ring::new(4);
    r.push(&ramp(3, 1));
    r.push(&ramp(3, 50));
    assert_eq!(r.queued_frames(), 4);

    let mut out = vec![0i16; 8];
    r.fill(&mut out);
    let mut want = ramp(3, 1);
    want.extend(&ramp(3, 50)[..2]);
    assert_eq!(out, want);
    assert_eq!(r.queued_frames(), 0);
}

#[test]
fn an_underrun_pads_with_silence_rather_than_repeating() {
    let r = Ring::new(64);
    r.push(&ramp(2, 9));
    let mut out = vec![-1i16; 8];
    r.fill(&mut out);
    assert_eq!(&out[..4], &ramp(2, 9)[..]);
    assert_eq!(&out[4..], &[0, 0, 0, 0]);
}

#[test]
fn muting_silences_the_device_but_still_drains_the_ring() {
    let r = Ring::new(64);
    r.push(&ramp(4, 1));
    r.set_muted(true);
    let mut out = vec![0i16; 4];
    r.fill(&mut out);
    assert!(out.iter().all(|s| *s == 0));
    assert_eq!(r.queued_frames(), 2);
}

#[test]
fn a_held_core_does_not_report_the_device_as_starved() {
    let r = Ring::new(ring_capacity(48_000));
    r.reopen(48_000);
    let mut out = vec![0i16; 4_000];
    while r.queued_frames() > 0 {
        r.fill(&mut out);
    }
    r.set_idle(true);
    let before = r.underruns();
    for _ in 0..20 {
        r.fill(&mut out);
    }
    assert_eq!(
        r.underruns(),
        before,
        "a held core was reported as a starve"
    );

    r.set_idle(false);
    r.fill(&mut out);
    assert!(r.underruns() > before, "a real starve is no longer counted");
}

#[test]
fn a_ring_that_ran_dry_rebuilds_its_cushion_before_playing_again() {
    let r = Ring::new(ring_capacity(48_000));
    let target = r.capacity_frames() / 2;
    let mut out = vec![0i16; 512 * 2];
    r.fill(&mut out);
    assert!(
        r.underruns() > 0,
        "a dry ring with a producer is an underrun"
    );

    r.push(&ramp(200, 1));
    r.fill(&mut out);
    assert!(
        out.iter().all(|s| *s == 0),
        "a fragment was played before the cushion was back"
    );
    assert_eq!(
        r.queued_frames(),
        200,
        "the fragment was drained rather than held"
    );

    r.push(&ramp(target, 1));
    r.fill(&mut out);
    assert!(
        out.iter().any(|s| *s != 0),
        "the cushion was full and still nothing played"
    );
}

#[test]
fn a_cart_sound_is_not_held_back_by_the_cushion() {
    let r = Ring::new(ring_capacity(48_000));
    let mut out = vec![0i16; 512 * 2];
    r.fill(&mut out);
    r.mix(&ramp(100, 1));
    r.fill(&mut out);
    assert!(
        out.iter().any(|s| *s != 0),
        "the cart sound was swallowed by the cushion"
    );
}

#[test]
fn a_rewinding_core_does_not_report_the_device_as_starved() {
    let mut sink = StubSink::new();
    sink.open(OUTPUT_HZ).expect("the stub refused to open");
    let ring = sink.ring();
    let emu = EmuHandle::spawn(
        Box::new(MockCore::new()),
        PathBuf::from("mock"),
        ring.clone(),
        None,
        None,
    );
    emu.set_speed(Speed::Normal);
    let deadline = Instant::now() + Duration::from_secs(5);
    while emu.state() == CoreState::Loading {
        assert!(Instant::now() < deadline, "the core never settled");
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..40 {
        sink.device_drain();
        std::thread::sleep(Duration::from_millis(2));
    }

    emu.set_rewinding(true);
    std::thread::sleep(Duration::from_millis(60));
    sink.device_drain();
    let before = ring.underruns();
    for _ in 0..10 {
        sink.device_read(512);
    }
    assert_eq!(
        ring.underruns(),
        before,
        "a rewind was reported as the device going hungry"
    );
}

#[test]
fn the_cushion_at_an_odd_capacity_still_ends_on_a_frame() {
    assert_eq!(
        ring_capacity(32_768) % 2,
        1,
        "this rate no longer has an odd capacity, so it no longer exercises the bug"
    );
    let r = Ring::new(ring_capacity(32_768));
    r.reopen(32_768);

    let mut cushion = vec![1i16; r.queued_frames() * 2];
    r.fill(&mut cushion);
    assert!(
        cushion.iter().all(|s| *s == 0),
        "the cushion was not silence"
    );
    let block: Vec<i16> = (0..64i16).flat_map(|k| [1000 + k, -1000 - k]).collect();
    r.push(&block);
    let mut out = vec![0i16; block.len()];
    r.fill(&mut out);
    assert_eq!(
        out, block,
        "the cushion ended mid frame, so left and right came back swapped"
    );
}

#[test]
fn silence_releases_the_device_only_after_three_quiet_seconds() {
    let mut s = slot::audio::Silence::new(Duration::from_secs(3));
    let period = Duration::from_millis(500);
    let quiet = [0i16; 64];
    for _ in 0..5 {
        assert!(s.hear(&quiet, period));
    }
    assert!(!s.hear(&quiet, period), "still holding after 3s of silence");
    assert!(
        s.hear(&[0, 0, 3, 0], period),
        "sound did not reclaim the device"
    );
    assert!(s.hear(&quiet, period), "one sound did not restart the wait");
}

#[test]
fn a_quiet_start_holds_no_device_until_the_first_sound() {
    let mut s = slot::audio::Silence::quiet(Duration::from_secs(3));
    let period = Duration::from_millis(500);
    let quiet = [0i16; 64];
    for _ in 0..10 {
        assert!(
            !s.hear(&quiet, period),
            "silence claimed the device at boot"
        );
    }
    assert!(
        s.hear(&[0, 0, 3, 0], period),
        "the first sound did not claim the device"
    );
}

#[test]
fn a_released_device_drains_at_the_audio_clock_despite_late_wakeups() {
    let span = Duration::from_secs_f64(512.0 / OUTPUT_HZ as f64);
    let mut pace = slot::audio::Pacer::new(span);
    let start = Instant::now();
    let mut now = start;
    let periods = 1_000u32;
    for _ in 0..periods {
        now += Duration::from_micros(200);
        now += pace.wait(now) + Duration::from_micros(500);
    }
    let drift = (now - start).abs_diff(span * periods);
    assert!(
        drift < span,
        "{periods} periods took {drift:?} longer than the audio they drained"
    );
}

#[test]
fn a_released_device_resyncs_after_a_long_stall_instead_of_bursting() {
    let span = Duration::from_millis(10);
    let mut pace = slot::audio::Pacer::new(span);
    let start = Instant::now();
    assert_eq!(pace.wait(start), span);
    assert_eq!(
        pace.wait(start + span * 2),
        Duration::ZERO,
        "a short lag was not caught up"
    );
    assert_eq!(
        pace.wait(start + Duration::from_secs(2)),
        span,
        "a stall was paid back by draining without sleeping"
    );
}

#[test]
fn reopening_the_device_forgets_the_released_schedule() {
    let span = Duration::from_millis(10);
    let mut pace = slot::audio::Pacer::new(span);
    let start = Instant::now();
    pace.wait(start);
    pace.reset();
    assert_eq!(pace.wait(start + span * 3), span);
}

struct ClosingSink {
    ring: std::sync::Arc<Ring>,
    closed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl AudioSink for ClosingSink {
    fn open(&mut self, sample_rate: u32) -> Result<(), slot::audio::AudioError> {
        self.ring.reopen(sample_rate);
        Ok(())
    }

    fn ring(&self) -> std::sync::Arc<Ring> {
        self.ring.clone()
    }

    fn close(&mut self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

struct PowerCut(slot_power::SimPlatform);

impl slot_power::Platform for PowerCut {
    fn set_backlight(&mut self, step: u8) {
        self.0.set_backlight(step);
    }

    fn battery(&self) -> Option<slot_power::Battery> {
        self.0.battery()
    }

    fn charge(&self) -> slot_power::Charge {
        self.0.charge()
    }

    fn set_led(&mut self, state: slot_power::LedState) {
        self.0.set_led(state);
    }

    fn poweroff(&mut self) -> ! {
        panic!("power cut")
    }

    fn restart(&mut self) -> ! {
        panic!("restarted")
    }

    fn root(&self) -> &std::path::Path {
        self.0.root()
    }

    fn now(&self) -> i64 {
        self.0.now()
    }

    fn set_clock(&mut self, secs: i64) {
        self.0.set_clock(secs);
    }

    fn set_rumble(&mut self, strength: u16) {
        self.0.set_rumble(strength);
    }
}

#[test]
fn the_speaker_is_let_go_before_the_power_is_cut() {
    let cuts: [(&str, fn(&mut slot::frontend::Frontend)); 2] = [
        ("power off", |f| f.poweroff()),
        ("restart", |f| f.restart()),
    ];
    for (what, cut) in cuts {
        let d = tempfile::tempdir().unwrap();
        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink = ClosingSink {
            ring: std::sync::Arc::new(Ring::new(ring_capacity(OUTPUT_HZ))),
            closed: closed.clone(),
        };
        let mut f = slot::frontend::Frontend::boot_with(
            Box::new(PowerCut(slot_power::SimPlatform::at(
                d.path().to_path_buf(),
            ))),
            Box::new(sink),
        );
        let reached = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cut(&mut f)));
        assert!(reached.is_err(), "{what} never reached the platform");
        assert!(
            closed.load(std::sync::atomic::Ordering::Relaxed),
            "{what}: the audio device was still open when the power was cut, which pops the RG SP's speaker (#142)"
        );
    }
}

#[test]
fn device_xruns_are_counted_until_the_faults_clear() {
    let r = Ring::new(ring_capacity(48_000));
    r.note_xrun();
    r.note_xrun();
    assert_eq!(r.xruns(), 2);
    r.clear_faults();
    assert_eq!(r.xruns(), 0);
}
