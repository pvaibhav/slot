mod common;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use slot::app::{GameMenu, LinkLegend, LinkRow};
use slot::link_kind::LinkKind;
use slot::link_net::Cancel;
use slot::link_radio::{LinkRole, RadioJob, RadioJobs};
use slot::link_screen::{draw_link_art, draw_network_label, LinkSprites, Sprite};
use slot::link_start::{LinkFail, LinkStarter, LinkStep};
use slot_input::Action;
use slot_store::Core;
use slot_ui::{
    arrows_hint_face, hint_face, link_art, menu_face, toast_face, CartFace, Draw, TexId, Toast,
    UndoFace, OUT_H, OUT_W,
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
        net_home: put(art.net_home),
        net_direct: put(art.net_direct),
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
    draw_link_art(menu, kind, now, &sprites, &mut out);
    let px = composite(&out, &faces);
    dump(&px, name);
    px
}

/// The picker as a player sees it: the art, then the plate that names the network.
fn render_picker(kind: LinkKind, home: bool, name: &str) -> Vec<u8> {
    let (sprites, faces) = sprites_and_faces();
    let mut out = Vec::new();
    let menu = GameMenu::Pick(LinkRow::Host);
    draw_link_art(menu, kind, 0, &sprites, &mut out);
    draw_network_label(menu, home, &sprites, &mut out);
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
    let slot = at(&px, 360, 406);
    assert!(
        slot.iter().all(|c| *c < 0x14),
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
    let plate = at(&px, 360, 388 - 12);
    assert!(
        plate[0] > 0x28 && plate[0] < 0x70,
        "no plate where the seated adapter's label goes: {plate:?}"
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
    let (x, y) = (356, 359);
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
    Toast::ALL
        .iter()
        .map(|t| (TexId::from_raw(800 + t.index()), toast_face(*t).into()))
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

fn mode_cap_ink() -> usize {
    let f: Face = hint_face("SELECT", "Mode").into();
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
fn the_legend_shows_select_mode_only_where_the_hardware_can_be_switched() {
    let switchable = legend_pixels("POKEMON RUBY", "AXVE", "legend-switchable");
    let fixed = legend_pixels("MARIO GOLF", "BMGE", "legend-fixed");

    assert!(
        ink(&fixed) > 0,
        "the legend put no type on the frame at all"
    );
    assert!(
        ink(&switchable) > ink(&fixed),
        "both screens carry the same type: {} and {}",
        ink(&switchable),
        ink(&fixed)
    );
    assert_eq!(
        ink(&switchable) - ink(&fixed),
        mode_cap_ink(),
        "the difference between the two frames is not one SELECT Mode cap"
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

fn first_step_pixels(warm: bool, name: &str) -> (Vec<u8>, Vec<RadioJob>) {
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
                Ok(())
            }),
            Box::new(|| {}),
            LinkRole::Host,
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
    let px = composite(&line, &faces);
    dump(&px, name);
    drop(release);
    let asked = radio.asked.lock().expect("radio log").clone();
    (px, asked)
}

fn line_ink(step: LinkStep) -> usize {
    let f: Face = menu_face(step.line()).into();
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
fn a_cold_radio_says_it_is_bringing_the_radio_up() {
    let (px, asked) = first_step_pixels(false, "step-radio-cold");
    assert!(ink(&px) > 0, "the step put no sentence on the frame at all");
    assert!(
        asked.contains(&RadioJob::Warm),
        "the screen never asked for a warm, so this frame says nothing about finishing one"
    );
    assert_ne!(
        line_ink(LinkStep::Radio),
        line_ink(LinkStep::Waiting),
        "the two sentences carry the same ink, so neither frame below proves anything"
    );
    assert_eq!(
        ink(&px),
        line_ink(LinkStep::Radio),
        "a screen whose warm has not finished is not saying the radio is coming up"
    );
}

#[test]
fn a_warm_radio_goes_straight_to_looking_for_the_other_player() {
    let (px, _) = first_step_pixels(true, "step-radio-warm");
    assert!(ink(&px) > 0, "the step put no sentence on the frame at all");
    assert_eq!(
        ink(&px),
        line_ink(LinkStep::Waiting),
        "a warm radio is still being announced as coming up"
    );
    let (first, last) = inked_rows(&px);
    assert!(
        last < OUT_H as usize / 2,
        "the line is not up where the link screen's sentence goes: rows {first}..{last}"
    );
}

/// The plate is lettering on the console strip: light ink somewhere inside its box, and the
/// two networks are two different plates rather than one plate with a word swapped.
#[test]
fn the_picker_prints_which_network_the_link_will_use() {
    let (x0, y0) = (slot_ui::NET_X as usize, slot_ui::NET_Y as usize);
    let ink = |px: &[u8]| {
        let mut n = 0;
        for y in y0..y0 + slot_ui::NET_H as usize {
            for x in x0..x0 + slot_ui::NET_W as usize {
                if at(px, x, y).iter().all(|c| *c > 0xc0) {
                    n += 1;
                }
            }
        }
        n
    };
    for kind in [LinkKind::Cable, LinkKind::Wireless] {
        let home = render_picker(kind, true, &format!("net-home-{kind:?}").to_lowercase());
        let direct = render_picker(kind, false, &format!("net-direct-{kind:?}").to_lowercase());
        assert!(
            ink(&home) > 60,
            "no lettering on the home plate: {}",
            ink(&home)
        );
        assert!(
            ink(&direct) > 60,
            "no lettering on the direct plate: {}",
            ink(&direct)
        );
        assert_ne!(home, direct);
        // The plate sits on the strip, clear of the port notch the plug lands in.
        assert!(at(&home, 360, 406).iter().all(|c| *c < 0x14));
    }
}
