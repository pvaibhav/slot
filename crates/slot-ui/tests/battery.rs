use slot_power::{Battery, Charge};
use slot_ui::{draw_gauge, Draw, Printed, TexId, GAUGE_H, GAUGE_W, HUD_INK, WALL};

fn percent_face() -> Printed {
    Printed { face: None, w: 30 }
}

fn quads(out: &[Draw]) -> Vec<(f32, f32, f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, y, w, h, .. } | Draw::Tex { x, y, w, h, .. } => Some((x, y, w, h)),
            _ => None,
        })
        .collect()
}

fn at(percent: u8, charge: Charge) -> Option<Battery> {
    Some(Battery { percent, charge })
}

#[test]
fn nothing_moves_when_the_charge_state_changes() {
    let mut idle = Vec::new();
    let mut charging = Vec::new();
    draw_gauge(
        24.0,
        400.0,
        at(68, Charge::Discharging),
        percent_face(),
        None,
        HUD_INK,
        &mut idle,
    );
    draw_gauge(
        24.0,
        400.0,
        at(68, Charge::Charging),
        percent_face(),
        Some(TexId::from_raw(7)),
        HUD_INK,
        &mut charging,
    );
    let idle = quads(&idle);
    for q in idle.iter() {
        assert!(
            quads(&charging).contains(q),
            "{q:?} moved or vanished when charging started"
        );
    }
}

#[test]
fn the_bolt_never_reaches_the_capsule() {
    let mut out = Vec::new();
    draw_gauge(
        24.0,
        400.0,
        at(68, Charge::Charging),
        percent_face(),
        Some(TexId::from_raw(7)),
        HUD_INK,
        &mut out,
    );
    let bolt_right = out
        .iter()
        .find_map(|d| match *d {
            Draw::Tex { x, w, tex, .. } if tex == TexId::from_raw(7) => Some(x + w),
            _ => None,
        })
        .expect("the bolt did not draw while charging");
    let capsule_left = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, .. } => Some(x),
            _ => None,
        })
        .fold(f32::MAX, f32::min);
    assert!(
        bolt_right <= capsule_left,
        "the bolt's right edge ({bolt_right}) reaches past the capsule's left wall ({capsule_left})"
    );
}

#[test]
fn the_bolt_is_only_drawn_while_charging() {
    let bolt = |charge| {
        let mut out = Vec::new();
        draw_gauge(
            24.0,
            400.0,
            at(68, charge),
            percent_face(),
            Some(TexId::from_raw(7)),
            HUD_INK,
            &mut out,
        );
        out.iter()
            .any(|d| matches!(d, Draw::Tex { tex: t, .. } if *t == TexId::from_raw(7)))
    };
    assert!(bolt(Charge::Charging));
    assert!(!bolt(Charge::Discharging));
    assert!(!bolt(Charge::Full));
    assert!(!bolt(Charge::Unknown));
}

#[test]
fn the_fill_tracks_the_percent_and_never_leaves_the_capsule() {
    let mut widths = Vec::new();
    for percent in [0u8, 1, 50, 99, 100] {
        let mut out = Vec::new();
        draw_gauge(
            24.0,
            400.0,
            at(percent, Charge::Discharging),
            percent_face(),
            None,
            HUD_INK,
            &mut out,
        );
        let capsule_left = out
            .iter()
            .filter_map(|d| match *d {
                Draw::Rect { x, .. } => Some(x),
                _ => None,
            })
            .fold(f32::MAX, f32::min);
        let inner_right = capsule_left + GAUGE_W - 2.0 * WALL;
        let fill = out.iter().find_map(|d| match *d {
            Draw::Rect { x, w, .. } if (x - (capsule_left + 2.0 * WALL)).abs() < 0.01 => Some(w),
            _ => None,
        });
        if let Some(w) = fill {
            assert!(
                capsule_left + 2.0 * WALL + w <= inner_right + 0.01,
                "a {percent}% fill burst through the capsule's own wall"
            );
        }
        widths.push(fill.unwrap_or(0.0));
    }
    for pair in widths.windows(2) {
        assert!(
            pair[1] > pair[0],
            "the fill must grow strictly with the percent, got {widths:?}"
        );
    }
}

#[test]
fn the_capsule_is_a_fifth_larger_than_it_was() {
    assert!((GAUGE_W - 22.0 * 1.2).abs() < 0.01);
    assert!((GAUGE_H - 11.0 * 1.2).abs() < 0.01);
    assert!((WALL - 1.5 * 1.2).abs() < 0.01);
}

#[test]
fn no_reading_draws_nothing() {
    let mut out = Vec::new();
    draw_gauge(24.0, 400.0, None, percent_face(), None, HUD_INK, &mut out);
    assert!(out.is_empty());
}
