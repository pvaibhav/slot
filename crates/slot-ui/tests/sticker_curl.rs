use slot_ui::sticker_curl::{strips, Pose};

const RECT: [f32; 4] = [48.0, 50.0, 264.0, 136.0];
const FOCUS: [f32; 2] = [360.0, 240.0];

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}

#[test]
fn flat_is_the_rect() {
    let s = strips(RECT, 1.0, 150.0, &Pose::REST, FOCUS);
    assert_eq!(s.len(), 33);
    let mut x = 48.0;
    for strip in &s {
        let [tl, tr, br, bl] = strip.corners;
        assert!(close(tl[0], x) && close(tl[1], 50.0), "{tl:?}");
        assert!(close(bl[0], x) && close(bl[1], 186.0), "{bl:?}");
        assert!(close(tr[1], 50.0) && close(br[1], 186.0));
        assert!(!strip.back);
        x = tr[0];
    }
    assert!(close(x, 312.0), "{x}");
    assert!(close(s[0].uv[0], 0.0));
    let last = s.last().unwrap();
    assert!(close(last.uv[0] + last.uv[2], 1.0));
}

#[test]
fn rolled_up_is_narrow() {
    let s = strips(RECT, 0.0, 420.0, &Pose::REST, FOCUS);
    let xs: Vec<f32> = s.iter().flat_map(|st| st.corners.iter().map(|c| c[0])).collect();
    let span = xs.iter().cloned().fold(f32::MIN, f32::max) - xs.iter().cloned().fold(f32::MAX, f32::min);
    assert!(span < 100.0, "{span}");
}

#[test]
fn a_full_roll_shows_the_back() {
    let s = strips(RECT, 0.0, 420.0, &Pose::REST, FOCUS);
    assert!(s.iter().any(|st| st.back));
    assert!(!s[0].back);
}

#[test]
fn a_gentle_curl_lifts_toward_the_viewer() {
    let flat = strips(RECT, 1.0, 150.0, &Pose::REST, FOCUS);
    let curled = strips(RECT, 0.5, 150.0, &Pose::REST, FOCUS);
    let tip = |s: &Vec<slot_ui::sticker_curl::Strip>| s.last().unwrap().corners[0][1];
    assert!(tip(&curled) < tip(&flat), "a lifted tip projects above the flat top edge");
}

#[test]
fn pose_moves_everything() {
    let rest = strips(RECT, 1.0, 150.0, &Pose::REST, FOCUS);
    let moved = strips(RECT, 1.0, 150.0, &Pose { x: 640.0, ..Pose::REST }, FOCUS);
    for (a, b) in rest.iter().zip(moved.iter()) {
        for (p, q) in a.corners.iter().zip(b.corners.iter()) {
            assert!(close(q[0] - p[0], 640.0) && close(q[1], p[1]), "{p:?} {q:?}");
        }
    }
}

#[test]
fn strips_come_back_to_front() {
    let s = strips(RECT, 0.0, 420.0, &Pose::REST, FOCUS);
    for pair in s.windows(2) {
        assert!(pair[0].z <= pair[1].z, "{} then {}", pair[0].z, pair[1].z);
    }
}
