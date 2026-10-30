use slot_ui::{draw_wifi_icon, Draw, Printed, TexId};

fn extent(out: &[Draw]) -> (f32, f32, f32, f32) {
    let mut e = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for d in out {
        if let Draw::Rect { x, y, w, h, .. } = *d {
            e = (e.0.min(x), e.1.min(y), e.2.max(x + w), e.3.max(y + h));
        }
    }
    e
}

#[test]
fn it_sits_right_of_the_percent() {
    let mut out = Vec::new();
    draw_wifi_icon(Printed::new(TexId::from_raw(1), 50), [255; 3], 3, &mut out);
    let (left, top, _, bottom) = extent(&out);
    assert!(left > 24.0 + 14.0 + 5.0 + 26.4 + 3.0 + 7.0 + 50.0, "{left}");
    assert!(top > 422.0 && bottom < 480.0, "{top}..{bottom}");
}

#[test]
fn it_uses_the_footer_ink() {
    let mut out = Vec::new();
    draw_wifi_icon(Printed::default(), [0, 0, 0], 3, &mut out);
    assert!(out.iter().all(|d| matches!(d, Draw::Rect { colour, .. } if colour[0] == 0.0)));
    assert!(!out.is_empty());
}

#[test]
fn weak_signal_dims_the_outer_arcs() {
    let lit = |bars: u8| {
        let mut out = Vec::new();
        draw_wifi_icon(Printed::default(), [255; 3], bars, &mut out);
        let on = out.iter().filter(|d| matches!(d, Draw::Rect { colour, .. } if colour[3] == 1.0)).count();
        (on, out.len())
    };
    let (one, two, three) = (lit(1), lit(2), lit(3));
    assert!(one.0 < two.0 && two.0 < three.0, "{one:?} {two:?} {three:?}");
    assert_eq!(one.1, three.1, "the dim arcs stay on screen");
    assert_eq!(three.0, three.1);
}
