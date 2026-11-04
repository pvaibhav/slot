const NAMES: [&str; 48] = [
    "Grayscale",
    "DMG Green",
    "GB Pocket",
    "GB Light",
    "GBC Brown ↑",
    "GBC Red ↑A",
    "GBC Dark Brown ↑B",
    "GBC Pale Yellow ↓",
    "GBC Orange ↓A",
    "GBC Yellow ↓B",
    "GBC Blue ←",
    "GBC Dark Blue ←A",
    "GBC Gray ←B",
    "GBC Green →",
    "GBC Dark Green →A",
    "GBC Reverse →B",
    "SGB 1-A",
    "SGB 1-B",
    "SGB 1-C",
    "SGB 1-D",
    "SGB 1-E",
    "SGB 1-F",
    "SGB 1-G",
    "SGB 1-H",
    "SGB 2-A",
    "SGB 2-B",
    "SGB 2-C",
    "SGB 2-D",
    "SGB 2-E",
    "SGB 2-F",
    "SGB 2-G",
    "SGB 2-H",
    "SGB 3-A",
    "SGB 3-B",
    "SGB 3-C",
    "SGB 3-D",
    "SGB 3-E",
    "SGB 3-F",
    "SGB 3-G",
    "SGB 3-H",
    "SGB 4-A",
    "SGB 4-B",
    "SGB 4-C",
    "SGB 4-D",
    "SGB 4-E",
    "SGB 4-F",
    "SGB 4-G",
    "SGB 4-H",
];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct GbPalette(u8);

impl GbPalette {
    pub const COUNT: usize = NAMES.len();
    pub const DEFAULT: GbPalette = GbPalette(1);
    pub const GRAYSCALE: GbPalette = GbPalette(0);

    pub fn all() -> impl Iterator<Item = GbPalette> {
        (0..Self::COUNT as u8).map(GbPalette)
    }

    pub fn index(self) -> usize {
        self.0 as usize
    }

    pub fn core_name(self) -> &'static str {
        NAMES[self.index()]
    }

    pub fn gambatte_name(self) -> &'static str {
        const GBC: [&str; 12] = [
            "GBC - Brown",
            "GBC - Red",
            "GBC - Dark Brown",
            "GBC - Pastel Mix",
            "GBC - Orange",
            "GBC - Yellow",
            "GBC - Blue",
            "GBC - Dark Blue",
            "GBC - Grayscale",
            "GBC - Green",
            "GBC - Dark Green",
            "GBC - Inverted",
        ];
        const SGB: [&str; 32] = [
            "SGB - 1A", "SGB - 1B", "SGB - 1C", "SGB - 1D", "SGB - 1E", "SGB - 1F", "SGB - 1G",
            "SGB - 1H", "SGB - 2A", "SGB - 2B", "SGB - 2C", "SGB - 2D", "SGB - 2E", "SGB - 2F",
            "SGB - 2G", "SGB - 2H", "SGB - 3A", "SGB - 3B", "SGB - 3C", "SGB - 3D", "SGB - 3E",
            "SGB - 3F", "SGB - 3G", "SGB - 3H", "SGB - 4A", "SGB - 4B", "SGB - 4C", "SGB - 4D",
            "SGB - 4E", "SGB - 4F", "SGB - 4G", "SGB - 4H",
        ];
        match self.index() {
            0 => "GBC - Grayscale",
            1 => "GB - DMG",
            2 => "GB - Pocket",
            3 => "GB - Light",
            i @ 4..=15 => GBC[i - 4],
            i => SGB[i - 16],
        }
    }

    pub fn label(self) -> &'static str {
        let name = self.core_name();
        match name.find(['↑', '↓', '←', '→']) {
            Some(at) => name[..at].trim_end(),
            None => name,
        }
    }

    pub fn next(self) -> GbPalette {
        GbPalette(((self.index() + 1) % Self::COUNT) as u8)
    }

    pub fn prev(self) -> GbPalette {
        GbPalette(((self.index() + Self::COUNT - 1) % Self::COUNT) as u8)
    }

    pub fn parse(name: &str) -> Option<GbPalette> {
        Self::all().find(|p| p.core_name() == name)
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum GbColors {
    Auto,
    Palette(GbPalette),
}

impl GbColors {
    pub fn all() -> impl Iterator<Item = GbColors> {
        std::iter::once(GbColors::Auto).chain(GbPalette::all().map(GbColors::Palette))
    }

    pub fn index(self) -> usize {
        match self {
            GbColors::Auto => 0,
            GbColors::Palette(p) => p.index() + 1,
        }
    }

    pub fn next(self) -> GbColors {
        match self {
            GbColors::Auto => GbColors::Palette(GbPalette(0)),
            GbColors::Palette(p) if p.index() + 1 == GbPalette::COUNT => GbColors::Auto,
            GbColors::Palette(p) => GbColors::Palette(p.next()),
        }
    }

    pub fn prev(self) -> GbColors {
        match self {
            GbColors::Auto => GbColors::Palette(GbPalette((GbPalette::COUNT - 1) as u8)),
            GbColors::Palette(p) if p.index() == 0 => GbColors::Auto,
            GbColors::Palette(p) => GbColors::Palette(p.prev()),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GbColors::Auto => "Auto",
            GbColors::Palette(p) => p.label(),
        }
    }

    pub fn core_name(self) -> &'static str {
        match self {
            GbColors::Auto => "Auto",
            GbColors::Palette(p) => p.core_name(),
        }
    }
}
