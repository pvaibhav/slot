#![cfg(all(target_os = "macos", feature = "nds"))]

mod common;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn tap(f: &mut Frontend, input: &mut Script, btn: Btn) {
    input.0.push_back(vec![RawEvent::Down(btn)]);
    f.advance(input);
    input.0.push_back(vec![RawEvent::Up(btn)]);
    f.advance(input);
}

fn run_for(f: &mut Frontend, input: &mut Script, secs: f32) {
    let until = Instant::now() + Duration::from_secs_f32(secs);
    while Instant::now() < until {
        f.advance(input);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn span(px: &[u8], w: usize, h: usize, channel: usize) -> (u8, u8) {
    let (mut lo, mut hi) = (255u8, 0u8);
    for y in (h / 6..h * 3 / 4).step_by(3) {
        for x in (w / 70..w / 4).step_by(5) {
            let v = px[(y * w + x) * 4 + channel];
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    (lo, hi)
}

#[test]
fn a_ds_game_draws_direct_and_shows_its_bottom_screen_under_the_menu() {
    std::env::remove_var("SLOT_CORE");
    let surface = HeadlessSurface::new().expect("headless gl");
    let mut c = Compositor::new(&surface).expect("compositor");
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    assert!(f.ds_direct());
    assert!(
        !f.app().records_rewind(),
        "a DS state is 6 MB; rewind snapshots every 2 frames would stall the RG DS+"
    );
    f.compose(&mut c);
    let frame = f.ds_frame().expect("a DS game is playing");
    assert_eq!(frame.len(), 256 * 384 * 4);
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let (w, h) = c.frame_size();
        let px = c.read_frame();
        let file = std::fs::File::create(format!("{dir}/nds-play.png")).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }

    tap(&mut f, &mut input, Btn::Menu);
    run_for(&mut f, &mut input, 0.6);
    assert!(f.app().in_game_menu().is_some());
    assert!(!f.ds_direct());
    assert!(f.ds_menu_below(), "the menu over a DS game is not on the bottom screen");
    f.compose(&mut c);
    let (w, h) = c.frame_size();
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let file = std::fs::File::create(format!("{dir}/nds-menu.png")).unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w, h);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
    let (w, h) = (w as usize, h as usize);
    let (rlo, rhi) = span(&px, w, h, 0);
    let (blo, bhi) = span(&px, w, h, 2);
    assert!(
        rhi > rlo + 8 || bhi > blo + 8,
        "the game area under the menu is flat: red {rlo}..{rhi} blue {blo}..{bhi}"
    );
    let (_, ghi) = span(&px, w, h, 1);
    assert!(
        ghi > 40,
        "green only reached {ghi}: the menu is not over the DS bottom screen"
    );
}

#[test]
fn a_ds_game_plays_its_top_screen_through_the_compositor_so_the_hud_lands_on_it() {
    std::env::remove_var("SLOT_CORE");
    let surface = HeadlessSurface::new().expect("headless gl");
    let mut c = Compositor::new(&surface).expect("compositor");
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    assert!(f.ds_direct());
    f.compose(&mut c);
    let (w, h) = c.frame_size();
    let px = c.read_frame();
    let (w, h) = (w as usize, h as usize);
    let (_, ghi) = span(&px, w, h, 1);
    assert!(ghi > 20, "the top screen is not drawn: green only reached {ghi}");
    assert!(ghi < 140, "green reached {ghi}: the bottom screen is drawn where the top belongs");
}

#[test]
fn a_ds_picture_keeps_its_shape_while_the_cart_ejects() {
    std::env::remove_var("SLOT_CORE");
    let surface = HeadlessSurface::new().expect("headless gl");
    let mut c = Compositor::new(&surface).expect("compositor");
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    std::fs::write(d.path().join("Games/NDS/Tetris.nds"), vec![0u8; 0x200]).unwrap();
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    assert!(f.ds_direct());
    f.compose(&mut c);
    f.app_mut().apply(slot_input::Action::Eject);
    f.advance(&mut input);
    assert!(matches!(f.app().phase(), slot::app::Phase::Ejecting { .. }), "{:?}", f.app().phase());
    assert!(!f.ds_menu_below(), "the eject is drawn on the bottom screen as if the menu were open");
    f.compose(&mut c);
    let (w, h) = c.frame_size();
    let px = c.read_frame();
    let (w, h) = (w as usize, h as usize);
    let x = w / 2;
    let reds: Vec<u8> = (h * 3 / 10..h * 6 / 10).step_by(h / 40).map(|y| px[(y * w + x) * 4]).collect();
    let (lo, hi) = (*reds.iter().min().unwrap(), *reds.iter().max().unwrap());
    assert!(hi - lo <= 6, "red runs {lo}..{hi} down one column: the DS picture is sheared while ejecting");
}

#[test]
fn a_ds_overlay_stays_on_the_top_screen_while_the_game_plays() {
    std::env::remove_var("SLOT_CORE");
    let surface = HeadlessSurface::new().expect("headless gl");
    let mut c = Compositor::new(&surface).expect("compositor");
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    let tile = d.path().join("Overlays/NDS/grid_tile.png");
    std::fs::create_dir_all(tile.parent().unwrap()).unwrap();
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 2, 2);
    enc.set_color(png::ColorType::Rgba);
    let mut writer = enc.write_header().unwrap();
    let mut px = vec![0u8; 16];
    px[..4].copy_from_slice(&[0, 255, 0, 255]);
    writer.write_image_data(&px).unwrap();
    drop(writer);
    std::fs::write(&tile, out).unwrap();
    let state = slot_store::read_slot_state(d.path());
    slot_store::write_slot_state(
        d.path(),
        &slot_store::SlotState {
            overlay_nds: Some("grid".into()),
            ..state
        },
    )
    .unwrap();
    clocked(d.path());
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    tap(&mut f, &mut input, Btn::A);
    run_for(&mut f, &mut input, 4.0);
    assert!(f.ds_direct());
    let mut green = 0;
    for _ in 0..50 {
        f.compose(&mut c);
        let px = c.read_frame();
        green = px.chunks_exact(4).filter(|p| p[..3] == [0, 255, 0]).count();
        if green > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(green > 1000, "only {green} overlay pixels on the top screen while the DS game plays");
}
