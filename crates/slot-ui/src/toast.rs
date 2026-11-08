use slot_gfx::Canvas;
use slot_store::GbColors;

use crate::hud::{HUD_INK, PLATE_H};
use crate::icon::{haloed, haloed_with, icon_coverage, Icon, HALO_PX, WARNING_INK};
use crate::text;
use crate::{CartFace, QuickRow};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Toast {
    StateSaved,
    StateLoaded,
    GameReset,
    ShotCaptured,
    ShotFailed,
    NeedsGpsp,
    NeedsMgba,
    NoLink,
    NoWifi,
    LinkEnded,
    PeerEnded,
    BiosMismatch,
    ColourOn,
    ColourOff,
    ShaderOff,
    ShaderLcd3x,
    ShaderGrid,
    ShaderDot,
    ShaderSimpletex,
    ShaderShimmerless,
    CardUnwritable,
    CardRepair,
    CardRepairCharge,
    CardRepaired,
    CardRepairFailed,
    Forgotten,
    Collection,
    KidUnlocked,
    Palette(GbColors),
    Duplicate(QuickRow),
}

impl Toast {
    const FIXED: [Toast; 28] = [
        Toast::StateSaved,
        Toast::StateLoaded,
        Toast::GameReset,
        Toast::ShotCaptured,
        Toast::ShotFailed,
        Toast::NeedsGpsp,
        Toast::NeedsMgba,
        Toast::NoLink,
        Toast::NoWifi,
        Toast::LinkEnded,
        Toast::PeerEnded,
        Toast::BiosMismatch,
        Toast::ColourOn,
        Toast::ColourOff,
        Toast::ShaderOff,
        Toast::ShaderLcd3x,
        Toast::ShaderGrid,
        Toast::ShaderDot,
        Toast::ShaderSimpletex,
        Toast::ShaderShimmerless,
        Toast::CardUnwritable,
        Toast::CardRepair,
        Toast::CardRepairCharge,
        Toast::CardRepaired,
        Toast::CardRepairFailed,
        Toast::Forgotten,
        Toast::Collection,
        Toast::KidUnlocked,
    ];

    pub fn all() -> Vec<Toast> {
        Self::FIXED
            .into_iter()
            .chain(GbColors::all().map(Toast::Palette))
            .chain(bound_rows().map(Toast::Duplicate))
            .collect()
    }

    pub fn index(self) -> usize {
        match self {
            Toast::Palette(p) => Self::FIXED.len() + p.index(),
            Toast::Duplicate(row) => {
                Self::FIXED.len()
                    + GbColors::all().count()
                    + bound_rows().position(|r| r == row).unwrap_or(0)
            }
            fixed => Self::FIXED.iter().position(|t| *t == fixed).unwrap_or(0),
        }
    }

    pub fn icon(self) -> Option<Icon> {
        match self {
            Toast::Duplicate(_) | Toast::ShotFailed => Some(Icon::Alert),
            Toast::ShotCaptured => Some(Icon::Camera),
            _ => None,
        }
    }

    pub fn alarm(self) -> bool {
        matches!(
            self,
            Toast::CardUnwritable | Toast::CardRepair | Toast::CardRepairCharge
        )
    }

    pub fn hold_ms(self) -> crate::hud::Millis {
        match self {
            Toast::CardRepaired | Toast::CardRepairFailed => REPAIR_HOLD_MS,
            Toast::Duplicate(_) => WARNING_HOLD_MS,
            _ => crate::hud::HUD_MS,
        }
    }

    pub fn detail(self) -> Option<&'static str> {
        match self {
            Toast::CardUnwritable => Some("Progress is not being saved"),
            Toast::CardRepair => Some("Press SELECT + START to repair"),
            Toast::CardRepairCharge => Some("Charge the battery to repair"),
            _ => None,
        }
    }

    pub fn text(self) -> String {
        match self {
            Toast::Duplicate(row) if row.game_button().is_some() => {
                format!("Same as Console {}", row.label())
            }
            Toast::Duplicate(row) => format!("Same as {}", row.label()),
            other => other.fixed_text().to_string(),
        }
    }

    fn fixed_text(self) -> &'static str {
        match self {
            Toast::StateSaved => "State Saved",
            Toast::StateLoaded => "State Loaded",
            Toast::GameReset => "Game Reset",
            Toast::ShotCaptured => "Screenshot Captured",
            Toast::ShotFailed => "Screenshot Failed",
            Toast::NeedsGpsp => "Please switch to gpSP",
            Toast::NeedsMgba => "Please switch to mGBA",
            Toast::NoLink => "No link support",
            Toast::NoWifi => "This device has no Wi-Fi",
            Toast::LinkEnded => "Link ended",
            Toast::PeerEnded => "Other player ended the link",
            Toast::BiosMismatch => "BIOS does not match",
            Toast::ColourOn => "Correction On",
            Toast::ColourOff => "Correction Off",
            Toast::ShaderOff => "Shader: Off",
            Toast::ShaderLcd3x => "Shader: LCD",
            Toast::ShaderGrid => "Shader: Grid",
            Toast::ShaderDot => "Shader: Dot",
            Toast::ShaderSimpletex => "Shader: Simpletex",
            Toast::ShaderShimmerless => "Shader: Shimmerless",
            Toast::CardUnwritable => "Can't write to SD card",
            Toast::CardRepair | Toast::CardRepairCharge => "Can't write to SD card",
            Toast::CardRepaired => "SD card repaired",
            Toast::CardRepairFailed => "Card repair failed",
            Toast::Forgotten => "Network forgotten",
            Toast::Collection => "Collection",
            Toast::KidUnlocked => "Kid Mode is off until restart",
            Toast::Palette(p) => p.label(),
            Toast::Duplicate(_) => "",
        }
    }
}

fn bound_rows() -> impl Iterator<Item = QuickRow> {
    QuickRow::ALL.into_iter().filter(|r| r.binds())
}

const REPAIR_HOLD_MS: crate::hud::Millis = 6_000;
const WARNING_HOLD_MS: crate::hud::Millis = 4_000;

const TOAST_W: u32 = 400;
const TOAST_H: u32 = 28;
const TOAST_PX: f32 = 20.0;
const TOAST_MIN_PX: f32 = 12.0;
const ICON_GAP: u32 = 8;
const TOAST_ICON_PX: f32 = 22.0;

const ALARM_W: u32 = 560;
const ALARM_H: u32 = 44;
const ALARM_PX: f32 = 32.0;
const DETAIL_H: u32 = 30;
const DETAIL_PX: f32 = 20.0;

struct Setting {
    w: u32,
    h: u32,
    px: f32,
    min_px: f32,
    lines: usize,
}

fn setting(toast: Toast) -> Setting {
    match toast.alarm() {
        true => Setting {
            w: ALARM_W,
            h: ALARM_H,
            px: ALARM_PX,
            min_px: ALARM_PX,
            lines: 1,
        },
        false => Setting {
            w: TOAST_W,
            h: TOAST_H,
            px: TOAST_PX,
            min_px: TOAST_MIN_PX,
            lines: 2,
        },
    }
}

fn layout_for(font: &fontdue::Font, toast: Toast, said: &str) -> text::Layout {
    let s = setting(toast);
    let room = toast
        .icon()
        .and_then(|i| icon_coverage(i, TOAST_ICON_PX))
        .map_or(s.w, |(_, iw, _)| s.w.saturating_sub(iw + ICON_GAP));
    match s.lines {
        1 => text::fit(font, said, room as f32, 1, s.px, s.min_px),
        _ => text::fit_even(font, said, room as f32, s.px, s.min_px),
    }
}

pub fn toast_lines(toast: Toast, said: &str) -> usize {
    text::label_font().map_or(1, |font| layout_for(font, toast, said).lines.len().max(1))
}

pub fn toast_band(lines: usize) -> f32 {
    PLATE_H + TOAST_H as f32 * lines.saturating_sub(1) as f32
}

pub fn toast_rect(canvas: Canvas, toast: Toast) -> (f32, f32, f32, f32) {
    toast_rect_lines(canvas, toast, toast_lines(toast, &toast.text()))
}

pub fn toast_rect_lines(canvas: Canvas, toast: Toast, lines: usize) -> (f32, f32, f32, f32) {
    let (w, h) = toast_box_lines(toast, lines);
    let (w, h) = (w as f32, h as f32);
    let middle = match toast.alarm() {
        true => canvas.h / 2.0,
        false => toast_band(lines) / 2.0,
    };
    (canvas.centred(w), middle - h / 2.0, w, h)
}

pub fn toast_box(toast: Toast) -> (u32, u32) {
    toast_box_lines(toast, toast_lines(toast, &toast.text()))
}

fn toast_box_lines(toast: Toast, lines: usize) -> (u32, u32) {
    let s = setting(toast);
    (s.w + 2 * HALO_PX, s.h * lines as u32 + detail_h(toast) + 2 * HALO_PX)
}

fn detail_h(toast: Toast) -> u32 {
    match toast.detail() {
        Some(_) => DETAIL_H,
        None => 0,
    }
}

pub fn toast_face(toast: Toast) -> CartFace {
    toast_face_text(toast, &toast.text())
}

pub fn toast_face_text(toast: Toast, said: &str) -> CartFace {
    let Some(font) = text::label_font() else {
        return CartFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let s = setting(toast);
    let icon = toast.icon().and_then(|i| icon_coverage(i, TOAST_ICON_PX));
    let layout = layout_for(font, toast, said);
    let tall = s.h * layout.lines.len().max(1) as u32;
    let (mut cov, tinted) = match icon {
        Some(icon) => beside(icon, font, &layout, s.w, tall),
        None => (text::coverage(s.w, tall, &layout), 0..0),
    };
    if let Some(detail) = toast.detail() {
        let layout = text::fit(font, detail, s.w as f32, 1, DETAIL_PX, DETAIL_PX);
        cov.extend(text::coverage(s.w, DETAIL_H, &layout));
    }
    if tinted.is_empty() || toast.icon() != Some(Icon::Alert) {
        return haloed(&cov, s.w, tall + detail_h(toast), HUD_INK);
    }
    haloed_with(&cov, s.w, tall + detail_h(toast), |x, _| {
        match tinted.contains(&(x as u32)) {
            true => WARNING_INK,
            false => HUD_INK,
        }
    })
}

fn beside(
    (icon, iw, ih): (Vec<u8>, u32, u32),
    font: &fontdue::Font,
    layout: &text::Layout,
    w: u32,
    h: u32,
) -> (Vec<u8>, std::ops::Range<u32>) {
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let tw = (set.ceil() as u32 + 2).min(w.saturating_sub(iw + ICON_GAP));
    let words = text::coverage(tw, h, layout);
    let left = w.saturating_sub(iw + ICON_GAP + tw) / 2;
    let mut cov = vec![0u8; (w * h) as usize];
    let top = h.saturating_sub(ih) / 2;
    for y in 0..ih.min(h) {
        for x in 0..iw {
            cov[((top + y) * w + left + x) as usize] = icon[(y * iw + x) as usize];
        }
    }
    let at = left + iw + ICON_GAP;
    for y in 0..h {
        for x in 0..tw {
            cov[(y * w + at + x) as usize] = words[(y * tw + x) as usize];
        }
    }
    (cov, left..left + iw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink_width(toast: Toast) -> u32 {
        let font = text::label_font().expect("label font");
        let s = setting(toast);
        let layout = text::fit(font, &toast.text(), s.w as f32, 1, s.px, s.min_px);
        let cov = text::coverage(s.w, s.h, &layout);
        let cols: Vec<u32> = (0..s.w)
            .filter(|x| (0..s.h).any(|y| cov[(y * s.w + x) as usize] > 0))
            .collect();
        match (cols.first(), cols.last()) {
            (Some(a), Some(b)) => b - a + 1,
            _ => 0,
        }
    }

    fn ink_rows(face: &CartFace) -> (u32, u32) {
        let mut first = None;
        let mut last = 0;
        for y in 0..face.h {
            let inked = (0..face.w).any(|x| face.rgba[((y * face.w + x) * 4 + 3) as usize] > 0);
            if inked {
                first.get_or_insert(y);
                last = y;
            }
        }
        (first.unwrap_or(0), last)
    }

    fn ink_height_at(toast: Toast, px: f32) -> u32 {
        let font = text::label_font().expect("label font");
        let layout = text::fit(font, &toast.text(), TOAST_W as f32, 1, px, px);
        let cov = text::coverage(TOAST_W, TOAST_H, &layout);
        let rows: Vec<u32> = (0..TOAST_H)
            .filter(|y| (0..TOAST_W).any(|x| cov[(y * TOAST_W + x) as usize] > 0))
            .collect();
        match (rows.first(), rows.last()) {
            (Some(a), Some(b)) => b - a + 1,
            _ => 0,
        }
    }

    #[test]
    fn the_ended_line_is_rastered_the_size_the_others_are() {
        let ended = toast_face(Toast::LinkEnded);
        let saved = toast_face(Toast::StateSaved);
        assert_eq!(
            (ended.w, ended.h),
            (saved.w, saved.h),
            "one box holds every banner"
        );
        let (top, bottom) = ink_rows(&ended);
        assert!(bottom > top, "the line rastered to nothing at all");

        let shipped = ink_height_at(Toast::LinkEnded, TOAST_PX);
        let shrunk = ink_height_at(Toast::LinkEnded, TOAST_MIN_PX);
        assert!(
            shipped > shrunk,
            "the shipped line is no taller than the {TOAST_MIN_PX} px fallback: {shipped} against {shrunk}"
        );
        let reference = ink_height_at(Toast::StateSaved, TOAST_PX);
        assert!(
            shipped.abs_diff(reference) <= 1,
            "LINK ENDED is {shipped} px of ink where STATE SAVED is {reference}"
        );
    }

    #[test]
    fn every_toast_is_set_at_full_size() {
        let font = text::label_font().expect("label font");
        for t in Toast::all() {
            let s = setting(t);
            let layout = layout_for(font, t, &t.text());
            assert_eq!(
                layout.px,
                s.px,
                "{:?} ({:?}) was shrunk to {} px to fit {}",
                t,
                t.text(),
                layout.px,
                s.w
            );
            assert!(layout.lines.len() <= s.lines, "{t:?} took {} lines", layout.lines.len());
            let w = ink_width(t);
            assert!(w <= s.w, "{:?} is {w} px wide in a {} px box", t, s.w);
        }
    }
}
