#![cfg(feature = "nds")]

use slot::studio_view::{layout, shelf_rect, well_of};
use slot_store::{Cart, Platform};

fn ds_cart() -> Cart {
    Cart {
        platform: Platform::Nds,
        stem: "Kart".into(),
        rom: "Games/NDS/Kart.nds".into(),
        label: None,
        title: String::new(),
        code: String::new(),
        shell: None,
    }
}

#[test]
fn the_studio_draws_a_ds_card_at_its_own_shape() {
    let (w, h) = slot_ui::cart_box(Platform::Nds);
    let want = w as f32 / h as f32;
    for canvas_w in [640.0, 720.0, 800.0] {
        let l = layout(canvas_w, Platform::Nds);
        assert!(
            (l.cart_w / l.cart_h - want).abs() < 0.01,
            "{canvas_w}: {}x{}",
            l.cart_w,
            l.cart_h
        );
    }
    let canvas = slot_gfx::Canvas { w: 720.0, h: 480.0 };
    let [_, _, sw, sh] = shelf_rect(canvas, &ds_cart());
    assert_eq!((sw as u32, sh as u32), (w, h));
}

#[test]
fn the_studio_prints_a_ds_label_at_the_ds_well() {
    assert_eq!(
        well_of(&ds_cart()),
        [
            slot_ui::DS_LABEL_X,
            slot_ui::DS_LABEL_Y,
            slot_ui::DS_LABEL_W,
            slot_ui::DS_LABEL_H
        ]
    );
}
