use std::collections::HashMap;
use std::path::Path;

use crate::Platform;

pub const SELECTED_CORE_FILE: &str = "Config/selected_core.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Core {
    #[default]
    Mgba,
    Gpsp,
    Gambatte,
    #[cfg(feature = "nds")]
    Dsperate,
}

impl Core {
    #[cfg(not(feature = "nds"))]
    pub const ALL: [Core; 3] = [Core::Mgba, Core::Gpsp, Core::Gambatte];
    #[cfg(feature = "nds")]
    pub const ALL: [Core; 4] = [Core::Mgba, Core::Gpsp, Core::Gambatte, Core::Dsperate];

    pub fn as_str(&self) -> &'static str {
        match self {
            Core::Mgba => "mgba",
            Core::Gpsp => "gpsp",
            Core::Gambatte => "gambatte",
            #[cfg(feature = "nds")]
            Core::Dsperate => "dsperate",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn text(self) -> &'static str {
        match self {
            Core::Mgba => "mGBA",
            Core::Gpsp => "gpSP",
            Core::Gambatte => "Gambatte",
            #[cfg(feature = "nds")]
            Core::Dsperate => "DSperate",
        }
    }

    pub fn parse(s: &str) -> Option<Core> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mgba" => Some(Core::Mgba),
            "gpsp" => Some(Core::Gpsp),
            "gambatte" => Some(Core::Gambatte),
            #[cfg(feature = "nds")]
            "dsperate" => Some(Core::Dsperate),
            _ => None,
        }
    }

    pub fn runs(self, platform: Platform) -> bool {
        match self {
            #[cfg(feature = "nds")]
            _ if platform == Platform::Nds => self == Core::Dsperate,
            Core::Mgba => true,
            Core::Gpsp => platform.hardware() == Platform::Gba,
            Core::Gambatte => matches!(platform, Platform::Gb | Platform::Gbc),
            #[cfg(feature = "nds")]
            Core::Dsperate => false,
        }
    }

    pub fn pair(platform: Platform) -> [Core; 2] {
        match platform {
            Platform::Gba | Platform::Gbav => [Core::Mgba, Core::Gpsp],
            Platform::Gb | Platform::Gbc => [Core::Mgba, Core::Gambatte],
            #[cfg(feature = "nds")]
            Platform::Nds => [Core::Dsperate, Core::Dsperate],
        }
    }
}

pub fn read_selected_cores(root: &Path) -> HashMap<String, Core> {
    crate::ini::read(root, SELECTED_CORE_FILE)
        .into_iter()
        .filter_map(|(stem, name)| Core::parse(&name).map(|core| (stem, core)))
        .collect()
}

pub fn core_for_platform(root: &Path, stem: &str, platform: Platform, gb_core: Core) -> Core {
    let mut lines = crate::ini::read(root, SELECTED_CORE_FILE);
    lines
        .remove(&core_key(platform, stem))
        .or_else(|| lines.remove(stem))
        .as_deref()
        .and_then(Core::parse)
        .filter(|core| core.runs(platform))
        .unwrap_or_else(|| match platform {
            Platform::Gb | Platform::Gbc if gb_core.runs(platform) => gb_core,
            #[cfg(feature = "nds")]
            Platform::Nds => Core::Dsperate,
            _ => Core::Mgba,
        })
}

pub fn write_selected_core(
    root: &Path,
    platform: Platform,
    stem: &str,
    core: Core,
) -> std::io::Result<()> {
    let path = root.join(SELECTED_CORE_FILE);
    let existing = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let bare = crate::ini::set(&existing, stem, None)?;
    let out = crate::ini::set(&bare, &core_key(platform, stem), Some(core.as_str()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic::atomic_write(&path, out.as_bytes())
}

fn core_key(platform: Platform, stem: &str) -> String {
    format!("{}/{stem}", platform.dir_name())
}
