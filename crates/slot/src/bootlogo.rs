use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CARD_LOGO: &str = "System/bootlogo.bmp";
pub const USER_LOGO: &str = "Config/bootlogo.bmp";
pub const INSTALLED_COPY: &str = "Cache/bootlogo.installed";
const LOGO: &str = "bootlogo.bmp";
const ORIGINAL: &str = "bootlogo.baseos.bmp";
const PARTITION: &str = "boot-resource";
const MOUNT: &str = "/tmp/slot-boot-resource";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outcome {
    Same,
    Installed,
    Invalid,
}

pub struct Bmp {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<[u8; 3]>,
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

pub fn size(bmp: &[u8]) -> Option<(usize, usize)> {
    if bmp.get(0..2)? != b"BM" || u32_at(bmp, 14)? < 40 {
        return None;
    }
    let w = u32_at(bmp, 18)? as i32;
    let h = u32_at(bmp, 22)? as i32;
    let (w, h) = (w.unsigned_abs() as usize, h.unsigned_abs() as usize);
    ((1..=4096).contains(&w) && (1..=4096).contains(&h)).then_some((w, h))
}

pub fn decode(bmp: &[u8]) -> Option<Bmp> {
    let (width, height) = size(bmp)?;
    if u16_at(bmp, 28)? != 24 || u32_at(bmp, 30)? != 0 {
        return None;
    }
    let top_down = (u32_at(bmp, 22)? as i32) < 0;
    let offset = u32_at(bmp, 10)? as usize;
    let stride = (width * 3 + 3) & !3;
    let data = bmp.get(offset..offset + stride * height)?;
    let mut rgb = Vec::with_capacity(width * height);
    for y in 0..height {
        let row = if top_down { y } else { height - 1 - y };
        for px in data[row * stride..].chunks_exact(3).take(width) {
            rgb.push([px[2], px[1], px[0]]);
        }
    }
    Some(Bmp { width, height, rgb })
}

pub fn encode(img: &Bmp) -> Vec<u8> {
    let stride = (img.width * 3 + 3) & !3;
    let data = stride * img.height;
    let mut out = Vec::with_capacity(54 + data);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&((54 + data) as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(img.width as i32).to_le_bytes());
    out.extend_from_slice(&(img.height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(data as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..img.height).rev() {
        for c in &img.rgb[y * img.width..(y + 1) * img.width] {
            out.extend_from_slice(&[c[2], c[1], c[0]]);
        }
        out.resize(out.len() + stride - img.width * 3, 0);
    }
    out
}

fn turned(img: &Bmp) -> Bmp {
    let (w, h) = (img.height, img.width);
    let mut rgb = vec![[0; 3]; w * h];
    for y in 0..img.height {
        for x in 0..img.width {
            rgb[(img.width - 1 - x) * w + y] = img.rgb[y * img.width + x];
        }
    }
    Bmp {
        width: w,
        height: h,
        rgb,
    }
}

pub fn compose(wordmark: &Bmp, (width, height): (usize, usize)) -> Option<Bmp> {
    let turned_mark;
    let mark = if height > width {
        turned_mark = turned(wordmark);
        &turned_mark
    } else {
        wordmark
    };
    if mark.width > width || mark.height > height {
        return None;
    }
    let mut rgb = vec![*mark.rgb.first()?; width * height];
    let (x0, y0) = ((width - mark.width) / 2, (height - mark.height) / 2);
    for y in 0..mark.height {
        let at = (y0 + y) * width + x0;
        rgb[at..at + mark.width].copy_from_slice(&mark.rgb[y * mark.width..(y + 1) * mark.width]);
    }
    Some(Bmp { width, height, rgb })
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Logo {
    Exact(Vec<u8>),
    Wordmark(Vec<u8>),
}

struct Plan {
    logo: Vec<u8>,
    current: Option<Vec<u8>>,
    original: Option<Vec<u8>>,
}

fn built(logo: &Logo, current: Option<&[u8]>, original: Option<&[u8]>) -> Option<Vec<u8>> {
    match logo {
        Logo::Exact(bytes) => Some(bytes.clone()),
        Logo::Wordmark(bytes) => {
            let mark = decode(bytes)?;
            let panel = original.or(current).and_then(size)?;
            compose(&mark, panel).map(|b| encode(&b))
        }
    }
}

fn plan(logo: &Logo, dir: &Path) -> Result<Plan, Outcome> {
    let current = fs::read(dir.join(LOGO)).ok();
    let original = fs::read(dir.join(ORIGINAL)).ok();
    let logo = built(logo, current.as_deref(), original.as_deref()).ok_or(Outcome::Invalid)?;
    if current.as_deref() == Some(&logo[..]) {
        return Err(Outcome::Same);
    }
    Ok(Plan {
        logo,
        current,
        original,
    })
}

pub fn pending(logo: &Logo, dir: &Path) -> Option<Outcome> {
    plan(logo, dir).err()
}

pub fn install(logo: &Logo, dir: &Path) -> io::Result<Outcome> {
    let Plan {
        logo,
        current,
        original,
    } = match plan(logo, dir) {
        Ok(plan) => plan,
        Err(outcome) => return Ok(outcome),
    };
    if let Some(current) = &current {
        if original.is_none() {
            replace(dir, ORIGINAL, current)?;
        }
        if current.len() == logo.len() {
            overwrite(dir, LOGO, &logo)?;
            return Ok(Outcome::Installed);
        }
    }
    replace(dir, LOGO, &logo)?;
    Ok(Outcome::Installed)
}

fn overwrite(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let mut f = fs::OpenOptions::new().write(true).open(dir.join(name))?;
    f.write_all(bytes)?;
    f.sync_all()
}

fn replace(dir: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    let tmp = dir.join("bootlogo.tmp");
    let mut f = File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, dir.join(name))?;
    File::open(dir)?.sync_all()
}

pub fn chosen(card: &Path) -> Option<Logo> {
    if let Ok(logo) = fs::read(card.join(USER_LOGO)) {
        return Some(Logo::Exact(logo));
    }
    let shipped = fs::read(card.join(CARD_LOGO)).ok()?;
    if decode(&shipped).is_none() {
        eprintln!("slot: bootlogo: {CARD_LOGO} is not a 24-bit bmp, skipping it");
        return None;
    }
    Some(Logo::Wordmark(shipped))
}

pub fn remember(card: &Path) {
    let user = card.join(USER_LOGO);
    let source = if user.is_file() {
        user
    } else {
        card.join(CARD_LOGO)
    };
    let Ok(logo) = fs::read(source) else {
        return;
    };
    let copy = card.join(INSTALLED_COPY);
    let (Some(dir), Some(name)) = (copy.parent(), copy.file_name().and_then(|n| n.to_str())) else {
        return;
    };
    if let Err(e) = fs::create_dir_all(dir).and_then(|_| replace(dir, name, &logo)) {
        eprintln!("slot: bootlogo: could not keep the installed copy: {e}");
    }
}

pub fn refresh(card: &Path) {
    let Some(logo) = chosen(card) else {
        return;
    };
    let Some(device) = partition(PARTITION) else {
        eprintln!("slot: bootlogo: no {PARTITION} partition");
        return;
    };
    if fs::create_dir_all(MOUNT).is_err()
        || !run("mount", &["-t", "vfat", "-o", "ro,noatime", &device, MOUNT])
    {
        eprintln!("slot: bootlogo: could not mount {device}");
        return;
    }
    match pending(&logo, Path::new(MOUNT)) {
        Some(Outcome::Invalid) => eprintln!("slot: bootlogo: no logo fits this panel, left alone"),
        Some(_) => {}
        None if !run("mount", &["-o", "remount,rw,noatime", MOUNT]) => {
            eprintln!("slot: bootlogo: could not remount {device} to write the logo")
        }
        None => match install(&logo, Path::new(MOUNT)) {
            Ok(Outcome::Installed) => eprintln!("slot: bootlogo: installed"),
            Ok(_) => {}
            Err(e) => eprintln!("slot: bootlogo: {e}"),
        },
    }
    if !run("umount", &[MOUNT]) {
        eprintln!("slot: bootlogo: could not unmount {MOUNT}");
    }
}

fn partition(name: &str) -> Option<String> {
    for entry in fs::read_dir("/sys/class/block").ok()?.flatten() {
        let uevent = fs::read_to_string(entry.path().join("uevent")).unwrap_or_default();
        if uevent.lines().any(|l| l == format!("PARTNAME={name}")) {
            return Some(
                PathBuf::from("/dev")
                    .join(entry.file_name())
                    .display()
                    .to_string(),
            );
        }
    }
    None
}

fn run(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .status()
        .is_ok_and(|s| s.success())
}
