use std::time::Duration;

use slot::pacer::Pacer;

#[test]
fn a_learning_pacer_follows_blocked_swaps() {
    let mut p = Pacer::new();
    let before = p.period();
    for _ in 0..200 {
        p.swapped_after(Duration::from_millis(3), true, Duration::from_millis(20));
    }
    assert!(
        p.period() > before + Duration::from_millis(2),
        "{:?}",
        p.period()
    );
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

mod locked {
    use std::time::{Duration, Instant};

    use slot::pacer::Pacer;

    const PERIOD: Duration = Duration::from_micros(16_667);
    const WORK: Duration = Duration::from_millis(3);
    const MARGIN: Duration = Duration::from_millis(5);

    fn locked(vblank: Instant) -> Pacer {
        let mut p = Pacer::fixed(PERIOD);
        for _ in 0..64 {
            p.swapped_after(WORK, false, PERIOD);
        }
        p.lock_to(vblank);
        p
    }

    fn ms(n: f64) -> Duration {
        Duration::from_secs_f64(n / 1e3)
    }

    #[test]
    fn it_wakes_so_the_swap_lands_a_margin_before_the_next_vblank() {
        let v = Instant::now();
        let mut p = locked(v);
        assert_eq!(p.plan(v + ms(2.0)), Some(v + PERIOD - WORK - MARGIN));
    }

    #[test]
    fn a_late_loop_that_can_still_make_the_vblank_goes_at_once() {
        let v = Instant::now();
        let mut p = locked(v);
        let now = v + ms(13.0);
        assert_eq!(p.plan(now), Some(now));
    }

    #[test]
    fn a_loop_too_late_for_a_vblank_aims_at_the_one_after() {
        let v = Instant::now();
        let mut p = locked(v);
        assert_eq!(p.plan(v + ms(14.0)), Some(v + PERIOD * 2 - WORK - MARGIN));
    }

    #[test]
    fn it_never_aims_two_frames_at_one_vblank() {
        let v = Instant::now();
        let mut p = locked(v);
        assert_eq!(p.plan(v + ms(2.0)), Some(v + PERIOD - WORK - MARGIN));
        assert_eq!(p.plan(v + ms(12.5)), Some(v + PERIOD * 2 - WORK - MARGIN));
    }

    #[test]
    fn a_vblank_long_past_still_sets_the_phase() {
        let v = Instant::now();
        let mut p = locked(v);
        let now = v + PERIOD * 100 + ms(2.0);
        assert_eq!(p.plan(now), Some(v + PERIOD * 101 - WORK - MARGIN));
    }
}
