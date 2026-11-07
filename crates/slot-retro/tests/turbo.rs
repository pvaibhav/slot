use slot_retro::ButtonMask;

fn pressed(mask: u16, frames: std::ops::Range<u32>) -> Vec<u16> {
    frames.map(|f| ButtonMask(mask).turbo(f).0).collect()
}

#[test]
fn x_pulses_a_three_frames_on_three_off() {
    let a = ButtonMask::A;
    assert_eq!(
        pressed(ButtonMask::X, 0..12),
        vec![a, a, a, 0, 0, 0, a, a, a, 0, 0, 0]
    );
}

#[test]
fn y_pulses_b() {
    let b = ButtonMask::B;
    assert_eq!(pressed(ButtonMask::Y, 0..6), vec![b, b, b, 0, 0, 0]);
}

#[test]
fn held_a_stays_down_under_turbo() {
    let both = ButtonMask::A | ButtonMask::X;
    assert_eq!(pressed(both, 0..6), vec![ButtonMask::A; 6]);
}

#[test]
fn other_buttons_pass_through_and_x_y_never_reach_the_core() {
    let held = ButtonMask::X | ButtonMask::Y | ButtonMask::RIGHT;
    for f in 0..6 {
        let m = ButtonMask(held).turbo(f).0;
        assert_ne!(m & ButtonMask::RIGHT, 0);
        assert_eq!(m & (ButtonMask::X | ButtonMask::Y), 0);
    }
}

#[test]
fn a_core_that_takes_xy_gets_x_and_y_untouched() {
    let held = ButtonMask::X | ButtonMask::Y | ButtonMask::L;
    for f in 0..6 {
        assert_eq!(slot_retro::mask_for(true, ButtonMask(held), f).0, held);
    }
}

#[test]
fn other_cores_still_get_turbo_from_mask_for() {
    for f in 0..6 {
        assert_eq!(
            slot_retro::mask_for(false, ButtonMask(ButtonMask::X), f),
            ButtonMask(ButtonMask::X).turbo(f)
        );
    }
}
