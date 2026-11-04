use std::borrow::Cow;
use std::io::Write;
use std::path::{Path, PathBuf};

use slot_store::{Cart, Platform};
use slot_ui::house_label::{stops, Label};
use slot_ui::hue::{fallback_hue, logo_luma};

use super::index::Look;

pub const STUDIO_CACHE: &str = "Cache/art/studio";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Real,
    Logo,
}

fn encoded(width: u32, height: u32, color: png::ColorType, pixels: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width, height);
        enc.set_color(color);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().ok()?.write_image_data(pixels).ok()?;
    }
    Some(out)
}

fn webp_as_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(bytes)).ok()?;
    let (width, height) = decoder.dimensions();
    let color = match decoder.has_alpha() {
        true => png::ColorType::Rgba,
        false => png::ColorType::Rgb,
    };
    let mut pixels = vec![0; decoder.output_buffer_size()?];
    decoder.read_image(&mut pixels).ok()?;
    encoded(width, height, color, &pixels)
}

pub fn as_png(bytes: &[u8]) -> Option<Cow<'_, [u8]>> {
    if bytes.starts_with(b"RIFF") {
        return webp_as_png(bytes).map(Cow::Owned);
    }
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return Some(Cow::Borrowed(bytes));
    }
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let color = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => png::ColorType::Rgb,
        jpeg_decoder::PixelFormat::L8 => png::ColorType::Grayscale,
        _ => return None,
    };
    encoded(u32::from(info.width), u32::from(info.height), color, &pixels).map(Cow::Owned)
}

pub fn real_label(scan: &[u8], platform: Platform) -> Option<Vec<u8>> {
    Label::full(&as_png(scan)?, platform).map(|l| l.png())
}

pub fn logo_label(
    wheel: &[u8],
    look: Option<&Look>,
    stem: &str,
    platform: Platform,
) -> Option<Vec<u8>> {
    let wheel = as_png(wheel)?;
    let wheel = &wheel[..];
    let deep = automatic_ground(wheel, look, stem);
    Label::from_png(wheel, deep, platform, look.and_then(|l| l.band)).map(|l| l.png())
}

pub fn automatic_ground(wheel: &[u8], look: Option<&Look>, stem: &str) -> [u8; 3] {
    match look {
        Some(look) => look.ground,
        None => stops(fallback_hue(stem), logo_luma(wheel)).0,
    }
}

pub fn text_label(cart: &Cart, colour: [u8; 3]) -> Vec<u8> {
    let (w, h) = slot_ui::label_size(cart);
    slot_ui::house_label::text_png(&slot_ui::text_label(cart, colour), w, h, colour)
}

pub fn label_path(root: &Path, cart: &Cart) -> PathBuf {
    root.join("Labels")
        .join(cart.platform.dir_name())
        .join(format!("{}.png", cart.stem))
}

fn part_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    path.with_file_name(name)
}

fn write_part(path: &Path, png: &[u8]) -> std::io::Result<PathBuf> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_of(path);
    let mut file = std::fs::File::create(&part)?;
    file.write_all(png)?;
    file.sync_all()?;
    Ok(part)
}

pub fn write_new(path: &Path, png: &[u8]) -> std::io::Result<bool> {
    if path.exists() {
        return Ok(false);
    }
    let part = write_part(path, png)?;
    if path.exists() {
        let _ = std::fs::remove_file(&part);
        return Ok(false);
    }
    std::fs::rename(&part, path)?;
    Ok(true)
}

pub fn keep_existing(root: &Path, cart: &Cart) -> std::io::Result<()> {
    let label = label_path(root, cart);
    let kept = root.join(STUDIO_CACHE).join("existing.png");
    if !label.is_file() || kept.is_file() {
        return Ok(());
    }
    write_replacing(&kept, &std::fs::read(label)?)
}

pub fn kept_label(root: &Path, cart: &Cart) -> std::io::Result<Option<PathBuf>> {
    let kept = root.join(STUDIO_CACHE).join("existing.png");
    if kept.is_file() {
        return Ok(Some(kept));
    }
    match cart.label {
        Some(_) => Err(std::io::ErrorKind::NotFound.into()),
        None => Ok(None),
    }
}

pub fn apply_label(root: &Path, cart: &Cart, src: Option<&Path>) -> std::io::Result<()> {
    let label = label_path(root, cart);
    let done = match src {
        Some(src) => write_replacing(&label, &std::fs::read(src)?),
        None if label.exists() => std::fs::remove_file(label),
        None => Ok(()),
    };
    forget_cached(root, cart);
    done
}

pub fn relabel(root: &Path, cart: &Cart, png: &[u8]) -> std::io::Result<()> {
    let done = write_replacing(&label_path(root, cart), png);
    forget_cached(root, cart);
    done
}

fn forget_cached(root: &Path, cart: &Cart) {
    let _ = std::fs::remove_file(crate::label_cache::cache_path(root, cart));
}

pub fn write_replacing(path: &Path, png: &[u8]) -> std::io::Result<()> {
    let part = write_part(path, png)?;
    std::fs::rename(&part, path)
}
