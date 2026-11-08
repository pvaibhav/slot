use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum Platform {
    #[default]
    Gba,
    Gb,
    Gbc,
    Gbav,
    #[cfg(feature = "nds")]
    Nds,
}

impl Platform {
    #[cfg(not(feature = "nds"))]
    pub const ALL: [Platform; 4] = [Platform::Gba, Platform::Gbav, Platform::Gbc, Platform::Gb];
    #[cfg(feature = "nds")]
    pub const ALL: [Platform; 5] = [
        Platform::Nds,
        Platform::Gba,
        Platform::Gbav,
        Platform::Gbc,
        Platform::Gb,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Platform::Gba => "Game Boy Advance",
            Platform::Gb => "Game Boy",
            Platform::Gbc => "Game Boy Color",
            Platform::Gbav => "Game Boy Advance Video",
            #[cfg(feature = "nds")]
            Platform::Nds => "Nintendo DS",
        }
    }

    pub fn dir_name(self) -> &'static str {
        match self {
            Platform::Gba => "GBA",
            Platform::Gb => "GB",
            Platform::Gbc => "GBC",
            Platform::Gbav => "GBAV",
            #[cfg(feature = "nds")]
            Platform::Nds => "NDS",
        }
    }

    pub fn hardware(self) -> Platform {
        match self {
            Platform::Gbav => Platform::Gba,
            p => p,
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Platform::Gba | Platform::Gbav => &["gba"],
            Platform::Gb | Platform::Gbc => &["gb", "gbc"],
            #[cfg(feature = "nds")]
            Platform::Nds => &["nds"],
        }
    }

    pub fn picture(self) -> (u32, u32) {
        match self {
            Platform::Gba | Platform::Gbav => (240, 160),
            Platform::Gb | Platform::Gbc => (160, 144),
            #[cfg(feature = "nds")]
            Platform::Nds => (256, 192),
        }
    }

    pub fn accepts(self, path: &Path) -> bool {
        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| {
                self.extensions()
                    .iter()
                    .any(|k| ext.eq_ignore_ascii_case(k))
            })
    }
}
