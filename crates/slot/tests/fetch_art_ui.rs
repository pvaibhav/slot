mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::App;
use slot_store::{write_slot_state, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

fn booted(wifi: bool) -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    if wifi {
        std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    }
    let (a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    (d, a)
}

#[test]
fn row_shown_only_while_connected() {
    for wifi in [false, true] {
        let (_d, a) = booted(wifi);
        assert!(
            a.hidden_rows().contains(&QuickRow::FetchArt),
            "wifi.conf {wifi}"
        );
    }
    let (_d, mut a) = booted(true);
    common::connect(&mut a);
    assert!(!a.hidden_rows().contains(&QuickRow::FetchArt));
}

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use slot::art::jobs::{ArtEvent, ArtJob, ArtJobs};
use slot::art::make::Kind;
use slot::fetch_art::{stage_at, FetchArt, Stage, SCENE_MS};
use slot::update::UpdateFail;
use slot_input::{Action, Btn};
use slot_store::Platform;

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

impl FakeArt {
    fn asked(&self) -> Vec<ArtJob> {
        self.0.lock().unwrap().asked.clone()
    }

    fn send(&self, a: &mut App, event: ArtEvent) {
        self.0.lock().unwrap().events.push_back(event);
        a.tick_ms(a.now() + 1);
    }
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn advance(a: &mut App, ms: f64) {
    let end = a.now() as f64 + ms;
    while (a.now() as f64) < end {
        a.tick_ms(a.now() + 25);
    }
}

fn landed(stem: &str) -> ArtEvent {
    ArtEvent::Landed {
        stem: stem.into(),
        platform: Platform::Gba,
        path: PathBuf::from(format!("Labels/GBA/{stem}.png")),
    }
}

fn choosing() -> (TempDir, App, FakeArt) {
    let (d, mut a) = booted(true);
    let art = FakeArt::default();
    a.set_art_jobs(Box::new(art.clone()));
    a.open_fetch_art();
    assert!(matches!(a.fetch_art(), Some(FetchArt::Choice { .. })));
    (d, a, art)
}

fn running() -> (TempDir, App, FakeArt) {
    let (d, mut a, art) = choosing();
    press(&mut a, Btn::A);
    (d, a, art)
}

fn scene_stem(a: &App) -> Option<String> {
    match a.fetch_art() {
        Some(FetchArt::Running(run)) => run.scene.as_ref().map(|s| s.cart.stem.clone()),
        _ => None,
    }
}

#[test]
fn opening_needs_no_network() {
    let (_d, _a, art) = choosing();
    assert!(art.asked().is_empty());
}

#[test]
fn bundled_examples_are_two_different_labels() {
    let [real, logo] = slot::fetch_faces::example_faces();
    assert_eq!((real.w, real.h), (360, 212));
    assert_ne!(real.rgba, logo.rgba);
}

#[test]
fn a_starts_the_picked_kind() {
    let (_d, mut a, art) = choosing();
    press(&mut a, Btn::Right);
    press(&mut a, Btn::A);
    assert_eq!(art.asked().last(), Some(&ArtJob::FetchMissing(Kind::Logo)));
}

#[test]
fn b_on_choice_returns_to_settings() {
    let (_d, mut a, _art) = choosing();
    press(&mut a, Btn::B);
    assert!(a.fetch_art().is_none());
}

#[test]
fn scenes_play_one_at_a_time() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("Emerald"));
    art.send(&mut a, landed("Fusion"));
    advance(&mut a, SCENE_MS * 0.5);
    assert_eq!(scene_stem(&a).as_deref(), Some("Emerald"));
    advance(&mut a, SCENE_MS * 0.6);
    assert_eq!(scene_stem(&a).as_deref(), Some("Fusion"));
}

#[test]
fn missed_carts_play_nothing() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(1));
    art.send(&mut a, ArtEvent::Missed { stem: "X".into() });
    assert!(scene_stem(&a).is_none());
    art.send(&mut a, ArtEvent::Done { landed: 0, missed: vec!["X".into()] });
    advance(&mut a, 100.0);
    match a.fetch_art() {
        Some(FetchArt::Summary { missed, .. }) => assert_eq!(missed, &vec!["X".to_string()]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn stop_finishes_the_current_cart() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("Emerald"));
    art.send(&mut a, landed("Fusion"));
    advance(&mut a, SCENE_MS * 0.3);
    press(&mut a, Btn::B);
    assert_eq!(art.asked().last(), Some(&ArtJob::Cancel));
    assert_eq!(scene_stem(&a).as_deref(), Some("Emerald"));
    art.send(&mut a, ArtEvent::Done { landed: 2, missed: vec![] });
    advance(&mut a, SCENE_MS);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Summary { .. })), "{:?}", a.fetch_art());
    assert!(a.fetch_landed().iter().any(|l| l.stem == "Fusion"));
}

#[test]
fn losing_wifi_shows_the_failure() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("A"));
    art.send(&mut a, ArtEvent::Failed(UpdateFail::Offline));
    advance(&mut a, SCENE_MS + 100.0);
    match a.fetch_art() {
        Some(FetchArt::Summary { failed, landed, .. }) => {
            assert_eq!(failed, &Some(UpdateFail::Offline));
            assert_eq!(*landed, 1);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn lid_close_cancels() {
    let (_d, mut a, art) = running();
    a.apply(Action::LidClose);
    assert!(art.asked().contains(&ArtJob::Cancel));
}

#[test]
fn stage_timeline() {
    assert_eq!(stage_at(0.0), (Stage::Arrive, 0.0));
    assert_eq!(stage_at(slot::fetch_art::stage_start(Stage::Peel) + 1.0).0, Stage::Peel);
    assert_eq!(stage_at(SCENE_MS - 1.0).0, Stage::Leave);
}

#[test]
fn relabel_reaches_favorites() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            favorites: vec![slot_store::Recent {
                platform: Platform::Gba,
                stem: "Fusion".into(),
            }],
            ..SlotState::default()
        },
    )
    .unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    let art = FakeArt::default();
    a.set_art_jobs(Box::new(art.clone()));
    a.open_fetch_art();
    press(&mut a, Btn::A);
    a.take_core_stale();
    let path = d.path().join("Labels/GBA/Fusion.png");
    art.send(&mut a, ArtEvent::Counted(1));
    art.send(
        &mut a,
        ArtEvent::Landed {
            stem: "Fusion".into(),
            platform: Platform::Gba,
            path: path.clone(),
        },
    );
    art.send(&mut a, ArtEvent::Done { landed: 1, missed: vec![] });
    advance(&mut a, SCENE_MS + 100.0);
    assert!(a.cart_labels(Platform::Gba, "Fusion").iter().all(Option::is_none));
    press(&mut a, Btn::B);
    advance(&mut a, 50.0);
    let copies = a.cart_labels(Platform::Gba, "Fusion");
    assert!(copies.len() >= 2, "{copies:?}");
    assert!(copies.iter().all(|l| l.as_deref() == Some(path.as_path())), "{copies:?}");
    assert!(a.take_core_stale());
}

#[test]
fn stop_waits_for_the_worker() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(3));
    press(&mut a, Btn::B);
    assert_eq!(art.asked().last(), Some(&ArtJob::Cancel));
    advance(&mut a, 100.0);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Running(_))), "{:?}", a.fetch_art());
    art.send(&mut a, landed("A"));
    art.send(&mut a, ArtEvent::Done { landed: 1, missed: vec![] });
    advance(&mut a, 100.0);
    match a.fetch_art() {
        Some(FetchArt::Summary { landed, .. }) => assert_eq!(*landed, 1),
        other => panic!("{other:?}"),
    }
    assert!(a.fetch_landed().iter().any(|l| l.stem == "A"));
}

#[test]
fn power_tap_cancels() {
    let (_d, mut a, art) = running();
    a.apply(Action::PowerTap);
    assert!(art.asked().contains(&ArtJob::Cancel));
}

#[test]
fn the_art_worker_holds_the_radio() {
    let (_d, mut a, art) = running();
    assert!(a.art_busy());
    art.send(&mut a, ArtEvent::Done { landed: 0, missed: vec![] });
    assert!(!a.art_busy());
}

#[test]
fn a_cart_off_the_shelf_does_not_hang() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(1));
    art.send(&mut a, landed("Ghost"));
    art.send(&mut a, ArtEvent::Done { landed: 1, missed: vec![] });
    advance(&mut a, 200.0);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Summary { .. })), "{:?}", a.fetch_art());
}

#[test]
fn choice_carts_fit_a_640_screen() {
    for w in [640.0f32, 720.0] {
        let [l, r] = slot::fetch_art::choice_xs(w);
        assert!(l >= 0.0 && r + 288.0 <= w, "{w}: {l} {r}");
        assert!(((l + r + 288.0) / 2.0 - w / 2.0).abs() < 1.0);
    }
}

#[test]
fn connecting_until_the_worker_counts() {
    let (_d, mut a, art) = running();
    assert_eq!(a.fetch_art_lines(), vec!["Connecting to Wi-Fi".to_string()]);
    art.send(&mut a, ArtEvent::Counted(2));
    assert_eq!(a.fetch_art_lines(), vec!["Fetching Art".to_string()]);
}

#[test]
fn summary_reads_as_rows() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(1));
    art.send(&mut a, ArtEvent::Missed { stem: "X".into() });
    art.send(&mut a, ArtEvent::Done { landed: 0, missed: vec!["X".into()] });
    advance(&mut a, 100.0);
    assert_eq!(a.fetch_art_lines(), ["All Done", "Added", "0", "No Art Found", "1"].map(String::from).to_vec());
}

#[test]
fn a_stopped_run_says_so() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(3));
    press(&mut a, Btn::B);
    art.send(&mut a, ArtEvent::Done { landed: 0, missed: vec![] });
    advance(&mut a, 100.0);
    assert_eq!(a.fetch_art_lines()[0], "Stopped");
}

#[test]
fn a_lost_connection_says_so() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Failed(UpdateFail::Offline));
    advance(&mut a, 100.0);
    assert_eq!(a.fetch_art_lines()[0], "Wi-Fi Lost");
}

#[test]
fn summary_features_a_cart_from_this_run() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("Emerald"));
    art.send(&mut a, landed("Fusion"));
    art.send(&mut a, ArtEvent::Done { landed: 2, missed: vec![] });
    advance(&mut a, SCENE_MS * 2.2);
    match a.fetch_art() {
        Some(FetchArt::Summary { featured: Some(cart), .. }) => {
            assert!(cart.stem == "Emerald" || cart.stem == "Fusion");
            assert_eq!(a.fetch_wanted(), vec![cart.clone()]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn nothing_landed_features_nothing() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Done { landed: 0, missed: vec![] });
    advance(&mut a, 100.0);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Summary { featured: None, .. })));
    assert!(a.fetch_wanted().is_empty());
}

#[test]
fn a_scene_reads_applying_labels() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("Emerald"));
    assert_eq!(
        a.fetch_art_lines(),
        ["Applying Labels", "Emerald", "1 of 2"].map(String::from).to_vec()
    );
}

#[test]
fn x_skips_the_animations() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(2));
    art.send(&mut a, landed("Emerald"));
    assert_eq!(scene_stem(&a).as_deref(), Some("Emerald"));
    press(&mut a, Btn::X);
    assert_eq!(scene_stem(&a), None);
    assert!(a.fetch_landed().iter().any(|l| l.stem == "Emerald"));
    assert_eq!(
        a.fetch_art_lines(),
        ["Applying Labels", "1 of 2"].map(String::from).to_vec()
    );
    art.send(&mut a, landed("Fusion"));
    assert_eq!(scene_stem(&a), None);
    assert_eq!(a.fetch_art_lines()[1], "2 of 2");
    assert!(a.fetch_wanted().is_empty());
    art.send(&mut a, ArtEvent::Done { landed: 2, missed: vec![] });
    assert!(matches!(a.fetch_art(), Some(FetchArt::Summary { .. })));
}

#[test]
fn a_scene_runs_about_two_and_a_quarter_seconds() {
    assert!((2000.0..=2500.0).contains(&SCENE_MS), "{SCENE_MS}");
}

#[test]
fn x_does_nothing_before_a_scene() {
    let (_d, mut a, art) = running();
    press(&mut a, Btn::X);
    art.send(&mut a, ArtEvent::Counted(1));
    press(&mut a, Btn::X);
    art.send(&mut a, landed("Emerald"));
    assert_eq!(scene_stem(&a).as_deref(), Some("Emerald"));
    assert!(a.fetch_can_skip());
}

#[test]
fn skipping_shows_a_progress_bar_and_no_carts() {
    use slot_gfx::Draw;
    let (_d, mut a, art) = running();
    a.set_fetch_text(vec![(slot_gfx::TexId::from_raw(50), 100, 30), (slot_gfx::TexId::from_raw(51), 60, 30)]);
    art.send(&mut a, ArtEvent::Counted(4));
    art.send(&mut a, landed("Emerald"));
    press(&mut a, Btn::X);
    art.send(&mut a, landed("Fusion"));
    a.tick_ms(a.now() + 1);
    assert_eq!(a.fetch_art_lines(), vec!["Applying Labels".to_string(), "2 of 4".to_string()]);
    assert!(a.fetch_wanted().is_empty(), "no cart should be built while skipping");
    let mut out = Vec::new();
    a.draw(&mut out);
    let rects: Vec<(f32, f32)> = out.iter().filter_map(|d| match d {
        Draw::Rect { w, h, .. } if *h == 14.0 && *w > 10.0 => Some((*w, *h)),
        _ => None,
    }).collect();
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert_eq!(rects[1].0, rects[0].0 / 2.0);
}

#[test]
fn the_summary_waits_for_its_cart_then_gives_up() {
    let (_d, mut a, art) = running();
    art.send(&mut a, ArtEvent::Counted(1));
    art.send(&mut a, landed("Emerald"));
    press(&mut a, Btn::X);
    a.require_fetch_faces();
    art.send(&mut a, ArtEvent::Done { landed: 1, missed: vec![] });
    assert!(matches!(a.fetch_art(), Some(FetchArt::Running(_))), "the summary opened without its cart");
    assert_eq!(a.fetch_wanted()[0].stem, "Emerald");
    assert_eq!(a.fetch_art_lines(), vec!["Applying Labels".to_string(), "1 of 1".to_string()]);
    a.tick_ms(a.now() + 500);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Running(_))));
    a.tick_ms(a.now() + 1500);
    assert!(matches!(a.fetch_art(), Some(FetchArt::Summary { .. })));
}

#[test]
fn no_skip_while_connecting() {
    let (_d, a, _art) = running();
    assert!(!a.fetch_can_skip());
}

#[test]
fn already_connected_skips_the_connecting_line() {
    let (_d, mut a, _art) = running();
    a.set_wifi(slot::wifi::Wifi::inline(Box::new(slot::wifi::Fake::demo())));
    a.wifi_send(slot::wifi::Cmd::Hold);
    a.tick_ms(a.now() + 1);
    assert_eq!(a.fetch_art_lines(), vec!["Connecting to Wi-Fi".to_string()]);
    a.wifi_send(slot::wifi::Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    a.tick_ms(a.now() + 1);
    assert_eq!(
        a.fetch_art_lines(),
        vec!["Fetching Art".to_string()]
    );
}
