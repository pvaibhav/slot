#![cfg(target_os = "macos")]

mod common;

use std::sync::{Mutex, MutexGuard, PoisonError};

use slot::app::App;
use slot_gfx::{Compositor, HeadlessSurface, TexId, OUT_H, OUT_W};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, Appearance, Core, Platform, SlotState};
use slot_ui::{
    board_face, cart_face, cart_shadow, chip_face, chip_shadow_face, edge, gb_cart_shadow, housing,
    opening, padded, recess, socket_face, GbShell, TURN_PAD,
};
use tempfile::TempDir;

static GL: Mutex<()> = Mutex::new(());

fn compositor() -> Option<(MutexGuard<'static, ()>, HeadlessSurface, Compositor)> {
    let guard = GL.lock().unwrap_or_else(PoisonError::into_inner);
    let surface = HeadlessSurface::new().ok()?;
    let compositor = Compositor::new(&surface).ok()?;
    Some((guard, surface, compositor))
}

fn tex(c: &mut Compositor, w: u32, h: u32, rgba: &[u8]) -> TexId {
    c.create_texture(w, h, rgba)
}

fn upload_faces(app: &mut App, c: &mut Compositor) {
    let row: Vec<TexId> = app
        .carts()
        .map(|cart| {
            let f = cart_face(cart);
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    app.set_faces(row);
    let shadow = cart_shadow();
    let shadow = tex(c, shadow.w, shadow.h, &shadow.rgba);
    app.set_cart_shadow(shadow);
    let notched = gb_cart_shadow(GbShell::Notched);
    let notched = tex(c, notched.w, notched.h, &notched.rgba);
    let rounded = gb_cart_shadow(GbShell::Rounded);
    let rounded = tex(c, rounded.w, rounded.h, &rounded.rgba);
    app.set_gb_cart_shadows(notched, rounded);

    let sockets = Core::ALL
        .iter()
        .map(|k| {
            let f = socket_face(*k);
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let chips = Core::ALL
        .iter()
        .map(|k| {
            let f = chip_face(Some(*k));
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let blank = chip_face(None);
    let blank = tex(c, blank.w, blank.h, &blank.rgba);
    let chip_shadow = chip_shadow_face();
    let chip_shadow = tex(c, chip_shadow.w, chip_shadow.h, &chip_shadow.rgba);
    app.set_core_part_faces(sockets, chips, blank, chip_shadow);
    let gb_sockets = Core::ALL
        .iter()
        .map(|k| {
            let f = slot_ui::gb_socket_face(*k);
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let gb_chips = Core::ALL
        .iter()
        .map(|k| {
            let f = slot_ui::gb_chip_face(Some(*k));
            tex(c, f.w, f.h, &f.rgba)
        })
        .collect();
    let gb_blank = slot_ui::gb_chip_face(None);
    let gb_blank = tex(c, gb_blank.w, gb_blank.h, &gb_blank.rgba);
    app.set_core_gb_part_faces(gb_sockets, gb_chips, gb_blank);

    let legend = slot_ui::core_button_legends(slot_ui::footer_ink(app.light()))
        .into_iter()
        .map(|f| (tex(c, f.w, f.h, &f.rgba), f.w))
        .collect::<Vec<_>>();
    app.set_core_legend_faces(legend.clone(), legend);
    let strip = match app.light() {
        true => slot_ui::slot_plugged_light(),
        false => slot_ui::slot_plugged(),
    };
    let plugged = tex(c, slot_ui::SLOT_W, slot_ui::SLOT_H, strip);
    app.set_plugged_face(plugged);

    let highlighted = app.selected_stem().map(str::to_string);
    let Some(cart) = app
        .carts()
        .find(|c| highlighted.as_deref() == Some(c.stem.as_str()))
        .cloned()
    else {
        return;
    };
    let interior = slot_ui::cart_interior(&cart);
    let interior = tex(c, interior.w, interior.h, &interior.rgba);
    app.set_core_interior_face(interior);
    let board = board_face(&cart);
    let board = tex(c, board.w, board.h, &board.rgba);
    let lid = padded(&cart_face(&cart), TURN_PAD);
    let lid = tex(c, lid.w, lid.h, &lid.rgba);
    app.set_core_board_faces(board, lid);
}

fn shelf(d: &TempDir, c: &mut Compositor) -> App {
    shelf_in(d, c, Appearance::Dark)
}

fn shelf_in(d: &TempDir, c: &mut Compositor, appearance: Appearance) -> App {
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            appearance,
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    upload_faces(&mut app, c);
    app
}

fn shot(app: &App, c: &mut Compositor, name: &str) -> Vec<u8> {
    let mut out = Vec::new();
    app.draw(&mut out);
    c.begin_frame();
    c.draw_list(&out);
    let px = c.read_frame();
    if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
        let path = format!("{dir}/shelf-{name}.png");
        let file = std::fs::File::create(&path).expect("create png");
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .expect("png header")
            .write_image_data(&px)
            .expect("png data");
        println!("wrote {path}");
    }
    px
}

fn let_it_hop(app: &mut App) {
    app.update(0.25);
}

#[test]
fn start_opens_the_picker_on_a_game_boy_cart() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };

    for (d, twin, name, core) in [
        (
            common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
            common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
            "gb",
            Core::Gambatte,
        ),
        (
            common::tmp_root_with_carts(&["Emerald", "Zzz"]),
            common::tmp_root_with_carts(&["Emerald", "Zzz"]),
            "gba",
            Core::Mgba,
        ),
    ] {
        let mut pressed = shelf(&d, &mut c);
        let mut untouched = shelf(&twin, &mut c);
        pressed.apply(Action::GbaDown(Btn::Start));
        let_it_hop(&mut pressed);
        let_it_hop(&mut untouched);
        let plain = shot(&untouched, &mut c, &format!("{name}-plain"));
        let after = shot(&pressed, &mut c, &format!("{name}-after-start"));
        assert_eq!(
            pressed.core_picker(),
            Some(core),
            "START opened no picker on the {name} cart"
        );
        assert_eq!(
            pressed.shelf_shake(),
            0.0,
            "START on the {name} cart shook the shelf"
        );
        assert!(
            after != plain,
            "the picker opened and the panel showed the same shelf"
        );
        let_it_hop(&mut pressed);
        let open = shot(&pressed, &mut c, &format!("{name}-open"));
        assert!(open != after, "the picker did not finish opening");
    }
}

fn colour_only_root(stem: &str) -> TempDir {
    let d = common::tmp_root_with_gb_carts(&["Zzz"]);
    let mut rom = vec![0u8; 0x150];
    let title = stem.to_uppercase();
    rom[0x134..0x134 + title.len()].copy_from_slice(title.as_bytes());
    rom[0x143] = 0xc0;
    std::fs::write(d.path().join("Games/GBC").join(format!("{stem}.gbc")), rom).expect("write rom");
    d
}

#[test]
fn render_core_picker_gb() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    for (d, shell) in [
        (
            common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]),
            "notched",
        ),
        (colour_only_root("Crystal"), "rounded"),
    ] {
        for panel in [(640, 480), (720, 480)] {
            let mut app = shelf(&d, &mut c);
            app.set_panel(panel);
            app.apply(Action::GbaDown(Btn::Start));
            let_it_hop(&mut app);
            let_it_hop(&mut app);
            let name = format!("picker-{shell}-{}", panel.0);
            let gambatte = shot(&app, &mut c, &format!("{name}-gambatte"));
            assert_eq!(app.core_picker_chip().unwrap().seated, Some(Core::Gambatte));
            app.apply(Action::GbaDown(Btn::Left));
            let_it_hop(&mut app);
            let mgba = shot(&app, &mut c, &format!("{name}-mgba"));
            assert_eq!(
                app.core_picker_chip().unwrap().seated,
                Some(Core::Mgba),
                "the chip did not land in the mGBA socket"
            );
            assert!(
                mgba != gambatte,
                "the chip moved and the panel did not change"
            );
        }
    }
}

#[test]
fn the_open_picker_footer_reads_in_light_mode() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    let mut app = shelf_in(&d, &mut c, Appearance::Light);
    assert!(app.light());
    app.apply(Action::GbaDown(Btn::Start));
    let_it_hop(&mut app);
    shot(&app, &mut c, "gba-light-opening");
    let_it_hop(&mut app);
    let px = shot(&app, &mut c, "gba-light-open");
    let y = OUT_H as usize - 20;
    let row = &px[y * OUT_W as usize * 4..(y + 1) * OUT_W as usize * 4];
    assert!(
        row.chunks(4).any(|p| p[0] < 90),
        "the light footer shows no dark ink"
    );
}

#[test]
fn the_card_says_which_tetris_is_in_the_slot() {
    let Some((_g, _s, mut c)) = compositor() else {
        eprintln!("no GL on this host, skipping");
        return;
    };
    let d = common::tmp_root_with_carts(&["Tetris", "Emerald"]);
    common::write_gb_cart(&d, "Tetris", "TETRIS");

    let gba = resumed_shot(&d, &mut c, Some(Platform::Gba), "resume-gba");
    let gb = resumed_shot(&d, &mut c, Some(Platform::Gb), "resume-gb");
    let unstated = resumed_shot(&d, &mut c, None, "resume-unstated");

    let (top, bottom) = cartridge_rows(&gba).expect("no cartridge in the slot at all");
    assert_eq!(
        cartridge_rows(&gb),
        Some((top, bottom)),
        "the two cartridges are not seated at the same depth, so what follows would be \
         comparing different parts of them"
    );
    let row = (top + bottom) / 2;
    let (dark, pale) = (centre(&gba, row), centre(&gb, row));
    assert!(
        (0..3).all(|k| pale[k] as i32 - dark[k] as i32 > 40),
        "the card named the Game Boy shelf and the slot is holding {pale:?} where the Game Boy \
         Advance cartridge reads {dark:?}: the wrong cartridge came back"
    );

    assert!(
        unstated == gba,
        "a card with no cart_platform line did not resume the Game Boy Advance cartridge"
    );
}

fn resumed_shot(
    d: &TempDir,
    c: &mut Compositor,
    platform: Option<Platform>,
    name: &str,
) -> Vec<u8> {
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Tetris".into()),
            cart_platform: platform,
            clock_set: true,
            ..Default::default()
        },
    )
    .unwrap();
    let mut app = App::boot(d.path());
    upload_faces(&mut app, c);
    shot(&app, c, name)
}

fn cartridge_rows(px: &[u8]) -> Option<(usize, usize)> {
    let flat = [[0.0, 0.0, 0.0, 1.0], housing(), opening(), edge(), recess()];
    let cart = |o: usize| {
        !flat
            .iter()
            .any(|f| (0..3).all(|k| px[o + k].abs_diff((f[k] * 255.0).round() as u8) <= 8))
    };
    let mut rows = (0..OUT_H as usize)
        .filter(|y| (0..OUT_W as usize).any(|x| cart((y * OUT_W as usize + x) * 4)));
    let first = rows.next()?;
    Some((first, rows.next_back().unwrap_or(first)))
}

fn centre(px: &[u8], row: usize) -> [u8; 3] {
    let o = (row * OUT_W as usize + (OUT_W / 2) as usize) * 4;
    [px[o], px[o + 1], px[o + 2]]
}
