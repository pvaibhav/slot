mod common;

use common::{app_booting_at, tmp_root_with_carts, CLOCK_IS_SET};
use slot::app::{App, KeyboardFor, Phase};
use slot::romm::{PairFail, Protocol, RommConf, RommEvent, RommJob, RommJobs};
use std::sync::{Arc, Mutex};
use slot_input::{Action, Btn, Millis};
use slot_store::{write_slot_state, Platform, SlotState};
use slot_ui::QuickRow;
use tempfile::TempDir;

fn booted() -> (TempDir, App) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    common::connect(&mut a);
    (d, a)
}

fn press(a: &mut App, btn: Btn) {
    a.apply(Action::GbaDown(btn));
    a.apply(Action::GbaUp(btn));
}

fn open_at(a: &mut App, row: QuickRow) {
    a.apply(Action::QuickMenu);
    let mut path = vec![row];
    while let Some(parent) = path.last().and_then(|r| r.parent()) {
        path.push(parent);
    }
    for (depth, step) in path.iter().rev().enumerate() {
        if depth > 0 {
            press(a, Btn::A);
        }
        for _ in 0..step.position(&a.hidden_rows()) {
            press(a, Btn::Down);
        }
    }
    assert_eq!(a.quick_menu(), Some(row), "the bar never reached {row:?}");
}

fn pair(a: &mut App) {
    a.romm.server = "romm.local".into();
    a.romm.device_id = Some("d1".into());
    a.romm.token = Some("t1".into());
}

fn shown(a: &App, page: &[QuickRow]) -> Vec<QuickRow> {
    page[0].shown(&a.hidden_rows())
}

#[test]
fn romm_row_reads_setup_until_paired() {
    let (_d, a) = booted();
    assert_eq!(a.romm_texts()[0], "SETUP");
}

#[test]
fn romm_row_reads_offline_when_paired_and_not_yet_heard_from() {
    let (_d, mut a) = booted();
    pair(&mut a);
    assert_eq!(a.romm_texts()[0], "OFFLINE");
    assert_eq!(a.romm_texts()[1], "romm.local");
}

#[test]
fn tools_lists_romm_after_gamebit() {
    assert_eq!(
        QuickRow::NETWORK.map(QuickRow::label),
        [
            "Gamebit",
            "RomM",
            "Fetch Missing Art",
            #[cfg(feature = "nds")]
            "Fetch Box Art",
            "Check for Updates"
        ]
    );
    let (_d, mut a) = booted();
    open_at(&mut a, QuickRow::Romm);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::RommProtocol));
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::Romm));
}

#[test]
fn setup_rows_in_order() {
    let (_d, a) = booted();
    assert_eq!(
        shown(&a, &QuickRow::ROMM)
            .into_iter()
            .map(QuickRow::label)
            .collect::<Vec<_>>(),
        ["Protocol", "Server", "Port", "Accept Self-Signed", "Pair"]
    );
}

#[test]
fn http_hides_accept_self_signed() {
    let (_d, mut a) = booted();
    open_at(&mut a, QuickRow::RommProtocol);
    press(&mut a, Btn::A);
    assert_eq!(a.romm.protocol, Protocol::Http);
    assert!(!shown(&a, &QuickRow::ROMM).contains(&QuickRow::RommSelfSigned));
    press(&mut a, Btn::Right);
    assert_eq!(a.romm.protocol, Protocol::Https);
    assert!(shown(&a, &QuickRow::ROMM).contains(&QuickRow::RommSelfSigned));
}

#[test]
fn server_keyboard_saves_to_conf() {
    let (d, mut a) = booted();
    open_at(&mut a, QuickRow::RommServer);
    press(&mut a, Btn::A);
    assert_eq!(a.keyboard_for(), Some(&KeyboardFor::RommServer));
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert_eq!(a.romm.server, "q");
    let saved = RommConf::load(d.path());
    assert_eq!(saved.server, "q");
    assert!(!saved.client_id.is_empty());
}

#[test]
fn port_rejects_above_65535() {
    let (d, mut a) = booted();
    open_at(&mut a, QuickRow::RommPort);
    press(&mut a, Btn::A);
    assert_eq!(a.keyboard_for(), Some(&KeyboardFor::RommPort));
    for _ in 0..5 {
        press(&mut a, Btn::A);
    }
    assert_eq!(a.keyboard().map(|k| k.text()), Some("77777".to_string()));
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_some());
    assert_eq!(
        a.update_text(),
        Some(("Invalid Port".to_string(), String::new()))
    );
    assert_eq!(a.romm.port, None);
    assert_eq!(RommConf::load(d.path()).port, None);
}

#[test]
fn blank_port_is_none() {
    let (d, mut a) = booted();
    a.romm.port = Some(80);
    open_at(&mut a, QuickRow::RommPort);
    press(&mut a, Btn::A);
    assert_eq!(a.keyboard().map(|k| k.text()), Some("80".to_string()));
    press(&mut a, Btn::B);
    press(&mut a, Btn::B);
    press(&mut a, Btn::Start);
    assert!(a.keyboard().is_none());
    assert_eq!(a.romm.port, None);
    assert_eq!(RommConf::load(d.path()).port, None);
}

#[test]
fn port_keeps_a_valid_number() {
    let (d, mut a) = booted();
    open_at(&mut a, QuickRow::RommPort);
    press(&mut a, Btn::A);
    press(&mut a, Btn::A);
    press(&mut a, Btn::Start);
    assert_eq!(a.romm.port, Some(7));
    assert_eq!(RommConf::load(d.path()).port, Some(7));
}

#[test]
fn pair_refuses_an_empty_server() {
    let (_d, mut a) = booted();
    open_at(&mut a, QuickRow::RommPair);
    press(&mut a, Btn::A);
    let (title, _) = a.update_text().expect("a message");
    assert_eq!(title, "Enter a server first");
}

#[test]
fn pair_saves_the_conf_with_a_client_id() {
    let (d, mut a) = booted();
    a.romm.server = "romm.local".into();
    open_at(&mut a, QuickRow::RommPair);
    press(&mut a, Btn::A);
    assert!(!RommConf::load(d.path()).client_id.is_empty());
    assert!(!a.romm.client_id.is_empty());
}

#[test]
fn paired_page_shows_status_server_disconnect() {
    let (_d, mut a) = booted();
    pair(&mut a);
    open_at(&mut a, QuickRow::Romm);
    press(&mut a, Btn::A);
    assert_eq!(a.quick_menu(), Some(QuickRow::RommStatus));
    assert_eq!(shown(&a, &QuickRow::ROMM), QuickRow::ROMM_PAIRED.to_vec());
}

#[derive(Clone, Default)]
struct Script {
    asked: Arc<Mutex<Vec<RommJob>>>,
    events: Arc<Mutex<Vec<RommEvent>>>,
}

impl RommJobs for Script {
    fn ask(&mut self, job: RommJob) {
        self.asked.lock().unwrap().push(job);
    }

    fn take(&mut self) -> Option<RommEvent> {
        let mut events = self.events.lock().unwrap();
        match events.is_empty() {
            true => None,
            false => Some(events.remove(0)),
        }
    }
}

fn pairing(a: &mut App) -> Script {
    let script = Script::default();
    a.set_romm_jobs(Box::new(script.clone()));
    a.romm.server = "romm.example.com".into();
    open_at(a, QuickRow::RommPair);
    press(a, Btn::A);
    script
}

fn pump(a: &mut App) {
    let now = a.now();
    a.tick_ms(now + 1);
}

fn texts(a: &App) -> Vec<String> {
    a.pair_view().unwrap().lines.into_iter().map(|(_, t)| t).collect()
}

#[test]
fn pair_screen_shows_code_and_b_cancels() {
    let (_d, mut a) = booted();
    let script = pairing(&mut a);
    assert!(matches!(a.phase(), Phase::RommPair));
    assert!(matches!(
        script.asked.lock().unwrap().as_slice(),
        [RommJob::Pair(conf)] if conf.server == "romm.example.com" && !conf.client_id.is_empty()
    ));
    script.events.lock().unwrap().push(RommEvent::PairShow {
        url: "https://romm.example.com/pair/device?user_code=K7M2QX9P".into(),
        code: "K7M2QX9P".into(),
    });
    pump(&mut a);
    let view = a.pair_view().unwrap();
    assert_eq!(
        view.url.as_deref(),
        Some("https://romm.example.com/pair/device?user_code=K7M2QX9P")
    );
    assert_eq!(
        texts(&a),
        ["Scan with", "Your phone", "Or enter", "K7M2QX9P", "Waiting"]
    );
    press(&mut a, Btn::B);
    assert_eq!(a.quick_menu(), Some(QuickRow::RommPair));
    assert_eq!(
        script.asked.lock().unwrap().last(),
        Some(&RommJob::CancelPair)
    );
}

#[test]
fn expired_offers_retry() {
    let (_d, mut a) = booted();
    let script = pairing(&mut a);
    script
        .events
        .lock()
        .unwrap()
        .push(RommEvent::PairFailed(PairFail::Expired));
    pump(&mut a);
    assert_eq!(texts(&a), ["Pairing", "expired"]);
    assert!(a.pair_view().unwrap().url.is_none());
    press(&mut a, Btn::A);
    assert!(matches!(a.phase(), Phase::RommPair));
    assert_eq!(texts(&a), ["Connecting"]);
    let asked = script.asked.lock().unwrap();
    assert_eq!(asked.len(), 2);
    assert!(matches!(asked[1], RommJob::Pair(_)));
}

#[test]
fn too_old_offers_no_retry() {
    let (_d, mut a) = booted();
    let script = pairing(&mut a);
    script
        .events
        .lock()
        .unwrap()
        .push(RommEvent::PairFailed(PairFail::TooOld));
    pump(&mut a);
    press(&mut a, Btn::A);
    assert_eq!(script.asked.lock().unwrap().len(), 1);
    assert!(matches!(a.phase(), Phase::RommPair));
}

#[test]
fn paired_writes_token_to_conf() {
    let (d, mut a) = booted();
    let script = pairing(&mut a);
    script.events.lock().unwrap().push(RommEvent::Paired {
        token: "rmm_secret".into(),
        device_id: "dev-9".into(),
    });
    pump(&mut a);
    let saved = RommConf::load(d.path());
    assert_eq!(saved.token.as_deref(), Some("rmm_secret"));
    assert_eq!(saved.device_id.as_deref(), Some("dev-9"));
    assert!(a.romm.paired());
    assert!(matches!(a.phase(), Phase::QuickMenu { .. }));
    assert_eq!(a.quick_menu(), Some(QuickRow::RommStatus));
}

#[test]
fn paired_after_leaving_the_screen_is_still_stored() {
    let (d, mut a) = booted();
    let script = pairing(&mut a);
    press(&mut a, Btn::B);
    assert!(!matches!(a.phase(), Phase::RommPair));
    script.events.lock().unwrap().push(RommEvent::Paired {
        token: "late".into(),
        device_id: "dev-7".into(),
    });
    pump(&mut a);
    let saved = RommConf::load(d.path());
    assert_eq!(saved.token.as_deref(), Some("late"));
    assert_eq!(saved.device_id.as_deref(), Some("dev-7"));
    assert!(a.romm.paired());
    assert!(!matches!(a.phase(), Phase::RommPair));
}

fn scripted(a: &mut App) -> Script {
    let script = Script::default();
    a.set_romm_jobs(Box::new(script.clone()));
    script
}

fn claims(script: &Script) -> usize {
    let asked = script.asked.lock().unwrap();
    asked.iter().filter(|j| **j == RommJob::Claim).count()
}

fn advance(a: &mut App, ms: u64) {
    let now = a.now();
    a.tick_ms(now + ms as Millis);
}

fn paired_on_the_shelf() -> (TempDir, App, Script) {
    let (d, mut a) = booted();
    pair(&mut a);
    a.romm.save(d.path()).unwrap();
    let script = scripted(&mut a);
    (d, a, script)
}

#[test]
fn start_goes_out_at_boot_when_paired() {
    let (_d, _a, script) = paired_on_the_shelf();
    assert!(matches!(
        script.asked.lock().unwrap().as_slice(),
        [RommJob::Start(conf)] if conf.device_id.as_deref() == Some("d1")
    ));
}

#[test]
fn no_start_at_boot_when_not_paired() {
    let (_d, mut a) = booted();
    let script = scripted(&mut a);
    assert!(script.asked.lock().unwrap().is_empty());
}

#[test]
fn start_goes_out_right_after_paired() {
    let (_d, mut a) = booted();
    let script = pairing(&mut a);
    script.events.lock().unwrap().push(RommEvent::Paired {
        token: "t".into(),
        device_id: "d".into(),
    });
    pump(&mut a);
    assert!(script
        .asked
        .lock()
        .unwrap()
        .iter()
        .any(|j| matches!(j, RommJob::Start(c) if c.token.as_deref() == Some("t"))));
}

#[test]
fn polls_every_thirty_seconds_on_the_shelf() {
    let (_d, mut a, script) = paired_on_the_shelf();
    advance(&mut a, 1);
    assert_eq!(claims(&script), 1);
    advance(&mut a, 29_000);
    assert_eq!(claims(&script), 1);
    advance(&mut a, 1_000);
    assert_eq!(claims(&script), 2);
    advance(&mut a, 29_000);
    assert_eq!(claims(&script), 2);
    advance(&mut a, 1_000);
    assert_eq!(claims(&script), 3);
}

#[test]
fn does_not_poll_in_a_game() {
    let (_d, mut a, script) = paired_on_the_shelf();
    a.apply(Action::Insert);
    assert!(a.seated_cart().is_some());
    advance(&mut a, 60_000);
    assert_eq!(claims(&script), 0);
}

#[test]
fn insert_stops_the_download() {
    let (_d, mut a, script) = paired_on_the_shelf();
    a.apply(Action::Insert);
    assert_eq!(script.asked.lock().unwrap().last(), Some(&RommJob::Stop));
}

#[test]
fn doze_stops_the_download_and_the_polls() {
    let (_d, mut a, script) = paired_on_the_shelf();
    a.apply(Action::LidClose);
    assert_eq!(script.asked.lock().unwrap().last(), Some(&RommJob::Stop));
    advance(&mut a, 1);
    assert_eq!(claims(&script), 0);
}

#[test]
fn losing_wifi_stops_and_its_return_claims_once_at_once() {
    let (_d, mut a, script) = paired_on_the_shelf();
    advance(&mut a, 1);
    assert_eq!(claims(&script), 1);
    a.set_wifi(slot::wifi::Wifi::inline(Box::new(slot::wifi::Fake::demo())));
    advance(&mut a, 1);
    assert!(!a.wifi_up());
    assert_eq!(script.asked.lock().unwrap().last(), Some(&RommJob::Stop));
    let asked = script.asked.lock().unwrap().len();
    advance(&mut a, 60_000);
    assert_eq!(script.asked.lock().unwrap().len(), asked);
    common::connect(&mut a);
    assert!(a.wifi_up());
    assert_eq!(claims(&script), 2);
    advance(&mut a, 1);
    assert_eq!(claims(&script), 2);
}

#[test]
fn waking_reopens_with_a_claim_at_once() {
    let (_d, mut a, script) = paired_on_the_shelf();
    advance(&mut a, 1);
    assert_eq!(claims(&script), 1);
    a.apply(Action::LidClose);
    advance(&mut a, 1);
    assert_eq!(claims(&script), 1);
    a.apply(Action::LidOpen);
    common::connect(&mut a);
    assert!(a.wifi_up());
    assert_eq!(claims(&script), 2);
}

#[test]
fn losing_wifi_closes_the_keyboard_under_the_notice() {
    let (_d, mut a) = booted();
    open_at(&mut a, QuickRow::RommPort);
    press(&mut a, Btn::A);
    assert!(a.keyboard().is_some());
    a.set_wifi(slot::wifi::Wifi::inline(Box::new(slot::wifi::Fake::demo())));
    advance(&mut a, 1);
    assert!(a.keyboard().is_none());
    assert!(a.update_text().is_some());
}

#[test]
fn a_gift_arriving_in_a_game_waits_for_the_shelf_to_write() {
    let (d, mut a, script) = paired_on_the_shelf();
    a.apply(Action::Insert);
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    script.events.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    pump(&mut a);
    assert!(!d.path().join(slot::romm::GIFTS).exists());
    a.apply(Action::Eject);
    for _ in 0..600 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert!(slot::romm::Gifts::load(d.path()).holds(Platform::Gba, "Sent"));
}

#[test]
fn does_not_poll_when_not_paired() {
    let (_d, mut a) = booted();
    let script = scripted(&mut a);
    advance(&mut a, 60_000);
    assert_eq!(claims(&script), 0);
}

#[test]
fn does_not_poll_when_wifi_is_down() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    let (mut a, _clock) = app_booting_at(d.path(), CLOCK_IS_SET);
    pair(&mut a);
    let script = scripted(&mut a);
    assert!(!a.wifi_up());
    advance(&mut a, 60_000);
    assert_eq!(claims(&script), 0);
}

#[test]
fn installed_adopts_the_rescan() {
    let (d, mut a, script) = paired_on_the_shelf();
    a.take_rescanned();
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    script.events.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    pump(&mut a);
    assert!(a.carts().any(|c| c.stem == "Sent"));
    assert!(a.take_rescanned());
}

#[test]
fn installed_during_a_game_waits_for_the_shelf() {
    let (d, mut a, script) = paired_on_the_shelf();
    a.apply(Action::Insert);
    a.take_rescanned();
    std::fs::write(d.path().join("Games/GBA/Sent.gba"), vec![0u8; 0x100]).unwrap();
    script.events.lock().unwrap().push(RommEvent::Installed {
        carts: slot_store::scan(d.path()).unwrap(),
        platform: Platform::Gba,
        stem: "Sent".into(),
    });
    pump(&mut a);
    assert!(!a.carts().any(|c| c.stem == "Sent"));
    assert!(!a.take_rescanned());
    a.apply(Action::Eject);
    for _ in 0..600 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Shelf), "{:?}", a.phase());
    assert!(a.carts().any(|c| c.stem == "Sent"));
    assert!(a.take_rescanned());
}

#[test]
fn disconnect_asks_the_worker_and_keeps_the_pairing_until_it_answers() {
    let (d, mut a, script) = paired_on_the_shelf();
    open_at(&mut a, QuickRow::RommDisconnect);
    press(&mut a, Btn::A);
    assert!(matches!(
        script.asked.lock().unwrap().last(),
        Some(RommJob::Disconnect(conf)) if conf.device_id.as_deref() == Some("d1")
    ));
    assert!(a.romm.paired());
    script.events.lock().unwrap().push(RommEvent::Unpaired);
    pump(&mut a);
    assert!(!a.romm.paired());
    assert!(!RommConf::load(d.path()).paired());
    assert_eq!(a.quick_menu(), Some(QuickRow::RommProtocol));
}

#[test]
fn unpaired_on_the_shelf_stays_on_the_shelf() {
    let (d, mut a, script) = paired_on_the_shelf();
    script.events.lock().unwrap().push(RommEvent::Unpaired);
    pump(&mut a);
    assert!(!RommConf::load(d.path()).paired());
    assert!(matches!(a.phase(), Phase::Shelf));
}

#[test]
fn romm_row_follows_whether_the_server_answers() {
    let (_d, mut a, script) = paired_on_the_shelf();
    assert_eq!(a.romm_texts()[0], "OFFLINE");
    script.events.lock().unwrap().push(RommEvent::Online(true));
    pump(&mut a);
    assert_eq!(a.romm_texts()[0], "ONLINE");
    script.events.lock().unwrap().push(RommEvent::Online(false));
    pump(&mut a);
    assert_eq!(a.romm_texts()[0], "OFFLINE");
}

#[test]
fn romm_row_reads_setup_again_after_unpairing() {
    let (_d, mut a, script) = paired_on_the_shelf();
    script.events.lock().unwrap().push(RommEvent::Online(true));
    pump(&mut a);
    script.events.lock().unwrap().push(RommEvent::Unpaired);
    pump(&mut a);
    assert_eq!(a.romm_texts()[0], "SETUP");
}

#[test]
fn a_note_shows_in_status_until_cleared() {
    let (_d, mut a, script) = paired_on_the_shelf();
    assert_eq!(a.romm_texts()[3], "Paired");
    script.events.lock().unwrap().push(RommEvent::Note("Off".into()));
    pump(&mut a);
    assert_eq!(a.romm_texts()[3], "Off");
    script.events.lock().unwrap().push(RommEvent::NoteCleared);
    pump(&mut a);
    assert_eq!(a.romm_texts()[3], "Paired");
}
