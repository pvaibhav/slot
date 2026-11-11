use slot_power::{Battery, Charge};
use slot_store::BatteryDisplay;
use slot_ui::{draw_gauge, Draw, Gauge, Printed, TexId, GAUGE_H, GAUGE_W, HUD_INK, WALL};

fn percent_face() -> Printed {
    Printed { face: None, w: 30 }
}

fn gauge(battery: Option<Battery>) -> Gauge {
    Gauge {
        battery,
        percent: percent_face(),
        display: BatteryDisplay::Both,
    }
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
        gauge(at(68, Charge::Discharging)),
        None,
        HUD_INK,
        &mut idle,
    );
    draw_gauge(
        24.0,
        400.0,
        gauge(at(68, Charge::Charging)),
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
fn the_bolt_is_only_drawn_while_charging() {
    let bolt = |charge| {
        let mut out = Vec::new();
        draw_gauge(
            24.0,
            400.0,
            gauge(at(68, charge)),
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
            gauge(at(percent, Charge::Discharging)),
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
    draw_gauge(24.0, 400.0, gauge(None), None, HUD_INK, &mut out);
    assert!(out.is_empty());
}

fn shown(display: BatteryDisplay) -> Vec<(f32, f32, f32, f32)> {
    let mut out = Vec::new();
    draw_gauge(
        24.0,
        400.0,
        Gauge {
            display,
            ..gauge(at(68, Charge::Discharging))
        },
        None,
        HUD_INK,
        &mut out,
    );
    quads(&out)
}

fn is_text(q: &(f32, f32, f32, f32)) -> bool {
    (q.2 - percent_face().w as f32).abs() < 0.01
}

#[test]
fn icon_draws_the_capsule_and_no_number() {
    let icon = shown(BatteryDisplay::Icon);
    assert!(!icon.iter().any(is_text), "{icon:?}");
    let both = shown(BatteryDisplay::Both);
    let capsule: Vec<_> = both.iter().filter(|q| !is_text(q)).copied().collect();
    assert_eq!(icon, capsule);
}

#[test]
fn percentage_draws_the_number_where_the_capsule_starts() {
    let both = shown(BatteryDisplay::Both);
    let capsule_left = both
        .iter()
        .filter(|q| !is_text(q))
        .map(|q| q.0)
        .fold(f32::MAX, f32::min);
    let percent = shown(BatteryDisplay::Percent);
    assert_eq!(percent.len(), 1, "{percent:?}");
    assert!(is_text(&percent[0]));
    assert!((percent[0].0 - capsule_left).abs() < 0.01, "{percent:?}");
}

#[test]
fn the_wifi_icon_keeps_the_same_gap_after_every_display() {
    let face = (TexId::from_raw(2), 18, 14);
    for display in BatteryDisplay::ALL {
        let g = Gauge {
            display,
            ..gauge(at(68, Charge::Charging))
        };
        let mut drawn = Vec::new();
        draw_gauge(
            24.0,
            400.0,
            g,
            Some(TexId::from_raw(7)),
            HUD_INK,
            &mut drawn,
        );
        let right = quads(&drawn).iter().map(|q| q.0 + q.2).fold(0.0, f32::max);
        let mut wifi = Vec::new();
        slot_ui::draw_wifi_icon(slot_ui::Canvas::FRAMED, g, Some(face), &mut wifi);
        let left = quads(&wifi)[0].0;
        assert!(
            (left - right - 12.0).abs() <= 1.0,
            "{display:?}: the gauge ends at {right} and Wi-Fi starts at {left}"
        );
    }
}
