//! Cheats on the real armGBA core, through slot's own cheat files and emulator thread.

mod arm;
mod common;

use std::time::{Duration, Instant};

use arm::{Asm, Cond};
use slot::audio::{AudioSink, StubSink};
use slot::emu::{CoreState, EmuHandle, Speed};
use slot_store::{Core, Platform};

const IO: u32 = 0;
const T: u32 = 1;
const VRAM: u32 = 2;
const FRAMES: u32 = 3;
const A: u32 = 4;
const RAM: u32 = 6;

/// Each frame, at the start of V-blank: shows the halfwords at 0x02000000, 0x02000010 and
/// 0x02000020 in pixels 1 to 3 of the top row, then writes 0x1111, 0x0022 and 0x3333 back. A
/// cheat, applied as the frame ends, shows instead.
fn program() -> Asm {
    let mut a = Asm::new();
    a.mov(IO, 0x0400_0000);
    a.mov(T, 0x400);
    a.orr(T, T, 3);
    a.str(T, IO, 0);
    a.mov(VRAM, 0x0600_0000);
    a.mov(RAM, 0x0200_0000);
    a.mov(FRAMES, 0);
    a.label("out");
    a.ldrh(A, IO, 6);
    a.cmp(A, 160);
    a.b(Cond::Eq, "out");
    a.label("in");
    a.ldrh(A, IO, 6);
    a.cmp(A, 160);
    a.b(Cond::Ne, "in");
    a.add(FRAMES, FRAMES, 1);
    a.strh(FRAMES, VRAM, 0);
    for (x, (at, own)) in [(0x00, 0x1111), (0x10, 0x0022), (0x20, 0x3333)]
        .into_iter()
        .enumerate()
    {
        a.ldrh(A, RAM, at);
        a.strh(A, VRAM, 2 + 2 * x as u32);
        a.mov(A, own & 0xFF00);
        a.orr(A, A, own & 0xFF);
        a.strh(A, RAM, at);
    }
    a.b(Cond::Al, "out");
    a
}

fn painted(picture: &[u8], x: usize) -> u16 {
    let pixel = &picture[x * 4..x * 4 + 4];
    u16::from(pixel[2] >> 3) | u16::from(pixel[1] >> 3) << 5 | u16::from(pixel[0] >> 3) << 10
}

/// The three halfwords once the screen has moved on `frames` frames past the moment asked.
fn shown(emu: &EmuHandle, frames: u16) -> [u16; 3] {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut first = None;
    loop {
        assert!(Instant::now() < deadline, "the game stopped drawing");
        if let Some(frame) = emu.latest_frame() {
            let counter = painted(&frame, 0);
            let first = *first.get_or_insert(counter);
            if counter.wrapping_sub(first) & 0x7FFF >= frames {
                return [painted(&frame, 1), painted(&frame, 2), painted(&frame, 3)];
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

const CHT: &str = "cheats = 3\n\
    cheat0_desc = \"CodeBreaker 16-bit\"\ncheat0_code = \"82000000+2BCD\"\ncheat0_enable = false\n\
    cheat1_desc = \"CodeBreaker 8-bit\"\ncheat1_code = \"32000010+0056\"\ncheat1_enable = false\n\
    cheat2_desc = \"VBA\"\ncheat2_code = \"02000020:4AFE\"\ncheat2_enable = false\n";

#[test]
fn a_raw_ram_cheat_writes_armgbas_memory_each_frame_and_stops_when_turned_off() {
    let _g = common::core_lock();
    let dylib = common::repo_root()
        .join("vendor")
        .join(slot::core::dylib_name(Core::Armgba));
    if !dylib.exists() {
        eprintln!("no armGBA core on this host, skipping");
        return;
    }
    let d = common::tmp_root_with_real_carts(&["Cheat"]);
    let rom = d.path().join("Games/GBA/Cheat.gba");
    std::fs::write(&rom, arm::cartridge("SLOT CHEAT", "SLTE", program())).expect("write rom");
    std::fs::create_dir_all(d.path().join("Cheats/GBA")).expect("cheats folder");
    std::fs::write(d.path().join("Cheats/GBA/Cheat.cht"), CHT).expect("write cht");
    let mut cheats = slot::cheats::load(d.path(), Platform::Gba, "Cheat", &rom);
    assert_eq!(
        cheats.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["CodeBreaker 16-bit", "CodeBreaker 8-bit", "VBA"]
    );

    let core = slot::core::open_core_for(d.path(), Core::Armgba, "auto", false, &[dylib]);
    let mut sink = StubSink::draining();
    sink.open(32_768).expect("open the stub sink");
    let emu = EmuHandle::spawn(core, rom.clone(), sink.ring(), None, None);
    emu.set_speed(Speed::Normal);
    let deadline = Instant::now() + Duration::from_secs(10);
    while emu.state() != CoreState::Ready {
        assert!(Instant::now() < deadline, "armGBA never loaded the game");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        shown(&emu, 3),
        [0x1111, 0x0022, 0x3333],
        "the game's own values"
    );

    for c in &mut cheats {
        c.on = true;
    }
    emu.set_cheats(slot::cheats::codes(&cheats));
    assert_eq!(
        shown(&emu, 3),
        [0x2BCD, 0x0056, 0x4AFE],
        "the cheats' values"
    );
    assert_eq!(
        shown(&emu, 30),
        [0x2BCD, 0x0056, 0x4AFE],
        "a cheat held for one frame only"
    );

    cheats[1].on = false;
    emu.set_cheats(slot::cheats::codes(&cheats));
    assert_eq!(
        shown(&emu, 3),
        [0x2BCD, 0x0022, 0x4AFE],
        "turning one cheat off left it writing, or took the others with it"
    );

    emu.set_cheats(Vec::new());
    assert_eq!(
        shown(&emu, 3),
        [0x1111, 0x0022, 0x3333],
        "a cheat kept writing once off"
    );
    assert_eq!(shown(&emu, 30), [0x1111, 0x0022, 0x3333]);
}
