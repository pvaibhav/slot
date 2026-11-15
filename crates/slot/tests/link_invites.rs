mod common;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use slot::app::{App, GameMenu, LinkRow, Phase};
use slot::ble::{BleWant, FakeBle, FakeBleRemote};
use slot::invite::decode;
use slot::link_net::{Cancel, TcpLink};
use slot::link_radio::{HostAp, RadioRole};
use slot::link_start::{LinkFail, LinkStarter, LinkStep};
use slot_input::{Action, Btn};
use slot_store::{Core, Platform};
use tempfile::TempDir;

const BAIL: Duration = Duration::from_secs(5);

const AP: HostAp = HostAp {
    freq_mhz: 5180,
    bssid: [2, 0, 0, 0, 0, 1],
};

fn ap(last: u8) -> HostAp {
    HostAp {
        freq_mhz: 5745,
        bssid: [2, 0, 0, 0, 0, last],
    }
}

type Roles = Arc<Mutex<Vec<RadioRole>>>;

fn playing_on(core: Core) -> (App, TempDir, FakeBleRemote, Roles) {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let mut app = common::boot(d.path());
    app.apply(Action::Insert);
    app.set_core(core);
    app.on_core_ready();
    for _ in 0..120 {
        app.update(1.0 / 60.0);
    }
    assert!(matches!(app.phase(), Phase::Playing { .. }), "never seated");
    let (ble, remote) = FakeBle::new();
    app.set_ble(Box::new(ble));
    let roles: Roles = Arc::new(Mutex::new(Vec::new()));
    let seen = roles.clone();
    app.set_link_spawn(Box::new(move |role, _port, held| {
        seen.lock().unwrap().push(role);
        LinkStarter::spawn_held_with(
            Box::new(|role, _| {
                Ok(match role {
                    RadioRole::Host => Some(AP),
                    RadioRole::Join(_) => None,
                })
            }),
            Box::new(|| {}),
            role,
            0,
            Box::new(|_, cancel| until_cancelled(cancel)),
            held,
        )
    }));
    (app, d, remote, roles)
}

fn until_cancelled(cancel: &Cancel) -> io::Result<TcpLink> {
    let deadline = Instant::now() + BAIL;
    while !cancel.is_cancelled() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(io::Error::new(io::ErrorKind::Interrupted, "from a test"))
}

fn pump(app: &mut App, done: impl Fn(&App) -> bool) -> bool {
    let deadline = Instant::now() + BAIL;
    while !done(app) {
        if Instant::now() >= deadline {
            return false;
        }
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    true
}

fn idle(app: &mut App, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn last_want(ble: &FakeBleRemote) -> Option<BleWant> {
    ble.wants().last().cloned()
}

fn host(app: &mut App, ble: &FakeBleRemote) {
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::A));
    assert!(
        pump(app, |_| matches!(
            last_want(ble),
            Some(BleWant::Advertise { .. })
        )),
        "the host never advertised: {:?}",
        ble.wants()
    );
}

fn join(app: &mut App) {
    app.apply(Action::GameMenu);
    app.apply(Action::GbaDown(Btn::Right));
    app.apply(Action::GbaDown(Btn::A));
}

fn waiting(app: &App) -> bool {
    matches!(
        app.game_menu(),
        Some(GameMenu::Working {
            role: LinkRow::Join,
            step: LinkStep::Waiting,
            ..
        })
    )
}

fn cancel(app: &mut App) {
    app.apply(Action::GbaDown(Btn::B));
    assert!(
        pump(app, |a| !a.game_menu_open()),
        "the cancel never landed"
    );
}

#[test]
fn hosting_advertises_once_the_hotspot_is_up() {
    let (mut app, d, ble, roles) = playing_on(Core::Gpsp);
    host(&mut app, &ble);
    assert_eq!(*roles.lock().unwrap(), vec![RadioRole::Host]);
    let Some(BleWant::Advertise { advert, scan }) = last_want(&ble) else {
        unreachable!();
    };
    let advert = decode(&advert).expect("the advert does not decode");
    let emerald = common::heard_host(&d, "Emerald", 1, AP, 0).advert;
    assert_eq!(advert.freq_mhz, AP.freq_mhz);
    assert_eq!(advert.bssid, AP.bssid);
    assert_eq!(advert.game_crc, emerald.game_crc);
    assert_eq!(advert.platform, Platform::Gba);
    assert_eq!(advert.core, Core::Gpsp);
    assert_ne!(advert.session, 0);
    assert_eq!(
        slot::invite::nickname_of(&scan),
        Some(app.nickname()),
        "the scan response does not carry the nickname"
    );
    cancel(&mut app);
}

#[test]
fn hosting_stops_advertising_when_cancelled() {
    let (mut app, _d, ble, _roles) = playing_on(Core::Gpsp);
    host(&mut app, &ble);
    cancel(&mut app);
    assert_eq!(last_want(&ble), Some(BleWant::Off));
}

#[test]
fn join_listens_then_joins_the_matching_host() {
    let (mut app, d, ble, roles) = playing_on(Core::Gpsp);
    join(&mut app);
    assert_eq!(last_want(&ble), Some(BleWant::Listen));
    assert!(waiting(&app), "Join did not wait: {:?}", app.game_menu());
    assert!(roles.lock().unwrap().is_empty(), "Join spawned a starter");
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(
        pump(&mut app, |_| !roles.lock().unwrap().is_empty()),
        "the matching host was never joined"
    );
    assert_eq!(*roles.lock().unwrap(), vec![RadioRole::Join(AP)]);
    assert_eq!(app.join_target(), Some(AP));
    assert_eq!(last_want(&ble), Some(BleWant::Off));
    cancel(&mut app);
}

#[test]
fn join_ignores_a_different_game() {
    let (mut app, d, ble, roles) = playing_on(Core::Gpsp);
    join(&mut app);
    let mut other = common::heard_host(&d, "Emerald", 7, AP, -60);
    other.advert.game_crc ^= 1;
    ble.hear(other);
    idle(&mut app, 200);
    assert!(
        waiting(&app),
        "a different game moved the menu: {:?}",
        app.game_menu()
    );
    assert!(
        roles.lock().unwrap().is_empty(),
        "a different game was joined"
    );
    assert_eq!(last_want(&ble), Some(BleWant::Listen));
    ble.hear(common::heard_host(&d, "Emerald", 8, AP, -60));
    assert!(
        pump(&mut app, |_| !roles.lock().unwrap().is_empty()),
        "the matching host after it was never joined"
    );
    cancel(&mut app);
}

#[test]
fn join_gives_up_after_the_host_bound() {
    let (mut app, _d, ble, roles) = playing_on(Core::Gpsp);
    join(&mut app);
    for _ in 0..31 {
        app.update(1.0);
    }
    assert!(
        matches!(
            app.game_menu(),
            Some(GameMenu::Failed {
                role: LinkRow::Join,
                fail: LinkFail::NobodyCame,
                ..
            })
        ),
        "the join did not give up: {:?}",
        app.game_menu()
    );
    assert_eq!(last_want(&ble), Some(BleWant::Ready));
    assert!(roles.lock().unwrap().is_empty());
}

#[test]
fn the_stronger_host_wins_and_sticks() {
    let (mut app, d, ble, roles) = playing_on(Core::Gpsp);
    join(&mut app);
    ble.hear(common::heard_host(&d, "Emerald", 1, ap(1), -70));
    ble.hear(common::heard_host(&d, "Emerald", 2, ap(2), -50));
    assert!(
        pump(&mut app, |_| !roles.lock().unwrap().is_empty()),
        "neither host was joined"
    );
    assert_eq!(*roles.lock().unwrap(), vec![RadioRole::Join(ap(2))]);
    ble.hear(common::heard_host(&d, "Emerald", 3, ap(3), -40));
    idle(&mut app, 100);
    assert_eq!(*roles.lock().unwrap(), vec![RadioRole::Join(ap(2))]);
    assert_eq!(app.join_target(), Some(ap(2)));
    cancel(&mut app);
}

fn kid_state(locked: bool, cart: Option<&str>) -> slot_store::SlotState {
    slot_store::SlotState {
        clock_set: true,
        cart: cart.map(str::to_string),
        kid_mode: match locked {
            true => slot_store::KidMode::Shelves,
            false => slot_store::KidMode::Off,
        },
        ..slot_store::SlotState::default()
    }
}

fn kid_card(locked: bool, cart: Option<&str>) -> TempDir {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    slot_store::write_slot_state(d.path(), &kid_state(locked, cart)).expect("write slot.state");
    d
}

fn kid_on_shelf(locked: bool) -> (App, TempDir, FakeBleRemote) {
    let d = kid_card(locked, None);
    let mut app = App::boot(d.path());
    let (ble, remote) = FakeBle::new();
    app.set_ble(Box::new(ble));
    idle(&mut app, 50);
    assert!(matches!(app.phase(), Phase::Shelf), "{:?}", app.phase());
    (app, d, remote)
}

fn kid_playing(stem: &str) -> (App, TempDir, FakeBleRemote) {
    let d = kid_card(true, Some(stem));
    let mut app = App::boot(d.path());
    app.set_snapshot(common::StubSnapshot::boxed());
    app.on_core_ready();
    assert!(
        pump(&mut app, |a| matches!(a.phase(), Phase::Playing { .. })),
        "never seated: {:?}",
        app.phase()
    );
    let (ble, remote) = FakeBle::new();
    app.set_ble(Box::new(ble));
    idle(&mut app, 50);
    (app, d, remote)
}

fn card_up(app: &mut App) -> bool {
    pump(app, |a| a.invite_card().is_some())
}

fn shown_session(app: &App) -> Option<u16> {
    app.invite_card().map(|c| c.heard.advert.session)
}

#[test]
fn a_kid_on_the_shelf_listens() {
    let (mut app, _d, ble) = kid_on_shelf(true);
    assert!(
        pump(&mut app, |_| last_want(&ble) == Some(BleWant::Listen)),
        "the kid's shelf never listened: {:?}",
        ble.wants()
    );
}

#[test]
fn an_unlocked_shelf_never_listens() {
    let (mut app, _d, ble) = kid_on_shelf(false);
    idle(&mut app, 200);
    assert!(
        matches!(last_want(&ble), None | Some(BleWant::Off)),
        "an unlocked shelf turned Bluetooth on: {:?}",
        ble.wants()
    );
    assert!(!ble.wants().contains(&BleWant::Listen));
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update(1.0);
    }
}

#[test]
fn bluetooth_stays_off_through_boot_then_warms() {
    let (mut app, _d, ble) = kid_on_shelf(false);
    assert!(ble.wants().is_empty(), "{:?}", ble.wants());
    settle(&mut app);
    assert_eq!(last_want(&ble), Some(BleWant::Ready), "{:?}", ble.wants());
}

#[test]
fn closing_the_lid_cools_bluetooth_and_opening_warms_it() {
    let (mut app, _d, ble) = kid_on_shelf(false);
    settle(&mut app);
    assert_eq!(last_want(&ble), Some(BleWant::Ready));
    app.apply(Action::LidClose);
    assert!(
        pump(&mut app, |_| last_want(&ble) == Some(BleWant::Off)),
        "the closed lid kept Bluetooth on: {:?}",
        ble.wants()
    );
    app.apply(Action::LidOpen);
    settle(&mut app);
    assert_eq!(last_want(&ble), Some(BleWant::Ready), "{:?}", ble.wants());
}

#[test]
fn a_matching_advert_opens_the_card_on_the_shelf() {
    let (mut app, d, ble) = kid_on_shelf(true);
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(card_up(&mut app), "the card never opened");
    let card = app.invite_card().expect("a card");
    assert_eq!(card.stem, "Emerald");
    assert_eq!(card.title, "Emerald");
    assert_eq!(card.nickname(), "Dad");
    assert_eq!(card.heard.advert.session, 7);
}

#[test]
fn a_different_game_in_play_shows_nothing() {
    let (mut app, d, ble) = kid_playing("Emerald");
    ble.hear(common::heard_host(&d, "Zzz", 7, AP, -60));
    idle(&mut app, 300);
    assert!(app.invite_card().is_none(), "a card over a different game");
    ble.hear(common::heard_host(&d, "Emerald", 8, AP, -60));
    assert!(card_up(&mut app), "the running game's invite never showed");
    assert_eq!(shown_session(&app), Some(8));
}

#[test]
fn the_same_session_shows_one_card() {
    let (mut app, d, ble) = kid_on_shelf(true);
    for _ in 0..50 {
        ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(card_up(&mut app), "the card never opened");
    app.apply(Action::GbaDown(Btn::B));
    app.apply(Action::GbaUp(Btn::B));
    assert!(app.invite_card().is_none(), "B left the card up");
    for _ in 0..50 {
        ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    idle(&mut app, 100);
    assert!(
        app.invite_card().is_none(),
        "an ignored session showed again"
    );
}

#[test]
fn a_new_session_from_the_same_host_shows_again() {
    let (mut app, d, ble) = kid_on_shelf(true);
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(card_up(&mut app), "the card never opened");
    app.apply(Action::GbaDown(Btn::B));
    assert!(app.invite_card().is_none(), "B left the card up");
    ble.hear(common::heard_host(&d, "Emerald", 9, AP, -60));
    assert!(card_up(&mut app), "a new session never showed");
    assert_eq!(shown_session(&app), Some(9));
}

#[test]
fn a_kid_powering_off_turns_bluetooth_off_and_shows_no_card() {
    let (mut app, d, ble) = kid_on_shelf(true);
    assert!(
        pump(&mut app, |_| last_want(&ble) == Some(BleWant::Listen)),
        "the kid's shelf never listened: {:?}",
        ble.wants()
    );
    app.apply(Action::PowerHold);
    assert!(app.powering_off(), "the hold did not power off");
    for _ in 0..30 {
        ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
        app.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    idle(&mut app, 200);
    assert_eq!(last_want(&ble), Some(BleWant::Off), "{:?}", ble.wants());
    assert!(app.invite_card().is_none(), "a card over Powering Down");
    app.poweroff();
    assert_eq!(last_want(&ble), Some(BleWant::Off), "{:?}", ble.wants());
}

#[test]
fn a_vanished_host_closes_the_card() {
    let (mut app, d, ble) = kid_on_shelf(true);
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(card_up(&mut app), "the card never opened");
    idle(&mut app, 2500);
    assert!(app.invite_card().is_some(), "the card closed too soon");
    idle(&mut app, 600);
    assert!(
        app.invite_card().is_none(),
        "the card stayed up after the host went quiet"
    );
}

#[test]
fn the_card_holds_the_game() {
    use slot::emu::Speed;
    use slot::session::Session;
    use slot_input::{Millis, RawEvent};

    fn step(s: &mut Session, now: &mut Millis, events: &[RawEvent]) {
        *now += 16;
        s.feed(events.iter().copied(), *now);
        s.update(1.0 / 60.0);
    }
    fn runs_at(s: &mut Session, now: &mut Millis, want: Speed) -> bool {
        let deadline = Instant::now() + BAIL;
        while s.observed_speed() != Some(want) {
            if Instant::now() >= deadline {
                return false;
            }
            step(s, now, &[]);
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_retail_header(&d, "Emerald", "POKEMON RUBY", "AXVE");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", Core::Mgba)
        .expect("write core");
    slot_store::write_slot_state(d.path(), &kid_state(true, Some("Emerald")))
        .expect("write slot.state");
    let mut s = Session::boot(d.path().to_path_buf());
    let mut now: Millis = 0;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    let (ble, remote) = FakeBle::new();
    s.app_mut().set_ble(Box::new(ble));
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "the game never ran"
    );
    let deadline = Instant::now() + BAIL;
    while s.app().invite_card().is_none() {
        assert!(Instant::now() < deadline, "the card never opened");
        remote.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
        step(&mut s, &mut now, &[]);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        runs_at(&mut s, &mut now, Speed::Paused),
        "the game ran on behind the card"
    );
    step(&mut s, &mut now, &[RawEvent::Down(Btn::B)]);
    step(&mut s, &mut now, &[RawEvent::Up(Btn::B)]);
    assert!(s.app().invite_card().is_none(), "B left the card up");
    assert!(
        runs_at(&mut s, &mut now, Speed::Normal),
        "Not now left the game paused"
    );
}

#[derive(Clone, Copy)]
enum Far {
    Waits,
    NobodyCame,
    Hosts(u16),
}

type Spawned = Arc<Mutex<Vec<(RadioRole, bool)>>>;

struct Kid {
    s: slot::session::Session,
    d: TempDir,
    now: slot_input::Millis,
    ble: FakeBleRemote,
    spawned: Spawned,
}

impl Kid {
    fn step(&mut self, events: &[slot_input::RawEvent]) {
        self.now += 16;
        self.s.feed(events.iter().copied(), self.now);
        self.s.update(1.0 / 60.0);
    }

    fn until(&mut self, what: &str, done: impl Fn(&Kid) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while !done(self) {
            assert!(
                Instant::now() < deadline,
                "{what}: phase {:?}, menu {:?}, core {:?}",
                self.s.app().phase(),
                self.s.app().game_menu(),
                self.s.app().core()
            );
            self.step(&[]);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn press(&mut self, btn: Btn) {
        self.step(&[slot_input::RawEvent::Down(btn)]);
        self.step(&[slot_input::RawEvent::Up(btn)]);
    }

    fn card_for(&mut self, heard: impl Fn(&TempDir) -> slot::ble::Heard) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.s.app().invite_card().is_none() {
            assert!(Instant::now() < deadline, "the card never opened");
            self.ble.hear(heard(&self.d));
            self.step(&[]);
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn ready_on(&mut self, core: Core) {
        self.until("the game never came up on the expected core", |k| {
            k.s.app().core() == core
                && matches!(k.s.app().phase(), Phase::Playing { .. })
                && k.s.emu().map(slot::emu::EmuHandle::state) == Some(slot::emu::CoreState::Ready)
        });
    }

    fn joined(&self) -> Vec<RadioRole> {
        self.spawned
            .lock()
            .unwrap()
            .iter()
            .map(|(r, _)| *r)
            .collect()
    }

    fn resume(&self, platform: Platform, core: Core, stem: &str) -> Option<Vec<u8>> {
        slot::persist::read_resume(self.d.path(), platform, core, stem).expect("read resume")
    }
}

type RadioUp = Box<
    dyn FnMut(RadioRole, &Cancel) -> Result<Option<HostAp>, slot::link_radio::RadioFail> + Send,
>;
type Socket = Box<dyn FnMut(u16, &Cancel) -> io::Result<TcpLink> + Send>;

fn far_spawn(
    root: std::path::PathBuf,
    watch: (Platform, Core, &'static str),
    far: Far,
    spawned: Spawned,
) -> Box<dyn FnMut(RadioRole, u16, bool) -> LinkStarter + Send> {
    Box::new(move |role, _port, held| {
        let root = root.clone();
        let spawned = spawned.clone();
        let opened = move || {
            let resumed = matches!(
                slot::persist::read_resume(&root, watch.0, watch.1, watch.2),
                Ok(Some(_))
            );
            spawned.lock().unwrap().push((role, resumed));
        };
        let radio: RadioUp = match far {
            Far::NobodyCame => Box::new(|_, _| Err(slot::link_radio::RadioFail::NoHost)),
            _ => Box::new(|_, _| Ok(None)),
        };
        let socket: Socket = match far {
            Far::Hosts(port) => Box::new(move |_, _| {
                opened();
                TcpLink::join("127.0.0.1", port)
            }),
            _ => Box::new(move |_, cancel| {
                opened();
                until_cancelled(cancel)
            }),
        };
        LinkStarter::spawn_held_with(radio, Box::new(|| {}), role, 0, socket, held)
    })
}

fn kid_slot(mode: slot_store::KidMode, cart: Option<(&str, Platform)>) -> slot_store::SlotState {
    slot_store::SlotState {
        clock_set: true,
        cart: cart.map(|(stem, _)| stem.to_string()),
        cart_platform: cart.map(|(_, p)| p),
        kid_mode: mode,
        ..slot_store::SlotState::default()
    }
}

fn kid_session(
    d: TempDir,
    state: slot_store::SlotState,
    watch: (Platform, Core, &'static str),
    far: Far,
) -> Kid {
    slot_store::write_slot_state(d.path(), &state).expect("write slot.state");
    let s = slot::session::Session::boot(d.path().to_path_buf());
    let (ble, remote) = FakeBle::new();
    let spawned: Spawned = Arc::new(Mutex::new(Vec::new()));
    let mut kid = Kid {
        s,
        d,
        now: 0,
        ble: remote,
        spawned: spawned.clone(),
    };
    let seated = state.cart.is_some();
    kid.until("the card never settled", |k| match seated {
        true => matches!(k.s.app().phase(), Phase::Playing { .. }),
        false => matches!(k.s.app().phase(), Phase::Shelf),
    });
    kid.s.app_mut().set_ble(Box::new(ble));
    let root = kid.d.path().to_path_buf();
    kid.s
        .app_mut()
        .set_link_spawn(far_spawn(root, watch, far, spawned));
    kid
}

fn emerald_on(core: Core, far: Far) -> Kid {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", core).expect("write core");
    let mut kid = kid_session(
        d,
        kid_slot(
            slot_store::KidMode::Shelves,
            Some(("Emerald", Platform::Gba)),
        ),
        (Platform::Gba, core, "Emerald"),
        far,
    );
    kid.ready_on(core);
    kid
}

fn emerald_from(core: Core) -> impl Fn(&TempDir) -> slot::ble::Heard {
    move |d| {
        let mut heard = common::heard_host(d, "Emerald", 7, AP, -60);
        heard.advert.core = core;
        heard
    }
}

fn selected_cores(d: &TempDir) -> Vec<u8> {
    std::fs::read(d.path().join(slot_store::SELECTED_CORE_FILE)).expect("read the core choices")
}

fn host_far(port: u16) -> std::thread::JoinHandle<TcpLink> {
    std::thread::spawn(move || TcpLink::host("127.0.0.1", port).expect("host"))
}

#[test]
fn accepting_in_game_writes_the_resume_state_first() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    assert_eq!(
        kid.resume(Platform::Gba, Core::Gpsp, "Emerald"),
        None,
        "the card already had a resume state"
    );
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    assert!(
        kid.resume(Platform::Gba, Core::Gpsp, "Emerald").is_some(),
        "A did not write the resume state"
    );
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert_eq!(
        *kid.spawned.lock().unwrap(),
        vec![(RadioRole::Join(AP), true)],
        "a starter was spawned before the resume state was on the card"
    );
}

#[test]
fn accepting_switches_to_the_hosts_core() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    kid.ready_on(Core::Mgba);
    assert_eq!(kid.s.app().link_core(), Some(Core::Mgba));
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert_eq!(kid.joined(), vec![RadioRole::Join(AP)]);
    assert_eq!(kid.s.app().join_target(), Some(AP));
}

#[test]
fn the_switch_lasts_one_link() {
    let port = common::free_port();
    let mut kid = emerald_on(Core::Gpsp, Far::Hosts(port));
    let before = selected_cores(&kid.d);
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let _far = far.join().expect("host thread");
    assert_eq!(kid.s.app().core(), Core::Mgba);
    kid.until("LINKED never handed the game back", |k| {
        !k.s.app().game_menu_open()
    });
    kid.s.app_mut().apply(Action::QuickMenu);
    assert!(!kid.s.app().link_active(), "eject left the link up");
    assert_eq!(kid.s.app().link_core(), None);
    assert_eq!(kid.s.app().join_target(), None);
    kid.until("the cart never came out", |k| {
        matches!(k.s.app().phase(), Phase::Shelf)
    });
    assert_eq!(kid.s.app().selected_stem(), Some("Emerald"));
    kid.s.app_mut().apply(Action::Insert);
    kid.ready_on(Core::Gpsp);
    assert_eq!(
        selected_cores(&kid.d),
        before,
        "the link wrote the kid's core choice"
    );
}

#[test]
fn accepting_from_the_shelf_starts_the_game() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let mut kid = kid_session(
        d,
        kid_slot(slot_store::KidMode::Shelves, None),
        (Platform::Gba, Core::Gpsp, "Emerald"),
        Far::Waits,
    );
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    assert!(
        matches!(kid.s.app().phase(), Phase::Inserting { cart, .. } if cart == "Emerald"),
        "A did not start the game: {:?}",
        kid.s.app().phase()
    );
    assert_eq!(kid.s.app().join_target(), Some(AP));
    kid.ready_on(Core::Gpsp);
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert_eq!(kid.joined(), vec![RadioRole::Join(AP)]);
}

fn returns_to_favorites(mode: slot_store::KidMode) {
    let d = common::tmp_root_with_carts(&["Aaa", "Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    let favorite = |stem: &str| slot_store::Recent {
        platform: Platform::Gba,
        stem: stem.into(),
    };
    let state = slot_store::SlotState {
        favorites: vec![favorite("Aaa"), favorite("Zzz")],
        kid_shelf: "favorites".into(),
        ..kid_slot(mode, None)
    };
    let port = common::free_port();
    let mut kid = kid_session(
        d,
        state,
        (Platform::Gba, Core::Gpsp, "Emerald"),
        Far::Hosts(port),
    );
    if mode == slot_store::KidMode::Shelves {
        for _ in 0..8 {
            if kid.s.app().shelf_platform_name() == Some("Favorites") {
                break;
            }
            kid.s.app_mut().apply(Action::NextShelf);
            kid.step(&[]);
        }
        assert_eq!(kid.s.app().shelf_platform_name(), Some("Favorites"));
    }
    assert_eq!(kid.s.app().selected_stem(), Some("Aaa"));
    kid.s.app_mut().apply(Action::GbaDown(Btn::Right));
    kid.s.app_mut().apply(Action::GbaUp(Btn::Right));
    kid.until("the shelf never moved", |k| {
        k.s.app().selected_stem() == Some("Zzz")
    });
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let _far = far.join().expect("host thread");
    kid.until("LINKED never handed the game back", |k| {
        !k.s.app().game_menu_open()
    });
    kid.s.app_mut().apply(Action::QuickMenu);
    kid.until("the cart never came out", |k| {
        matches!(k.s.app().phase(), Phase::Shelf)
    });
    assert_eq!(kid.s.app().selected_stem(), Some("Zzz"));
    kid.s.app_mut().apply(Action::GbaDown(Btn::Left));
    kid.s.app_mut().apply(Action::GbaUp(Btn::Left));
    kid.until("the shelf never moved back", |k| {
        k.s.app().selected_stem() != Some("Zzz")
    });
    assert_eq!(
        kid.s.app().selected_stem(),
        Some("Aaa"),
        "the kid came back to a shelf other than Favorites"
    );
}

#[test]
fn a_kid_returns_to_their_shelf_after_an_invite() {
    returns_to_favorites(slot_store::KidMode::OneShelf);
}

#[test]
fn a_kid_on_every_shelf_returns_to_their_shelf_after_an_invite() {
    returns_to_favorites(slot_store::KidMode::Shelves);
}

#[test]
fn a_failed_join_goes_back_to_the_kids_game() {
    let mut kid = emerald_on(Core::Gpsp, Far::NobodyCame);
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    let accepted = kid.resume(Platform::Gba, Core::Gpsp, "Emerald");
    assert!(accepted.is_some(), "A did not write the resume state");
    kid.until("the join never failed", |k| {
        matches!(
            k.s.app().game_menu(),
            Some(GameMenu::Failed {
                fail: LinkFail::NobodyCame,
                ..
            })
        )
    });
    assert_eq!(kid.s.app().core(), Core::Mgba);
    kid.press(Btn::A);
    kid.ready_on(Core::Gpsp);
    assert!(!kid.s.app().game_menu_open());
    assert_eq!(kid.s.app().link_core(), None);
    assert_eq!(
        kid.resume(Platform::Gba, Core::Gpsp, "Emerald"),
        accepted,
        "the link touched the kid's own resume state"
    );
}

#[test]
fn gambatte_switches_to_mgba_for_the_link() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zzz"]);
    slot_store::write_selected_core(d.path(), Platform::Gb, "Tetris", Core::Gambatte)
        .expect("write core");
    let mut kid = kid_session(
        d,
        kid_slot(slot_store::KidMode::Shelves, Some(("Tetris", Platform::Gb))),
        (Platform::Gb, Core::Gambatte, "Tetris"),
        Far::Waits,
    );
    kid.ready_on(Core::Gambatte);
    let rom = std::fs::read_dir(kid.d.path().join("Games").join(Platform::Gb.dir_name()))
        .expect("list GB")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.file_stem().is_some_and(|s| s == "Tetris"))
        .expect("the Tetris rom");
    let crc = slot::art::rom::rom_crc(&rom).expect("hash the rom");
    kid.card_for(move |_| slot::ble::Heard {
        advert: slot::invite::Advert {
            session: 7,
            game_crc: crc,
            core: Core::Mgba,
            bios_crc: 0,
            kind: slot::link_kind::LinkKind::Cable,
            freq_mhz: AP.freq_mhz,
            bssid: AP.bssid,
            platform: Platform::Gb,
        },
        nickname: Some("Dad".to_string()),
        rssi: -60,
        at: slot::ble::clock(),
    });
    kid.press(Btn::A);
    kid.ready_on(Core::Mgba);
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert_eq!(kid.joined(), vec![RadioRole::Join(AP)]);
}

fn counter(kid: &Kid) -> u64 {
    let state = kid
        .s
        .emu()
        .expect("a core is running")
        .request_state()
        .recv_timeout(BAIL)
        .expect("the core gave up no state");
    u64::from_le_bytes(state.try_into().expect("mock state is 8 bytes"))
}

fn eject_and_land(kid: &mut Kid) {
    kid.s.app_mut().apply(Action::QuickMenu);
    kid.until("the cart never came out", |k| {
        matches!(k.s.app().phase(), Phase::Shelf)
    });
}

#[test]
fn a_switched_core_starts_clean_and_keeps_the_gpsp_resume() {
    const OLD: u64 = 1_000_000;
    let port = common::free_port();
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    slot::persist::flush(
        d.path(),
        Platform::Gba,
        Core::Gpsp,
        "Emerald",
        Some(&OLD.to_le_bytes()),
        None,
    )
    .expect("plant an old gpSP resume");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", Core::Mgba)
        .expect("write core");
    let mut kid = kid_session(
        d,
        kid_slot(
            slot_store::KidMode::Shelves,
            Some(("Emerald", Platform::Gba)),
        ),
        (Platform::Gba, Core::Mgba, "Emerald"),
        Far::Hosts(port),
    );
    kid.ready_on(Core::Mgba);
    let gpsp_before = kid.resume(Platform::Gba, Core::Gpsp, "Emerald");
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    let mgba_before = kid.resume(Platform::Gba, Core::Mgba, "Emerald");
    assert!(mgba_before.is_some(), "A did not write the kid's resume");
    kid.ready_on(Core::Gpsp);
    assert!(
        counter(&kid) < OLD,
        "the link core opened its old resume state"
    );
    kid.until("the link never came up", |k| k.s.app().link_active());
    let _far = far.join().expect("host thread");
    kid.until("LINKED never handed the game back", |k| {
        !k.s.app().game_menu_open()
    });
    for _ in 0..30 {
        kid.step(&[]);
    }
    eject_and_land(&mut kid);
    assert_eq!(
        kid.resume(Platform::Gba, Core::Gpsp, "Emerald"),
        gpsp_before,
        "the link core wrote over the gpSP resume"
    );
    assert_eq!(
        kid.resume(Platform::Gba, Core::Mgba, "Emerald"),
        None,
        "the pre-link resume is still there to undo a trade"
    );
}

#[test]
fn a_lid_shut_before_the_game_starts_drops_the_invite() {
    let d = common::tmp_root_with_carts(&["Emerald", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    for stem in ["Emerald", "Zzz"] {
        slot_store::write_selected_core(d.path(), Platform::Gba, stem, Core::Mgba)
            .expect("write core");
    }
    let mut kid = kid_session(
        d,
        kid_slot(slot_store::KidMode::Shelves, None),
        (Platform::Gba, Core::Mgba, "Zzz"),
        Far::Waits,
    );
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    assert!(
        matches!(kid.s.app().phase(), Phase::Inserting { .. }),
        "A did not start the game: {:?}",
        kid.s.app().phase()
    );
    kid.s.app_mut().apply(Action::LidClose);
    kid.step(&[]);
    kid.s.app_mut().apply(Action::LidOpen);
    kid.until("the lid never opened onto the shelf", |k| {
        matches!(k.s.app().phase(), Phase::Shelf)
    });
    assert_eq!(kid.s.app().link_core(), None);
    assert_eq!(kid.s.app().join_target(), None);
    for _ in 0..4 {
        if kid.s.app().selected_stem() == Some("Zzz") {
            break;
        }
        kid.s.app_mut().apply(Action::GbaDown(Btn::Right));
        kid.s.app_mut().apply(Action::GbaUp(Btn::Right));
        idle_kid(&mut kid, 300);
    }
    assert_eq!(kid.s.app().selected_stem(), Some("Zzz"));
    kid.s.app_mut().apply(Action::Insert);
    kid.ready_on(Core::Mgba);
    idle_kid(&mut kid, 300);
    assert!(kid.joined().is_empty(), "a dropped invite joined later");
    assert_eq!(kid.s.app().link_core(), None);
    assert!(!kid.s.app().game_menu_open());
}

fn idle_kid(kid: &mut Kid, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < until {
        kid.step(&[]);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn kid_on_real_gpsp(own: Core, far: Far) -> Option<Kid> {
    let core = common::vendored_core()?;
    let d = common::tmp_root_with_real_carts(&["Emerald", "Zzz"]);
    common::write_real_cart_as(&d, "Emerald", "POKEMON EMER", "BPEE");
    slot_store::write_selected_core(d.path(), Platform::Gba, "Emerald", own).expect("write core");
    std::fs::copy(
        &core,
        d.path()
            .join("System")
            .join(slot::core::dylib_name(Core::Gpsp)),
    )
    .expect("plant a core under gpSP's name");
    let mut kid = kid_session(
        d,
        kid_slot(
            slot_store::KidMode::Shelves,
            Some(("Emerald", Platform::Gba)),
        ),
        (Platform::Gba, own, "Emerald"),
        far,
    );
    kid.ready_on(own);
    Some(kid)
}

fn take_the_rom_away(kid: &Kid) {
    std::fs::remove_file(kid.d.path().join("Games/GBA").join("Emerald.gba"))
        .expect("take the rom away");
}

#[test]
fn a_game_the_link_core_cannot_load_shows_the_join_failure() {
    let _g = common::core_lock();
    let Some(mut kid) = kid_on_real_gpsp(Core::Mgba, Far::Waits) else {
        eprintln!("no host-openable dylib on this machine, skipping");
        return;
    };
    kid.card_for(emerald_from(Core::Gpsp));
    take_the_rom_away(&kid);
    kid.press(Btn::A);
    kid.until("no join failure was shown", |k| {
        matches!(
            k.s.app().game_menu(),
            Some(GameMenu::Failed {
                role: LinkRow::Join,
                ..
            })
        )
    });
    assert!(kid.joined().is_empty(), "a join started on a failed reload");
    kid.press(Btn::A);
    kid.ready_on(Core::Mgba);
    assert!(!kid.s.app().game_menu_open());
    assert_eq!(kid.s.app().link_core(), None);
}

#[test]
fn a_kid_whose_own_core_will_not_reload_goes_back_to_the_shelf() {
    let _g = common::core_lock();
    let Some(mut kid) = kid_on_real_gpsp(Core::Gpsp, Far::NobodyCame) else {
        eprintln!("no host-openable dylib on this machine, skipping");
        return;
    };
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    kid.until("the join never failed", |k| {
        matches!(k.s.app().game_menu(), Some(GameMenu::Failed { .. }))
    });
    take_the_rom_away(&kid);
    kid.press(Btn::A);
    kid.until("the cart never came back out", |k| {
        matches!(k.s.app().phase(), Phase::Ejecting { .. } | Phase::Shelf)
    });
    kid.until("the refused cart never reached the shelf", |k| {
        matches!(k.s.app().phase(), Phase::Shelf)
    });
}

fn frame_now(kid: &Kid) -> Option<u64> {
    let emu = kid.s.emu()?;
    if emu.state() != slot::emu::CoreState::Ready {
        return None;
    }
    let state = emu.request_state().recv_timeout(BAIL).ok()?;
    Some(u64::from_le_bytes(state.try_into().ok()?))
}

fn frames_of(resume: &Option<Vec<u8>>) -> u64 {
    let bytes = resume.clone().expect("a resume state");
    u64::from_le_bytes(bytes.try_into().expect("mock state is 8 bytes"))
}

fn jump_to(kid: &mut Kid, frame: u64) {
    kid.s
        .emu()
        .expect("a core is running")
        .request_load(frame.to_le_bytes().to_vec());
    kid.until("the core never took the planted state", |k| {
        frame_now(k).is_some_and(|f| f >= frame)
    });
}

fn hosts_its_game(link: &mut TcpLink, frame: u64) {
    for packet in slot::cable::state_packets(&frame.to_le_bytes()) {
        slot_retro::LinkChannel::send(link, slot_retro::NETPACKET_RELIABLE, &packet);
    }
}

fn back_in_play(kid: &mut Kid, core: Core, below: u64) {
    kid.until("the kid never came back to their own game", |k| {
        !k.s.app().link_active()
            && !k.s.app().game_menu_open()
            && k.s.app().core() == core
            && matches!(k.s.app().phase(), Phase::Playing { .. })
            && frame_now(k).is_some_and(|f| f < below)
    });
}

#[test]
fn a_gpsp_switch_comes_back_from_the_in_game_save() {
    const OLD: u64 = 1_000_000;
    let port = common::free_port();
    let mut kid = emerald_on(Core::Mgba, Far::Hosts(port));
    jump_to(&mut kid, OLD);
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Gpsp));
    kid.press(Btn::A);
    assert!(
        frames_of(&kid.resume(Platform::Gba, Core::Mgba, "Emerald")) >= OLD,
        "A did not write the kid's resume"
    );
    kid.ready_on(Core::Gpsp);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let mut far = far.join().expect("host thread");
    assert_eq!(
        kid.resume(Platform::Gba, Core::Mgba, "Emerald"),
        None,
        "the pre-link resume is still there to load"
    );
    let traded: Vec<u8> = (0..8 * 1024).map(|i| (i % 251) as u8).collect();
    slot::persist::write_sav(kid.d.path(), Platform::Gba, "Emerald", &traded)
        .expect("the trade lands in the save");
    kid.s.emu().expect("gpSP is running").refuse_save_ram();
    far.send_end();
    back_in_play(&mut kid, Core::Mgba, OLD);
    assert_eq!(kid.s.app().link_core(), None);
    eject_and_land(&mut kid);
    assert_eq!(
        slot::persist::read_sav(kid.d.path(), Platform::Gba, "Emerald").expect("read the save"),
        Some(traded),
        "the kid's game did not start from the traded save"
    );
    kid.s.app_mut().apply(Action::Insert);
    kid.ready_on(Core::Mgba);
    assert!(
        frame_now(&kid).is_some_and(|f| f < OLD),
        "the next launch resumed into the pre-link game"
    );
}

fn mgba_link_comes_back(own: Core) {
    const HOST: u64 = 5_000_000;
    let port = common::free_port();
    let mut kid = emerald_on(own, Far::Hosts(port));
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    let before = kid.resume(Platform::Gba, own, "Emerald");
    let pre_link = frames_of(&before);
    kid.ready_on(Core::Mgba);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let mut far = far.join().expect("host thread");
    hosts_its_game(&mut far, HOST);
    kid.until("the host's game never arrived", |k| {
        frame_now(k).is_some_and(|f| f >= HOST)
    });
    far.send_end();
    back_in_play(&mut kid, own, HOST);
    assert!(
        frame_now(&kid).is_some_and(|f| f >= pre_link),
        "the kid's game did not come back from the pre-link resume"
    );
    assert_eq!(
        kid.resume(Platform::Gba, own, "Emerald"),
        before,
        "the link wrote over the kid's resume"
    );
    assert_eq!(kid.s.app().link_core(), None);
}

#[test]
fn an_mgba_link_comes_back_to_the_kids_resume() {
    mgba_link_comes_back(Core::Gpsp);
    mgba_link_comes_back(Core::Mgba);
}

#[test]
fn a_same_core_link_returns_to_the_kids_game() {
    const HOST: u64 = 5_000_000;
    let port = common::free_port();
    let mut kid = emerald_on(Core::Mgba, Far::Hosts(port));
    let far = host_far(port);
    kid.card_for(emerald_from(Core::Mgba));
    kid.press(Btn::A);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let mut far = far.join().expect("host thread");
    hosts_its_game(&mut far, HOST);
    kid.until("the host's game never arrived", |k| {
        frame_now(k).is_some_and(|f| f >= HOST)
    });
    far.send_end();
    back_in_play(&mut kid, Core::Mgba, HOST);
    assert!(
        matches!(kid.s.app().phase(), Phase::Playing { cart } if cart == "Emerald"),
        "the kid had to eject to get their game back"
    );
    kid.s.app_mut().flush_resume();
    assert!(
        frames_of(&kid.resume(Platform::Gba, Core::Mgba, "Emerald")) < HOST,
        "saves are still off after the link"
    );
}

fn kid_on_a_long_shelf() -> Kid {
    let d = common::tmp_root_with_carts(&["Aaa", "Bbb", "Ccc", "Emerald", "Fff", "Ggg", "Zzz"]);
    common::write_hack_header(&d, "Emerald", "POKEMON EMER", "BPEE");
    kid_session(
        d,
        kid_slot(slot_store::KidMode::Shelves, None),
        (Platform::Gba, Core::Gpsp, "Emerald"),
        Far::Waits,
    )
}

fn stem_of(kid: &Kid) -> String {
    kid.s.app().selected_stem().expect("a cart").to_string()
}

fn behind_the_card(kid: &mut Kid, frames: usize, events: &[slot_input::RawEvent]) {
    let at = stem_of(kid);
    kid.ble.hear(emerald_from(Core::Gpsp)(&kid.d));
    kid.step(events);
    for frame in 0..frames {
        assert!(kid.s.app().invite_card().is_some(), "the card closed");
        assert_eq!(
            stem_of(kid),
            at,
            "the shelf moved behind the card at frame {frame}"
        );
        kid.ble.hear(emerald_from(Core::Gpsp)(&kid.d));
        kid.step(&[]);
    }
    assert_eq!(stem_of(kid), at, "the shelf moved behind the card");
}

fn shelf_stays(kid: &mut Kid, frames: usize, what: &str) {
    let at = stem_of(kid);
    for frame in 0..frames {
        kid.step(&[]);
        assert_eq!(stem_of(kid), at, "{what} at frame {frame}");
    }
}

fn one_press_moves_one_cart(kid: &mut Kid) {
    let at = stem_of(kid);
    kid.press(Btn::Right);
    assert_ne!(stem_of(kid), at, "a new press did not move the shelf");
    shelf_stays(kid, 60, "the shelf ran on after a new press");
}

#[test]
fn a_held_direction_does_not_scroll_the_shelf_behind_the_card() {
    use slot_input::RawEvent;
    let mut kid = kid_on_a_long_shelf();
    kid.step(&[RawEvent::Down(Btn::Left)]);
    kid.card_for(emerald_from(Core::Gpsp));
    behind_the_card(&mut kid, 90, &[]);
    behind_the_card(&mut kid, 90, &[RawEvent::Up(Btn::Left)]);
    kid.press(Btn::B);
    assert!(kid.s.app().invite_card().is_none(), "B left the card up");
    shelf_stays(&mut kid, 90, "the shelf scrolled on its own after Not now");
    one_press_moves_one_cart(&mut kid);
}

#[test]
fn a_direction_pressed_behind_the_card_does_not_move_the_shelf() {
    use slot_input::RawEvent;
    let mut kid = kid_on_a_long_shelf();
    kid.card_for(emerald_from(Core::Gpsp));
    behind_the_card(&mut kid, 30, &[RawEvent::Down(Btn::Left)]);
    behind_the_card(&mut kid, 30, &[RawEvent::Up(Btn::Left)]);
    behind_the_card(&mut kid, 30, &[RawEvent::Down(Btn::Right)]);
    behind_the_card(&mut kid, 30, &[RawEvent::Up(Btn::Right)]);
    kid.press(Btn::B);
    assert!(kid.s.app().invite_card().is_none(), "B left the card up");
    shelf_stays(&mut kid, 90, "the shelf scrolled on its own after Not now");
    one_press_moves_one_cart(&mut kid);
}

#[test]
fn a_direction_held_through_not_now_stops_on_release() {
    use slot_input::RawEvent;
    let mut kid = kid_on_a_long_shelf();
    kid.step(&[RawEvent::Down(Btn::Left)]);
    kid.card_for(emerald_from(Core::Gpsp));
    behind_the_card(&mut kid, 60, &[]);
    kid.press(Btn::B);
    assert!(kid.s.app().invite_card().is_none(), "B left the card up");
    shelf_stays(
        &mut kid,
        90,
        "the shelf scrolled under a hold from before the card",
    );
    kid.step(&[RawEvent::Up(Btn::Left)]);
    shelf_stays(&mut kid, 90, "the shelf scrolled on after the release");
    one_press_moves_one_cart(&mut kid);
}

#[test]
fn a_held_direction_through_a_starts_the_game_without_scrolling() {
    use slot_input::RawEvent;
    let mut kid = kid_on_a_long_shelf();
    kid.step(&[RawEvent::Down(Btn::Left)]);
    kid.card_for(emerald_from(Core::Gpsp));
    behind_the_card(&mut kid, 60, &[]);
    kid.press(Btn::A);
    assert!(
        matches!(kid.s.app().phase(), Phase::Inserting { cart, .. } if cart == "Emerald"),
        "A did not start the game: {:?}",
        kid.s.app().phase()
    );
    kid.step(&[RawEvent::Up(Btn::Left)]);
    for frame in 0..60 {
        kid.step(&[]);
        assert_eq!(
            kid.s.app().selected_stem(),
            Some("Emerald"),
            "the shelf moved off the invited game at frame {frame}"
        );
    }
}

fn named(d: &TempDir, session: u16, nickname: Option<&str>) -> slot::ble::Heard {
    let mut heard = common::heard_host(d, "Emerald", session, AP, -60);
    heard.nickname = nickname.map(str::to_string);
    heard
}

#[test]
fn a_nameless_advert_waits_briefly_for_the_hosts_name() {
    let (mut app, d, ble) = kid_on_shelf(true);
    ble.hear(named(&d, 7, None));
    idle(&mut app, 250);
    assert!(
        app.invite_card().is_none(),
        "the card opened before the host's name could arrive"
    );
    ble.hear(named(&d, 7, Some("kwallbmoc")));
    assert!(card_up(&mut app), "the card never opened");
    assert_eq!(app.invite_card().unwrap().nickname(), "kwallbmoc");
}

#[test]
fn a_name_that_arrives_after_the_card_opens_replaces_the_fallback() {
    let (mut app, d, ble) = kid_on_shelf(true);
    let heard_at = Instant::now();
    ble.hear(named(&d, 7, None));
    assert!(card_up(&mut app), "a nameless host never opened the card");
    assert!(
        heard_at.elapsed() >= Duration::from_millis(slot::invite::NAME_WAIT_MS),
        "the card did not wait for the name"
    );
    assert_eq!(
        app.invite_card().unwrap().nickname(),
        slot::invite_card::NO_NAME
    );
    ble.hear(named(&d, 7, Some("kwallbmoc")));
    assert!(
        pump(&mut app, |a| a
            .invite_card()
            .is_some_and(|c| c.nickname() == "kwallbmoc")),
        "the host's name never reached the open card: {:?}",
        app.invite_card().map(|c| c.nickname().to_string())
    );
    ble.hear(named(&d, 7, None));
    idle(&mut app, 100);
    assert_eq!(app.invite_card().unwrap().nickname(), "kwallbmoc");
}

#[test]
fn a_held_when_the_card_opens_does_not_start_the_cart_behind_it() {
    let (mut app, d, ble) = kid_on_shelf(true);
    app.apply(Action::GbaDown(Btn::A));
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    let deadline = Instant::now() + BAIL;
    while app.invite_card().is_none() {
        assert!(Instant::now() < deadline, "the card never opened");
        app.update(0.0);
        std::thread::sleep(Duration::from_millis(1));
    }
    for _ in 0..60 {
        app.update(1.0 / 60.0);
    }
    assert!(
        matches!(app.phase(), Phase::Shelf),
        "the held A started a cart behind the card: {:?}",
        app.phase()
    );
    assert!(app.invite_card().is_some(), "the card closed");
}

struct EndOfLink {
    toast: u64,
    unplugged: u64,
    reload: Option<u64>,
}

fn end_of_link(kid: &mut Kid, own: Core) -> EndOfLink {
    let face = slot_ui::TexId::from_raw(900 + slot_ui::Toast::PeerEnded.index());
    let port = fake_link_sprites().port.tex;
    let faces = slot_ui::Toast::all()
        .into_iter()
        .map(|t| slot_ui::TexId::from_raw(900 + t.index()))
        .collect();
    kid.s.app_mut().set_toast_faces(faces);
    let launches = kid.s.launches();
    let mut seen = EndOfLink {
        toast: 0,
        unplugged: 0,
        reload: None,
    };
    let mut since = None;
    let (mut toast_on, mut unplug_on) = (true, true);
    let mut back = false;
    let mut after = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    while after < 200 {
        assert!(
            Instant::now() < deadline,
            "the kid never came back after the link"
        );
        kid.step(&[]);
        if let Some(GameMenu::Unplug { since: at, .. }) = kid.s.app().game_menu() {
            since.get_or_insert(at);
        }
        let Some(start) = since else {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        };
        let age = kid.s.app().now() - start;
        let mut out = Vec::new();
        kid.s.app().draw(&mut out);
        let has = |want: slot_ui::TexId| {
            out.iter().any(|d| {
                matches!(*d, slot_ui::Draw::Tex { tex, alpha, .. } if tex == want && alpha > 0.0)
            })
        };
        let unplug = matches!(kid.s.app().game_menu(), Some(GameMenu::Unplug { .. })) && has(port);
        toast_on = toast_on && has(face);
        unplug_on = unplug_on && unplug;
        if toast_on {
            seen.toast = age;
        }
        if unplug_on {
            seen.unplugged = age;
        }
        let reloading = kid.s.emu().is_none() || kid.s.launches() != launches;
        if reloading && seen.reload.is_none() {
            seen.reload = Some(age);
        }
        back = back
            || (!kid.s.app().link_active()
                && !kid.s.app().game_menu_open()
                && kid.s.app().core() == own
                && frame_now(kid).is_some());
        if back {
            after += 1;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    seen
}

fn the_joiner_shows_the_friend_unplugged(own: Core, host: Core) {
    let port = common::free_port();
    let mut kid = emerald_on(own, Far::Hosts(port));
    with_fake_art(&mut kid);
    let far = host_far(port);
    kid.card_for(emerald_from(host));
    kid.press(Btn::A);
    kid.ready_on(host);
    kid.until("the link never came up", |k| k.s.app().link_active());
    let mut far = far.join().expect("host thread");
    if host == Core::Mgba {
        hosts_its_game(&mut far, 5_000_000);
        kid.until("the host's game never arrived", |k| {
            frame_now(k).is_some_and(|f| f >= 5_000_000)
        });
    }
    far.send_end();
    let seen = end_of_link(&mut kid, own);
    let hold = slot::app::UNPLUG_HOLD_MS;
    let frame = 17;
    assert!(
        seen.unplugged + frame >= hold,
        "{own:?} joining {host:?}: the unplug screen and its port were drawn for {} ms of {hold}",
        seen.unplugged
    );
    assert!(
        seen.toast + frame >= hold,
        "{own:?} joining {host:?}: \"Your friend unplugged\" was drawn for {} ms of {hold}",
        seen.toast
    );
    let reload = seen
        .reload
        .unwrap_or_else(|| panic!("{own:?} joining {host:?}: the kid's own game never reloaded"));
    assert!(
        reload >= hold,
        "{own:?} joining {host:?}: the reload back began {reload} ms in, inside the {hold} ms hold"
    );
}

#[test]
fn a_joiner_shows_the_friend_unplugged_on_the_way_back() {
    the_joiner_shows_the_friend_unplugged(Core::Mgba, Core::Mgba);
    the_joiner_shows_the_friend_unplugged(Core::Gpsp, Core::Mgba);
    the_joiner_shows_the_friend_unplugged(Core::Mgba, Core::Gpsp);
}

fn fake_link_sprites() -> slot::link_screen::LinkSprites {
    let s = |n: usize| slot::link_screen::Sprite {
        tex: slot_ui::TexId::from_raw(n),
        w: 10,
        h: 10,
    };
    slot::link_screen::LinkSprites {
        port: s(1),
        plug_host: s(2),
        plug_join: s(3),
        adapter: s(4),
        arcs_right: [s(7), s(8), s(9)],
        arcs_left: [s(10), s(11), s(12)],
        clicks: s(13),
        arrow_left: s(14),
        arrow_right: s(15),
    }
}

fn with_fake_art(kid: &mut Kid) {
    let s = |n: usize| slot::link_screen::Sprite {
        tex: slot_ui::TexId::from_raw(n),
        w: 10,
        h: 10,
    };
    let app = kid.s.app_mut();
    app.set_link_sprites(fake_link_sprites());
    app.set_invite_faces(slot::invite_card::InviteFaces {
        card: s(20),
        line_one: s(21),
        line_two: s(22),
        not_now: s(23),
        join: s(24),
        art: None,
    });
}

fn drew(app: &App, n: usize) -> bool {
    let mut out = Vec::new();
    app.draw(&mut out);
    out.iter().any(|d| {
        matches!(*d, slot_ui::Draw::Tex { tex, .. } | slot_ui::Draw::Turned { tex, .. }
            if tex == slot_ui::TexId::from_raw(n))
    })
}

#[test]
fn a_kid_joining_a_wireless_host_sees_the_adapter() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    with_fake_art(&mut kid);
    kid.card_for(|d| {
        let mut heard = common::heard_host(d, "Emerald", 7, AP, -60);
        heard.advert.kind = slot::link_kind::LinkKind::Wireless;
        heard
    });
    assert!(drew(kid.s.app(), 4), "the invite card drew no adapter");
    assert!(
        !drew(kid.s.app(), 3),
        "the invite card drew a plug for a host on the adapter"
    );
    kid.press(Btn::A);
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert!(
        kid.s.app().game_menu_open(),
        "the link screen is not up to draw"
    );
    assert!(drew(kid.s.app(), 4), "the joiner drew no adapter");
    assert!(
        !drew(kid.s.app(), 3),
        "the joiner drew a plug for a host on the adapter"
    );
}

#[test]
fn a_kid_joining_a_cable_host_sees_the_plug() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    with_fake_art(&mut kid);
    kid.card_for(emerald_from(Core::Gpsp));
    assert!(drew(kid.s.app(), 3), "the invite card drew no plug");
    assert!(!drew(kid.s.app(), 4), "the invite card drew the adapter");
    kid.press(Btn::A);
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert!(drew(kid.s.app(), 3), "the joiner drew no plug");
    assert!(!drew(kid.s.app(), 4), "the joiner drew the adapter");
}

#[test]
fn the_card_joins_the_hotspot_early_but_not_the_game() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    kid.card_for(emerald_from(Core::Gpsp));
    assert_eq!(kid.s.app().joining_early(), Some(AP));
    for _ in 0..30 {
        kid.ble.hear(emerald_from(Core::Gpsp)(&kid.d));
        kid.step(&[]);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        kid.joined().is_empty(),
        "the game connection opened before Join: {:?}",
        kid.joined()
    );
    kid.press(Btn::A);
    kid.until("the join never started", |k| !k.joined().is_empty());
    assert_eq!(kid.joined(), vec![RadioRole::Join(AP)]);
    assert_eq!(kid.s.app().joining_early(), None);
}

#[test]
fn not_now_leaves_the_hotspot() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    kid.card_for(emerald_from(Core::Gpsp));
    assert_eq!(kid.s.app().joining_early(), Some(AP));
    kid.press(Btn::B);
    kid.step(&[]);
    assert_eq!(kid.s.app().joining_early(), None);
    assert!(kid.joined().is_empty());
}

#[test]
fn a_vanished_host_leaves_the_hotspot() {
    let mut kid = emerald_on(Core::Gpsp, Far::Waits);
    kid.card_for(emerald_from(Core::Gpsp));
    assert_eq!(kid.s.app().joining_early(), Some(AP));
    kid.until("the card never closed", |k| {
        k.s.app().invite_card().is_none()
    });
    assert_eq!(kid.s.app().joining_early(), None);
    assert!(kid.joined().is_empty());
}

#[test]
fn home_wifi_waits_while_the_card_joins_early() {
    let (mut app, d, ble) = kid_on_shelf(true);
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut fake = slot::wifi::Fake::demo();
    slot::wifi::Backend::add(&mut fake, "Home", "password", false);
    let log = fake.log.clone();
    app.set_wifi(slot::wifi::Wifi::inline(Box::new(fake)));
    let starts = || log.lock().unwrap().iter().filter(|l| *l == "start").count();
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(card_up(&mut app), "the card never opened");
    assert_eq!(app.joining_early(), Some(AP));
    for _ in 0..4 {
        ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
        app.update(1.0);
    }
    assert!(app.invite_card().is_some(), "the card closed early");
    assert_eq!(starts(), 0, "home Wi-Fi took the radio from the early join");
    app.apply(Action::GbaDown(Btn::B));
    app.apply(Action::GbaUp(Btn::B));
    for _ in 0..6 {
        app.update(1.0);
    }
    assert_eq!(app.joining_early(), None);
    assert_eq!(starts(), 1, "home Wi-Fi never came back after Not now");
}

#[test]
fn the_early_join_waits_for_home_wifi_to_let_go() {
    let (mut app, d, ble) = kid_on_shelf(true);
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut fake = slot::wifi::Fake::demo();
    slot::wifi::Backend::add(&mut fake, "Home", "password", false);
    let log = fake.log.clone();
    app.set_wifi(slot::wifi::Wifi::inline(Box::new(fake)));
    for _ in 0..4 {
        app.update(1.0);
    }
    assert!(app.wifi().held, "home Wi-Fi never came up first");
    let spawned: Arc<Mutex<Vec<(RadioRole, bool, usize)>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = spawned.clone();
    let stops = log.clone();
    app.set_link_spawn(Box::new(move |role, _port, held| {
        let stopped = stops
            .lock()
            .unwrap()
            .iter()
            .filter(|l| *l == "stop")
            .count();
        seen.lock().unwrap().push((role, held, stopped));
        LinkStarter::spawn_held_with(
            Box::new(|_, _| Ok(None)),
            Box::new(|| {}),
            role,
            0,
            Box::new(|_, cancel| until_cancelled(cancel)),
            held,
        )
    }));
    ble.hear(common::heard_host(&d, "Emerald", 7, AP, -60));
    assert!(card_up(&mut app), "the card never opened");
    assert!(
        pump(&mut app, |_| !spawned.lock().unwrap().is_empty()),
        "the early join never started"
    );
    assert_eq!(
        *spawned.lock().unwrap(),
        vec![(RadioRole::Join(AP), true, 1)],
        "the early join started before home Wi-Fi stopped"
    );
}
