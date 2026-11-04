use slot_store::{Cart, Platform};
use slot_ui::{draw_empty_slot, shelf_cart, Canvas, Draw, Shelf, TexId, CART_W, SLOT_W};

const NATIVE: Canvas = Canvas { w: 640.0, h: 480.0 };

fn carts(stems: &[&str]) -> Vec<Cart> {
    stems
        .iter()
        .map(|stem| Cart {
            platform: Platform::Gba,
            stem: (*stem).to_string(),
            rom: format!("Games/GBA/{stem}.gba").into(),
            label: None,
            code: String::new(),
            shell: None,
            title: stem.to_uppercase(),
            sealed: false,
        })
        .collect()
}

#[test]
fn the_selected_cart_centres_on_a_native_canvas() {
    let shelf = Shelf::new(carts(&["Emerald", "Fusion", "Zelda"]));
    let (x, scale) = shelf.selected_at(NATIVE);
    let w = CART_W as f32 * scale;
    assert_eq!(x + w / 2.0, 320.0);
    let placed = shelf_cart(NATIVE);
    assert_eq!(placed.x + placed.w / 2.0, 320.0);
}

#[test]
fn the_slot_strip_on_a_native_canvas_is_the_centre_of_its_bitmap() {
    let mut out = Vec::new();
    let (back, front) = (TexId::from_raw(1), TexId::from_raw(2));
    draw_empty_slot(NATIVE, Some((back, front)), &mut out);
    let strip = out.iter().find_map(|d| match *d {
        Draw::Part { x, w, tex, uv, .. } if tex == back => Some((x, w, uv)),
        _ => None,
    });
    assert_eq!(strip, Some((0.0, 640.0, NATIVE.window(SLOT_W as f32))));
}

#[test]
fn the_framed_canvas_draws_the_slot_strip_whole_as_today() {
    let mut out = Vec::new();
    let back = TexId::from_raw(1);
    draw_empty_slot(Canvas::FRAMED, Some((back, TexId::from_raw(2))), &mut out);
    assert!(out.iter().any(|d| matches!(*d,
        Draw::Part { x, w, tex, uv, .. }
            if tex == back && x == 0.0 && w == 720.0 && uv == [0.0, 0.0, 1.0, 1.0])));
}

#[test]
fn a_native_wallpaper_is_the_centre_of_the_framed_one() {
    let mut out = Vec::new();
    let ground = TexId::from_raw(3);
    slot_ui::draw_ground(NATIVE, Some((ground, 720)), &mut out);
    assert!(out.iter().any(|d| matches!(*d,
        Draw::Part { w, tex, uv, .. } if tex == ground && w == 640.0 && uv == NATIVE.window(720.0))));
}

#[test]
fn a_panel_sized_wallpaper_is_cropped_top_and_bottom_in_the_frame() {
    let mut out = Vec::new();
    let ground = TexId::from_raw(3);
    slot_ui::draw_ground(Canvas::FRAMED, Some((ground, 640)), &mut out);
    let shown = 640.0 / 720.0;
    assert!(out.iter().any(|d| matches!(*d,
        Draw::Part { w, h, tex, uv, .. }
            if tex == ground && w == 720.0 && h == 480.0
                && uv == [0.0, (1.0 - shown) / 2.0, 1.0, shown])));
}
