#![cfg(target_os = "macos")]

use std::sync::{Mutex, MutexGuard, PoisonError};

use slot_gfx::{Canvas, Compositor, HeadlessSurface};
use slot_store::{Cart, Platform};
use slot_ui::{cart_face, cart_shadow, Draw, Shelf, SlotChrome, TexId, CART_W, OUT_H, OUT_W};

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn shelf_with(n: usize) -> Shelf {
    Shelf::new(
        (0..n)
            .map(|i| Cart {
                platform: Platform::Gba,
                stem: format!("Game {i}"),
                rom: format!("Games/GBA/Game {i}.gba").into(),
                label: None,
                code: String::new(),
                shell: None,
                title: format!("GAME {i}"),
                sealed: false,
                tags: Vec::new(),
            })
            .collect(),
    )
}

fn uploaded(n: usize, c: &mut Compositor) -> (Shelf, Vec<TexId>) {
    let mut s = shelf_with(n);
    let faces: Vec<TexId> = s
        .carts
        .iter()
        .map(|cart| {
            let f = cart_face(cart);
            c.create_texture(f.w, f.h, &f.rgba)
        })
        .collect();
    s.set_faces(faces.clone());
    let shadow = cart_shadow();
    let tex = c.create_texture(shadow.w, shadow.h, &shadow.rgba);
    s.set_shadow(tex);
    (s, faces)
}

fn composed(c: &mut Compositor, list: &[Draw]) -> Vec<u8> {
    c.begin_frame();
    c.draw_list(list);
    c.read_frame()
}

fn write_png(px: &[u8], path: &str) {
    let file = std::fs::File::create(path).expect("create png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("png header")
        .write_image_data(px)
        .expect("png data");
    println!("wrote {path}");
}

fn shot(px: &[u8], name: &str) {
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        write_png(px, &format!("{dir}/{name}.png"));
    }
}

const BAND: std::ops::Range<usize> = (OUT_H as usize / 4)..(OUT_H as usize / 2);

fn occupied(px: &[u8], x: usize) -> bool {
    BAND.map(|y| (y * OUT_W as usize + x) * 4)
        .any(|o| px[o] > 0x18 || px[o + 1] > 0x18 || px[o + 2] > 0x18)
}

fn bare_edges(px: &[u8]) -> (usize, usize) {
    let w = OUT_W as usize;
    let left = (0..w).take_while(|x| !occupied(px, *x)).count();
    let right = (0..w).take_while(|x| !occupied(px, w - 1 - *x)).count();
    (left, right)
}

fn held_scroll(c: &mut Compositor, n: usize, frames: usize) -> Vec<Vec<u8>> {
    let (mut s, _) = uploaded(n, c);
    s.select(n - 1);
    s.hold_right(0);
    (0..frames)
        .map(|f| {
            s.tick(f as u64 * 1000 / 60);
            s.update(1.0 / 60.0);
            let mut list = Vec::new();
            s.draw(Canvas::FRAMED, 0.0, None, &mut list);
            composed(c, &list)
        })
        .collect()
}

#[test]
fn a_short_row_leaves_no_more_of_the_panel_bare_than_a_long_one() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    const FRAMES: usize = 120;
    let worst = |frames: &[Vec<u8>]| -> (usize, usize) {
        frames
            .iter()
            .map(|px| bare_edges(px))
            .fold((0, 0), |a, b| (a.0.max(b.0), a.1.max(b.1)))
    };
    let long = held_scroll(&mut c, 10, FRAMES);
    let (lref, rref) = worst(&long);
    shot(&long[1], "row-10-early");
    for n in [2usize, 3, 4, 5, 8] {
        let frames = held_scroll(&mut c, n, FRAMES);
        shot(&frames[1], &format!("row-{n}-early"));
        let (left, right) = worst(&frames);
        assert!(
            left <= lref + 2 && right <= rref + 2,
            "{n} carts left {left} px bare at the left and {right} at the right against a ten \
             cart row's {lref} and {rref}: the row has a hole in it where a cartridge should be \
             leaving the frame"
        );
    }
}

#[test]
fn a_shelf_of_one_does_not_move_when_a_shoulder_is_held() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let frames = held_scroll(&mut c, 1, 120);
    shot(&frames[0], "row-1-first");
    shot(&frames[119], "row-1-last");
    let want = bare_edges(&frames[0]);
    assert_eq!(
        want,
        ((OUT_W - CART_W) as usize / 2, (OUT_W - CART_W) as usize / 2),
        "the lone cart is not standing centred at its own width even before the press"
    );
    for (f, px) in frames.iter().enumerate() {
        assert_eq!(
            bare_edges(px),
            want,
            "frame {f}: the lone cart moved to {:?}",
            bare_edges(px)
        );
    }
}

#[test]
fn the_slot_takes_over_the_cart_where_the_row_had_it() {
    let Some((_g, _s, mut c)) = compositor() else {
        return;
    };
    let (mut s, faces) = uploaded(5, &mut c);
    s.select(0);
    s.right();
    s.update(1.0 / 60.0);
    s.update(1.0 / 60.0);
    let mut before = Vec::new();
    s.draw_row(Canvas::FRAMED, None, 0.0, 0.0, 1.0, &mut before);
    let before = composed(&mut c, &before);
    shot(&before, "handover-before");

    let cart = s.carts[s.index].clone();
    let (rest, scale) = s.selected_at(Canvas::FRAMED);
    let mut after = Vec::new();
    s.draw_row(Canvas::FRAMED, Some(&cart.stem), 0.0, 0.0, 1.0, &mut after);
    SlotChrome {
        cart: &cart,
        face: Some(faces[s.index]),
        rest,
        scale,
        seat: 0.0,
        alert: None,
        dim: 0.0,
        screen: 0.0,
        game: false,
        over: true,
        behind: [None, None],
        slot: None,
        canvas: Canvas::FRAMED,
    }
    .draw(&mut after);
    let after = composed(&mut c, &after);
    shot(&after, "handover-after");

    let columns =
        |px: &[u8]| -> Vec<bool> { (0..OUT_W as usize).map(|x| occupied(px, x)).collect() };
    let (a, b) = (columns(&before), columns(&after));
    let moved = a.iter().zip(&b).filter(|(x, y)| x != y).count();
    assert!(
        moved <= 2,
        "{moved} columns of the row changed on the frame the slot took the cart over: the \
         cartridge jumped rather than being handed across"
    );
}
