#![cfg(feature = "nds")]

mod common;

use common::{boot, tmp_root_with_carts, StubSnapshot};
use slot::screen::Screen;

#[test]
fn one_panel_draws_the_same_list_as_before() {
    let d = tmp_root_with_carts(&["Emerald", "Zelda"]);
    let mut a = boot(d.path());
    a.set_panel((1024, 768));
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    let mut before = Vec::new();
    a.draw(&mut before);
    let mut top = Vec::new();
    a.draw_screen(Screen::Top, &mut top);
    assert_eq!(format!("{before:?}"), format!("{top:?}"));
    let mut bottom = Vec::new();
    a.draw_screen(Screen::Bottom, &mut bottom);
    assert!(
        bottom.is_empty(),
        "one panel drew {} things on a bottom screen it does not have",
        bottom.len()
    );
}

#[test]
fn two_panels_is_off_until_the_device_says_so() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    assert!(!a.two_panels());
    a.set_two_panels(true);
    assert!(a.two_panels());
}

fn two_panel_app() -> (tempfile::TempDir, slot::app::App) {
    let d = tmp_root_with_carts(&["Emerald", "Zelda"]);
    let mut a = boot(d.path());
    a.set_two_panels(true);
    a.set_panel((1024, 768));
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    (d, a)
}

fn big_tex(list: &[slot_gfx::Draw]) -> Option<[f32; 4]> {
    list.iter().find_map(|d| match *d {
        slot_gfx::Draw::Tex { x, y, w, h, .. } | slot_gfx::Draw::Rect { x, y, w, h, .. }
            if h > 100.0 && w < 600.0 =>
        {
            Some([x, y, w, h])
        }
        _ => None,
    })
}

fn carts_shown(list: &[slot_gfx::Draw], canvas_w: f32) -> usize {
    list.iter()
        .filter(|d| match **d {
            slot_gfx::Draw::Tex { x, w, h, .. } | slot_gfx::Draw::Rect { x, w, h, .. } => {
                h > 100.0 && w < 600.0 && x + w > 0.0 && x < canvas_w
            }
            _ => false,
        })
        .count()
}

fn bottom(a: &slot::app::App) -> Vec<slot_gfx::Draw> {
    let mut out = Vec::new();
    a.draw_screen(Screen::Bottom, &mut out);
    out
}

fn top(a: &slot::app::App) -> Vec<slot_gfx::Draw> {
    let mut out = Vec::new();
    a.draw_screen(Screen::Top, &mut out);
    out
}

#[test]
fn the_shelf_draws_on_the_bottom_screen() {
    let (_d, a) = two_panel_app();
    assert_eq!(a.bottom_source(), slot::app::BottomSource::Compositor);
    assert!(
        big_tex(&bottom(&a)).is_some(),
        "no carts on the bottom screen while browsing"
    );
    assert!(
        big_tex(&top(&a)).is_none(),
        "a cart is on the top screen while browsing: {:?}",
        top(&a)
    );
    assert!(
        !top(&a).is_empty(),
        "the top screen is empty while browsing"
    );
}

#[test]
fn a_gba_cart_inserts_on_the_bottom_screen() {
    let (_d, mut a) = two_panel_app();
    a.apply(slot_input::Action::Insert);
    let mut frames = 0;
    for _ in 0..120 {
        a.update(1.0 / 120.0);
        if !matches!(a.phase(), slot::app::Phase::Inserting { .. }) || a.seat() >= 1.0 {
            break;
        }
        frames += 1;
        assert!(
            big_tex(&bottom(&a)).is_some(),
            "the cart left the bottom screen at seat {}",
            a.seat()
        );
        assert!(
            big_tex(&top(&a)).is_none(),
            "a cart showed on the top screen at seat {}",
            a.seat()
        );
    }
    assert!(frames > 5, "the insert never ran");
}

#[test]
fn a_gba_cart_stays_seated_in_the_bottom_slot_while_it_plays() {
    let (_d, mut a) = two_panel_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(
        matches!(a.phase(), slot::app::Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    assert_eq!(a.bottom_source(), slot::app::BottomSource::Compositor);
    assert!(
        big_tex(&bottom(&a)).is_some(),
        "no seated cart on the bottom screen"
    );
    a.set_game_ready(true);
    let mut top = Vec::new();
    a.draw_screen(Screen::Top, &mut top);
    assert!(
        top.iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
        "the game left the top screen"
    );
}

#[test]
fn a_gba_cart_ejects_from_the_bottom_slot() {
    let (_d, mut a) = two_panel_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(slot_input::Action::Eject);
    let mut saw_bottom = false;
    for _ in 0..240 {
        a.update(1.0 / 120.0);
        if matches!(a.phase(), slot::app::Phase::Ejecting { .. })
            && a.seat() > 0.55
            && a.seat() < 1.0
        {
            saw_bottom |= big_tex(&bottom(&a)).is_some();
        }
    }
    assert!(saw_bottom, "the cart never showed leaving the bottom slot");
    assert!(
        matches!(a.phase(), slot::app::Phase::Shelf),
        "{:?}",
        a.phase()
    );
}

fn two_panel_ds_app() -> (tempfile::TempDir, slot::app::App) {
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    std::fs::write(d.path().join("Games/NDS/Zoo.nds"), vec![1u8; 0x200]).unwrap();
    let mut a = boot(d.path());
    a.set_two_panels(true);
    a.set_panel((1024, 768));
    for _ in 0..30 {
        a.update(1.0 / 60.0);
    }
    (d, a)
}

#[test]
fn a_ds_cart_flings_up_while_the_shelf_parts_on_the_bottom() {
    let (_d, mut a) = two_panel_ds_app();
    a.apply(slot_input::Action::Insert);
    let (mut early, mut parted, mut late, mut secs) = (0, 0, 0, 0.0);
    for _ in 0..1200 {
        a.update(1.0 / 240.0);
        let seat = a.seat();
        if !matches!(a.phase(), slot::app::Phase::Inserting { .. }) || seat >= 1.0 {
            break;
        }
        secs += 1.0 / 240.0;
        assert_eq!(
            a.bottom_source(),
            slot::app::BottomSource::Compositor,
            "the shelf left the bottom at seat {seat}"
        );
        let w = a.canvas().w;
        if seat > 0.2 && seat < 0.25 {
            parted += 1;
            assert_eq!(
                carts_shown(&bottom(&a), w),
                1,
                "a neighbour is still on the bottom at seat {seat}"
            );
        }
        if seat >= 0.3 {
            assert_eq!(carts_shown(&bottom(&a), w), 0, "carts on the bottom at seat {seat}");
        }
        if seat < 0.25 {
            early += 1;
            assert!(
                big_tex(&bottom(&a)).is_some(),
                "the card is not leaving the bottom at seat {seat}"
            );
            assert!(
                big_tex(&top(&a)).is_none(),
                "the cart reached the top screen at seat {seat}: {:?}",
                top(&a)
            );
        }
        if seat > 0.7 {
            late += 1;
            let [x, _, w, _] =
                big_tex(&top(&a)).unwrap_or_else(|| panic!("no cart on the top at seat {seat}"));
            assert!(
                (x + w / 2.0 - a.canvas().w / 2.0).abs() < 4.0,
                "the DS cart is off centre on the top"
            );
        }
    }
    assert!(
        early > 0 && parted > 0 && late > 0,
        "the insert never passed every stage: {early} {parted} {late}"
    );
    assert!(
        secs > 1.2,
        "the DS card seated in {secs:.2} s, too fast to read as a throw"
    );
}

#[test]
fn a_ds_cart_springs_out_of_the_top_slot_and_lands_on_the_shelf() {
    let (_d, mut a) = two_panel_ds_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    for _ in 0..600 {
        a.update(1.0 / 120.0);
    }
    assert!(
        matches!(a.phase(), slot::app::Phase::Playing { .. }),
        "{:?}",
        a.phase()
    );
    a.apply(slot_input::Action::Eject);
    let (mut top_first, mut lands, mut secs) = (0, 0, 0.0);
    for _ in 0..2400 {
        a.update(1.0 / 240.0);
        if !matches!(a.phase(), slot::app::Phase::Ejecting { .. }) {
            break;
        }
        let seat = a.seat();
        if seat >= 1.0 {
            continue;
        }
        secs += 1.0 / 240.0;
        if seat > 0.7 {
            top_first += 1;
            assert!(
                big_tex(&top(&a)).is_some(),
                "the card is not leaving the top slot at seat {seat}"
            );
        }
        if seat < 0.15 {
            lands += 1;
            assert!(
                big_tex(&top(&a)).is_none(),
                "the card is still on the top screen at seat {seat}"
            );
        }
    }
    assert!(top_first > 0 && lands > 0, "{top_first} {lands}");
    assert!(secs > 1.1, "the eject flight took {secs:.2} s");
    assert!(
        matches!(a.phase(), slot::app::Phase::Shelf),
        "{:?}",
        a.phase()
    );
    let w = a.canvas().w;
    assert_eq!(
        carts_shown(&bottom(&a), w),
        1,
        "the neighbours were back before the card landed"
    );
    for _ in 0..120 {
        a.update(1.0 / 240.0);
    }
    assert!(
        carts_shown(&bottom(&a), w) > 1,
        "the neighbours did not come back after the card landed"
    );
}

#[cfg(target_os = "macos")]
mod rendered {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    use super::common::{clocked, tmp_root_with_carts};
    use slot::app::BottomSource;
    use slot::frontend::Frontend;
    use slot::screen::Screen;
    use slot_gfx::{Compositor, HeadlessSurface};
    use slot_input::{Btn, InputSource, Millis, RawEvent};
    use slot_power::SimPlatform;
    use slot_ui::QuickRow;

    pub const PANEL: (u32, u32) = (1024, 768);

    pub struct Script(pub VecDeque<Vec<RawEvent>>);

    impl InputSource for Script {
        fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
            self.0.pop_front().unwrap_or_default()
        }
    }

    pub struct Rig {
        pub _surface: HeadlessSurface,
        pub c: Compositor,
        pub f: Frontend,
        pub input: Script,
        pub _dir: tempfile::TempDir,
    }

    impl Rig {
        pub fn new(dir: tempfile::TempDir) -> Rig {
            std::env::remove_var("SLOT_CORE");
            clocked(dir.path());
            let surface = HeadlessSurface::new().expect("headless gl");
            let mut c = Compositor::new(&surface).expect("compositor");
            let mut f = Frontend::boot(Box::new(SimPlatform::at(dir.path().to_path_buf())));
            f.set_two_panels(true);
            c.fit(PANEL);
            f.upload_faces(&mut c);
            let mut rig = Rig {
                _surface: surface,
                c,
                f,
                input: Script(VecDeque::new()),
                _dir: dir,
            };
            rig.run_for(0.5);
            rig
        }

        pub fn gba() -> Rig {
            Rig::new(tmp_root_with_carts(&["Emerald", "Zelda"]))
        }

        pub fn ds() -> Rig {
            let d = tmp_root_with_carts(&[]);
            std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
            std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
            std::fs::write(d.path().join("Games/NDS/Zoo.nds"), vec![1u8; 0x200]).unwrap();
            Rig::new(d)
        }

        pub fn tap(&mut self, btn: Btn) {
            self.input.0.push_back(vec![RawEvent::Down(btn)]);
            self.f.advance(&mut self.input);
            self.input.0.push_back(vec![RawEvent::Up(btn)]);
            self.f.advance(&mut self.input);
            self.run_for(0.15);
        }

        pub fn run_for(&mut self, secs: f32) {
            let until = Instant::now() + Duration::from_secs_f32(secs);
            while Instant::now() < until {
                self.f.advance(&mut self.input);
                std::thread::sleep(Duration::from_millis(2));
            }
        }

        pub fn play(&mut self) {
            self.tap(Btn::A);
            self.run_for(4.0);
        }

        pub fn open_menu(&mut self) {
            self.tap(Btn::Menu);
            self.run_for(0.6);
        }

        pub fn shot(&mut self, screen: Screen, name: &str) -> Vec<u8> {
            if screen == Screen::Bottom {
                self.f.render_screen(&mut self.c, PANEL, Screen::Top);
            }
            self.f.render_screen(&mut self.c, PANEL, screen);
            let px = self.c.read_window(PANEL);
            if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
                let file = std::fs::File::create(format!("{dir}/{name}.png")).unwrap();
                let mut e = png::Encoder::new(std::io::BufWriter::new(file), PANEL.0, PANEL.1);
                e.set_color(png::ColorType::Rgba);
                e.set_depth(png::BitDepth::Eight);
                e.write_header().unwrap().write_image_data(&px).unwrap();
            }
            px
        }
    }

    pub fn white_in(px: &[u8], x0: usize, y0: usize, x1: usize, y1: usize) -> usize {
        let w = PANEL.0 as usize;
        let mut n = 0;
        for y in y0..y1 {
            for x in x0..x1 {
                let o = (y * w + x) * 4;
                if px[o] > 220 && px[o + 1] > 220 && px[o + 2] > 220 {
                    n += 1;
                }
            }
        }
        n
    }

    fn menu_band(px: &[u8]) -> usize {
        white_in(px, 300, 140, 700, 560)
    }

    #[test]
    fn a_gba_games_menu_draws_on_the_bottom_screen() {
        let mut r = Rig::gba();
        r.play();
        r.open_menu();
        assert!(r.f.app().in_game_menu().is_some());
        let bottom = r.shot(Screen::Bottom, "gba-menu-bottom");
        let top = r.shot(Screen::Top, "gba-menu-top");
        assert!(
            menu_band(&bottom) > 400,
            "no menu text on the bottom screen: {}",
            menu_band(&bottom)
        );
        assert_ne!(top, bottom);
        let mut list = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut list);
        assert!(
            list.iter().filter(|d| matches!(d, slot_gfx::Draw::Rect { colour, w, .. } if colour[3] > 0.5 && *w > 600.0)).count() <= 1,
            "the menu's scrim is on the top screen"
        );
        assert_eq!(r.f.bottom_source(), BottomSource::Compositor);
    }

    #[test]
    fn the_shelf_menu_draws_on_the_bottom_screen() {
        let mut r = Rig::gba();
        r.tap(Btn::Menu);
        r.run_for(0.6);
        assert!(r.f.app().quick_menu().is_some(), "{:?}", r.f.app().phase());
        let bottom = r.shot(Screen::Bottom, "shelf-menu-bottom");
        let top = r.shot(Screen::Top, "shelf-menu-top");
        assert!(
            menu_band(&bottom) > 400,
            "no menu text on the bottom screen: {}",
            menu_band(&bottom)
        );
        assert!(
            menu_band(&top) < 50,
            "menu text on the top screen: {}",
            menu_band(&top)
        );
    }

    #[test]
    fn the_about_screen_opened_from_the_menu_draws_on_the_bottom() {
        let mut r = Rig::gba();
        r.tap(Btn::Menu);
        r.run_for(0.6);
        let hidden = r.f.app().hidden_rows();
        for _ in 0..QuickRow::System.position(&hidden) {
            r.tap(Btn::Down);
        }
        r.tap(Btn::A);
        for _ in 0..QuickRow::About.position(&hidden) {
            r.tap(Btn::Down);
        }
        assert_eq!(r.f.app().quick_menu(), Some(QuickRow::About));
        r.tap(Btn::A);
        r.run_for(0.6);
        assert!(
            matches!(r.f.app().phase(), slot::app::Phase::About),
            "{:?}",
            r.f.app().phase()
        );
        assert_eq!(r.f.bottom_source(), BottomSource::Compositor);
        let mut top = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut top);
        let mut bottom = Vec::new();
        r.f.app().draw_screen(Screen::Bottom, &mut bottom);
        let scrim = |l: &[slot_gfx::Draw]| {
            l.iter().any(|d| matches!(d, slot_gfx::Draw::Rect { colour, w, .. } if colour[3] > 0.5 && *w > 600.0))
        };
        assert!(
            scrim(&bottom),
            "the about screen's ground is not on the bottom: {bottom:?}"
        );
        assert!(
            !scrim(&top),
            "the about screen's ground is on the top: {top:?}"
        );
        let shown = r.shot(Screen::Bottom, "about-bottom");
        let top_px = r.shot(Screen::Top, "about-top");
        assert_ne!(
            shown, top_px,
            "the about screen drew the same on both screens"
        );
    }

    #[test]
    fn a_ds_game_playing_direct_uses_the_ds_bottom_screen() {
        let mut r = Rig::ds();
        r.shot(Screen::Top, "ds-shelf");
        r.play();
        assert!(r.f.ds_direct());
        assert_eq!(r.f.bottom_source(), BottomSource::DsScreen);
        r.open_menu();
        assert_eq!(r.f.bottom_source(), BottomSource::Compositor);
    }

    #[test]
    fn the_bottom_screen_shows_the_gba_slot_on_the_shelf_and_while_playing() {
        let mut r = Rig::gba();
        assert_eq!(r.f.bottom_source(), BottomSource::Compositor);
        let shelf = r.shot(Screen::Bottom, "slot-shelf-bottom");
        r.shot(Screen::Top, "slot-shelf-top");
        let lit = shelf
            .chunks(4)
            .filter(|p| p[0] > 30 || p[1] > 30 || p[2] > 30)
            .count();
        assert!(
            lit > 100_000,
            "the bottom screen is dark on the shelf: {lit}"
        );
        let mut now: Millis = 100_000;
        r.input.0.push_back(vec![RawEvent::Down(Btn::A)]);
        r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
        r.input.0.push_back(vec![RawEvent::Up(Btn::A)]);
        for i in 0..8 {
            for _ in 0..4 {
                now += 16;
                r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
            }
            r.shot(Screen::Top, &format!("slot-insert-top-{i}"));
            r.shot(Screen::Bottom, &format!("slot-insert-bottom-{i}"));
        }
        r.run_for(4.0);
        let playing = r.shot(Screen::Bottom, "slot-play-bottom");
        r.shot(Screen::Top, "slot-play-top");
        assert_ne!(
            shelf, playing,
            "the seated cart does not show on the bottom"
        );
    }

    pub fn dimmed_game(draws: &[slot_gfx::Draw]) -> bool {
        draws.iter().any(|d| matches!(d, slot_gfx::Draw::Game))
            && draws.iter().any(|d| matches!(d, slot_gfx::Draw::Rect { colour, .. } if *colour == [0.0, 0.0, 0.0, 0.8]))
    }

    fn assert_dimmed_game(r: &Rig) {
        let mut draws = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut draws);
        assert!(dimmed_game(&draws), "no dimmed game on the top screen");
    }

    #[test]
    fn the_top_screen_shows_the_box_screen_while_the_menu_is_open() {
        let mut r = Rig::gba();
        r.play();
        let before = r.shot(Screen::Top, "box-before");
        r.open_menu();
        r.run_for(0.3);
        let top = r.shot(Screen::Top, "box-top");
        assert_dimmed_game(&r);
        assert_ne!(before, top);
        let mut list = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut list);
        assert!(
            dimmed_game(&list),
            "the game is not dimmed under the menu"
        );
    }

    #[test]
    fn the_top_screen_previews_the_game_on_a_screen_row() {
        let mut r = Rig::gba();
        r.play();
        r.open_menu();
        let hidden = r.f.app().in_game_hidden();
        for _ in 0..QuickRow::Settings.position(&hidden) {
            r.tap(Btn::Down);
        }
        r.tap(Btn::A);
        assert_eq!(r.f.app().in_game_menu(), Some(QuickRow::PlayScreen));
        let mut list = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut list);
        assert!(
            list.iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
            "no game preview on the Screen row"
        );
        r.tap(Btn::A);
        assert!(r
            .f
            .app()
            .in_game_menu()
            .is_some_and(|row| row.page() == &QuickRow::PLAY_SCREEN[..]));
        let mut list = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut list);
        assert!(
            list.iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
            "no game preview on a screen setting"
        );
        r.tap(Btn::B);
        r.tap(Btn::Up);
        let mut list = Vec::new();
        r.f.app().draw_screen(Screen::Top, &mut list);
        assert!(
            dimmed_game(&list),
            "the game is not dimmed after leaving the screen rows"
        );
    }

    fn slot_band(px: &[u8]) -> Vec<u8> {
        let w = PANEL.0 as usize;
        let mut out = Vec::new();
        for y in 690..768 {
            out.extend_from_slice(&px[(y * w + 360) * 4..(y * w + 664) * 4]);
        }
        out
    }

    #[test]
    fn a_ds_games_menu_shows_its_cart_in_the_top_slot() {
        let mut gba = Rig::gba();
        gba.play();
        gba.open_menu();
        let empty = slot_band(&gba.shot(Screen::Top, "gba-slot-top"));
        let mut ds = Rig::ds();
        ds.play();
        ds.open_menu();
        let seated = slot_band(&ds.shot(Screen::Top, "ds-slot-top"));
        let differ = empty
            .chunks(4)
            .zip(seated.chunks(4))
            .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 24))
            .count();
        assert!(
            differ > 800,
            "the DS slot looks the same as the empty slot: {differ} pixels differ"
        );
    }

    fn png(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut e = png::Encoder::new(&mut out, w, h);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header()
                .unwrap()
                .write_image_data(&px.repeat((w * h) as usize))
                .unwrap();
        }
        out
    }

    fn with_box(bytes: Vec<u8>) -> Rig {
        let d = tmp_root_with_carts(&["Emerald", "Zelda"]);
        let cart = slot_store::scan(d.path())
            .unwrap()
            .into_iter()
            .find(|c| c.stem == "Emerald")
            .unwrap();
        let (boxed, _) = slot::art::jobs::box_art_paths(d.path(), &cart);
        std::fs::create_dir_all(boxed.parent().unwrap()).unwrap();
        std::fs::write(&boxed, bytes).unwrap();
        Rig::new(d)
    }

    fn red_left(px: &[u8]) -> usize {
        let w = PANEL.0 as usize;
        let mut n = 0;
        for y in 0..680 {
            for x in 0..512 {
                let o = (y * w + x) * 4;
                if px[o] > 200 && px[o + 1] < 40 && px[o + 2] < 40 {
                    n += 1;
                }
            }
        }
        n
    }

    fn menu_with_art(r: &mut Rig) -> Vec<u8> {
        r.play();
        r.open_menu();
        let until = Instant::now() + Duration::from_secs(3);
        while r.f.app().box_art().is_none() && Instant::now() < until {
            r.run_for(0.05);
            r.f.render_screen(&mut r.c, PANEL, Screen::Top);
        }
        r.shot(Screen::Top, "box-art-top")
    }

    #[test]
    fn the_box_shows_once_the_loader_delivers() {
        let mut r = with_box(png(100, 130, [255, 0, 0, 255]));
        let top = menu_with_art(&mut r);
        assert!(
            r.f.app().box_art().is_some_and(|a| a.boxed.is_some()),
            "the box never arrived"
        );
        assert!(
            red_left(&top) > 20_000,
            "no box on the left of the top screen: {}",
            red_left(&top)
        );
    }

    #[test]
    fn a_corrupt_box_file_keeps_the_dimmed_game() {
        let mut r = with_box(b"not a png at all".to_vec());
        let top = menu_with_art(&mut r);
        assert!(
            r.f.app().box_art().is_some_and(|a| a.boxed.is_none()),
            "{:?}",
            r.f.app().box_art()
        );
        assert_eq!(red_left(&top), 0);
        assert_dimmed_game(&r);
    }

    #[test]
    fn a_cart_without_box_art_keeps_the_dimmed_game() {
        let mut r = Rig::gba();
        let _top = menu_with_art(&mut r);
        assert!(r
            .f
            .app()
            .box_art()
            .is_none_or(|a| a.boxed.is_none() && a.logo.is_none()));
        assert_dimmed_game(&r);
    }

    #[test]
    fn a_ds_cart_fling_draws_on_both_screens() {
        let mut r = Rig::ds();
        let mut now: Millis = 100_000;
        r.input.0.push_back(vec![RawEvent::Down(Btn::A)]);
        r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
        r.input.0.push_back(vec![RawEvent::Up(Btn::A)]);
        let mut tops = Vec::new();
        for i in 0..20 {
            for _ in 0..8 {
                now += 16;
                r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
            }
            tops.push(r.shot(Screen::Top, &format!("ds-fling-top-{i:02}")));
            r.shot(Screen::Bottom, &format!("ds-fling-bottom-{i:02}"));
        }
        assert_ne!(tops[0], tops[9], "nothing reached the top screen");
    }

    #[test]
    fn a_ds_cart_eject_draws_on_both_screens() {
        let mut r = Rig::ds();
        r.play();
        let mut now: Millis = 200_000;
        r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
        r.input.0.push_back(vec![RawEvent::Down(Btn::Menu)]);
        for _ in 0..70 {
            now += 16;
            r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
            if matches!(r.f.app().phase(), slot::app::Phase::Ejecting { .. }) {
                break;
            }
        }
        r.input.0.push_back(vec![RawEvent::Up(Btn::Menu)]);
        assert!(
            matches!(r.f.app().phase(), slot::app::Phase::Ejecting { .. }),
            "{:?}",
            r.f.app().phase()
        );
        let mut bottoms = Vec::new();
        for i in 0..18 {
            for _ in 0..8 {
                now += 16;
                r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
            }
            r.shot(Screen::Top, &format!("ds-eject-top-{i:02}"));
            bottoms.push(r.shot(Screen::Bottom, &format!("ds-eject-bottom-{i:02}")));
        }
        assert!(
            matches!(r.f.app().phase(), slot::app::Phase::Shelf),
            "{:?}",
            r.f.app().phase()
        );
    }
}

#[test]
fn the_audio_output_is_kept_awake_while_a_cart_goes_in_or_out() {
    let (_d, mut a) = two_panel_app();
    assert!(!a.expects_sound(), "the shelf keeps the audio awake");
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.update(1.0 / 60.0);
    assert!(
        a.expects_sound(),
        "an insert lets the audio sleep before its click"
    );
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    a.apply(slot_input::Action::Eject);
    a.update(1.0 / 60.0);
    assert!(
        a.expects_sound(),
        "an eject lets the audio sleep before its click"
    );
}

#[test]
fn a_session_wakes_the_ring_while_a_cart_goes_in() {
    let d = tmp_root_with_carts(&["Emerald", "Zelda"]);
    common::clocked(d.path());
    let (mut s, _motor) = common::session_with_platform(d.path());
    s.update(1.0 / 60.0);
    assert!(!s.ring().awake());
    s.app_mut().apply(slot_input::Action::Insert);
    s.update(1.0 / 60.0);
    assert!(
        s.ring().awake(),
        "the session did not wake the audio for the insert"
    );
}

fn sounds_through(a: &mut slot::app::App, secs: f32) -> Vec<(slot::audio::Sfx, f32, f32)> {
    let mut heard = Vec::new();
    let steps = (secs * 240.0) as usize;
    for _ in 0..steps {
        a.update(1.0 / 240.0);
        if let Some(sfx) = a.take_sfx() {
            heard.push((sfx, a.seat(), a.screen_power()));
        }
    }
    heard
}

#[cfg(feature = "nds")]
#[test]
fn a_ds_card_plays_its_own_insert_and_eject_clicks_on_time() {
    use slot::audio::Sfx;
    let (_d, mut a) = two_panel_ds_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    let heard = sounds_through(&mut a, 3.0);
    assert_eq!(heard.len(), 1, "{heard:?}");
    let (sfx, seat, _) = heard[0];
    assert_eq!(sfx, Sfx::DsInsert);
    let lead_seat = Sfx::DsInsert.lead() / 1.8;
    assert!(
        (seat - (1.0 - lead_seat)).abs() < 0.03,
        "the insert sound started at seat {seat}, so its latch misses the seating"
    );
    a.apply(slot_input::Action::Eject);
    let heard = sounds_through(&mut a, 3.0);
    assert_eq!(heard.len(), 1, "{heard:?}");
    let (sfx, seat, screen) = heard[0];
    assert_eq!(sfx, Sfx::DsEject);
    assert_eq!(
        screen, 0.0,
        "the eject click played before the screens went dark"
    );
    assert!(
        seat < 0.9 && seat > 0.8,
        "the eject latch click is not at the press: seat {seat}"
    );
}

#[test]
fn a_gba_cart_keeps_the_gba_clicks_on_the_ds_plus() {
    use slot::audio::Sfx;
    let (_d, mut a) = two_panel_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    let heard = sounds_through(&mut a, 2.0);
    assert_eq!(
        heard.iter().map(|h| h.0).collect::<Vec<_>>(),
        vec![Sfx::Insert]
    );
    a.apply(slot_input::Action::Eject);
    let heard = sounds_through(&mut a, 2.0);
    assert_eq!(
        heard.iter().map(|h| h.0).collect::<Vec<_>>(),
        vec![Sfx::Eject]
    );
}

#[cfg(feature = "nds")]
fn top_card_foot(a: &slot::app::App) -> Option<f32> {
    big_tex(&top(a)).map(|[_, y, _, h]| y + h)
}

#[cfg(feature = "nds")]
#[test]
fn a_ds_card_sits_deeper_than_flush_between_its_two_clicks() {
    use slot::audio::Sfx;
    let (_d, mut a) = two_panel_ds_app();
    let [_, sy, _, sh] = slot_ui::seated_rect(a.canvas(), slot_store::Platform::Nds);
    let flush = sy + sh;
    let gap = 0.53;
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    let mut clock = 0.0f32;
    let mut sound_at = None;
    let mut deep = Vec::new();
    for _ in 0..(3.0 * 240.0) as usize {
        a.update(1.0 / 240.0);
        clock += 1.0 / 240.0;
        if a.take_sfx() == Some(Sfx::DsInsert) {
            sound_at = Some(clock);
        }
        if let (Some(at), Some(foot)) = (sound_at, top_card_foot(&a)) {
            let since = clock - at;
            let first_click = Sfx::DsInsert.lead() - gap;
            if since > first_click + 0.03 && since < Sfx::DsInsert.lead() - 0.12 {
                deep.push(foot - flush);
            }
        }
        if a.seat() >= 1.0 {
            break;
        }
    }
    assert!(sound_at.is_some(), "no insert sound");
    assert!(
        !deep.is_empty() && deep.iter().all(|d| *d > 4.0),
        "the card is not held deep between the clicks: {deep:?}"
    );
    for _ in 0..600 {
        a.update(1.0 / 120.0);
    }
    a.take_sfx();
    a.apply(slot_input::Action::Eject);
    let (mut clock, mut sound_at, mut held) = (0.0f32, None, Vec::new());
    for _ in 0..(3.0 * 240.0) as usize {
        a.update(1.0 / 240.0);
        clock += 1.0 / 240.0;
        if a.take_sfx() == Some(Sfx::DsEject) {
            sound_at = Some(clock);
        }
        if let Some(at) = sound_at {
            let since = clock - at - Sfx::DsEject.lead();
            if since > 0.05 && since < gap - 0.05 {
                held.push(top_card_foot(&a).map(|f| f - flush));
            }
        }
    }
    assert!(sound_at.is_some(), "no eject sound");
    assert!(
        !held.is_empty() && held.iter().all(|d| d.is_some_and(|d| d > 4.0)),
        "the card left before the second eject click: {held:?}"
    );
}

#[cfg(feature = "nds")]
#[test]
fn both_screens_power_on_together_when_a_ds_card_seats() {
    let (_d, mut a) = two_panel_ds_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    a.set_game_ready(true);
    let mut powering = 0;
    for _ in 0..1200 {
        a.update(1.0 / 240.0);
        if powering > 0 && a.screen_power() >= 1.0 {
            break;
        }
        let power = a.screen_power();
        if a.seat() >= 1.0 && power > 0.0 && power < 1.0 {
            powering += 1;
            assert_eq!(
                a.bottom_source(),
                slot::app::BottomSource::Compositor,
                "the bottom left the compositor at power {power}"
            );
            assert!(
                bottom(&a).iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
                "the bottom has no game to power on at {power}"
            );
            assert!(
                top(&a).iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
                "the top has no game to power on at {power}"
            );
        }
    }
    assert!(
        powering > 0,
        "the screens never powered on during the insert"
    );
}

#[cfg(feature = "nds")]
#[test]
fn both_screens_power_off_together_when_a_ds_card_ejects() {
    let (_d, mut a) = two_panel_ds_app();
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(slot_input::Action::Insert);
    a.on_core_ready();
    a.set_game_ready(true);
    for _ in 0..1200 {
        a.update(1.0 / 240.0);
    }
    a.apply(slot_input::Action::Eject);
    let (mut powering, mut dark_hold) = (0, 0);
    for _ in 0..1200 {
        a.update(1.0 / 240.0);
        if !matches!(a.phase(), slot::app::Phase::Ejecting { .. }) || a.seat() < 1.0 {
            break;
        }
        assert_eq!(
            a.bottom_source(),
            slot::app::BottomSource::Compositor,
            "the bottom left the compositor at power {}",
            a.screen_power()
        );
        assert!(
            !bottom(&a).is_empty(),
            "the bottom is empty during the eject"
        );
        let power = a.screen_power();
        if power > 0.0 && power < 1.0 {
            powering += 1;
            assert!(
                bottom(&a).iter().any(|d| matches!(d, slot_gfx::Draw::Game)),
                "the bottom has no game to power off at {power}"
            );
        }
        if power == 0.0 {
            dark_hold += 1;
        }
    }
    assert!(powering > 0 && dark_hold > 0, "{powering} {dark_hold}");
}

#[cfg(target_os = "macos")]
mod menu_step {
    use super::rendered::Rig;
    use slot::app::BottomSource;
    use slot::screen::Screen;
    use slot_input::{Btn, Millis, RawEvent};

    #[test]
    fn a_ds_games_menu_changes_both_screens_on_the_same_frame() {
        let mut r = Rig::ds();
        r.play();
        let mut now: Millis = 300_000;
        r.input.0.push_back(vec![RawEvent::Down(Btn::Menu)]);
        r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
        r.input.0.push_back(vec![RawEvent::Up(Btn::Menu)]);
        let (mut top_at, mut bottom_at): (Option<u32>, Option<u32>) = (None, None);
        for frame in 0..60 {
            now += 16;
            r.f.advance_at(&mut r.input, now, 1.0 / 60.0);
            let mut top = Vec::new();
            r.f.app().draw_screen(Screen::Top, &mut top);
            let game_on_top = top.iter().any(|d| matches!(d, slot_gfx::Draw::Game));
            if game_on_top {
                let px = r.shot(Screen::Top, &format!("menu-step-top-{frame:02}"));
                let w = 1024usize;
                let green = (150..600)
                    .step_by(7)
                    .flat_map(|y| (20..250).step_by(9).map(move |x| (y * w + x) * 4 + 1))
                    .map(|i| px[i])
                    .max()
                    .unwrap_or(0);
                assert!(green < 140, "the top screen showed the DS bottom screen on frame {frame} (green {green})");
            }
            if top_at.is_none() && super::rendered::dimmed_game(&top) {
                top_at = Some(frame);
            }
            if bottom_at.is_none() && r.f.bottom_source() == BottomSource::Compositor {
                bottom_at = Some(frame);
            }
            if top_at.is_some() && bottom_at.is_some() {
                break;
            }
        }
        let (Some(t), Some(b)) = (top_at, bottom_at) else {
            panic!("the menu never showed: top {top_at:?} bottom {bottom_at:?}");
        };
        assert_eq!(t, b, "the top changed on frame {t}, the bottom on frame {b}");
    }

}
