#![cfg(feature = "nds")]

use slot_gfx::{Canvas, Draw, TexId};
use slot_ui::{BoxScreen, MOUTH_H};

const CANVAS: Canvas = Canvas { w: 720.0, h: 480.0 };
const BOX: TexId = TexId::from_raw(1);
const LOGO: TexId = TexId::from_raw(2);

fn drawn(screen: BoxScreen) -> Vec<Draw> {
    let mut out = Vec::new();
    screen.draw(&mut out);
    out
}

fn rect_of(out: &[Draw], want: TexId) -> Option<(f32, f32, f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex, ..
        } if tex == want => Some((x, y, w, h)),
        _ => None,
    })
}

fn full() -> BoxScreen {
    BoxScreen {
        canvas: CANVAS,
        boxed: Some((BOX, 554, 705)),
        logo: Some((LOGO, 600, 300)),
        over_game: false,
    }
}

#[test]
fn a_box_draws_left_and_the_logo_centred_right_of_it() {
    let out = drawn(full());
    let (bx, by, bw, bh) = rect_of(&out, BOX).expect("no box");
    let (lx, ly, lw, lh) = rect_of(&out, LOGO).expect("no logo");
    assert!(
        bx + bw <= CANVAS.w / 2.0 + 20.0,
        "the box reaches past the left half: {bx} {bw}"
    );
    assert!(
        lx >= bx + bw && lx + lw <= CANVAS.w,
        "the logo leaves the right column"
    );
    assert!(
        by >= 0.0 && by + bh <= CANVAS.h - MOUTH_H,
        "the box runs into the footer"
    );
    assert!(
        ((bw / bh) - 554.0 / 705.0).abs() < 0.01,
        "the box lost its shape"
    );
    assert!(((lw / lh) - 2.0).abs() < 0.01, "the logo lost its shape");
    assert!(
        (ly + lh / 2.0 - (by + bh / 2.0)).abs() < 0.5,
        "the logo is not centred on the box: {} vs {}",
        ly + lh / 2.0,
        by + bh / 2.0
    );
    assert_eq!(out.len(), 2, "something besides the box and logo drew");
}

#[test]
fn no_box_centres_the_logo() {
    let out = drawn(BoxScreen {
        boxed: None,
        ..full()
    });
    assert!(rect_of(&out, BOX).is_none());
    let (lx, _, lw, _) = rect_of(&out, LOGO).expect("no logo");
    assert!(
        (lx + lw / 2.0 - CANVAS.w / 2.0).abs() < 1.0,
        "the logo is not centred: {lx} {lw}"
    );
}

#[test]
fn over_the_game_a_dim_goes_down_first() {
    let out = drawn(BoxScreen {
        over_game: true,
        ..full()
    });
    match out.first() {
        Some(&Draw::Rect { w, h, colour, .. }) => {
            assert_eq!((w, h), (CANVAS.w, CANVAS.h));
            assert_eq!(&colour[..3], &[0.0, 0.0, 0.0]);
            assert!(
                colour[3] > 0.0 && colour[3] < 1.0,
                "not a dim: {}",
                colour[3]
            );
        }
        other => panic!("no dim under the art: {other:?}"),
    }
}
