//! Device panel calibration, ported from NextUI's common/displaycal.c. The hardware
//! applies this after composition, to games and UI alike, with no per-frame work.

use std::ffi::{c_int, c_ulong};
use std::fs::OpenOptions;
use std::io;
use std::os::fd::AsRawFd;

const SET_GAMMA_TABLE: c_ulong = 0x10b;
const ENABLE_GAMMA: c_ulong = 0x10c;

extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

// BASEOS_TARGET names the exact hardware; BASEOS_DEVICE is only the frontend family.
fn device_target<'a>(exported: Option<&'a str>, release: &'a str) -> Option<&'a str> {
    exported.or_else(|| {
        release.lines().find_map(|line| {
            line.trim()
                .strip_prefix("BASEOS_TARGET=")
                .map(|value| value.trim().trim_matches(['\"', '\'']))
        })
    })
}

fn device_table(target: Option<&str>) -> Option<[u32; 256]> {
    let cube = match target? {
        "rgsp" => include_str!("../assets/rgsp.cube"),
        "rg34xxsp" => include_str!("../assets/rg34xxsp.cube"),
        _ => return None,
    };
    Some(parse_table(cube))
}

// Embedded NextUI skeleton/SYSTEM/res/displaycal assets, including measured black/white
// points. Distribution needs no extra files or configurable paths.
fn parse_table(cube: &str) -> [u32; 256] {
    let mut rows = cube
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'));
    assert!(rows.next().is_some_and(|line| line.starts_with("TITLE ")));
    assert_eq!(rows.next(), Some("LUT_1D_SIZE 256"));
    assert_eq!(
        rows.next(),
        Some("DOMAIN_MIN 0.000000000 0.000000000 0.000000000")
    );
    assert_eq!(
        rows.next(),
        Some("DOMAIN_MAX 1.000000000 1.000000000 1.000000000")
    );
    // The input is a fixed, tested asset, not a general-purpose .cube loader. Match
    // NextUI's double precision, clamp and nearest-integer conversion to 0x00RRGGBB.
    let table = std::array::from_fn(|_| {
        let mut channels = rows.next().expect("LUT row").split_whitespace();
        let rgb = std::array::from_fn::<_, 3, _>(|_| {
            let value: f64 = channels.next().expect("LUT channel").parse().unwrap();
            assert!(value.is_finite());
            (value * 255.0).clamp(0.0, 255.0).round() as u32
        });
        assert!(channels.next().is_none());
        (rgb[0] << 16) | (rgb[1] << 8) | rgb[2]
    });
    assert!(rows.next().is_none());
    table
}

fn apply_table(
    table: &[u32; 256],
    mut send: impl FnMut(c_ulong, &[c_ulong; 4]) -> io::Result<()>,
) -> io::Result<()> {
    // Allwinner's ABI uses four unsigned longs, a userspace pointer and a BYTE count.
    send(
        SET_GAMMA_TABLE,
        &[
            0,
            table.as_ptr() as c_ulong,
            std::mem::size_of_val(table) as c_ulong,
            0,
        ],
    )?;
    send(ENABLE_GAMMA, &[0; 4])
}

pub(crate) fn apply_device() -> io::Result<()> {
    let exported = std::env::var("BASEOS_TARGET").ok();
    let release = if exported.is_none() {
        std::fs::read_to_string("/etc/baseos-release").unwrap_or_default()
    } else {
        String::new()
    };
    let Some(table) = device_table(device_target(exported.as_deref(), &release)) else {
        return Ok(());
    };
    let disp = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/disp")?;
    apply_table(&table, |request, args| {
        // Both the argument array and table stay alive until the synchronous ioctl returns.
        if unsafe { ioctl(disp.as_raw_fd(), request, args.as_ptr()) } < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseos_exact_target_selects_the_panel() {
        let release = "BASEOS_DEVICE=h700\nBASEOS_TARGET=rg34xxsp\n";
        assert_eq!(device_target(None, release), Some("rg34xxsp"));
        assert_eq!(device_target(Some("rgsp"), release), Some("rgsp"));
        assert_eq!(
            device_target(None, "BASEOS_TARGET=\"rgsp\"\n"),
            Some("rgsp")
        );
        assert_eq!(device_target(None, "BASEOS_DEVICE=h700\n"), None);
        for target in [None, Some(""), Some("h700"), Some("rg35xxsp")] {
            assert!(device_table(target).is_none());
        }
        let rgsp = device_table(Some("rgsp")).unwrap();
        let rg34xxsp = device_table(Some("rg34xxsp")).unwrap();
        assert_ne!(rgsp, rg34xxsp);
        assert_eq!(rg34xxsp[0], 0x090000);
        assert_eq!(rg34xxsp[255], 0xffeaeb);
    }

    #[test]
    fn measured_rgsp_points_are_preserved() {
        let table = device_table(Some("rgsp")).unwrap();
        assert_eq!(table[0], 0x050000);
        assert_eq!(table[1], 0x080403);
        assert_eq!(table[255], 0xffeade);
        for channel in [0, 8, 16] {
            assert!(table
                .windows(2)
                .all(|w| (w[0] >> channel) & 255 <= (w[1] >> channel) & 255));
        }
    }

    #[test]
    fn colour_temperature_still_warms_the_calibrated_output() {
        use crate::grade::{blue_light_gain, BLUE_LIGHT_MAX};

        for model in ["rgsp", "rg34xxsp"] {
            let table = device_table(Some(model)).unwrap();
            // Model scanout after the existing final-blit shader, including its 8-bit
            // quantization. Check the full grey ramp, not only the LUT's white point.
            for input in 0..=255 {
                let output = |step| {
                    let gain = blue_light_gain(step);
                    std::array::from_fn::<_, 3, _>(|channel| {
                        let index = (input as f32 * gain[channel]).round() as usize;
                        (table[index] >> (16 - channel * 8)) & 255
                    })
                };
                let neutral = output(0);
                assert_eq!(
                    neutral,
                    [
                        table[input] >> 16,
                        (table[input] >> 8) & 255,
                        table[input] & 255
                    ]
                );
                for step in 1..=BLUE_LIGHT_MAX {
                    let previous = output(step - 1);
                    let warmer = output(step);
                    assert_eq!(warmer[0], previous[0]);
                    assert!(warmer[1] <= previous[1] && warmer[2] <= previous[2]);
                }
                if input >= 128 {
                    let warmest = output(BLUE_LIGHT_MAX);
                    assert!(warmest[1] < neutral[1] && warmest[2] < neutral[2]);
                    assert!(neutral[2] - warmest[2] > neutral[1] - warmest[1]);
                }
            }
        }
    }

    #[test]
    fn uploads_screen_zero_table_before_enabling() {
        let table = device_table(Some("rgsp")).unwrap();
        let mut calls = Vec::new();
        apply_table(&table, |request, args| {
            calls.push((request, *args));
            Ok(())
        })
        .unwrap();
        assert_eq!(
            calls,
            vec![
                (SET_GAMMA_TABLE, [0, table.as_ptr() as c_ulong, 1024, 0]),
                (ENABLE_GAMMA, [0; 4]),
            ]
        );
    }

    #[test]
    fn failed_upload_does_not_enable_gamma() {
        let mut calls = 0;
        assert!(
            apply_table(&device_table(Some("rgsp")).unwrap(), |request, _| {
                calls += 1;
                assert_eq!(request, SET_GAMMA_TABLE);
                Err(io::Error::other("upload failed"))
            })
            .is_err()
        );
        assert_eq!(calls, 1);
    }
}
