mod common;

use common::{tmp_root_with_gb_carts, app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{write_slot_state, SlotState};
use tempfile::TempDir;

fn on_shelf() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf));
    common::connect(&mut a);
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

#[test]
fn tap_opens_the_studio_for_the_selected_cart() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    assert_eq!(a.studio().map(|s| s.cart.stem.as_str()), Some("Emerald"));
}

#[test]
fn no_studio_off_the_shelf() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::QuickMenu);
    a.apply(Action::SelectTap);
    assert!(a.studio().is_none());
}

#[test]
fn the_trailing_select_up_is_ignored() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    a.apply(Action::GbaUp(Btn::Select));
    assert!(a.studio().is_some());
}

#[test]
fn the_studio_takes_the_input_context() {
    let (_d, mut a) = on_shelf();
    assert_eq!(a.input_context(), slot_input::Context::Shelf);
    a.apply(Action::SelectTap);
    assert_eq!(a.input_context(), slot_input::Context::Menu);
}

fn pink() -> slot_store::ShellChoice {
    slot_store::ShellChoice {
        outline: slot_store::Outline::Auto,
        colour: [0xec, 0x94, 0xb4],
        finish: slot_store::ShellFinish::Solid,
    }
}

#[test]
fn a_shell_change_reaches_every_copy() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            favorites: vec![slot_store::Recent {
                platform: slot_store::Platform::Gba,
                stem: "Fusion".into(),
            }],
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.set_cart_shell(slot_store::Platform::Gba, "Fusion", Some(pink()));
    let shells = a.cart_shells(slot_store::Platform::Gba, "Fusion");
    assert!(shells.len() >= 2, "{shells:?}");
    assert!(shells.iter().all(|s| *s == Some(pink())), "{shells:?}");
}

#[test]
fn a_shell_change_drops_face_and_interior() {
    let (_d, mut a) = on_shelf();
    let tex = slot_ui::TexId::from_raw(7);
    a.set_cart_face(0, 0, tex);
    a.set_cart_interior(0, 0, tex);
    a.set_cart_shell(slot_store::Platform::Gba, "Emerald", Some(pink()));
    assert_eq!(a.set_cart_face(0, 0, tex), None);
    assert_eq!(a.set_cart_interior(0, 0, tex), None);
}

#[test]
fn closing_writes_the_shell_and_updates_the_shelf() {
    let (d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    press(&mut a, Btn::B);
    let t = a.now();
    a.tick_ms(t + 400);
    assert!(a.studio().is_none());
    let text = std::fs::read_to_string(d.path().join("Labels/cart_shell.txt")).unwrap();
    assert!(text.contains("Emerald"), "{text}");
    assert!(a.cart_shells(slot_store::Platform::Gba, "Emerald")[0].is_some());
}

mod radio {
    use super::*;
    use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs, StudioArt};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct Log {
        asked: Vec<ArtJob>,
        events: VecDeque<ArtEvent>,
    }

    #[derive(Clone, Default)]
    struct FakeArt(Arc<Mutex<Log>>);

    impl ArtJobs for FakeArt {
        fn ask(&mut self, job: ArtJob) {
            self.0.lock().unwrap().asked.push(job);
        }

        fn take(&mut self) -> Option<ArtEvent> {
            self.0.lock().unwrap().events.pop_front()
        }
    }

    fn with_art() -> (TempDir, App, FakeArt) {
        let (d, mut a) = on_shelf();
        let art = FakeArt::default();
        a.set_art_jobs(Box::new(art.clone()));
        (d, a, art)
    }

    #[test]
    fn the_studio_waits_for_its_own_save() {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        write_slot_state(d.path(), &SlotState { clock_set: true, ..SlotState::default() }).unwrap();
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
        let (mut first, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
        common::connect(&mut first);
        first.set_art_jobs(Box::new(FakeArt::default()));
        first.apply(Action::SelectTap);
        let cart = first.studio().unwrap().cart.clone();
        let logo = slot_ui::house_label::Label::from_png(
            &{
                let mut out = Vec::new();
                let mut enc = png::Encoder::new(&mut out, 8, 4);
                enc.set_color(png::ColorType::Rgba);
                enc.set_depth(png::BitDepth::Eight);
                enc.write_header().unwrap().write_image_data(&[255; 8 * 4 * 4]).unwrap();
                out
            },
            [60, 20, 90],
            slot_store::Platform::Gba,
            None,
        )
        .unwrap()
        .png();
        let label = slot::art::make::label_path(d.path(), &cart);
        std::fs::create_dir_all(label.parent().unwrap()).unwrap();
        std::fs::write(&label, logo).unwrap();
        let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
        let art = FakeArt::default();
        a.set_art_jobs(Box::new(art.clone()));
        a.apply(Action::SelectTap);
        art.0.lock().unwrap().events.push_back(ArtEvent::StudioArt(StudioArt {
            stem: cart.stem.clone(),
            wheel: Some("w.png".into()),
            ..StudioArt::default()
        }));
        a.tick_ms(a.now() + 400);
        assert!(a.studio().unwrap().rows().contains(&slot::studio::Row::LabelColor));
        while a.studio().unwrap().row() != Some(slot::studio::Row::LabelColor) {
            press(&mut a, Btn::Down);
        }
        press(&mut a, Btn::A);
        press(&mut a, Btn::Down);
        press(&mut a, Btn::B);
        assert!(a.studio().unwrap().recolored());
        a.close_studio();
        a.apply(Action::SelectTap);
        assert!(a.studio().is_none(), "opened over an unfinished save");
        art.0.lock().unwrap().events.push_back(ArtEvent::StudioSaved(cart.stem.clone()));
        a.tick_ms(a.now() + 1);
        a.apply(Action::SelectTap);
        assert!(a.studio().is_some());
    }

    #[test]
    fn the_studio_preloads_on_open() {
        let (_d, mut a, art) = with_art();
        a.apply(Action::SelectTap);
        let n = art.0.lock().unwrap().asked.iter().filter(|j| matches!(j, ArtJob::StudioFetch(_))).count();
        assert_eq!(n, 1);
        press(&mut a, Btn::A);
        let n = art.0.lock().unwrap().asked.iter().filter(|j| matches!(j, ArtJob::StudioFetch(_))).count();
        assert_eq!(n, 1);
    }
}

mod opening {
    use super::*;
    use slot_ui::{Draw, TexId};

    fn face_rect(a: &App, tex: TexId) -> Option<(f32, f32)> {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.iter().rev().find_map(|d| match *d {
            Draw::Tex { x, w, tex: t, .. } if t == tex => Some((x, w)),
            Draw::Quad { corners, tex: t, .. } if t == tex => {
                let w = (corners[1][0] - corners[0][0]).hypot(corners[1][1] - corners[0][1]);
                Some(((corners[0][0] + corners[2][0] - w) / 2.0, w))
            }
            _ => None,
        })
    }

    fn ground_y(a: &App) -> Option<f32> {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.iter().rev().find_map(|d| match *d {
            Draw::Rect { y, w, h, .. } if w >= 720.0 && h >= 480.0 => Some(y),
            _ => None,
        })
    }

    #[test]
    fn the_grid_drops_in_and_lifts_out_the_top() {
        let (_d, mut a) = on_shelf();
        a.apply(Action::SelectTap);
        let t = a.now();
        a.tick_ms(t + 120);
        let y = ground_y(&a).unwrap();
        assert!(y < 0.0 && y > -480.0, "{y}");
        a.tick_ms(t + 400);
        assert_eq!(ground_y(&a), Some(0.0));
        press(&mut a, Btn::B);
        let t = a.now();
        a.tick_ms(t + 120);
        let y = ground_y(&a).unwrap();
        assert!(y < 0.0 && y > -480.0, "{y}");
    }

    #[test]
    fn a_clear_cart_keeps_its_board_while_the_studio_loads() {
        let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
        write_slot_state(
            d.path(),
            &SlotState {
                clock_set: true,
                ..SlotState::default()
            },
        )
        .unwrap();
        std::fs::write(
            d.path().join("Labels/cart_shell.txt"),
            "Emerald = auto 3366cc clear\n",
        )
        .unwrap();
        let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
        let (face, interior) = (TexId::from_raw(7), TexId::from_raw(8));
        for want in a.face_wants() {
            a.set_cart_face(want.shelf, want.index, face);
            a.set_cart_interior(want.shelf, want.index, interior);
        }
        a.apply(Action::SelectTap);
        assert!(a.studio_tex().is_none());
        let t = a.now();
        a.tick_ms(t + 120);
        let mut out = Vec::new();
        a.draw(&mut out);
        let quad = |tex: TexId| {
            out.iter().rposition(|d| matches!(*d, Draw::Quad { tex: t, .. } if t == tex))
        };
        let (under, over) = (quad(interior), quad(face));
        assert!(under.is_some(), "the board vanished");
        assert!(under < over);
    }

    #[test]
    fn the_cart_tilts_as_it_arrives() {
        let (_d, mut a) = on_shelf();
        let tex = TexId::from_raw(7);
        for want in a.face_wants() {
            a.set_cart_face(want.shelf, want.index, tex);
        }
        a.apply(Action::SelectTap);
        let t = a.now();
        a.tick_ms(t + 400);
        let mut out = Vec::new();
        a.draw(&mut out);
        let turn = out.iter().find_map(|d| match *d {
            Draw::Quad { corners, tex: t, .. } if t == tex => {
                let (dx, dy) = (corners[1][0] - corners[0][0], corners[1][1] - corners[0][1]);
                Some(dy.atan2(dx).to_degrees())
            }
            _ => None,
        });
        assert!((turn.unwrap() - slot::studio_view::CART_TILT).abs() < 0.1, "{turn:?}");
    }

    #[test]
    fn the_shelf_face_slides_to_the_studio_spot() {
        let (_d, mut a) = on_shelf();
        let tex = TexId::from_raw(7);
        for want in a.face_wants() {
            a.set_cart_face(want.shelf, want.index, tex);
        }
        a.apply(Action::SelectTap);
        let (x, w) = face_rect(&a, tex).expect("the shelf face is drawn at once");
        assert!((w - slot_ui::CART_W as f32).abs() < 1.0, "{w}");
        assert!((x - 180.0).abs() < 1.0, "{x}");
        let t = a.now();
        a.tick_ms(t + 120);
        let (x, _) = face_rect(&a, tex).unwrap();
        assert!(x < 180.0 && x > 20.0, "{x}");
        a.tick_ms(t + 400);
        let (x, w) = face_rect(&a, tex).unwrap();
        let want = slot::studio_view::layout(720.0, slot_store::Platform::Gba).cart_x;
        assert!((x - want).abs() < 1.0 && (w - 360.0).abs() < 1.0, "{x} {w}");
    }

    #[test]
    fn leaving_slides_the_cart_back_to_center() {
        let (_d, mut a) = on_shelf();
        let tex = TexId::from_raw(7);
        for want in a.face_wants() {
            a.set_cart_face(want.shelf, want.index, tex);
        }
        a.apply(Action::SelectTap);
        let t = a.now();
        a.tick_ms(t + 400);
        press(&mut a, Btn::B);
        assert!(a.studio().is_some(), "the studio left before the slide");
        let t = a.now();
        a.tick_ms(t + 120);
        let (x, _) = face_rect(&a, tex).unwrap();
        assert!(x > 20.0 && x < 180.0, "{x}");
        a.tick_ms(t + 400);
        assert!(a.studio().is_none());
        let mut out = Vec::new();
        a.draw(&mut out);
        let centered = out.iter().any(|d| match *d {
            Draw::Tex { x, w, tex: t, .. } => {
                t == tex && (x - 180.0).abs() < 1.0 && (w - 360.0).abs() < 1.0
            }
            _ => false,
        });
        assert!(centered);
    }

    #[test]
    fn legends_follow_the_theme() {
        for appearance in [slot_store::Appearance::Dark, slot_store::Appearance::Light] {
            let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
            write_slot_state(d.path(), &SlotState { clock_set: true, appearance, ..SlotState::default() }).unwrap();
            let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
            let (dark, light) = (TexId::from_raw(21), TexId::from_raw(22));
            for (is_light, tex) in [(false, dark), (true, light)] {
                for (l, left) in [(slot::studio::Legend::Back, true), (slot::studio::Legend::Open, false)] {
                    a.set_studio_legend((l, left, is_light), (tex, 80));
                }
            }
            a.apply(Action::SelectTap);
            a.tick_ms(a.now() + 400);
            let (want, other) = if appearance == slot_store::Appearance::Light { (light, dark) } else { (dark, light) };
            assert!(face_rect(&a, want).is_some());
            assert!(face_rect(&a, other).is_none());
        }
    }
}

mod reopening {
    use super::*;
    use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs, StudioArt};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<VecDeque<ArtEvent>>>);

    impl ArtJobs for Fake {
        fn ask(&mut self, _job: ArtJob) {}

        fn take(&mut self) -> Option<ArtEvent> {
            self.0.lock().unwrap().pop_front()
        }
    }

    fn art() -> ArtEvent {
        ArtEvent::StudioArt(StudioArt {
            stem: "Emerald".into(),
            real: Some("old-real.png".into()),
            ..StudioArt::default()
        })
    }

    #[test]
    fn a_late_fetch_does_not_land_in_the_next_visit() {
        let (_d, mut a) = on_shelf();
        let jobs = Fake::default();
        a.set_art_jobs(Box::new(jobs.clone()));
        a.apply(Action::SelectTap);
        press(&mut a, Btn::B);
        a.tick_ms(a.now() + 400);
        a.apply(Action::SelectTap);
        jobs.0.lock().unwrap().push_back(art());
        a.tick_ms(a.now() + 1);
        assert!(!a.studio().unwrap().art_ready());
        jobs.0.lock().unwrap().push_back(art());
        a.tick_ms(a.now() + 1);
        assert!(a.studio().unwrap().art_ready());
    }
}

mod printing {
    use super::*;
    use slot::app::StudioTex;
    use slot::studio::LabelKind;
    use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs, StudioArt};
    use slot_ui::TexId;

    struct Art(bool);

    impl ArtJobs for Art {
        fn ask(&mut self, _job: ArtJob) {}

        fn take(&mut self) -> Option<ArtEvent> {
            std::mem::take(&mut self.0).then(|| {
                ArtEvent::StudioArt(StudioArt {
                    stem: "Emerald".into(),
                    real: Some("r.png".into()),
                    logo: Some("l.png".into()),
                    ..StudioArt::default()
                })
            })
        }
    }

    #[test]
    fn the_sheet_prints_each_time_it_opens() {
        let (_d, mut a) = on_shelf();
        a.set_art_jobs(Box::new(Art(true)));
        a.apply(Action::SelectTap);
        let t = TexId::from_raw(3);
        a.set_studio_tex(StudioTex {
            faces: vec![(LabelKind::Real, t), (LabelKind::Logo, t), (LabelKind::Title, t)],
            bare: t,
            under: None,
            w: 360,
            h: 212,
        });
        a.tick_ms(a.now() + 50);
        assert_eq!(a.studio_slip_count(), 0, "printed before the screen opened");
        press(&mut a, Btn::A);
        a.tick_ms(a.now() + 50);
        assert_eq!(a.studio_slip_count(), 1);
        press(&mut a, Btn::B);
        a.tick_ms(a.now() + 50);
        assert_eq!(a.studio_slip_count(), 0);
    }
}

#[test]
fn shortcuts_fire_in_the_studio() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    let t = a.now();
    a.tick_ms(t + 400);
    let (level, blue) = (a.brightness(), a.blue_light());
    let count = |a: &slot::app::App| {
        let mut out = Vec::new();
        a.draw(&mut out);
        out.len()
    };
    let before = count(&a);
    a.apply(Action::BrightnessUp);
    a.apply(Action::BlueLightUp);
    assert_ne!(a.brightness(), level);
    assert_ne!(a.blue_light(), blue);
    assert!(a.studio().is_some());
    assert!(count(&a) > before, "no HUD over the studio");
}

#[test]
fn a_text_color_is_saved_as_a_tagged_label() {
    let (d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    let t = a.now();
    a.tick_ms(t + 400);
    let cart = a.studio().unwrap().cart.clone();
    assert!(cart.label.is_none());
    press(&mut a, Btn::Down);
    assert_eq!(a.studio().unwrap().row(), Some(slot::studio::Row::LabelColor));
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    assert!(a.studio().unwrap().recolored());
    press(&mut a, Btn::B);
    let t = a.now();
    a.tick_ms(t + 400);
    assert!(a.studio().is_none());
    let path = slot::art::make::label_path(d.path(), &cart);
    let tag = slot_ui::house_label::label_tag(&std::fs::read(path).unwrap());
    assert!(matches!(tag, Some(slot_ui::house_label::LabelTag::Text(_))), "{tag:?}");
}

#[test]
fn a_text_color_saved_with_a_new_shape_uses_that_shape() {
    use slot::studio::Row;
    let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
    write_slot_state(d.path(), &SlotState { clock_set: true, ..SlotState::default() }).unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    a.apply(Action::SelectTap);
    let t = a.now();
    a.tick_ms(t + 400);
    let mut cart = a.studio().unwrap().cart.clone();
    assert_eq!(a.studio().unwrap().row(), Some(Row::LabelColor));
    press(&mut a, Btn::A);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::B);
    let ground = a.studio().unwrap().ground.unwrap();
    while a.studio().unwrap().row() != Some(Row::Shape) {
        press(&mut a, Btn::Down);
    }
    let was = slot_ui::text_label(&cart, ground);
    let mut picked = None;
    for _ in 0..2 {
        press(&mut a, Btn::Right);
        cart.shell = a.studio().unwrap().shell;
        if slot_ui::text_label(&cart, ground) != was {
            picked = cart.shell;
            break;
        }
    }
    assert!(picked.is_some(), "no shape changes the text label");
    a.close_studio();
    let saved = std::fs::read(slot::art::make::label_path(d.path(), &cart)).unwrap();
    assert_eq!(saved, slot::art::make::text_label(&cart, ground));
}

#[test]
fn a_new_hex_readout_frees_the_last_one() {
    use slot::studio_view::Style;
    use slot_gfx::TexId;
    let (_d, mut a) = on_shelf();
    let (old, new) = (TexId::from_raw(40), TexId::from_raw(41));
    a.set_studio_text(("#112233".into(), Style::Detail), (old, 10, 10));
    a.shed_faces();
    a.set_studio_text(("#445566".into(), Style::Detail), (new, 10, 10));
    assert!(a.shed_faces().contains(&old));
}

#[test]
fn a_studio_edit_marks_the_core_picker_stale() {
    let (_d, mut a) = on_shelf();
    assert!(!a.take_core_stale());
    a.set_cart_shell(slot_store::Platform::Gba, "Emerald", None);
    assert!(a.take_core_stale());
    assert!(!a.take_core_stale());
}

#[test]
fn the_printer_body_stays_light_in_both_themes() {
    use slot_gfx::{Draw, TexId};
    let (_d, mut a) = on_shelf();
    let (themed, light) = (TexId::from_raw(30), TexId::from_raw(31));
    a.set_plugged_face(themed);
    a.set_printer_face(light);
    a.apply(Action::SelectTap);
    let t = a.now();
    a.tick_ms(t + 400);
    press(&mut a, Btn::A);
    let mut out = Vec::new();
    a.draw(&mut out);
    let body = |tex: TexId| out.iter().any(|d| matches!(*d, Draw::Part { tex: t, .. } if t == tex));
    assert!(body(light));
    assert!(!body(themed));
}

#[test]
fn stickers_only_shuffle_when_you_move_between_them() {
    let d = tmp_root_with_gb_carts(&["Kirby", "Tetris"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    common::connect(&mut a);
    a.set_art_jobs(slot::art::jobs::no_art_jobs());
    a.apply(Action::SelectTap);
    let t = a.now();
    a.tick_ms(t + 400);
    press(&mut a, Btn::A);
    let t = a.now();
    a.tick_ms(t + 10);
    a.tick_ms(t + 60);
    assert_eq!(a.studio_apart(), 0.0, "the sheet jostled as it opened");
    a.tick_ms(t + 1000);
    press(&mut a, Btn::Down);
    let t = a.now();
    a.tick_ms(t + 60);
    assert!(a.studio_apart() > 0.0, "no shuffle");
}

#[test]
fn holding_a_on_the_shelf_does_not_launch_behind_the_studio() {
    let (_d, mut a) = on_shelf();
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::SelectTap);
    assert!(a.studio().is_some());
    a.apply(Action::GbaUp(Btn::A));
    let t = a.now();
    a.tick_ms(t + 800);
    assert!(a.seated_cart().is_none(), "a game started behind the studio");
}

#[test]
fn powering_off_keeps_studio_edits() {
    let (d, mut a) = on_shelf();
    a.apply(Action::SelectTap);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Down);
    press(&mut a, Btn::Right);
    a.apply(Action::PowerHold);
    let text = std::fs::read_to_string(d.path().join("Labels/cart_shell.txt")).unwrap_or_default();
    assert!(text.contains("Emerald"), "{text:?}");
}
