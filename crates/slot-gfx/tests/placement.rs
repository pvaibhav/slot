use slot_gfx::{framed, game_layout, screen_rect, GameLayout, Sizing, OUT_H, OUT_W};

fn px(window: (u32, u32), l: GameLayout) -> (i32, i32, i32, i32) {
    let area = match l.panel || !framed(window) {
        true if framed(window) => window,
        true => (OUT_W, OUT_H),
        false => (window.0, screen_rect(window).3 as u32),
    };
    let (sx, sy) = (area.0 as f32 / OUT_W as f32, area.1 as f32 / OUT_H as f32);
    (
        (l.rect[0] * sx).round() as i32,
        (l.rect[1] * sy).round() as i32,
        (l.rect[2] * sx).round() as i32,
        (l.rect[3] * sy).round() as i32,
    )
}

#[test]
fn integer_picks_the_largest_whole_multiple_and_takes_the_strip_only_when_it_must() {
    let gb = game_layout((720, 480), (160, 144), Sizing::Integer);
    assert!(!gb.panel);
    assert_eq!(px((720, 480), gb), (120, 24, 480, 432));
    let gb = game_layout((640, 480), (160, 144), Sizing::Integer);
    assert!(gb.panel);
    assert_eq!(px((640, 480), gb), (80, 24, 480, 432));
    let gba = game_layout((640, 480), (240, 160), Sizing::Integer);
    assert!(!gba.panel);
    assert_eq!(px((640, 480), gba), (80, 53, 480, 320));
}

#[test]
fn band_and_panel_fill_their_area() {
    for window in [(720, 480), (640, 480)] {
        assert_eq!(
            game_layout(window, (160, 144), Sizing::Band),
            GameLayout::BAND
        );
    }
    assert_eq!(
        game_layout((720, 480), (160, 144), Sizing::Panel),
        GameLayout::BAND
    );
    assert_eq!(
        game_layout((640, 480), (160, 144), Sizing::Panel),
        GameLayout {
            panel: true,
            ..GameLayout::BAND
        }
    );
}

#[test]
fn fit_is_the_largest_uncropped_picture() {
    let wide = game_layout((720, 480), (160, 144), Sizing::Fit);
    assert!(!wide.panel);
    assert_eq!(px((720, 480), wide), (93, 0, 533, 480));
    let square = game_layout((640, 480), (160, 144), Sizing::Fit);
    assert!(square.panel);
    assert_eq!(px((640, 480), square), (53, 0, 533, 480));
    for (window, l) in [((720, 480), wide), ((640, 480), square)] {
        let (_, _, w, h) = px(window, l);
        assert!((w as f32 - h as f32 * 160.0 / 144.0).abs() <= 1.0);
    }
}

const GBA: (u32, u32) = (240, 160);
const GB: (u32, u32) = (160, 144);
const ALL: [slot_gfx::Sizing; 4] = [
    slot_gfx::Sizing::Band,
    slot_gfx::Sizing::Integer,
    slot_gfx::Sizing::Fit,
    slot_gfx::Sizing::Panel,
];

fn row(canvas: (u32, u32), picture: (u32, u32)) -> Vec<Option<[f32; 4]>> {
    ALL.iter()
        .map(|s| slot_gfx::table_placement(canvas, picture, *s))
        .collect()
}

#[test]
fn gba_on_the_square_panel_is_3x_with_bars_above_and_below() {
    assert_eq!(row((720, 720), GBA), vec![Some([0.0, 120.0, 720.0, 480.0]); 4]);
}

#[test]
fn gba_on_the_ds_plus_is_4x_for_every_setting() {
    assert_eq!(row((1024, 768), GBA), vec![Some([32.0, 64.0, 960.0, 640.0]); 4]);
}

#[test]
fn game_boy_on_the_square_panel_uses_the_chosen_values() {
    assert_eq!(
        row((720, 720), GB),
        vec![
            Some([0.0, 120.0, 720.0, 480.0]),
            Some([40.0, 72.0, 640.0, 576.0]),
            Some([0.0, 36.0, 720.0, 648.0]),
            Some([0.0, 0.0, 720.0, 720.0]),
        ]
    );
}

#[test]
fn game_boy_on_the_ds_plus_uses_the_chosen_values() {
    assert_eq!(
        row((1024, 768), GB),
        vec![
            Some([32.0, 64.0, 960.0, 640.0]),
            Some([112.0, 24.0, 800.0, 720.0]),
            Some([85.0, 0.0, 853.0, 768.0]),
            Some([0.0, 0.0, 1024.0, 768.0]),
        ]
    );
}

#[test]
fn the_reference_panels_keep_their_own_path() {
    for canvas in [(720, 480), (640, 480)] {
        for picture in [GBA, GB] {
            assert_eq!(row(canvas, picture), vec![None; 4], "{canvas:?} {picture:?}");
        }
    }
}

fn canvas_px(window: (u32, u32), l: GameLayout) -> [f32; 4] {
    let c = slot_gfx::Canvas::of(window);
    let (sx, sy) = (c.w / OUT_W as f32, c.h / OUT_H as f32);
    let [x, y, w, h] = l.rect;
    [x * sx, y * sy, w * sx, h * sy].map(|v| (v * 100.0).round() / 100.0)
}

#[test]
fn a_2x_host_of_a_4_by_3_panel_lays_out_like_the_panel() {
    for sizing in ALL {
        assert_eq!(
            game_layout((1280, 960), GBA, sizing),
            game_layout((640, 480), GBA, sizing),
            "{sizing:?}"
        );
    }
}

#[test]
fn an_unlisted_canvas_keeps_the_picture_shape_at_a_whole_scale() {
    let at = |sizing| canvas_px((1000, 700), game_layout((1000, 700), GBA, sizing));
    assert_eq!(at(Sizing::Integer), [20.0, 30.0, 960.0, 640.0]);
    assert_eq!(at(Sizing::Band), [20.0, 30.0, 960.0, 640.0]);
    assert_eq!(at(Sizing::Panel), [0.0, 0.0, 1000.0, 700.0]);
    let [_, _, w, h] = at(Sizing::Fit);
    assert!((w / h - 1.5).abs() < 0.01, "fit changed the shape: {w}x{h}");
}
