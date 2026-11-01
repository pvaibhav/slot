use slot_ui::{net_bars_face, net_lock_face, UndoFace};

fn ink(f: &UndoFace) -> usize {
    f.rgba.chunks(4).filter(|p| p[3] > 128).count()
}

#[test]
fn every_signal_level_and_the_lock_draw_a_glyph() {
    let lock = net_lock_face(true);
    assert!(ink(&lock) > 40, "lock ink {}", ink(&lock));
    let fills: Vec<usize> = (0..4).map(|n| ink(&net_bars_face(n, true))).collect();
    assert!(fills[0] > 10, "outline ink {}", fills[0]);
    assert!(fills.windows(2).all(|w| w[1] > w[0]), "{fills:?}");
}

#[test]
fn the_glyphs_share_one_box() {
    let lock = net_lock_face(false);
    for n in 0..4 {
        let bars = net_bars_face(n, false);
        assert_eq!((bars.w, bars.h), (lock.w, lock.h));
    }
}
