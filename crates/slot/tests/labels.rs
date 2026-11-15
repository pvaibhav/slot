mod common;

use slot::app::App;
use slot_gfx::TexId;
use slot_store::Cart;

fn cart(stem: &str) -> Cart {
    Cart {
        tags: Vec::new(),
        platform: slot_store::Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        title: "".into(),
        code: "".into(),
        shell: None,
        sealed: false,
    }
}

#[test]
fn a_completed_label_reuses_the_texture_and_preserves_selection() {
    let carts = vec![cart("Advance Wars"), cart("Metroid")];
    let mut app = App::new(carts.clone());
    let a = TexId::from_raw(4);
    let b = TexId::from_raw(5);
    app.set_faces(vec![a, b]);
    let selection = app.selected_stem().map(str::to_owned);
    let phase = std::mem::discriminant(app.phase());
    let label = "Labels/GBA/Metroid.png".into();
    assert_eq!(app.attach_label(&carts[1].rom, label), vec![b]);
    assert_eq!(
        app.carts().nth(1).unwrap().label.as_deref(),
        Some(std::path::Path::new("Labels/GBA/Metroid.png"))
    );
    assert_eq!(app.selected_stem(), selection.as_deref());
    assert_eq!(std::mem::discriminant(app.phase()), phase);
    assert!(app
        .attach_label(&carts[1].rom, "stale.png".into())
        .is_empty());
    assert!(app
        .attach_label(
            std::path::Path::new("Other/Metroid.gba"),
            "wrong.png".into()
        )
        .is_empty());
}

#[test]
fn existing_custom_labels_are_preserved() {
    let mut cart = cart("Custom");
    cart.label = Some("Labels/GBA/Custom.png".into());
    let mut app = App::new(vec![cart.clone()]);
    app.set_faces(vec![TexId::from_raw(9)]);
    assert!(app
        .attach_label(&cart.rom, "download.png".into())
        .is_empty());
    assert_eq!(app.carts().next().unwrap().label, cart.label);
}

#[test]
fn a_download_updates_the_platform_favorites_and_recents_copies() {
    use slot_store::{write_slot_state, Platform, Recent, SlotState};
    let d = common::tmp_root_with_carts(&["Metroid"]);
    let entry = Recent {
        platform: Platform::Gba,
        stem: "Metroid".into(),
    };
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            show_recents: true,
            recents: vec![entry.clone()],
            favorites: vec![entry],
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    let rom = app.carts().next().unwrap().rom.clone();
    let label = d.path().join("Labels/GBA/Metroid.png");
    app.attach_label(&rom, label.clone());
    let copies: Vec<_> = (0..5).filter_map(|shelf| app.face_cart(shelf, 0)).collect();
    assert_eq!(copies.len(), 3);
    for c in copies {
        assert_eq!(
            c.label.as_ref(),
            Some(&label),
            "{} is missing its downloaded label",
            c.rom.display()
        );
    }
}

#[test]
fn same_named_favorites_from_different_platforms_keep_their_own_texture() {
    use slot_store::{write_slot_state, Platform, Recent, SlotState};
    let d = common::tmp_root_with_carts(&["Metroid"]);
    common::write_gb_cart(&d, "Metroid", "METROID");
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            favorites: [Platform::Gba, Platform::Gb]
                .map(|platform| Recent {
                    platform,
                    stem: "Metroid".into(),
                })
                .to_vec(),
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    let gb = (0..2)
        .find(|&i| app.face_cart(1, i).unwrap().platform == Platform::Gb)
        .unwrap();
    let other = 1 - gb;
    let wanted = TexId::from_raw(42);
    app.set_cart_face(1, gb, wanted);
    app.set_cart_face(1, other, TexId::from_raw(43));
    let rom = app.face_cart(1, gb).unwrap().rom.clone();
    assert_eq!(
        app.attach_label(&rom, d.path().join("Labels/GB/Metroid.png")),
        vec![wanted]
    );
    assert!(app.face_cart(1, other).unwrap().label.is_none());
}
