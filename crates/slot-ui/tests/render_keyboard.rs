#![cfg(target_os = "macos")]

use std::sync::{Mutex, MutexGuard, PoisonError};

use slot_gfx::{Canvas, Compositor, HeadlessSurface};
use slot_input::Btn;
use slot_ui::{
    default_background, draw_empty_slot, draw_keyboard, draw_keyboard_legends, key_rect,
    keyboard_faces, slot_back, slot_front, Cap, Draw, Keyboard, Layout, Special, OUT_H, OUT_W,
    SLOT_H, SLOT_W,
};

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn shot(px: &[u8], name: &str) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let path = format!("{dir}/{name}.png");
    let file = std::fs::File::create(&path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(px)
        .expect("png data");
    println!("wrote {path}");
}

fn rendered(c: &mut Compositor, kb: &Keyboard) -> Vec<u8> {
    let canvas = Canvas::FRAMED;
    let ground = default_background().map(|g| slot_ui::MenuGround {
        tex: c.create_texture(OUT_W, OUT_H, g),
        w: OUT_W,
        tint: 0.0,
    });
    let back = c.create_texture(SLOT_W, SLOT_H, slot_back());
    let front = c.create_texture(SLOT_W, SLOT_H, slot_front());
    let tex = keyboard_faces(kb, canvas).upload(|f| c.create_texture(f.w, f.h, &f.rgba));
    let mut list: Vec<Draw> = Vec::new();
    draw_keyboard(canvas, ground, &tex, kb, &mut list);
    if !kb.help() {
        draw_empty_slot(canvas, Some((back, front)), &mut list);
        draw_keyboard_legends(canvas, &tex, &mut list);
    }
    c.begin_frame();
    c.draw_list(&list);
    c.read_frame()
}

fn luma(px: &[u8], x: usize, y: usize) -> f32 {
    let o = (y * OUT_W as usize + x) * 4;
    (px[o] as f32 * 0.3 + px[o + 1] as f32 * 0.59 + px[o + 2] as f32 * 0.11) / 255.0
}

fn tile(px: &[u8], kb: &Keyboard, row: usize, col: usize) -> (f32, usize, usize) {
    let [x, y, w, h] = key_rect(Canvas::FRAMED, kb.layout(), row, col);
    let (x0, y0, x1, y1) = (x as usize, y as usize, (x + w) as usize, (y + h) as usize);
    let (mut sum, mut light, mut dark, mut n) = (0.0, 0, 0, 0);
    for yy in y0 + 4..y1 - 4 {
        for xx in x0 + 4..x1 - 4 {
            let l = luma(px, xx, yy);
            sum += l;
            n += 1;
            light += (l > 0.75) as usize;
            dark += (l < 0.25) as usize;
        }
    }
    (sum / n as f32, light, dark)
}

#[test]
fn the_keyboard_renders_with_icons() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let mut kb = Keyboard::new(Layout::General, "Password", Some("HOME"), "hunter22");
    kb.press(Btn::Down);
    kb.press(Btn::Left);
    assert_eq!(kb.rows()[kb.row()][kb.col()], Cap::Special(Special::Done));
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-general");

    let (done, _, done_ink) = tile(&px, &kb, kb.row(), kb.col());
    assert!(done > 0.7, "the chosen key is not filled light: {done}");
    assert!(
        done_ink > 30,
        "the Done key has no dark icon: {done_ink} dark pixels"
    );

    let [x, y, _, h] = key_rect(Canvas::FRAMED, kb.layout(), 1, 0);
    let edge = luma(&px, (x + 5.0) as usize, (y + h / 2.0) as usize);
    assert!(edge < 0.25, "an unchosen key is not a dark tile: {edge}");
    let (q, q_ink, _) = tile(&px, &kb, 1, 0);
    assert!(q < 0.4, "the q key is not dark: {q}");
    assert!(q_ink > 10, "the q key has no light letter: {q_ink}");

    for (row, col) in [(0, 10), (3, 0), (4, 0)] {
        let (_, ink, _) = tile(&px, &kb, row, col);
        assert!(
            ink > 20,
            "the icon on key {row},{col} drew {ink} light pixels"
        );
    }
}

#[test]
fn the_number_pad_and_its_help_render() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let mut kb = Keyboard::new(Layout::Numeric, "Port", None, "");
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-numeric");
    let (seven, _, _) = tile(&px, &kb, 0, 0);
    assert!(seven > 0.7, "the 7 key is not chosen: {seven}");
    let (del, ink, _) = tile(&px, &kb, 0, 3);
    assert!(
        del < 0.4 && ink > 20,
        "the Delete key on the pad: {del}, {ink}"
    );

    kb.press(Btn::Menu);
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-numeric-help");
    help_is_full_screen(&px);
}

fn lit_in(px: &[u8], [x0, y0, x1, y1]: [usize; 4]) -> usize {
    (y0..y1)
        .flat_map(|y| (x0..x1).map(move |x| (x, y)))
        .filter(|&(x, y)| luma(px, x, y) > 0.75)
        .count()
}

fn help_is_full_screen(px: &[u8]) {
    let (w, h) = (OUT_W as usize, OUT_H as usize);
    assert!(lit_in(px, [w / 2 - 80, 0, w / 2 + 80, 40]) > 50, "no title");
    assert!(lit_in(px, [0, h - 40, 140, h]) > 50, "no B Back");
    assert!(
        lit_in(px, [w - 140, h - 40, w, h]) == 0,
        "a legend on the help"
    );
    assert!(
        lit_in(px, [w / 2 - 120, 120, w / 2 + 120, 360]) > 300,
        "no rows"
    );
    for (x, y) in [(20, 200), (w - 20, 300), (60, 149), (w - 60, 149)] {
        assert!(
            luma(px, x, y) < 0.05,
            "the keyboard shows through at {x},{y}"
        );
    }
}

#[test]
fn the_keyboard_help_is_a_full_screen_page_and_menu_is_the_right_legend() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let mut kb = Keyboard::new(Layout::General, "Password", Some("HOME"), "hunter22");
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-legends");
    let (w, h) = (OUT_W as usize, OUT_H as usize);
    assert!(lit_in(&px, [w - 140, h - 40, w, h]) > 50, "no right legend");
    kb.press(Btn::Menu);
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-general-help");
    help_is_full_screen(&px);
}

#[test]
fn a_long_password_keeps_the_cursor_on_screen() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let kb = Keyboard::new(
        Layout::General,
        "Password",
        Some("A very long network name"),
        "correct horse battery staple and then some more words",
    );
    let px = rendered(&mut c, &kb);
    shot(&px, "keyboard-long");
    let lit = (100..146).any(|y| luma(&px, OUT_W as usize - 62, y) > 0.9);
    assert!(lit, "nothing is drawn at the right end of the field");
}
