use super::*;
use serde_json::{json, Value};
use slot_retro::{AvInfo, ButtonMask, CoreError};
use std::path::Path;
use storage::{Achievement, Auth, Game};

fn achievement(definition: &str) -> Achievement {
    Achievement {
        badge: "12345".into(),
        id: 7,
        title: "Test unlocked".into(),
        description: "A real runtime trigger".into(),
        points: 5,
        flags: 3,
        definition: definition.into(),
    }
}

fn game() -> Game {
    Game {
        presence: String::new(),
        id: 1,
        console: 5,
        title: "Test game".into(),
        achievements: vec![achievement("0xH000000=1")],
    }
}

fn configured() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("Config")).unwrap();
    std::fs::create_dir_all(root.path().join("Games/GBA")).unwrap();
    std::fs::write(
        root.path().join("Config/retroachievements.toml"),
        "enabled = true\nusername = 'Player'\ntoken = 'fixture-token'\n",
    )
    .unwrap();
    root
}

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "worker did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn toml_config_accepts_comments_and_literal_credentials_without_leaking_errors() {
    let root = configured();
    let path = root.path().join("Config/retroachievements.toml");
    std::fs::write(
        &path,
        "# Account\nenabled = true\nusername = 'Player'\npassword = 'secret\\with#characters'\n",
    )
    .unwrap();
    let config = Config::read(&path).unwrap();
    assert!(config.enabled);
    assert_eq!(config.password, "secret\\with#characters");
    assert!(config.token.is_empty());
    std::fs::write(&path, "password = 'secret'\nenabeld = true\n").unwrap();
    assert!(matches!(
        Config::read(&path),
        Err("Invalid achievement config")
    ));
    let service = Service::start_with(root.path().into(), Offline);
    wait_for(|| service.sync_status() == SyncStatus::Attention);
    assert!(!service.enabled.load(Ordering::Acquire));
}

struct Offline;
impl network::Transport for Offline {
    fn call(&mut self, _: &[(&str, String)]) -> Result<Value, network::Failure> {
        Err(network::Failure::Network)
    }
}

struct Server {
    calls: mpsc::Sender<String>,
}

#[test]
fn wifi_reconnect_wakes_offline_sync_without_restarting_or_opening_a_game() {
    struct Reconnect {
        online: Arc<AtomicBool>,
        server: Server,
    }
    impl network::Transport for Reconnect {
        fn call(&mut self, fields: &[(&str, String)]) -> Result<Value, network::Failure> {
            if self.online.load(Ordering::Acquire) {
                self.server.call(fields)
            } else {
                Err(network::Failure::Network)
            }
        }
    }
    let root = configured();
    let mut store = Store::open(root.path(), "Player").unwrap();
    store
        .record(Unlock {
            id: 7,
            hash: "abcdef".into(),
            earned_at: network::now(),
            synced: false,
        })
        .unwrap();
    let online = Arc::new(AtomicBool::new(false));
    let (calls, _) = mpsc::channel();
    let service = Service::start_with(
        root.path().into(),
        Reconnect {
            online: online.clone(),
            server: Server { calls },
        },
    );
    wait_for(|| service.sync_status() == SyncStatus::Offline);
    online.store(true, Ordering::Release);
    service.network_available();
    // wait_for's five-second deadline is much shorter than the thirty-second backoff.
    wait_for(|| service.sync_status() == SyncStatus::Ready);
    assert!(Store::open(root.path(), "Player").unwrap().unlocks[&7].synced);
}
impl network::Transport for Server {
    fn badge(&mut self, name: &str) -> Result<Vec<u8>, network::Failure> {
        let _ = self.calls.send(format!("badge:{name}"));
        Ok(badge_png())
    }
    fn call(&mut self, fields: &[(&str, String)]) -> Result<Value, network::Failure> {
        let request = fields.iter().find(|(k, _)| *k == "r").unwrap().1.as_str();
        let _ = self.calls.send(request.into());
        Ok(match request {
            "login2" => json!({"Success":true,"User":"Player","Token":"fixture-token"}),
            "gameid" => json!({"Success":true,"GameID":1}),
            "patch" => json!({"Success":true,"PatchData":game()}),
            "unlocks" => json!({"Success":true,"UserUnlocks":[]}),
            "startsession" => json!({"Success":true,"Unlocks":[],"HardcoreUnlocks":[]}),
            "ping" => {
                assert!(fields
                    .iter()
                    .any(|(k, v)| *k == "m" && v == "Playing Test game"));
                assert!(fields.iter().any(|(k, v)| *k == "x" && v.len() == 32));
                json!({"Success":true})
            }
            "awardachievement" => json!({"Success":true}),
            _ => panic!("unexpected API {request}"),
        })
    }
}

fn badge_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[40, 120, 200, 255].repeat(4))
            .unwrap();
    }
    bytes
}

#[test]
fn badge_cache_survives_restart_and_repairs_corrupt_images() {
    let root = configured();
    let store = Store::open(root.path(), "Player").unwrap();
    let path = badges::path(&store.dir, "12345").unwrap();
    let (calls, requests) = mpsc::channel();
    let mut http = Server { calls };
    let mut queue = badges::Queue::new(&store.dir);
    queue.enqueue(&game());
    queue.enqueue(&game());
    queue.step(&mut http);
    assert!(!queue.pending());
    assert_eq!(requests.try_iter().count(), 1);
    let mut restarted = badges::Queue::new(&store.dir);
    restarted.enqueue(&game());
    assert!(!restarted.pending());
    // Notifications carry a local path even offline, with no network dependency.
    assert_eq!(load_badge(&path).unwrap().width, 2);
    std::fs::write(&path, b"corrupted PNG").unwrap();
    restarted.enqueue(&game());
    restarted.step(&mut http);
    assert!(load_badge(&path).is_some());
    assert!(badges::path(&store.dir, "../../auth").is_none());
    assert!(badges::path(&store.dir, "https://example.org/badge").is_none());
}

#[test]
fn a_badge_failure_does_not_hold_up_awards_or_cached_game_data() {
    struct NoBadges(Server);
    impl network::Transport for NoBadges {
        fn call(&mut self, fields: &[(&str, String)]) -> Result<Value, network::Failure> {
            self.0.call(fields)
        }
    }
    let root = configured();
    std::fs::write(root.path().join("Games/GBA/Test.gba"), b"fixture").unwrap();
    let mut store = Store::open(root.path(), "Player").unwrap();
    store
        .record(Unlock {
            id: 7,
            hash: "abcdef".into(),
            earned_at: network::now(),
            synced: false,
        })
        .unwrap();
    let (calls, _) = mpsc::channel();
    let service = Service::start_with(root.path().into(), NoBadges(Server { calls }));
    wait_for(|| service.sync_status() == SyncStatus::Attention);
    let store = Store::open(root.path(), "Player").unwrap();
    assert!(store.unlocks[&7].synced);
    let hash = format!("{:x}", md5::compute(b"fixture"));
    assert!(store.dir.join(format!("{hash}.json")).exists());
    assert!(load_badge(&badges::path(&store.dir, "12345").unwrap()).is_none());
}

#[derive(Default)]
struct TestCore {
    value: u8,
}
impl RetroCore for TestCore {
    fn load(&mut self, _: &Path) -> Result<(), CoreError> {
        Ok(())
    }
    fn run_frame(&mut self, input: ButtonMask) {
        self.value = u8::from(input.0 != 0);
    }
    fn video_xrgb8888(&self) -> &[u8] {
        &[]
    }
    fn take_audio(&mut self) -> Vec<i16> {
        vec![]
    }
    fn serialize(&mut self) -> Result<Vec<u8>, CoreError> {
        Ok(vec![self.value])
    }
    fn unserialize(&mut self, _: &[u8]) -> Result<(), CoreError> {
        Ok(())
    }
    fn save_ram(&self) -> Option<Vec<u8>> {
        None
    }
    fn load_save_ram(&mut self, _: &[u8]) -> Result<(), CoreError> {
        Ok(())
    }
    fn av_info(&self) -> AvInfo {
        AvInfo {
            fps: 60.0,
            sample_rate: 48000.0,
        }
    }
    fn achievement_memory(&self, ram: &mut [u8]) -> [usize; 3] {
        ram[0] = self.value;
        [0x8000, 0x40000, 0]
    }
}

#[test]
fn runtime_evaluates_real_definitions_and_never_awards_missing_memory() {
    let mut runtime = runtime::Runtime::new().unwrap();
    assert!(runtime.activate(7, "0xH000000=1"));
    assert!(runtime.activate(8, "0xH048000=1"));
    assert!(!runtime.activate(9, "not a definition"));
    let mut ram = vec![0; RAM_SIZE];
    assert!(runtime.frame(&ram, &[0x8000, 0x40000, 0]).is_empty());
    ram[0] = 1;
    ram[0x48000] = 1;
    assert_eq!(runtime.frame(&ram, &[0x8000, 0x40000, 0]), &[7]);
    assert!(runtime.frame(&ram, &[0x8000, 0x40000, 0]).is_empty());
}

#[test]
fn offline_unlock_is_durable_then_syncs_without_reopening_the_game() {
    let root = configured();
    let rom = root.path().join("Games/GBA/Test.gba");
    std::fs::write(&rom, b"fixture ROM").unwrap();
    let store = Store::open(root.path(), "Player").unwrap();
    let hash = library::hash(&store.dir, &rom).unwrap();
    storage::write(
        &store.dir.join("auth.json"),
        &Auth {
            username: "Player".into(),
            token: "fixture-token".into(),
        },
    )
    .unwrap();
    storage::write(
        &store.dir.join(format!("{hash}.json")),
        &(game(), std::collections::BTreeSet::<u32>::new()),
    )
    .unwrap();
    let service = Service::start_with(root.path().into(), Offline);
    let mut core = service.wrap(Box::<TestCore>::default());
    core.load(&rom).unwrap();
    wait_for(|| {
        service
            .take_notice()
            .is_some_and(|n| n.title.starts_with("Achievements ready"))
    });
    core.run_frame(ButtonMask(0));
    core.run_frame(ButtonMask(1));
    // Eject immediately: queued final frames must be evaluated before Unload retires them.
    drop(core);
    wait_for(|| {
        service
            .take_notice()
            .is_some_and(|n| n.kind == NoticeKind::Earned)
    });
    let saved = Store::open(root.path(), "Player").unwrap();
    assert!(!saved.unlocks[&7].synced);
    assert_eq!(saved.unlocks[&7].hash, hash);
    wait_for(|| service.sync_status() == SyncStatus::Offline);
    drop(service);

    let restarted = Service::start_with(root.path().into(), Offline);
    let mut core = restarted.wrap(Box::<TestCore>::default());
    core.load(&rom).unwrap();
    wait_for(|| {
        restarted
            .take_notice()
            .is_some_and(|n| n.title.starts_with("Achievements ready"))
    });
    core.run_frame(ButtonMask(0));
    core.run_frame(ButtonMask(1));
    drop(core);
    // A reconnect uses the same persistent account ledger without needing a running core.
    drop(restarted);
    let (calls, requests) = mpsc::channel();
    let online = Service::start_with(root.path().into(), Server { calls });
    wait_for(|| {
        online
            .take_notice()
            .is_some_and(|n| n.title == "Achievements synced")
    });
    assert!(Store::open(root.path(), "Player").unwrap().unlocks[&7].synced);
    assert_eq!(
        requests
            .try_iter()
            .filter(|r| r == "awardachievement")
            .count(),
        1
    );
    assert!(Store::open(root.path(), "OtherPlayer")
        .unwrap()
        .unlocks
        .is_empty());
}

#[test]
fn an_unopened_library_is_prepared_automatically_without_starting_sessions() {
    let root = configured();
    let rom = root.path().join("Games/GBA/Unopened.gba");
    std::fs::write(&rom, b"unopened ROM").unwrap();
    let (calls, requests) = mpsc::channel();
    let service = Service::start_with(root.path().into(), Server { calls });
    let store = Store::open(root.path(), "Player").unwrap();
    let hash = format!("{:x}", md5::compute(b"unopened ROM"));
    wait_for(|| store.dir.join(format!("{hash}.json")).exists());
    wait_for(|| service.sync_status() == SyncStatus::Ready);
    let badge = badges::path(&store.dir, "12345").unwrap();
    assert_eq!(
        load_badge(&badge).unwrap().rgba,
        [40, 120, 200, 255].repeat(4)
    );
    let (cached, _): (Game, std::collections::BTreeSet<u32>) =
        storage::read(&store.dir.join(format!("{hash}.json"))).unwrap();
    assert_eq!(cached.achievements[0].id, 7);
    let calls: Vec<_> = requests.try_iter().collect();
    assert!(calls.iter().any(|c| c == "unlocks"));
    assert!(!calls.iter().any(|c| c == "startsession"));
    wait_for(|| library::pending(root.path(), &store.dir, network::now()).is_empty());
    std::fs::write(&rom, b"changed content, different size").unwrap();
    assert_eq!(
        library::pending(root.path(), &store.dir, network::now()).len(),
        1
    );
    assert_ne!(library::hash(&store.dir, &rom).unwrap(), hash);
}

#[test]
fn a_full_worker_queue_cannot_block_the_emulator() {
    let (controls, _controls_rx) = mpsc::channel();
    let (frames, _frames_rx) = mpsc::sync_channel(QUEUE_SIZE);
    let (_, notices) = mpsc::channel();
    let service = Service {
        controls,
        frames,
        notices,
        enabled: Arc::new(AtomicBool::new(true)),
        current: Arc::new(AtomicU64::new(1)),
        flushing: AtomicBool::new(false),
        flushed: Arc::new(AtomicBool::new(false)),
        status: Arc::new(Status::default()),
    };
    let mut core = service.wrap(Box::<TestCore>::default());
    let (done, completed) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        for _ in 0..1000 {
            core.run_frame(ButtonMask(1));
        }
        done.send(()).unwrap();
    });
    let result = completed.recv_timeout(Duration::from_secs(2));
    // Release the receiver even on failure, so a regressed blocking send can unwind.
    drop(_frames_rx);
    worker.join().unwrap();
    result.expect("emulation waited for the achievement worker");
}

#[test]
fn damaged_ledger_is_preserved_and_failed_writes_are_not_acknowledged() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::open(root.path(), "Player").unwrap();
    let unlock = Unlock {
        id: 7,
        hash: "hash".into(),
        earned_at: 1000,
        synced: false,
    };
    store.record(unlock.clone()).unwrap();
    assert!(!store.record(unlock).unwrap());
    let path = store.dir.join("unlocks.json");
    std::fs::write(&path, b"broken").unwrap();
    assert!(Store::open(root.path(), "Player").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(store.ack(7).is_err());
    assert!(!store.unlocks[&7].synced);
}

fn frame(sequence: u64, timeline: u64, value: u8) -> Frame {
    let mut ram = vec![0; RAM_SIZE];
    ram[0] = value;
    let (recycle, _) = mpsc::sync_channel(1);
    Frame {
        generation: 1,
        sequence,
        timeline,
        earned_at: 1000,
        ram,
        valid: [0x8000, 0x40000, 0],
        recycle,
    }
}

#[test]
fn dropped_frames_and_rewinds_reset_partial_hit_counts() {
    for timeline in [0, 1] {
        let root = tempfile::tempdir().unwrap();
        let store = Arc::new(Mutex::new(Store::open(root.path(), "Player").unwrap()));
        let mut runtime = runtime::Runtime::new().unwrap();
        let a = achievement("0xH000000=1.2.");
        assert!(runtime.activate(a.id, &a.definition));
        let mut playing = Some(Playing {
            generation: 1,
            hash: "hash".into(),
            runtime,
            achievements: [(7, a)].into(),
            previous: None,
            warned_gap: false,
        });
        let (notices, _) = mpsc::channel();
        let mut unsaved = BTreeMap::new();
        evaluate(frame(1, 0, 0), &mut playing, &store, &notices, &mut unsaved);
        evaluate(frame(2, 0, 1), &mut playing, &store, &notices, &mut unsaved);
        evaluate(
            frame(if timeline == 0 { 4 } else { 3 }, timeline, 1),
            &mut playing,
            &store,
            &notices,
            &mut unsaved,
        );
        assert!(store.lock().unwrap().unlocks.is_empty());
        evaluate(
            frame(5, timeline, 0),
            &mut playing,
            &store,
            &notices,
            &mut unsaved,
        );
        evaluate(
            frame(6, timeline, 1),
            &mut playing,
            &store,
            &notices,
            &mut unsaved,
        );
        evaluate(
            frame(7, timeline, 1),
            &mut playing,
            &store,
            &notices,
            &mut unsaved,
        );
        assert!(store.lock().unwrap().unlocks.contains_key(&7));
    }
}

#[test]
fn a_cached_game_can_earn_and_flush_while_https_is_stalled() {
    struct Stalled {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl network::Transport for Stalled {
        fn call(&mut self, _: &[(&str, String)]) -> Result<Value, network::Failure> {
            let _ = self.entered.send(());
            let _ = self.release.recv();
            Err(network::Failure::Network)
        }
    }
    let root = configured();
    let rom = root.path().join("Games/GBA/Test.gba");
    std::fs::write(&rom, b"fixture ROM").unwrap();
    let store = Store::open(root.path(), "Player").unwrap();
    let hash = library::hash(&store.dir, &rom).unwrap();
    storage::write(
        &store.dir.join("auth.json"),
        &Auth {
            username: "Player".into(),
            token: "fixture-token".into(),
        },
    )
    .unwrap();
    storage::write(
        &store.dir.join(format!("{hash}.json")),
        &(game(), std::collections::BTreeSet::<u32>::new()),
    )
    .unwrap();
    let (entered, blocked) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let service = Service::start_with(
        root.path().into(),
        Stalled {
            entered,
            release: wait,
        },
    );
    blocked.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(service.sync_status(), SyncStatus::Syncing);
    let mut core = service.wrap(Box::<TestCore>::default());
    core.load(&rom).unwrap();
    wait_for(|| {
        service
            .take_notice()
            .is_some_and(|n| n.title.starts_with("Achievements ready"))
    });
    core.run_frame(ButtonMask(0));
    core.run_frame(ButtonMask(1));
    wait_for(|| service.flush_ready());
    assert!(Store::open(root.path(), "Player")
        .unwrap()
        .unlocks
        .contains_key(&7));
    // No HTTPS response was needed for preparation, evaluation, notification, or flushing.
    drop(release);
}

#[test]
fn disabling_achievements_cannot_hold_up_poweroff() {
    let root = tempfile::tempdir().unwrap();
    let service = Service::start_with(root.path().into(), Offline);
    wait_for(|| service.flush_ready());
    assert_eq!(service.sync_status(), SyncStatus::Disabled);
}

#[test]
fn unsaved_unlocks_retry_with_the_original_time_after_storage_recovers() {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Mutex::new(Store::open(root.path(), "Player").unwrap()));
    let path = store.lock().unwrap().dir.join("unlocks.json");
    std::fs::create_dir(&path).unwrap();
    let mut runtime = runtime::Runtime::new().unwrap();
    let a = achievement("0xH000000=1");
    runtime.activate(a.id, &a.definition);
    let mut playing = Some(Playing {
        generation: 1,
        hash: "hash".into(),
        runtime,
        achievements: [(7, a)].into(),
        previous: None,
        warned_gap: false,
    });
    let (notices, received) = mpsc::channel();
    let mut unsaved = BTreeMap::new();
    evaluate(frame(1, 0, 0), &mut playing, &store, &notices, &mut unsaved);
    evaluate(frame(2, 0, 1), &mut playing, &store, &notices, &mut unsaved);
    assert_eq!(unsaved.len(), 1);
    assert!(store.lock().unwrap().unlocks.is_empty());
    assert!(!received.try_iter().any(|n| n.kind == NoticeKind::Earned));
    std::fs::remove_dir(path).unwrap();
    retry_unsaved(&mut unsaved, &store, &notices);
    assert!(unsaved.is_empty());
    assert_eq!(
        Store::open(root.path(), "Player").unwrap().unlocks[&7].earned_at,
        1000
    );
    assert!(received.try_iter().any(|n| n.kind == NoticeKind::Earned));
}

#[test]
fn cache_percentage_counts_unique_badges_and_resumes_from_disk() {
    let root = configured();
    let store = Store::open(root.path(), "Player").unwrap();
    let mut data = game();
    let mut second = achievement("0xH000000=2");
    second.id = 8;
    second.badge = "00002".into();
    data.achievements.push(second);
    let (calls, _) = mpsc::channel();
    let mut http = Server { calls };
    let mut queue = badges::Queue::new(&store.dir);
    queue.enqueue(&data);
    queue.enqueue(&data);
    assert_eq!(queue.percent(), 0);
    queue.step(&mut http);
    assert_eq!(queue.percent(), 50);
    queue.step(&mut Offline);
    assert_eq!(queue.percent(), 50); // Failures never count as completed downloads.
    let mut restarted = badges::Queue::new(&store.dir);
    restarted.enqueue(&data);
    assert_eq!(restarted.percent(), 50);
    restarted.step(&mut http);
    assert_eq!(restarted.percent(), 100);
    assert!(!restarted.pending());
}

#[test]
fn rich_presence_tracks_memory_and_clears_invalid_replacements() {
    let mut runtime = runtime::Runtime::new().unwrap();
    assert!(runtime.activate_presence("Display:\n?0xH000000=1?In the castle\nOn the map"));
    let mut ram = vec![0; RAM_SIZE];
    runtime.frame(&ram, &[0x8000, 0x40000, 0x10000]);
    assert_eq!(runtime.presence(), "On the map");
    ram[0] = 1;
    runtime.frame(&ram, &[0x8000, 0x40000, 0x10000]);
    assert_eq!(runtime.presence(), "In the castle");
    assert!(!runtime.activate_presence("not a valid script"));
    runtime.frame(&ram, &[0x8000, 0x40000, 0x10000]);
    assert!(runtime.presence().is_empty());
}

#[test]
fn rich_presence_cache_accepts_old_data_and_retains_new_scripts() {
    let mut value = serde_json::to_value(game()).unwrap();
    value.as_object_mut().unwrap().remove("RichPresencePatch");
    assert!(serde_json::from_value::<Game>(value)
        .unwrap()
        .presence
        .is_empty());
    let mut data = game();
    data.presence = "Display:\nOn the map".into();
    let root = configured();
    let path = root.path().join("game.json");
    storage::write(&path, &data).unwrap();
    assert_eq!(
        storage::read::<Game>(&path).unwrap().presence,
        data.presence
    );
}

#[test]
fn loading_a_game_sends_presence_without_waiting_for_heartbeat() {
    let root = configured();
    let rom = root.path().join("Games/GBA/Test.gba");
    std::fs::write(&rom, b"fixture ROM").unwrap();
    let (calls, requests) = mpsc::channel();
    let service = Service::start_with(root.path().into(), Server { calls });
    wait_for(|| service.enabled.load(Ordering::Acquire));
    let mut core = service.wrap(Box::<TestCore>::default());
    core.load(&rom).unwrap();
    wait_for(|| requests.try_iter().any(|request| request == "ping"));
}

#[test]
fn null_rich_presence_from_the_service_is_an_empty_script() {
    let mut value = serde_json::to_value(game()).unwrap();
    value["RichPresencePatch"] = Value::Null;
    let parsed = serde_json::from_value::<Game>(value.clone()).unwrap();
    assert!(parsed.presence.is_empty());
    assert_eq!(parsed.achievements.len(), 1);
    value["RichPresencePatch"] = json!(42);
    assert!(serde_json::from_value::<Game>(value).is_err());
}

#[test]
fn artwork_identity_follows_rom_bytes_instead_of_its_filename() {
    let root = configured();
    let rom = root.path().join("Games/GBA/Completely renamed.gba");
    std::fs::write(&rom, b"same rom bytes").unwrap();
    let hash = library::content_hash(&rom).unwrap();
    let store = Store::open(root.path(), "Player").unwrap();
    let mut data = game();
    data.title = "Tomb Raider: Legend".into();
    let ids = std::collections::BTreeSet::<u32>::new();
    storage::write(&store.dir.join(format!("{hash}.json")), &(&data, ids)).unwrap();
    assert_eq!(
        artwork_title(root.path(), &rom).as_deref(),
        Some("Tomb Raider: Legend")
    );
    let moved = rom.with_file_name("Yet another name.gba");
    std::fs::rename(&rom, &moved).unwrap();
    assert_eq!(
        artwork_title(root.path(), &moved).as_deref(),
        Some("Tomb Raider: Legend")
    );
    std::fs::write(&moved, b"different rom bytes").unwrap();
    assert!(artwork_title(root.path(), &moved).is_none());
}
