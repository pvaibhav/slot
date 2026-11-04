//! A dump's CRC32, which names one exact dump in `Artwork.pack`, so finding its label needs no
//! agreement about spelling or region.
use std::io::Write;
use std::path::Path;

struct Crc(crc32fast::Hasher);
impl Write for Crc {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn crc32(rom: &Path) -> std::io::Result<u32> {
    let mut crc = Crc(crc32fast::Hasher::new());
    std::io::copy(&mut std::fs::File::open(rom)?, &mut crc)?;
    Ok(crc.0.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksum_is_the_standard_crc32() {
        let dir = tempfile::tempdir().unwrap();
        let rom = dir.path().join("rom.gb");
        std::fs::write(&rom, b"123456789").unwrap();
        assert_eq!(crc32(&rom).unwrap(), 0xCBF4_3926);
    }
}
