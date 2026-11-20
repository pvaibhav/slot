use slot_store::{Cart, Outline, Platform, ShellChoice, ShellFinish};
use slot_ui::{cart_face_with, label_panel, seal_rect, CartFace, GbShell, CART_H, CART_W};

fn cart(platform: Platform, shell: Option<ShellChoice>, sealed: bool) -> Cart {
    Cart {
        tags: Vec::new(),
        platform,
        stem: "Seal Test".into(),
        rom: "Games/GBA/Seal Test.gba".into(),
        label: None,
        code: String::new(),
        shell,
        title: String::new(),
        sealed,
    }
}

/// The box around every pixel the seal changed.
fn changed(plain: &CartFace, sealed: &CartFace) -> (u32, u32, u32, u32) {
    assert_eq!((plain.w, plain.h), (sealed.w, sealed.h));
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for (i, (a, b)) in plain
        .rgba
        .chunks_exact(4)
        .zip(sealed.rgba.chunks_exact(4))
        .enumerate()
    {
        if a != b {
            let (x, y) = (i as u32 % plain.w, i as u32 / plain.w);
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
    }
    assert!(x1 >= x0, "the seal changed nothing");
    (x0, y0, x1, y1)
}

#[test]
fn a_sealed_cart_wears_the_seal_on_its_label_corner_and_nowhere_else() {
    let rounded = ShellChoice {
        outline: Outline::Rounded,
        colour: [0x60, 0x30, 0xa0],
        finish: ShellFinish::Clear,
    };
    for (platform, choice, shell) in [
        (Platform::Gba, None, None),
        (Platform::Gb, None, Some(GbShell::Notched)),
        (Platform::Gbc, Some(rounded), Some(GbShell::Rounded)),
    ] {
        let plain = cart_face_with(&cart(platform, choice, false), None);
        let sealed = cart_face_with(&cart(platform, choice, true), None);
        let (x0, y0, x1, y1) = changed(&plain, &sealed);
        let (sx, sy, sw, sh) = seal_rect(shell);
        let (w, h) = (plain.w as f32, plain.h as f32);
        assert!(
            x0 as f32 >= (sx * w).floor()
                && y0 as f32 >= (sy * h).floor()
                && x1 as f32 <= ((sx + sw) * w).ceil()
                && y1 as f32 <= ((sy + sh) * h).ceil(),
            "{platform:?}: the seal spilled outside its box"
        );
    }
    let (lx, _, _, ly1) = label_panel(CART_W, CART_H);
    let (sx, sy, sw, sh) = seal_rect(None);
    let (sx, sy) = (sx * CART_W as f32, sy * CART_H as f32);
    let (sw, sh) = (sw * CART_W as f32, sh * CART_H as f32);
    assert!(
        sx < lx as f32 && sx + sw > lx as f32 && sy < ly1 as f32 && sy + sh > ly1 as f32,
        "the seal straddles the label's bottom-left corner"
    );
}
