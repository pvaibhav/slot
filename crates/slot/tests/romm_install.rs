use std::io::Write;
use std::path::Path;

use slot::romm::install::{free_space, install, sweep, target_dir, Stopped, TEMP_DIR};
use slot::romm::{
    Api, ApiError, Cancel, Install, Outcome, Pending, Poll, RomFile, RomInfo, RommConf,
};
use slot_store::Platform;
use tempfile::tempdir;

struct Fake {
    files: Vec<RomFile>,
    asked: std::sync::Mutex<Vec<String>>,
    slug: &'static str,
    name: String,
    bytes: Vec<u8>,
    fail: Option<ApiError>,
}

impl Fake {
    fn new(slug: &'static str, name: &str, bytes: Vec<u8>) -> Self {
        Fake {
            slug,
            files: Vec::new(),
            asked: std::sync::Mutex::default(),
            name: name.to_string(),
            bytes,
            fail: None,
        }
    }
}

impl Api for Fake {
    fn version(&self) -> Result<String, ApiError> {
        Ok("5.4.0".into())
    }
    fn pair_start(&self, _: &RommConf) -> Result<Pending, ApiError> {
        Err(ApiError::Offline)
    }
    fn pair_poll(&self, _: &str) -> Result<Poll, ApiError> {
        Err(ApiError::Offline)
    }
    fn enable_install(&self, _: &str, _: &str) -> Result<(), ApiError> {
        Ok(())
    }
    fn claim(&self) -> Result<Vec<Install>, ApiError> {
        Ok(Vec::new())
    }
    fn device_exists(&self) -> Result<bool, ApiError> {
        Ok(true)
    }
    fn rom(&self, _: u64) -> Result<RomInfo, ApiError> {
        Ok(RomInfo {
            platform_slug: self.slug.into(),
            fs_name: self.name.clone(),
            files: self.files.clone(),
        })
    }
    fn download(
        &self,
        _: u64,
        name: &str,
        _: &[u64],
        to: &Path,
        cancel: &Cancel,
    ) -> Result<(), ApiError> {
        self.asked.lock().unwrap().push(name.to_string());
        std::fs::write(to, &self.bytes).unwrap();
        if let Some(e) = &self.fail {
            if *e == ApiError::Cancelled {
                cancel.set();
            }
            return Err(e.clone());
        }
        Ok(())
    }
    fn report(&self, _: &str, _: &Outcome) -> Result<(), ApiError> {
        Ok(())
    }
    fn delete_device(&self) -> Result<(), ApiError> {
        Ok(())
    }
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(data).unwrap();
        }
        w.finish().unwrap();
    }
    out
}

fn req() -> Install {
    Install {
        id: "r1".into(),
        rom_id: 7,
        file_ids: vec![1],
    }
}

fn roomy(_: &Path) -> u64 {
    u64::MAX
}

fn run(root: &Path, api: &Fake) -> Result<slot::romm::install::Finished, Stopped> {
    slot::root::ensure(root);
    install(root, api, &req(), &Cancel::default(), roomy)
}

fn failed_with(r: Result<slot::romm::install::Finished, Stopped>) -> String {
    match r.unwrap().outcome {
        Outcome::Failed(reason) => reason,
        other => panic!("{other:?}"),
    }
}

fn temps_empty(root: &Path) -> bool {
    std::fs::read_dir(root.join(TEMP_DIR)).unwrap().next().is_none()
}

#[test]
fn target_dir_maps_the_three_slugs() {
    assert_eq!(target_dir("gb"), Some(Platform::Gb));
    assert_eq!(target_dir("gbc"), Some(Platform::Gbc));
    assert_eq!(target_dir("GBA"), Some(Platform::Gba));
    assert_eq!(target_dir("gbav"), None);
    assert_eq!(target_dir("snes"), None);
}

#[test]
fn plain_rom_lands_in_its_folder() {
    let d = tempdir().unwrap();
    let api = Fake::new("gb", "Tetris.gb", b"rom".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert_eq!(done.stem.as_deref(), Some("Tetris"));
    assert_eq!(std::fs::read(d.path().join("Games/GB/Tetris.gb")).unwrap(), b"rom");
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_one_rom_installs_under_the_inner_name() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("readme.txt", b"hi"), ("Game.gba", b"gba-bytes")]);
    let api = Fake::new("gba", "Pack.zip", z);
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert_eq!(done.stem.as_deref(), Some("Game"));
    assert_eq!(
        std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(),
        b"gba-bytes"
    );
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_two_roms_fails() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("A.gba", b"a"), ("B.gba", b"b")]);
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.zip", z)));
    assert_eq!(reason, "Zip holds more than one ROM");
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_with_no_rom_fails() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("readme.txt", b"a")]);
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.zip", z)));
    assert_eq!(reason, "Zip holds no playable ROM");
}

#[test]
fn seven_z_fails() {
    let d = tempdir().unwrap();
    let reason = failed_with(run(d.path(), &Fake::new("gba", "Pack.7z", b"x".to_vec())));
    assert_eq!(reason, "Unsupported file type");
}

#[test]
fn other_platform_fails_with_reason() {
    let d = tempdir().unwrap();
    let reason = failed_with(run(d.path(), &Fake::new("snes", "Mario.sfc", b"x".to_vec())));
    assert_eq!(reason, "slot only plays GB, GBC and GBA");
}

#[test]
fn multi_file_rom_fails() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let mut r = req();
    r.file_ids = vec![1, 2];
    let api = Fake::new("gba", "Game.gba", b"x".to_vec());
    let out = install(d.path(), &api, &r, &Cancel::default(), roomy);
    assert_eq!(failed_with(out), "Multi-file ROMs are not supported");
}

#[test]
fn same_name_is_already_installed() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    std::fs::write(d.path().join("Games/GBA/Game.gba"), b"mine").unwrap();
    let api = Fake::new("gba", "Game.gba", b"theirs".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::AlreadyInstalled);
    assert_eq!(std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(), b"mine");

    let z = zip_of(&[("Game.gba", b"theirs")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::AlreadyInstalled);
    assert_eq!(std::fs::read(d.path().join("Games/GBA/Game.gba")).unwrap(), b"mine");
    assert!(temps_empty(d.path()));
}

#[test]
fn zip_entry_with_parent_path_stays_in_the_folder() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("../../evil.gba", b"evil")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/evil.gba").is_file());
    assert!(!d.path().join("evil.gba").exists());
}

#[test]
fn zip_entry_with_a_trailing_space_is_trimmed() {
    let d = tempdir().unwrap();
    let z = zip_of(&[("Game.gba ", b"x")]);
    let done = run(d.path(), &Fake::new("gba", "Pack.zip", z)).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/Game.gba").is_file());
}

#[test]
fn name_with_a_slash_stays_in_the_folder() {
    let d = tempdir().unwrap();
    let api = Fake::new("gba", "../../evil.gba", b"x".to_vec());
    let done = run(d.path(), &api).unwrap();
    assert_eq!(done.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/evil.gba").is_file());
    assert!(!d.path().join("evil.gba").exists());
}

#[test]
fn rename_failure_says_it_could_not_save() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let games = d.path().join("Games/GBA");
    std::fs::set_permissions(&games, std::fs::Permissions::from_mode(0o555)).unwrap();
    let api = Fake::new("gba", "Game.gba", b"x".to_vec());
    let out = run(d.path(), &api);
    std::fs::set_permissions(&games, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(failed_with(out), "Could not save the game");
    assert!(temps_empty(d.path()));
}

#[test]
fn write_error_fails_at_once_with_no_retry() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Write("disk".into()));
    assert_eq!(failed_with(run(d.path(), &api)), "Card is full");
    assert!(temps_empty(d.path()));
}

#[test]
fn errors_map_to_stops() {
    for (err, want) in [
        (ApiError::Offline, Stopped::Offline),
        (ApiError::Server(502), Stopped::Transient),
        (ApiError::Truncated, Stopped::Transient),
        (ApiError::NotFound, Stopped::Lost(ApiError::NotFound)),
        (ApiError::Unauthorized, Stopped::Lost(ApiError::Unauthorized)),
    ] {
        let d = tempdir().unwrap();
        let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
        api.fail = Some(err);
        assert_eq!(run(d.path(), &api), Err(want));
        assert!(temps_empty(d.path()));
    }
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Client(403));
    assert!(failed_with(run(d.path(), &api)).contains("403"));
}

#[test]
fn a_folder_rom_with_one_file_installs_under_that_file_name() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Folder", b"x".to_vec());
    api.files = vec![
        RomFile {
            id: 1,
            file_name: "Game.gba".into(),
        },
        RomFile {
            id: 2,
            file_name: "Notes.txt".into(),
        },
    ];
    let out = run(d.path(), &api).unwrap();
    assert_eq!(out.outcome, Outcome::Done);
    assert!(d.path().join("Games/GBA/Game.gba").exists());
    assert_eq!(*api.asked.lock().unwrap(), ["Game.gba"]);
}

#[test]
fn cancel_leaves_no_temp_and_no_rom() {
    let d = tempdir().unwrap();
    let mut api = Fake::new("gba", "Game.gba", b"x".to_vec());
    api.fail = Some(ApiError::Cancelled);
    assert_eq!(run(d.path(), &api), Err(Stopped::Cancelled));
    assert!(temps_empty(d.path()));
    assert!(!d.path().join("Games/GBA/Game.gba").exists());
}

#[test]
fn sweep_clears_temps() {
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    std::fs::write(d.path().join(TEMP_DIR).join("a.part"), b"x").unwrap();
    sweep(d.path());
    assert!(temps_empty(d.path()));
    std::fs::write(d.path().join(TEMP_DIR).join("b.rom"), b"x").unwrap();
    slot::root::ensure(d.path());
    assert!(temps_empty(d.path()));
}

#[test]
fn full_card_fails() {
    fn tight(_: &Path) -> u64 {
        10
    }
    let d = tempdir().unwrap();
    slot::root::ensure(d.path());
    let z = zip_of(&[("Game.gba", &[7u8; 4096])]);
    let out = install(
        d.path(),
        &Fake::new("gba", "Pack.zip", z),
        &req(),
        &Cancel::default(),
        tight,
    );
    assert_eq!(failed_with(out), "Card is full");
    assert!(temps_empty(d.path()));
    assert!(!d.path().join("Games/GBA/Game.gba").exists());
}

#[test]
fn free_space_reads_the_real_card() {
    let d = tempdir().unwrap();
    assert!(free_space(d.path()) > 0);
}
