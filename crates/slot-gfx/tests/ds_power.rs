use slot_gfx::{ds_rect, ds_rect_powered};

const PANEL: (u32, u32) = (1024, 768);

fn centre([x, y, w, h]: [i32; 4]) -> (i32, i32) {
    (x + w / 2, y + h / 2)
}

#[test]
fn a_powered_ds_screen_fills_its_rect() {
    assert_eq!(ds_rect_powered(PANEL, 1.0), ds_rect(PANEL));
}

#[test]
fn a_powering_off_ds_screen_squashes_about_its_centre_like_the_top_picture() {
    let full = ds_rect(PANEL);
    let half = ds_rect_powered(PANEL, 0.5);
    assert_eq!(half[2], full[2], "the width only goes in the last moment");
    assert!(half[3] < full[3] && half[3] > full[3] / 2, "{half:?}");
    assert!((centre(half).1 - centre(full).1).abs() <= 1, "{half:?} is not centred on {full:?}");
}

#[test]
fn a_powered_off_ds_screen_is_a_dot() {
    let off = ds_rect_powered(PANEL, 0.0);
    assert!(off[2] <= 3 && off[3] <= 3, "{off:?}");
    let (cx, cy) = centre(ds_rect(PANEL));
    assert!((centre(off).0 - cx).abs() <= 2 && (centre(off).1 - cy).abs() <= 2, "{off:?}");
}
