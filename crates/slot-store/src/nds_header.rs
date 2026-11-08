use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const ICON: usize = 32;
const ICON_AT: u64 = 0x20;
const PALETTE_AT: u64 = 0x220;
const ENGLISH_AT: u64 = 0x340;
const TITLE_BYTES: usize = 0x100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NdsBanner {
    pub title: String,
    pub icon_rgba: Vec<u8>,
}

pub fn read_banner(rom: &Path) -> Option<NdsBanner> {
    let mut file = std::fs::File::open(rom).ok()?;
    let mut header = [0u8; 0x6c];
    file.read_exact(&mut header).ok()?;
    let fallback = || NdsBanner {
        title: header_title(&header[..12]),
        icon_rgba: vec![0u8; ICON * ICON * 4],
    };
    let at = u32::from_le_bytes([header[0x68], header[0x69], header[0x6a], header[0x6b]]) as u64;
    if at == 0 {
        return Some(fallback());
    }
    let mut read = |offset: u64, len: usize| -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        file.seek(SeekFrom::Start(at + offset)).ok()?;
        file.read_exact(&mut buf).ok()?;
        Some(buf)
    };
    let (Some(tiles), Some(palette), Some(title)) = (
        read(ICON_AT, 0x200),
        read(PALETTE_AT, 0x20),
        read(ENGLISH_AT, TITLE_BYTES),
    ) else {
        return Some(fallback());
    };
    let title = first_line(&title);
    Some(NdsBanner {
        title: if title.is_empty() {
            fallback().title
        } else {
            title
        },
        icon_rgba: icon(&tiles, &palette),
    })
}

fn header_title(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .trim()
        .to_string()
}

fn first_line(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let text = String::from_utf16_lossy(&units);
    text.split(['\n', '\0'])
        .next()
        .unwrap_or("")
        .trim()
        .to_string()
}

fn icon(tiles: &[u8], palette: &[u8]) -> Vec<u8> {
    let color = |i: usize| -> [u8; 4] {
        if i == 0 {
            return [0, 0, 0, 0];
        }
        let c = u16::from_le_bytes([palette[i * 2], palette[i * 2 + 1]]);
        let wide = |v: u16| ((v & 0x1f) as u32 * 255 / 31) as u8;
        [wide(c), wide(c >> 5), wide(c >> 10), 255]
    };
    let mut rgba = vec![0u8; ICON * ICON * 4];
    for (n, byte) in tiles.iter().enumerate() {
        let tile = n / 32;
        let within = n % 32;
        let (tx, ty) = (tile % 4, tile / 4);
        let y = ty * 8 + within / 4;
        let x = tx * 8 + (within % 4) * 2;
        for (dx, index) in [(0, byte & 0x0f), (1, byte >> 4)] {
            let o = (y * ICON + x + dx) * 4;
            rgba[o..o + 4].copy_from_slice(&color(index as usize));
        }
    }
    rgba
}
