//! armGBA's link cable between two devices, each a whole slot session in its own process on the
//! real armGBA core, with slot's own transport between them over loopback sockets. A shim on the
//! socket can delay the bytes as a radio would. Everything here is a simulation on one machine.
//!
//! The parent test spawns this test binary again as each device: `device` serves commands on
//! stdin and answers on stdout lines that start with `@ `.

mod arm;
mod common;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use arm::{Asm, Cond};
use slot::app::Phase;
use slot::audio::{AudioSink, StubSink};
use slot::emu::EmuHandle;
use slot::link_net::TcpLink;
use slot::link_radio::{HostAp, LinkRole, RadioRole};
use slot::link_start::LinkStarter;
use slot::persist::Snapshot;
use slot::session::Session;
use slot_input::{Btn, Millis, RawEvent};
use slot_store::{write_slot_state, Core, Platform, SlotState};

const DEVICE: &str = "SLOT_ARMGBA_DEVICE";

fn vendored(core: Core) -> Option<PathBuf> {
    let path = common::repo_root()
        .join("vendor")
        .join(slot::core::dylib_name(core));
    path.exists().then_some(path)
}

fn armgba() -> bool {
    let here = vendored(Core::Armgba).is_some();
    if !here {
        eprintln!("no armGBA core on this host, skipping");
    }
    here
}

// The test programs. Registers: r0 I/O, r2 VRAM, r3 frames, r5 I/O + 0x100 (serial), r7 I/O +
// 0x200 (interrupts), r8 transfers ended, r9 errors, r10 transfers left this frame, r11 the most
// transfers (0: no limit), r12 in V-blank, r13 the other console's last count, r14 frames since a
// transfer ended; r1, r4 and r6 are scratch. Row 0 of the screen shows: frames, transfers,
// the received words, errors and SIOCNT.

const IO: u32 = 0;
const T: u32 = 1;
const VRAM: u32 = 2;
const FRAMES: u32 = 3;
const A: u32 = 4;
const SIO: u32 = 5;
const B: u32 = 6;
const IRQ: u32 = 7;
const DONE: u32 = 8;
const ERRORS: u32 = 9;
const BUDGET: u32 = 10;
const LIMIT: u32 = 11;
const VBLANK: u32 = 12;
const PREV: u32 = 13;
const IDLE: u32 = 14;

const FRESH: u32 = 0x100;

const SIODATA32_LO: u32 = 0x20;
const SIODATA32_HI: u32 = 0x22;
const SIOMULTI0: u32 = 0x20;
const SIOMULTI1: u32 = 0x22;
const SIOCNT: u32 = 0x28;
const SIODATA8: u32 = 0x2A;
const SIOMLT_SEND: u32 = 0x2A;
const KEYINPUT: u32 = 0x30;
const RCNT: u32 = 0x34;
const IF: u32 = 0x02;
const VCOUNT: u32 = 0x06;

/// Multiplayer mode at 115200 bps, with the interrupt flag on each transfer's end.
const MULTI: u32 = 0x6003;

const PX_FRAMES: usize = 0;
const PX_TRANSFERS: usize = 1;
const PX_LOW: usize = 2;
const PX_HIGH: usize = 3;
const PX_ERRORS: usize = 4;
const PX_SIOCNT: usize = 5;

fn px(x: usize) -> u32 {
    x as u32 * 2
}

/// The screen in bitmap mode, the serial port as `siocnt` says, and 5A A5 at the start of the
/// cartridge's SRAM: the save a link must leave as it was.
fn setup(a: &mut Asm, siocnt: u32, limit: u32) {
    a.mov(IO, 0x0400_0000);
    a.mov(T, 0x400);
    a.orr(T, T, 3);
    a.str(T, IO, 0);
    a.mov(VRAM, 0x0600_0000);
    a.mov(FRAMES, 0);
    a.add(SIO, IO, 0x100);
    a.add(IRQ, IO, 0x200);
    a.mov(B, 0);
    a.strh(B, SIO, RCNT);
    a.mov(B, siocnt & 0xFF00);
    a.orr(B, B, siocnt & 0xFF);
    a.strh(B, SIO, SIOCNT);
    a.mov(DONE, 0);
    a.mov(ERRORS, 0);
    a.mov(BUDGET, 0);
    a.mov(LIMIT, limit);
    a.mov(VBLANK, 0);
    a.mov(PREV, FRESH);
    a.mov(IDLE, 0);
    a.mov(T, 0x0E00_0000);
    a.mov(B, 0x5A);
    a.strb(B, T, 0);
    a.mov(B, 0xA5);
    a.strb(B, T, 1);
}

/// From "loop": a transfer's end goes to "ended"; the start of V-blank counts the frame and goes
/// to "frame_done", after 30 frames without a transfer by way of the code that follows; any other
/// pass goes to `otherwise`.
fn each_pass(a: &mut Asm, per_frame: u32, otherwise: &'static str) {
    a.label("loop");
    a.ldrh(A, IRQ, IF);
    a.tst(A, 0x80);
    a.b(Cond::Ne, "ended");
    a.ldrh(A, IO, VCOUNT);
    a.cmp(A, 160);
    a.mov_if(Cond::Eq, T, 1);
    a.mov_if(Cond::Ne, T, 0);
    a.cmp_reg(T, VBLANK);
    a.mov_reg(VBLANK, T);
    a.b(Cond::Eq, otherwise);
    a.cmp(T, 0);
    a.b(Cond::Eq, otherwise);
    a.add(FRAMES, FRAMES, 1);
    a.strh(FRAMES, VRAM, px(PX_FRAMES));
    a.ldrh(A, SIO, SIOCNT);
    a.strh(A, VRAM, px(PX_SIOCNT));
    a.mov(BUDGET, per_frame);
    a.add(IDLE, IDLE, 1);
    a.cmp(IDLE, 30);
    a.b(Cond::Lo, "frame_done");
    a.mov(PREV, FRESH);
    a.mov(IDLE, 0);
}

/// A transfer's end clears the busy bit a cycle before it raises IF: with the busy bit clear,
/// count the transfer before readying the next, or the next would go out with the old count.
fn ended_first(a: &mut Asm) {
    a.ldrh(T, IRQ, IF);
    a.tst(T, 0x80);
    a.b(Cond::Ne, "ended");
}

/// Rows 1 and 2 of the screen keep each transfer's received low and high halves, by its number.
fn keep_history(a: &mut Asm) {
    a.cmp(DONE, 240);
    a.b(Cond::Hs, "no_history");
    a.add_lsl(T, VRAM, DONE, 1);
    a.add(T, T, 480);
    a.strh(A, T, 0);
    a.add(T, T, 480);
    a.strh(B, T, 0);
    a.label("no_history");
}

/// The other console's count must follow the last one, unless the link is new. `A` holds it,
/// `mask` its bits; the end counts the transfer and shows it.
fn count(a: &mut Asm, mask: u32) {
    a.and(A, A, mask);
    a.cmp(PREV, FRESH);
    a.b(Cond::Eq, "counted");
    a.add(B, PREV, 1);
    a.and(B, B, mask);
    a.cmp_reg(A, B);
    a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
    a.label("counted");
    a.mov_reg(PREV, A);
    a.add(DONE, DONE, 1);
    a.strh(DONE, VRAM, px(PX_TRANSFERS));
    a.strh(ERRORS, VRAM, px(PX_ERRORS));
}

/// Both consoles run this. The parent starts up to `per_frame` transfers a frame once every
/// console is ready (SD high), `limit` in all (0: no limit). Each console sends its own count,
/// tagged 0x1000 by the parent and 0x2000 by the child, and checks the other's follows on.
fn multiplayer(per_frame: u32, limit: u32) -> Asm {
    let mut a = Asm::new();
    setup(&mut a, MULTI, limit);
    each_pass(&mut a, per_frame, "word");
    a.mov(B, MULTI & 0xFF00);
    a.orr(B, B, MULTI & 0xFF);
    a.strh(B, SIO, SIOCNT);
    a.label("frame_done");
    a.label("word");
    a.ldrh(A, SIO, SIOCNT);
    a.and(T, DONE, 0xFF);
    a.tst(A, 4);
    a.orr_if(Cond::Eq, B, T, 0x1000);
    a.orr_if(Cond::Ne, B, T, 0x2000);
    a.strh(B, SIO, SIOMLT_SEND);
    a.label("start");
    a.ldrh(A, SIO, SIOCNT);
    a.tst(A, 4);
    a.b(Cond::Ne, "loop");
    a.tst(A, 8);
    a.b(Cond::Eq, "loop");
    a.tst(A, 0x80);
    a.b(Cond::Ne, "loop");
    ended_first(&mut a);
    a.cmp(BUDGET, 0);
    a.b(Cond::Eq, "loop");
    a.cmp(LIMIT, 0);
    a.b(Cond::Eq, "go");
    a.cmp_reg(DONE, LIMIT);
    a.b(Cond::Hs, "loop");
    a.label("go");
    a.sub(BUDGET, BUDGET, 1);
    a.orr(A, A, 0x80);
    a.strh(A, SIO, SIOCNT);
    a.b(Cond::Al, "loop");
    a.label("ended");
    a.mov(B, 0x80);
    a.strh(B, IRQ, IF);
    a.mov(IDLE, 0);
    a.ldrh(A, SIO, SIOMULTI0);
    a.ldrh(B, SIO, SIOMULTI1);
    a.strh(A, VRAM, px(PX_LOW));
    a.strh(B, VRAM, px(PX_HIGH));
    keep_history(&mut a);
    a.ldrh(T, SIO, SIOCNT);
    a.tst(T, 4);
    a.b(Cond::Ne, "as_child");
    a.and(T, DONE, 0xFF);
    a.orr(T, T, 0x1000);
    a.cmp_reg(A, T);
    a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
    a.mov_reg(A, B);
    a.mov(T, 0x2000);
    a.b(Cond::Al, "other");
    a.label("as_child");
    a.and(T, DONE, 0xFF);
    a.orr(T, T, 0x2000);
    a.cmp_reg(B, T);
    a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
    a.mov(T, 0x1000);
    a.label("other");
    a.and(B, A, 0xFF00);
    a.cmp_reg(B, T);
    a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
    count(&mut a, 0xFF);
    a.b(Cond::Al, "word");
    a
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Width {
    Byte,
    Word,
}

/// Normal mode. The master clocks one transfer a frame while A is held, `limit` in all; the
/// slave waits on the master's clock. A 32-bit master sends 0x2500_10nn and a slave 0x3A00_20nn,
/// nn their own count; an 8-bit master sends 0x40 + n and a slave 0xC0 + n, n in six bits.
fn normal(width: Width, master: bool, limit: u32) -> Asm {
    let mode = match width {
        Width::Byte => 0x4000,
        Width::Word => 0x5000,
    };
    let siocnt = match master {
        true => mode | 3,
        false => mode,
    };
    let mut a = Asm::new();
    setup(&mut a, siocnt, limit);
    each_pass(&mut a, 1, "start");
    a.label("frame_done");
    a.label("start");
    a.ldrh(A, SIO, SIOCNT);
    a.tst(A, 0x80);
    a.b(Cond::Ne, "loop");
    ended_first(&mut a);
    if master {
        a.ldrh(T, SIO, KEYINPUT);
        a.tst(T, 1);
        a.b(Cond::Ne, "loop");
        a.cmp(BUDGET, 0);
        a.b(Cond::Eq, "loop");
        a.cmp(LIMIT, 0);
        a.b(Cond::Eq, "go");
        a.cmp_reg(DONE, LIMIT);
        a.b(Cond::Hs, "loop");
        a.label("go");
        a.sub(BUDGET, BUDGET, 1);
    }
    match width {
        Width::Word => {
            a.and(T, DONE, 0xFF);
            a.orr(T, T, if master { 0x1000 } else { 0x2000 });
            a.strh(T, SIO, SIODATA32_LO);
            a.mov(T, if master { 0x2500 } else { 0x3A00 });
            a.strh(T, SIO, SIODATA32_HI);
        }
        Width::Byte => {
            a.and(T, DONE, 0x3F);
            a.orr(T, T, if master { 0x40 } else { 0xC0 });
            a.strh(T, SIO, SIODATA8);
        }
    }
    a.mov(A, siocnt & 0xFF00);
    a.orr(A, A, siocnt & 0xFF | 0x80);
    a.strh(A, SIO, SIOCNT);
    a.b(Cond::Al, "loop");
    a.label("ended");
    a.mov(B, 0x80);
    a.strh(B, IRQ, IF);
    a.mov(IDLE, 0);
    match width {
        Width::Word => {
            a.ldrh(A, SIO, SIODATA32_LO);
            a.ldrh(B, SIO, SIODATA32_HI);
            a.strh(A, VRAM, px(PX_LOW));
            a.strh(B, VRAM, px(PX_HIGH));
            keep_history(&mut a);
            a.cmp(B, if master { 0x3A00 } else { 0x2500 });
            a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
            a.and(B, A, 0xFF00);
            a.cmp(B, if master { 0x2000 } else { 0x1000 });
            a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
            count(&mut a, 0xFF);
        }
        Width::Byte => {
            a.ldrh(A, SIO, SIODATA8);
            a.and(A, A, 0xFF);
            a.strh(A, VRAM, px(PX_LOW));
            a.mov(B, 0);
            keep_history(&mut a);
            a.and(B, A, 0xC0);
            a.cmp(B, if master { 0xC0 } else { 0x40 });
            a.add_if(Cond::Ne, ERRORS, ERRORS, 1);
            count(&mut a, 0x3F);
        }
    }
    a.b(Cond::Al, "loop");
    a
}

fn rom(name: &str, title: &str, program: Asm) -> PathBuf {
    let mut bytes = arm::cartridge(title, "SLTE", program);
    bytes[0x7F00..0x7F09].copy_from_slice(b"SRAM_V113");
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.gba"));
    std::fs::write(&path, bytes).expect("write rom");
    path
}

fn painted(picture: &[u8], at: usize) -> u16 {
    let pixel = &picture[at * 4..at * 4 + 4];
    u16::from(pixel[2] >> 3) | u16::from(pixel[1] >> 3) << 5 | u16::from(pixel[0] >> 3) << 10
}

// The device: one slot session.

struct Rig {
    session: Box<Session>,
    _root: tempfile::TempDir,
    held: Vec<RawEvent>,
    now: Millis,
}

impl Rig {
    fn emu(&self) -> Option<&EmuHandle> {
        self.session.emu()
    }

    fn tick(&mut self, began: Instant) {
        let wall = began.elapsed().as_millis() as Millis;
        if wall < self.now + 16 {
            return;
        }
        self.now = wall;
        self.session.feed(std::mem::take(&mut self.held), self.now);
        self.session.update(1.0 / 60.0);
    }

    /// Takes the newest frame, if one came since, into `seen`: rows 0 to 2 of the screen.
    fn look(&self, seen: &mut Vec<u16>) -> bool {
        let Some(frame) = self.emu().and_then(EmuHandle::latest_frame) else {
            return false;
        };
        *seen = (0..3 * 240).map(|at| painted(&frame, at)).collect();
        true
    }
}

fn pixels(seen: &[u16]) -> [u16; 6] {
    std::array::from_fn(|x| seen.get(x).copied().unwrap_or_default())
}

/// The words each transfer received, from rows 1 and 2.
fn history(seen: &[u16], transfers: usize) -> Vec<(u16, u16)> {
    (0..transfers.min(240))
        .map(|k| {
            let at = |row: usize| seen.get(row * 240 + k).copied().unwrap_or_default();
            (at(1), at(2))
        })
        .collect()
}

fn root_with(stem: &str, rom: &Path, core: Core) -> tempfile::TempDir {
    let d = common::tmp_root_with_real_carts(&[stem]);
    let cart = d
        .path()
        .join("Games")
        .join(Platform::Gba.dir_name())
        .join(format!("{stem}.{}", Platform::Gba.extensions()[0]));
    std::fs::copy(rom, &cart).expect("place the cart");
    let dylib = vendored(core).expect("the core to run");
    std::os::unix::fs::symlink(
        &dylib,
        d.path().join("System").join(slot::core::dylib_name(core)),
    )
    .expect("plant the core");
    slot_store::write_selected_core(d.path(), Platform::Gba, stem, core).expect("write core");
    d
}

fn boot(core: Core, rom: &Path, began: Instant) -> Rig {
    let stem = "Linked";
    let root = root_with(stem, rom, core);
    write_slot_state(
        root.path(),
        &SlotState {
            cart: Some(stem.into()),
            clock_set: true,
            ..Default::default()
        },
    )
    .expect("write slot.state");
    let mut sink = StubSink::draining();
    sink.open(32_768).expect("open the stub sink");
    let session = Box::new(Session::boot_with(
        root.path().to_path_buf(),
        Box::new(sink),
    ));
    let mut rig = Rig {
        session,
        _root: root,
        held: Vec::new(),
        now: 0,
    };
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        rig.tick(began);
        let playing = matches!(rig.session.app().phase(), Phase::Playing { .. });
        if playing && rig.emu().is_some_and(|e| e.published_count() > 30) {
            return rig;
        }
        assert!(Instant::now() < deadline, "the game never started");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn starter(role: LinkRole, port: u16) -> LinkStarter {
    let radio = match role {
        LinkRole::Host => RadioRole::Host,
        LinkRole::Join => RadioRole::Join(HostAp {
            freq_mhz: 2412,
            bssid: [0; 6],
        }),
    };
    LinkStarter::spawn_with(
        Box::new(|_, _| Ok(None)),
        Box::new(|| {}),
        radio,
        port,
        Box::new(move |port, cancel| match role {
            LinkRole::Host => {
                TcpLink::host_until("127.0.0.1", port, Duration::from_secs(30), cancel)
            }
            LinkRole::Join => {
                TcpLink::join_until("127.0.0.1", port, Duration::from_secs(30), cancel)
            }
        }),
    )
}

fn button(name: &str) -> Btn {
    match name {
        "a" => Btn::A,
        "b" => Btn::B,
        "select" => Btn::Select,
        "menu" => Btn::Menu,
        "r2" => Btn::R2,
        "l2" => Btn::L2,
        other => panic!("no button {other}"),
    }
}

struct Watch {
    first: [u16; 6],
    last: [u16; 6],
    began: Instant,
    moved: Instant,
    drawn: Instant,
    stall: Duration,
    frozen: Duration,
    published: u64,
    back: bool,
}

impl Watch {
    fn new(pixels: [u16; 6], published: u64) -> Watch {
        let now = Instant::now();
        Watch {
            first: pixels,
            last: pixels,
            began: now,
            moved: now,
            drawn: now,
            stall: Duration::ZERO,
            frozen: Duration::ZERO,
            published,
            back: false,
        }
    }

    fn see(&mut self, pixels: [u16; 6]) {
        let now = Instant::now();
        let step = pixels[PX_FRAMES].wrapping_sub(self.last[PX_FRAMES]) & 0x7FFF;
        self.back |= step > 0x4000;
        if pixels[PX_TRANSFERS] != self.last[PX_TRANSFERS] {
            self.stall = self.stall.max(now - self.moved);
            self.moved = now;
        }
        if pixels[PX_FRAMES] != self.last[PX_FRAMES] {
            self.frozen = self.frozen.max(now - self.drawn);
            self.drawn = now;
        }
        self.last = pixels;
    }

    fn report(&self, published: u64) -> String {
        let now = Instant::now();
        let ms = (now - self.began).as_millis();
        let stall = self.stall.max(now - self.moved).as_millis();
        let frozen = self.frozen.max(now - self.drawn).as_millis();
        let delta = |x: usize| self.last[x].wrapping_sub(self.first[x]) & 0x7FFF;
        format!(
            "ms={ms} frames={} transfers={} errors={} new_errors={} stall={stall} frozen={frozen} \
             published={} low={:#x} high={:#x} siocnt={:#x} done={} back={}",
            delta(PX_FRAMES),
            delta(PX_TRANSFERS),
            self.last[PX_ERRORS],
            delta(PX_ERRORS),
            published - self.published,
            self.last[PX_LOW],
            self.last[PX_HIGH],
            self.last[PX_SIOCNT],
            self.last[PX_TRANSFERS],
            u8::from(self.back),
        )
    }
}

fn serve() {
    let (tx, rx) = channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let began = Instant::now();
    let mut rig: Option<Rig> = None;
    let mut seen: Vec<u16> = Vec::new();
    let say = |words: String| {
        println!("@ {words}");
        let _ = std::io::stdout().flush();
    };
    loop {
        if let Some(r) = rig.as_mut() {
            r.tick(began);
            r.look(&mut seen);
        }
        let line = match rx.recv_timeout(Duration::from_millis(2)) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["boot", core, rom] => {
                let core = Core::parse(core).expect("a core");
                rig = Some(boot(core, Path::new(rom), began));
                say("ready".into());
            }
            ["link", role, port] => {
                let role = match *role {
                    "host" => LinkRole::Host,
                    _ => LinkRole::Join,
                };
                let port: u16 = port.parse().expect("port");
                let client_id = match role {
                    LinkRole::Host => 0,
                    LinkRole::Join => 1,
                };
                let session = &mut rig.as_mut().expect("booted").session;
                session.app_mut().start_link(starter(role, port), client_id);
                say("linking".into());
            }
            ["status"] => {
                let r = rig.as_ref().expect("booted");
                let emu = r.emu().expect("a running core");
                let app = r.session.app();
                say(format!(
                    "linked={} live={} lost={} ended={} toast={} speed={:?} rewind={} ff={} load={}",
                    u8::from(app.link_active()),
                    u8::from(emu.net().is_active()),
                    u8::from(emu.link_lost()),
                    u8::from(emu.peer_ended()),
                    format!("{:?}", app.toast()).replace(' ', ""),
                    emu.observed_speed(),
                    u8::from(app.may_rewind()),
                    u8::from(app.may_fast_forward()),
                    u8::from(app.may_load_state()),
                ));
            }
            ["press", name] | ["release", name] => {
                let btn = button(name);
                if let Some(r) = rig.as_mut() {
                    r.held.push(match words[0] {
                        "press" => RawEvent::Down(btn),
                        _ => RawEvent::Up(btn),
                    });
                }
                say("pressed".into());
            }
            ["end"] => {
                let r = rig.as_mut().expect("booted");
                let steps = [
                    vec![RawEvent::Down(Btn::Select), RawEvent::Down(Btn::Menu)],
                    vec![RawEvent::Up(Btn::Menu), RawEvent::Up(Btn::Select)],
                    vec![RawEvent::Down(Btn::A)],
                    vec![RawEvent::Up(Btn::A)],
                ];
                for events in steps {
                    r.held.extend(events);
                    let until = Instant::now() + Duration::from_millis(50);
                    while Instant::now() < until {
                        r.tick(began);
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                say("ended".into());
            }
            ["watch", ms] => {
                let r = rig.as_mut().expect("booted");
                let span = Duration::from_millis(ms.parse().expect("ms"));
                let published = r.emu().map_or(0, EmuHandle::published_count);
                let mut watch = Watch::new(pixels(&seen), published);
                let until = Instant::now() + span;
                while Instant::now() < until {
                    r.tick(began);
                    if r.look(&mut seen) {
                        watch.see(pixels(&seen));
                    }
                    std::thread::sleep(Duration::from_micros(500));
                }
                let published = r.emu().map_or(0, EmuHandle::published_count);
                say(watch.report(published));
            }
            ["history", n] => {
                let words = history(&seen, n.parse().expect("count"));
                let shown: Vec<String> = words
                    .iter()
                    .map(|(low, high)| format!("{high:04x}:{low:04x}"))
                    .collect();
                say(format!("words={}", shown.join(",")));
            }
            ["sav"] => {
                let r = rig.as_ref().expect("booted");
                let sav = r.emu().and_then(|e| e.snapshot().save_ram());
                say(match sav {
                    Some(bytes) => format!(
                        "len={} head={:02x?} hash={:016x}",
                        bytes.len(),
                        &bytes[..bytes.len().min(4)],
                        fnv1a(&bytes)
                    ),
                    None => "len=0".into(),
                });
            }
            ["state"] => {
                let r = rig.as_ref().expect("booted");
                let state = r
                    .emu()
                    .and_then(|e| e.request_state().recv_timeout(Duration::from_secs(5)).ok());
                say(format!("len={}", state.map_or(0, |s| s.len())));
            }
            ["quit"] => return,
            other => panic!("no command {other:?}"),
        }
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    })
}

#[test]
fn device() {
    if std::env::var_os(DEVICE).is_some() {
        serve();
    }
}

// The parent's side.

struct Device {
    name: String,
    child: Child,
    stdin: ChildStdin,
    answers: Receiver<String>,
}

impl Device {
    fn spawn(name: &str) -> Device {
        let log = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("armgba-{name}.log"));
        let mut child = Command::new(std::env::current_exe().expect("this test binary"))
            .args(["--exact", "device", "--nocapture", "--test-threads=1"])
            .env(DEVICE, name)
            .env("SLOT_SILENT", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(std::fs::File::create(log).expect("device log"))
            .spawn()
            .expect("spawn a device");
        let stdout = child.stdout.take().expect("device stdout");
        let (tx, answers) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                // The first answer shares its line with libtest's "test device ... ".
                if let Some((_, answer)) = line.split_once("@ ") {
                    if tx.send(answer.to_string()).is_err() {
                        return;
                    }
                }
            }
        });
        Device {
            name: name.into(),
            stdin: child.stdin.take().expect("device stdin"),
            child,
            answers,
        }
    }

    fn booted(name: &str, core: &str, rom: &Path) -> Device {
        let mut d = Device::spawn(name);
        assert_eq!(d.ask(&format!("boot {core} {}", rom.display())), "ready");
        d
    }

    fn send(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").expect("write to the device");
        self.stdin.flush().expect("flush to the device");
    }

    fn answer(&mut self) -> String {
        self.answers
            .recv_timeout(Duration::from_secs(90))
            .unwrap_or_else(|e| panic!("{} did not answer: {e}", self.name))
    }

    fn ask(&mut self, command: &str) -> String {
        self.send(command);
        self.answer()
    }

    fn status(&mut self) -> Fields {
        Fields::parse(&self.ask("status"))
    }

    fn watch(&mut self, ms: u64) -> Fields {
        Fields::parse(&self.ask(&format!("watch {ms}")))
    }

    fn wait_for(&mut self, what: &str, within: Duration, done: impl Fn(&Fields) -> bool) -> Fields {
        let deadline = Instant::now() + within;
        loop {
            let status = self.status();
            if done(&status) {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "{}: {what} never happened: {status:?}",
                self.name
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.kill();
    }
}

#[derive(Debug, Clone)]
struct Fields(Vec<(String, String)>);

impl Fields {
    fn parse(line: &str) -> Fields {
        Fields(
            line.split_whitespace()
                .filter_map(|w| w.split_once('='))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    fn text(&self, key: &str) -> &str {
        self.0
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or_else(|| panic!("no {key} in {self:?}"))
    }

    fn num(&self, key: &str) -> u64 {
        let v = self.text(key);
        match v.strip_prefix("0x") {
            Some(hex) => u64::from_str_radix(hex, 16),
            None => v.parse(),
        }
        .unwrap_or_else(|_| panic!("{key}={v} is no number"))
    }

    fn on(&self, key: &str) -> bool {
        self.num(key) == 1
    }
}

/// A radio between the joiner and the host: each chunk of bytes crosses after the one-way delay
/// plus up to `jitter` more, and now and then (`loss` of chunks) after a TCP retransmission timeout
/// more. TCP keeps the order, so a late chunk holds back the ones behind it; nothing is lost.
#[derive(Copy, Clone, Debug)]
struct Net {
    one_way: Duration,
    jitter: Duration,
    loss: f64,
    rto: Duration,
}

impl Net {
    fn ms(one_way: u64, jitter: u64) -> Net {
        Net {
            one_way: Duration::from_millis(one_way),
            jitter: Duration::from_millis(jitter),
            loss: 0.0,
            rto: Duration::from_millis(200),
        }
    }
}

struct Shim {
    port: u16,
    net: Arc<Mutex<Net>>,
}

impl Shim {
    fn to(host_port: u16) -> Shim {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind the shim");
        let port = listener.local_addr().expect("shim address").port();
        let net = Arc::new(Mutex::new(Net::ms(0, 0)));
        let shared = net.clone();
        std::thread::spawn(move || {
            for (n, joiner) in listener.incoming().enumerate() {
                let Ok(joiner) = joiner else {
                    return;
                };
                let deadline = Instant::now() + Duration::from_secs(30);
                let host = loop {
                    match TcpStream::connect(("127.0.0.1", host_port)) {
                        Ok(s) => break s,
                        Err(_) if Instant::now() < deadline => {
                            std::thread::sleep(Duration::from_millis(20))
                        }
                        Err(e) => panic!("the host never listened: {e}"),
                    }
                };
                let _ = joiner.set_nodelay(true);
                let _ = host.set_nodelay(true);
                let seed = 0x9E37_79B9_7F4A_7C15 ^ n as u64;
                carry(
                    joiner.try_clone().expect("clone"),
                    host.try_clone().expect("clone"),
                    shared.clone(),
                    seed,
                );
                carry(host, joiner, shared.clone(), seed.rotate_left(17));
            }
        });
        Shim { port, net }
    }

    fn set(&self, net: Net) {
        *self.net.lock().expect("shim settings") = net;
    }
}

fn uniform(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64
}

fn carry(mut from: TcpStream, mut to: TcpStream, net: Arc<Mutex<Net>>, seed: u64) {
    let (tx, rx) = channel::<Option<(Instant, Vec<u8>)>>();
    std::thread::spawn(move || {
        let mut rng = seed;
        let mut last = Instant::now();
        let mut buf = [0u8; 8192];
        loop {
            let n = match from.read(&mut buf) {
                Ok(0) | Err(_) => {
                    let _ = tx.send(None);
                    return;
                }
                Ok(n) => n,
            };
            let net = *net.lock().expect("shim settings");
            let mut delay = net.one_way + net.jitter.mul_f64(uniform(&mut rng));
            if uniform(&mut rng) < net.loss {
                delay += net.rto;
            }
            let at = (Instant::now() + delay).max(last);
            last = at;
            if tx.send(Some((at, buf[..n].to_vec()))).is_err() {
                return;
            }
        }
    });
    std::thread::spawn(move || {
        for item in rx {
            let Some((at, bytes)) = item else {
                break;
            };
            if let Some(wait) = at.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
            if to.write_all(&bytes).is_err() {
                break;
            }
        }
        let _ = to.shutdown(Shutdown::Both);
    });
}

/// A host and a joiner on these games, linked through a shim, both live.
fn linked_pair(host_rom: &Path, joiner_rom: &Path) -> (Device, Device, Shim) {
    let mut host = Device::booted("host", "armgba", host_rom);
    let mut joiner = Device::booted("joiner", "armgba", joiner_rom);
    let port = common::free_port();
    let shim = Shim::to(port);
    host.ask(&format!("link host {port}"));
    joiner.ask(&format!("link join {}", shim.port));
    for d in [&mut host, &mut joiner] {
        d.wait_for("the link going live", Duration::from_secs(20), |s| {
            s.on("linked") && s.on("live")
        });
    }
    (host, joiner, shim)
}

/// Waits until a device has ended `transfers` transfers, then returns its screen.
fn transfers_reach(d: &mut Device, transfers: u64) -> Fields {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let w = d.watch(100);
        if w.num("done") >= transfers {
            return w;
        }
        assert!(
            Instant::now() < deadline,
            "{}: stuck at {} transfers: {w:?}",
            d.name,
            w.num("done")
        );
    }
}

/// Each transfer's received words on a device, `high:low` in hex, oldest first.
fn received(d: &mut Device, transfers: usize) -> Vec<String> {
    let answer = Fields::parse(&d.ask(&format!("history {transfers}")));
    answer
        .text("words")
        .split(',')
        .map(str::to_string)
        .collect()
}

fn words(each: impl Fn(u16) -> String) -> Vec<String> {
    (0..8).map(each).collect()
}

#[test]
fn two_armgba_consoles_trade_multiplayer_words_through_slots_link() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let game = rom("multiplayer-8", "SLOT MULTI", multiplayer(1, 8));
    let (mut host, mut joiner, _shim) = linked_pair(&game, &game);

    let parent = transfers_reach(&mut host, 8);
    let child = transfers_reach(&mut joiner, 8);
    std::thread::sleep(Duration::from_millis(500));

    let both = words(|k| format!("{:04x}:{:04x}", 0x2000 + k, 0x1000 + k));
    for (side, d) in [("host", &mut host), ("joiner", &mut joiner)] {
        let w = d.watch(200);
        let seen = received(d, 8);
        assert_eq!(w.num("done"), 8, "{side} ran past the limit: {w:?}");
        assert_eq!(
            seen, both,
            "{side}'s SIOMULTI1:SIOMULTI0 after each transfer"
        );
        assert_eq!(w.num("errors"), 0, "{side} saw a word out of turn: {w:?}");
    }
    assert_eq!(
        parent.num("siocnt") & 0x3C,
        0x08,
        "the host is not multiplayer ID 0 with every console ready: {parent:?}"
    );
    assert_eq!(
        child.num("siocnt") & 0x3C,
        0x1C,
        "the joiner is not multiplayer ID 1 with every console ready: {child:?}"
    );
}

/// Eight normal-mode transfers, the master on the host or the joiner: what the master and the
/// slave each received.
fn normal_mode_pair(width: Width, master_hosts: bool) -> (Vec<String>, Vec<String>) {
    let tag = format!("{width:?}").to_lowercase();
    let master = rom(
        &format!("master-{tag}"),
        "SLOT NORMAL",
        normal(width, true, 8),
    );
    let slave = rom(
        &format!("slave-{tag}"),
        "SLOT NORMAL",
        normal(width, false, 0),
    );
    let (host_rom, joiner_rom) = match master_hosts {
        true => (&master, &slave),
        false => (&slave, &master),
    };
    let (mut host, mut joiner, _shim) = linked_pair(host_rom, joiner_rom);
    let (m, s) = match master_hosts {
        true => (&mut host, &mut joiner),
        false => (&mut joiner, &mut host),
    };
    m.watch(1500);
    m.ask("press a");
    transfers_reach(m, 8);
    transfers_reach(s, 8);
    m.ask("release a");
    std::thread::sleep(Duration::from_millis(500));
    for (side, d) in [("master", &mut *m), ("slave", &mut *s)] {
        let w = d.watch(200);
        assert_eq!(w.num("done"), 8, "the {side} ran past the limit: {w:?}");
        assert_eq!(
            w.num("errors"),
            0,
            "the {side} saw a word out of turn: {w:?}"
        );
    }
    (received(m, 8), received(s, 8))
}

#[test]
fn normal_mode_32_bit_crosses_whichever_device_clocks_it() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    for master_hosts in [true, false] {
        let (master, slave) = normal_mode_pair(Width::Word, master_hosts);
        let at = if master_hosts { "host" } else { "joiner" };
        assert_eq!(
            master,
            words(|k| format!("3a00:{:04x}", 0x2000 + k)),
            "the master on the {at} received the slave's words"
        );
        assert_eq!(
            slave,
            words(|k| format!("2500:{:04x}", 0x1000 + k)),
            "the slave opposite the {at} received the master's words"
        );
    }
}

#[test]
fn normal_mode_8_bit_crosses_between_devices() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let (master, slave) = normal_mode_pair(Width::Byte, true);
    assert_eq!(master, words(|k| format!("0000:{:04x}", 0xC0 + k)));
    assert_eq!(slave, words(|k| format!("0000:{:04x}", 0x40 + k)));
}

/// Watches both devices over the same span.
fn watch_both(a: &mut Device, b: &mut Device, ms: u64) -> (Fields, Fields) {
    a.send(&format!("watch {ms}"));
    b.send(&format!("watch {ms}"));
    (Fields::parse(&a.answer()), Fields::parse(&b.answer()))
}

fn per_second(w: &Fields, key: &str) -> f64 {
    w.num(key) as f64 * 1000.0 / w.num("ms") as f64
}

struct Row {
    name: &'static str,
    host: Fields,
    joiner: Fields,
}

impl Row {
    fn line(&self) -> String {
        let (h, j) = (&self.host, &self.joiner);
        let held = |w: &Fields| w.num("published").saturating_sub(w.num("frames"));
        format!(
            "| {:<22} | {:>5.1} | {:>5.1} | {:>6.1} | {:>6.1} | {:>5} / {:<5} | {:>5} / {:<5} | {:>3} / {:<3} | {} / {} |",
            self.name,
            per_second(h, "frames"),
            per_second(j, "frames"),
            per_second(h, "transfers"),
            per_second(j, "transfers"),
            h.num("stall"),
            j.num("stall"),
            h.num("frozen"),
            j.num("frozen"),
            held(h),
            held(j),
            h.num("new_errors"),
            j.num("new_errors"),
        )
    }
}

const HEADER: &str = "| link (one way + jitter) | host fps | joiner fps | host transfers/s | joiner transfers/s | longest wait between transfers, ms (host / joiner) | longest frame, ms | frames shown again | new errors |";

/// Runs each kind of network for its span, after a settling second and a half on it.
fn sweep(
    host: &mut Device,
    joiner: &mut Device,
    shim: &Shim,
    cases: &[(&'static str, Net, u64)],
) -> Vec<Row> {
    let mut rows = Vec::new();
    for &(name, net, ms) in cases {
        shim.set(net);
        watch_both(host, joiner, 1500);
        let (h, j) = watch_both(host, joiner, ms);
        let row = Row {
            name,
            host: h,
            joiner: j,
        };
        eprintln!("{}", row.line());
        rows.push(row);
    }
    rows
}

fn lossy(net: Net, loss: f64) -> Net {
    Net { loss, ..net }
}

#[test]
fn the_cable_keeps_every_word_in_turn_as_the_network_slows_and_breaks_past_three_seconds() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let game = rom("multiplayer", "SLOT MULTI", multiplayer(1, 0));
    let (mut host, mut joiner, shim) = linked_pair(&game, &game);
    eprintln!("one transfer a frame (a game that links every frame)\n{HEADER}");
    let rows = sweep(
        &mut host,
        &mut joiner,
        &shim,
        &[
            ("loopback", Net::ms(0, 0), 4000),
            ("1 ms + 1", Net::ms(1, 1), 4000),
            ("5 ms + 5", Net::ms(5, 5), 4000),
            ("20 ms + 10", Net::ms(20, 10), 4000),
            ("50 ms + 25", Net::ms(50, 25), 5000),
            ("150 ms + 50", Net::ms(150, 50), 8000),
            ("5 ms + 5, 2% +200 ms", lossy(Net::ms(5, 5), 0.02), 6000),
            ("1600 ms", Net::ms(1600, 0), 10000),
            ("back to loopback", Net::ms(0, 0), 4000),
        ],
    );
    let row = |name: &str| rows.iter().find(|r| r.name == name).expect("a row");
    for r in rows.iter().filter(|r| r.name != "1600 ms") {
        for (side, w) in [("host", &r.host), ("joiner", &r.joiner)] {
            assert_eq!(
                w.num("new_errors"),
                0,
                "{}: {side} saw a word out of turn",
                r.name
            );
            assert!(w.num("transfers") > 0, "{}: {side}'s cable stopped", r.name);
            assert_eq!(w.num("back"), 0, "{}: {side}'s game went backwards", r.name);
        }
    }
    for name in ["loopback", "1 ms + 1", "back to loopback"] {
        for (side, w) in [("host", &row(name).host), ("joiner", &row(name).joiner)] {
            assert!(
                per_second(w, "frames") >= 55.0,
                "{name}: {side} below full speed: {w:?}"
            );
            assert!(
                per_second(w, "transfers") >= 55.0,
                "{name}: {side}'s transfers: {w:?}"
            );
        }
    }
    let broken = &row("1600 ms").host;
    assert!(
        broken.num("new_errors") > 0,
        "a reply later than armGBA's three seconds still arrived in time: {broken:?}"
    );

    drop((host, joiner, shim));
    let burst = rom("multiplayer-burst", "SLOT MULTI", multiplayer(255, 0));
    let (mut host, mut joiner, shim) = linked_pair(&burst, &burst);
    eprintln!("transfers back to back (a game sending a block)\n{HEADER}");
    let rows = sweep(
        &mut host,
        &mut joiner,
        &shim,
        &[
            ("loopback", Net::ms(0, 0), 4000),
            ("1 ms + 1", Net::ms(1, 1), 4000),
            ("5 ms + 5", Net::ms(5, 5), 4000),
        ],
    );
    for r in &rows {
        for (side, w) in [("host", &r.host), ("joiner", &r.joiner)] {
            assert_eq!(
                w.num("new_errors"),
                0,
                "burst {}: {side} saw a word out of turn",
                r.name
            );
            assert!(
                w.num("transfers") > 0,
                "burst {}: {side}'s cable stopped",
                r.name
            );
        }
    }
}

#[test]
fn back_to_back_transfers_keep_both_games_running_over_a_20_ms_link() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let burst = rom("multiplayer-burst", "SLOT MULTI", multiplayer(255, 0));
    let (mut host, mut joiner, shim) = linked_pair(&burst, &burst);
    shim.set(Net::ms(20, 10));
    for _ in 0..6 {
        let (h, j) = watch_both(&mut host, &mut joiner, 5000);
        for (side, w) in [("host", &h), ("joiner", &j)] {
            assert!(
                w.num("published") > 0,
                "{side}'s core stopped returning frames: {w:?}"
            );
            assert!(w.num("transfers") > 0, "{side}'s cable stopped: {w:?}");
        }
    }
}

#[test]
fn a_session_outlives_its_peer_leaving_cleanly_or_not_and_links_again() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let game = rom("multiplayer", "SLOT MULTI", multiplayer(1, 0));
    let (mut host, mut joiner, shim) = linked_pair(&game, &game);
    let (h, j) = watch_both(&mut host, &mut joiner, 2000);
    for (side, w) in [("host", &h), ("joiner", &j)] {
        assert!(w.num("transfers") >= 100, "{side} was not linking: {w:?}");
        assert_eq!(w.num("errors"), 0, "{side}: {w:?}");
    }

    assert_eq!(joiner.ask("end"), "ended");
    let left = joiner.wait_for("its own ending", Duration::from_secs(5), |s| {
        !s.on("linked") && !s.on("live")
    });
    assert_eq!(left.text("toast"), "Some(LinkEnded)");
    let told = host.wait_for("the joiner's ending", Duration::from_secs(5), |s| {
        !s.on("linked") && !s.on("live")
    });
    assert_eq!(told.text("toast"), "Some(PeerEnded)", "{told:?}");
    for d in [&mut host, &mut joiner] {
        d.wait_for("the unplug screen closing", Duration::from_secs(5), |s| {
            s.text("speed") == "Normal"
        });
    }
    watch_both(&mut host, &mut joiner, 1000);
    let (h, j) = watch_both(&mut host, &mut joiner, 2000);
    for (side, w) in [("host", &h), ("joiner", &j)] {
        assert!(
            per_second(w, "frames") >= 55.0,
            "{side} stopped playing alone: {w:?}"
        );
        assert_eq!(
            w.num("transfers"),
            0,
            "{side} still transfers with no one: {w:?}"
        );
    }
    let errors = h.num("errors");

    let port = common::free_port();
    let shim_again = Shim::to(port);
    host.ask(&format!("link host {port}"));
    joiner.ask(&format!("link join {}", shim_again.port));
    for d in [&mut host, &mut joiner] {
        d.wait_for("linking again", Duration::from_secs(20), |s| s.on("live"));
    }
    let (h, _) = watch_both(&mut host, &mut joiner, 2000);
    assert!(
        h.num("transfers") >= 100,
        "the second session did not link: {h:?}"
    );
    assert_eq!(
        h.num("errors"),
        errors,
        "the second session put a word out of turn: {h:?}"
    );

    joiner.kill();
    let gone = host.watch(9000);
    let after = host.wait_for("the lost session ending", Duration::from_secs(10), |s| {
        !s.on("linked") && !s.on("live")
    });
    host.watch(1000);
    let alone = host.watch(2000);
    eprintln!("a joiner killed mid-session: the host's longest frame {} ms, its longest wait between transfers {} ms; then {:.1} fps alone",
        gone.num("frozen"), gone.num("stall"), per_second(&alone, "frames"));
    assert!(
        gone.num("frozen") < 3500,
        "the host stalled longer than armGBA's three-second reply timeout: {gone:?}"
    );
    assert_eq!(after.text("toast"), "None", "{after:?}");
    assert!(
        per_second(&alone, "frames") >= 55.0,
        "the host did not play on alone: {alone:?}"
    );
    assert_eq!(alone.num("transfers"), 0, "{alone:?}");
    drop(shim);

    let mut newcomer = Device::booted("newcomer", "armgba", &game);
    let port = common::free_port();
    host.ask(&format!("link host {port}"));
    newcomer.ask(&format!("link join {port}"));
    for d in [&mut host, &mut newcomer] {
        d.wait_for("a third session", Duration::from_secs(20), |s| s.on("live"));
    }
    let (h, n) = watch_both(&mut host, &mut newcomer, 2000);
    assert!(h.num("transfers") >= 100, "{h:?}");
    assert_eq!(n.num("new_errors"), 0, "{n:?}");
    let last = h.num("new_errors");
    let (h, _) = watch_both(&mut host, &mut newcomer, 1000);
    assert_eq!(
        h.num("new_errors"),
        0,
        "the host's words stayed out of turn ({last} on joining): {h:?}"
    );
}

fn save_of(d: &mut Device) -> String {
    d.ask("sav")
}

#[test]
fn two_different_games_link_like_a_cable_and_neither_hangs_or_touches_a_save() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let ours = rom("multiplayer", "SLOT MULTI", multiplayer(1, 0));
    let theirs = rom("multiplayer-other", "SLOT OTHER", multiplayer(1, 0));
    let (mut host, mut joiner, _shim) = linked_pair(&ours, &theirs);
    let (h, j) = watch_both(&mut host, &mut joiner, 2000);
    for (side, w) in [("host", &h), ("joiner", &j)] {
        assert!(
            w.num("transfers") >= 100,
            "{side}: two versions of a game did not trade: {w:?}"
        );
        assert_eq!(w.num("errors"), 0, "{side}: {w:?}");
    }
    drop((host, joiner));

    let plain = Path::new(env!("CARGO_TARGET_TMPDIR")).join("plain.gba");
    std::fs::write(&plain, common::gba_rom()).expect("write rom");
    let mut host = Device::booted("host", "armgba", &ours);
    let mut joiner = Device::booted("joiner", "armgba", &plain);
    let saves = (save_of(&mut host), save_of(&mut joiner));
    let port = common::free_port();
    host.ask(&format!("link host {port}"));
    joiner.ask(&format!("link join {port}"));
    for d in [&mut host, &mut joiner] {
        d.wait_for("the link going live", Duration::from_secs(20), |s| {
            s.on("live")
        });
    }
    let (h, j) = watch_both(&mut host, &mut joiner, 3000);
    assert!(per_second(&h, "frames") >= 55.0, "{h:?}");
    assert!(per_second(&j, "frames") >= 55.0, "{j:?}");
    // A console in general-purpose mode leaves SD high, so the parent clocks transfers and, as
    // with a real cable, reads nothing back from a console not taking part.
    assert!(h.num("transfers") >= 100, "{h:?}");
    assert_eq!(
        h.num("high"),
        0x7FFF,
        "SIOMULTI1 is not 0xFFFF (a pixel keeps 15 bits), so the other game answered: {h:?}"
    );
    assert_eq!(
        (save_of(&mut host), save_of(&mut joiner)),
        saves,
        "a save changed"
    );
}

#[test]
fn a_live_armgba_session_withholds_fast_forward_rewind_and_state_loads_and_keeps_its_save() {
    let _g = common::core_lock();
    if !armgba() {
        return;
    }
    let game = rom("multiplayer", "SLOT MULTI", multiplayer(1, 0));
    let mut host = Device::booted("host", "armgba", &game);
    let before = host.status();
    assert!(
        before.on("rewind") && before.on("ff") && before.on("load"),
        "{before:?}"
    );
    let save = save_of(&mut host);
    assert!(
        save.starts_with("len=32768 head=[5a, a5"),
        "the game's save: {save}"
    );
    let mut joiner = Device::booted("joiner", "armgba", &game);
    let port = common::free_port();
    host.ask(&format!("link host {port}"));
    joiner.ask(&format!("link join {port}"));
    for d in [&mut host, &mut joiner] {
        d.wait_for("the link going live", Duration::from_secs(20), |s| {
            s.on("live")
        });
    }
    host.watch(1500);
    let during = host.status();
    assert!(
        !during.on("rewind") && !during.on("ff") && !during.on("load"),
        "a session left rewind, fast-forward or state loads on: {during:?}"
    );

    for held in ["r2", "l2"] {
        host.ask(&format!("press {held}"));
        let (h, j) = watch_both(&mut host, &mut joiner, 1500);
        host.ask(&format!("release {held}"));
        assert_eq!(
            host.status().text("speed"),
            "Normal",
            "{held} sped the game up"
        );
        for (side, w) in [("host", &h), ("joiner", &j)] {
            assert_eq!(
                w.num("back"),
                0,
                "{held}: {side}'s game went backwards: {w:?}"
            );
            assert!(
                per_second(w, "frames") <= 62.0,
                "{held}: {side} ran fast: {w:?}"
            );
            assert!(
                w.num("transfers") >= 70,
                "{held}: {side}'s cable stopped: {w:?}"
            );
            assert_eq!(w.num("new_errors"), 0, "{held}: {side}: {w:?}");
        }
    }

    assert!(
        host.ask("state").starts_with("len="),
        "no state during a session"
    );
    assert_ne!(
        host.ask("state"),
        "len=0",
        "the core gave no state during a session"
    );
    let (h, j) = watch_both(&mut host, &mut joiner, 1500);
    for (side, w) in [("host", &h), ("joiner", &j)] {
        assert!(
            w.num("transfers") >= 70,
            "a state save stopped {side}'s cable: {w:?}"
        );
        assert_eq!(
            w.num("new_errors"),
            0,
            "a state save put {side}'s words out of turn: {w:?}"
        );
    }
    assert_eq!(
        save_of(&mut host),
        save,
        "the save read during a session changed"
    );

    host.ask("press select");
    host.ask("press menu");
    host.ask("release menu");
    host.ask("release select");
    let (h, _) = watch_both(&mut host, &mut joiner, 1500);
    assert_eq!(
        host.status().text("speed"),
        "Normal",
        "the link screen paused a session"
    );
    assert!(
        h.num("transfers") >= 70,
        "the link screen stopped the cable: {h:?}"
    );
    host.ask("press b");
    host.ask("release b");
}
