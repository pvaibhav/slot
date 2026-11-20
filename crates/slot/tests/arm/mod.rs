//! Just enough of an ARM assembler to write small Game Boy Advance test programs readably,
//! with labels for branches. Each method is one ARM (not Thumb) instruction.

#![allow(dead_code)]

use std::collections::HashMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    Eq = 0,
    Ne = 1,
    Hs = 2,
    Lo = 3,
    Al = 14,
}

const AND: u32 = 0;
const SUB: u32 = 2;
const ADD: u32 = 4;
const TST: u32 = 8;
const CMP: u32 = 10;
const ORR: u32 = 12;
const MOV: u32 = 13;

/// Where the program starts in the ROM: the header's first word branches here.
pub const ENTRY: usize = 0xC0;

#[derive(Default)]
pub struct Asm {
    words: Vec<u32>,
    labels: HashMap<&'static str, usize>,
    branches: Vec<(usize, Cond, &'static str)>,
}

/// An ARM data-processing immediate: eight bits rotated right by twice the 4-bit rotation. Of the
/// encodings a value may have, the one with the byte on a byte boundary is chosen.
pub fn immediate(value: u32) -> u32 {
    (0..16)
        .step_by(2)
        .chain((1..16).step_by(2))
        .find_map(|rot| {
            let byte = value.rotate_left(rot * 2);
            (byte <= 0xFF).then_some(rot << 8 | byte)
        })
        .unwrap_or_else(|| panic!("{value:#x} is no ARM immediate"))
}

impl Asm {
    pub fn new() -> Asm {
        Asm::default()
    }

    pub fn label(&mut self, name: &'static str) {
        assert!(
            self.labels.insert(name, self.words.len()).is_none(),
            "label {name} placed twice"
        );
    }

    fn put(&mut self, word: u32) {
        self.words.push(word);
    }

    fn data(&mut self, cond: Cond, op: u32, set: bool, rd: u32, rn: u32, value: u32) {
        let s = u32::from(set) << 20;
        self.put(
            (cond as u32) << 28 | 1 << 25 | op << 21 | s | rn << 16 | rd << 12 | immediate(value),
        );
    }

    pub fn mov(&mut self, rd: u32, value: u32) {
        self.data(Cond::Al, MOV, false, rd, 0, value);
    }

    pub fn mov_if(&mut self, cond: Cond, rd: u32, value: u32) {
        self.data(cond, MOV, false, rd, 0, value);
    }

    pub fn mov_reg_if(&mut self, cond: Cond, rd: u32, rm: u32) {
        self.put((cond as u32) << 28 | MOV << 21 | rd << 12 | rm);
    }

    pub fn mov_reg(&mut self, rd: u32, rm: u32) {
        self.mov_reg_if(Cond::Al, rd, rm);
    }

    pub fn add(&mut self, rd: u32, rn: u32, value: u32) {
        self.data(Cond::Al, ADD, false, rd, rn, value);
    }

    pub fn add_if(&mut self, cond: Cond, rd: u32, rn: u32, value: u32) {
        self.data(cond, ADD, false, rd, rn, value);
    }

    pub fn sub(&mut self, rd: u32, rn: u32, value: u32) {
        self.data(Cond::Al, SUB, false, rd, rn, value);
    }

    pub fn and(&mut self, rd: u32, rn: u32, value: u32) {
        self.data(Cond::Al, AND, false, rd, rn, value);
    }

    pub fn orr(&mut self, rd: u32, rn: u32, value: u32) {
        self.data(Cond::Al, ORR, false, rd, rn, value);
    }

    pub fn orr_if(&mut self, cond: Cond, rd: u32, rn: u32, value: u32) {
        self.data(cond, ORR, false, rd, rn, value);
    }

    pub fn cmp(&mut self, rn: u32, value: u32) {
        self.data(Cond::Al, CMP, true, 0, rn, value);
    }

    /// `add rd, rn, rm, lsl #shift`
    pub fn add_lsl(&mut self, rd: u32, rn: u32, rm: u32, shift: u32) {
        self.put((Cond::Al as u32) << 28 | ADD << 21 | rn << 16 | rd << 12 | shift << 7 | rm);
    }

    pub fn cmp_reg(&mut self, rn: u32, rm: u32) {
        self.put((Cond::Al as u32) << 28 | CMP << 21 | 1 << 20 | rn << 16 | rm);
    }

    pub fn tst(&mut self, rn: u32, value: u32) {
        self.data(Cond::Al, TST, true, 0, rn, value);
    }

    /// `ldrh rd, [rn, #offset]`
    pub fn ldrh(&mut self, rd: u32, rn: u32, offset: u32) {
        self.halfword(true, rd, rn, offset);
    }

    /// `strh rd, [rn, #offset]`
    pub fn strh(&mut self, rd: u32, rn: u32, offset: u32) {
        self.halfword(false, rd, rn, offset);
    }

    fn halfword(&mut self, load: bool, rd: u32, rn: u32, offset: u32) {
        assert!(offset < 0x100, "halfword offset {offset:#x} out of reach");
        let l = u32::from(load) << 20;
        self.put(0xE1C0_00B0 | l | rn << 16 | rd << 12 | (offset & 0xF0) << 4 | offset & 0xF);
    }

    /// `str rd, [rn, #offset]`
    pub fn str(&mut self, rd: u32, rn: u32, offset: u32) {
        assert!(offset < 0x1000);
        self.put(0xE580_0000 | rn << 16 | rd << 12 | offset);
    }

    /// `strb rd, [rn, #offset]`
    pub fn strb(&mut self, rd: u32, rn: u32, offset: u32) {
        assert!(offset < 0x1000);
        self.put(0xE5C0_0000 | rn << 16 | rd << 12 | offset);
    }

    pub fn b(&mut self, cond: Cond, label: &'static str) {
        self.branches.push((self.words.len(), cond, label));
        self.put(0);
    }

    pub fn words(mut self) -> Vec<u32> {
        for (at, cond, label) in std::mem::take(&mut self.branches) {
            let target = *self
                .labels
                .get(label)
                .unwrap_or_else(|| panic!("no label {label}"));
            let offset = target as i64 - (at as i64 + 2);
            self.words[at] = (cond as u32) << 28 | 0x0A00_0000 | (offset as u32 & 0x00FF_FFFF);
        }
        self.words
    }
}

/// A 32 KiB cartridge: a header naming the game, then the program at `ENTRY`.
pub fn cartridge(title: &str, code: &str, program: Asm) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0..4].copy_from_slice(&0xEA00_002Eu32.to_le_bytes());
    rom[0xA0..0xA0 + title.len()].copy_from_slice(title.as_bytes());
    rom[0xAC..0xAC + code.len()].copy_from_slice(code.as_bytes());
    rom[0xB0..0xB2].copy_from_slice(b"00");
    rom[0xB2] = 0x96;
    let sum = rom[0xA0..0xBD].iter().fold(0u8, |a, b| a.wrapping_add(*b));
    rom[0xBD] = 0u8.wrapping_sub(sum).wrapping_sub(0x19);
    let words = program.words();
    assert!(
        ENTRY + words.len() * 4 < rom.len(),
        "the program overflows the cartridge"
    );
    for (i, word) in words.iter().enumerate() {
        rom[ENTRY + i * 4..ENTRY + i * 4 + 4].copy_from_slice(&word.to_le_bytes());
    }
    rom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_match_the_hand_assembled_words_the_other_tests_use() {
        let mut a = Asm::new();
        a.mov(0, 0x0400_0000);
        a.mov(1, 0x400);
        a.orr(1, 1, 3);
        a.add(5, 0, 0x100);
        a.mov(6, 0x2000);
        a.strh(6, 5, 0x28);
        a.ldrh(3, 5, 0x28);
        a.ldrh(4, 0, 6);
        a.cmp(4, 160);
        a.and(3, 3, 0x30);
        a.orr(3, 3, 0x1000);
        a.add(3, 3, 1);
        a.str(1, 0, 0);
        assert_eq!(
            a.words(),
            [
                0xe3a00404, 0xe3a01c04, 0xe3811003, 0xe2805c01, 0xe3a06a02, 0xe1c562b8, 0xe1d532b8,
                0xe1d040b6, 0xe35400a0, 0xe2033030, 0xe3833a01, 0xe2833001, 0xe5801000,
            ]
        );
    }

    #[test]
    fn branches_count_from_two_words_ahead() {
        let mut a = Asm::new();
        a.label("top");
        a.ldrh(4, 0, 6);
        a.cmp(4, 160);
        a.b(Cond::Ne, "top");
        a.b(Cond::Al, "end");
        a.label("end");
        assert_eq!(a.words(), [0xe1d040b6, 0xe35400a0, 0x1afffffc, 0xeaffffff]);
    }
}
