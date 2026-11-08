#![cfg(feature = "nds")]

use slot_gfx::{Canvas, Draw, TexId};
use slot_ui::{BoxScreen, Printed, MOUTH_H};

const CANVAS: Canvas = Canvas { w: 720.0, h: 480.0 };
const BOX: TexId = TexId::from_raw(1);
const LOGO: TexId = TexId::from_raw(2);
const TITLE: TexId = TexId::from_raw(3);
const SYSTEM: TexId = TexId::from_raw(4);

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
        title: Printed::new(TITLE, 200),
        system: Printed::new(SYSTEM, 120),
    }
}

#[test]
fn a_box_draws_left_and_the_title_right() {
    let out = drawn(full());
    let (bx, by, bw, bh) = rect_of(&out, BOX).expect("no box");
    let (tx, ty, tw, _) = rect_of(&out, TITLE).expect("no title");
    let (lx, ly, lw, lh) = rect_of(&out, LOGO).expect("no logo");
    let (sx, sy, _, _) = rect_of(&out, SYSTEM).expect("no system");
    assert!(
        bx + bw <= CANVAS.w / 2.0 + 20.0,
        "the box reaches past the left half: {bx} {bw}"
    );
    assert!(tx >= bx + bw, "the title overlaps the box");
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
        ly + lh <= ty && ty < sy,
        "logo, title, system are not stacked in order"
    );
    assert!(
        (tx + tw / 2.0 - (lx + lw / 2.0)).abs() < 1.0,
        "the title is not centred under the logo"
    );
    assert!(sx >= bx + bw);
}

#[test]
fn no_box_centres_the_title() {
    let out = drawn(BoxScreen {
        boxed: None,
        ..full()
    });
    assert!(rect_of(&out, BOX).is_none());
    let (tx, _, tw, _) = rect_of(&out, TITLE).expect("no title");
    assert!(
        (tx + tw / 2.0 - CANVAS.w / 2.0).abs() < 1.0,
        "the title is not centred: {tx} {tw}"
    );
}

#[test]
fn no_logo_puts_the_title_where_the_logo_goes() {
    let with = drawn(full());
    let without = drawn(BoxScreen {
        logo: None,
        ..full()
    });
    let (_, ly, _, _) = rect_of(&with, LOGO).unwrap();
    let (_, ty, _, _) = rect_of(&without, TITLE).unwrap();
    let (_, ty_with, _, _) = rect_of(&with, TITLE).unwrap();
    assert!(rect_of(&without, LOGO).is_none());
    assert!(
        ty < ty_with,
        "the title did not move up into the logo's place"
    );
    assert!(ty >= ly - 1.0, "the title moved above where the logo was");
}

#[test]
fn the_system_name_is_dimmer_than_the_title() {
    let out = drawn(full());
    let alpha = |want| {
        out.iter().find_map(|d| match *d {
            Draw::Tex { tex, alpha, .. } if tex == want => Some(alpha),
            _ => None,
        })
    };
    assert!(alpha(SYSTEM).unwrap() < alpha(TITLE).unwrap());
}
