use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use slot::ble::{Ble, BleWant, Heard};
use slot::frontend::Frontend;
use slot_gfx::{blue_light_gain, dim_gain, Compositor, HeadlessSurface};
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
    fn start(path: String, (w, h): (u32, u32)) -> Clip {
        let mut ffmpeg = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args(["-s", &format!("{w}x{h}"), "-r", "60", "-i", "-"])
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

struct UdpBle {
    socket: std::net::UdpSocket,
    want: std::sync::Arc<std::sync::Mutex<BleWant>>,
}

impl UdpBle {
    fn start(port: u16) -> UdpBle {
        let (socket, peer) = match std::net::UdpSocket::bind(("127.0.0.1", port)) {
            Ok(socket) => (socket, port + 1),
            Err(_) => (
                std::net::UdpSocket::bind(("127.0.0.1", port + 1)).expect("fake BLE port"),
                port,
            ),
        };
        socket.set_nonblocking(true).expect("fake BLE socket");
        eprintln!(
            "record: fake BLE on {}, peer on {peer}",
            socket.local_addr().unwrap()
        );
        let want = std::sync::Arc::new(std::sync::Mutex::new(BleWant::Off));
        let air = socket.try_clone().expect("fake BLE socket");
        let wanted = want.clone();
        std::thread::spawn(move || loop {
            let sent = match &*wanted.lock().unwrap() {
                BleWant::Advertise { advert, scan } => Some([&advert[..], scan].concat()),
                _ => None,
            };
            if let Some(bytes) = sent {
                let _ = air.send_to(&bytes, ("127.0.0.1", peer));
            }
            std::thread::sleep(Duration::from_millis(100));
        });
        UdpBle { socket, want }
    }
}

impl Ble for UdpBle {
    fn set(&mut self, want: BleWant) {
        eprintln!("record: fake BLE wants {want:?}");
        *self.want.lock().unwrap() = want;
    }

    fn heard(&mut self) -> Vec<Heard> {
        let listening = *self.want.lock().unwrap() == BleWant::Listen;
        let mut heard = Vec::new();
        let mut buf = [0u8; 128];
        while let Ok(n) = self.socket.recv(&mut buf) {
            let data = &buf[..n];
            if !listening || n < 31 {
                continue;
            }
            if let Some(advert) = slot::invite::decode(&data[..31]) {
                heard.push(Heard {
                    advert,
                    nickname: slot::invite::nickname_of(&data[31..]),
                    rssi: -50,
                    at: slot::ble::clock(),
                });
            }
        }
        heard
    }
}

fn fake_radio(app: &mut slot::app::App) {
    use slot::link_radio::{HostAp, RadioRole};
    use slot::link_start::{LinkStarter, HOST_ADDR};
    let Some(port) = std::env::var("SLOT_FAKE_BLE_PORT")
        .ok()
        .and_then(|p| p.trim().parse::<u16>().ok())
    else {
        return;
    };
    app.set_ble(Box::new(UdpBle::start(port)));
    app.set_link_spawn(Box::new(|role, port, held| {
        LinkStarter::spawn_held_with(
            Box::new(|role, _| {
                Ok(match role {
                    RadioRole::Host => Some(HostAp {
                        freq_mhz: 2437,
                        bssid: [0x02, 0x51, 0x07, 0x00, 0x00, 0x01],
                    }),
                    RadioRole::Join(_) => None,
                })
            }),
            Box::new(|| {}),
            role,
            port,
            Box::new(move |port, cancel| match role {
                RadioRole::Host => slot::link_net::TcpLink::host_until(
                    HOST_ADDR,
                    port,
                    slot::link_net::HOST_BOUND,
                    cancel,
                ),
                RadioRole::Join(_) => slot::link_net::TcpLink::join_until(
                    HOST_ADDR,
                    port,
                    slot::link_net::HOST_BOUND,
                    cancel,
                ),
            }),
            held,
        )
    }));
}

fn panel_size(script: &str) -> Option<(u32, u32)> {
    let size = script
        .lines()
        .find_map(|line| line.trim().strip_prefix("# panel:"))?;
    let (w, h) = size.trim().split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
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
    if let Some(panel) = panel_size(&script) {
        frontend.upload_bezel(&mut compositor, panel);
        compositor.fit(panel);
    }
    frontend.drive_emulator();
    fake_radio(frontend.app_mut());
    let size = compositor.frame_size();
    if script.lines().any(|line| line.trim() == "# wifi") {
        connect(frontend.app_mut());
    }
    if script.lines().any(|line| line.trim() == "# offline") {
        frontend
            .app_mut()
            .set_wifi(slot::wifi::Wifi::inline(Box::new(slot::wifi::Fake::demo())));
    }

    let mut input = Scripted::default();
    let lit = frontend.app().brightness().max(1);
    let settle = !script.lines().any(|line| line.trim() == "# no settle");
    let mut frame = 0u64;
    let mut clip: Option<Clip> = None;
    let present = Duration::from_secs_f64(1.0 / FPS);
    let mut due = std::time::Instant::now();
    let mut run = |input: &mut Scripted, frame: &mut u64, clip: &mut Option<Clip>| {
        let loading = std::time::Instant::now();
        while settle && frontend.core_settling() && loading.elapsed() < Duration::from_secs(5) {
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
                clip = Some(Clip::start(path, size));
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
