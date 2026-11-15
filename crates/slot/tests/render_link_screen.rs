mod common;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use slot::app::{GameMenu, LinkLegend, LinkRow};
use slot::link_kind::LinkKind;
use slot::link_net::Cancel;
use slot::link_radio::{RadioFail, RadioJob, RadioJobs, RadioRole};
use slot::link_screen::{draw_link_art, LinkSprites, Sprite};
use slot::link_start::{LinkFail, LinkStarter, LinkStep};
use slot_input::Action;
use slot_store::Core;
use slot_ui::{
    arrows_hint_face, hint_face, link_art, menu_detail_face, menu_face, toast_face, Canvas,
    CartFace, Draw, TexId, Toast, UndoFace, OUT_H, OUT_W, PORT_Y,
};

struct Face {
    rgba: Vec<u8>,
    w: u32,
    h: u32,
}

impl From<CartFace> for Face {
    fn from(f: CartFace) -> Self {
        Face {
            rgba: f.rgba,
            w: f.w,
            h: f.h,
        }
    }
}

impl From<UndoFace> for Face {
    fn from(f: UndoFace) -> Self {
        Face {
            rgba: f.rgba,
            w: f.w,
            h: f.h,
        }
    }
}

fn sprites_and_faces() -> (LinkSprites, Vec<(TexId, Face)>) {
    let art = link_art();
    let mut faces = Vec::new();
    let mut n = 0;
    let mut put = |f: CartFace| {
        n += 1;
        let tex = TexId::from_raw(n);
        let sprite = Sprite {
            tex,
            w: f.w,
            h: f.h,
        };
        faces.push((tex, f.into()));
        sprite
    };
    let [ar0, ar1, ar2] = art.arcs_right;
    let [al0, al1, al2] = art.arcs_left;
    let sprites = LinkSprites {
        port: put(art.port),
        plug_host: put(art.plug_host),
        plug_join: put(art.plug_join),
        adapter: put(art.adapter),
        arcs_right: [put(ar0), put(ar1), put(ar2)],
        arcs_left: [put(al0), put(al1), put(al2)],
        clicks: put(art.clicks),
        arrow_left: put(art.arrow_left),
        arrow_right: put(art.arrow_right),
    };
    (sprites, faces)
}

fn composite(out: &[Draw], faces: &[(TexId, Face)]) -> Vec<u8> {
    let (w, h) = (OUT_W as usize, OUT_H as usize);
    let mut px = vec![0u8; w * h * 4];
    for i in 0..w * h {
        px[i * 4..i * 4 + 4].copy_from_slice(&[0x05, 0x05, 0x08, 255]);
    }
    let blend = |x: i32, y: i32, c: [u8; 4], alpha: f32, px: &mut Vec<u8>| {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 {
            return;
        }
        let a = (c[3] as f32 / 255.0) * alpha;
        let d = (y as usize * w + x as usize) * 4;
        for (k, cc) in c.iter().take(3).enumerate() {
            px[d + k] = (*cc as f32 * a + px[d + k] as f32 * (1.0 - a)).round() as u8;
        }
    };
    for d in out {
        let (x, y, dw, dh, tex, alpha, turn) = match *d {
            Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha,
            } => (x, y, w, h, tex, alpha, 0.0),
            Draw::Turned {
                x,
                y,
                w,
                h,
                tex,
                alpha,
                turn,
            } => (x, y, w, h, tex, alpha, turn),
            _ => continue,
        };
        let face = &faces
            .iter()
            .find(|(t, _)| *t == tex)
            .expect("unknown tex")
            .1;
        let (cx, cy) = (x + dw / 2.0, y + dh / 2.0);
        let (sin, cos) = turn.sin_cos();
        let reach = (dw.max(dh) * 0.75) as i32;
        for py in (cy as i32 - reach)..(cy as i32 + reach) {
            for qx in (cx as i32 - reach)..(cx as i32 + reach) {
                let (rx, ry) = (qx as f32 + 0.5 - cx, py as f32 + 0.5 - cy);
                let (ux, uy) = (
                    rx * cos + ry * sin + dw / 2.0,
                    -rx * sin + ry * cos + dh / 2.0,
                );
                if ux < 0.0 || uy < 0.0 || ux >= face.w as f32 || uy >= face.h as f32 {
                    continue;
                }
                let s = (uy as usize * face.w as usize + ux as usize) * 4;
                let c = [
                    face.rgba[s],
                    face.rgba[s + 1],
                    face.rgba[s + 2],
                    face.rgba[s + 3],
                ];
                blend(qx, py, c, alpha, &mut px);
            }
        }
    }
    px
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

fn render(menu: GameMenu, kind: LinkKind, now: u64, name: &str) -> Vec<u8> {
    let (sprites, faces) = sprites_and_faces();
    let mut out = Vec::new();
    draw_link_art(Canvas::FRAMED, menu, kind, now, &sprites, &mut out);
    let px = composite(&out, &faces);
    dump(&px, name);
    px
}

fn dump(px: &[u8], name: &str) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let path = format!("{dir}/link-{name}.png");
    let file = std::fs::File::create(&path).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(px).unwrap();
    println!("wrote {path}");
}

#[test]
fn the_host_picks_a_purple_plug_over_the_port() {
    let px = render(
        GameMenu::Pick(LinkRow::Host),
        LinkKind::Cable,
        0,
        "cable-host-pick",
    );
    let housing = at(&px, 356, 330 - 60);
    assert!(
        housing[2] > housing[1] + 30,
        "no purple housing above the port: {housing:?}"
    );
    let slot = at(&px, 360, PORT_Y as usize + 18);
    assert!(
        slot.iter().all(|c| *c < 0x40),
        "no port under the plug: {slot:?}"
    );
}

#[test]
fn a_joiner_picks_a_gray_plug() {
    let px = render(
        GameMenu::Pick(LinkRow::Join),
        LinkKind::Cable,
        0,
        "cable-join-pick",
    );
    let housing = at(&px, 356, 330 - 60);
    assert!(
        (housing[0] as i32 - housing[2] as i32).abs() < 14 && housing[0] > 0x70,
        "not gray: {housing:?}"
    );
}

#[test]
fn a_wireless_link_seats_the_adapter_with_its_label_plate() {
    let menu = GameMenu::Linked {
        role: LinkRow::Host,
        worked: 0,
        since: 600,
        opened: false,
    };
    let px = render(menu, LinkKind::Wireless, 2000, "wireless-linked");
    let y = PORT_Y as usize - 12;
    let body = (300..420).filter(|x| at(&px, *x, y)[0] > 0x60).count();
    assert!(
        body > 100,
        "the seated adapter's printed plate is not above the port: {body} light pixels"
    );
}

#[test]
fn a_failed_plug_leaves_where_it_waited() {
    let working = GameMenu::Working {
        role: LinkRow::Host,
        step: LinkStep::Waiting,
        since: 0,
    };
    let waiting = render(working, LinkKind::Cable, 1200, "cable-working");
    let failed = GameMenu::Failed {
        role: LinkRow::Host,
        fail: LinkFail::NobodyCame,
        worked: 0,
        since: 1200,
    };
    let gone = render(failed, LinkKind::Cable, 1600, "cable-failed");
    let spot = (356, 350 - 60);
    assert_ne!(
        at(&waiting, spot.0, spot.1),
        at(&gone, spot.0, spot.1),
        "the plug never lifted away"
    );
}

const GROUND: [u8; 3] = [0x05, 0x05, 0x08];

#[test]
fn ending_a_link_pulls_the_plug_back_out_of_the_port() {
    let menu = GameMenu::Unplug {
        role: LinkRow::Host,
        since: 0,
    };
    let seated = render(menu, LinkKind::Cable, 0, "cable-unplug-start");
    let gone = render(menu, LinkKind::Cable, 400, "cable-unplug-end");
    // Below the resting tip (367) and above the port, so only a seated plug reaches it.
    let (x, y) = (356, 400);
    assert_ne!(
        at(&seated, x, y),
        GROUND,
        "no plug in the port on the frame the unplug begins"
    );
    assert_eq!(
        at(&gone, x, y),
        GROUND,
        "the plug never came back out of the port"
    );
}

#[test]
fn ending_a_wireless_link_lifts_the_adapter_off_the_port() {
    let menu = GameMenu::Unplug {
        role: LinkRow::Join,
        since: 0,
    };
    let seated = render(menu, LinkKind::Wireless, 0, "wireless-unplug-start");
    let gone = render(menu, LinkKind::Wireless, 400, "wireless-unplug-end");
    let (x, y) = (360, 380);
    assert_ne!(
        at(&seated, x, y),
        GROUND,
        "no adapter on the port on the frame the unplug begins"
    );
    assert_eq!(
        at(&gone, x, y),
        GROUND,
        "the adapter never lifted off the port"
    );
}

fn lit(px: &[u8], o: usize) -> bool {
    px[o] != 0x05 || px[o + 1] != 0x05 || px[o + 2] != 0x08
}

fn ink(px: &[u8]) -> usize {
    (0..px.len() / 4).filter(|i| lit(px, i * 4)).count()
}

fn inked_rows(px: &[u8]) -> (usize, usize) {
    let rows: Vec<usize> = (0..OUT_H as usize)
        .filter(|y| (0..OUT_W as usize).any(|x| lit(px, (y * OUT_W as usize + x) * 4)))
        .collect();
    (
        *rows.first().expect("nothing on the frame"),
        *rows.last().expect("nothing on the frame"),
    )
}

fn legend_faces() -> Vec<(TexId, Face)> {
    LinkLegend::ALL
        .iter()
        .map(|k| {
            let f: Face = match k {
                LinkLegend::Cancel => hint_face("B", "Cancel"),
                LinkLegend::Mode => hint_face("SELECT", "Mode"),
                LinkLegend::Swap => arrows_hint_face("Swap"),
                LinkLegend::Link => hint_face("A", "Link"),
                LinkLegend::Ok => hint_face("A", "OK"),
                LinkLegend::Back => hint_face("B", "Back"),
                LinkLegend::EndLink => hint_face("A", "End Link"),
            }
            .into();
            (TexId::from_raw(900 + k.index()), f)
        })
        .collect()
}

fn legend_pixels(title: &str, code: &str, name: &str) -> Vec<u8> {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", title, code);
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let faces = legend_faces();
    app.set_link_legend_faces(faces.iter().map(|(t, f)| (*t, f.w)).collect());
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open(), "{code} never opened its link screen");
    let mut out = Vec::new();
    app.draw(&mut out);
    let legend: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let px = composite(&legend, &faces);
    dump(&px, name);
    px
}

fn connected_legend_pixels(name: &str) -> Vec<u8> {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", "POKEMON RUBY", "AXVE");
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let faces = legend_faces();
    app.set_link_legend_faces(faces.iter().map(|(t, f)| (*t, f.w)).collect());
    app.begin_link(0);
    app.apply(Action::GameMenu);
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Linked { opened: true, .. })),
        "the shortcut did not open the connected screen over a live session"
    );
    let mut out = Vec::new();
    app.draw(&mut out);
    let legend: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let px = composite(&legend, &faces);
    dump(&px, name);
    px
}

fn cap_ink(key: &'static str, label: &'static str) -> usize {
    let f: Face = hint_face(key, label).into();
    let (w, h) = (f.w as f32, f.h as f32);
    let tex = TexId::from_raw(1);
    let draw = Draw::Tex {
        x: 100.0,
        y: 422.0,
        w,
        h,
        tex,
        alpha: 1.0,
    };
    ink(&composite(&[draw], &[(tex, f)]))
}

#[test]
fn the_connected_screen_shows_back_and_end_link() {
    let px = connected_legend_pixels("legend-connected");
    assert!(ink(&px) > 0, "the connected screen drew no legend at all");
    assert_eq!(
        ink(&px),
        cap_ink("B", "Back") + cap_ink("A", "End Link"),
        "the row is not exactly a Back cap and an End Link cap"
    );
}

fn toast_faces() -> Vec<(TexId, Face)> {
    Toast::all()
        .into_iter()
        .map(|t| (TexId::from_raw(800 + t.index()), toast_face(t).into()))
        .collect()
}

fn banner_pixels(core: Core, title: &str, code: &str, name: &str) -> Vec<u8> {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", title, code);
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(core);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let faces = toast_faces();
    app.set_toast_faces(faces.iter().map(|(t, _)| *t).collect());
    app.apply(Action::GameMenu);
    assert!(
        !app.game_menu_open(),
        "{code} on {core:?} opened a link screen instead of refusing"
    );
    let mut out = Vec::new();
    app.draw(&mut out);
    let banner: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let px = composite(&banner, &faces);
    dump(&px, name);
    px
}

fn banner_ink(t: Toast) -> usize {
    let f: Face = toast_face(t).into();
    let (w, h) = (f.w as f32, f.h as f32);
    let tex = TexId::from_raw(1);
    let draw = Draw::Tex {
        x: 100.0,
        y: 20.0,
        w,
        h,
        tex,
        alpha: 1.0,
    };
    ink(&composite(&[draw], &[(tex, f)]))
}

#[test]
fn a_cart_nothing_can_link_reads_no_link_support_on_the_glass() {
    let px = banner_pixels(Core::Gpsp, "APOTRIS", "2ATE", "banner-apotris-gpsp");
    assert!(
        ink(&px) > 0,
        "the refusal put no banner on the frame at all"
    );
    assert_eq!(
        ink(&px),
        banner_ink(Toast::NoLink),
        "the banner is not the NO LINK SUPPORT line"
    );
    assert_ne!(
        banner_ink(Toast::NoLink),
        banner_ink(Toast::NeedsGpsp),
        "the two sentences carry the same ink, so this frame proves nothing"
    );
    let (first, last) = inked_rows(&px);
    assert!(
        last < OUT_H as usize / 4,
        "the banner is not up in the plate band: rows {first}..{last}"
    );
}

#[test]
fn a_cart_gpsp_can_link_reads_please_switch_to_gpsp() {
    let px = banner_pixels(Core::Mgba, "POKEMON EMER", "BPEE", "banner-emerald-mgba");
    assert!(
        ink(&px) > 0,
        "the refusal put no banner on the frame at all"
    );
    assert_eq!(
        ink(&px),
        banner_ink(Toast::NeedsGpsp),
        "the banner is not the PLEASE SWITCH TO GPSP line"
    );
}

#[test]
fn a_gambatte_game_reads_please_switch_to_mgba_on_the_glass() {
    let px = banner_pixels(Core::Gambatte, "POKEMON RED", "BPRE", "banner-gambatte");
    assert_eq!(
        ink(&px),
        banner_ink(Toast::NeedsMgba),
        "the banner is not the PLEASE SWITCH TO MGBA line"
    );
}

#[test]
fn the_switch_to_mgba_banner_sits_inside_both_panels() {
    for (panel, name) in [
        ((640, 480), "needs-mgba-640"),
        ((720, 480), "needs-mgba-720"),
    ] {
        let canvas = Canvas::of(panel);
        let f: Face = toast_face(Toast::NeedsMgba).into();
        let (x, y, w, h) = slot_ui::toast_rect(canvas, Toast::NeedsMgba);
        let tex = TexId::from_raw(1);
        let draw = Draw::Tex {
            x,
            y,
            w,
            h,
            tex,
            alpha: 1.0,
        };
        let px = composite(&[draw], &[(tex, f)]);
        assert!(ink(&px) > 0);
        assert!(
            x >= 0.0 && x + w <= canvas.w,
            "{name}: the banner leaves the panel"
        );
        dump(&px, name);
    }
}

#[test]
fn the_legend_shows_select_mode_only_where_the_hardware_can_be_switched() {
    let switchable = legend_pixels("POKEMON EMER", "BPEE", "legend-switchable");
    let fixed = legend_pixels("MARIO GOLF", "BMGE", "legend-fixed");

    assert_eq!(
        ink(&switchable),
        cap_ink("B", "Cancel") + cap_ink("SELECT", "Mode"),
        "the switchable row is not exactly a Cancel cap and a SELECT Mode cap"
    );
    assert_eq!(
        ink(&fixed),
        cap_ink("B", "Cancel") + cap_ink("A", "Link"),
        "the fixed row is not exactly a Cancel cap and a Link cap"
    );

    for (px, what) in [(&switchable, "switchable"), (&fixed, "fixed")] {
        let (first, last) = inked_rows(px);
        assert!(
            first >= OUT_H as usize * 3 / 4,
            "the {what} legend is not down on the strip: rows {first}..{last}"
        );
        assert!(
            last < OUT_H as usize,
            "the {what} legend runs off the bottom of the panel"
        );
    }
}

#[derive(Clone, Default)]
struct FakeRadio {
    warm: Arc<AtomicBool>,
    asked: Arc<Mutex<Vec<RadioJob>>>,
}

impl RadioJobs for FakeRadio {
    fn ask(&mut self, job: RadioJob) {
        self.asked.lock().expect("radio log").push(job);
    }

    fn warmed(&self) -> bool {
        self.warm.load(Ordering::SeqCst)
    }
}

fn step_faces() -> Vec<(TexId, Face)> {
    LinkStep::ALL
        .iter()
        .map(|s| (TexId::from_raw(950 + s.index()), menu_face(s.line()).into()))
        .collect()
}

fn first_step_pixels(warm: bool, name: &str) -> (Vec<u8>, Vec<RadioJob>, Vec<LinkStep>) {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", "POKEMON RUBY", "AXVE");
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let radio = FakeRadio::default();
    radio.warm.store(warm, Ordering::SeqCst);
    app.set_radio_jobs(Box::new(radio.clone()));
    let faces = step_faces();
    app.set_link_step_faces(faces.iter().map(|(t, f)| (*t, f.w, f.h)).collect());
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open(), "the link screen never opened");
    let (release, held) = channel::<()>();
    app.start_link(
        LinkStarter::spawn_with(
            Box::new(move |_, _| {
                let _ = held.recv();
                Ok(None)
            }),
            Box::new(|| {}),
            RadioRole::Host,
            0,
            Box::new(|_, _: &Cancel| Err(io::Error::new(io::ErrorKind::TimedOut, "from a test"))),
        ),
        0,
    );
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Working {
                step: LinkStep::Radio,
                ..
            })
        ),
        "the screen is not on the step this is about: {:?}",
        app.game_menu()
    );
    let mut out = Vec::new();
    app.draw(&mut out);
    let line: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let drawn = line
        .iter()
        .filter_map(|d| match *d {
            Draw::Tex { tex, .. } => LinkStep::ALL
                .into_iter()
                .find(|s| TexId::from_raw(950 + s.index()) == tex),
            _ => None,
        })
        .collect();
    let px = composite(&line, &faces);
    dump(&px, name);
    drop(release);
    let asked = radio.asked.lock().expect("radio log").clone();
    (px, asked, drawn)
}

fn line_ink(step: LinkStep) -> usize {
    sentence_ink(step.line())
}

fn says_waiting_for_a_friend_up_top(px: &[u8], what: &str) {
    assert!(
        ink(px) > 0,
        "{what}: the step put no sentence on the frame at all"
    );
    assert_eq!(
        ink(px),
        line_ink(LinkStep::Waiting),
        "{what}: the sentence on the frame is not Waiting for a friend"
    );
    let (first, last) = inked_rows(px);
    assert!(
        last < OUT_H as usize / 2,
        "{what}: the line is not up where the link screen's sentence goes: rows {first}..{last}"
    );
}

#[test]
fn both_radio_steps_say_waiting_for_a_friend() {
    for step in LinkStep::ALL {
        assert_eq!(step.line(), "Waiting for a friend", "{step:?}");
    }
}

#[test]
fn a_cold_radio_asks_for_a_warm_and_waits_for_a_friend() {
    let (px, asked, drawn) = first_step_pixels(false, "step-radio-cold");
    assert!(
        asked.contains(&RadioJob::Warm),
        "the screen never asked for a warm, so this frame says nothing about finishing one"
    );
    assert_eq!(
        drawn,
        vec![LinkStep::Radio],
        "a screen whose warm has not finished drew another step's face"
    );
    says_waiting_for_a_friend_up_top(&px, "cold");
}

#[test]
fn a_warm_radio_goes_straight_to_the_waiting_step() {
    let (px, _, drawn) = first_step_pixels(true, "step-radio-warm");
    assert_eq!(
        drawn,
        vec![LinkStep::Waiting],
        "a warm radio is still drawing the radio step's face"
    );
    says_waiting_for_a_friend_up_top(&px, "warm");
}

type FailFaces = (Vec<(TexId, Face)>, Vec<Option<(TexId, Face)>>);

fn fail_faces() -> FailFaces {
    let lines = LinkFail::SHOWN
        .iter()
        .enumerate()
        .map(|(i, f)| {
            (
                TexId::from_raw(970 + i),
                menu_face(f.line(LinkKind::Cable)).into(),
            )
        })
        .collect();
    let hints = LinkFail::SHOWN
        .iter()
        .enumerate()
        .map(|(i, f)| {
            f.hint()
                .map(|h| (TexId::from_raw(980 + i), menu_detail_face(h).into()))
        })
        .collect();
    (lines, hints)
}

fn failure_pixels(fail: RadioFail, name: &str) -> Vec<u8> {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", "POKEMON RUBY", "AXVE");
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let (lines, hints) = fail_faces();
    app.set_link_fail_faces(
        lines.iter().map(|(t, f)| (*t, f.w, f.h)).collect(),
        hints
            .iter()
            .map(|h| h.as_ref().map(|(t, f)| (*t, f.w, f.h)))
            .collect(),
    );
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open(), "the link screen never opened");
    app.start_link(
        LinkStarter::spawn_with(
            Box::new(move |_, _| Err(fail.clone())),
            Box::new(|| {}),
            RadioRole::Host,
            0,
            Box::new(|_, _: &Cancel| Err(io::Error::new(io::ErrorKind::TimedOut, "from a test"))),
        ),
        0,
    );
    for _ in 0..200 {
        if matches!(app.game_menu(), Some(GameMenu::Failed { .. })) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update(1.0 / 60.0);
    }
    assert!(
        matches!(app.game_menu(), Some(GameMenu::Failed { .. })),
        "the screen never failed: {:?}",
        app.game_menu()
    );
    let faces: Vec<(TexId, Face)> = lines
        .into_iter()
        .chain(hints.into_iter().flatten())
        .collect();
    let mut out = Vec::new();
    app.draw(&mut out);
    let text: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let px = composite(&text, &faces);
    dump(&px, name);
    px
}

fn bands(px: &[u8]) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for y in 0..OUT_H as usize {
        if !(0..OUT_W as usize).any(|x| lit(px, (y * OUT_W as usize + x) * 4)) {
            continue;
        }
        match out.last_mut() {
            Some((_, end)) if *end + 1 == y => *end = y,
            _ => out.push((y, y)),
        }
    }
    out
}

#[test]
fn a_radio_failure_says_check_logs_on_the_line_below() {
    let px = failure_pixels(RadioFail::Radio("from a test".into()), "fail-radio");
    let rows = bands(&px);
    assert_eq!(
        rows.len(),
        2,
        "want the sentence and the hint, got bands {rows:?}"
    );
    assert!(
        rows[1].0 > rows[0].1,
        "the hint is not below the sentence: {rows:?}"
    );
}

#[test]
fn nobody_joining_has_no_second_line() {
    let px = failure_pixels(RadioFail::NoHost, "fail-nobody");
    assert_eq!(bands(&px).len(), 1, "a timeout grew a hint line");
}

fn dropped_pixels(title: &str, code: &str, name: &str) -> (Vec<u8>, Vec<u8>, Vec<Draw>) {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", title, code);
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let (sprites, mut faces) = sprites_and_faces();
    app.set_link_sprites(sprites);
    let (lines, _) = fail_faces();
    let wireless: Vec<Option<(TexId, Face)>> = LinkFail::SHOWN
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let line = f.line(LinkKind::Wireless);
            (line != f.line(LinkKind::Cable))
                .then(|| (TexId::from_raw(990 + i), menu_face(line).into()))
        })
        .collect();
    app.set_link_fail_faces(
        lines.iter().map(|(t, f)| (*t, f.w, f.h)).collect(),
        vec![None; LinkFail::SHOWN.len()],
    );
    app.set_link_fail_wireless_faces(
        wireless
            .iter()
            .map(|w| w.as_ref().map(|(t, f)| (*t, f.w, f.h)))
            .collect(),
    );
    app.apply(Action::GameMenu);
    assert!(app.game_menu_open(), "the link screen never opened");
    app.start_link(
        LinkStarter::spawn_with(
            Box::new(|_, _| Ok(None)),
            Box::new(|| {}),
            RadioRole::Host,
            0,
            Box::new(|_, _: &Cancel| {
                Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "from a test",
                ))
            }),
        ),
        0,
    );
    for _ in 0..200 {
        if matches!(app.game_menu(), Some(GameMenu::Failed { .. })) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        app.update(1.0 / 60.0);
    }
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Failed {
                fail: LinkFail::PeerVanished,
                ..
            })
        ),
        "the link never dropped: {:?}",
        app.game_menu()
    );
    for _ in 0..60 {
        app.update(1.0 / 60.0);
    }
    let text: Vec<(TexId, Face)> = lines
        .into_iter()
        .chain(wireless.into_iter().flatten())
        .collect();
    let text_ids: Vec<TexId> = text.iter().map(|(t, _)| *t).collect();
    faces.extend(text);
    let mut out = Vec::new();
    app.draw(&mut out);
    let out: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } | Draw::Turned { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let line: Vec<Draw> = out
        .iter()
        .filter(|d| matches!(**d, Draw::Tex { tex, .. } if text_ids.contains(&tex)))
        .copied()
        .collect();
    let whole = composite(&out, &faces);
    dump(&whole, name);
    (composite(&line, &faces), whole, out)
}

fn sentence_ink(line: &str) -> usize {
    let f: Face = menu_face(line).into();
    let (w, h) = (f.w as f32, f.h as f32);
    let tex = TexId::from_raw(1);
    let draw = Draw::Tex {
        x: 100.0,
        y: 20.0,
        w,
        h,
        tex,
        alpha: 1.0,
    };
    ink(&composite(&[draw], &[(tex, f)]))
}

fn drew(out: &[Draw], s: Sprite) -> bool {
    out.iter()
        .any(|d| matches!(*d, Draw::Tex { tex, .. } | Draw::Turned { tex, .. } if tex == s.tex))
}

#[test]
fn a_dropped_wireless_link_says_it_lost_the_connection() {
    let (line, _, out) = dropped_pixels("POKEMON EMER", "BPEE", "fail-dropped-wireless");
    let (sprites, _) = sprites_and_faces();
    assert!(drew(&out, sprites.adapter), "no adapter over the failure");
    assert_ne!(
        sentence_ink("Lost the connection"),
        sentence_ink("The cable came loose"),
        "the two sentences carry the same ink, so the frame below proves nothing"
    );
    assert_eq!(
        ink(&line),
        sentence_ink("Lost the connection"),
        "a dropped adapter link does not say Lost the connection"
    );
    let (first, last) = inked_rows(&line);
    assert!(
        last < OUT_H as usize / 2,
        "the line is not up where the link screen's sentence goes: rows {first}..{last}"
    );
}

#[test]
fn a_dropped_cable_link_says_the_cable_came_loose() {
    let (line, _, out) = dropped_pixels("POKEMON RUBY", "AXVE", "fail-dropped-cable");
    let (sprites, _) = sprites_and_faces();
    assert!(drew(&out, sprites.plug_host), "no plug over the failure");
    assert_eq!(
        ink(&line),
        sentence_ink("The cable came loose"),
        "a dropped cable link does not say The cable came loose"
    );
}

fn unplug_pixels(title: &str, code: &str, role: LinkRow, age: u64, name: &str) -> Vec<u8> {
    let d = common::tmp_root_with_carts(&["Zzz"]);
    common::write_retail_header(&d, "Cart", title, code);
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(Core::Gpsp);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    let (sprites, faces) = sprites_and_faces();
    app.set_link_sprites(sprites);
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(slot_input::Btn::B));
    assert!(
        !app.game_menu_open(),
        "{name}: the link screen never closed"
    );
    app.begin_link(role.client_id());
    match role {
        LinkRow::Host => {
            app.apply(Action::GameMenu);
            app.apply(Action::GbaDown(slot_input::Btn::A));
        }
        LinkRow::Join => app.peer_ended(),
    }
    let Some(GameMenu::Unplug { since, .. }) = app.game_menu() else {
        panic!("{name}: the end did not unplug: {:?}", app.game_menu());
    };
    while app.now() < since + age {
        app.update(1.0 / 60.0);
    }
    let mut out = Vec::new();
    app.draw(&mut out);
    let out: Vec<Draw> = out
        .into_iter()
        .filter(|d| matches!(*d, Draw::Tex { tex, .. } | Draw::Turned { tex, .. } if faces.iter().any(|(t, _)| *t == tex)))
        .collect();
    let px = composite(&out, &faces);
    dump(&px, name);
    px
}

fn port_only() -> Vec<u8> {
    let (sprites, faces) = sprites_and_faces();
    let mut out = Vec::new();
    let dx = Canvas::FRAMED.centred(sprites.port.w as f32);
    out.push(Draw::Tex {
        x: dx,
        y: PORT_Y,
        w: sprites.port.w as f32,
        h: sprites.port.h as f32,
        tex: sprites.port.tex,
        alpha: 1.0,
    });
    composite(&out, &faces)
}

#[test]
fn the_unplug_screen_keeps_the_port_along_the_bottom_until_it_leaves() {
    let port = port_only();
    let below_the_plug = PORT_Y as usize + 36;
    let band = |px: &[u8]| -> Vec<u8> { px[below_the_plug * OUT_W as usize * 4..].to_vec() };
    assert!(
        ink(&band(&port)) > 1000,
        "the port strip carries too little ink below the plug to prove anything"
    );
    let late = slot::app::UNPLUG_HOLD_MS - 100;
    for (title, code, kind) in [
        ("POKEMON RUBY", "AXVE", "cable"),
        ("POKEMON EMER", "BPEE", "wireless"),
    ] {
        for role in LinkRow::ALL {
            for age in [0, 300, late] {
                let name = format!("unplug-{kind}-{}-{age}ms", role.text().to_lowercase());
                let px = unplug_pixels(title, code, role, age, &name);
                assert!(
                    band(&px) == band(&port),
                    "{name}: the port strip is not along the bottom of the unplug screen"
                );
            }
        }
    }
}
