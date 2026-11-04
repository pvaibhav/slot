use slot_store::{GbColors, GbPalette};
use slot_ui::{
    toast_face, toast_rect, Canvas, Draw, Hud, HudKind, Icon, QuickRow, TexId, Toast, ALARM_BAND_H,
    OUT_H, OUT_W, PLATE_H,
};

#[test]
fn saving_and_loading_say_which_one_happened() {
    assert_eq!(Toast::StateSaved.text(), "State Saved");
    assert_eq!(Toast::StateLoaded.text(), "State Loaded");
}

#[test]
fn the_link_shortcut_on_the_wrong_core_says_to_switch() {
    assert_eq!(Toast::NeedsGpsp.text(), "Please switch to gpSP");
    let f = toast_face(Toast::NeedsGpsp);
    assert!(f.rgba.chunks(4).any(|p| p[3] > 0), "the banner is blank");
}

#[test]
fn sunlight_on_gpsp_says_to_switch_to_mgba() {
    assert_eq!(Toast::NeedsMgba.text(), "Please switch to mGBA");
    let f = toast_face(Toast::NeedsMgba);
    assert!(f.rgba.chunks(4).any(|p| p[3] > 0), "the banner is blank");
}

#[test]
fn a_cart_gpsp_cannot_link_says_there_is_no_link() {
    assert_eq!(Toast::NoLink.text(), "No link support");
    assert_eq!(Toast::NoWifi.text(), "This device has no Wi-Fi");
    let f = toast_face(Toast::NoLink);
    assert!(f.rgba.chunks(4).any(|p| p[3] > 0), "the banner is blank");
}

#[test]
fn the_link_ending_says_who_ended_it() {
    assert_eq!(Toast::LinkEnded.text(), "Link ended");
    assert_eq!(Toast::PeerEnded.text(), "Other player ended the link");
}

#[test]
fn the_banner_says_what_happened_and_never_what_is_on_screen() {
    let all = Toast::all();
    assert_eq!(
        all[..30],
        [
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
            Toast::ShaderAuthentic,
            Toast::ModelGameBoy,
            Toast::ModelPocket,
            Toast::ModelLight,
            Toast::CardUnwritable,
            Toast::CardRepair,
            Toast::CardRepairCharge,
            Toast::CardRepaired,
            Toast::CardRepairFailed,
            Toast::Forgotten,
        ],
        "a banner was added or dropped: every face is uploaded by its place in this list"
    );
    let palettes = GbColors::all().count();
    assert_eq!(palettes, 49);
    assert_eq!(
        all[30..30 + palettes],
        GbColors::all().map(Toast::Palette).collect::<Vec<_>>()[..],
        "the palette banners do not follow the fixed ones in palette order"
    );
    assert_eq!(
        all[30 + palettes..],
        QuickRow::ALL
            .into_iter()
            .filter(|r| r.binds())
            .map(Toast::Duplicate)
            .collect::<Vec<_>>()[..],
        "the duplicate warnings do not follow the palettes in row order"
    );
    for (i, t) in all.iter().enumerate() {
        assert_eq!(t.index(), i, "{t:?} does not answer to its own place");
        let f = toast_face(*t);
        assert!(
            f.rgba.chunks(4).any(|p| p[3] > 0),
            "{t:?} rastered to a blank banner"
        );
    }
}

#[test]
fn no_toast_is_shrunk_to_fit_its_box() {
    let rows = |t: Toast| {
        let f = toast_face(t);
        let (w, h) = (f.w as usize, f.h as usize);
        let ink = |x: usize, y: usize| f.rgba[(y * w + x) * 4 + 3] > 0;
        let mut from = 0;
        if t.icon().is_some() {
            let cols: Vec<usize> = (0..w).filter(|x| (0..h).any(|y| ink(*x, y))).collect();
            let gap = cols
                .windows(2)
                .position(|p| p[1] - p[0] > 3)
                .expect("no icon gap");
            from = cols[gap + 1];
        }
        let inked: Vec<usize> = (0..h).filter(|y| (from..w).any(|x| ink(x, *y))).collect();
        let first = *inked.first().expect("the banner is blank");
        let last = *inked.last().expect("the banner is blank");
        (first, last)
    };
    let (top, bottom) = rows(Toast::StateSaved);
    for t in Toast::all().into_iter().filter(|t| !t.alarm()) {
        let (a, b) = rows(t);
        assert!(
            a.abs_diff(top) <= 1 && b.abs_diff(bottom) <= 1,
            "{t:?} sits on rows {a}..{b} where the others sit on {top}..{bottom}, so it was shrunk to fit"
        );
    }
}

#[test]
fn a_toast_fades_on_the_same_curve_as_the_bar() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    assert!(h.toast_visible(2_499));
    assert!(!h.toast_visible(2_500));
}

#[test]
fn saying_the_same_thing_twice_re_shows_it_rather_than_stacking() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    h.toast(Toast::StateSaved, 2_400);
    assert_eq!(h.said(3_400), Some(Toast::StateSaved), "it did not re-show");
    assert_eq!(h.said(3_900), None, "it never faded");
}

#[test]
fn a_second_banner_replaces_the_first() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 1_000);
    h.toast(Toast::LinkEnded, 1_100);
    assert_eq!(h.said(1_200), Some(Toast::LinkEnded));
}

#[test]
fn a_toast_is_centred() {
    let (x, _, w, _) = toast_rect(Canvas::FRAMED, Toast::StateSaved);
    assert_eq!(x + w / 2.0, OUT_W as f32 / 2.0);
}

#[test]
fn a_toast_carries_its_own_halo() {
    let f = toast_face(Toast::StateSaved);
    let dark = f
        .rgba
        .chunks(4)
        .any(|p| p[3] > 0 && p[0] < 0x40 && p[1] < 0x40 && p[2] < 0x40);
    assert!(dark, "there is nothing dark behind the type");
}

#[test]
fn a_toast_sits_in_the_plate_band_and_is_backed_by_it() {
    let mut h = Hud::new();
    h.toast(Toast::StateSaved, 0);
    let mut out = Vec::new();
    h.draw(Canvas::FRAMED, 0, &mut out);

    let plate = out
        .iter()
        .find(|d| matches!(d, Draw::Rect { w, .. } if *w == OUT_W as f32))
        .expect("the toast has nothing to be read against");
    let Draw::Rect { colour, h: ph, .. } = plate else {
        unreachable!()
    };
    assert!(colour[3] > 0.6, "the plate is too faint to give contrast");
    assert!((*ph - PLATE_H).abs() < 0.01, "the plate is not the band");

    let (_, y, _, th) = toast_rect(Canvas::FRAMED, Toast::StateSaved);
    assert!(
        y >= 0.0 && y + th <= PLATE_H,
        "the toast at {y} is outside the band"
    );
}

#[test]
fn a_toast_takes_the_band_from_the_bar() {
    let mut h = Hud::new();
    h.show(HudKind::Volume, 50, false, 0);
    let mut bar_only = Vec::new();
    h.draw(Canvas::FRAMED, 0, &mut bar_only);
    let bars = bar_only.len();

    h.toast(Toast::StateSaved, 0);
    let mut both = Vec::new();
    h.draw(Canvas::FRAMED, 0, &mut both);
    assert!(
        both.len() < bars,
        "the bar is still drawn underneath the toast"
    );
}

#[test]
fn a_palette_banner_names_the_palette_without_its_boot_combo() {
    let p = GbPalette::parse("GBC Dark Green →A").unwrap();
    assert_eq!(
        Toast::Palette(GbColors::Palette(p)).text(),
        "GBC Dark Green"
    );
    assert_eq!(Toast::Palette(GbColors::Auto).text(), "Auto");
}

fn band(h: &Hud, now: u64) -> Option<(f32, f32, [f32; 4])> {
    let mut out = Vec::new();
    h.draw(Canvas::FRAMED, now, &mut out);
    out.iter().rev().find_map(|d| match d {
        Draw::Rect {
            y, w, h, colour, ..
        } if *w == OUT_W as f32 && *h == ALARM_BAND_H => Some((*y, *h, *colour)),
        _ => None,
    })
}

#[test]
fn the_card_alarm_stays_up_until_it_is_cleared() {
    let mut h = Hud::new();
    h.set_alarm(true, 1_000);
    assert!(
        band(&h, 3_600_000).is_some(),
        "the alarm went away by itself"
    );
    h.set_alarm(false, 3_600_001);
    let mut out = Vec::new();
    h.draw(Canvas::FRAMED, 3_600_002, &mut out);
    assert!(out.is_empty(), "the alarm outlived a write that landed");
}

#[test]
fn the_card_alarm_is_a_red_band_across_the_middle_that_pulses() {
    let mut h = Hud::new();
    h.set_alarm(true, 0);
    let (y, bh, bright) = band(&h, 0).expect("no alarm band");
    let middle = OUT_H as f32 / 2.0;
    assert!(
        (y + bh / 2.0 - middle).abs() < 0.5,
        "the band is not centred"
    );
    assert!(
        bright[0] > 2.0 * bright[1] && bright[0] > 2.0 * bright[2],
        "{bright:?} is not red"
    );
    let (_, _, dim) = band(&h, 250).unwrap();
    assert!(
        dim[0] < bright[0] - 0.2,
        "the band did not pulse: {bright:?} then {dim:?}"
    );
}

#[test]
fn a_second_failure_does_not_restart_the_pulse() {
    let mut h = Hud::new();
    h.set_alarm(true, 0);
    h.set_alarm(true, 250);
    let mut fresh = Hud::new();
    fresh.set_alarm(true, 0);
    assert_eq!(band(&h, 500), band(&fresh, 500));
}

#[test]
fn the_card_alarm_text_is_large_and_inside_its_band() {
    let alarm = toast_face(Toast::CardUnwritable);
    let toast = toast_face(Toast::StateSaved);
    assert!(alarm.h > toast.h && alarm.w > toast.w);
    let (x, y, w, th) = toast_rect(Canvas::FRAMED, Toast::CardUnwritable);
    let top = (OUT_H as f32 - ALARM_BAND_H) / 2.0;
    assert!(
        y >= top && y + th <= top + ALARM_BAND_H,
        "the alarm text overhangs its band"
    );
    assert_eq!(x + w / 2.0, OUT_W as f32 / 2.0);
}

#[test]
fn the_card_alarm_is_drawn_over_the_volume_bar() {
    let mut h = Hud::new();
    h.set_alarm(true, 0);
    h.show(HudKind::Volume, 50, false, 1_000);
    let mut out = Vec::new();
    h.draw(Canvas::FRAMED, 1_100, &mut out);
    let alarm = out
        .iter()
        .position(|d| matches!(d, Draw::Rect { h, .. } if *h == ALARM_BAND_H))
        .expect("the bar hid the alarm");
    assert!(alarm > 0, "the bar was not drawn under the alarm");
    assert!(
        out[alarm + 1..]
            .iter()
            .all(|d| matches!(d, Draw::Tex { .. })),
        "something was drawn over the alarm band"
    );
}

#[test]
fn the_card_alarm_says_what_failed_and_what_it_costs() {
    assert_eq!(Toast::CardUnwritable.text(), "Can't write to SD card");
    assert_eq!(
        Toast::CardUnwritable.detail(),
        Some("Progress is not being saved")
    );
    assert_eq!(Toast::StateSaved.detail(), None);
}

#[test]
fn the_card_alarm_inks_both_of_its_lines() {
    let f = toast_face(Toast::CardUnwritable);
    let inked = |rows: std::ops::Range<u32>| {
        rows.into_iter()
            .any(|y| (0..f.w).any(|x| f.rgba[((y * f.w + x) * 4 + 3) as usize] > 0x80))
    };
    let half = f.h / 2;
    assert!(inked(0..half), "the first line is blank");
    assert!(inked(half + 8..f.h), "the second line is blank");
}

#[test]
fn a_card_that_can_be_repaired_says_how() {
    assert!(Toast::CardRepair.alarm());
    assert_eq!(Toast::CardRepair.text(), Toast::CardUnwritable.text());
    assert_eq!(
        Toast::CardRepair.detail(),
        Some("Press SELECT + START to repair")
    );
    assert!(!Toast::CardRepaired.alarm());
    assert!(!Toast::CardRepairFailed.alarm());
}

#[test]
fn the_alarm_band_carries_the_banner_it_was_given() {
    let ids: Vec<TexId> = (0..Toast::all().len()).map(TexId::from_raw).collect();
    let alarm_tex = |h: &Hud| {
        let mut out = Vec::new();
        h.draw(Canvas::FRAMED, 0, &mut out);
        out.iter().find_map(|d| match d {
            Draw::Tex { tex, .. } => Some(*tex),
            _ => None,
        })
    };
    let mut h = Hud::new();
    h.set_toasts(ids.clone());
    h.set_alarm(true, 0);
    assert_eq!(alarm_tex(&h), Some(ids[Toast::CardUnwritable.index()]));
    h.set_alarm_toast(Toast::CardRepair);
    assert_eq!(alarm_tex(&h), Some(ids[Toast::CardRepair.index()]));
}

#[test]
fn the_repair_prompt_line_keeps_every_word() {
    let f = slot_ui::menu_detail_face("slot restarts when it is done.");
    let inked: Vec<bool> = (0..f.w as usize)
        .map(|x| (0..f.h as usize).any(|y| f.rgba[(y * f.w as usize + x) * 4 + 3] > 0))
        .collect();
    let first = inked.iter().position(|i| *i).expect("blank line");
    let last = inked.iter().rposition(|i| *i).unwrap();
    let mut gaps = 0;
    let mut run = 0;
    for i in &inked[first..=last] {
        if *i {
            if run >= 7 {
                gaps += 1;
            }
            run = 0;
        } else {
            run += 1;
        }
    }
    assert_eq!(gaps + 1, 6, "the line kept {} of its 6 words", gaps + 1);
}

#[test]
fn a_repair_result_stays_up_longer_than_a_toast() {
    for t in [Toast::CardRepaired, Toast::CardRepairFailed] {
        let mut h = Hud::new();
        h.toast(t, 1_000);
        assert!(h.toast_visible(6_999), "{t:?} went before it could be read");
        assert!(!h.toast_visible(7_000), "{t:?} never went");
    }
}

#[test]
fn the_shot_toast_says_screenshot_captured_beside_a_camera() {
    assert_eq!(Toast::ShotCaptured.text(), "Screenshot Captured");
    assert_eq!(Toast::ShotCaptured.icon(), Some(Icon::Camera));
    let f = toast_face(Toast::ShotCaptured);
    let beside = toast_face(Toast::Duplicate(QuickRow::ScSave));
    assert_eq!(
        (f.w, f.h),
        (beside.w, beside.h),
        "the camera is not laid out beside the words"
    );
    let warning = f
        .rgba
        .chunks(4)
        .any(|p| p[3] > 0xc0 && p[0] > 0xf0 && (0xa0..0xd0).contains(&p[1]) && p[2] < 0x50);
    assert!(!warning, "the camera is inked like a warning");
}

#[test]
fn a_failed_shot_says_so_beside_the_alert() {
    assert_eq!(Toast::ShotFailed.text(), "Screenshot Failed");
    assert_eq!(Toast::ShotFailed.icon(), Some(Icon::Alert));
}
