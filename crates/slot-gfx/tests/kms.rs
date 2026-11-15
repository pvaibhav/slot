use std::time::Duration;

use slot_gfx::{free_crtc, preferred_mode, retry_until, Panel, CONNECTOR_DSI, MODE_TYPE_PREFERRED};

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

#[test]
fn waits_for_the_boot_logo_to_let_go_of_the_display() {
    let mut calls = 0;
    let got = retry_until(
        || {
            calls += 1;
            calls == 3
        },
        10,
        Duration::ZERO,
    );
    assert!(got);
    assert_eq!(calls, 3);
}

#[test]
fn gives_up_on_the_display_after_the_last_try() {
    let mut calls = 0;
    let got = retry_until(
        || {
            calls += 1;
            false
        },
        4,
        Duration::ZERO,
    );
    assert!(!got);
    assert_eq!(calls, 4);
}

#[test]
fn panels_by_index_are_top_then_bottom() {
    assert_eq!(Panel::of_index(0), Some(Panel::Top));
    assert_eq!(Panel::of_index(1), Some(Panel::Bottom));
    assert_eq!(Panel::of_index(2), None);
}

use slot_gfx::{atomic_plan, primary_plane_for, AtomicIds, PlaneInfo, PLANE_TYPE_PRIMARY};

#[test]
fn each_crtc_gets_its_own_primary_plane() {
    let planes = [
        PlaneInfo {
            id: 30,
            possible_crtcs: 0b01,
            kind: 0,
        },
        PlaneInfo {
            id: 31,
            possible_crtcs: 0b01,
            kind: PLANE_TYPE_PRIMARY,
        },
        PlaneInfo {
            id: 40,
            possible_crtcs: 0b10,
            kind: PLANE_TYPE_PRIMARY,
        },
        PlaneInfo {
            id: 41,
            possible_crtcs: 0b11,
            kind: 2,
        },
    ];
    assert_eq!(primary_plane_for(&planes, 0, &[]), Some(31));
    assert_eq!(primary_plane_for(&planes, 1, &[]), Some(40));
    assert_eq!(primary_plane_for(&planes, 1, &[40]), None);
}

fn ids(plane: u32, crtc: u32, connector: u32) -> AtomicIds {
    AtomicIds {
        plane,
        crtc,
        connector,
        plane_fb_id: plane * 100 + 1,
        plane_crtc_id: plane * 100 + 2,
        plane_src: [
            plane * 100 + 3,
            plane * 100 + 4,
            plane * 100 + 5,
            plane * 100 + 6,
        ],
        plane_dst: [
            plane * 100 + 7,
            plane * 100 + 8,
            plane * 100 + 9,
            plane * 100 + 10,
        ],
        crtc_mode_id: crtc * 100 + 1,
        crtc_active: crtc * 100 + 2,
        connector_crtc_id: connector * 100 + 1,
    }
}

#[test]
fn a_later_commit_only_changes_both_framebuffers() {
    let outs = [
        (ids(31, 71, 76), 501, (1024, 768), 0),
        (ids(40, 72, 74), 502, (1024, 768), 0),
    ];
    let plan = atomic_plan(&outs, false);
    assert_eq!(plan, vec![(31, 3101, 501), (40, 4001, 502)]);
}

#[test]
fn the_first_commit_sets_the_mode_and_wires_each_panel() {
    let outs = [(ids(31, 71, 76), 501, (1024, 768), 900)];
    let plan = atomic_plan(&outs, true);
    assert!(plan.contains(&(76, 7601, 71)), "connector to crtc");
    assert!(plan.contains(&(71, 7101, 900)), "mode blob");
    assert!(plan.contains(&(71, 7102, 1)), "crtc active");
    assert!(plan.contains(&(31, 3101, 501)), "framebuffer");
    assert!(plan.contains(&(31, 3102, 71)), "plane on crtc");
    assert!(
        plan.contains(&(31, 3105, 1024 << 16)),
        "source width in 16.16"
    );
    assert!(plan.contains(&(31, 3110, 768)), "destination height");
}

#[test]
fn a_panel_with_no_flip_in_flight_flips_a_new_frame_at_once() {
    let mut q = slot_gfx::FlipQueue::default();
    let step = q.submit(1);
    assert_eq!((step.flip, step.release), (Some(1), None));
}

#[test]
fn a_frame_drawn_while_a_flip_is_in_flight_waits_without_blocking() {
    let mut q = slot_gfx::FlipQueue::default();
    q.submit(1);
    let step = q.submit(2);
    assert_eq!((step.flip, step.release), (None, None));
}

#[test]
fn a_newer_waiting_frame_replaces_the_older_one() {
    let mut q = slot_gfx::FlipQueue::default();
    q.submit(1);
    q.submit(2);
    let step = q.submit(3);
    assert_eq!((step.flip, step.release), (None, Some(2)));
    let step = q.retire();
    assert_eq!(step.flip, Some(3));
}

#[test]
fn a_retired_flip_frees_the_old_frame_and_flips_the_waiting_one() {
    let mut q = slot_gfx::FlipQueue::default();
    q.show(0);
    q.submit(1);
    q.submit(2);
    let step = q.retire();
    assert_eq!((step.flip, step.release), (Some(2), Some(0)));
    let step = q.retire();
    assert_eq!((step.flip, step.release), (None, Some(1)));
    assert_eq!(q.submit(3).flip, Some(3));
}

#[test]
fn a_failed_flip_hands_its_frame_back() {
    let mut q = slot_gfx::FlipQueue::default();
    q.submit(1);
    assert_eq!(q.flip_failed(), Some(1));
    assert_eq!(q.submit(2).flip, Some(2));
}
