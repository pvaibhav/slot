use slot::app::App;
use slot_store::{Cart, Platform};
use slot_ui::TexId;

fn cart(platform: Platform, stem: &str) -> Cart {
    Cart {
        platform,
        stem: stem.into(),
        rom: format!("/x/{stem}.gba").into(),
        label: None,
        title: String::new(),
        code: String::new(),
        shell: None,
        sealed: false,
        tags: Vec::new(),
    }
}

fn library(gba: usize, gb: usize) -> App {
    let mut carts: Vec<Cart> = (0..gba)
        .map(|i| cart(Platform::Gba, &format!("G{i:04}")))
        .collect();
    carts.extend((0..gb).map(|i| cart(Platform::Gb, &format!("B{i:04}"))));
    App::new(carts)
}

#[test]
fn a_big_library_asks_only_for_the_carts_near_the_cursor() {
    let app = library(500, 0);
    let wants = app.face_wants();
    assert_eq!(
        wants.len(),
        17,
        "eight either side of the selected cart, and it"
    );
    let screen: Vec<usize> = wants
        .iter()
        .filter(|w| w.on_screen)
        .map(|w| w.index)
        .collect();
    assert_eq!(screen, vec![0, 1, 499, 2, 498, 3, 497]);
    assert!(
        wants[..7].iter().all(|w| w.on_screen),
        "an off-screen cart was asked for before an on-screen one"
    );
}

#[test]
fn the_neighbouring_shelf_is_asked_for_after_what_is_on_screen() {
    let app = library(40, 30);
    let wants = app.face_wants();
    let gba = wants[0].shelf;
    assert!(wants[..7].iter().all(|w| w.shelf == gba && w.on_screen));
    assert!(
        wants[7..14].iter().all(|w| w.shelf != gba && !w.on_screen),
        "the next shelf's on-screen carts are not queued right after this one's"
    );
}

#[test]
fn a_face_already_held_is_not_asked_for_again() {
    let mut app = library(500, 0);
    let gba = app.face_wants()[0].shelf;
    app.set_cart_face(gba, 0, TexId::from_raw(1));
    assert!(app.face_wants().iter().all(|w| w.index != 0));
}

#[test]
fn faces_far_from_the_cursor_are_shed_and_near_ones_kept() {
    let mut app = library(500, 0);
    let gba = app.face_wants()[0].shelf;
    app.set_cart_face(gba, 0, TexId::from_raw(1));
    app.set_cart_face(gba, 12, TexId::from_raw(2));
    app.set_cart_face(gba, 13, TexId::from_raw(3));
    app.set_cart_face(gba, 250, TexId::from_raw(4));
    let mut shed = app.shed_faces();
    shed.sort_by_key(|t| format!("{t:?}"));
    assert_eq!(shed, vec![TexId::from_raw(3), TexId::from_raw(4)]);
    assert!(app.shed_faces().is_empty());
}

#[test]
fn sealing_a_cart_rebuilds_only_its_face_and_reports_it_once() {
    let mut app = library(3, 0);
    let gba = app.face_wants()[0].shelf;
    for i in 0..3 {
        app.set_cart_face(gba, i, TexId::from_raw(i + 1));
    }
    let ready = std::collections::HashSet::from([std::path::PathBuf::from("/x/G0001.gba")]);
    assert_eq!(app.seal(&ready, true), vec!["G0001".to_string()]);
    assert!(app.face_cart(gba, 1).unwrap().sealed);
    assert!(!app.face_cart(gba, 0).unwrap().sealed);
    let wanted: Vec<usize> = app.face_wants().iter().map(|w| w.index).collect();
    assert_eq!(wanted, vec![1], "only the sealed cart is redrawn");
    assert_eq!(app.shed_faces(), vec![TexId::from_raw(2)]);
    assert!(app.seal(&ready, true).is_empty(), "no change, no redraw");
    assert_eq!(
        app.seal(&Default::default(), false),
        vec!["G0001".to_string()]
    );
    assert!(!app.face_cart(gba, 1).unwrap().sealed);
}

#[test]
fn a_label_arriving_for_a_sealed_cart_rebuilds_its_face_with_the_seal() {
    let mut app = library(2, 0);
    let gba = app.face_wants()[0].shelf;
    app.set_cart_face(gba, 0, TexId::from_raw(1));
    app.set_cart_face(gba, 1, TexId::from_raw(2));
    let rom = std::path::PathBuf::from("/x/G0000.gba");
    app.seal(&std::collections::HashSet::from([rom.clone()]), false);
    app.set_cart_face(gba, 0, TexId::from_raw(3));
    // The labels worker drew its face without the seal, so it must not be uploaded in place.
    assert!(app.attach_label(&rom, "/l/G0000.png".into()).is_empty());
    assert_eq!(
        app.face_cart(gba, 0).unwrap().label,
        Some("/l/G0000.png".into())
    );
    assert!(app.face_wants().iter().any(|w| w.index == 0));
    let unsealed = std::path::PathBuf::from("/x/G0001.gba");
    assert_eq!(
        app.attach_label(&unsealed, "/l/G0001.png".into()),
        vec![TexId::from_raw(2)]
    );
}

fn glints(app: &App, frames: &[TexId]) -> usize {
    let mut out = Vec::new();
    app.draw(&mut out);
    out.iter()
        .filter(|d| matches!(d, slot_ui::Draw::Tex { tex, .. } if frames.contains(tex)))
        .count()
}

fn press(app: &mut App, btn: slot_input::Btn) {
    app.apply(slot_input::Action::GbaDown(btn));
    app.apply(slot_input::Action::GbaUp(btn));
    app.update(0.05);
}

#[test]
fn only_a_cart_sealed_in_view_glints() {
    let frames: Vec<TexId> = (0..slot_ui::GLINT_FRAMES)
        .map(|i| TexId::from_raw(100 + i))
        .collect();
    let mut app = library(40, 0);
    app.set_glint_faces(frames.clone());
    app.update(1.0);
    let off_screen = std::path::PathBuf::from("/x/G0020.gba");
    app.seal(&std::collections::HashSet::from([off_screen.clone()]), true);
    // Reaching it is covered by app::seal_tests, which skips the scroll and its clock.
    for _ in 0..20 {
        press(&mut app, slot_input::Btn::Right);
    }
    for _ in 0..60 {
        app.update(1.0 / 60.0);
    }
    assert_eq!(app.selected_stem(), Some("G0020"));
    let both = std::collections::HashSet::from([off_screen, "/x/G0019.gba".into()]);
    app.seal(&both, false);
    app.update(0.05);
    assert_eq!(
        glints(&app, &frames),
        0,
        "the first scan restores, it does not award"
    );
    let in_view = std::collections::HashSet::from(["/x/G0021.gba".into()]);
    app.seal(&in_view, true);
    app.update(0.05);
    assert_eq!(glints(&app, &frames), 1);
    app.seal(&Default::default(), true);
    assert_eq!(glints(&app, &frames), 0, "unsealing stops the glint");
}

#[test]
fn a_glint_cut_short_by_an_insert_does_not_resume_after_the_eject() {
    let frames: Vec<TexId> = (0..slot_ui::GLINT_FRAMES)
        .map(|i| TexId::from_raw(100 + i))
        .collect();
    let mut app = library(3, 0);
    app.set_glint_faces(frames.clone());
    app.update(1.0);
    app.seal(
        &std::collections::HashSet::from(["/x/G0001.gba".into()]),
        true,
    );
    app.update(0.05);
    assert_eq!(glints(&app, &frames), 1);
    app.apply(slot_input::Action::Insert);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    app.apply(slot_input::Action::Eject);
    for _ in 0..240 {
        app.update(1.0 / 60.0);
        assert_eq!(
            glints(&app, &frames),
            0,
            "the glint came back after the game"
        );
    }
}
