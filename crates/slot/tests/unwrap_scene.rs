use slot::unwrap_scene::{body_width, box_face, cart_rect, rim_y, unwrap_art, UnwrapScene};
use slot_gfx::{Canvas, OUT_H};
use slot_store::Platform;
use slot_ui::{box_size, cart_box};

fn t(ms: u64) -> u64 {
    ms
}

const PLATFORMS: [Platform; 2] = [Platform::Gba, Platform::Gb];

fn canvases() -> [Canvas; 2] {
    [Canvas::of((720, OUT_H)), Canvas::of((640, OUT_H))]
}

#[test]
fn the_timeline_matches_the_table() {
    let s = UnwrapScene::new(0);
    assert!(s.frame(t(260)).shake.abs() > 0.0);
    assert_eq!(s.frame(t(520)).shake, 0.0);
    assert!(s.frame(t(1049)).lid.is_some());
    assert!(s.frame(t(1050)).lid.is_none());
    assert!(s.frame(t(819)).cart.is_none());
    assert!(s.frame(t(820)).cart.is_some());
    assert!(s.frame(t(1000)).box_drop == 0.0);
    assert!(s.frame(t(1760)).glow == 0.0);
    assert_eq!(s.frame(t(760)).glow, 1.0);
    assert_eq!(s.frame(t(1740)).box_alpha, 0.0);
    assert_eq!(s.frame(t(2220)).cart, Some((1.0, 0.0)));
    assert!(!s.done(t(2219)));
    assert!(s.done(t(2220)));
}

#[test]
fn the_scene_runs_from_its_start_time() {
    let s = UnwrapScene::new(5000);
    assert_eq!(s.frame(5000 + 1380), UnwrapScene::new(0).frame(1380));
    assert!(!s.done(5000));
    assert!(s.done(5000 + 2220));
}

#[test]
fn the_box_holds_still_until_the_cart_is_clear() {
    let s = UnwrapScene::new(0);
    for ms in 0..1380 {
        assert_eq!(s.frame(t(ms)).box_drop, 0.0, "{ms}");
    }
    let cart = s.frame(t(1380)).cart.unwrap();
    for p in PLATFORMS {
        for canvas in canvases() {
            let [_, y, _, h] = cart_rect(cart, p, canvas);
            assert!(y + h < rim_y(p), "{p:?}: bottom {} rim {}", y + h, rim_y(p));
            assert!(y > 0.0, "{p:?}: top {y}");
        }
    }
}

#[test]
fn the_cart_stays_on_screen_through_the_overshoot() {
    let s = UnwrapScene::new(0);
    for ms in 820..=2220 {
        let cart = s.frame(t(ms)).cart.unwrap();
        for p in PLATFORMS {
            let [_, y, _, _] = cart_rect(cart, p, canvases()[0]);
            assert!(y >= 0.0, "{p:?} at {ms}: {y}");
        }
    }
}

#[test]
fn the_cart_starts_narrower_than_the_box() {
    let s = UnwrapScene::new(0);
    let start = s.frame(t(820)).cart.unwrap();
    for p in PLATFORMS {
        assert!(cart_box(p).0 as f32 * 0.62 < body_width(p));
        for canvas in canvases() {
            let [x, y, w, _] = cart_rect(start, p, canvas);
            assert!(w < body_width(p), "{p:?}: {w}");
            assert!((x + w / 2.0 - canvas.w / 2.0).abs() < 0.5);
            assert!(y >= rim_y(p), "{p:?}: top {y} above rim {}", rim_y(p));
        }
    }
}

#[test]
fn the_cart_lands_exactly_on_its_shelf_spot() {
    let end = UnwrapScene::new(0).frame(t(2220)).cart.unwrap();
    for p in PLATFORMS {
        let canvas = canvases()[1];
        let (w, h) = cart_box(p);
        let [x, y, rw, rh] = cart_rect(end, p, canvas);
        assert_eq!((rw, rh), (w as f32, h as f32));
        assert_eq!(x, (canvas.w - w as f32) / 2.0);
        assert_eq!(y, slot_ui::rest_y(h as f32));
    }
}

#[test]
fn a_box_face_is_the_same_each_build() {
    let art = unwrap_art().unwrap();
    let a = box_face(&art, Platform::Gba, "Emerald");
    assert_eq!(a, box_face(&art, Platform::Gba, "Emerald"));
    let (w, h) = box_size(Platform::Gb);
    assert_eq!(box_face(&art, Platform::Gb, "Tetris").len(), (w * h * 4) as usize);
}
