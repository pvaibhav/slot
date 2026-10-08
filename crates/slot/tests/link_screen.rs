use slot::app::{GameMenu, LinkRow};
use slot::link_kind::LinkKind;
use slot::link_screen::*;
use slot::link_start::{LinkFail, LinkStep};
use slot_ui::*;

fn working(role: LinkRow, since: u64) -> GameMenu {
    GameMenu::Working {
        role,
        step: LinkStep::Waiting,
        since,
    }
}

fn failed(since: u64) -> GameMenu {
    GameMenu::Failed {
        role: LinkRow::Host,
        fail: LinkFail::NobodyCame,
        worked: 0,
        since,
    }
}

#[test]
fn pick_hangs_the_plug_above_the_port() {
    assert_eq!(plug_tip(GameMenu::Pick(LinkRow::Host), 0), 330.0);
}

#[test]
fn working_seats_the_plug_in_200ms_and_holds_it_there() {
    let m = working(LinkRow::Host, 1000);
    assert_eq!(plug_tip(m, 1000), 330.0);
    assert!((plug_tip(m, 1200) - 419.0).abs() < 0.01);
    let held: Vec<f32> = (0..3000)
        .step_by(50)
        .map(|t| plug_tip(m, 1200 + t))
        .collect();
    assert!(held.iter().all(|y| (y - 419.0).abs() < 0.01), "{held:?}");
}

#[test]
fn linked_seats_the_plug_from_where_it_was_and_clicks_as_it_lands() {
    let (worked, since) = (1000, 2700);
    let before = plug_tip(working(LinkRow::Host, worked), since);
    let m = GameMenu::Linked {
        role: LinkRow::Host,
        worked,
        since,
        opened: false,
    };
    assert!(
        (plug_tip(m, since) - before).abs() < 0.01,
        "the plug jumped"
    );
    assert!((plug_tip(m, since + 160) - 419.0).abs() < 0.01);
    assert_eq!(clicks_alpha(m, since + 100), 0.0);
    assert_eq!(clicks_alpha(m, since + 160), 1.0);
}

#[test]
fn failed_lifts_tilts_and_dims_over_250ms() {
    let m = failed(5000);
    assert_eq!(plug_turn(m, 5000), 0.0);
    assert!((plug_tip(m, 5250) - 276.0).abs() < 0.01);
    assert!((plug_turn(m, 5250) - 14f32.to_radians()).abs() < 1e-4);
    assert!((art_alpha(m, 5250) - 0.45).abs() < 1e-4);
}

#[test]
fn the_adapter_seats_while_working_and_stays_seated() {
    assert_eq!(adapter_base(GameMenu::Pick(LinkRow::Join), 0), 336.0);
    assert!((adapter_base(working(LinkRow::Join, 0), 200) - PORT_Y).abs() < 0.01);
    assert!((adapter_base(failed(3000), 3250) - PORT_Y).abs() < 0.01);
}

#[test]
fn the_arcs_call_in_turn_hold_when_linked_and_die_on_failure() {
    let a = arc_alphas(working(LinkRow::Host, 0), 200 + 600);
    assert!(
        (a[0] - 1.0).abs() < 0.01,
        "ring 0 is not at its peak: {a:?}"
    );
    assert!(a[1] < 1.0);
    assert_eq!(arc_alphas(GameMenu::Pick(LinkRow::Host), 0), [0.0; 3]);
    let linked = GameMenu::Linked {
        role: LinkRow::Host,
        worked: 0,
        since: 4000,
        opened: false,
    };
    assert_eq!(arc_alphas(linked, 4160), [1.0; 3]);
    assert_eq!(arc_alphas(failed(4000), 4250), [0.0; 3]);
}

fn sprites() -> LinkSprites {
    let s = |n: usize, w: u32, h: u32| Sprite {
        tex: TexId::from_raw(n),
        w,
        h,
    };
    let arc = |n: usize, i: usize| s(n, ARCS[i].2, ARCS[i].3);
    LinkSprites {
        port: s(1, PORT_W, PORT_H),
        plug_host: s(2, PLUG_W, PLUG_H),
        plug_join: s(3, PLUG_W, PLUG_H),
        adapter: s(4, ADAPTER_W, ADAPTER_H),
        arcs_right: [arc(7, 0), arc(8, 1), arc(9, 2)],
        arcs_left: [arc(10, 0), arc(11, 1), arc(12, 2)],
        clicks: s(13, CLICKS_W, CLICKS_H),
        arrow_left: s(14, ARROW_W, ARROW_H),
        arrow_right: s(15, ARROW_W, ARROW_H),
        net_home: s(16, NET_W, NET_H),
        net_direct: s(17, NET_W, NET_H),
    }
}

#[test]
fn a_searching_plug_sits_in_the_port() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(
        working(LinkRow::Host, 0),
        LinkKind::Cable,
        1000,
        &s,
        &mut out,
    );
    let plug = out
        .iter()
        .position(|d| matches!(d, Draw::Tex { tex, .. } if *tex == s.plug_host.tex))
        .expect("no plug");
    assert!(
        matches!(out[plug], Draw::Tex { y, .. } if y == 419.0 - PLUG_H as f32),
        "the plug is not seated in the port: {:?}",
        out[plug]
    );
    let port = out
        .iter()
        .position(|d| matches!(d, Draw::Tex { tex, .. } if *tex == s.port.tex))
        .expect("no port");
    assert!(port > plug, "the port must be drawn over the seated plug");
}

fn texes(out: &[Draw]) -> Vec<TexId> {
    out.iter()
        .filter_map(|d| match d {
            Draw::Tex { tex, .. } | Draw::Turned { tex, .. } => Some(*tex),
            _ => None,
        })
        .collect()
}

#[test]
fn cable_carts_draw_the_plug_under_the_port_and_wireless_carts_the_adapter() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(
        GameMenu::Pick(LinkRow::Host),
        LinkKind::Cable,
        0,
        &s,
        &mut out,
    );
    let t = texes(&out);
    let plug = t
        .iter()
        .position(|x| *x == s.plug_host.tex)
        .expect("no plug");
    let port = t.iter().position(|x| *x == s.port.tex).expect("no port");
    assert!(
        port > plug,
        "the port must be drawn over a plug that goes into it"
    );
    assert!(!t.contains(&s.adapter.tex));

    out.clear();
    draw_link_art(
        GameMenu::Pick(LinkRow::Host),
        LinkKind::Wireless,
        0,
        &s,
        &mut out,
    );
    let t = texes(&out);
    assert!(t.contains(&s.adapter.tex) && !t.contains(&s.plug_host.tex));
}

#[test]
fn a_joiner_holds_the_gray_plug() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(
        GameMenu::Pick(LinkRow::Join),
        LinkKind::Cable,
        0,
        &s,
        &mut out,
    );
    let t = texes(&out);
    assert!(t.contains(&s.plug_join.tex) && !t.contains(&s.plug_host.tex));
}

#[test]
fn the_swap_arrows_show_on_pick_only() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(
        GameMenu::Pick(LinkRow::Host),
        LinkKind::Cable,
        0,
        &s,
        &mut out,
    );
    assert!(texes(&out).contains(&s.arrow_left.tex));
    out.clear();
    draw_link_art(working(LinkRow::Host, 0), LinkKind::Cable, 50, &s, &mut out);
    assert!(!texes(&out).contains(&s.arrow_left.tex));
}

#[test]
fn a_failed_plug_is_drawn_turned() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(failed(0), LinkKind::Cable, 250, &s, &mut out);
    assert!(out.iter().any(
        |d| matches!(d, Draw::Turned { tex, turn, .. } if *tex == s.plug_host.tex && *turn > 0.2)
    ));
}

#[test]
fn unplug_pulls_the_plug_back_out_of_the_port_and_leaves_it_there() {
    let m = GameMenu::Unplug {
        role: LinkRow::Host,
        since: 1000,
    };
    assert!(
        (plug_tip(m, 1000) - 419.0).abs() < 0.01,
        "the unplug did not start from where a seated plug sits"
    );
    assert!(
        (plug_tip(m, 1260) - 330.0).abs() < 0.01,
        "the plug is not back where it was picked up"
    );
    let held: Vec<f32> = (0..2000)
        .step_by(100)
        .map(|t| plug_tip(m, 1260 + t))
        .collect();
    assert!(held.iter().all(|y| (y - 330.0).abs() < 0.01), "{held:?}");
}

#[test]
fn unplug_lifts_the_adapter_off_the_port_and_dies_its_arcs() {
    let m = GameMenu::Unplug {
        role: LinkRow::Join,
        since: 0,
    };
    assert!((adapter_base(m, 0) - PORT_Y).abs() < 0.01);
    assert!((adapter_base(m, 260) - 336.0).abs() < 0.01);
    assert_eq!(
        arc_alphas(m, 0),
        [1.0; 3],
        "a seated adapter's arcs do not start full"
    );
    assert_eq!(arc_alphas(m, 260), [0.0; 3], "the arcs outlived the link");
    let mid = arc_alphas(m, 130);
    assert!(
        mid[0] > 0.0 && mid[0] < 1.0,
        "the arcs did not fade: {mid:?}"
    );
}

#[test]
fn the_unplug_is_the_seating_motion_run_backwards() {
    let out = GameMenu::Unplug {
        role: LinkRow::Host,
        since: 0,
    };
    assert!(
        (plug_tip(out, 0) - plug_tip(working(LinkRow::Host, 0), 1000)).abs() < 0.01,
        "the unplug does not start where a seated plug is"
    );
    assert!(
        (plug_tip(out, 260) - plug_tip(GameMenu::Pick(LinkRow::Host), 0)).abs() < 0.01,
        "the unplug does not end where the plug is picked up"
    );
}

#[test]
fn the_adapter_calls_out_with_its_arcs_while_working() {
    let s = sprites();
    let mut out = Vec::new();
    draw_link_art(
        working(LinkRow::Host, 0),
        LinkKind::Wireless,
        800,
        &s,
        &mut out,
    );
    let t = texes(&out);
    assert!(
        t.contains(&s.arcs_right[0].tex) && t.contains(&s.arcs_left[0].tex),
        "no arcs while working"
    );
}

#[test]
fn the_network_plate_is_printed_on_the_console_only_on_the_role_picker() {
    let s = sprites();
    let plate = |menu: GameMenu, home: bool| {
        let mut out = Vec::new();
        draw_network_label(menu, home, &s, &mut out);
        out
    };
    let home = plate(GameMenu::Pick(LinkRow::Host), true);
    let direct = plate(GameMenu::Pick(LinkRow::Join), false);
    assert!(
        matches!(home[..], [Draw::Tex { tex, x, y, .. }] if tex == s.net_home.tex && x == NET_X && y == NET_Y)
    );
    assert!(matches!(direct[..], [Draw::Tex { tex, .. }] if tex == s.net_direct.tex));
    // Nothing else says which network: once a link is starting, seated or gone it is decided.
    for menu in [
        working(LinkRow::Host, 0),
        failed(0),
        GameMenu::Linked {
            role: LinkRow::Host,
            worked: 0,
            since: 0,
            opened: false,
        },
    ] {
        assert!(plate(menu, true).is_empty());
    }
}
