use slot_ui::{badge_at, sticker_rect, toast_rect, Canvas, Toast, STICKER_W};

const NATIVE: Canvas = Canvas { w: 640.0, h: 480.0 };

#[test]
fn the_about_sticker_fits_a_native_canvas_with_its_margin() {
    let (x, _, w, _) = sticker_rect(NATIVE);
    assert!(
        x >= 16.0 && x + w <= 624.0,
        "the sticker runs to {x}..{}",
        x + w
    );
    assert_eq!(sticker_rect(Canvas::FRAMED).2, STICKER_W as f32);
}

#[test]
fn a_badge_hugs_the_native_right_edge() {
    assert_eq!(badge_at(NATIVE, 20.0, 10.0).0, 640.0 - 12.0 - 20.0);
}

#[test]
fn a_toast_centres_on_a_native_canvas() {
    let (x, _, w, _) = toast_rect(NATIVE, Toast::StateSaved);
    assert_eq!(x + w / 2.0, 320.0);
}
