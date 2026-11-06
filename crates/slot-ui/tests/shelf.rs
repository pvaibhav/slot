use slot_power::{Battery, Charge};
use slot_store::{Cart, Platform};
use slot_ui::{
    draw_footer, footer_ink, label_colour, rest_y, Canvas, Draw, GbShell, Printed, Shelf, TexId,
    CART_H, CART_W, LABEL_W, LABEL_X, LABEL_Y, OUT_W,
};

fn shelf_with(n: usize) -> Shelf {
    Shelf::new(
        (0..n)
            .map(|i| Cart {
                platform: Platform::Gba,
                stem: format!("Game {i}"),
                rom: format!("Games/GBA/Game {i}.gba").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: format!("GAME {i}"),
            })
            .collect(),
    )
}

fn placed(s: &Shelf) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    out.iter()
        .map(|d| match *d {
            Draw::Rect { x, w, .. } => (x, w),
            Draw::Tex { x, w, .. } => (x, w),
            Draw::Part { x, w, .. } => (x, w),
            Draw::Turned { x, w, .. } => (x, w),
            Draw::Game | Draw::Shot { .. } | Draw::Capture | Draw::Quad { .. } | Draw::Paper { .. } => (0.0, OUT_W as f32),
        })
        .collect()
}

fn xw(d: &Draw) -> (f32, f32) {
    match *d {
        Draw::Rect { x, w, .. }
        | Draw::Tex { x, w, .. }
        | Draw::Part { x, w, .. }
        | Draw::Turned { x, w, .. } => (x, w),
        Draw::Game | Draw::Shot { .. } | Draw::Capture | Draw::Quad { .. } | Draw::Paper { .. } => (0.0, OUT_W as f32),
    }
}

fn settle(s: &mut Shelf) {
    for _ in 0..600 {
        s.update(1.0 / 60.0);
    }
}

fn drawn_cart_indices(out: &[Draw]) -> Vec<usize> {
    let keys: Vec<[u8; 3]> = (0..16)
        .map(|i| label_colour(&format!("Game {i}")))
        .collect();
    out.iter()
        .map(|d| {
            let Draw::Rect { colour, .. } = d else {
                panic!("a cart with no face should draw as a rect");
            };
            let rgb = [0, 1, 2].map(|c| (colour[c] * 255.0).round() as u8);
            keys.iter()
                .position(|k| *k == rgb)
                .unwrap_or_else(|| panic!("quad {rgb:?} belongs to no cart"))
        })
        .collect()
}

#[test]
fn the_shelf_wraps_at_both_ends() {
    let mut s = shelf_with(4);
    s.left();
    assert_eq!(
        s.index, 3,
        "going left from the first cart should reach the last"
    );
    s.right();
    assert_eq!(
        s.index, 0,
        "going right from the last cart should reach the first"
    );
}

#[test]
fn wrapping_animates_one_step_not_the_long_way_back() {
    let mut s = shelf_with(8);
    for _ in 0..7 {
        s.right();
    }
    settle(&mut s);
    let before = s.scroll;
    s.right();
    let travel = (s.scroll_target() - before).abs();
    assert!(
        travel < 1.5,
        "the spring is travelling {travel} slots to move one"
    );
}

#[test]
fn a_settled_wrap_still_lands_on_the_selected_cart() {
    let mut s = shelf_with(5);
    s.left();
    settle(&mut s);
    assert_eq!(s.index, 4);
    assert!(
        (s.scroll.rem_euclid(5.0) - 4.0).abs() < 0.01,
        "scroll {} did not settle",
        s.scroll
    );
}

#[test]
fn the_neighbour_of_the_last_cart_is_the_first() {
    let s = shelf_with(4);
    assert_eq!(
        s.cart_at_offset(-1),
        Some(3),
        "left of the first is the last"
    );
    assert_eq!(s.cart_at_offset(1), Some(1));
}

#[test]
fn no_cart_is_drawn_twice_in_a_settled_row_of_three_or_more() {
    for n in [3usize, 4, 7] {
        let s = shelf_with(n);
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
        let drawn = drawn_cart_indices(&out);
        let mut uniq = drawn.clone();
        uniq.sort();
        uniq.dedup();
        assert_eq!(drawn.len(), uniq.len(), "{n} carts: one is on screen twice");
    }
}

#[test]
fn two_carts_repeat_around_the_ring() {
    let mut s = shelf_with(2);
    settle(&mut s);
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    assert_eq!(
        drawn_cart_indices(&out),
        vec![1, 0, 1],
        "a row of two is not the other cart, the selection, the other cart again"
    );
    let row = placed(&s);
    let centres: Vec<f32> = row.iter().map(|(x, w)| x + w / 2.0).collect();
    assert!(
        (centres[1] - 360.0).abs() < 0.5,
        "the selected cart sits at {}, not the middle of the screen",
        centres[1]
    );
    for (a, b) in [(centres[0], centres[1]), (centres[1], centres[2])] {
        assert!(
            (b - a - CART_W as f32).abs() < 0.5,
            "the row is {} apart rather than one pitch",
            b - a
        );
    }
    assert!(
        (row[0].1 - row[2].1).abs() < 0.01,
        "the two images of one cart came out at different sizes: {} and {}",
        row[0].1,
        row[2].1
    );
}

#[test]
fn a_press_slides_the_row_rather_than_redrawing_it() {
    let occupied = |s: &Shelf| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
        let which = drawn_cart_indices(&out);
        let mut row: Vec<(i64, usize)> = out
            .iter()
            .zip(which)
            .map(|(d, i)| {
                let (x, w) = xw(d);
                (((x + w / 2.0 - 360.0) / 240.0 * 1000.0).round() as i64, i)
            })
            .collect();
        row.sort();
        row
    };
    for n in [2usize, 3, 4, 5, 10] {
        for (name, press) in [
            ("right", Shelf::right as fn(&mut Shelf)),
            ("left", Shelf::left as fn(&mut Shelf)),
        ] {
            let mut s = shelf_with(n);
            settle(&mut s);
            let before = occupied(&s);
            press(&mut s);
            assert_eq!(
                occupied(&s),
                before,
                "{n} carts: the {name} press redrew the row instead of moving it"
            );
        }
    }
}

#[test]
fn a_row_travels_the_way_it_was_pressed() {
    for n in [2usize, 3, 4, 5, 10] {
        for (name, press, way) in [
            ("right", Shelf::right as fn(&mut Shelf), 1.0f32),
            ("left", Shelf::left as fn(&mut Shelf), -1.0),
        ] {
            let mut s = shelf_with(n);
            s.select(if way > 0.0 { n - 1 } else { 0 });
            settle(&mut s);
            let mut aim = s.scroll_target();
            for tap in 0..2 * n {
                press(&mut s);
                let sent = s.scroll_target();
                assert!(
                    (sent - aim - way).abs() < 0.01,
                    "{n} carts, tap {tap} {name}: the row was sent {} from {aim}, not one slot \
                     {name}",
                    sent - aim
                );
                assert_eq!(
                    (sent - s.scroll).signum(),
                    way,
                    "{n} carts, tap {tap} {name}: the row is travelling the other way"
                );
                aim = sent;
                for _ in 0..4 {
                    s.update(1.0 / 60.0);
                }
                assert_eq!(
                    s.scroll_target(),
                    sent,
                    "{n} carts, tap {tap} {name}: the row changed its mind in mid flight"
                );
            }
        }
    }
}

#[test]
fn a_held_scroll_never_travels_against_the_button() {
    for n in [2usize, 3, 4, 5, 10] {
        for (name, hold, way) in [
            ("right", Shelf::hold_right as fn(&mut Shelf, u64), 1.0f32),
            ("left", Shelf::hold_left as fn(&mut Shelf, u64), -1.0),
        ] {
            let mut s = shelf_with(n);
            s.select(if way > 0.0 { n - 1 } else { 0 });
            hold(&mut s, 0);
            let mut was = s.scroll;
            for f in 1..120u64 {
                s.tick(f * 1000 / 60);
                s.update(1.0 / 60.0);
                assert!(
                    (s.scroll - was) * way >= -1e-4,
                    "{n} carts, held {name}: the row travelled {} at frame {f}",
                    s.scroll - was
                );
                was = s.scroll;
            }
            let gone = (s.scroll - (if way > 0.0 { n - 1 } else { 0 }) as f32) * way;
            assert!(
                gone > 14.0,
                "{n} carts, held {name}: two seconds of holding moved the row {gone} pitches"
            );
        }
    }
}

#[test]
fn the_shelf_says_where_its_selected_cart_stands() {
    for n in [1usize, 2, 3, 5] {
        for frames in [0usize, 1, 2, 3, 5, 8, 13, 400] {
            let mut s = shelf_with(n);
            settle(&mut s);
            s.right();
            for _ in 0..frames {
                s.update(1.0 / 60.0);
            }
            let stem = s.carts[s.index].stem.clone();
            let mut whole = Vec::new();
            s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut whole);
            let mut without = Vec::new();
            s.draw_row(Canvas::FRAMED, Some(&stem), 0.0, 0.0, 1.0, &mut without);
            let dropped: Vec<(f32, f32)> = whole
                .iter()
                .map(xw)
                .filter(|q| !without.iter().map(xw).any(|k| k == *q))
                .collect();
            if n != 2 || frames == 400 {
                assert_eq!(
                    dropped.len(),
                    1,
                    "{n} carts, {frames} frames in: the row drew {} images of its selection",
                    dropped.len()
                );
            }
            let (said_x, scale) = s.selected_at(Canvas::FRAMED);
            let said = (said_x, CART_W as f32 * scale);
            assert!(
                dropped
                    .iter()
                    .any(|(x, w)| (x - said.0).abs() < 0.01 && (w - said.1).abs() < 0.01),
                "{n} carts, {frames} frames in: the shelf says its selection is at {said:?} and \
                 the row drew it at {dropped:?}"
            );
        }
    }
}

#[test]
fn shelf_scroll_settles_on_the_selected_index() {
    let mut s = shelf_with(5);
    s.right();
    s.right();
    for _ in 0..600 {
        s.update(1.0 / 60.0);
    }
    assert!(
        (s.scroll - 2.0).abs() < 0.01,
        "scroll {} did not settle",
        s.scroll
    );
}

#[test]
fn scroll_never_overshoots_the_cart_it_lands_on() {
    let mut s = shelf_with(5);
    s.right();
    for _ in 0..600 {
        s.update(1.0 / 60.0);
        assert!(s.scroll <= 1.0 + 1e-4, "overshot to {}", s.scroll);
    }
}

#[test]
fn an_empty_shelf_is_inert() {
    let mut s = shelf_with(0);
    s.right();
    s.left();
    assert_eq!(s.index, 0);
    s.update(1.0 / 60.0);
    assert!(placed(&s).is_empty());
}

#[test]
fn holding_a_direction_repeats_after_a_delay() {
    let mut s = shelf_with(6);
    s.hold_right(0);
    assert_eq!(s.index, 1, "the first press did not move");
    s.tick(399);
    assert_eq!(s.index, 1, "it repeated before the delay");
    s.tick(400);
    assert_eq!(s.index, 2, "it never repeated");
    s.tick(510);
    assert_eq!(s.index, 3);
    s.release_right();
    s.tick(2_000);
    assert_eq!(s.index, 3, "it kept repeating after release");
}

#[test]
fn a_held_direction_winds_down_to_a_floor() {
    let mut s = shelf_with(40);
    s.hold_right(0);
    assert_eq!(s.index, 1, "the first press did not move");

    let mut at = 400;
    for (rate, want) in [(110, 2), (85, 3), (65, 4), (50, 5)] {
        s.tick(at - 1);
        assert_eq!(s.index, want - 1, "it repeated early on its way to {want}");
        s.tick(at);
        assert_eq!(s.index, want, "it never repeated into {want}");
        at += rate;
    }

    for want in 6..=9 {
        s.tick(at - 1);
        assert_eq!(s.index, want - 1, "the floor gave way before {want}");
        s.tick(at);
        assert_eq!(s.index, want, "the floor stopped repeating at {want}");
        at += 50;
    }
}

#[test]
fn a_new_press_starts_the_repeat_over_at_the_slow_rate() {
    let mut s = shelf_with(40);
    s.hold_right(0);
    let mut at = 400;
    for rate in [110, 85, 65] {
        s.tick(at);
        at += rate;
    }
    assert_eq!(s.index, 4, "the hold did not wind down as expected");

    s.release_right();
    s.hold_right(at);
    assert_eq!(s.index, 5, "the fresh press did not move");
    s.tick(at + 400 - 1);
    assert_eq!(s.index, 5, "the fresh press repeated before the full delay");
    s.tick(at + 400);
    assert_eq!(s.index, 6, "the fresh press never repeated");
    s.tick(at + 400 + 110 - 1);
    assert_eq!(s.index, 6, "the fresh press kept the old hold's fast rate");
}

#[test]
fn the_other_direction_letting_go_does_not_stop_the_repeat() {
    let mut s = shelf_with(6);
    s.hold_right(0);
    s.release_left();
    s.tick(400);
    assert_eq!(s.index, 2, "releasing left stopped a held right");
}

#[test]
fn the_gauge_is_white_in_dark_mode_and_black_in_light_mode() {
    let gauge = |light| {
        let mut out = Vec::new();
        draw_footer(
            Canvas::FRAMED,
            Some(Battery {
                percent: 68,
                charge: Charge::Discharging,
            }),
            Printed { face: None, w: 30 },
            None,
            Printed { face: None, w: 40 },
            footer_ink(light),
            &mut out,
        );
        out.into_iter()
            .find_map(|d| match d {
                Draw::Rect { colour, .. } => Some(colour),
                _ => None,
            })
            .expect("the gauge did not draw")
    };
    assert_eq!(gauge(false), [1.0, 1.0, 1.0, 1.0]);
    assert_eq!(gauge(true), [0.0, 0.0, 0.0, 1.0]);
}

#[test]
fn the_band_prints_the_charge_at_one_margin_and_the_clock_at_the_other() {
    let mut out = Vec::new();
    draw_footer(
        Canvas::FRAMED,
        Some(Battery {
            percent: 68,
            charge: Charge::Charging,
        }),
        Printed { face: None, w: 30 },
        Some(TexId::from_raw(1)),
        Printed { face: None, w: 40 },
        [255, 255, 255],
        &mut out,
    );
    let xs: Vec<(f32, f32)> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, w, .. } | Draw::Tex { x, w, .. } => Some((x, x + w)),
            _ => None,
        })
        .collect();
    let leftmost = xs.iter().map(|(a, _)| *a).fold(f32::MAX, f32::min);
    let last = xs
        .iter()
        .cloned()
        .fold((0.0, 0.0), |m, r| if r.1 > m.1 { r } else { m });
    assert_eq!(
        leftmost, 24.0,
        "the charge does not start at the case margin"
    );
    assert_eq!(
        last.1,
        OUT_W as f32 - 24.0,
        "the clock does not end at the case margin"
    );
    assert_eq!(
        last.1 - last.0,
        40.0,
        "the rightmost thing on the band is not the clock"
    );
    for (a, b) in &xs {
        assert!(
            *b <= 224.0 || *a >= 496.0,
            "something is printed from {a} to {b}, across the cart bay at 224..496"
        );
    }
}

#[test]
fn the_footer_does_not_move_the_gauge_when_the_charge_state_changes() {
    let mut idle = Vec::new();
    draw_footer(
        Canvas::FRAMED,
        Some(Battery {
            percent: 68,
            charge: Charge::Discharging,
        }),
        Printed { face: None, w: 30 },
        None,
        Printed { face: None, w: 40 },
        [255, 255, 255],
        &mut idle,
    );
    let mut charging = Vec::new();
    draw_footer(
        Canvas::FRAMED,
        Some(Battery {
            percent: 68,
            charge: Charge::Charging,
        }),
        Printed { face: None, w: 30 },
        Some(TexId::from_raw(2)),
        Printed { face: None, w: 40 },
        [255, 255, 255],
        &mut charging,
    );
    for d in &idle {
        assert!(
            charging.contains(d),
            "{d:?} moved or vanished when charging started"
        );
    }
}

#[test]
fn a_band_with_no_gauge_still_draws_its_clock() {
    let mut out = Vec::new();
    draw_footer(
        Canvas::FRAMED,
        None,
        Printed::default(),
        None,
        Printed { face: None, w: 40 },
        [255, 255, 255],
        &mut out,
    );
    assert_eq!(out.len(), 1);
}

#[test]
fn a_refusal_moves_the_carts_and_leaves_the_device_where_it_is() {
    let s = shelf_with(3);
    let (mut still, mut shaken) = (Vec::new(), Vec::new());
    s.draw(Canvas::FRAMED, 0.0, None, &mut still);
    s.draw(Canvas::FRAMED, 9.0, None, &mut shaken);
    assert_eq!(still.len(), shaken.len(), "the shake changed the row");
    let mut row = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut row);
    let carts = row.len();
    assert!(
        carts > 0 && carts < still.len(),
        "{carts} of {}",
        still.len()
    );
    for (i, (a, b)) in still.iter().zip(&shaken).enumerate() {
        let ((ax, _), (bx, _)) = (xw(a), xw(b));
        if i < carts {
            assert!((bx - ax - 9.0).abs() < 0.01, "a cart stood still");
        } else {
            assert_eq!(ax, bx, "the device moved with the carts");
        }
    }
}

#[test]
fn carts_past_the_edges_of_the_row_are_not_drawn() {
    let mut s = shelf_with(30);
    for _ in 0..8 {
        s.right();
    }
    settle(&mut s);
    let n = placed(&s).len();
    assert!(n > 1, "only {n} carts drawn, the neighbours should peek in");
    assert!(n <= 5, "{n} carts drawn into a 720 px row");
}

#[test]
fn the_neighbours_peek_in_from_both_edges() {
    let s = shelf_with(5);
    let mut out = Vec::new();
    s.draw(Canvas::FRAMED, 0.0, None, &mut out);
    let mut spans = cart_spans(&out);
    spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    assert_eq!(spans.len(), 3, "expected three carts, got {}", spans.len());
    let (left, mid, right) = (spans[0], spans[1], spans[2]);
    assert!(
        mid.0 >= 0.0 && mid.1 <= OUT_W as f32,
        "the selected cart runs off screen at {mid:?}"
    );
    assert!(
        left.0 < 0.0 && left.1 > 20.0,
        "the left neighbour is not peeking in: {left:?}"
    );
    assert!(
        right.1 > OUT_W as f32 && right.0 < OUT_W as f32 - 20.0,
        "the right neighbour is not peeking in: {right:?}"
    );
}

fn cart_spans(out: &[Draw]) -> Vec<(f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect { x, w, h, .. }
            | Draw::Tex { x, w, h, .. }
            | Draw::Part { x, w, h, .. }
            | Draw::Turned { x, w, h, .. } => (h > 60.0).then_some((x, x + w)),
            Draw::Game | Draw::Shot { .. } | Draw::Capture | Draw::Quad { .. } | Draw::Paper { .. } => None,
        })
        .filter(|(x0, x1)| *x1 > 0.0 && *x0 < OUT_W as f32)
        .collect()
}

#[test]
fn the_row_parts_for_the_cart_going_in() {
    let s = shelf_with(5);
    let at = |recede: f32| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, Some("Game 0"), 0.0, recede, 1.0, &mut out);
        out
    };
    let start = at(0.0);
    let part = at(0.5);
    assert_eq!(start.len(), part.len(), "a cart left the row early");

    let centre = OUT_W as f32 / 2.0;
    for (a, b) in start.iter().zip(&part) {
        let (ax, aw) = xw(a);
        let (bx, _) = xw(b);
        let side = (ax + aw / 2.0) - centre;
        assert!(
            (bx - ax).signum() == side.signum(),
            "a cart at {ax} moved to {bx}, which is towards the slot, not away from it"
        );
        assert!((bx - ax).abs() > 1.0, "the cart at {ax} did not move");
    }
    assert!(
        at(1.0).is_empty(),
        "the row is still on screen with the cart seated"
    );
}

#[test]
fn dim_darkens_a_side_carts_face_and_not_the_black_under_it() {
    let mut s = shelf_with(3);
    let shadow = TexId::from_raw(99);
    s.set_shadow(shadow);
    let side = TexId::from_raw(11);
    s.set_faces(vec![TexId::from_raw(10), side, TexId::from_raw(12)]);
    let drawn = |dim: f32| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, Some("Game 0"), 0.0, 0.3, dim, &mut out);
        let (x, face) = out
            .iter()
            .find_map(|d| match *d {
                Draw::Tex { x, tex, alpha, .. } if tex == side => Some((x, alpha)),
                _ => None,
            })
            .expect("the side cart is not drawn");
        let under = out
            .iter()
            .find_map(|d| match *d {
                Draw::Tex {
                    x: at, tex, alpha, ..
                } if tex == shadow && at == x => Some(alpha),
                _ => None,
            })
            .expect("nothing is drawn under the side cart");
        (face, under)
    };
    let (face, under) = drawn(1.0);
    let (dimmed, dimmed_under) = drawn(0.5);
    assert!(
        (dimmed - face * 0.5).abs() < 1e-6,
        "the face went from {face} to {dimmed} at half dim"
    );
    assert_eq!(
        dimmed_under, under,
        "the black under the side cart changed with the dim"
    );
}

#[test]
fn a_cart_off_centre_stays_opaque() {
    let mut s = shelf_with(3);
    let side = TexId::from_raw(11);
    s.set_faces(vec![TexId::from_raw(10), side, TexId::from_raw(12)]);
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, Some("Game 0"), 0.0, 0.0, 1.0, &mut out);
    let alpha = out
        .iter()
        .find_map(|d| match *d {
            Draw::Tex { tex, alpha, .. } if tex == side => Some(alpha),
            _ => None,
        })
        .expect("the side cart is not drawn");
    assert_eq!(alpha, 1.0, "an off-centre cart faded to {alpha}");
}

fn gb_shelf_with(n: usize) -> Shelf {
    Shelf::new(
        (0..n)
            .map(|i| Cart {
                platform: Platform::Gb,
                stem: format!("Pak {i}"),
                rom: format!("Games/GB/Pak {i}.gb").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: format!("PAK {i}"),
            })
            .collect(),
    )
}

#[test]
fn a_game_boy_row_backs_its_carts_with_the_game_boy_shadow() {
    let mut s = gb_shelf_with(3);
    settle(&mut s);
    s.set_faces((0..3).map(|i| TexId::from_raw(20 + i)).collect());
    let gba = TexId::from_raw(98);
    s.set_shadow(gba);
    // Off-centre carts stay opaque, so only a receding row (the core picker) shows a shadow.
    let drawn = |s: &Shelf| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, None, 0.0, 0.3, 1.0, &mut out);
        out
    };
    assert!(
        !drawn(&s)
            .iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == gba)),
        "the GBA silhouette was stretched under a Game Boy pak"
    );
    let gb = TexId::from_raw(97);
    s.set_gb_shadow(GbShell::Notched, gb);
    assert!(
        drawn(&s)
            .iter()
            .any(|d| matches!(*d, Draw::Tex { tex, .. } if tex == gb)),
        "nothing backs the dimmed paks once their own shadow is uploaded"
    );
}

#[test]
fn a_colour_pak_and_a_grey_one_are_backed_by_their_own_shells() {
    let d = tempfile::tempdir().expect("tempdir");
    let games = d.path().join("Games/GB");
    std::fs::create_dir_all(&games).expect("create games dir");
    let carts: Vec<Cart> = [("Grey", 0x00u8), ("Clear", 0xc0)]
        .iter()
        .map(|(stem, cgb)| {
            let mut rom = vec![0u8; 0x150];
            rom[0x143] = *cgb;
            let path = games.join(format!("{stem}.gb"));
            std::fs::write(&path, rom).expect("write rom");
            Cart {
                platform: Platform::Gb,
                stem: (*stem).into(),
                rom: path,
                label: None,
                code: String::new(),
                shell: None,
                title: (*stem).to_uppercase(),
            }
        })
        .collect();

    let notched = TexId::from_raw(90);
    let rounded = TexId::from_raw(91);
    for (selected, neighbour_shell) in [(0usize, rounded), (1, notched)] {
        let mut s = Shelf::new(carts.clone());
        s.select(selected);
        settle(&mut s);
        s.set_faces(vec![TexId::from_raw(20), TexId::from_raw(21)]);
        for cart in &carts {
            slot_store::gb::cached_header(&cart.rom);
        }
        s.set_gb_shadow(GbShell::Notched, notched);
        s.set_gb_shadow(GbShell::Rounded, rounded);
        let mut out = Vec::new();
        s.draw_row(
            Canvas::FRAMED,
            Some(&carts[selected].stem),
            0.0,
            0.3,
            1.0,
            &mut out,
        );
        let backings: Vec<TexId> = out
            .iter()
            .filter_map(|d| match *d {
                Draw::Tex { tex, .. } if tex == notched || tex == rounded => Some(tex),
                _ => None,
            })
            .collect();
        assert_eq!(
            backings,
            vec![neighbour_shell; 2],
            "with the {} pak selected, its neighbour was backed by the wrong shell",
            carts[selected].stem
        );
    }
}

#[test]
fn a_cart_pushed_onto_the_row_draws_rather_than_stopping_the_device() {
    let mut s = gb_shelf_with(2);
    let gb = TexId::from_raw(97);
    let gba = TexId::from_raw(98);
    s.set_shadow(gba);
    s.set_gb_shadow(GbShell::Notched, gb);
    settle(&mut s);
    let backed = |s: &Shelf| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, Some("Pak 0"), 0.0, 0.3, 1.0, &mut out);
        out.iter()
            .filter(|d| matches!(**d, Draw::Tex { tex, .. } if tex == gb || tex == gba))
            .count()
    };
    assert_eq!(backed(&s), 2, "the neighbour was not backed to begin with");

    s.carts.push(Cart {
        platform: Platform::Gb,
        stem: "Pushed".into(),
        rom: "Games/GB/Pushed.gb".into(),
        label: None,
        code: String::new(),
        shell: None,
        title: "PUSHED".into(),
    });
    settle(&mut s);
    assert_eq!(
        backed(&s),
        2,
        "the row holding a cart the shells never heard of did not draw both its neighbours"
    );
}

#[test]
fn a_clear_cart_is_drawn_over_its_board_and_a_solid_one_is_not() {
    let cart = |stem: &str, code: &str| Cart {
        platform: Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        code: code.into(),
        shell: None,
        title: stem.to_uppercase(),
    };
    let board = TexId::from_raw(80);
    for (code, clear) in [("BPEE", true), ("BPRE", false)] {
        let mut s = Shelf::new(vec![cart("Game", code)]);
        settle(&mut s);
        let face = TexId::from_raw(20);
        s.set_faces(vec![face]);
        s.set_backing(board);
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
        let at = |t: TexId| {
            out.iter()
                .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == t))
        };
        assert_eq!(at(board).is_some(), clear, "{code}: board layer");
        if clear {
            assert!(at(board) < at(face), "the board was drawn over the plastic");
        }
    }
}

#[test]
fn a_clear_cart_draws_its_interior_then_the_board_then_the_plastic() {
    let mut s = Shelf::new(vec![Cart {
        platform: Platform::Gba,
        stem: "Game".into(),
        rom: "Games/GBA/Game.gba".into(),
        label: None,
        code: "BPEE".into(),
        shell: None,
        title: "GAME".into(),
    }]);
    settle(&mut s);
    let (interior, board, face) = (
        TexId::from_raw(81),
        TexId::from_raw(80),
        TexId::from_raw(20),
    );
    s.set_faces(vec![face]);
    s.set_backing(board);
    s.set_interior(0, interior);
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    let at = |t: TexId| {
        out.iter()
            .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == t))
            .expect("layer drawn")
    };
    assert!(at(interior) < at(board) && at(board) < at(face));
    let size = |t: TexId| {
        out.iter().find_map(|d| match *d {
            Draw::Tex { w, h, tex, .. } if tex == t => Some((w, h)),
            _ => None,
        })
    };
    assert_eq!(
        size(interior),
        size(face),
        "the back shell is not the cart's size"
    );
    assert_eq!(size(board), size(face), "the board is not the cart's size");
    assert_eq!(s.take_interior(0), Some(interior));
}

#[test]
fn a_clear_pak_draws_its_interior_then_the_board_then_the_plastic() {
    use slot_store::{Outline, ShellChoice, ShellFinish};
    let mut s = Shelf::new(vec![Cart {
        platform: Platform::Gb,
        stem: "Pak".into(),
        rom: "Games/GB/Pak.gb".into(),
        label: None,
        code: String::new(),
        shell: Some(ShellChoice {
            outline: Outline::Notched,
            colour: [0xff; 3],
            finish: ShellFinish::Clear,
        }),
        title: "PAK".into(),
    }]);
    settle(&mut s);
    let (interior, board, face) = (
        TexId::from_raw(81),
        TexId::from_raw(80),
        TexId::from_raw(20),
    );
    s.set_faces(vec![face]);
    s.set_gb_backing(GbShell::Notched, board);
    s.set_interior(0, interior);
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    let at = |t: TexId| {
        out.iter()
            .position(|d| matches!(*d, Draw::Tex { tex, .. } if tex == t))
            .expect("layer drawn")
    };
    assert!(at(interior) < at(board) && at(board) < at(face));
}

#[test]
fn only_a_clear_cart_has_anything_behind_its_plastic() {
    let cart = |stem: &str, code: &str| Cart {
        platform: Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        code: code.into(),
        shell: None,
        title: stem.to_uppercase(),
    };
    let (interior, board) = (TexId::from_raw(81), TexId::from_raw(80));
    for (code, clear) in [("BPEE", true), ("BPRE", false)] {
        let mut s = Shelf::new(vec![cart("Game", code)]);
        s.set_faces(vec![TexId::from_raw(20)]);
        s.set_backing(board);
        s.set_interior(0, interior);
        let want = if clear {
            [Some(interior), Some(board)]
        } else {
            [None, None]
        };
        assert_eq!(s.behind("Game"), want, "{code}");
    }
    let s = Shelf::new(vec![cart("Game", "BPEE")]);
    assert_eq!(s.behind("Nothing"), [None, None]);
}

#[test]
fn a_pak_draws_its_folder_shape_until_its_rom_is_indexed() {
    let d = tempfile::tempdir().expect("tempdir");
    let carts: Vec<Cart> = ["Front", "Dual"]
        .iter()
        .map(|stem| Cart {
            platform: Platform::Gbc,
            stem: (*stem).into(),
            rom: d.path().join(format!("{stem}.gbc")),
            label: None,
            code: String::new(),
            shell: None,
            title: stem.to_uppercase(),
        })
        .collect();
    let mut rom = vec![0u8; 0x150];
    rom[0x143] = 0x80;
    std::fs::write(&carts[1].rom, rom).expect("write rom");
    let mut s = Shelf::new(carts);
    settle(&mut s);
    s.set_faces(vec![TexId::from_raw(20), TexId::from_raw(21)]);
    let notched = TexId::from_raw(90);
    let rounded = TexId::from_raw(91);
    s.set_gb_shadow(GbShell::Notched, notched);
    s.set_gb_shadow(GbShell::Rounded, rounded);
    let shadow = |s: &Shelf| {
        let mut out = Vec::new();
        s.draw_row(Canvas::FRAMED, Some("Front"), 0.0, 0.3, 1.0, &mut out);
        out.iter().find_map(|d| match *d {
            Draw::Tex { tex, .. } if tex == notched || tex == rounded => Some(tex),
            _ => None,
        })
    };
    assert_eq!(shadow(&s), Some(rounded), "the shelf read the ROM while drawing");
    slot_store::gb::cached_header(&s.carts[1].rom);
    assert_eq!(shadow(&s), Some(notched), "the indexed shape was not used");
}

fn mixed_shelf(platforms: &[Platform]) -> Shelf {
    let mut s = Shelf::new(
        platforms
            .iter()
            .enumerate()
            .map(|(i, p)| Cart {
                platform: *p,
                stem: format!("Mixed {i}"),
                rom: match p {
                    Platform::Gba => format!("Games/GBA/Mixed {i}.gba"),
                    _ => format!("Games/GB/Mixed {i}.gb"),
                }
                .into(),
                label: None,
                code: String::new(),
                shell: None,
                title: format!("MIXED {i}"),
            })
            .collect(),
    );
    s.set_faces(
        (0..platforms.len())
            .map(|i| TexId::from_raw(60 + i))
            .collect(),
    );
    s
}

fn face_spans(s: &Shelf, n: usize) -> Vec<Option<(f32, f32)>> {
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    (0..n)
        .map(|i| {
            out.iter().find_map(|d| match *d {
                Draw::Tex { x, w, tex, .. } if tex == TexId::from_raw(60 + i) => Some((x, x + w)),
                _ => None,
            })
        })
        .collect()
}

#[test]
fn a_game_boy_pak_beside_a_gba_cart_sits_the_gap_two_gba_carts_do() {
    let gba = mixed_shelf(&[Platform::Gba, Platform::Gba, Platform::Gba]);
    let mut gba = gba;
    gba.select(1);
    settle(&mut gba);
    let spans = face_spans(&gba, 3);
    let gba_gap = spans[1].unwrap().0 - spans[0].unwrap().1;
    assert!(gba_gap > 10.0, "two GBA carts touch: {gba_gap}");

    let order = [
        Platform::Gb,
        Platform::Gba,
        Platform::Gb,
        Platform::Gba,
        Platform::Gb,
    ];
    for selected in [1, 2] {
        let mut s = mixed_shelf(&order);
        s.select(selected);
        settle(&mut s);
        let spans = face_spans(&s, 5);
        let at = |i: usize| spans[i].expect("a neighbour of the selected cart is not drawn");
        for (a, b) in [(selected - 1, selected), (selected, selected + 1)] {
            let gap = at(b).0 - at(a).1;
            assert!(
                (gap - gba_gap).abs() < 0.5,
                "with cart {selected} selected, carts {a} and {b} sit {gap} apart, two GBA carts {gba_gap}"
            );
        }
    }
}

#[test]
fn a_mixed_row_does_not_jump_when_the_selection_moves() {
    let order = [
        Platform::Gb,
        Platform::Gba,
        Platform::Gb,
        Platform::Gba,
        Platform::Gb,
    ];
    let mut s = mixed_shelf(&order);
    s.select(2);
    settle(&mut s);
    let before = face_spans(&s, 5);
    s.right();
    let after = face_spans(&s, 5);
    for i in 1..=3 {
        let (b, a) = (before[i].unwrap(), after[i].unwrap());
        assert!(
            (b.0 - a.0).abs() < 0.01,
            "cart {i} jumped from {b:?} to {a:?} the moment the selection moved"
        );
    }
}

#[test]
fn only_a_hearted_cart_wears_the_heart_in_its_label_top_right_corner() {
    let heart = TexId::from_raw(9);
    let mut s = shelf_with(3);
    s.set_heart(heart, 22, 22);
    s.mark_hearted(|c| c.stem == "Game 1");
    s.select(1);
    settle(&mut s);
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    let hearts: Vec<(f32, f32)> = out
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex { x, y, w, tex, .. } if tex == heart => Some((x + w, y)),
            _ => None,
        })
        .collect();
    assert_eq!(hearts.len(), 1, "{out:?}");
    let (right, top) = hearts[0];
    let x0 = 360.0 - CART_W as f32 / 2.0;
    let y0 = rest_y(slot_ui::Canvas::FRAMED, CART_H as f32);
    let (lx, ly) = (x0 + LABEL_X as f32, y0 + LABEL_Y as f32);
    let label_right = lx + LABEL_W as f32;
    assert!(
        right <= label_right && right > label_right - 16.0,
        "heart right edge {right}, label right {label_right}"
    );
    assert!(
        top >= ly && top < ly + 16.0,
        "heart top {top}, label top {ly}"
    );
}

fn screen_is_full_of_carts(s: &Shelf) -> Result<(), String> {
    let mut out = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut out);
    let mut spans: Vec<(f32, f32)> = cart_spans(&out)
        .into_iter()
        .map(|(a, b)| (a.max(0.0), b.min(OUT_W as f32)))
        .collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (mut covered, mut reach) = (0.0f32, 0.0f32);
    for (a, b) in spans {
        if b > reach {
            covered += b - a.max(reach);
            reach = b;
        }
    }
    match covered >= 0.75 * OUT_W as f32 {
        true => Ok(()),
        false => Err(format!(
            "carts cover {covered} of {OUT_W} px, scroll {} target {}",
            s.scroll,
            s.scroll_target()
        )),
    }
}

#[test]
fn a_row_held_at_full_speed_leaves_no_empty_screen() {
    for (name, hold) in [
        ("right", Shelf::hold_right as fn(&mut Shelf, u64)),
        ("left", Shelf::hold_left as fn(&mut Shelf, u64)),
    ] {
        let mut s = shelf_with(500);
        hold(&mut s, 0);
        for f in 1..360u64 {
            s.tick(f * 1000 / 60);
            s.update(1.0 / 60.0);
            if let Err(e) = screen_is_full_of_carts(&s) {
                panic!("held {name}, frame {f}: {e}");
            }
        }
    }
}

#[test]
fn a_slow_frame_mid_scroll_does_not_fling_the_row_off_screen() {
    for stall in [0.1f32, 0.2, 0.32, 1.0] {
        let mut s = shelf_with(500);
        s.hold_right(0);
        let mut now = 0u64;
        for f in 0..200 {
            let dt = if f == 120 { stall } else { 1.0 / 60.0 };
            now += (dt * 1000.0) as u64;
            s.tick(now);
            s.update(dt);
            if let Err(e) = screen_is_full_of_carts(&s) {
                panic!("a {stall} s frame at frame 120, frame {f}: {e}");
            }
        }
    }
}

#[test]
fn a_row_held_through_slow_frames_stays_on_screen() {
    for frame_ms in [60u64, 75, 100, 150] {
        let mut s = shelf_with(500);
        s.hold_right(0);
        for f in 1..(4000 / frame_ms) {
            s.tick(f * frame_ms);
            s.update(frame_ms as f32 / 1000.0);
            if let Err(e) = screen_is_full_of_carts(&s) {
                panic!("held through {frame_ms} ms frames, frame {f}: {e}");
            }
        }
    }
}

fn lettered_shelf(n: usize) -> Shelf {
    Shelf::new(
        (0..n)
            .map(|i| {
                let letter = (b'A' + (i * 26 / n) as u8) as char;
                Cart {
                    platform: Platform::Gba,
                    stem: format!("{letter} Game {i}"),
                    rom: format!("Games/GBA/{letter} Game {i}.gba").into(),
                    label: None,
                    code: String::new(),
                    shell: None,
                    title: format!("{letter} GAME {i}"),
                }
            })
            .collect(),
    )
}

#[test]
fn letter_jumps_held_through_slow_frames_keep_the_row_on_screen() {
    for frame_ms in [16u64, 70, 100, 150] {
        for (name, jump) in [
            ("down", Shelf::jump_next_letter as fn(&mut Shelf)),
            ("up", Shelf::jump_prev_letter as fn(&mut Shelf)),
        ] {
            let mut s = lettered_shelf(5000);
            let mut next = 0u64;
            for f in 0..(4000 / frame_ms) {
                let now = f * frame_ms;
                if now >= next {
                    jump(&mut s);
                    next = now + 100;
                }
                s.update(frame_ms as f32 / 1000.0);
                assert!(
                    s.scroll.is_finite(),
                    "held {name}, {frame_ms} ms frames: scroll {}",
                    s.scroll
                );
                if let Err(e) = screen_is_full_of_carts(&s) {
                    panic!("held {name} through {frame_ms} ms frames, frame {f}: {e}");
                }
            }
        }
    }
}

#[test]
fn a_wrapped_cart_stands_on_the_cart_foot_with_no_cart_parts_around_it() {
    let mut s = shelf_with(1);
    s.select(0);
    s.set_face(0, TexId::from_raw(3));
    s.set_interior(0, TexId::from_raw(4));
    s.set_wrapped(0, Some(TexId::from_raw(7)));
    let mut d = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut d);
    let [Draw::Tex { x, y, w, h, tex, .. }] = d[..] else {
        panic!("{d:?}");
    };
    let (bw, bh) = slot_ui::box_size(Platform::Gba);
    assert_eq!(tex, TexId::from_raw(7));
    assert_eq!((w, h), (bw as f32, bh as f32));
    assert_eq!(x + w / 2.0, Canvas::FRAMED.w / 2.0);
    assert_eq!(y + h, slot_ui::foot_y(slot_ui::Canvas::FRAMED, CART_H as f32));
}

#[test]
fn a_wrapped_cart_draws_its_box_and_an_unwrapped_one_draws_its_cart() {
    let mut s = shelf_with(1);
    s.select(0);
    let mut plain = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut plain);
    assert!(plain.iter().all(|d| matches!(d, Draw::Rect { .. })));
    s.set_wrapped(0, Some(TexId::from_raw(7)));
    let mut boxed = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut boxed);
    assert!(boxed.iter().any(|d| matches!(d, Draw::Tex { tex, .. } if *tex == TexId::from_raw(7))));
    s.set_wrapped(0, None);
    let mut after = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut after);
    assert_eq!(after.len(), plain.len());
    assert!(after.iter().all(|d| matches!(d, Draw::Rect { .. })));
}

#[test]
fn restocking_keeps_the_faces_of_carts_that_stay() {
    let mut s = shelf_with(4);
    for i in 0..4 {
        s.set_face(i, TexId::from_raw(100 + i));
        s.set_interior(i, TexId::from_raw(200 + i));
    }
    let kept: Vec<Cart> = [0, 2, 3].iter().map(|&i| s.carts[i].clone()).collect();
    let freed = s.restock(kept);
    assert_eq!(freed, [TexId::from_raw(101), TexId::from_raw(201)]);
    assert_eq!(s.face(0), Some(TexId::from_raw(100)));
    assert_eq!(s.face(1), Some(TexId::from_raw(102)));
    assert_eq!(s.face(2), Some(TexId::from_raw(103)));
}
