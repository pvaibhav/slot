use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use slot::frontend::Frontend;
use slot_gfx::{blue_light_gain, dim_gain, Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;

const FPS: f64 = 60.0;

const CLOCK: i64 = 1_790_784_000;

enum Step {
    Wait(u32),
    Down(Btn),
    Up(Btn),
    Rec(Option<String>),
    Cut,
}

struct Clip {
    path: String,
    frames: std::sync::mpsc::Sender<Vec<u8>>,
    writer: std::thread::JoinHandle<()>,
    ffmpeg: std::process::Child,
    written: u32,
}

impl Clip {
    fn start(path: String) -> Clip {
        let mut ffmpeg = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args(["-s", &format!("{OUT_W}x{OUT_H}"), "-r", "60", "-i", "-"])
            .args(["-c:v", "libx264", "-preset", "slow", "-crf", "20"])
            .args(["-pix_fmt", "yuv420p", "-movflags", "+faststart", &path])
            .stdin(Stdio::piped())
            .spawn()
            .expect("ffmpeg");
        let mut pipe = ffmpeg.stdin.take().unwrap();
        let (frames, queued) = std::sync::mpsc::channel::<Vec<u8>>();
        let writer = std::thread::spawn(move || {
            for pixels in queued {
                pipe.write_all(&pixels).expect("ffmpeg went away");
            }
        });
        Clip {
            path,
            frames,
            writer,
            ffmpeg,
            written: 0,
        }
    }

    fn finish(self) {
        drop(self.frames);
        self.writer.join().unwrap();
        let mut ffmpeg = self.ffmpeg;
        let status = ffmpeg.wait().expect("ffmpeg");
        eprintln!(
            "record: {} frames ({:.2} s) to {}, {status}",
            self.written,
            f64::from(self.written) / FPS,
            self.path
        );
    }
}

fn btn(name: &str) -> Btn {
    match name {
        "up" => Btn::Up,
        "down" => Btn::Down,
        "left" => Btn::Left,
        "right" => Btn::Right,
        "a" => Btn::A,
        "b" => Btn::B,
        "x" => Btn::X,
        "y" => Btn::Y,
        "l1" => Btn::L1,
        "r1" => Btn::R1,
        "l2" => Btn::L2,
        "r2" => Btn::R2,
        "start" => Btn::Start,
        "select" => Btn::Select,
        "menu" => Btn::Menu,
        other => panic!("unknown button {other}"),
    }
}

fn parse(script: &str) -> Vec<Step> {
    let mut steps = Vec::new();
    for line in script.lines() {
        let words: Vec<&str> = line.split('#').next().unwrap().split_whitespace().collect();
        let n = |i: usize, default: u32| words.get(i).map_or(default, |w| w.parse().unwrap());
        match words.as_slice() {
            [] => {}
            ["wait", ..] => steps.push(Step::Wait(n(1, 1))),
            ["tap", b, ..] | ["hold", b, ..] => {
                steps.push(Step::Down(btn(b)));
                steps.push(Step::Wait(n(2, 6)));
                steps.push(Step::Up(btn(b)));
            }
            ["down", b] => steps.push(Step::Down(btn(b))),
            ["up", b] => steps.push(Step::Up(btn(b))),
            ["rec"] => steps.push(Step::Rec(None)),
            ["rec", name] => steps.push(Step::Rec(Some(name.to_string()))),
            ["cut"] => steps.push(Step::Cut),
            _ => panic!("cannot read {line:?}"),
        }
    }
    steps
}

fn panel(pixels: &mut [u8], app: &slot::app::App, lit: u8) {
    let light = f32::from(app.brightness()) / f32::from(lit) * dim_gain(app.dim());
    let gain = blue_light_gain(app.blue_light()).map(|g| g * light);
    if gain == [1.0; 3] {
        return;
    }
    for px in pixels.chunks_exact_mut(4) {
        for (c, g) in px.iter_mut().zip(gain) {
            *c = (f32::from(*c) * g).round().min(255.0) as u8;
        }
    }
}

fn connect(app: &mut slot::app::App) {
    use slot::wifi::Backend;
    let mut fake = slot::wifi::Fake::demo();
    fake.add("Home", "password", false);
    app.set_wifi(slot::wifi::Wifi::inline(Box::new(fake)));
    let tick = |app: &mut slot::app::App| {
        let now = app.now();
        app.tick_ms(now + 1_000);
    };
    for _ in 0..10 {
        if app.wifi().held {
            break;
        }
        tick(app);
    }
    app.wifi_send(slot::wifi::Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    for _ in 0..4 {
        tick(app);
    }
    assert!(app.wifi_up(), "the fake network did not connect");
}

#[derive(Default)]
struct Scripted(Vec<RawEvent>);

impl InputSource for Scripted {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        std::mem::take(&mut self.0)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [root, script, out] = args.as_slice() else {
        eprintln!("usage: record ROOT SCRIPT OUT");
        std::process::exit(2);
    };
    let script = std::fs::read_to_string(script).expect("read the script");
    let steps = parse(&script);

    let surface = HeadlessSurface::new().expect("headless GL");
    let mut compositor = Compositor::new(&surface).expect("compositor");
    let mut frontend = Frontend::boot(Box::new(SimPlatform::at(root.into()).stopped_at(CLOCK)));
    frontend.upload_faces(&mut compositor);
    frontend.drive_emulator();
    if script.lines().any(|line| line.trim() == "# wifi") {
        connect(frontend.app_mut());
    }

    let mut input = Scripted::default();
    let lit = frontend.app().brightness().max(1);
    let mut frame = 0u64;
    let mut clip: Option<Clip> = None;
    let present = Duration::from_secs_f64(1.0 / FPS);
    let mut due = std::time::Instant::now();
    let mut run = |input: &mut Scripted, frame: &mut u64, clip: &mut Option<Clip>| {
        let loading = std::time::Instant::now();
        while frontend.core_settling() && loading.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        due += present;
        match due.checked_duration_since(std::time::Instant::now()) {
            Some(wait) if clip.is_some() => std::thread::sleep(wait),
            _ => due = std::time::Instant::now(),
        }
        *frame += 1;
        let now = (*frame as f64 * 1000.0 / FPS) as Millis;
        frontend.advance_at(input, now, (1.0 / FPS) as f32);
        frontend.step_emulator(Duration::from_secs(1));
        frontend.compose(&mut compositor);
        let mut pixels = compositor.read_frame();
        panel(&mut pixels, frontend.app(), lit);
        if let Some(clip) = clip {
            clip.frames.send(pixels).unwrap();
            clip.written += 1;
        }
    };
    for step in steps {
        match step {
            Step::Wait(n) => {
                for _ in 0..n {
                    run(&mut input, &mut frame, &mut clip);
                }
            }
            Step::Down(b) => input.0.push(RawEvent::Down(b)),
            Step::Up(b) => input.0.push(RawEvent::Up(b)),
            Step::Rec(name) => {
                if let Some(done) = clip.take() {
                    done.finish();
                }
                let path = match name {
                    Some(name) => format!("{out}/{name}.mp4"),
                    None => out.clone(),
                };
                clip = Some(Clip::start(path));
            }
            Step::Cut => {
                if let Some(done) = clip.take() {
                    done.finish();
                }
            }
        }
    }
    if let Some(done) = clip.take() {
        done.finish();
    }
}
