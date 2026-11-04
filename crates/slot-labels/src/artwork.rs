use crate::Error;
use image::codecs::jpeg::JpegEncoder;
use image::{imageops::FilterType, DynamicImage, ImageFormat, ImageReader, Limits, RgbImage};
use slot_store::Platform;
use std::io::Cursor;

fn bad(e: image::ImageError) -> Error {
    Error::Image(e.to_string())
}

fn decode(bytes: &[u8]) -> Result<DynamicImage, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Image(e.to_string()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(bad)
}

fn png(label: RgbImage) -> Result<Vec<u8>, Error> {
    let mut out = Cursor::new(Vec::new());
    label.write_to(&mut out, ImageFormat::Png).map_err(bad)?;
    Ok(out.into_inner())
}

/// A label from an artwork pack, which holds it at its own platform's size: a Game Boy cart filed
/// on the Game Boy Color shelf is resized to that shelf's label.
pub(crate) fn fit(bytes: &[u8], platform: Platform) -> Result<Vec<u8>, Error> {
    let (lw, lh) = crate::label_size(platform);
    let image = decode(bytes)?;
    if image.width() < 32 || image.height() < 32 {
        return Err(Error::Image("pack label too small".into()));
    }
    if (image.width(), image.height()) == (lw, lh) {
        return png(image.to_rgb8());
    }
    png(image.resize_exact(lw, lh, FilterType::Lanczos3).to_rgb8())
}

/// A straight-on photo of the whole cart, cropped to the label on it.
#[cfg(test)]
pub(crate) fn prepare(bytes: &[u8], platform: Platform) -> Result<Vec<u8>, Error> {
    png(cart_label(bytes, platform)?)
}

/// The same label, as the JPEG an artwork pack holds.
pub fn cart_label_jpeg(bytes: &[u8], platform: Platform) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 88)
        .encode_image(&cart_label(bytes, platform)?)
        .map_err(bad)?;
    Ok(out)
}

fn cart_label(bytes: &[u8], platform: Platform) -> Result<RgbImage, Error> {
    let image = decode(bytes)?;
    let (w, h) = (image.width(), image.height());
    let aspect = w as f64 / h as f64;
    // Only a straight-on scan of the whole cart. Reject box art, angled photos, and tiny
    // thumbnails rather than crop arbitrary imagery.
    let (left, top, right, bottom) = match (platform.hardware(), w, h) {
        (Platform::Gba, 1000, 574) => (137, 130, 865, 497),
        (Platform::Gba, 600, 355) => (82, 82, 520, 305),
        (Platform::Gba, 473, 283) => (69, 68, 402, 246),
        (Platform::Gba, ..) if w >= 390 && (1.65..=1.80).contains(&aspect) => (
            w * 137 / 1000,
            h * 226 / 1000,
            w * 865 / 1000,
            h * 866 / 1000,
        ),
        (Platform::Gb | Platform::Gbc, ..) if w >= 300 && (0.84..=0.93).contains(&aspect) => (
            w * 135 / 1000,
            h * 285 / 1000,
            w * 865 / 1000,
            h * 858 / 1000,
        ),
        _ => {
            return Err(Error::Image(format!(
                "unsupported cartridge layout {w}x{h}"
            )))
        }
    };
    let (lw, lh) = crate::label_size(platform);
    let cw = right - left;
    let ch = bottom - top;
    // Fit without distortion. Anchor to the top, as in the approved standalone labels.
    let (cw, ch) = if cw * lh > ch * lw {
        (ch * lw / lh, ch)
    } else {
        (cw, cw * lh / lw)
    };
    let left = left + (right - left - cw) / 2;
    let label = image
        .crop_imm(left, top, cw, ch)
        .resize_exact(lw, lh, FilterType::Lanczos3)
        .to_rgb8();
    Ok(label)
}
