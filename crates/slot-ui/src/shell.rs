use std::path::Path;

use slot_store::gb::Class;
use slot_store::{Cart, Platform, ShellFinish};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Finish {
    Solid,
    Translucent,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Shell {
    pub colour: [u8; 3],
    pub finish: Finish,
}

pub const DEFAULT_SHELL: Shell = Shell {
    colour: [0x35, 0x35, 0x3a],
    finish: Finish::Solid,
};

const fn shell(colour: [u8; 3], finish: Finish) -> Shell {
    Shell { colour, finish }
}

const EXACT: &[(&str, Shell)] = &[
    ("AXV", shell([0xc2, 0x33, 0x2e], Finish::Translucent)),
    ("AXP", shell([0x2f, 0x5c, 0xc0], Finish::Translucent)),
    ("BPE", shell([0x39, 0x9c, 0x3c], Finish::Translucent)),
    ("BPR", shell([0xd8, 0x52, 0x24], Finish::Solid)),
    ("BPG", shell([0x63, 0xb0, 0x44], Finish::Solid)),
    ("U3I", CLEAR),
    ("U32", CLEAR),
    ("U33", CLEAR),
    ("V49", shell([0xa9, 0x51, 0x3b], Finish::Solid)),
    ("KYGE", YOSHI),
    ("KYGP", YOSHI),
    ("RZW", shell([0x5f, 0x62, 0x64], Finish::Translucent)),
    ("RZWJ", shell([0xec, 0xee, 0xe8], Finish::Solid)),
];

const CLEAR: Shell = shell([0xd9, 0xdb, 0xd8], Finish::Translucent);
const YOSHI: Shell = shell([0x2f, 0x8f, 0x4e], Finish::Solid);

const FAMILY: &[(u8, Shell)] = &[(b'M', shell([0xc6, 0xc6, 0xc9], Finish::Solid))];

pub const DMG_SHELL: Shell = shell([0x9a, 0x97, 0x8f], Finish::Solid);

pub const DUAL_MODE_SHELL: Shell = shell([0x33, 0x30, 0x31], Finish::Solid);

pub const GB_CLEAR_SHELL: Shell = shell([0x7c, 0x7a, 0x8a], Finish::Translucent);

const GB_CODES: &[(&str, Shell)] = &[
    ("AAU", shell([0xb3, 0x8b, 0x3a], Finish::Solid)),
    ("AAUJ", DUAL_MODE_SHELL),
    ("AAX", shell([0xa9, 0xaa, 0xa7], Finish::Solid)),
    ("AAXJ", DUAL_MODE_SHELL),
    ("BYT", CRYSTAL),
    ("BXT", CRYSTAL),
    ("KTN", KIRBY),
    ("KKK", KIRBY),
    ("KCE", shell([0x1f, 0x9f, 0xb6], Finish::Translucent)),
    ("VCA", shell([0xf0, 0xa9, 0x5e], Finish::Translucent)),
    ("VPHJ", shell([0xe2, 0xb4, 0x13], Finish::Solid)),
    ("BMG", DUAL_MODE_SHELL),
];

const GB_OVERSEAS_TITLES: &[(&str, Shell)] = &[
    ("POKEMON RED", shell([0xc0, 0x28, 0x2c], Finish::Solid)),
    ("POKEMON BLU", shell([0x2b, 0x3a, 0x88], Finish::Solid)),
    ("POKEMON YEL", shell([0xe9, 0xa8, 0x26], Finish::Solid)),
];

const CRYSTAL: Shell = shell([0x86, 0xb9, 0xbf], Finish::Glitter);
const KIRBY: Shell = shell([0xec, 0x94, 0xb4], Finish::Translucent);

pub fn shell_for(cart: &Cart) -> Shell {
    if let Some(shell) = chosen_shell(cart) {
        return shell;
    }
    match cart.platform {
        Platform::Gba | Platform::Gbav => gba_shell_for(&gba_code(cart)),
        Platform::Gb | Platform::Gbc => gb_shell_for(&cart.rom),
    }
}

pub fn indexed_shell(cart: &Cart) -> Option<Shell> {
    if let Some(shell) = chosen_shell(cart) {
        return Some(shell);
    }
    match cart.platform {
        Platform::Gba | Platform::Gbav => match cart.code.is_empty() {
            false => Some(gba_shell_for(&cart.code)),
            true => slot_store::header_index::peek_gba_code(&cart.rom).map(|c| gba_shell_for(&c)),
        },
        Platform::Gb | Platform::Gbc => slot_store::header_index::peek_gb_header(&cart.rom)
            .map(|h| gb_shell_from(h.as_ref(), &cart.rom)),
    }
}

fn gba_code(cart: &Cart) -> String {
    match cart.code.is_empty() {
        false => cart.code.clone(),
        true => slot_store::header_index::gba_code(&cart.rom),
    }
}

fn chosen_shell(cart: &Cart) -> Option<Shell> {
    let choice = cart.shell?;
    let finish = match choice.finish {
        ShellFinish::Solid => Finish::Solid,
        ShellFinish::Clear => Finish::Translucent,
        ShellFinish::Glitter => Finish::Glitter,
    };
    Some(shell(choice.colour, finish))
}

pub fn gba_shell_for(code: &str) -> Shell {
    lookup(code, EXACT, FAMILY)
}

fn gb_shell_for(rom: &Path) -> Shell {
    gb_shell_from(slot_store::gb::cached_header(rom).as_ref(), rom)
}

fn gb_shell_from(h: Option<&slot_store::gb::Header>, rom: &Path) -> Shell {
    let Some(h) = h else {
        return flag_shell(slot_store::gb::class(rom));
    };
    let by_title = || {
        GB_OVERSEAS_TITLES
            .iter()
            .find(|(t, _)| !h.japan && *t == h.title)
            .map(|(_, s)| *s)
    };
    by_code(&h.code, GB_CODES)
        .or_else(by_title)
        .unwrap_or_else(|| flag_shell(h.class()))
}

fn flag_shell(class: Class) -> Shell {
    match class {
        Class::Original => DMG_SHELL,
        Class::DualMode => DUAL_MODE_SHELL,
        Class::ColourOnly => GB_CLEAR_SHELL,
    }
}

pub fn table_keys() -> Vec<&'static str> {
    EXACT.iter().map(|(k, _)| *k).collect()
}

pub fn gba_plastics() -> Vec<[u8; 3]> {
    let mut colours = vec![DEFAULT_SHELL.colour, FAMILY[0].1.colour];
    colours.extend(EXACT.iter().map(|(_, s)| s.colour));
    unique(colours)
}

pub fn gb_plastics() -> Vec<[u8; 3]> {
    let mut colours = vec![
        DMG_SHELL.colour,
        DUAL_MODE_SHELL.colour,
        GB_CLEAR_SHELL.colour,
    ];
    colours.extend(
        GB_CODES
            .iter()
            .chain(GB_OVERSEAS_TITLES)
            .map(|(_, s)| s.colour),
    );
    unique(colours)
}

fn unique(colours: Vec<[u8; 3]>) -> Vec<[u8; 3]> {
    let mut out = Vec::new();
    for colour in colours {
        if !out.contains(&colour) {
            out.push(colour);
        }
    }
    out
}

pub fn gb_table_shells() -> Vec<Shell> {
    GB_CODES
        .iter()
        .chain(GB_OVERSEAS_TITLES)
        .map(|(_, s)| *s)
        .collect()
}

pub fn lookup_order_is_exact_then_family_then_default() -> bool {
    const A: Shell = shell([1, 1, 1], Finish::Solid);
    const B: Shell = shell([2, 2, 2], Finish::Solid);
    let exact = [("MSK", A)];
    let family = [(b'M', B)];
    lookup("MSKE", &exact, &family) == A
        && lookup("MPOE", &exact, &family) == B
        && lookup("ZZZZ", &exact, &family) == DEFAULT_SHELL
}

fn lookup(code: &str, exact: &[(&str, Shell)], family: &[(u8, Shell)]) -> Shell {
    if let Some(s) = by_code(code, exact) {
        return s;
    }
    if let Some(first) = code.as_bytes().first() {
        if let Some((_, s)) = family.iter().find(|(k, _)| k == first) {
            return *s;
        }
    }
    DEFAULT_SHELL
}

fn by_code(code: &str, table: &[(&str, Shell)]) -> Option<Shell> {
    let whole: String = code.chars().take(4).collect();
    let prefix: String = code.chars().take(3).collect();
    [whole, prefix]
        .iter()
        .find_map(|key| table.iter().find(|(k, _)| k == key).map(|(_, s)| *s))
}

pub fn shell_presets() -> Vec<(&'static str, Shell)> {
    const GBA: &[(&str, &str)] = &[
        ("Pokémon Ruby", "AXV"),
        ("Pokémon Sapphire", "AXP"),
        ("Pokémon Emerald", "BPE"),
        ("Pokémon FireRed", "BPR"),
        ("Pokémon LeafGreen", "BPG"),
        ("Boktai clear", "U3I"),
        ("Drill Dozer red", "V49"),
        ("Yoshi green", "KYGE"),
        ("WarioWare smoke", "RZW"),
        ("Made in Wario white", "RZWJ"),
    ];
    const GB: &[(&str, &str)] = &[
        ("Pokémon Gold", "AAU"),
        ("Pokémon Silver", "AAX"),
        ("Pokémon Crystal", "BYT"),
        ("Kirby pink", "KTN"),
        ("Command Master turquoise", "KCE"),
        ("Chee-Chai Alien orange", "VCA"),
        ("Pinball yellow", "VPHJ"),
    ];
    let mut out = vec![
        ("Game Boy grey", DMG_SHELL),
        ("Game Boy black", DUAL_MODE_SHELL),
        ("Game Boy Color clear", GB_CLEAR_SHELL),
        ("Advance charcoal", DEFAULT_SHELL),
        ("Advance Video grey", FAMILY[0].1),
    ];
    out.extend(
        GBA.iter()
            .filter_map(|(n, k)| Some((*n, by_code(k, EXACT)?))),
    );
    out.extend(
        GB.iter()
            .filter_map(|(n, k)| Some((*n, by_code(k, GB_CODES)?))),
    );
    out.extend(
        [
            ("Pokémon Red", "POKEMON RED"),
            ("Pokémon Blue", "POKEMON BLU"),
            ("Pokémon Yellow", "POKEMON YEL"),
        ]
        .iter()
        .filter_map(|(n, t)| {
            let s = GB_OVERSEAS_TITLES.iter().find(|(k, _)| k == t)?.1;
            Some((*n, s))
        }),
    );
    out
}
