use slot_ui::{grid_cell, grid_rows_shown, Canvas, ShotMode, ShotViewer, SHOT_HINTS};

const C720: Canvas = Canvas { w: 720.0, h: 480.0 };

fn viewer(n: usize) -> ShotViewer {
    ShotViewer::new(
        (0..n)
            .map(|i| {
                let stamp = format!("2026-10-22_17-30-{:02}", 59 - i);
                slot_store::ShotEntry {
                    path: format!("{stamp}.png").into(),
                    stamp,
                }
            })
            .collect(),
    )
}

#[test]
fn opens_in_the_grid_on_the_newest() {
    let v = viewer(7);
    assert_eq!(v.mode(), ShotMode::Grid);
    assert_eq!(v.index, 0);
    assert_eq!(v.top_row(), 0);
}

#[test]
fn right_at_a_row_end_goes_to_the_next_row_start() {
    let mut v = viewer(7);
    for _ in 0..3 {
        v.grid_move(1, 0, C720);
    }
    assert_eq!(v.index, 3);
}

#[test]
fn left_and_right_stop_at_the_ends() {
    let mut v = viewer(4);
    v.grid_move(-1, 0, C720);
    assert_eq!(v.index, 0);
    for _ in 0..9 {
        v.grid_move(1, 0, C720);
    }
    assert_eq!(v.index, 3);
}

#[test]
fn up_on_the_top_row_and_down_on_the_bottom_row_do_nothing() {
    let mut v = viewer(5);
    v.grid_move(0, -1, C720);
    assert_eq!(v.index, 0);
    v.index = 4;
    v.grid_move(0, 1, C720);
    assert_eq!(v.index, 4);
}

#[test]
fn down_onto_a_shorter_last_row_lands_on_its_last_shot() {
    let mut v = viewer(4);
    v.index = 2;
    v.grid_move(0, 1, C720);
    assert_eq!(v.index, 3);
}

#[test]
fn the_grid_scrolls_a_row_at_a_time_to_keep_the_box_shown() {
    let mut v = viewer(12);
    for _ in 0..2 {
        v.grid_move(0, 1, C720);
    }
    assert_eq!(v.top_row(), 1);
    v.grid_move(0, -1, C720);
    assert_eq!(v.top_row(), 1);
    v.grid_move(0, -1, C720);
    assert_eq!(v.top_row(), 0);
}

#[test]
fn deleting_at_the_bottom_keeps_the_box_and_scroll_valid() {
    let mut v = viewer(7);
    for _ in 0..2 {
        v.grid_move(0, 1, C720);
    }
    assert_eq!((v.index, v.top_row()), (6, 1));
    v.remove_selected(C720);
    assert_eq!(v.index, 5);
    assert_eq!(v.top_row(), 0);
}

#[test]
fn a_opens_full_and_back_returns_to_the_same_shot() {
    let mut v = viewer(9);
    v.index = 7;
    v.open();
    assert_eq!(v.mode(), ShotMode::Full);
    v.step(-1);
    v.to_grid(C720);
    assert_eq!(v.mode(), ShotMode::Grid);
    assert_eq!(v.index, 6);
    assert!(v.top_row() <= 2 && 2 < v.top_row() + 2, "{}", v.top_row());
}

#[test]
fn full_screen_steps_stop_at_either_end() {
    let mut v = viewer(2);
    v.open();
    v.step(-1);
    assert_eq!(v.index, 0);
    v.step(1);
    v.step(1);
    assert_eq!(v.index, 1);
}

#[test]
fn the_count_is_one_based() {
    let mut v = viewer(22);
    v.index = 1;
    assert_eq!(v.count_text(), "2 of 22");
}

fn labels(v: &ShotViewer) -> Vec<&'static str> {
    v.legend().iter().map(|&i| SHOT_HINTS[i].1).collect()
}

#[test]
fn each_mode_has_its_legend() {
    let mut v = viewer(2);
    assert_eq!(labels(&v), ["View", "Back", "Delete", "Multi"]);
    v.open();
    assert_eq!(labels(&v), ["Zoom", "Back", "Delete"]);
}

#[test]
fn the_grid_cell_matches_the_mockup() {
    let (w, h) = grid_cell(C720);
    assert!(
        (w - 218.67).abs() < 0.01 && (h - 145.78).abs() < 0.01,
        "{w} {h}"
    );
    let c640 = Canvas { w: 640.0, h: 480.0 };
    assert_eq!(grid_cell(c640), (192.0, 128.0));
    assert_eq!(grid_rows_shown(C720), 2);
    assert_eq!(grid_rows_shown(c640), 2);
}

fn full_of(n: usize, w: u32, h: u32) -> ShotViewer {
    let mut v = viewer(n);
    v.open();
    v.set_face(Some((slot_ui::TexId::from_raw(5), w, h)));
    v
}

fn full(w: u32, h: u32) -> ShotViewer {
    full_of(1, w, h)
}

#[test]
fn zoom_steps_one_to_four_and_back() {
    let mut v = full(720, 480);
    for z in [2, 3, 4, 4] {
        v.zoom_in(C720);
        assert_eq!(v.zoom(), z);
    }
    assert_eq!(v.mode(), ShotMode::Zoomed);
    for z in [3, 2, 1] {
        v.zoom_out(C720);
        assert_eq!(v.zoom(), z);
    }
    assert_eq!(v.mode(), ShotMode::Full);
}

#[test]
fn zoom_keeps_the_centre() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    assert_eq!(v.shot_rect(C720), Some((-360.0, -240.0, 1440.0, 960.0)));
}

#[test]
fn no_zoom_before_the_shot_has_decoded() {
    let mut v = viewer(1);
    v.open();
    v.zoom_in(C720);
    assert_eq!(v.zoom(), 1);
    assert_eq!(v.mode(), ShotMode::Full);
}

#[test]
fn a_tap_pans_a_short_step_and_stops_at_the_edge() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    v.pan_tap(-1, 0, C720);
    assert_eq!(v.shot_rect(C720).unwrap().0, -336.0);
    for _ in 0..40 {
        v.pan_tap(-1, 0, C720);
    }
    assert_eq!(v.shot_rect(C720).unwrap().0, 0.0);
}

#[test]
fn holding_pans_a_panel_width_a_second() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    v.zoom_in(C720);
    v.pan_hold(1, 0, true);
    v.tick(1000, C720);
    v.tick(1250, C720);
    let x = v.shot_rect(C720).unwrap().0;
    assert!((x - (-720.0 - 180.0)).abs() < 0.5, "{x}");
    v.pan_hold(1, 0, false);
    v.tick(2000, C720);
    assert_eq!(v.shot_rect(C720).unwrap().0, x);
}

#[test]
fn a_narrower_shot_stays_centred_on_its_bar_axis() {
    let mut v = full(640, 480);
    v.zoom_in(C720);
    let (x, _, w, _) = v.shot_rect(C720).unwrap();
    assert_eq!(w, 1280.0);
    v.pan_tap(-1, 0, C720);
    assert_eq!(v.shot_rect(C720).unwrap().0, (x + 24.0).min(0.0));
    let mut v = full(640, 480);
    assert_eq!(v.shot_rect(C720), Some((40.0, 0.0, 640.0, 480.0)));
    v.pan_tap(-1, 0, C720);
    assert_eq!(v.shot_rect(C720).unwrap().0, 40.0);
}

#[test]
fn the_view_fraction_follows_the_zoom() {
    let mut v = full(720, 480);
    assert_eq!(v.view_frac(C720), Some((0.0, 0.0, 1.0, 1.0)));
    v.zoom_in(C720);
    assert_eq!(v.view_frac(C720), Some((0.25, 0.25, 0.5, 0.5)));
}

#[test]
fn stepping_resets_the_zoom() {
    let mut v = full_of(3, 720, 480);
    v.zoom_in(C720);
    v.zoom_out(C720);
    v.zoom_in(C720);
    v.step(1);
    assert_eq!(v.zoom(), 1);
    assert_eq!(v.mode(), ShotMode::Full);
}

use slot_ui::{thumbnail, Draw, Gauge, Printed, TexId};

fn drawn(v: &ShotViewer, canvas: Canvas) -> Vec<Draw> {
    let mut out = Vec::new();
    v.draw(canvas, Gauge::default(), None, Printed::default(), &mut out);
    out
}

fn tex_at(out: &[Draw], tex: TexId) -> Option<(f32, f32, f32, f32)> {
    out.iter().find_map(|d| match *d {
        Draw::Tex {
            x, y, w, h, tex: t, ..
        } if t == tex => Some((x, y, w, h)),
        _ => None,
    })
}

fn near(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    (a.0 - b.0).abs() < 0.5
        && (a.1 - b.1).abs() < 0.5
        && (a.2 - b.2).abs() < 0.5
        && (a.3 - b.3).abs() < 0.5
}

fn thumbed(n: usize) -> ShotViewer {
    let mut v = viewer(n);
    for i in 0..n {
        let path = v.entries[i].path.clone();
        v.set_thumb(&path, Some((TexId::from_raw(100 + i), 218, 145)));
    }
    v
}

fn white_rects(out: &[Draw]) -> Vec<(f32, f32, f32, f32)> {
    out.iter()
        .filter_map(|d| match *d {
            Draw::Rect {
                x,
                y,
                w,
                h,
                colour: [1.0, 1.0, 1.0, 1.0],
            } => Some((x, y, w, h)),
            _ => None,
        })
        .collect()
}

#[test]
fn grid_cells_sit_where_the_mockup_has_them() {
    let v = thumbed(7);
    let out = drawn(&v, C720);
    let (cw, ch) = grid_cell(C720);
    let at = |i: usize| tex_at(&out, TexId::from_raw(100 + i)).expect("thumb not drawn");
    let (x, y, w, h) = at(0);
    assert!((x + w / 2.0 - (16.0 + cw / 2.0)).abs() < 0.5, "{x} {w}");
    assert!((y + h / 2.0 - (56.0 + ch / 2.0)).abs() < 0.5, "{y} {h}");
    let (x, y, w, h) = at(4);
    assert!((x + w / 2.0 - (32.0 + cw * 1.5)).abs() < 0.5, "{x}");
    assert!((y + h / 2.0 - (72.0 + ch * 1.5)).abs() < 0.5, "{y}");
    let (.., w, h) = at(0);
    assert!(
        w <= cw + 0.01 && h <= ch + 0.01 && (w - 218.0).abs() < 1.0,
        "{w} {h}"
    );
}

#[test]
fn the_selected_cell_has_a_white_outline() {
    let mut v = thumbed(4);
    v.index = 1;
    let out = drawn(&v, C720);
    let (cw, ch) = grid_cell(C720);
    let cell_x = 32.0 + cw;
    let rects = white_rects(&out);
    assert_eq!(rects.len(), 4, "{rects:?}");
    let left = rects.iter().map(|r| r.0).fold(f32::MAX, f32::min);
    let right = rects.iter().map(|r| r.0 + r.2).fold(f32::MIN, f32::max);
    let top = rects.iter().map(|r| r.1).fold(f32::MAX, f32::min);
    let bottom = rects.iter().map(|r| r.1 + r.3).fold(f32::MIN, f32::max);
    assert!(near(
        (left, top, right, bottom),
        (cell_x - 5.0, 51.0, cell_x + cw + 5.0, 56.0 + ch + 5.0)
    ));
}

#[test]
fn the_scrolled_grid_starts_at_top_row() {
    let mut v = thumbed(12);
    v.grid_move(0, 1, C720);
    v.grid_move(0, 1, C720);
    let out = drawn(&v, C720);
    let (_, ch) = grid_cell(C720);
    let (_, y, _, h) = tex_at(&out, TexId::from_raw(103)).expect("row 1 not drawn");
    assert!((y + h / 2.0 - (56.0 + ch / 2.0)).abs() < 0.5, "{y}");
    assert!(
        tex_at(&out, TexId::from_raw(100)).is_none(),
        "row 0 still drawn"
    );
}

#[test]
fn a_wider_shot_is_fitted_with_bars() {
    let c640 = Canvas { w: 640.0, h: 480.0 };
    let tex = TexId::from_raw(5);
    let mut v = viewer(1);
    v.open();
    v.set_face(Some((tex, 720, 480)));
    assert!(near(
        tex_at(&drawn(&v, c640), tex).unwrap(),
        (0.0, 26.67, 640.0, 426.67)
    ));
    v.set_face(Some((tex, 640, 480)));
    assert!(near(
        tex_at(&drawn(&v, c640), tex).unwrap(),
        (0.0, 0.0, 640.0, 480.0)
    ));
}

#[test]
fn before_the_shot_decodes_full_screen_shows_its_thumb() {
    let mut v = viewer(1);
    let path = v.entries[0].path.clone();
    v.set_thumb(&path, Some((TexId::from_raw(9), 216, 144)));
    v.open();
    assert!(near(
        tex_at(&drawn(&v, C720), TexId::from_raw(9)).unwrap(),
        (0.0, 0.0, 720.0, 480.0)
    ));
}

#[test]
fn zoomed_shows_the_position_box_not_the_count() {
    let mut v = full(720, 480);
    v.set_count_face(Some((TexId::from_raw(11), 90)));
    let out = drawn(&v, C720);
    let (x, ..) = tex_at(&out, TexId::from_raw(11)).expect("no count in full screen");
    assert!((x + 90.0 - 704.0).abs() < 0.5, "{x}");
    v.zoom_in(C720);
    let out = drawn(&v, C720);
    assert!(tex_at(&out, TexId::from_raw(11)).is_none());
    let boxed = out.iter().any(|d| matches!(*d, Draw::Rect { x, w, h, .. } if (x - 656.0).abs() < 0.5 && w == 48.0 && h == 32.0));
    assert!(boxed, "no 48x32 position box at x 656");
}

#[test]
fn the_zoomed_legend_drops_zoom_at_four() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    assert_eq!(labels(&v), ["Pan", "Zoom", "Zoom Out"]);
    v.zoom_in(C720);
    v.zoom_in(C720);
    assert_eq!(labels(&v), ["Pan", "Zoom Out"]);
}

#[test]
fn thumbnail_fits_whole_and_averages() {
    let px = [255u8, 0, 0, 255, 0, 0, 255, 255].repeat(2);
    let (t, w, h) = thumbnail(&px, 2, 2, 1, 1);
    assert_eq!((w, h), (1, 1));
    assert_eq!(&t[..3], &[128, 0, 128]);
    let (_, w, h) = thumbnail(&vec![0; 720 * 480 * 4], 720, 480, 218, 145);
    assert_eq!((w, h), (217, 145));
    let (_, w, h) = thumbnail(&[0; 4 * 4 * 4], 4, 4, 218, 145);
    assert_eq!((w, h), (4, 4));
}

#[test]
fn the_pan_hint_is_drawn_at_its_arrows_width() {
    let mut v = full(720, 480);
    v.set_hint_faces((0..9).map(|i| TexId::from_raw(200 + i)).collect());
    v.zoom_in(C720);
    let out = drawn(&v, C720);
    let (.., w, _) = tex_at(&out, TexId::from_raw(204)).expect("no pan hint");
    assert_eq!(w, slot_ui::pad_hint_width("Pan") as f32);
}

#[test]
fn rolling_between_two_held_directions_keeps_panning() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    v.zoom_in(C720);
    v.pan_hold(1, 0, true);
    v.pan_hold(0, -1, true);
    v.pan_hold(0, -1, false);
    v.tick(1000, C720);
    v.tick(1100, C720);
    let x = v.shot_rect(C720).unwrap().0;
    assert!(x < -720.0, "the right pan stopped: {x}");
}

#[test]
fn holding_vertically_crosses_the_panel_height_in_a_second() {
    let mut v = full(720, 480);
    v.zoom_in(C720);
    v.zoom_in(C720);
    v.pan_hold(0, 1, true);
    v.tick(1000, C720);
    v.tick(1250, C720);
    let y = v.shot_rect(C720).unwrap().1;
    assert!((y - (-480.0 - 120.0)).abs() < 0.5, "{y}");
}

#[test]
fn select_toggles_multi_select_in_the_grid_only() {
    let mut v = viewer(5);
    v.toggle_selecting();
    assert!(v.selecting());
    v.toggle_selecting();
    assert!(!v.selecting());
    v.open();
    v.toggle_selecting();
    assert!(!v.selecting());
}

#[test]
fn a_ticks_and_unticks_and_the_count_says_how_many() {
    let mut v = viewer(5);
    v.toggle_selecting();
    v.tick_selected();
    v.grid_move(1, 0, C720);
    v.tick_selected();
    assert_eq!(v.ticked_count(), 2);
    assert_eq!(v.count_text(), "2 selected");
    v.tick_selected();
    assert_eq!(v.ticked_count(), 1);
    assert!(v.is_ticked(0) && !v.is_ticked(1));
}

#[test]
fn leaving_multi_select_clears_the_ticks() {
    let mut v = viewer(3);
    v.toggle_selecting();
    v.tick_selected();
    v.toggle_selecting();
    assert_eq!(v.ticked_count(), 0);
    assert_eq!(v.count_text(), "1 of 3");
}

#[test]
fn removing_the_ticked_keeps_the_rest_and_leaves_multi_select() {
    let mut v = viewer(7);
    let keep: Vec<String> = [0, 2, 3, 6]
        .iter()
        .map(|&i| v.entries[i].stamp.clone())
        .collect();
    v.toggle_selecting();
    for i in [1, 4, 5] {
        v.index = i;
        v.tick_selected();
    }
    let gone = v.remove_ticked(C720);
    assert_eq!(gone.len(), 3);
    assert_eq!(
        v.entries
            .iter()
            .map(|e| e.stamp.clone())
            .collect::<Vec<_>>(),
        keep
    );
    assert!(!v.selecting());
    assert_eq!(v.index, 1);
    assert_eq!(v.ticked_count(), 0);
}

#[test]
fn the_multi_select_legend() {
    let mut v = viewer(3);
    assert_eq!(labels(&v), ["View", "Back", "Delete", "Multi"]);
    v.toggle_selecting();
    assert_eq!(labels(&v), ["Tick", "Delete", "Done"]);
}

#[test]
fn ticked_cells_wear_the_tick_badge() {
    let mut v = thumbed(4);
    v.set_tick_face(Some((TexId::from_raw(77), 20, 20)));
    v.toggle_selecting();
    v.grid_move(1, 0, C720);
    v.tick_selected();
    let out = drawn(&v, C720);
    let badges: Vec<_> = out
        .iter()
        .filter(|d| matches!(d, Draw::Tex { tex, .. } if *tex == TexId::from_raw(77)))
        .collect();
    assert_eq!(badges.len(), 1);
    let (x, y, ..) = tex_at(&out, TexId::from_raw(77)).unwrap();
    let (cw, _) = grid_cell(C720);
    assert!(
        x > 32.0 + cw && x < 32.0 + 2.0 * cw && (56.0..90.0).contains(&y),
        "{x} {y}"
    );
}

#[test]
fn the_thumb_window_is_the_visible_rows_and_one_either_side() {
    let mut v = viewer(30);
    assert_eq!(v.thumb_window(C720), 0..12);
    for _ in 0..5 {
        v.grid_move(0, 1, C720);
    }
    assert_eq!(v.top_row(), 4);
    assert_eq!(v.thumb_window(C720), 9..24);
    let v = viewer(4);
    assert_eq!(v.thumb_window(C720), 0..4);
}

#[test]
fn thumbs_load_on_screen_rows_first() {
    let mut v = viewer(30);
    for _ in 0..5 {
        v.grid_move(0, 1, C720);
    }
    let order = v.thumb_order(C720);
    assert_eq!(order.len(), 15);
    assert_eq!(&order[..9], &[12, 13, 14, 15, 16, 17, 18, 19, 20]);
    let mut rest = order[9..].to_vec();
    rest.sort();
    assert_eq!(rest, [9, 10, 11, 21, 22, 23]);
}

#[test]
fn thumbs_follow_their_shots_through_a_mass_delete() {
    let mut v = thumbed(6);
    v.toggle_selecting();
    for i in [0, 2, 4] {
        v.index = i;
        v.tick_selected();
    }
    v.remove_ticked(C720);
    let kept: Vec<_> = (0..3).map(|i| v.thumb(i).map(|t| t.0)).collect();
    assert_eq!(kept, [101, 103, 105].map(|i| Some(TexId::from_raw(i))));
}

#[test]
fn deleting_uses_the_panel_it_is_given_to_scroll() {
    let tall = Canvas { w: 720.0, h: 960.0 };
    let mut v = viewer(30);
    v.index = 21;
    v.remove_selected(tall);
    assert_eq!(v.top_row(), 7 + 1 - grid_rows_shown(tall));
}
