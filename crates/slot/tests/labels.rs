use slot::app::App;
use slot_gfx::TexId;
use slot_store::Cart;

fn cart(stem: &str) -> Cart {
    Cart {
        platform: slot_store::Platform::Gba,
        stem: stem.into(),
        rom: format!("Games/GBA/{stem}.gba").into(),
        label: None,
        title: "".into(),
        code: "".into(),
        shell: None,
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
    assert_eq!(app.attach_label(&carts[1].rom, label), Some(b));
    assert_eq!(
        app.carts().nth(1).unwrap().label.as_deref(),
        Some(std::path::Path::new("Labels/GBA/Metroid.png"))
    );
    assert_eq!(app.selected_stem(), selection.as_deref());
    assert_eq!(std::mem::discriminant(app.phase()), phase);
    assert!(app
        .attach_label(&carts[1].rom, "stale.png".into())
        .is_none());
    assert!(app
        .attach_label(
            std::path::Path::new("Other/Metroid.gba"),
            "wrong.png".into()
        )
        .is_none());
}

#[test]
fn existing_custom_labels_are_preserved() {
    let mut cart = cart("Custom");
    cart.label = Some("Labels/GBA/Custom.png".into());
    let mut app = App::new(vec![cart.clone()]);
    app.set_faces(vec![TexId::from_raw(9)]);
    assert!(app.attach_label(&cart.rom, "download.png".into()).is_none());
    assert_eq!(app.carts().next().unwrap().label, cart.label);
}
