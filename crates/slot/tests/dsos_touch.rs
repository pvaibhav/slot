use slot::input::evdev::Ev;
use slot::input::touch::{TouchMap, TouchTracker};

const EV_SYN: u16 = 0;
const EV_ABS: u16 = 3;
const ABS_MT_SLOT: u16 = 0x2f;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const ABS_MT_TRACKING_ID: u16 = 0x39;

fn ev(kind: u16, code: u16, value: i32) -> Ev {
    Ev { kind, code, value }
}

fn map(max_x: i32, max_y: i32) -> TouchMap {
    TouchMap {
        max_x,
        max_y,
        swap_xy: false,
        invert_x: false,
        invert_y: false,
    }
}

#[test]
fn corners_map_to_ds_corners() {
    let m = map(1023, 767);
    assert_eq!(m.to_ds(0, 0), (0, 0));
    assert_eq!(m.to_ds(1023, 767), (255, 191));
    assert_eq!(m.to_ds(512, 384), (128, 96));
}

#[test]
fn out_of_range_clamps() {
    assert_eq!(map(1023, 767).to_ds(-5, 900), (0, 191));
}

#[test]
fn swapped_axes_are_honoured() {
    let m = TouchMap {
        swap_xy: true,
        ..map(767, 1023)
    };
    assert_eq!(m.to_ds(0, 1023), (255, 0));
    assert_eq!(m.to_ds(767, 0), (0, 191));
}

#[test]
fn inverted_axes_are_honoured() {
    let m = TouchMap {
        invert_x: true,
        invert_y: true,
        ..map(1023, 767)
    };
    assert_eq!(m.to_ds(0, 0), (255, 191));
}

#[test]
fn a_contact_is_reported_at_sync_and_a_lift_clears_it() {
    let mut t = TouchTracker::default();
    assert_eq!(t.feed(ev(EV_ABS, ABS_MT_SLOT, 0)), None);
    t.feed(ev(EV_ABS, ABS_MT_TRACKING_ID, 7));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_X, 100));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_Y, 200));
    assert_eq!(t.feed(ev(EV_SYN, 0, 0)), Some(Some((100, 200))));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_X, 110));
    assert_eq!(t.feed(ev(EV_SYN, 0, 0)), Some(Some((110, 200))));
    t.feed(ev(EV_ABS, ABS_MT_TRACKING_ID, -1));
    assert_eq!(t.feed(ev(EV_SYN, 0, 0)), Some(None));
}

#[test]
fn only_the_first_finger_counts() {
    let mut t = TouchTracker::default();
    t.feed(ev(EV_ABS, ABS_MT_SLOT, 0));
    t.feed(ev(EV_ABS, ABS_MT_TRACKING_ID, 1));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_X, 10));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_Y, 20));
    t.feed(ev(EV_ABS, ABS_MT_SLOT, 1));
    t.feed(ev(EV_ABS, ABS_MT_TRACKING_ID, 2));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_X, 500));
    t.feed(ev(EV_ABS, ABS_MT_POSITION_Y, 600));
    assert_eq!(t.feed(ev(EV_SYN, 0, 0)), Some(Some((10, 20))));
    t.feed(ev(EV_ABS, ABS_MT_TRACKING_ID, -1));
    assert_eq!(t.feed(ev(EV_SYN, 0, 0)), Some(Some((10, 20))));
}
