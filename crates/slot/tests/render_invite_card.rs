#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use slot::ble::FakeBle;
use slot::frontend::Frontend;
use slot::invite_card::{card_origin, CARD_H, CARD_W};
use slot::link_kind::LinkKind;
use slot::link_radio::HostAp;
use slot_gfx::{Compositor, HeadlessSurface};
use slot_input::{InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::{write_slot_state, KidMode, SlotState};

const AP: HostAp = HostAp {
    freq_mhz: 5180,
    bssid: [2, 0, 0, 0, 0, 1],
};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn write_png(name: &str, w: u32, h: u32, rgba: &[u8]) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let path = format!("{dir}/{name}.png");
    let file = std::fs::File::create(&path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(rgba)
        .expect("png data");
    println!("wrote {path}");
}

fn at(px: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
    let o = ((y * w + x) * 4) as usize;
    [px[o], px[o + 1], px[o + 2]]
}

fn sum(c: [u8; 3]) -> u32 {
    c.iter().map(|v| *v as u32).sum()
}

fn advance_until(
    f: &mut Frontend,
    c: &mut Compositor,
    input: &mut Script,
    done: impl Fn(&Frontend) -> bool,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(f) {
        if Instant::now() >= deadline {
            return false;
        }
        f.advance(input);
        f.compose(c);
        std::thread::sleep(Duration::from_millis(2));
    }
    true
}

fn render_card(panel: (u32, u32), name: &str, kind: LinkKind) {
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };
    let d = common::tmp_root_with_carts(&["Emerald", "Mario Kart", "Wario Ware"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            kid_mode: KidMode::Shelves,
            ..SlotState::default()
        },
    )
    .expect("write slot.state");
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    f.upload_bezel(&mut c, panel);
    let (ble, remote) = FakeBle::new();
    f.app_mut().set_ble(Box::new(ble));
    let mut input = Script(VecDeque::new());
    assert!(
        advance_until(&mut f, &mut c, &mut input, |f| f.app().link_sprites_ready()),
        "{name}: the link art never built"
    );
    for _ in 0..10 {
        f.advance(&mut input);
    }
    c.fit(panel);
    f.compose(&mut c);
    let before = c.read_frame();
    write_png(&format!("invite-{name}-before"), panel.0, panel.1, &before);
    let ink = |xs: std::ops::Range<u32>| {
        (436..466)
            .flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| sum(at(&before, panel.0, x, y)) > 600)
            .count()
    };
    let (by_battery, by_clock) = (ink(140..175), ink(panel.0 - 115..panel.0 - 85));
    if panel.0 * 3 <= panel.1 * 4 {
        assert!(
            by_battery == 0 && by_clock > 20,
            "{name}: the bear should sit left of the clock (battery side {by_battery}, clock side {by_clock})"
        );
    } else {
        assert!(
            by_battery > 20 && by_clock == 0,
            "{name}: the bear should follow the battery (battery side {by_battery}, clock side {by_clock})"
        );
    }

    let mut heard = common::heard_host(&d, "Mario Kart", 7, AP, -60);
    heard.advert.kind = kind;
    remote.hear(heard);
    assert!(
        advance_until(&mut f, &mut c, &mut input, |f| f
            .app()
            .invite_card()
            .is_some()),
        "{name}: the card never opened"
    );
    f.advance(&mut input);
    c.fit(panel);
    f.compose(&mut c);
    assert_eq!(c.frame_size(), panel);
    let px = c.read_frame();
    write_png(&format!("invite-{name}"), panel.0, panel.1, &px);

    let (w, _) = panel;
    let (x0, y0) = card_origin(panel);
    let (x0, y0) = (x0 as u32, y0 as u32);
    let mid = x0 + CARD_W / 2;
    let edge = at(&px, w, mid, y0);
    assert!(
        edge.iter()
            .zip([0x2a, 0x27, 0x33])
            .all(|(got, want)| got.abs_diff(want) <= 8),
        "{name}: the card's top edge is {edge:?}, not the border color"
    );
    let outside = (0..x0.saturating_sub(4))
        .flat_map(|x| (y0 + 120..y0 + 260).map(move |y| (x, y)))
        .max_by_key(|&(x, y)| sum(at(&before, w, x, y)))
        .expect("a shelf pixel");
    let (lit, dimmed) = (
        sum(at(&before, w, outside.0, outside.1)),
        sum(at(&px, w, outside.0, outside.1)),
    );
    assert!(
        lit > 90,
        "{name}: the shelf beside the card is dark already"
    );
    assert!(
        dimmed as f32 <= lit as f32 * 0.45,
        "{name}: the shelf at {outside:?} went from {lit} to {dimmed}, not dimmed 55%"
    );
    let band = (y0 + 166..y0 + 186)
        .flat_map(|y| (x0 + 20..x0 + CARD_W - 20).map(move |x| (x, y)))
        .filter(|&(x, y)| at(&px, w, x, y).iter().any(|v| *v > 0x60))
        .count();
    assert!(band > 100, "{name}: line one has {band} inked pixels");
    let bottom = at(&px, w, mid, y0 + CARD_H - 1);
    assert!(
        bottom
            .iter()
            .zip([0x2a, 0x27, 0x33])
            .all(|(got, want)| got.abs_diff(want) <= 8),
        "{name}: the card's bottom edge is {bottom:?}"
    );
    let lit_in = |xs: std::ops::Range<u32>, ys: std::ops::Range<u32>| {
        ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
            .filter(|&(x, y)| sum(at(&px, w, x, y)) > 150)
            .count()
    };
    let beside = lit_in(mid - 80..mid - 32, y0 + 50..y0 + 140)
        + lit_in(mid + 32..mid + 80, y0 + 50..y0 + 140);
    let middle = lit_in(mid - 20..mid + 20, y0 + 50..y0 + 140);
    let above = lit_in(mid - 80..mid + 80, y0 + 4..y0 + 36);
    match kind {
        LinkKind::Cable => {
            assert!(middle > 200, "{name}: no plug in the middle ({middle})");
            assert!(
                beside == 0,
                "{name}: something wider than the plug ({beside})"
            );
        }
        LinkKind::Wireless => {
            assert!(beside > 1000, "{name}: no adapter body ({beside})");
            assert!(
                above == 0,
                "{name}: art above the adapter, a cable still hangs from the top ({above})"
            );
        }
    }
}

#[test]
fn the_card_on_the_shelf_at_720() {
    render_card((720, 480), "720", LinkKind::Cable);
}

#[test]
fn the_card_on_the_shelf_at_640() {
    render_card((640, 480), "640", LinkKind::Cable);
}

#[test]
fn a_wireless_card_on_the_shelf_at_720() {
    render_card((720, 480), "wireless-720", LinkKind::Wireless);
}

#[test]
fn a_wireless_card_on_the_shelf_at_640() {
    render_card((640, 480), "wireless-640", LinkKind::Wireless);
}
