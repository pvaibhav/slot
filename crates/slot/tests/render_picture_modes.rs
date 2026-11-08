#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;
use std::path::Path;
use std::time::{Duration, Instant};

use common::{clocked, core_lock, repo_root, tmp_root_with_gb_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent};
use slot_power::SimPlatform;
use slot_store::{Core, Platform};

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

fn at(px: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [px[o], px[o + 1], px[o + 2]]
}

const BAR_W: usize = 120;
const BAR_H: usize = 24;

fn lit(px: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|(x, y)| at(px, *x, *y).iter().any(|c| *c > 8))
        .count()
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

fn side_by_side(left: &[u8], right: &[u8], gap: usize) -> (u32, u32, Vec<u8>) {
    let w = OUT_W as usize * 2 + gap;
    let mut out = vec![0u8; w * OUT_H as usize * 4];
    for y in 0..OUT_H as usize {
        let row = y * w * 4;
        let src = y * OUT_W as usize * 4;
        let line = OUT_W as usize * 4;
        out[row..row + line].copy_from_slice(&left[src..src + line]);
        let over = row + line;
        for p in out[over..over + gap * 4].chunks_exact_mut(4) {
            p.copy_from_slice(&[0x60, 0x60, 0x60, 0xff]);
        }
        let from = over + gap * 4;
        out[from..from + line].copy_from_slice(&right[src..src + line]);
    }
    (w as u32, OUT_H, out)
}

fn zoom(
    left: &[u8],
    right: &[u8],
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    n: usize,
) -> (u32, u32, Vec<u8>) {
    let gap = 12;
    let out_w = w * n * 2 + gap;
    let out_h = h * n;
    let mut out = vec![0u8; out_w * out_h * 4];
    for y in 0..out_h {
        for x in 0..out_w {
            let (src, sx) = match x < w * n {
                true => (left, x / n),
                false if x < w * n + gap => {
                    let o = (y * out_w + x) * 4;
                    out[o..o + 4].copy_from_slice(&[0x60, 0x60, 0x60, 0xff]);
                    continue;
                }
                false => (right, (x - w * n - gap) / n),
            };
            let c = at(src, x0 + sx, y0 + y / n);
            let o = (y * out_w + x) * 4;
            out[o..o + 4].copy_from_slice(&[c[0], c[1], c[2], 0xff]);
        }
    }
    (out_w as u32, out_h as u32, out)
}

fn put_card_cart(root: &Path) -> Option<()> {
    let from = repo_root().join("sdcard/Games/GB/Tetris Rosy Retrospection.gb");
    let rom = std::fs::read(&from).ok()?;
    std::fs::write(root.join("Games/GB/Tetris Rosy Retrospection.gb"), rom)
        .expect("copy the card's cart");
    Some(())
}

fn run_for(f: &mut Frontend, input: &mut Script, secs: f32) {
    let until = Instant::now() + Duration::from_secs_f32(secs);
    while Instant::now() < until {
        f.advance(input);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn l_widens_the_picture_like_a_gba_and_r_gives_back_the_centred_picture() {
    let _core = core_lock();
    let core = Core::Mgba;
    let file = format!(
        "{}_libretro.{}",
        core.as_str(),
        std::env::consts::DLL_EXTENSION
    );
    let dylib = repo_root().join("vendor").join(&file);
    if !dylib.exists() {
        eprintln!("no {} dylib, skipping", core.as_str());
        return;
    }
    let Ok(surface) = HeadlessSurface::new() else {
        return;
    };
    let Ok(mut c) = Compositor::new(&surface) else {
        return;
    };

    let d = tmp_root_with_gb_carts(&["Zzz"]);
    let Some(()) = put_card_cart(d.path()) else {
        eprintln!("no Game Boy cart on this machine's card, skipping");
        return;
    };
    std::fs::copy(&dylib, d.path().join("System").join(&file)).expect("plant the core");
    clocked(d.path());
    slot_store::write_selected_core(d.path(), Platform::Gb, "Tetris Rosy Retrospection", Core::Mgba)
        .expect("write core");

    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut c);
    let mut input = Script(VecDeque::new());
    input.0.push_back(vec![RawEvent::Down(Btn::A)]);
    f.advance(&mut input);
    input.0.push_back(vec![RawEvent::Up(Btn::A)]);
    f.advance(&mut input);
    run_for(&mut f, &mut input, 5.0);

    f.compose(&mut c);
    let actual = c.read_frame();
    write_png("picture-actual", OUT_W, OUT_H, &actual);

    let mid_y = OUT_H as usize / 2 - 40..OUT_H as usize / 2 + 40;
    assert!(
        lit(&actual, BAR_W..OUT_W as usize - BAR_W, mid_y.clone()) > 1000,
        "nothing is playing: the panel is dark where the picture should be"
    );
    assert_eq!(
        lit(&actual, 0..BAR_W, 0..OUT_H as usize),
        0,
        "actual size has something in the left margin"
    );
    assert_eq!(
        lit(
            &actual,
            OUT_W as usize - BAR_W..OUT_W as usize,
            0..OUT_H as usize
        ),
        0,
        "actual size has something in the right margin"
    );
    assert_eq!(
        lit(&actual, 0..OUT_W as usize, 0..BAR_H),
        0,
        "actual size has something above the picture"
    );

    input.0.push_back(vec![RawEvent::Down(Btn::L1)]);
    f.advance(&mut input);
    input.0.push_back(vec![RawEvent::Up(Btn::L1)]);
    run_for(&mut f, &mut input, 1.0);
    f.compose(&mut c);
    let stretched = c.read_frame();
    write_png("picture-stretch", OUT_W, OUT_H, &stretched);

    let left = lit(&stretched, 0..BAR_W, 0..OUT_H as usize);
    let top = lit(&stretched, 0..OUT_W as usize, 0..BAR_H);
    assert!(
        left > BAR_W * OUT_H as usize / 4,
        "the left margin is still mostly empty in fullscreen: {left} lit"
    );
    assert_eq!(
        top, 0,
        "the stretch grew the picture upward; a GBA keeps the 144 lines"
    );

    let (w, h, both) = side_by_side(&actual, &stretched, 8);
    write_png("picture-side-by-side", w, h, &both);
    let (w, h, close) = zoom(&actual, &stretched, 300, 200, 90, 60, 6);
    write_png("picture-zoom", w, h, &close);

    input.0.push_back(vec![RawEvent::Down(Btn::R1)]);
    f.advance(&mut input);
    input.0.push_back(vec![RawEvent::Up(Btn::R1)]);
    run_for(&mut f, &mut input, 1.0);
    f.compose(&mut c);
    let back = c.read_frame();
    write_png("picture-back", OUT_W, OUT_H, &back);
    assert_eq!(
        lit(&back, 0..BAR_W, 0..OUT_H as usize),
        0,
        "R did not give the margins back"
    );
    assert!(
        lit(&back, BAR_W..OUT_W as usize - BAR_W, mid_y) > 1000,
        "R left the panel dark"
    );
}
