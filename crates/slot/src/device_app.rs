use std::path::PathBuf;
use std::time::{Duration, Instant};

use slot::frontend::Frontend;
use slot::input::DeviceInput;
use slot::pacing::FramePacer;
use slot::panel_turn::{panel_turn, PanelTurn};
use slot_gfx::{Compositor, FbdevSurface, Surface};
use slot_power::{trace_first_frame, DevicePlatform};

const CARD: &str = "/mnt/sdcard";

const STEP_TIMEOUT: Duration = Duration::from_millis(12);

const LOG_SYNC: Duration = Duration::from_secs(1);

pub fn run() {
    let mut cpu = slot_power::CpuGovernors::new();
    cpu.set(slot_power::CpuGovernor::Schedutil);
    slot::boot_time::mark("start");
    let _ = std::thread::Builder::new()
        .name("slot-fonts".into())
        .spawn(slot_ui::warm_fonts);
    let audio = std::thread::Builder::new()
        .name("slot-audio-open".into())
        .spawn(slot::audio::opened_sink);
    let root = std::env::var_os("SLOT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CARD));
    let mut surface = match FbdevSurface::new() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    slot::boot_time::mark("surface");
    let mut compositor = match Compositor::new(&surface) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("slot: {e}");
            return;
        }
    };
    slot::boot_time::mark("compositor");
    let panel = surface.window_size();
    let env = |key| std::env::var(key).ok();
    let turn = panel_turn(
        panel,
        slot::baseos::var("BASEOS_PANEL_ROTATION_CCW").as_deref(),
        env("SLOT_ROTATE").as_deref(),
    );
    let turned = turn != PanelTurn::None;
    let window = if turned { (panel.1, panel.0) } else { panel };
    eprintln!(
        "slot: baseos target {}, model {}, rotation ccw {}",
        slot::baseos::var("BASEOS_TARGET").unwrap_or_default(),
        slot::baseos::var("BASEOS_MODEL").unwrap_or_default(),
        slot::baseos::var("BASEOS_PANEL_ROTATION_CCW").unwrap_or_default()
    );
    eprintln!(
        "slot: panel {}x{}, canvas {}x{}, turn {turn:?}",
        panel.0, panel.1, window.0, window.1
    );
    if turned {
        if let Err(e) = compositor.set_panel_turn(panel, turn == PanelTurn::Ccw) {
            eprintln!("slot: {e}");
            return;
        }
    }
    let platform = DevicePlatform::new(root.clone());
    eprintln!("slot: {}", platform.report());
    platform.trace_boot();
    slot::boot_time::mark("platform");
    let sink = match audio.map(|h| h.join()) {
        Ok(Ok(sink)) => sink,
        _ => slot::audio::opened_sink(),
    };
    slot::boot_time::mark("audio");
    let mut frontend = Frontend::boot_with(Box::new(platform), sink);
    slot::boot_time::mark("frontend");
    let target = slot::baseos::var("BASEOS_TARGET").unwrap_or_default();
    frontend.set_left_stick(slot::input::evdev::has_left_stick(&target));
    frontend.upload_boot_faces(&mut compositor);
    slot::boot_time::mark("faces");
    frontend.upload_bezel(&mut compositor, window);
    slot::boot_time::mark("bezel");
    let mut input = DeviceInput::open(&root);
    slot::boot_time::mark("input");
    let card = root.clone();
    let _ = std::thread::Builder::new()
        .name("slot-bootlogo".into())
        .spawn(move || {
            slot::bootlogo::refresh(&card);
            slot::root::sweep_temps(&card);
        });
    let _ = std::thread::Builder::new()
        .name("slot-logsync".into())
        .spawn(sync_log_writes);
    let mut drawn = false;
    frontend.drive_emulator();
    let mut pacer = FramePacer::new(Instant::now());
    let (mut frames, mut steps, mut stale) = (0u32, 0u32, 0u32);
    let mut said = Instant::now();
    loop {
        let deadline = pacer.deadline(frontend.frame_clock(), Instant::now());
        if let Some(left) = deadline.checked_duration_since(Instant::now()) {
            std::thread::sleep(left);
        }
        input.steer_with_stick(frontend.app().left_stick_dpad());
        frontend.advance(&mut input);
        cpu.set(frontend.cpu_governor());
        if frontend.restarting() {
            cpu.restore();
            frontend.restart();
        }
        if frontend.relaunching() {
            frontend.relaunch();
            return;
        }
        if frontend.repairing() {
            cpu.restore();
            frontend.repair();
        }
        if frontend.powering_off() {
            cpu.restore();
            frontend.poweroff();
            return;
        }
        if slot::crash::stopping() {
            eprintln!("slot: told to stop, saving first");
            frontend.stop();
            return;
        }
        let due = pacer.due(frontend.frame_clock(), Instant::now());
        for step in 0..due {
            if step > 0 {
                frontend.advance(&mut input);
                cpu.set(frontend.cpu_governor());
            }
            let late = frontend.step_emulator(STEP_TIMEOUT);
            steps += 1;
            stale += u32::from(late);
            if !pacer.stepped(frontend.frame_clock(), Instant::now()) || late {
                break;
            }
        }
        frontend.render(&mut compositor, window);
        let swap = Instant::now();
        if let Err(e) = surface.swap() {
            eprintln!("slot: {e}");
            frontend.stop();
            return;
        }
        let swap_took = swap.elapsed();
        slot::latency::swapped(swap_took.as_secs_f64() * 1000.0);
        if slot::latency::tracing() {
            frames += 1;
            if frames == 600 {
                eprintln!(
                    "slot: pace: {frames} presents, {steps} steps, {stale} stale, {:.2} presents/s, {:.2} nominal fps, {} missed game periods",
                    frames as f64 / said.elapsed().as_secs_f64(),
                    1.0 / frontend.frame_clock().period.as_secs_f64(),
                    pacer.dropped
                );
                (frames, steps, stale) = (0, 0, 0);
                said = Instant::now();
            }
        }
        if !drawn {
            drawn = true;
            slot::boot_time::mark("first frame");
            trace_first_frame();
        }
    }
}

fn sync_log_writes() {
    let mut synced = 0;
    loop {
        std::thread::sleep(LOG_SYNC);
        let mut st: libc::stat = unsafe { std::mem::zeroed() };
        if unsafe { libc::fstat(2, &mut st) } != 0 {
            return;
        }
        if st.st_size != synced {
            #[cfg(target_os = "linux")]
            unsafe {
                libc::syncfs(2);
            }
            synced = st.st_size;
        }
    }
}
