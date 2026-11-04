//! Missing cartridge labels from `Labels/Artwork.pack`, prepared on a caller's worker thread. A
//! ROM's checksum finds its label in the pack, so this works with no network at all. Carts the
//! pack does not know are left for the Network menu's Fetch Missing Art.
mod artwork;
mod catalog;
pub mod cheats;
mod pack;
mod source;

/// What the examples need to build the LaunchBox index and the packs the way the device reads
/// them.
#[doc(hidden)]
pub mod build {
    pub use crate::artwork::cart_label_jpeg;
    pub use crate::pack::FILE as PACK;
    pub use crate::source::{normalize, platforms, rank, title, PHOTO, RENDER};

    pub fn write_pack(
        out: &mut impl std::io::Write,
        entries: Vec<(u32, Vec<u8>)>,
    ) -> std::io::Result<()> {
        crate::pack::write(out, crate::pack::ARTWORK, entries)
    }

    pub fn write_cheat_pack(
        out: &mut impl std::io::Write,
        entries: Vec<(u32, Vec<u8>)>,
    ) -> std::io::Result<()> {
        crate::pack::write(out, crate::pack::CHEATS, entries)
    }
}

use std::io::Write;
use std::path::{Path, PathBuf};

use slot_store::{Cart, Platform};

#[derive(Debug)]
pub enum Error {
    Unavailable(String),
    Image(String),
    Storage(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(s) | Self::Image(s) => f.write_str(s),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Storage(e)
    }
}

/// The size a label is prepared at: the label well slot draws on that platform's cart.
pub fn label_size(platform: Platform) -> (u32, u32) {
    match platform {
        Platform::Gba | Platform::Gbav => (264, 136),
        Platform::Gb => (223, 199),
        Platform::Gbc => (223, 199),
    }
}

/// The pack's label for a cart, written beside the cart's other labels. Blocking; call only on a
/// background worker. An existing label is never replaced. `Ok(None)` when the card has no pack,
/// the pack does not know this dump, or the pack is damaged.
pub fn from_pack(root: &Path, cart: &Cart) -> Result<Option<PathBuf>, Error> {
    // Use the actual filename, retaining every tag and punctuation mark.
    let filename = cart
        .rom
        .file_name()
        .ok_or_else(|| Error::Unavailable("missing ROM filename".into()))?;
    let target = root
        .join("Labels")
        .join(cart.platform.dir_name())
        .join(Path::new(filename).with_extension("png"));
    if target.try_exists()? {
        return Ok(Some(target));
    }
    let pack = root.join("Labels").join(pack::FILE);
    if !pack.is_file() {
        return Ok(None);
    }
    let crc = catalog::crc32(&cart.rom)?;
    let label = pack::find(&pack, pack::ARTWORK, crc)
        .map_err(Error::from)
        .and_then(|image| image.map(|i| artwork::fit(&i, cart.platform)).transpose());
    match label {
        Ok(Some(png)) => {
            publish(&target, &png)?;
            Ok(Some(target))
        }
        Ok(None) => Ok(None),
        Err(e) => {
            eprintln!("slot: labels: {}: {e}", pack.display());
            Ok(None)
        }
    }
}

fn publish(target: &Path, bytes: &[u8]) -> Result<(), Error> {
    let dir = target
        .parent()
        .ok_or_else(|| Error::Unavailable("missing label directory".into()))?;
    std::fs::create_dir_all(dir)?;
    // The complete file appears in one operation. A custom label copied in during the
    // download wins, including on FAT/exFAT where hard-link based installation cannot work.
    let mut tmp = tempfile::Builder::new()
        .prefix(".slot-label-")
        .tempfile_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    match tmp.persist_noclobber(target) {
        Ok(_) => {
            if let Ok(dir) = std::fs::File::open(dir) {
                let _ = dir.sync_all();
            }
            Ok(())
        }
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        // Older vendor exFAT drivers reject RENAME_NOREPLACE and hard links. Their
        // ordinary rename is atomic; the single label worker checks again immediately
        // before publishing so an already present custom label is preserved.
        Err(e) if matches!(e.error.raw_os_error(), Some(1 | 22 | 38 | 95)) => {
            publish_legacy(e.file, target)
        }
        Err(e) => Err(Error::Storage(e.error)),
    }
}

fn publish_legacy(tmp: tempfile::NamedTempFile, target: &Path) -> Result<(), Error> {
    if target.try_exists()? {
        return Ok(());
    }
    tmp.persist(target).map_err(|e| Error::Storage(e.error))?;
    if let Some(dir) = target
        .parent()
        .and_then(|dir| std::fs::File::open(dir).ok())
    {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
