use std::path::{Path, PathBuf};

use slot_retro::{LibretroCore, MockCore, RetroCore};
use slot_store::{Blending, Core, GbColors, Platform};

use crate::root;

pub use slot_store::SUNLIGHT_MAX;

const CORE_ENV: &str = "SLOT_CORE";

pub const SUNLIGHT_OPTION: &str = "mgba_solar_sensor_level";
const SUNLIGHT_NOON: i64 = 8;

pub fn dylib_name(core: Core) -> String {
    #[cfg(feature = "nds")]
    if core == Core::Dsperate {
        return format!("slot_ds.{}", std::env::consts::DLL_EXTENSION);
    }
    format!(
        "{}_libretro.{}",
        core.as_str(),
        std::env::consts::DLL_EXTENSION
    )
}

fn candidates(root: &Path, core: Core) -> Vec<PathBuf> {
    if let Some(named) = std::env::var_os(CORE_ENV) {
        return vec![PathBuf::from(named)];
    }
    let name = dylib_name(core);
    let mut paths = vec![root.join("System").join(&name)];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(&name));
        paths.push(dir.join("vendor").join(&name));
    }
    paths.push(Path::new("vendor").join(&name));
    paths
}

pub struct Opened {
    pub core: Box<dyn RetroCore>,
    pub named: bool,
}

#[allow(clippy::too_many_arguments)]
pub fn open_core(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    blending: Blending,
    palette: Option<GbColors>,
    link: Option<u8>,
) -> Opened {
    let paths = candidates(root, core);
    #[cfg(feature = "nds")]
    if core == Core::Dsperate {
        return open_ds(root, &paths);
    }
    match open_named(
        root, core, platform, serial, colour, blending, palette, link, &paths,
    ) {
        Some(core) => Opened { core, named: true },
        None => {
            report_missing(core, &paths);
            Opened {
                core: Box::new(MockCore::new()),
                named: false,
            }
        }
    }
}

#[cfg(feature = "nds")]
fn open_ds(root: &Path, paths: &[PathBuf]) -> Opened {
    let bios = root::bios_dir(root);
    for path in paths.iter().filter(|p| p.exists()) {
        match slot_retro::DsperateCore::open(path, &bios) {
            Ok(core) => {
                eprintln!("slot: core {}", path.display());
                return Opened {
                    core: Box::new(core),
                    named: true,
                };
            }
            Err(e) => eprintln!("slot: {}: {e}", path.display()),
        }
    }
    report_missing(Core::Dsperate, paths);
    Opened {
        core: Box::new(MockCore::with_size(slot_retro::DS_W, slot_retro::DS_H)),
        named: false,
    }
}

pub fn open_core_for(
    root: &Path,
    core: Core,
    serial: &str,
    colour: bool,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    open_core_for_palette(root, core, Platform::Gba, serial, colour, None, paths)
}

pub fn open_core_for_palette(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    palette: Option<GbColors>,
    paths: &[PathBuf],
) -> Box<dyn RetroCore> {
    open_named(
        root,
        core,
        platform,
        serial,
        colour,
        Blending::Off,
        palette,
        None,
        paths,
    )
    .unwrap_or_else(|| {
        report_missing(core, paths);
        Box::new(MockCore::new())
    })
}

#[allow(clippy::too_many_arguments)]
fn open_named(
    root: &Path,
    core: Core,
    platform: Platform,
    serial: &str,
    colour: bool,
    blending: Blending,
    palette: Option<GbColors>,
    link: Option<u8>,
    paths: &[PathBuf],
) -> Option<Box<dyn RetroCore>> {
    let bios = root::bios_dir(root);
    let saves = root::saves_dir(root);
    for path in paths {
        if !path.exists() {
            continue;
        }
        match LibretroCore::open_preset(path, &bios, &saves, init_options(core)) {
            Ok(mut opened) => {
                apply_core_options(
                    &mut opened,
                    core,
                    platform,
                    serial,
                    root::has_real_bios(root),
                    colour,
                    palette,
                );
                apply_blending(&mut opened, core, blending);
                if let Some(player) = link {
                    apply_link_options(&mut opened, core, player);
                }
                eprintln!("slot: core {}", path.display());
                return Some(Box::new(opened));
            }
            Err(e) => eprintln!("slot: {}: {e}", path.display()),
        }
    }
    None
}

pub fn palette_for(
    chosen: GbColors,
    platform: Platform,
    rom: &Path,
    link: Option<u8>,
) -> Option<GbColors> {
    (link.is_none() && platform.hardware() != Platform::Gba && dmg_only(rom)).then_some(chosen)
}

pub fn dmg_only(rom: &Path) -> bool {
    use std::io::Read;
    let mut header = [0u8; 0x150];
    std::fs::File::open(rom)
        .and_then(|mut f| f.read_exact(&mut header))
        .is_ok_and(|_| header[0x143] & 0x80 == 0)
}

fn report_missing(core: Core, paths: &[PathBuf]) {
    eprintln!(
        "slot: no {} core found, running the mock test pattern instead. Looked in: {}",
        core.as_str(),
        paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

pub fn colour_option(
    which: Core,
    platform: Platform,
    on: bool,
) -> Option<(&'static str, &'static str)> {
    match which {
        #[cfg(feature = "nds")]
        Core::Dsperate => None,
        Core::Mgba => Some((
            "mgba_color_correction",
            if on && platform == Platform::Gb {
                "GBC"
            } else if on {
                "Auto"
            } else {
                "OFF"
            },
        )),
        Core::Gpsp => Some((
            "gpsp_color_correction",
            if on { "enabled" } else { "disabled" },
        )),
        Core::Gambatte => Some((
            "gambatte_gbc_color_correction",
            if !on {
                "disabled"
            } else if platform == Platform::Gb {
                "always"
            } else {
                "GBC only"
            },
        )),
    }
}

pub fn game_boy_player(platform: Platform, rom: &Path) -> bool {
    platform == Platform::Gba
        && !slot_store::header_code(rom).is_some_and(|code| code.starts_with('M'))
}

pub fn apply_game_boy_player(
    core: &mut dyn RetroCore,
    which: Core,
    platform: Platform,
    rom: &Path,
) {
    if which == Core::Mgba {
        let on = game_boy_player(platform, rom);
        core.set_option("mgba_force_gbp", if on { "ON" } else { "OFF" });
    }
}

pub fn sunlight_at(local_secs: i64) -> u8 {
    let from_noon = (local_secs.rem_euclid(86_400) / 60 - 720).abs();
    (SUNLIGHT_NOON * (360 - from_noon) / 360).max(0) as u8
}

pub fn blend_option(which: Core, blending: Blending) -> (&'static str, &'static str) {
    match (which, blending.on(which)) {
        (Core::Mgba, Blending::Off) => ("mgba_interframe_blending", "OFF"),
        (Core::Mgba, Blending::Mix) => ("mgba_interframe_blending", "mix"),
        (Core::Mgba, Blending::Ghosting) => ("mgba_interframe_blending", "lcd_ghosting"),
        (Core::Gpsp, Blending::Off) => ("gpsp_frame_mixing", "disabled"),
        (Core::Gpsp, Blending::Mix | Blending::Ghosting) => ("gpsp_frame_mixing", "enabled"),
        (Core::Gambatte, Blending::Off) => ("gambatte_mix_frames", "disabled"),
        (Core::Gambatte, Blending::Mix) => ("gambatte_mix_frames", "mix"),
        (Core::Gambatte, Blending::Ghosting) => ("gambatte_mix_frames", "lcd_ghosting"),
        #[cfg(feature = "nds")]
        (Core::Dsperate, _) => ("dsperate_blending", "off"),
    }
}

pub fn apply_blending(core: &mut LibretroCore, which: Core, blending: Blending) {
    let (key, value) = blend_option(which, blending);
    core.set_option(key, value);
}

pub fn apply_link_options(core: &mut LibretroCore, which: Core, player: u8) {
    if which != Core::Mgba {
        return;
    }
    core.set_option("mgba_link", "on");
    core.set_option("mgba_link_player", &player.to_string());
    eprintln!("slot: core: link mode on, player {player}");
}

pub fn palette_options(which: Core, colors: GbColors) -> Vec<(&'static str, &'static str)> {
    match (which, colors) {
        (Core::Gambatte, GbColors::Palette(p)) => vec![
            ("gambatte_gb_colorization", "internal"),
            ("gambatte_gb_internal_palette", p.gambatte_name()),
        ],
        (Core::Gambatte, GbColors::Auto) => vec![("gambatte_gb_colorization", "GBC")],
        _ => vec![("mgba_gb_colors", colors.core_name())],
    }
}

fn init_options(core: Core) -> &'static [(&'static str, &'static str)] {
    match core {
        Core::Gambatte => &[
            ("gambatte_gb_bootloader", "enabled"),
            ("gambatte_rumble_level", "10"),
        ],
        _ => &[],
    }
}

pub fn apply_core_options(
    core: &mut LibretroCore,
    which: Core,
    platform: Platform,
    serial: &str,
    bios: bool,
    colour: bool,
    palette: Option<GbColors>,
) {
    if which != Core::Gambatte {
        core.set_option(&format!("{}_frameskip", which.as_str()), "auto");
    }
    if which == Core::Mgba {
        core.set_option("mgba_sgb_borders", "OFF");
        match palette {
            Some(p) => {
                core.set_option("mgba_gb_model", "Game Boy");
                core.set_option("mgba_gb_colors_preset", "0");
                core.set_option("mgba_gb_colors", p.core_name());
            }
            None => {
                core.set_option("mgba_gb_model", "Autodetect");
                core.set_option("mgba_gb_colors_preset", "1");
                core.set_option("mgba_gb_colors", "GBC Dark Green →A");
            }
        }
        if let Some((key, value)) = colour_option(which, platform, colour) {
            core.set_option(key, value);
        }
    }
    if which == Core::Gambatte {
        core.set_option("gambatte_gb_hwmode", "Auto");
        for (key, value) in palette_options(which, palette.unwrap_or(GbColors::Auto)) {
            core.set_option(key, value);
        }
        if let Some((key, value)) = colour_option(which, platform, colour) {
            core.set_option(key, value);
        }
    }
    if which == Core::Gpsp {
        core.set_option("gpsp_serial", serial);
        core.set_option("gpsp_rtc_time_source", "system");
        if bios {
            core.set_option("gpsp_boot_mode", "bios");
        }
        if let Some((key, value)) = colour_option(which, platform, colour) {
            core.set_option(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn palette_options_are_spelled_per_core() {
        let p = slot_store::GbPalette::parse("SGB 1-A").unwrap();
        assert_eq!(
            palette_options(Core::Mgba, GbColors::Palette(p)),
            vec![("mgba_gb_colors", "SGB 1-A")]
        );
        assert_eq!(
            palette_options(Core::Mgba, GbColors::Auto),
            vec![("mgba_gb_colors", "Auto")]
        );
        assert_eq!(
            palette_options(Core::Gambatte, GbColors::Palette(p)),
            vec![
                ("gambatte_gb_colorization", "internal"),
                ("gambatte_gb_internal_palette", "SGB - 1A")
            ]
        );
        assert_eq!(
            palette_options(Core::Gambatte, GbColors::Auto),
            vec![("gambatte_gb_colorization", "GBC")]
        );
    }

    #[test]
    fn gambatte_spells_blending_and_color_correction() {
        for (blending, want) in [
            (Blending::Off, "disabled"),
            (Blending::Mix, "mix"),
            (Blending::Ghosting, "lcd_ghosting"),
        ] {
            assert_eq!(
                blend_option(Core::Gambatte, blending),
                ("gambatte_mix_frames", want)
            );
        }
        assert_eq!(
            colour_option(Core::Gambatte, Platform::Gbc, true),
            Some(("gambatte_gbc_color_correction", "GBC only"))
        );
        assert_eq!(
            colour_option(Core::Gambatte, Platform::Gb, true),
            Some(("gambatte_gbc_color_correction", "always"))
        );
        assert_eq!(
            colour_option(Core::Gambatte, Platform::Gb, false),
            Some(("gambatte_gbc_color_correction", "disabled"))
        );
    }

    #[test]
    fn each_core_spells_frame_blending_its_own_way() {
        for (which, blending, want) in [
            (
                Core::Mgba,
                Blending::Off,
                ("mgba_interframe_blending", "OFF"),
            ),
            (
                Core::Mgba,
                Blending::Mix,
                ("mgba_interframe_blending", "mix"),
            ),
            (
                Core::Mgba,
                Blending::Ghosting,
                ("mgba_interframe_blending", "lcd_ghosting"),
            ),
            (Core::Gpsp, Blending::Off, ("gpsp_frame_mixing", "disabled")),
            (Core::Gpsp, Blending::Mix, ("gpsp_frame_mixing", "enabled")),
            (
                Core::Gpsp,
                Blending::Ghosting,
                ("gpsp_frame_mixing", "enabled"),
            ),
        ] {
            assert_eq!(
                blend_option(which, blending),
                want,
                "{which:?} {blending:?}"
            );
        }
    }

    #[test]
    fn each_core_spells_colour_correction_its_own_way() {
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gba, true),
            Some(("mgba_color_correction", "Auto"))
        );
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gba, false),
            Some(("mgba_color_correction", "OFF"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, Platform::Gba, true),
            Some(("gpsp_color_correction", "enabled"))
        );
        assert_eq!(
            colour_option(Core::Gpsp, Platform::Gba, false),
            Some(("gpsp_color_correction", "disabled"))
        );
    }

    #[test]
    fn mgba_uses_gbc_correction_for_game_boy_games() {
        assert_eq!(
            colour_option(Core::Mgba, Platform::Gb, true),
            Some(("mgba_color_correction", "GBC"))
        );
    }

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn candidates_search_the_named_cores_own_filename_only() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);

        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        assert_ne!(mgba, gpsp, "the two cores searched the same paths");
        assert!(!mgba.is_empty());
        assert!(!gpsp.is_empty());
        for path in &mgba {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("mgba_libretro"), "{name} is not mGBA's");
        }
        for path in &gpsp {
            let name = path.file_name().unwrap().to_str().unwrap();
            assert!(name.starts_with("gpsp_libretro"), "{name} is not gpSP's");
        }
    }

    #[test]
    fn candidates_search_the_roots_own_system_directory() {
        let _g = lock();
        std::env::remove_var(CORE_ENV);
        let root = Path::new("/some/content/root");
        assert_eq!(
            candidates(root, Core::Gpsp)[0],
            root.join("System").join(dylib_name(Core::Gpsp)),
        );
    }

    #[test]
    fn the_env_override_ignores_which_core_was_asked_for() {
        let _g = lock();
        std::env::set_var(CORE_ENV, "/dev/null/named-core");
        let root = Path::new("/root");
        let mgba = candidates(root, Core::Mgba);
        let gpsp = candidates(root, Core::Gpsp);
        std::env::remove_var(CORE_ENV);
        assert_eq!(mgba, vec![PathBuf::from("/dev/null/named-core")]);
        assert_eq!(
            mgba, gpsp,
            "the override stopped winning for one of the cores"
        );
    }
}
