use slot_store::{Cart, Outline, Platform, ShellChoice, ShellFinish};
use slot_ui::{bare_face, cart_face_with, gb_shell_of, CartFace, GbShell};

const STEMS: [&str; 16] = [
    "Minish Cap",
    "Mother 3",
    "MKSC",
    "Bootleg 4 in 1",
    "Tetris",
    "Pokemon Crystal",
    "Golden Sun",
    "Metroid Fusion",
    "Advance Wars",
    "Kirby",
    "Link's Awakening DX",
    "Super Mario Land 2",
    "Shantae",
    "Wario Land 3",
    "Zelda Oracle of Ages",
    "Drill Dozer",
];

fn cart(platform: Platform, stem: &str, colour: [u8; 3]) -> Cart {
    Cart {
        platform,
        stem: stem.into(),
        rom: format!("Games/X/{stem}.gba").into(),
        label: None,
        code: String::new(),
        shell: Some(ShellChoice {
            outline: Outline::Auto,
            colour,
            finish: ShellFinish::Solid,
        }),
        title: stem.to_uppercase(),
        tags: Vec::new(),
        sealed: false,
    }
}

fn rect(c: &Cart) -> (u32, u32, u32, u32) {
    match gb_shell_of(c) {
        None => (
            slot_ui::LABEL_X,
            slot_ui::LABEL_Y,
            slot_ui::LABEL_W,
            slot_ui::LABEL_H,
        ),
        Some(GbShell::Notched) => (
            slot_ui::GB_LABEL_X,
            slot_ui::GB_LABEL_Y,
            slot_ui::GB_LABEL_W,
            slot_ui::GB_LABEL_H,
        ),
        Some(GbShell::Rounded) => (
            slot_ui::GBC_LABEL_X,
            slot_ui::GBC_LABEL_Y,
            slot_ui::GBC_LABEL_W,
            slot_ui::GBC_LABEL_H,
        ),
    }
}

fn inside(r: (u32, u32, u32, u32), x: u32, y: u32) -> bool {
    (r.0..r.0 + r.2).contains(&x) && (r.1..r.1 + r.3).contains(&y)
}

fn label_pixels(face: &CartFace, r: (u32, u32, u32, u32)) -> Vec<[u8; 4]> {
    let mut out = Vec::new();
    for y in r.1..r.1 + r.3 {
        for x in r.0..r.0 + r.2 {
            let i = ((y * face.w + x) * 4) as usize;
            out.push([
                face.rgba[i],
                face.rgba[i + 1],
                face.rgba[i + 2],
                face.rgba[i + 3],
            ]);
        }
    }
    out
}

fn luma(p: [u8; 4]) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

#[test]
fn no_ink_outside_the_recess() {
    for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        let c = cart(
            platform,
            "Pokemon - Emerald Version (USA, Europe) (Rev 1) (Beta) (Proto)",
            [0x2f, 0x8a, 0x55],
        );
        let face = cart_face_with(&c, None);
        let bare = bare_face(&c);
        let r = rect(&c);
        for y in 0..face.h {
            for x in 0..face.w {
                if inside(r, x, y) {
                    continue;
                }
                let i = ((y * face.w + x) * 4) as usize;
                assert_eq!(
                    face.rgba[i..i + 4],
                    bare.rgba[i..i + 4],
                    "{platform:?} differs at {x},{y}"
                );
            }
        }
    }
}

#[test]
fn plastic_ink_follows_the_shell() {
    let stem = STEMS[0];
    let dark = cart(Platform::Gba, stem, [0x30, 0x31, 0x36]);
    let light = cart(Platform::Gba, stem, [0xe6, 0xe0, 0xd2]);
    let r = rect(&dark);
    let bright = label_pixels(&cart_face_with(&dark, None), r)
        .into_iter()
        .map(luma)
        .fold(0.0, f32::max);
    let deep = label_pixels(&cart_face_with(&light, None), r)
        .into_iter()
        .map(luma)
        .fold(255.0, f32::min);
    assert!(
        bright > 200.0,
        "white ink on the dark shell, brightest {bright}"
    );
    assert!(deep < 60.0, "black ink on the light shell, darkest {deep}");
}

#[test]
fn the_ink_draws_on_every_platform() {
    for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        for stem in STEMS {
            let c = cart(platform, stem, [0xb0, 0x3a, 0x90]);
            let r = rect(&c);
            let drawn = label_pixels(&cart_face_with(&c, None), r);
            let bare = label_pixels(&bare_face(&c), r);
            let changed = drawn.iter().zip(&bare).filter(|(a, b)| a != b).count();
            let total = (r.2 * r.3) as usize;
            assert!(
                changed > total / 50,
                "{platform:?} {stem} changed only {changed} pixels"
            );
            assert!(
                changed < total * 4 / 5,
                "{platform:?} {stem} hides the recess, {changed} of {total} changed"
            );
        }
    }
}

#[test]
fn the_tag_line_is_written_only_for_a_cart_that_carries_tags() {
    let plain = cart(Platform::Gba, "Mother 3 (Translated)", [40, 40, 40]);
    let tagged = Cart {
        sealed: false,
        tags: vec!["Translated".into()],
        ..plain.clone()
    };
    assert_ne!(
        cart_face_with(&tagged, None).rgba,
        cart_face_with(&plain, None).rgba
    );
}

#[test]
fn carts_sharing_a_title_get_their_own_handwriting() {
    let a = cart(Platform::Gba, "Mother 3 (Japan)", [40, 40, 40]);
    let b = cart(Platform::Gba, "Mother 3 (Japan) (Translated)", [40, 40, 40]);
    assert_ne!(cart_face_with(&a, None).rgba, cart_face_with(&b, None).rgba);
    assert_eq!(
        cart_face_with(&a, None).rgba,
        cart_face_with(&a.clone(), None).rgba
    );
}

#[test]
fn ink_is_opaque_on_a_clear_shell() {
    let mut c = cart(Platform::Gba, "Mother 3", [90, 160, 220]);
    c.shell = Some(ShellChoice {
        outline: Outline::Auto,
        colour: [90, 160, 220],
        finish: ShellFinish::Clear,
    });
    let face = cart_face_with(&c, None);
    let reveal = slot_ui::hand_reveal(&c, 4).expect("handwriting");
    let (ink, _) = reveal.frames.last().expect("a frame");
    let mut checked = 0;
    for y in 0..reveal.h {
        for x in 0..reveal.w {
            if ink[((y * reveal.w + x) * 4 + 3) as usize] < 255 {
                continue;
            }
            let at = (((y + slot_ui::LABEL_Y) * face.w + x + slot_ui::LABEL_X) * 4 + 3) as usize;
            checked += 1;
            assert_eq!(face.rgba[at], 255, "ink at {x},{y} is see-through");
        }
    }
    assert!(checked > 100);
}
