mod common;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use common::{
    app_playing_in, boot, clocked, panel, session_with_platform, tmp_root_with_carts,
    tmp_root_with_real_carts, StubSnapshot,
};
use slot::app::Phase;
use slot::emu::Speed;
use slot::session::Session;
use slot_gfx::Draw;
use slot_input::{Action, Btn, Millis, RawEvent, POWER_HOLD_MS};
use slot_store::{read_slot_state, write_slot_state, Core, Platform, SlotState, StateRing};

use slot::link_radio::{RadioJob, RadioJobs};

const FRAME_MS: Millis = 16;
const DT: f32 = 1.0 / 60.0;

#[test]
fn lid_close_flushes_resume_before_dozing() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::LidClose);
    let r = StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald");
    assert!(
        r.read_resume().unwrap().is_some(),
        "state must be durable before doze"
    );
    assert!(matches!(a.phase(), Phase::Doze { .. }));
    assert!(
        r.list().unwrap().is_empty(),
        "lid close must not create a polaroid"
    );
}

#[test]
fn lid_open_returns_to_the_game_without_a_button() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn doze_timeout_powers_off_with_the_cart_still_seated() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::LidClose);
    a.on_doze_timeout();
    assert_eq!(
        read_slot_state(d.path()).cart,
        Some("Emerald".into()),
        "power off is not an eject"
    );
}

#[test]
fn lid_close_on_the_shelf_wakes_back_to_the_shelf() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = boot(d.path());
    a.set_snapshot(StubSnapshot::boxed());
    a.apply(Action::LidClose);
    assert!(matches!(a.phase(), Phase::Doze { cart: None }));
    assert!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .read_resume()
            .unwrap()
            .is_none()
    );
    a.apply(Action::LidOpen);
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn lid_close_over_the_switcher_wakes_into_the_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::SaveState);
    a.apply(Action::Polaroids);
    a.apply(Action::LidClose);
    a.apply(Action::LidOpen);
    assert!(matches!(a.phase(), Phase::Playing { .. }));
    assert_eq!(
        StateRing::new(d.path(), Platform::Gba, Core::Mgba, "Emerald")
            .list()
            .unwrap()
            .len(),
        1,
        "only the deliberate save belongs in the ring"
    );
}

#[test]
fn a_doze_that_outlasts_the_timeout_powers_off_by_itself() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_power(panel(d.path(), Duration::from_secs(2)).0);
    a.apply(Action::LidClose);
    for _ in 0..100 {
        a.update(1.0 / 60.0);
    }
    assert!(!a.powering_off(), "1.6 s is short of the 2 s timeout");
    for _ in 0..40 {
        a.update(1.0 / 60.0);
    }
    assert!(a.powering_off());
}

#[test]
fn a_stray_doze_timeout_does_not_power_off_a_running_game() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.on_doze_timeout();
    assert!(!a.powering_off());
    assert!(matches!(a.phase(), Phase::Playing { .. }));
}

#[test]
fn the_backlight_follows_brightness_from_boot() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            brightness: 3,
            clock_set: true,
            utc_offset_min: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let mut a = boot(d.path());
    let (power, step) = panel(d.path(), Duration::from_secs(60));
    a.set_power(power);
    assert_eq!(step.load(Ordering::Relaxed), 3);
    a.apply(Action::BrightnessUp);
    assert_eq!(step.load(Ordering::Relaxed), 4);
}

#[test]
fn a_power_off_draws_a_shutdown_screen_over_everything() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerHold);

    let mut out = Vec::new();
    a.draw(&mut out);

    match out.first() {
        Some(Draw::Rect { w, h, colour, .. }) => {
            assert_eq!(*colour, [0.0, 0.0, 0.0, 1.0], "the shutdown is black");
            assert!(*w > 0.0 && *h > 0.0, "and covers the panel");
        }
        other => panic!("the shutdown drew {other:?} rather than a panel of black"),
    }
    assert_eq!(
        out.len(),
        1,
        "nothing of the previous phase survives the shutdown screen"
    );
}

#[test]
fn the_shutdown_screen_is_up_before_the_machine_may_stop() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerHold);

    assert!(a.powering_off(), "the hold starts shutdown immediately");
    assert!(
        !a.ready_to_power_off(),
        "but the binary may not act until the screen has been presented"
    );

    let mut out = Vec::new();
    a.draw(&mut out);
    assert!(
        matches!(out.first(), Some(Draw::Rect { colour, .. }) if *colour == [0.0, 0.0, 0.0, 1.0]),
        "and the screen is what the loop is drawing in the meantime"
    );

    a.tick_ms(600_000);
    assert!(a.ready_to_power_off(), "then it may stop");
}

#[test]
fn a_dozing_device_powers_off_by_itself_rather_than_waiting_for_the_lid() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_power(panel(d.path(), Duration::from_secs(2)).0);
    a.apply(Action::LidClose);
    for _ in 0..180 {
        a.update(1.0 / 60.0);
    }
    assert!(
        a.powering_off(),
        "three seconds is past the two second timeout"
    );
    assert!(
        a.ready_to_power_off(),
        "the shutdown screen has had its 250 ms and the machine is still not allowed to stop"
    );
}

/// A device with five seconds of rcK ahead of it is
#[test]
fn a_committed_shutdown_holds_the_core_still() {
    let d = tmp_root_with_real_carts(&["Advance Wars", "Emerald"]);
    let (mut s, _motor) = session_with_platform(d.path());
    let mut now = 0;
    play(&mut s, &mut now);

    hold_power(&mut s, &mut now);
    assert!(s.app().powering_off(), "the hold starts shutdown");
    s.update(DT);
    await_paused(&mut s);
}

#[test]
fn power_pressed_while_dozing_lights_the_panel_before_shutdown() {
    let d = tmp_root_with_carts(&["Emerald"]);
    clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    let (power, backlight) = panel(d.path(), Duration::from_secs(180));
    s.app_mut().set_power(power);
    let lit = backlight.load(Ordering::Relaxed);
    assert!(
        lit > 0,
        "the panel never came on, so going dark proves nothing"
    );

    let mut now = 0;
    event(&mut s, RawEvent::Down(Btn::Power), &mut now);
    event(&mut s, RawEvent::Up(Btn::Power), &mut now);
    assert!(
        matches!(s.app().phase(), Phase::Doze { .. }),
        "the device never dozed"
    );
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        0,
        "the doze left the panel lit"
    );

    event(&mut s, RawEvent::Down(Btn::Power), &mut now);
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        lit,
        "the panel is still dark under the thumb trying to wake it"
    );
    assert!(
        !matches!(s.app().phase(), Phase::Doze { .. }),
        "the panel came on over a device still dozing behind it"
    );

    // turns the device off — and now its message is on a panel the user can read.
    let pressed = now;
    while now < pressed + POWER_HOLD_MS + FRAME_MS {
        step(&mut s, &mut now);
    }
    assert!(s.app().powering_off(), "the hold no longer starts shutdown");
    assert_eq!(
        backlight.load(Ordering::Relaxed),
        lit,
        "the shutdown message is on a panel nobody can see"
    );
}

#[test]
fn the_press_that_woke_the_panel_does_not_doze_again_when_it_is_let_go() {
    let d = tmp_root_with_carts(&["Emerald"]);
    clocked(d.path());
    let mut s = Session::boot(d.path().to_path_buf());
    let (power, backlight) = panel(d.path(), Duration::from_secs(180));
    s.app_mut().set_power(power);
    let lit = backlight.load(Ordering::Relaxed);

    let mut now = 0;
    event(&mut s, RawEvent::Down(Btn::Power), &mut now);
    event(&mut s, RawEvent::Up(Btn::Power), &mut now);
    assert!(matches!(s.app().phase(), Phase::Doze { .. }));

    event(&mut s, RawEvent::Down(Btn::Power), &mut now);
    event(&mut s, RawEvent::Up(Btn::Power), &mut now);
    assert!(
        !matches!(s.app().phase(), Phase::Doze { .. }),
        "the tap that woke the device put it straight back to sleep"
    );
    assert_eq!(backlight.load(Ordering::Relaxed), lit);

    event(&mut s, RawEvent::Down(Btn::Power), &mut now);
    event(&mut s, RawEvent::Up(Btn::Power), &mut now);
    assert!(
        matches!(s.app().phase(), Phase::Doze { .. }),
        "POWER stopped being able to put the device out"
    );
    assert_eq!(backlight.load(Ordering::Relaxed), 0);
}

fn await_paused(s: &mut Session) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while s.observed_speed() != Some(Speed::Paused) {
        assert!(
            Instant::now() < deadline,
            "the core was still running behind the shutdown"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// POWER down, then frames until the gesture layer's hold threshold fires.
fn hold_power(s: &mut Session, now: &mut Millis) {
    let pressed = *now;
    event(s, RawEvent::Down(Btn::Power), now);
    while *now < pressed + POWER_HOLD_MS + FRAME_MS {
        step(s, now);
    }
}

fn step(s: &mut Session, now: &mut Millis) {
    *now += FRAME_MS;
    s.feed([], *now);
    s.update(DT);
}

fn event(s: &mut Session, ev: RawEvent, now: &mut Millis) {
    *now += FRAME_MS;
    s.feed([ev], *now);
    s.update(DT);
}

fn play(s: &mut Session, now: &mut Millis) {
    event(s, RawEvent::Down(Btn::A), now);
    event(s, RawEvent::Up(Btn::A), now);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(s, now);
        std::thread::sleep(Duration::from_millis(1));
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while s.observed_speed() != Some(Speed::Normal) {
        assert!(Instant::now() < deadline, "the core never started");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[derive(Clone, Default)]
struct RadioLog(std::sync::Arc<std::sync::Mutex<Vec<RadioJob>>>);

impl RadioLog {
    fn jobs(&self) -> Vec<RadioJob> {
        self.0.lock().expect("radio log").clone()
    }
}

impl RadioJobs for RadioLog {
    fn ask(&mut self, job: RadioJob) {
        self.0.lock().expect("radio log").push(job);
    }

    fn warmed(&self) -> bool {
        false
    }
}

#[test]
fn a_shut_lid_cools_the_radio() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let log = RadioLog::default();
    a.set_radio_jobs(Box::new(log.clone()));
    a.apply(Action::LidClose);
    assert!(
        log.jobs().contains(&RadioJob::Cool),
        "the lid closed with the radio left loaded behind it"
    );
}

#[test]
fn a_shut_lid_over_a_session_takes_its_network_down_too() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    let log = RadioLog::default();
    a.set_radio_jobs(Box::new(log.clone()));
    a.begin_link(0);
    a.apply(Action::LidClose);
    let jobs = log.jobs();
    assert_eq!(
        jobs.first(),
        Some(&RadioJob::Down),
        "the session's own network must come down before the driver does: {jobs:?}"
    );
    assert!(jobs.contains(&RadioJob::Cool));
}

/// Releasing POWER and pressing a former menu button cannot cancel or delay shutdown.
#[test]
fn shutdown_ignores_further_input_without_resetting_its_deadline() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.apply(Action::PowerHold);
    let held_at = a.now();
    a.tick_ms(held_at + 200);
    for action in [
        Action::PowerOff,
        Action::PowerHold,
        Action::GbaDown(Btn::B),
        Action::GbaDown(Btn::A),
        Action::PowerTap,
        Action::LidClose,
        Action::LidOpen,
    ] {
        a.apply(action);
        assert!(a.powering_off());
    }
    a.tick_ms(held_at + 250);
    assert!(a.ready_to_power_off());
}
