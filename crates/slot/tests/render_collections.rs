#![cfg(target_os = "macos")]

mod common;

use std::collections::VecDeque;

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Btn, InputSource, Millis, RawEvent, FAVORITE_HOLD_MS, SELECT_HOLD_MS};
use slot_power::SimPlatform;

struct Script(VecDeque<Vec<RawEvent>>);

impl InputSource for Script {
    fn poll(&mut self, _now: Millis) -> Vec<RawEvent> {
        self.0.pop_front().unwrap_or_default()
    }
}

struct Rig {
    f: Frontend,
    c: Compositor,
    input: Script,
    now: Millis,
    _d: tempfile::TempDir,
    _surface: HeadlessSurface,
}

impl Rig {
    fn new() -> Option<Rig> {
        Rig::with(|_| {})
    }

    fn with(tweak: impl Fn(&mut slot_store::SlotState)) -> Option<Rig> {
        Rig::build(&[("Puzzlers", "gba/Apotris\ngb/Catrap\n")], tweak)
    }

    fn with_collections(cols: &[(&str, &str)]) -> Option<Rig> {
        let (name, body) = cols
            .iter()
            .find(|(n, _)| *n == "Puzzlers")
            .or(cols.first())
            .copied()?;
        let first = body.lines().next()?.to_string();
        let (platform, stem) = first.split_once('/')?;
        let platform = match platform {
            "gb" => slot_store::Platform::Gb,
            _ => slot_store::Platform::Gba,
        };
        let stem = stem.to_string();
        let name = name.to_string();
        Rig::build(cols, move |s| {
            s.shelf = Some(format!("collection/{name}"));
            s.collection = Some(name.clone());
            s.shelf_cart = Some(slot_store::Recent {
                platform,
                stem: stem.clone(),
            });
        })
    }

    fn build(cols_in: &[(&str, &str)], tweak: impl Fn(&mut slot_store::SlotState)) -> Option<Rig> {
        let surface = HeadlessSurface::new().ok()?;
        let mut c = Compositor::new(&surface).ok()?;
        let d = tmp_root_with_carts(&["Apotris", "Boktai", "Drill Dozer", "Fusion"]);
        common::write_gb_cart(&d, "Catrap", "CATRAP");
        clocked(d.path());
        let cols = d.path().join("Collections");
        std::fs::create_dir_all(&cols).unwrap();
        for (name, body) in cols_in {
            std::fs::write(cols.join(format!("{name}.txt")), body).unwrap();
        }
        let order: String = cols_in.iter().map(|(n, _)| format!("{n}\n")).collect();
        std::fs::write(cols.join("order.txt"), order).unwrap();
        let mut s = slot_store::read_slot_state(d.path());
        s.shelf = Some("gba".into());
        s.shelf_cart = Some(slot_store::Recent {
            platform: slot_store::Platform::Gba,
            stem: "Boktai".into(),
        });
        tweak(&mut s);
        slot_store::write_slot_state(d.path(), &s).unwrap();
        let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
        f.upload_faces(&mut c);
        let now = f.app().now();
        let mut rig = Rig {
            f,
            c,
            input: Script(VecDeque::new()),
            now,
            _d: d,
            _surface: surface,
        };
        rig.settle(90);
        Some(rig)
    }

    fn step(&mut self, events: Vec<RawEvent>, ms: Millis) {
        self.input.0.push_back(events);
        self.now += ms;
        self.f
            .advance_at(&mut self.input, self.now, ms as f32 / 1000.0);
    }

    fn settle(&mut self, frames: usize) {
        for _ in 0..frames {
            self.step(vec![], 16);
        }
    }

    fn tap(&mut self, btn: Btn) {
        self.step(vec![RawEvent::Down(btn)], 16);
        self.step(vec![RawEvent::Up(btn)], 16);
        self.settle(40);
    }

    fn hold_select(&mut self) {
        self.hold(Btn::Select, SELECT_HOLD_MS);
    }

    fn hold(&mut self, btn: Btn, ms: Millis) {
        self.step(vec![RawEvent::Down(btn)], 16);
        self.step(vec![], ms + 20);
        self.step(vec![RawEvent::Up(btn)], 16);
        self.settle(90);
    }

    fn frame(&mut self, name: &str) -> Vec<u8> {
        self.f.compose(&mut self.c);
        let px = self.c.read_frame();
        if let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") {
            let file = std::fs::File::create(format!("{dir}/collections-{name}.png")).unwrap();
            let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
            e.set_color(png::ColorType::Rgba);
            e.set_depth(png::BitDepth::Eight);
            e.write_header().unwrap().write_image_data(&px).unwrap();
        }
        px
    }
}

fn changed(a: &[u8], b: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    let mut n = 0;
    for y in ys {
        for x in xs.clone() {
            let o = (y * OUT_W as usize + x) * 4;
            if (0..3).any(|k| a[o + k].abs_diff(b[o + k]) > 24) {
                n += 1;
            }
        }
    }
    n
}

fn lit_in(px: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let o = (y * OUT_W as usize + x) * 4;
            px[o..o + 3].iter().all(|&c| c > 0xb0)
        })
        .count()
}

fn lit_centre(px: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> f32 {
    let hits: Vec<usize> = ys
        .flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let o = (y * OUT_W as usize + x) * 4;
            px[o..o + 3].iter().all(|&c| c > 0x80)
        })
        .map(|(x, _)| x)
        .collect();
    hits.iter().sum::<usize>() as f32 / hits.len().max(1) as f32
}

fn on_puzzlers(r: &mut Rig) {
    for _ in 0..6 {
        if r.f.app().shelf_platform_name() == Some("Puzzlers") {
            return;
        }
        r.tap(Btn::R1);
    }
    panic!("never reached Puzzlers");
}

const W: usize = OUT_W as usize;
const H: usize = OUT_H as usize;

#[test]
fn ticked_carts_show_no_box() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let before = r.frame("browse");
    r.hold_select();
    assert_eq!(r.f.app().picked_count(), Some(1));
    r.tap(Btn::A);
    assert_eq!(r.f.app().picked_count(), Some(0));
    let after = r.frame("selecting");
    assert!(
        changed(&before, &after, 0..W, 40..380) < 60,
        "selecting changed the carts"
    );
}

#[test]
fn the_a_legend_says_select_or_unselect() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    r.hold_select();
    let ticked = r.frame("legend-unselect");
    r.tap(Btn::A);
    let unticked = r.frame("legend-select");
    assert!(
        changed(&ticked, &unticked, W / 2 - 110..W / 2 + 60, H - 50..H) > 100,
        "the A legend did not change"
    );
}

#[test]
fn selecting_plugs_the_slot() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let before = r.frame("slot-before");
    r.hold_select();
    let after = r.frame("slot-plugged");
    assert!(
        changed(&before, &after, W / 3..2 * W / 3, H - 50..H) > 500,
        "the mouth did not change to the plugged slot"
    );
    assert!(
        changed(&before, &after, 0..120, H - 50..H) > 50,
        "no legend on the left of the plugged slot"
    );
}

#[test]
fn the_add_screen_draws_stacks_and_the_heading() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let shelf = r.frame("add-shelf");
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    assert_eq!(r.f.app().choice_names(), ["Puzzlers", "New Collection"]);
    r.settle(60);
    let add = r.frame("add-pair");
    assert!(
        lit_in(&add, W / 2 - 160..W / 2 + 160, 10..34) > 60,
        "no heading"
    );
    assert!(
        changed(&shelf, &add, 0..W / 2, 180..360) > 4000,
        "no stack on the left"
    );
    assert!(
        changed(&shelf, &add, W / 2..W, 180..360) > 2000,
        "no New Collection on the right"
    );
}

#[test]
fn the_picked_name_stays() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    r.tap(Btn::Right);
    r.settle(200);
    let px = r.frame("add-name-later");
    assert!(
        lit_in(&px, 0..W / 2, 355..395) > 60,
        "the picked name faded"
    );
}

#[test]
fn the_remove_screen_draws_the_stack_low_and_the_cart_under_the_heading() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    on_puzzlers(&mut r);
    let shelf = r.frame("remove-shelf");
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    assert_eq!(r.f.app().removing(), Some(("Puzzlers", false)));
    r.settle(60);
    let px = r.frame("remove");
    assert!(
        lit_in(&px, W / 2 - 160..W / 2 + 160, 10..34) > 60,
        "no heading"
    );
    assert!(
        changed(&shelf, &px, W / 2 - 90..W / 2 + 90, 40..170) > 2000,
        "no floating cart under the heading"
    );
}

#[test]
fn the_last_game_heading_is_larger() {
    let Some(mut r) = Rig::with_collections(&[("Solo", "gba/Boktai\n")]) else {
        return;
    };
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    assert_eq!(r.f.app().removing(), Some(("Solo", true)));
    r.settle(60);
    let px = r.frame("remove-last");
    assert!(
        lit_in(&px, 0..W, 8..40) > 400,
        "the heading is not the larger one"
    );
}

#[test]
fn moving_draws_the_move_heading() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    on_puzzlers(&mut r);
    r.tap(Btn::X);
    assert_eq!(r.f.app().moving(), Some("Move".to_string()));
    assert_eq!(r.f.app().edit_heading(), Some(("Move".to_string(), true)));
    let px = r.frame("move");
    assert!(
        lit_in(&px, W / 2 - 60..W / 2 + 60, 10..34) > 30,
        "no Move heading"
    );
}

#[test]
fn the_collection_row_names_sit_under_their_stacks() {
    let Some(mut r) = Rig::with_collections(&[
        ("Racing", "gba/Fusion\n"),
        ("Puzzlers", "gba/Apotris\ngb/Catrap\n"),
        ("Dozers", "gba/Drill Dozer\n"),
    ]) else {
        return;
    };
    r.hold(Btn::X, FAVORITE_HOLD_MS);
    assert!(r.f.app().moving().is_some_and(|m| m.starts_with("Move ")));
    r.settle(60);
    let px = r.frame("move-collection");
    let left = lit_centre(&px, 0..W / 3, 368..392);
    let right = lit_centre(&px, 2 * W / 3..W, 368..392);
    assert!(
        (left - (W as f32 / 2.0 - 262.0)).abs() < 24.0,
        "left name at {left}"
    );
    assert!(
        (right - (W as f32 / 2.0 + 262.0)).abs() < 24.0,
        "right name at {right}"
    );
}

fn three_collections() -> Option<Rig> {
    Rig::with_collections(&[
        ("Racing", "gba/Fusion\n"),
        ("Puzzlers", "gba/Apotris\ngb/Catrap\n"),
        ("Dozers", "gba/Drill Dozer\n"),
    ])
}

#[test]
fn a_lifted_collection_does_not_jiggle() {
    let Some(mut r) = three_collections() else {
        return;
    };
    r.hold(Btn::X, FAVORITE_HOLD_MS);
    r.settle(60);
    let a = r.frame("still-a");
    r.run(110);
    let b = r.frame("still-b");
    assert!(
        changed(&a, &b, 0..W, 40..380) < 50,
        "the held stack moved while idle"
    );
}

#[test]
fn moving_a_collection_has_no_jumps() {
    let Some(mut r) = three_collections() else {
        return;
    };
    r.hold(Btn::X, FAVORITE_HOLD_MS);
    r.settle(60);
    r.step(vec![RawEvent::Down(Btn::Right)], 16);
    let mut last = r.frame("pass-0");
    let mut steps = Vec::new();
    for k in 0..30 {
        if k == 12 {
            r.step(vec![RawEvent::Up(Btn::Right)], 16);
        } else {
            r.step(vec![], 16);
        }
        let px = r.frame(&format!("pass-{}", k + 1));
        steps.push(changed(&last, &px, 0..W, 40..380));
        last = px;
    }
    let mut sorted = steps.clone();
    sorted.sort();
    let median = sorted[sorted.len() / 2].max(200);
    let worst = *sorted.last().unwrap();
    assert!(worst < median * 3, "a frame jumped: {steps:?}");
}

#[test]
fn moving_a_game_has_no_jumps() {
    let Some(mut r) = Rig::with_collections(&[(
        "Puzzlers",
        "gba/Apotris\ngba/Boktai\ngba/Drill Dozer\ngba/Fusion\n",
    )]) else {
        return;
    };
    r.tap(Btn::X);
    assert_eq!(r.f.app().moving(), Some("Move".to_string()));
    r.step(vec![RawEvent::Down(Btn::Right)], 16);
    let mut last = r.frame("game-pass-0");
    let mut steps = Vec::new();
    for k in 0..40 {
        if k == 20 {
            r.step(vec![RawEvent::Up(Btn::Right)], 16);
        } else {
            r.step(vec![], 16);
        }
        let px = r.frame(&format!("game-pass-{}", k + 1));
        steps.push(changed(&last, &px, 0..W, 40..380));
        last = px;
    }
    let mut sorted = steps.clone();
    sorted.sort();
    let median = sorted[sorted.len() / 2].max(200);
    let worst = *sorted.last().unwrap();
    assert!(worst < median * 3, "a frame jumped: {steps:?}");
}

impl Rig {
    fn press_for(&mut self, btn: Btn, ms: Millis) {
        self.step(vec![RawEvent::Down(btn)], 16);
        let mut left = ms;
        while left > 16 {
            self.step(vec![], 16);
            left -= 16;
        }
    }

    fn run(&mut self, ms: Millis) {
        let mut left = ms;
        while left >= 16 {
            self.step(vec![], 16);
            left -= 16;
        }
    }
}

#[test]
fn transition_entry_mid_frame_has_neighbours_leaving() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let shelf = r.frame("t-shelf");
    r.press_for(Btn::Y, FAVORITE_HOLD_MS + 20);
    r.step(vec![RawEvent::Up(Btn::Y)], 16);
    r.run(100);
    let mid = r.frame("t-entry-mid");
    r.run(1000);
    let end = r.frame("t-entry-end");
    assert!(
        changed(&shelf, &mid, 0..120, 100..340) > 1500,
        "neighbours never moved"
    );
    assert!(
        changed(&mid, &end, W / 2 - 100..W / 2 + 100, 40..340) > 1500,
        "the cart never finished moving"
    );
}

#[test]
fn transition_the_hold_jiggles_the_cart() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let shelf = r.frame("t-still");
    r.press_for(Btn::Y, 330);
    let held = r.frame("t-jiggle");
    assert!(
        changed(&shelf, &held, W / 2 - 180..W / 2 + 180, 60..380) > 300,
        "no jiggle"
    );
    r.step(vec![RawEvent::Up(Btn::Y)], 16);
}

#[test]
fn transition_add_exit_grows_home_on_an_empty_shelf() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    let shelf = r.frame("t-add-shelf");
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    r.tap(Btn::Right);
    assert_eq!(r.f.app().picked_choice(), Some("Puzzlers"));
    r.step(vec![RawEvent::Down(Btn::A)], 16);
    r.step(vec![RawEvent::Up(Btn::A)], 16);
    r.run(150);
    r.frame("t-add-slot");
    r.run(400);
    let growing = r.frame("t-add-growing");
    r.run(1500);
    let done = r.frame("t-add-done");
    assert!(
        changed(&shelf, &growing, 0..110, 120..330) > 1500,
        "neighbours were already home"
    );
    assert!(
        changed(&shelf, &done, 0..W, 40..380) < 400,
        "did not end on the shelf"
    );
}

#[test]
fn transition_remove_rises_off_the_top() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    on_puzzlers(&mut r);
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    let screen = r.frame("t-remove-screen");
    r.step(vec![RawEvent::Down(Btn::A)], 16);
    r.step(vec![RawEvent::Up(Btn::A)], 16);
    r.run(150);
    let rising = r.frame("t-remove-rising");
    r.run(300);
    r.frame("t-remove-fan");
    assert!(
        changed(&screen, &rising, W / 2 - 90..W / 2 + 90, 40..170) > 800,
        "the cart never rose"
    );
}

#[test]
fn transition_gather_frames_converge() {
    let Some(mut r) = Rig::with_collections(&[
        ("Racing", "gba/Fusion\n"),
        ("Puzzlers", "gba/Apotris\ngb/Catrap\n"),
        ("Dozers", "gba/Drill Dozer\n"),
    ]) else {
        return;
    };
    let shelf = r.frame("t-gather-shelf");
    r.press_for(Btn::X, FAVORITE_HOLD_MS + 20);
    r.run(150);
    let mid = r.frame("t-gather-mid");
    r.step(vec![RawEvent::Up(Btn::X)], 16);
    r.run(1000);
    let end = r.frame("t-gather-end");
    assert!(
        changed(&shelf, &mid, 0..W, 60..380) > 2000,
        "nothing gathered"
    );
    assert!(
        changed(&mid, &end, 0..W, 60..380) > 2000,
        "the gather never finished"
    );
}

#[test]
fn transition_keep_on_the_last_game_settles_from_where_it_was() {
    let Some(mut r) = Rig::with_collections(&[("Solo", "gba/Boktai\n")]) else {
        return;
    };
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    assert_eq!(r.f.app().removing(), Some(("Solo", true)));
    let screen = r.frame("t-last-screen");
    r.step(vec![RawEvent::Down(Btn::B)], 16);
    r.step(vec![RawEvent::Up(Btn::B)], 16);
    let first = r.frame("t-last-kept");
    assert!(
        changed(&screen, &first, W / 2 - 120..W / 2 + 120, 150..330) < 3000,
        "the cart jumped away on Keep"
    );
}

#[test]
fn transition_the_carousel_slides_under_the_cart() {
    let Some(mut r) = Rig::build(
        &[
            ("Racing", "gba/Fusion\n"),
            ("Puzzlers", "gba/Apotris\ngb/Catrap\n"),
            ("Dozers", "gba/Drill Dozer\n"),
        ],
        |_| {},
    ) else {
        return;
    };
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    assert_eq!(r.f.app().choice_names().len(), 4);
    r.step(vec![RawEvent::Down(Btn::Right)], 16);
    r.step(vec![RawEvent::Up(Btn::Right)], 16);
    r.run(48);
    let mid = r.frame("t-carousel-mid");
    r.run(600);
    let end = r.frame("t-carousel-end");
    assert!(
        changed(&mid, &end, 0..W, 150..360) > 3000,
        "the row jumped instead of sliding"
    );
}

impl Rig {
    fn type_word(&mut self, word: &str) {
        for ch in word.chars() {
            let want = ch.to_string();
            let (row, col) = {
                let kb = self.f.app().keyboard().unwrap();
                kb.rows()
                    .iter()
                    .enumerate()
                    .find_map(|(i, rw)| {
                        rw.iter()
                            .position(|cap| kb.label(*cap) == want)
                            .map(|c| (i, c))
                    })
                    .unwrap()
            };
            while self.f.app().keyboard().unwrap().row() != row {
                self.tap(Btn::Down);
            }
            while self.f.app().keyboard().unwrap().col() != col {
                self.tap(Btn::Right);
            }
            self.tap(Btn::A);
        }
    }
}

#[test]
fn a_new_collection_shows_its_name_as_the_cart_drops_in() {
    let Some(mut r) = Rig::build(&[], |_| {}) else {
        return;
    };
    r.hold(Btn::Y, FAVORITE_HOLD_MS);
    let asking = r.frame("fresh-asking");
    r.tap(Btn::A);
    r.type_word("ab");
    r.step(vec![RawEvent::Down(Btn::Start)], 16);
    r.step(vec![RawEvent::Up(Btn::Start)], 16);
    r.run(260);
    let dropping = r.frame("fresh-dropping");
    assert!(
        changed(&asking, &dropping, W / 2 - 140..W / 2 + 140, 372..404) > 200,
        "the name under the box did not change"
    );
}

fn bright_in(px: &[u8], xs: std::ops::Range<usize>, ys: std::ops::Range<usize>) -> usize {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let o = (y * OUT_W as usize + x) * 4;
            px[o..o + 3].iter().any(|&c| c > 0x70)
        })
        .count()
}

#[test]
fn a_bundle_of_game_boy_carts_stays_under_the_heading() {
    let Some(mut r) = Rig::new() else {
        return;
    };
    r.hold_select();
    for _ in 0..6 {
        if r.f.app().shelf_platform_name() == Some("Game Boy") {
            break;
        }
        r.tap(Btn::R1);
    }
    assert_eq!(r.f.app().shelf_platform_name(), Some("Game Boy"));
    r.tap(Btn::A);
    r.tap(Btn::Start);
    let last = r.f.app().choice_names().last().cloned();
    for _ in 0..4 {
        if r.f.app().picked_choice().map(str::to_string) == last {
            break;
        }
        r.tap(Btn::Right);
    }
    assert_eq!(r.f.app().picked_count(), Some(2));
    r.settle(60);
    let add = r.frame("add-gb-bundle");
    assert!(
        lit_in(&add, W / 2 - 160..W / 2 + 160, 10..34) > 60,
        "no heading"
    );
    assert_eq!(
        bright_in(&add, W / 2..W, 35..40),
        0,
        "the bundle reaches into the heading row"
    );
}
