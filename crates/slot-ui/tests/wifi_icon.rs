use slot_ui::{draw_wifi_icon, wifi_face, Draw, Printed, TexId, UndoFace};

fn tex(out: &[Draw]) -> (f32, f32, f32, f32) {
    match out {
        [Draw::Tex { x, y, w, h, .. }] => (*x, *y, *x + *w, *y + *h),
        _ => panic!("{out:?}"),
    }
}

fn ink(f: &UndoFace) -> Vec<[u8; 3]> {
    f.rgba
        .chunks(4)
        .filter(|p| p[3] > 128)
        .map(|p| [p[0], p[1], p[2]])
        .collect()
}

#[test]
fn it_sits_right_of_the_percent() {
    let mut out = Vec::new();
    let face = (TexId::from_raw(2), 18, 14);
    draw_wifi_icon(Printed::new(TexId::from_raw(1), 50), Some(face), &mut out);
    let (left, top, _, bottom) = tex(&out);
    assert!(left > 24.0 + 14.0 + 5.0 + 26.4 + 3.0 + 7.0 + 50.0, "{left}");
    assert!(top > 422.0 && bottom < 480.0, "{top}..{bottom}");
}

#[test]
fn nothing_draws_before_the_faces_arrive() {
    let mut out = Vec::new();
    draw_wifi_icon(Printed::default(), None, &mut out);
    assert!(out.is_empty());
}

#[test]
fn it_wears_the_footer_ink() {
    let face = wifi_face(3, [10, 20, 30]);
    let inked = ink(&face);
    assert!(inked.len() > 30, "{}", inked.len());
    assert!(inked.iter().all(|c| *c == [10, 20, 30]));
}

#[test]
fn a_stronger_signal_fills_more_of_the_fan_in_the_same_box() {
    let faces: Vec<UndoFace> = (1..=3).map(|n| wifi_face(n, [255; 3])).collect();
    let fills: Vec<usize> = faces.iter().map(|f| ink(f).len()).collect();
    assert!(fills.windows(2).all(|w| w[1] > w[0]), "{fills:?}");
    assert!(faces.iter().all(|f| (f.w, f.h) == (faces[0].w, faces[0].h)));
    assert!(faces[0].h <= 16, "{}", faces[0].h);
}
