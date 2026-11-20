use slot::core_picker::{CorePicker, Outcome, Press, CLOSE_MS, HOP_MS, LIFT_MS, OPEN_MS, SLIDE_MS};
use slot_store::{Core, Platform};
use slot_ui::Millis;

const PAIR: &[Core] = &[Core::Mgba, Core::Gpsp];
const GB_PAIR: &[Core] = &[Core::Mgba, Core::Gambatte];
const THREE: &[Core] = &[Core::Mgba, Core::Armgba, Core::Gpsp];

fn opened(seat: Core, at: Millis) -> CorePicker {
    opened_on(PAIR, seat, at)
}

fn opened_on(row: &'static [Core], seat: Core, at: Millis) -> CorePicker {
    let mut p = CorePicker::open(row, seat, at);
    p.start(at);
    p
}

#[test]
fn it_opens_over_the_open_time_and_then_rests() {
    let p = opened(Core::Mgba, 1000);
    assert_eq!(p.openness(1000), 0.0);
    assert_eq!(p.openness(1000 + OPEN_MS / 4), 0.25);
    assert_eq!(p.openness(1000 + OPEN_MS), 1.0);
    assert_eq!(p.openness(1000 + 10 * OPEN_MS), 1.0);
}

#[test]
fn the_chip_starts_seated_in_the_carts_own_core() {
    let chip = opened(Core::Gpsp, 0).chip(0);
    assert_eq!(chip.seated, Some(Core::Gpsp));
    assert_eq!((chip.across, chip.lift, chip.tip), (1.0, 0.0, 0.0));
}

#[test]
fn right_from_mgba_hops_blank_and_lands_named_in_gpsp() {
    let mut p = opened(Core::Mgba, 0);
    assert_eq!(p.press(Press::Right, 500), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Gpsp);

    let mid = p.chip(500 + HOP_MS / 2);
    assert_eq!(mid.seated, None, "the chip wears a name in flight");
    assert!((mid.across - 0.5).abs() < 0.01, "across {}", mid.across);
    assert!((mid.lift - 1.0).abs() < 0.01, "lift {}", mid.lift);
    assert!(mid.tip > 0.0, "a chip moving right should lean right");

    let landed = p.chip(500 + HOP_MS);
    assert_eq!(landed.seated, Some(Core::Gpsp));
    assert_eq!((landed.across, landed.lift), (1.0, 0.0));
}

#[test]
fn toward_the_socket_it_is_in_is_refused_and_only_shakes() {
    let mut p = opened(Core::Gpsp, 0);
    assert_eq!(p.press(Press::Right, 400), Outcome::Refused);
    assert_eq!(p.seat(), Core::Gpsp);
    assert_ne!(p.chip(400).shake, 0.0, "a refusal with no shake");
    assert_eq!(p.chip(400).seated, Some(Core::Gpsp));
    assert_eq!(p.chip(700).shake, 0.0, "the shake outlived its 300 ms");
}

#[test]
fn back_mid_hop_turns_the_chip_round_from_where_it_is() {
    let mut p = opened(Core::Mgba, 0);
    p.press(Press::Right, 400);
    let at = 400 + HOP_MS / 4;
    let before = p.chip(at).across;
    assert_eq!(p.press(Press::Left, at), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Mgba);
    assert!(
        (p.chip(at).across - before).abs() < 0.02,
        "the chip jumped from {before} to {} as it turned",
        p.chip(at).across
    );
    assert_eq!(p.chip(at + HOP_MS).seated, Some(Core::Mgba));
}

#[test]
fn onward_mid_hop_does_nothing_and_is_not_a_refusal() {
    let mut p = opened(Core::Mgba, 0);
    p.press(Press::Right, 400);
    assert_eq!(p.press(Press::Right, 450), Outcome::Nothing);
    assert_eq!(p.chip(450).shake, 0.0);
    assert_eq!(p.seat(), Core::Gpsp);
}

#[test]
fn keep_writes_where_the_chip_is_heading_and_closes() {
    let mut p = opened(Core::Mgba, 0);
    p.press(Press::Right, 400);
    assert_eq!(p.press(Press::Keep, 450), Outcome::Write(Core::Gpsp));
    assert!(p.closing());
    assert!(!p.finished(450));
    assert!(p.finished(450 + CLOSE_MS));
}

#[test]
fn back_closes_without_a_write() {
    let mut p = opened(Core::Mgba, 0);
    assert_eq!(p.press(Press::Back, 400), Outcome::Nothing);
    assert!(p.closing());
    assert!(p.finished(400 + CLOSE_MS));
}

#[test]
fn a_close_during_the_lift_reverses_from_where_the_lid_had_got_to() {
    let mut p = opened(Core::Mgba, 0);
    let at = OPEN_MS / 2;
    let open = p.openness(at);
    p.press(Press::Back, at);
    assert!(
        (p.openness(at) - open).abs() < 1e-6,
        "the lid snapped as the close began"
    );
    assert!(
        p.finished(at + CLOSE_MS),
        "a partial close outlasted a whole one"
    );
}

#[test]
fn a_hop_started_soon_after_a_refusal_does_not_carry_its_shake() {
    let mut p = opened(Core::Gpsp, 0);
    assert_eq!(p.press(Press::Right, 0), Outcome::Refused);
    assert_eq!(p.press(Press::Left, 100), Outcome::Nothing);
    assert_eq!(p.chip(100).shake, 0.0, "the refusal rode along on the hop");
    assert_eq!(p.chip(150).shake, 0.0, "the refusal rode along on the hop");
}

#[test]
fn presses_during_the_close_do_nothing() {
    let mut p = opened(Core::Mgba, 0);
    p.press(Press::Back, 400);
    assert_eq!(p.press(Press::Right, 420), Outcome::Nothing);
    assert_eq!(p.press(Press::Keep, 430), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Mgba);
}

#[test]
fn the_open_is_a_slide_then_a_lift_and_the_board_agrees() {
    assert_eq!(OPEN_MS, SLIDE_MS + LIFT_MS);
    assert!((slot_ui::SLIDE_SHARE - SLIDE_MS as f32 / OPEN_MS as f32).abs() < 1e-6);
}

#[test]
fn a_picker_waits_until_it_is_started() {
    let mut p = CorePicker::open(PAIR, Core::Mgba, 100);
    assert!(p.waiting());
    assert_eq!(p.openness(5_000), 0.0);
    assert_eq!(p.waited(350), 250);
    p.start(400);
    assert!(!p.waiting());
    assert_eq!(p.openness(400), 0.0);
    assert_eq!(p.openness(400 + OPEN_MS), 1.0);
}

#[test]
fn a_second_start_keeps_the_first() {
    let mut p = CorePicker::open(PAIR, Core::Mgba, 0);
    p.start(300);
    p.start(500);
    assert_eq!(p.openness(300 + OPEN_MS), 1.0);
}

#[test]
fn backing_out_while_waiting_finishes_at_once() {
    let mut p = CorePicker::open(PAIR, Core::Mgba, 0);
    p.press(Press::Back, 50);
    assert!(p.finished(50));
}

#[test]
fn a_close_from_rest_takes_the_close_time() {
    let mut p = opened(Core::Mgba, 0);
    p.press(Press::Back, OPEN_MS);
    assert_eq!(p.openness(OPEN_MS + CLOSE_MS / 4), 0.75);
    assert!(p.finished(OPEN_MS + CLOSE_MS));
}

#[test]
fn the_arrows_do_nothing_while_the_picker_waits() {
    let mut p = CorePicker::open(PAIR, Core::Mgba, 0);
    assert_eq!(p.press(Press::Right, 100), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Mgba, "a hop started while the cart waited");
    let chip = p.chip(100);
    assert_eq!((chip.seated, chip.shake), (Some(Core::Mgba), 0.0));
    assert_eq!(
        p.press(Press::Left, 120),
        Outcome::Nothing,
        "toward its own socket while the cart waited was refused"
    );
    assert_eq!(p.chip(120).shake, 0.0);

    p.start(200);
    assert_eq!(p.press(Press::Right, 300), Outcome::Nothing);
    assert_eq!(
        p.seat(),
        Core::Gpsp,
        "the arrow did nothing once the cart had opened"
    );
    assert_eq!(
        p.chip(300 + HOP_MS / 2).seated,
        None,
        "no hop once the cart had opened"
    );
}

#[test]
fn a_game_boy_row_hops_right_through_armgba_to_gambatte() {
    let mut p = opened_on(Core::row(Platform::Gb), Core::Mgba, 0);
    assert_eq!(p.press(Press::Right, 400), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Armgba);
    let mid = p.chip(400 + HOP_MS / 2);
    assert_eq!(mid.seated, None);
    assert!(mid.tip > 0.0, "a chip moving right should lean right");
    let landed = p.chip(400 + HOP_MS);
    assert_eq!(landed.seated, Some(Core::Armgba));
    assert_eq!((landed.across, landed.lift), (1.0, 0.0));
    assert_eq!(p.press(Press::Right, 400 + HOP_MS), Outcome::Nothing);
    let landed = p.chip(400 + 2 * HOP_MS);
    assert_eq!(landed.seated, Some(Core::Gambatte));
    assert_eq!((landed.across, landed.lift), (2.0, 0.0));
    assert_eq!(
        p.press(Press::Keep, 400 + 2 * HOP_MS),
        Outcome::Write(Core::Gambatte)
    );
}

#[test]
fn a_game_boy_row_refuses_left_from_mgba() {
    let mut p = opened_on(Core::row(Platform::Gbc), Core::Mgba, 0);
    assert_eq!(p.press(Press::Left, 400), Outcome::Refused);
    assert_eq!(p.seat(), Core::Mgba);
    assert_ne!(p.chip(400).shake, 0.0);
}

#[test]
fn a_gba_row_never_seats_gambatte() {
    let mut p = opened_on(Core::row(Platform::Gba), Core::Mgba, 0);
    p.press(Press::Right, 400);
    assert_eq!(p.seat(), Core::Armgba);
    p.press(Press::Right, 400 + HOP_MS);
    assert_eq!(p.seat(), Core::Gpsp);
    assert_eq!(p.press(Press::Right, 400 + 2 * HOP_MS), Outcome::Refused);
    p.press(Press::Left, 400 + 3 * HOP_MS);
    assert_eq!(p.seat(), Core::Armgba);
    assert_eq!(p.chip(400 + 4 * HOP_MS).seated, Some(Core::Armgba));
}

#[test]
fn the_chip_rests_by_place_in_the_row() {
    let gambatte = opened_on(GB_PAIR, Core::Gambatte, 0).chip(0);
    assert_eq!(gambatte.seated, Some(Core::Gambatte));
    assert_eq!(gambatte.across, 1.0);
    assert_eq!(opened_on(GB_PAIR, Core::Mgba, 0).chip(0).across, 0.0);
    assert_eq!(opened_on(PAIR, Core::Gpsp, 0).chip(0).across, 1.0);
}

#[test]
fn back_mid_hop_on_a_game_boy_pair_turns_back_to_mgba() {
    let mut p = opened_on(GB_PAIR, Core::Gambatte, 0);
    p.press(Press::Left, 400);
    let mid = p.chip(400 + HOP_MS / 2);
    assert!(mid.tip < 0.0, "a chip moving left should lean left");
    assert_eq!(p.chip(400 + HOP_MS).seated, Some(Core::Mgba));
    assert_eq!(p.chip(400 + HOP_MS).across, 0.0);
}

#[test]
fn the_chip_rests_on_its_own_seat_in_a_row_of_three() {
    for (seat, across) in [(Core::Mgba, 0.0), (Core::Armgba, 1.0), (Core::Gpsp, 2.0)] {
        let chip = opened_on(THREE, seat, 0).chip(0);
        assert_eq!((chip.seated, chip.across), (Some(seat), across));
    }
}

#[test]
fn from_the_middle_seat_left_and_right_each_hop_one_seat() {
    for (press, lands, mid, rest, lean) in [
        (Press::Left, Core::Mgba, 0.5, 0.0, -1.0),
        (Press::Right, Core::Gpsp, 1.5, 2.0, 1.0),
    ] {
        let mut p = opened_on(THREE, Core::Armgba, 0);
        assert_eq!(p.press(press, 400), Outcome::Nothing);
        assert_eq!(p.seat(), lands);
        let flying = p.chip(400 + HOP_MS / 2);
        assert_eq!(flying.seated, None);
        assert!(
            (flying.across - mid).abs() < 0.01,
            "across {}",
            flying.across
        );
        assert_eq!(flying.tip.signum(), lean);
        let landed = p.chip(400 + HOP_MS);
        assert_eq!((landed.seated, landed.across), (Some(lands), rest));
    }
}

#[test]
fn either_end_of_a_row_of_three_refuses_with_a_shake_and_no_wrap() {
    for (seat, press) in [(Core::Mgba, Press::Left), (Core::Gpsp, Press::Right)] {
        let mut p = opened_on(THREE, seat, 0);
        assert_eq!(p.press(press, 400), Outcome::Refused);
        assert_eq!(p.seat(), seat);
        assert_ne!(p.chip(400).shake, 0.0, "{seat:?} refused without a shake");
        assert_eq!(p.chip(400 + HOP_MS).seated, Some(seat));
    }
}

#[test]
fn a_second_press_mid_hop_hops_on_as_the_chip_lands() {
    let mut p = opened_on(THREE, Core::Mgba, 0);
    p.press(Press::Right, 400);
    assert_eq!(p.press(Press::Right, 450), Outcome::Nothing);
    assert_eq!(p.chip(450).shake, 0.0, "the second press was refused");
    assert_eq!(p.seat(), Core::Gpsp);

    let landing = p.chip(400 + HOP_MS);
    assert_eq!((landing.seated, landing.across), (None, 1.0));
    let onward = p.chip(400 + HOP_MS + HOP_MS / 2);
    assert!(
        (onward.across - 1.5).abs() < 0.01,
        "across {}",
        onward.across
    );
    assert!(onward.tip > 0.0);
    let landed = p.chip(400 + 2 * HOP_MS);
    assert_eq!((landed.seated, landed.across), (Some(Core::Gpsp), 2.0));
}

#[test]
fn a_press_past_the_end_mid_hop_is_dropped_without_a_shake() {
    let mut p = opened_on(THREE, Core::Armgba, 0);
    p.press(Press::Right, 400);
    assert_eq!(p.press(Press::Right, 450), Outcome::Nothing);
    assert_eq!(p.chip(450).shake, 0.0);
    assert_eq!(p.seat(), Core::Gpsp);
    assert_eq!(p.chip(400 + 2 * HOP_MS).seated, Some(Core::Gpsp));
}

#[test]
fn back_mid_hop_in_a_row_of_three_turns_round_without_a_jump() {
    let mut p = opened_on(THREE, Core::Armgba, 0);
    p.press(Press::Right, 400);
    let at = 400 + HOP_MS / 4;
    let before = p.chip(at);
    assert_eq!(p.press(Press::Left, at), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Armgba);
    let after = p.chip(at);
    assert!(
        (after.across - before.across).abs() < 0.02 && (after.lift - before.lift).abs() < 0.02,
        "the chip jumped from {before:?} to {after:?} as it turned"
    );
    assert!(
        p.chip(at + 1).tip < 0.0,
        "the turned chip still leans right"
    );
    let landed = p.chip(at + HOP_MS);
    assert_eq!((landed.seated, landed.across), (Some(Core::Armgba), 1.0));
}

#[test]
fn back_while_a_second_hop_waits_cancels_only_the_second_hop() {
    let mut p = opened_on(THREE, Core::Mgba, 0);
    p.press(Press::Right, 400);
    p.press(Press::Right, 450);
    assert_eq!(p.press(Press::Left, 500), Outcome::Nothing);
    assert_eq!(p.seat(), Core::Armgba);
    assert!(p.chip(500).tip > 0.0, "the chip turned round");
    assert_eq!(p.chip(400 + HOP_MS).seated, Some(Core::Armgba));
    assert_eq!(p.chip(400 + 2 * HOP_MS).seated, Some(Core::Armgba));
}

#[test]
fn keep_while_a_second_hop_waits_writes_the_seat_it_heads_for() {
    let mut p = opened_on(THREE, Core::Mgba, 0);
    p.press(Press::Right, 400);
    p.press(Press::Right, 450);
    assert_eq!(p.press(Press::Keep, 460), Outcome::Write(Core::Gpsp));
}

#[test]
fn a_row_of_one_refuses_both_ways() {
    let mut p = opened_on(&[Core::Mgba], Core::Mgba, 0);
    assert_eq!(p.press(Press::Left, 400), Outcome::Refused);
    assert_eq!(p.press(Press::Right, 800), Outcome::Refused);
    assert_eq!(p.chip(800).across, 0.0);
    assert_eq!(p.press(Press::Keep, 900), Outcome::Write(Core::Mgba));
}
