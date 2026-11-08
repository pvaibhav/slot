#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverlayPart {
    Tile,
    Both,
    Top,
    Bottom,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayEntry {
    pub name: String,
    pub tile: Option<String>,
    pub both: Option<String>,
    pub top: Option<String>,
    pub bottom: Option<String>,
}

impl OverlayEntry {
    pub fn is_empty(&self) -> bool {
        self.tile.is_none() && self.both.is_none() && self.top.is_none() && self.bottom.is_none()
    }

    fn slot(&mut self, part: OverlayPart) -> &mut Option<String> {
        match part {
            OverlayPart::Tile => &mut self.tile,
            OverlayPart::Both => &mut self.both,
            OverlayPart::Top => &mut self.top,
            OverlayPart::Bottom => &mut self.bottom,
        }
    }
}

pub fn overlay_part(file: &str) -> Option<(String, OverlayPart)> {
    if file.starts_with('.') {
        return None;
    }
    let lower = file.to_ascii_lowercase();
    let stem_len = lower.strip_suffix(".png")?.len();
    let (len, part) = [
        ("_tile", OverlayPart::Tile),
        ("_top", OverlayPart::Top),
        ("_bottom", OverlayPart::Bottom),
    ]
    .into_iter()
    .find(|(suffix, _)| lower[..stem_len].ends_with(suffix))
    .map_or((stem_len, OverlayPart::Both), |(suffix, part)| {
        (stem_len - suffix.len(), part)
    });
    (len > 0).then(|| (file[..len].to_string(), part))
}

pub fn overlay_entries(files: &[String]) -> Vec<OverlayEntry> {
    group(files.iter().filter_map(|f| Some((f, overlay_part(f)?))))
}

pub fn overlay_entries_one_screen(files: &[String]) -> Vec<OverlayEntry> {
    group(files.iter().filter_map(|f| {
        let (name, part) = overlay_part(f)?;
        Some(match part {
            OverlayPart::Top | OverlayPart::Bottom => (
                f,
                (f[..f.len() - ".png".len()].to_string(), OverlayPart::Both),
            ),
            _ => (f, (name, part)),
        })
    }))
}

fn group<'a>(
    parts: impl Iterator<Item = (&'a String, (String, OverlayPart))>,
) -> Vec<OverlayEntry> {
    let mut entries: Vec<OverlayEntry> = Vec::new();
    for (file, (name, part)) in parts {
        let at = match entries.iter().position(|e| e.name == name) {
            Some(at) => at,
            None => {
                entries.push(OverlayEntry {
                    name,
                    ..OverlayEntry::default()
                });
                entries.len() - 1
            }
        };
        *entries[at].slot(part) = Some(file.clone());
    }
    entries.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}
