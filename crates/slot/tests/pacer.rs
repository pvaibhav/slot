use std::time::Duration;

use slot::pacer::Pacer;

#[test]
fn a_learning_pacer_follows_blocked_swaps() {
    let mut p = Pacer::new();
    let before = p.period();
    for _ in 0..200 {
        p.swapped_after(Duration::from_millis(3), true, Duration::from_millis(20));
    }
    assert!(p.period() > before + Duration::from_millis(2), "{:?}", p.period());
}

#[test]
fn a_fixed_pacer_keeps_the_panel_period_whatever_the_swaps_do() {
    let panel = Duration::from_nanos(16_683_000);
    let mut p = Pacer::fixed(panel);
    for _ in 0..200 {
        p.swapped_after(Duration::from_millis(3), true, Duration::from_millis(20));
    }
    assert_eq!(p.period(), panel);
}

#[test]
fn the_mode_period_comes_from_the_pixel_clock() {
    assert_eq!(
        slot_gfx::mode_period(65_000, 1344, 806),
        Some(Duration::from_nanos(16_665_600))
    );
    assert_eq!(slot_gfx::mode_period(0, 1344, 806), None);
}
