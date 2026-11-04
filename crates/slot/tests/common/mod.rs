#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use slot::app::App;
use slot::persist::Snapshot;
use slot::session::Session;
use slot_power::{Battery, Charge, LedState, Motor, Platform, Power, SimPlatform};
use slot_retro::{ButtonMask, MockCore, RetroCore};
use slot_store::{write_slot_state, Platform as CartPlatform, SlotState};
use tempfile::TempDir;

pub type Loaded = Arc<Mutex<Option<Vec<u8>>>>;

static CORE_LOCK: Mutex<()> = Mutex::new(());

pub fn core_lock() -> MutexGuard<'static, ()> {
    CORE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .expect("loopback would not give out a port")
        .local_addr()
        .expect("a bound listener with no address")
        .port()
}

static LINK_PORT_LOCK: Mutex<()> = Mutex::new(());

pub fn link_port_lock() -> MutexGuard<'static, ()> {
    LINK_PORT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn tmp_root_with_carts(stems: &[&str]) -> TempDir {
    let d = tmp_root();
    for stem in stems {
        let mut rom = vec![0u8; 0x100];
        let title = stem.to_uppercase();
        rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
        std::fs::write(rom_path(&d, stem), rom).expect("write rom");
    }
    d
}

pub fn tmp_root_with_gb_carts(stems: &[&str]) -> TempDir {
    let d = tmp_root();
    for stem in stems {
        let title = stem.to_uppercase();
        write_gb_cart(&d, stem, &title[..title.len().min(11)]);
    }
    d
}

pub fn tmp_root_with_gbc_carts(stems: &[&str]) -> TempDir {
    let d = tmp_root_with_gb_carts(&[]);
    for stem in stems {
        let title = stem.to_uppercase();
        let mut rom = vec![0u8; 0x150];
        rom[0x134..0x134 + title.len().min(11)]
            .copy_from_slice(&title.as_bytes()[..title.len().min(11)]);
        rom[0x143] = 0x80;
        std::fs::write(cart_path(&d, CartPlatform::Gbc, stem), rom).expect("write rom");
    }
    d
}

pub fn write_gb_cart(d: &TempDir, stem: &str, title: &str) {
    assert!(
        title.len() <= 11,
        "a Game Boy header title is eleven bytes, and {title:?} is longer"
    );
    let mut rom = vec![0u8; 0x150];
    rom[0x134..0x134 + title.len()].copy_from_slice(title.as_bytes());
    std::fs::write(cart_path(d, CartPlatform::Gb, stem), rom).expect("write rom");
}

pub fn tmp_root_with_real_carts(stems: &[&str]) -> TempDir {
    let d = tmp_root();
    for stem in stems {
        std::fs::write(rom_path(&d, stem), gba_rom()).expect("write rom");
    }
    d
}

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in slot::root::DIRS {
        std::fs::create_dir(d.path().join(sub)).expect("create content dir");
    }
    d
}

fn rom_path(d: &TempDir, stem: &str) -> PathBuf {
    cart_path(d, CartPlatform::Gba, stem)
}

fn cart_path(d: &TempDir, platform: CartPlatform, stem: &str) -> PathBuf {
    d.path()
        .join("Games")
        .join(platform.dir_name())
        .join(format!("{stem}.{}", platform.extensions()[0]))
}

pub fn write_retail_header(d: &TempDir, stem: &str, title: &str, code: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[3] = 0xEA;
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    rom[0xac..0xac + code.len()].copy_from_slice(code.as_bytes());
    rom[0xb2] = 0x96;
    std::fs::write(rom_path(d, stem), rom).expect("write rom");
}

pub fn write_hack_header(d: &TempDir, stem: &str, title: &str, code: &str) {
    write_retail_header(d, stem, title, code);
    let mut rom = std::fs::read(rom_path(d, stem)).expect("read rom");
    rom[0xb2] = 0;
    std::fs::write(rom_path(d, stem), rom).expect("write rom");
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn vendored_core() -> Option<PathBuf> {
    let p = repo_root().join(format!(
        "vendor/mgba_libretro.{}",
        std::env::consts::DLL_EXTENSION
    ));
    p.exists().then_some(p)
}

pub fn real_bios() -> Option<PathBuf> {
    let p = repo_root().join("sdcard/BIOS/gba_bios.bin");
    p.exists().then_some(p)
}

pub fn logo_rom() -> Option<Vec<u8>> {
    let logo = std::fs::read_dir(repo_root().join("sdcard/Games/GBA"))
        .ok()?
        .find_map(|e| {
            let p = e.ok()?.path();
            let rom = (p.extension()? == "gba").then(|| std::fs::read(&p).ok())??;
            (rom.get(4..8)? == [0x24, 0xff, 0xae, 0x51]).then(|| rom[4..0xa0].to_vec())
        })?;
    let mut rom = gba_rom();
    rom[4..0xa0].copy_from_slice(&logo);
    Some(rom)
}

pub fn mostly_lit(frame: &[u8]) -> bool {
    let lit = frame
        .chunks(4)
        .filter(|p| p[0] > 0x40 && p[1] > 0x40 && p[2] > 0x40)
        .count();
    lit * 2 > (slot_retro::GBA_W * slot_retro::GBA_H) as usize
}

pub fn gba_rom() -> Vec<u8> {
    const CODE: [u32; 15] = [
        0xe3a00404, 0xe3a01c04, 0xe3811003, 0xe5801000, 0xe3a02406, 0xe3a03000, 0xe1d040b6,
        0xe35400a0, 0x1afffffc, 0xe2833001, 0xe1c230b0, 0xe1d040b6, 0xe35400a0, 0x0afffffc,
        0xeafffff6,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[0..4].copy_from_slice(&0xea00002eu32.to_le_bytes());
    rom[0xa0..0xac].copy_from_slice(b"SLOT TEST\0\0\0");
    rom[0xac..0xb0].copy_from_slice(b"SLTE");
    rom[0xb0..0xb2].copy_from_slice(b"00");
    rom[0xb2] = 0x96;
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    for (i, w) in CODE.iter().enumerate() {
        let o = 0xc0 + i * 4;
        rom[o..o + 4].copy_from_slice(&w.to_le_bytes());
    }
    rom
}

pub fn write_real_cart_as(d: &TempDir, stem: &str, title: &str, code: &str) {
    let mut rom = gba_rom();
    rom[0xa0..0xac].fill(0);
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    rom[0xac..0xac + code.len()].copy_from_slice(code.as_bytes());
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    std::fs::write(rom_path(d, stem), rom).expect("write rom");
}

pub struct StubSnapshot {
    pub state: Vec<u8>,
    pub sav: Option<Vec<u8>>,
    pub thumb: Option<Vec<u8>>,
    pub loaded: Loaded,
}

impl StubSnapshot {
    pub fn boxed() -> Box<dyn Snapshot> {
        StubSnapshot::pair().0
    }

    pub fn pair() -> (Box<dyn Snapshot>, Loaded) {
        let loaded = Loaded::default();
        let stub = StubSnapshot {
            state: vec![9u8; 1024],
            sav: None,
            thumb: Some(b"png".to_vec()),
            loaded: loaded.clone(),
        };
        (Box::new(stub), loaded)
    }
}

impl Snapshot for StubSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        Some(self.state.clone())
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        self.sav.clone()
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        self.thumb.clone()
    }

    fn load(&self, state: Vec<u8>) {
        *self.loaded.lock().expect("loaded") = Some(state);
    }
}

#[derive(Clone, Default)]
pub struct CoreSnapshot(Arc<Mutex<MockCore>>);

impl CoreSnapshot {
    pub fn new() -> Self {
        let core = CoreSnapshot::default();
        core.with(|c| c.load(Path::new("unused")).expect("load"));
        core
    }

    pub fn boxed(&self) -> Box<dyn Snapshot> {
        Box::new(self.clone())
    }

    pub fn run_frame(&self) {
        self.with(|c| c.run_frame(ButtonMask::default()));
    }

    pub fn bytes(&self) -> Vec<u8> {
        self.with(|c| c.serialize().expect("serialize"))
    }

    fn with<T>(&self, f: impl FnOnce(&mut MockCore) -> T) -> T {
        f(&mut self.0.lock().expect("core"))
    }
}

impl Snapshot for CoreSnapshot {
    fn state(&self) -> Option<Vec<u8>> {
        Some(self.bytes())
    }

    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }

    fn thumb(&self) -> Option<Vec<u8>> {
        Some(b"png".to_vec())
    }

    fn load(&self, state: Vec<u8>) {
        self.with(|c| c.unserialize(&state).expect("unserialize"));
    }

    fn reset(&self) {
        self.with(|c| c.reset());
    }
}

#[derive(Clone, Default)]
pub struct Clock(Arc<AtomicI64>, Arc<AtomicU8>);

impl Clock {
    pub fn at(secs: i64) -> Self {
        Clock(Arc::new(AtomicI64::new(secs)), Arc::new(AtomicU8::new(0)))
    }

    pub fn get(&self) -> i64 {
        self.0.load(Ordering::Relaxed)
    }

    pub fn advance(&self, secs: i64) {
        self.0.fetch_add(secs, Ordering::Relaxed);
    }

    pub fn network_time(&self) -> Option<bool> {
        match self.1.load(Ordering::Relaxed) {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        }
    }
}

pub struct StubPlatform {
    backlight: Arc<AtomicU8>,
    root: PathBuf,
    clock: Clock,
    charge: Arc<AtomicU8>,
    percent: Arc<AtomicU8>,
    led: Arc<AtomicU8>,
    led_writes: Arc<AtomicUsize>,
    headphones: Arc<std::sync::atomic::AtomicBool>,
    read_only: Arc<std::sync::atomic::AtomicBool>,
    card_log: Option<PathBuf>,
}

pub fn led_code(state: LedState) -> u8 {
    match state {
        LedState::Off => 0,
        LedState::Running => 1,
        LedState::Low => 2,
        LedState::Charging => 3,
        LedState::Charged => 4,
    }
}

pub const CLOCK_IS_SET: i64 = 1_786_568_000;

pub fn panel(root: &Path, timeout: Duration) -> (Power, Arc<AtomicU8>) {
    let (power, backlight, _, _, _) = rig_with_charge(root, timeout, CLOCK_IS_SET, 0, 50);
    (power, backlight)
}

pub fn panel_with_battery(
    root: &Path,
    timeout: Duration,
    charge: u8,
    percent: u8,
) -> (Power, Arc<AtomicU8>) {
    let (power, backlight, _, _, _) = rig_with_charge(root, timeout, CLOCK_IS_SET, charge, percent);
    (power, backlight)
}

fn rig(root: &Path, timeout: Duration, secs: i64) -> (Power, Arc<AtomicU8>, Clock) {
    let (power, backlight, clock, _, _) = rig_with_charge(root, timeout, secs, 0, 50);
    (power, backlight, clock)
}

fn rig_with_charge(
    root: &Path,
    timeout: Duration,
    secs: i64,
    charge: u8,
    percent: u8,
) -> (Power, Arc<AtomicU8>, Clock, Arc<AtomicU8>, Arc<AtomicU8>) {
    let (power, backlight, clock, charge, percent, _led, _led_writes) =
        rig_with_led(root, timeout, secs, charge, percent);
    (power, backlight, clock, charge, percent)
}

#[allow(clippy::type_complexity)]
fn rig_with_led(
    root: &Path,
    timeout: Duration,
    secs: i64,
    charge: u8,
    percent: u8,
) -> (
    Power,
    Arc<AtomicU8>,
    Clock,
    Arc<AtomicU8>,
    Arc<AtomicU8>,
    Arc<AtomicU8>,
    Arc<AtomicUsize>,
) {
    let backlight = Arc::new(AtomicU8::new(0));
    let clock = Clock::at(secs);
    let charge = Arc::new(AtomicU8::new(charge));
    let percent = Arc::new(AtomicU8::new(percent));
    let led = Arc::new(AtomicU8::new(u8::MAX));
    let led_writes = Arc::new(AtomicUsize::new(0));
    let platform = StubPlatform {
        backlight: backlight.clone(),
        root: root.to_path_buf(),
        clock: clock.clone(),
        charge: charge.clone(),
        percent: percent.clone(),
        led: led.clone(),
        led_writes: led_writes.clone(),
        headphones: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        read_only: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        card_log: None,
    };
    (
        Power::new(Box::new(platform), timeout),
        backlight,
        clock,
        charge,
        percent,
        led,
        led_writes,
    )
}

pub fn session_with_platform(root: &Path) -> (Session, Motor) {
    clocked(root);
    let platform = SimPlatform::at(root.to_path_buf());
    let motor = platform.motor();
    let mut session = Session::boot(root.to_path_buf());
    session
        .app_mut()
        .set_power(Power::new(Box::new(platform), Duration::from_secs(300)));
    (session, motor)
}

pub fn app_booting_with_clock(root: &Path) -> (App, Clock) {
    app_booting_at(root, 0)
}

pub fn app_booting_at(root: &Path, secs: i64) -> (App, Clock) {
    let mut a = App::boot(root);
    let (power, _, clock) = rig(root, Duration::from_secs(60), secs);
    a.set_power(power);
    (a, clock)
}

impl Platform for StubPlatform {
    fn set_backlight(&mut self, step: u8) {
        self.backlight.store(step, Ordering::Relaxed);
    }

    fn charge(&self) -> Charge {
        match self.charge.load(Ordering::Relaxed) {
            1 => Charge::Discharging,
            2 => Charge::Charging,
            3 => Charge::Full,
            _ => Charge::Unknown,
        }
    }

    fn battery(&self) -> Option<Battery> {
        Some(Battery {
            percent: self.percent.load(Ordering::Relaxed),
            charge: self.charge(),
        })
    }

    fn set_led(&mut self, state: LedState) {
        self.led.store(led_code(state), Ordering::Relaxed);
        self.led_writes.fetch_add(1, Ordering::Relaxed);
    }

    fn restart(&mut self) -> ! {
        panic!("the stub platform never powers off")
    }

    fn poweroff(&mut self) -> ! {
        panic!("the stub platform never powers off")
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn now(&self) -> i64 {
        self.clock.get()
    }

    fn set_clock(&mut self, secs: i64) {
        self.clock.0.store(secs, Ordering::Relaxed);
    }

    fn set_network_time(&mut self, on: bool) {
        self.clock
            .1
            .store(if on { 2 } else { 1 }, Ordering::Relaxed);
    }

    fn set_rumble(&mut self, _strength: u16) {}

    fn headphones(&self) -> bool {
        self.headphones.load(Ordering::Relaxed)
    }

    fn card_read_only(&self) -> bool {
        self.read_only.load(Ordering::Relaxed)
    }

    fn card_log(&self) -> Option<PathBuf> {
        self.card_log.clone()
    }

    fn card_kernel_lines(&self) -> Vec<String> {
        vec![STUB_KERNEL_LINE.to_owned()]
    }
}

pub fn clocked(root: &Path) {
    let mut s = slot_store::read_slot_state(root);
    s.clock_set = true;
    write_slot_state(root, &s).expect("write slot.state");
}

pub fn boot(root: &Path) -> App {
    clocked(root);
    App::boot(root)
}

pub fn app_playing_in(root: &Path, stem: &str) -> App {
    app_playing_with(root, stem, StubSnapshot::boxed())
}

pub fn app_playing_with_jack(root: &Path, stem: &str) -> (App, Arc<std::sync::atomic::AtomicBool>) {
    let mut a = app_playing_with(root, stem, StubSnapshot::boxed());
    let jack = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let platform = StubPlatform {
        backlight: Arc::new(AtomicU8::new(0)),
        root: root.to_path_buf(),
        clock: Clock::at(CLOCK_IS_SET),
        charge: Arc::new(AtomicU8::new(0)),
        percent: Arc::new(AtomicU8::new(50)),
        led: Arc::new(AtomicU8::new(u8::MAX)),
        led_writes: Arc::new(AtomicUsize::new(0)),
        headphones: jack.clone(),
        read_only: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        card_log: None,
    };
    a.set_power(Power::new(Box::new(platform), Duration::from_secs(300)));
    (a, jack)
}

pub const STUB_KERNEL_LINE: &str = "FAT-fs (mmcblk0p7): error, invalid access to FAT";

pub fn app_playing_with_card(root: &Path, stem: &str) -> (App, Arc<std::sync::atomic::AtomicBool>) {
    app_playing_with_card_log(root, stem, None)
}

pub fn app_playing_with_card_log(
    root: &Path,
    stem: &str,
    card_log: Option<PathBuf>,
) -> (App, Arc<std::sync::atomic::AtomicBool>) {
    let mut a = app_playing_with(root, stem, StubSnapshot::boxed());
    let read_only = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let platform = StubPlatform {
        backlight: Arc::new(AtomicU8::new(0)),
        root: root.to_path_buf(),
        clock: Clock::at(CLOCK_IS_SET),
        charge: Arc::new(AtomicU8::new(0)),
        percent: Arc::new(AtomicU8::new(50)),
        led: Arc::new(AtomicU8::new(u8::MAX)),
        led_writes: Arc::new(AtomicUsize::new(0)),
        headphones: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        read_only: read_only.clone(),
        card_log,
    };
    a.set_power(Power::new(Box::new(platform), Duration::from_secs(300)));
    (a, read_only)
}

pub fn app_playing_with_charge(root: &Path, stem: &str) -> (App, Arc<AtomicU8>, Arc<AtomicU8>) {
    let mut a = app_playing_with(root, stem, StubSnapshot::boxed());
    let (power, _backlight, _clock, charge, percent) =
        rig_with_charge(root, Duration::from_secs(60), 0, 1, 50);
    a.set_power(power);
    (a, charge, percent)
}

pub fn app_playing_with_led(
    root: &Path,
    stem: &str,
) -> (
    App,
    Arc<AtomicU8>,
    Arc<AtomicU8>,
    Arc<AtomicU8>,
    Arc<AtomicUsize>,
) {
    let mut a = app_playing_with(root, stem, StubSnapshot::boxed());
    let (power, _backlight, _clock, charge, percent, led, led_writes) =
        rig_with_led(root, Duration::from_secs(60), 0, 1, 50);
    a.set_power(power);
    (a, charge, percent, led, led_writes)
}

pub fn app_in_switcher(root: &Path, stem: &str) -> App {
    let mut a = app_playing_in(root, stem);
    a.apply(slot_input::Action::Polaroids);
    a
}

pub fn app_playing_with_volume(root: &Path, volume: u8) -> App {
    seated(
        root,
        StubSnapshot::boxed(),
        SlotState {
            cart: Some("Emerald".to_string()),
            volume,
            clock_set: true,
            ..Default::default()
        },
    )
}

pub fn app_playing_with(root: &Path, stem: &str, snapshot: Box<dyn Snapshot>) -> App {
    seated(
        root,
        snapshot,
        SlotState {
            cart: Some(stem.to_string()),
            clock_set: true,
            ..Default::default()
        },
    )
}

fn seated(root: &Path, snapshot: Box<dyn Snapshot>, state: SlotState) -> App {
    write_slot_state(root, &state).expect("write slot.state");
    let mut a = App::boot(root);
    a.set_snapshot(snapshot);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a
}

pub fn through(a: &mut App, btn: slot_input::Btn) {
    let now = a.now();
    feed_through(a, btn, now, |a, action| a.apply(action));
}

pub fn through_at(a: &mut App, btn: slot_input::Btn, at: slot_input::Millis) {
    feed_through(a, btn, at, |a, action| a.apply_at(action, at));
}

fn feed_through(
    a: &mut App,
    btn: slot_input::Btn,
    now: slot_input::Millis,
    apply: impl Fn(&mut App, slot_input::Action),
) {
    let mut g = slot_input::Gestures::new();
    g.set_entries(a.binding_table().entries());
    for ev in [
        slot_input::RawEvent::Down(btn),
        slot_input::RawEvent::Up(btn),
    ] {
        g.set_context(a.input_context());
        for action in g.feed(ev, now) {
            apply(a, action);
        }
    }
}

pub fn connect(a: &mut App) {
    use slot::wifi::Backend;
    let mut fake = slot::wifi::Fake::demo();
    fake.add("Home", "password", false);
    a.set_wifi(slot::wifi::Wifi::inline(Box::new(fake)));
    let tick = |a: &mut App| {
        let t = a.now();
        a.tick_ms(t + 1_000);
    };
    for _ in 0..10 {
        if a.wifi().held {
            break;
        }
        tick(a);
    }
    assert!(a.wifi().held, "the app never held the radio");
    a.wifi_send(slot::wifi::Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    for _ in 0..4 {
        tick(a);
    }
    assert!(a.wifi_up(), "the fake network did not connect");
}
