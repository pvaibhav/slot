use slot_retro::{ButtonMask, LibretroCore, RetroCore};
use std::path::Path;

#[test]
fn real_cores_expose_iwram_before_ewram_in_ra_address_order() {
    // An original tiny program writes different markers to the two banks, then spins.
    // This catches both reversed banks and gpSP's nonzero IWRAM descriptor offset.
    let code: [u32; 7] = [
        0xe3a00403, 0xe3a01012, 0xe5c01000, 0xe3a00402, 0xe3a01034, 0xe5c01000, 0xeafffffe,
    ];
    let mut rom = vec![0u8; 0x8000];
    rom[..4].copy_from_slice(&0xea00002eu32.to_le_bytes());
    rom[0xa0..0xac].copy_from_slice(b"SLOT RA TEST");
    rom[0xac..0xb0].copy_from_slice(b"SLTE");
    rom[0xb0..0xb2].copy_from_slice(b"00");
    rom[0xb2] = 0x96;
    let sum = rom[0xa0..0xbd].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xbd] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    for (i, instruction) in code.iter().enumerate() {
        rom[0xc0 + i * 4..0xc4 + i * 4].copy_from_slice(&instruction.to_le_bytes());
    }
    let rom_path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("ra-memory.gba");
    std::fs::write(&rom_path, rom).unwrap();
    for name in ["mgba", "gpsp"] {
        let dylib = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../vendor/{name}_libretro.{}",
            std::env::consts::DLL_EXTENSION
        ));
        if !dylib.exists() {
            eprintln!("skipping {name}: host core unavailable");
            continue;
        }
        let mut core = LibretroCore::open(&dylib).unwrap();
        core.load(&rom_path).unwrap();
        for _ in 0..30 {
            core.run_frame(ButtonMask::default());
            core.take_audio();
        }
        let mut ram = vec![0; 0x58000];
        let valid = core.achievement_memory(&mut ram);
        assert_eq!(&valid[..2], &[0x8000, 0x40000], "{name}");
        assert_eq!(ram[0], 0x12, "{name}: IWRAM/descriptor offset");
        assert_eq!(ram[0x8000], 0x34, "{name}: EWRAM/address order");
        assert_eq!(core.achievement_memory(&mut [0; 10]), [0; 3]);
    }
}
