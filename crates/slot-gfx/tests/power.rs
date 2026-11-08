use slot_gfx::{screen_brightness, screen_rect_above, screen_scale, screen_width, OUT_H, OUT_W};

#[test]
fn power_on_expands_from_a_line_to_the_full_frame() {
    assert!(screen_scale(0.0) < 0.02, "it starts at full height");
    assert!(screen_scale(0.5) > 0.2 && screen_scale(0.5) < 1.0);
    assert!(
        (screen_scale(1.0) - 1.0).abs() < 0.001,
        "it never reaches full height"
    );
}

#[test]
fn the_brightness_overshoots_before_it_settles() {
    let peak = (0..=100)
        .map(|i| screen_brightness(i as f32 / 100.0))
        .fold(0.0f32, f32::max);
    assert!(peak > 1.05, "no overshoot, peak {peak:.2}");
    assert!(
        (screen_brightness(1.0) - 1.0).abs() < 0.01,
        "it settles bright or dim"
    );
}

#[test]
fn the_picture_scales_above_a_footer_and_leaves_the_strip_alone() {
    let footer = 58.0;
    let (x, y, w, h) = screen_rect_above(slot_gfx::Canvas::FRAMED, 1.0, footer);
    assert!(
        (x - 0.0).abs() < 0.01 && (y - 0.0).abs() < 0.01,
        "a full picture above the footer starts at the origin, at {x},{y}"
    );
    assert!((w - OUT_W as f32).abs() < 0.01, "width {w}");
    assert!(
        (h - (OUT_H as f32 - footer)).abs() < 0.01,
        "the open picture is {h}px, the footer was included"
    );
    let (_, mid_y, _, mid_h) = screen_rect_above(slot_gfx::Canvas::FRAMED, 0.5, footer);
    let view = OUT_H as f32 - footer;
    assert!(
        (mid_y + mid_h / 2.0 - view / 2.0).abs() < 0.5,
        "the collapsing line is not centred above the footer"
    );
    assert!(
        mid_y + mid_h <= view + 0.01,
        "the picture enters the footer at {}",
        mid_y + mid_h
    );
    let (_, full_y, _, full_h) = screen_rect_above(slot_gfx::Canvas::FRAMED, 0.5, 0.0);
    assert!(
        (full_y + full_h / 2.0 - OUT_H as f32 / 2.0).abs() < 0.5,
        "without a footer the picture still centres on the whole frame"
    );
}

#[test]
fn the_line_closes_to_a_dot_at_the_very_end() {
    assert!(screen_width(0.0) < 0.02, "the line never closes to a dot");
    assert!(
        screen_width(0.5) > 0.99,
        "the collapse eats the whole animation"
    );
}

#[test]
fn power_off_collapses_to_the_canvas_center() {
    let c = slot_gfx::Canvas {
        w: 1024.0,
        h: 768.0,
    };
    assert_eq!(
        slot_gfx::screen_rect_above(c, 1.0, 58.0),
        (0.0, 0.0, 1024.0, 710.0)
    );
    let (x, y, w, h) = slot_gfx::screen_rect_above(c, 0.5, 58.0);
    assert_eq!((x + w / 2.0, y + h / 2.0), (512.0, 355.0));
}
