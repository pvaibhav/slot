//! Builds the optional `Cheats.pack`: the libretro database's cheats for every No-Intro dump they
//! can be matched to, keyed by the dump's CRC32 the way the device looks them up.
//!
//!     cheats-pack <cht dir> <Cheats.pack> GB.dat GBC.dat GBA.dat
//!
//! The database names its files after the games but not always after one dump: "(USA, Europe)"
//! for a game No-Intro lists as "(USA)" and "(Europe)", `_` for `&`, no language list. So a dump
//! takes the files with its title whose regions overlap its own, and with the same revision and
//! release status, preferring an exact name, then its own revision, then the narrowest region.
//! Files that are one game's codes in another format ("(GameShark)", "(Game Genie)") are merged.
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[path = "shared/dat.rs"]
mod dat;

use slot_labels::build;
use slot_labels::cheats::{self, Cheat};
use slot_store::Platform;

const KINDS: [&str; 9] = [
    "code breaker",
    "codebreaker",
    "gameshark",
    "game shark",
    "game genie",
    "action replay",
    "pro action replay",
    "xploder",
    "gamebuster",
];

const REGIONS: [&str; 27] = [
    "world",
    "usa",
    "europe",
    "japan",
    "australia",
    "germany",
    "france",
    "spain",
    "italy",
    "netherlands",
    "sweden",
    "korea",
    "china",
    "canada",
    "brazil",
    "asia",
    "taiwan",
    "hong kong",
    "scandinavia",
    "uk",
    "denmark",
    "finland",
    "norway",
    "russia",
    "unknown",
    "latin america",
    "greece",
];

/// Tags that make a different release, not a different name for the same one.
const STATUS: [&str; 10] = [
    "proto",
    "beta",
    "demo",
    "sample",
    "kiosk",
    "unl",
    "pirate",
    "aftermarket",
    "hack",
    "possibly fake",
];

#[derive(Debug)]
struct Name {
    title: String,
    regions: Option<Vec<String>>,
    rev: Option<String>,
    status: Vec<String>,
    /// Everything but the code format, for an exact match.
    plain: String,
}

fn parse_name(name: &str) -> Name {
    let lower = name.to_lowercase().replace('_', "&");
    let (title, rest) = match lower.find(" (").into_iter().chain(lower.find(" [")).min() {
        Some(at) => (&lower[..at], &lower[at..]),
        None => (lower.as_str(), ""),
    };
    let mut groups: Vec<String> = Vec::new();
    let mut depth = None;
    for (i, ch) in rest.char_indices() {
        match (ch, depth) {
            ('(' | '[', None) => depth = Some(i + 1),
            (')' | ']', Some(start)) => {
                groups.push(rest[start..i].trim().to_owned());
                depth = None;
            }
            _ => {}
        }
    }
    let mut out = Name {
        title: title.split_whitespace().collect::<Vec<_>>().join(" "),
        regions: None,
        rev: None,
        status: Vec::new(),
        plain: String::new(),
    };
    let mut kept = vec![out.title.clone()];
    for g in groups {
        let items: Vec<String> = g.split(',').map(|s| s.trim().to_owned()).collect();
        if KINDS.iter().any(|k| g.starts_with(k)) {
            continue;
        }
        kept.push(g.clone());
        if out.regions.is_none() && items.iter().all(|i| REGIONS.contains(&i.as_str())) {
            out.regions = Some(items);
        } else if g.starts_with("rev ")
            || (g.starts_with('v') && g[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            out.rev = Some(g);
        } else if let Some(s) = STATUS.iter().find(|s| g.starts_with(**s)) {
            out.status.push((*s).to_owned());
        } else if g.starts_with("t+") || g.starts_with("t-") || g.starts_with("h ") {
            out.status.push("hack".into());
        }
    }
    out.status.sort();
    out.status.dedup();
    out.plain = kept.join(" ");
    out
}

/// How well a file fits a dump, or `None` when it is for another release. Higher is better.
fn fit(dump: &Name, file: &Name) -> Option<(u8, bool, i32)> {
    if dump.title != file.title || dump.status != file.status {
        return None;
    }
    if dump.plain == file.plain {
        return Some((3, true, 0));
    }
    if file.rev.is_some() && file.rev != dump.rev {
        return None;
    }
    let tier = match (&dump.regions, &file.regions) {
        (Some(d), Some(f)) => {
            let world = |r: &[String]| r.iter().any(|r| r == "world");
            if !(world(d) || world(f) || d.iter().any(|r| f.contains(r))) {
                return None;
            }
            2
        }
        _ => 1,
    };
    let narrow = match &file.regions {
        Some(r) if !r.iter().any(|r| r == "world") => -(r.len() as i32),
        Some(_) => -99,
        None => -100,
    };
    Some((tier, file.rev == dump.rev, narrow))
}

struct File {
    name: Name,
    cheats: Vec<Cheat>,
}

fn files(dir: &Path) -> Vec<File> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("cht")))
        .collect();
    paths.sort();
    paths
        .into_iter()
        .filter_map(|p| {
            let stem = p.file_stem()?.to_str()?.to_owned();
            let bytes = std::fs::read(&p).ok()?;
            let cheats = cheats::parse(&String::from_utf8_lossy(&bytes));
            (!cheats.is_empty()).then(|| File {
                name: parse_name(&stem),
                cheats,
            })
        })
        .collect()
}

fn folder(platform: Platform) -> &'static str {
    match platform {
        Platform::Gb => "Nintendo - Game Boy",
        Platform::Gbc => "Nintendo - Game Boy Color",
        Platform::Gba | Platform::Gbav => "Nintendo - Game Boy Advance",
        #[cfg(feature = "nds")]
        Platform::Nds => "Nintendo - Nintendo DS",
    }
}

/// The folders a dump's cheats may be filed in: the database files a game that runs on both
/// Game Boys under either.
fn shelves(platform: Platform) -> &'static [Platform] {
    match platform {
        Platform::Gb | Platform::Gbc => &[Platform::Gb, Platform::Gbc],
        Platform::Gba | Platform::Gbav => &[Platform::Gba],
        #[cfg(feature = "nds")]
        Platform::Nds => &[Platform::Nds],
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [cht, out, gb, gbc, gba] = args.as_slice() else {
        eprintln!("usage: cheats-pack <cht dir> <Cheats.pack> GB.dat GBC.dat GBA.dat");
        std::process::exit(2);
    };
    let mut library: HashMap<Platform, Vec<File>> = HashMap::new();
    for platform in [Platform::Gb, Platform::Gbc, Platform::Gba] {
        library.insert(platform, files(&cht.join(folder(platform))));
    }
    let mut used: HashMap<(Platform, usize), ()> = HashMap::new();
    let mut entries = Vec::new();
    for (platform, dat) in [
        (Platform::Gb, gb),
        (Platform::Gbc, gbc),
        (Platform::Gba, gba),
    ] {
        let dumps = dat::dumps(dat);
        let mut matched = 0;
        for dump in &dumps {
            let name = parse_name(&dump.name);
            let mut best: Option<(u8, bool, i32)> = None;
            let mut picked: Vec<(Platform, usize)> = Vec::new();
            for &shelf in shelves(platform) {
                for (i, file) in library[&shelf].iter().enumerate() {
                    let Some(score) = fit(&name, &file.name) else {
                        continue;
                    };
                    if best.is_none_or(|b| score > b) {
                        best = Some(score);
                        picked.clear();
                    }
                    if best == Some(score) {
                        picked.push((shelf, i));
                    }
                }
            }
            let mut merged: Vec<Cheat> = Vec::new();
            for &(shelf, i) in &picked {
                used.insert((shelf, i), ());
                for c in &library[&shelf][i].cheats {
                    let seen = merged
                        .iter()
                        .any(|m| m.code == c.code || m.name.eq_ignore_ascii_case(&c.name));
                    if !seen {
                        merged.push(c.clone());
                    }
                }
            }
            if !merged.is_empty() {
                matched += 1;
                entries.push((dump.crc, cheats::write(&merged).into_bytes()));
            }
        }
        let files = &library[&platform];
        let unused = (0..files.len())
            .filter(|i| !used.contains_key(&(platform, *i)))
            .count();
        eprintln!(
            "{}: {matched} of {} dumps have cheats; {} of {} cheat files matched no dump",
            folder(platform),
            dumps.len(),
            unused,
            files.len()
        );
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(out).expect("create pack"));
    let count = entries.len();
    build::write_cheat_pack(&mut file, entries).expect("write pack");
    std::io::Write::flush(&mut file).expect("write pack");
    eprintln!("{}: {count} dumps", out.display());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fits(dump: &str, file: &str) -> Option<(u8, bool, i32)> {
        fit(&parse_name(dump), &parse_name(file))
    }

    #[test]
    fn a_shared_region_and_the_database_spelling_still_match() {
        assert!(fits(
            "Kirby & The Amazing Mirror (USA)",
            "Kirby _ the Amazing Mirror (USA, Europe) (Code Breaker)"
        )
        .is_some());
        assert!(fits(
            "Metal Slug Advance (USA)",
            "Metal Slug Advance (World) (Code Breaker)"
        )
        .is_some());
        assert!(fits(
            "Lara Croft Tomb Raider - Legend (USA) (En,Fr,De,Es,It)",
            "Lara Croft Tomb Raider - Legend (USA)"
        )
        .is_some());
    }

    #[test]
    fn another_region_revision_or_release_does_not() {
        assert_eq!(
            fits("Castlevania (Europe)", "Castlevania (USA) (Code Breaker)"),
            None
        );
        assert_eq!(fits("Tetris (World)", "Tetris (World) (Rev 1)"), None);
        assert_eq!(
            fits(
                "Pokemon - Gold Version (USA, Europe)",
                "Pokemon - Gold Version (USA, Europe) (Beta)"
            ),
            None
        );
    }

    #[test]
    fn the_exact_name_beats_a_neighbour() {
        let exact = fits(
            "Tetris (World) (Rev 1)",
            "Tetris (World) (Rev 1) (GameShark)",
        )
        .unwrap();
        let loose = fits("Tetris (World) (Rev 1)", "Tetris (World)").unwrap();
        assert!(exact > loose);
        let narrow = fits("Wario Land II (USA, Europe)", "Wario Land II (USA)").unwrap();
        let wide = fits("Wario Land II (USA, Europe)", "Wario Land II (World)").unwrap();
        assert!(narrow > wide);
    }
}
