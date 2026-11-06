use slot_gfx::{free_crtc, preferred_mode, Panel, CONNECTOR_DSI, MODE_TYPE_PREFERRED};

#[test]
fn preferred_mode_wins_over_first() {
    assert_eq!(preferred_mode(&[0, MODE_TYPE_PREFERRED | 1, 0]), Some(1));
}

#[test]
fn first_mode_without_a_preferred_one() {
    assert_eq!(preferred_mode(&[0, 0]), Some(0));
    assert_eq!(preferred_mode(&[]), None);
}

#[test]
fn keeps_the_crtc_a_panel_is_already_on() {
    assert_eq!(free_crtc(52, 0b11, &[51, 52], &[]), Some(52));
}

#[test]
fn skips_a_crtc_the_other_panel_took() {
    assert_eq!(free_crtc(0, 0b11, &[51, 52], &[51]), Some(52));
    assert_eq!(free_crtc(51, 0b11, &[51, 52], &[51]), Some(52));
}

#[test]
fn only_crtcs_the_encoder_can_drive() {
    assert_eq!(free_crtc(0, 0b10, &[51, 52], &[]), Some(52));
    assert_eq!(free_crtc(0, 0b01, &[51, 52], &[51]), None);
}

#[test]
fn top_is_dsi_2_and_bottom_is_dsi_1() {
    assert_eq!(Panel::Top.connector(), (CONNECTOR_DSI, 2));
    assert_eq!(Panel::Bottom.connector(), (CONNECTOR_DSI, 1));
}
