use std::io::{Cursor, Read, Write};

use slot::update::{
    check_with, clean_baseos, clock_is_wrong, install_baseos_with, install_with, newer,
    parse_baseos, parse_release, Device, read_counted, stage, verify, wifi_conf, will_apply, Kind, Release,
    Running, Stoppable, Ticket, UpdateEvent, UpdateFail,
};

const DIGEST: &str = "e78b2771abbc41a07774fe870e08d3d870c1a06c6db42a1d799626190156aba5";

fn release_json(tag: &str, zip: &str, sha256: &str) -> String {
    let version = tag.trim_start_matches('v');
    format!(
        r#"{{
  "stable": {{
    "version": "{version}",
    "tag": "{tag}",
    "zip": "{zip}",
    "size": 6919551,
    "sha256": "{sha256}"
  }}
}}"#
    )
}

fn slot_only(slot: &str) -> Running<'_> {
    Running { slot, baseos: None }
}

const BOS: &str = "7e4e9ea57b6c52bb52bd627196017263e6072691a54c66e440e85faf03f1a6b1";

fn baseos_json(version: &str, rgsp_name: &str, sha256: &str) -> String {
    let slot = v1_11();
    let slot = slot.trim_end().trim_end_matches('}');
    format!(
        r#"{slot},
  "baseos": {{
    "version": "{version}",
    "tag": "v{version}",
    "files": {{
      "rgsp": {{"name": "{rgsp_name}", "size": 54917120, "sha256": "{sha256}"}},
      "rg35xxsp": {{"name": "baseos-rg35xxsp-{version}.bosupd", "size": 54900000, "sha256": "{BOS}"}}
    }}
  }}
}}"#
    )
}

fn both_json() -> String {
    baseos_json("1.4.0", "baseos-rgsp-1.4.0.bosupd", BOS)
}

#[test]
fn parse_baseos_reads_this_targets_file() {
    let r = parse_baseos(&both_json(), "rgsp", "1.3.0").unwrap();
    assert_eq!(r.kind, Kind::BaseOs);
    assert_eq!(r.version, "1.4.0");
    assert_eq!(r.running, "1.3.0");
    assert_eq!(
        r.url,
        "https://github.com/pvaibhav/BaseOS/releases/download/v1.4.0/baseos-rgsp-1.4.0.bosupd"
    );
    assert_eq!(r.size, 54917120);
    assert_eq!(&r.sha256[..2], &[0x7e, 0x4e]);
}

#[test]
fn parse_baseos_ignores_another_target_a_suffix_a_bad_name_and_a_short_digest() {
    assert_eq!(parse_baseos(&both_json(), "rg28xx", "1.3.0"), None);
    let suffix = baseos_json("1.5.0-rc1", "baseos-rgsp-1.5.0-rc1.bosupd", BOS);
    assert_eq!(parse_baseos(&suffix, "rgsp", "1.3.0"), None);
    let named = baseos_json("1.4.0", "baseos-rg35xxh-1.4.0.bosupd", BOS);
    assert_eq!(parse_baseos(&named, "rgsp", "1.3.0"), None);
    let short = baseos_json("1.4.0", "baseos-rgsp-1.4.0.bosupd", &BOS[1..]);
    assert_eq!(parse_baseos(&short, "rgsp", "1.3.0"), None);
    assert_eq!(parse_baseos(&v1_11(), "rgsp", "1.3.0"), None);
}

fn kinds(e: UpdateEvent) -> Vec<Kind> {
    match e {
        UpdateEvent::Available(v) => v.iter().map(|r| r.kind).collect(),
        other => panic!("got {other:?}"),
    }
}

fn check_both(json: String, slot: &str, baseos: &str) -> UpdateEvent {
    let mut fetch = |_: &str| Ok(json.clone().into_bytes());
    check_with(
        u64::MAX / 2,
        &mut fetch,
        Running {
            slot,
            baseos: Some(("rgsp", baseos)),
        },
    )
}

#[test]
fn a_check_offers_slot_then_baseos() {
    assert_eq!(
        kinds(check_both(both_json(), "1.10.0", "1.3.0")),
        [Kind::Slot, Kind::BaseOs]
    );
}

#[test]
fn a_check_offers_only_what_is_newer() {
    assert_eq!(
        kinds(check_both(both_json(), "1.10.0", "1.5.2")),
        [Kind::Slot]
    );
    assert_eq!(
        kinds(check_both(both_json(), "1.11.0", "1.3.0")),
        [Kind::BaseOs]
    );
    assert_eq!(
        check_both(both_json(), "1.11.0", "1.4.0"),
        UpdateEvent::UpToDate
    );
}

#[test]
fn a_broken_baseos_entry_never_fails_the_check() {
    let broken = baseos_json("1.4.0", "baseos-rgsp-1.4.0.bosupd", "nothex");
    assert_eq!(kinds(check_both(broken, "1.10.0", "1.3.0")), [Kind::Slot]);
}

#[test]
fn no_baseos_target_means_no_baseos_offer() {
    let mut fetch = |_: &str| Ok(both_json().into_bytes());
    assert_eq!(
        kinds(check_with(u64::MAX / 2, &mut fetch, slot_only("1.10.0"))),
        [Kind::Slot]
    );
}

fn v1_11() -> String {
    release_json("v1.11.0", "slot-v1.11.0-h700.zip", DIGEST)
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let opts = zip::write::SimpleFileOptions::default();
    for (name, bytes) in entries {
        w.start_file(*name, opts).unwrap();
        w.write_all(bytes).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn release_zip(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("slot-v1.12.0/System/slot", b"bin"),
        ("slot-v1.12.0/System/launch_frontend.sh", b"sh"),
        ("slot-v1.12.0/System/mgba_libretro.so", b"mgba"),
        ("slot-v1.12.0/System/gpsp_libretro.so", b"gpsp"),
        ("slot-v1.12.0/System/gambatte_libretro.so", b"gambatte"),
    ];
    entries.extend_from_slice(extra);
    zip_of(&entries)
}

#[test]
fn parse_release_reads_the_version_url_and_digest() {
    let r = parse_release(&v1_11(), Device::H700).unwrap();
    assert_eq!(r.version, "1.11.0");
    assert_eq!(
        r.url,
        "https://github.com/slot-cfw/slot/releases/download/v1.11.0/slot-v1.11.0-h700.zip"
    );
    assert_eq!(r.size, 6919551);
    assert_eq!(&r.sha256[..2], &[0xe7, 0x8b]);
}

#[test]
fn parse_release_refuses_a_release_without_the_h700_zip() {
    let json = release_json("v1.11.0", "slot-v1.11.0-other.zip", DIGEST);
    assert_eq!(parse_release(&json, Device::H700), Err(UpdateFail::Bad));
}

#[test]
fn parse_release_refuses_a_manifest_without_stable() {
    assert_eq!(parse_release(r#"{"beta": {}}"#, Device::H700), Err(UpdateFail::Bad));
}

#[test]
fn parse_release_refuses_a_release_with_a_short_digest() {
    let json = release_json("v1.11.0", "slot-v1.11.0-h700.zip", "e78b27");
    assert_eq!(parse_release(&json, Device::H700), Err(UpdateFail::Bad));
}

#[test]
fn newer_compares_numerically() {
    assert!(newer("1.12.0", "1.11.0"));
    assert!(newer("1.10.0", "1.9.0"));
    assert!(!newer("1.11.0", "1.11.0"));
    assert!(!newer("1.11.0", "1.12.0"));
    assert!(!newer("1.12.0-rc1", "1.11.0"));
}

#[test]
fn verify_rejects_one_flipped_byte() {
    let bytes = b"slot release".to_vec();
    let sha: [u8; 32] = {
        use sha2::Digest;
        sha2::Sha256::digest(&bytes).into()
    };
    let release = Release {
        kind: Kind::Slot,
        running: "1.11.0".into(),
        version: "1.12.0".into(),
        url: String::new(),
        sha256: sha,
        size: bytes.len() as u64,
    };
    assert_eq!(verify(&bytes, &release), Ok(()));
    let mut bad = bytes.clone();
    bad[0] ^= 1;
    assert_eq!(verify(&bad, &release), Err(UpdateFail::Checksum));
}

#[test]
fn stage_unpacks_only_system_and_writes_the_manifest_last() {
    let d = tempfile::tempdir().unwrap();
    let zip = release_zip(&[
        ("slot-v1.12.0/System/bezels/640x480.png", b"png"),
        ("slot-v1.12.0/Games/GBA/x.gba", b"rom"),
    ]);
    stage(d.path(), &zip).unwrap();
    let new = d.path().join("System.new");
    assert_eq!(
        std::fs::read(new.join("bezels/640x480.png")).unwrap(),
        b"png"
    );
    assert!(!d.path().join("Games/GBA/x.gba").exists());
    assert!(!new.join("Games").exists());
    let mut listed: Vec<String> = std::fs::read_to_string(new.join(".manifest"))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect();
    listed.sort();
    assert_eq!(
        listed,
        [
            "bezels/640x480.png",
            "gambatte_libretro.so",
            "gpsp_libretro.so",
            "launch_frontend.sh",
            "mgba_libretro.so",
            "slot"
        ]
    );
}

#[test]
fn stage_replaces_an_old_staging_folder() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("System.new")).unwrap();
    std::fs::write(d.path().join("System.new/leftover"), b"x").unwrap();
    stage(d.path(), &release_zip(&[])).unwrap();
    assert!(!d.path().join("System.new/leftover").exists());
}

#[test]
fn stage_refuses_a_zip_missing_a_core() {
    let d = tempfile::tempdir().unwrap();
    let zip = zip_of(&[
        ("slot-v1.12.0/System/slot", b"bin"),
        ("slot-v1.12.0/System/launch_frontend.sh", b"sh"),
        ("slot-v1.12.0/System/mgba_libretro.so", b"mgba"),
    ]);
    assert_eq!(stage(d.path(), &zip), Err(UpdateFail::Bad));
    assert!(!d.path().join("System.new").exists());
}

#[test]
fn ota_bundle_carries_gambatte() {
    let d = tempfile::tempdir().unwrap();
    let zip = zip_of(&[
        ("slot-v1.12.0/System/slot", b"bin"),
        ("slot-v1.12.0/System/launch_frontend.sh", b"sh"),
        ("slot-v1.12.0/System/mgba_libretro.so", b"mgba"),
        ("slot-v1.12.0/System/gpsp_libretro.so", b"gpsp"),
    ]);
    assert_eq!(stage(d.path(), &zip), Err(UpdateFail::Bad));
    stage(d.path(), &release_zip(&[])).unwrap();
    let listed = std::fs::read_to_string(d.path().join("System.new/.manifest")).unwrap();
    assert!(listed.lines().any(|l| l == "gambatte_libretro.so"));
}

#[test]
fn an_entry_outside_system_is_never_written() {
    let top = tempfile::tempdir().unwrap();
    let root = top.path().join("card");
    std::fs::create_dir_all(&root).unwrap();
    let zip = release_zip(&[("slot-v1.12.0/System/../../escape", b"x")]);
    assert_eq!(stage(&root, &zip), Err(UpdateFail::Bad));
    assert!(!root.join("escape").exists());
    assert!(!top.path().join("escape").exists());
    assert!(!root.join("System.new").exists());
}

#[test]
fn clock_is_wrong_before_the_build_date() {
    assert!(clock_is_wrong(0));
    assert!(!clock_is_wrong(u64::MAX / 2));
}

#[test]
fn wifi_conf_is_seen_only_when_present() {
    let d = tempfile::tempdir().unwrap();
    assert!(!wifi_conf(d.path()));
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}").unwrap();
    assert!(wifi_conf(d.path()));
}

#[test]
fn a_check_on_a_wrong_clock_fails_without_connecting() {
    let mut fetch = |_: &str| -> Result<Vec<u8>, UpdateFail> { panic!("connected") };
    assert_eq!(
        check_with(0, &mut fetch, slot_only("1.10.0")),
        UpdateEvent::Failed(UpdateFail::Clock)
    );
}

#[test]
fn a_check_reports_a_newer_release() {
    let now = u64::MAX / 2;
    let mut fetch = |_: &str| Ok(v1_11().into_bytes());
    match check_with(now, &mut fetch, slot_only("1.10.0")) {
        UpdateEvent::Available(v) => {
            assert_eq!(v.len(), 1);
            assert_eq!(v[0].version, "1.11.0");
            assert_eq!(v[0].running, "1.10.0");
        }
        other => panic!("got {other:?}"),
    }
    assert_eq!(
        check_with(now, &mut fetch, slot_only("1.11.0")),
        UpdateEvent::UpToDate
    );
}

#[test]
fn a_check_that_cannot_connect_says_offline() {
    let mut fetch = |_: &str| Err(UpdateFail::Offline);
    assert_eq!(
        check_with(u64::MAX / 2, &mut fetch, slot_only("1.10.0")),
        UpdateEvent::Failed(UpdateFail::Offline)
    );
}

fn release_for(bytes: &[u8]) -> Release {
    use sha2::Digest;
    Release {
        kind: Kind::Slot,
        running: "1.11.0".into(),
        version: "1.12.0".into(),
        url: "https://example.invalid/slot.zip".into(),
        sha256: sha2::Sha256::digest(bytes).into(),
        size: bytes.len() as u64,
    }
}

#[test]
fn an_install_stages_a_verified_download() {
    let d = tempfile::tempdir().unwrap();
    let zip = release_zip(&[]);
    let release = release_for(&zip);
    let mut fetch = |_: &str| Ok(zip.clone());
    assert_eq!(
        install_with(d.path(), &mut fetch, &release),
        UpdateEvent::Staged
    );
    assert!(d.path().join("System.new/.manifest").is_file());
}

#[test]
fn a_short_download_fails_and_stages_nothing() {
    let d = tempfile::tempdir().unwrap();
    let zip = release_zip(&[]);
    let release = release_for(&zip);
    let mut fetch = |_: &str| Ok(zip[..zip.len() - 1].to_vec());
    assert_eq!(
        install_with(d.path(), &mut fetch, &release),
        UpdateEvent::Failed(UpdateFail::Download)
    );
    assert!(!d.path().join("System.new").exists());
}

#[test]
fn a_damaged_download_fails_and_stages_nothing() {
    let d = tempfile::tempdir().unwrap();
    let zip = release_zip(&[]);
    let release = release_for(&zip);
    let mut bad = zip.clone();
    let last = bad.len() - 1;
    bad[last] ^= 1;
    let mut fetch = |_: &str| Ok(bad.clone());
    assert_eq!(
        install_with(d.path(), &mut fetch, &release),
        UpdateEvent::Failed(UpdateFail::Checksum)
    );
    assert!(!d.path().join("System.new").exists());
}

#[test]
fn a_counted_read_reports_every_chunk_up_to_the_whole_body() {
    let body: Vec<u8> = (0..200_000u32).map(|i| i as u8).collect();
    let mut seen = Vec::new();
    let got = read_counted(&mut Cursor::new(body.clone()), 1 << 20, &mut |n| {
        seen.push(n)
    })
    .unwrap();
    assert_eq!(got, body);
    assert!(seen.len() > 1, "only {} reports", seen.len());
    assert!(seen.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(seen.last(), Some(&200_000));
}

#[test]
fn a_counted_read_past_the_limit_fails_as_a_download() {
    let body = vec![0u8; 5000];
    assert_eq!(
        read_counted(&mut Cursor::new(body), 4096, &mut |_| {}),
        Err(UpdateFail::Download)
    );
}

const NAME: &str = "baseos-rgsp-1.4.0.bosupd";

fn status_of(blocks: &[(&str, &str)]) -> String {
    let mut s = String::from("version: 1.3.0 (b)\nslot:    a\ntrial:   none (committed)\n");
    for (name, verdict) in blocks {
        s.push_str(&format!(
            "payload: /mnt/sdcard/{name}\n         rgsp 1.4.0 (b)\n         {verdict}\n"
        ));
    }
    s
}

#[test]
fn will_apply_reads_only_our_block() {
    let other = "baseos-rgsp-1.9.0.bosupd";
    let ours_skipped = status_of(&[
        (other, "WILL APPLY on the next boot"),
        (NAME, "skipped: not newer than 1.5.2"),
    ]);
    assert_eq!(
        will_apply(&ours_skipped, NAME),
        Err("not newer than 1.5.2".to_string())
    );
    let ours_applies = status_of(&[
        (other, "skipped: not newer than 1.5.2"),
        (NAME, "WILL APPLY on the next boot"),
    ]);
    assert_eq!(will_apply(&ours_applies, NAME), Ok(()));
}

#[test]
fn will_apply_without_our_payload_is_an_error() {
    let none = "version: 1.3.0 (b)\npayload: none on the card\n";
    assert!(will_apply(none, NAME).is_err());
}

fn bosupd_release(bytes: &[u8]) -> Release {
    use sha2::Digest;
    Release {
        kind: Kind::BaseOs,
        running: "1.3.0".into(),
        version: "1.4.0".into(),
        url: format!("https://github.com/pvaibhav/BaseOS/releases/download/v1.4.0/{NAME}"),
        sha256: sha2::Sha256::digest(bytes).into(),
        size: bytes.len() as u64,
    }
}

fn install_baseos(
    root: &std::path::Path,
    body: &[u8],
    release: &Release,
    status: &str,
) -> UpdateEvent {
    let mut fetch_into = |_: &str, out: &mut dyn Write| read_into(body, out);
    let mut status = || Some(status.to_string());
    install_baseos_with(root, u64::MAX, &mut fetch_into, &mut status, release)
}

fn read_into(body: &[u8], out: &mut dyn Write) -> Result<[u8; 32], UpdateFail> {
    slot::update::copy_counted(&mut Cursor::new(body.to_vec()), out, u64::MAX, &mut |_| {})
}

#[test]
fn a_baseos_install_leaves_only_the_verified_file() {
    let d = tempfile::tempdir().unwrap();
    let body = b"bosupd payload".to_vec();
    let release = bosupd_release(&body);
    let status = status_of(&[(NAME, "WILL APPLY on the next boot")]);
    assert_eq!(
        install_baseos(d.path(), &body, &release, &status),
        UpdateEvent::Staged
    );
    assert_eq!(std::fs::read(d.path().join(NAME)).unwrap(), body);
    assert!(!d.path().join(format!("{NAME}.part")).exists());
}

fn left_nothing(d: &tempfile::TempDir) -> bool {
    !d.path().join(NAME).exists() && !d.path().join(format!("{NAME}.part")).exists()
}

#[test]
fn a_damaged_baseos_download_leaves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let body = b"bosupd payload".to_vec();
    let release = bosupd_release(&body);
    let mut bad = body.clone();
    bad[0] ^= 1;
    let status = status_of(&[(NAME, "WILL APPLY on the next boot")]);
    assert_eq!(
        install_baseos(d.path(), &bad, &release, &status),
        UpdateEvent::Failed(UpdateFail::Checksum)
    );
    assert!(left_nothing(&d));
}

#[test]
fn a_short_baseos_download_leaves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let body = b"bosupd payload".to_vec();
    let release = bosupd_release(&body);
    let status = status_of(&[(NAME, "WILL APPLY on the next boot")]);
    assert_eq!(
        install_baseos(d.path(), &body[..body.len() - 1], &release, &status),
        UpdateEvent::Failed(UpdateFail::Download)
    );
    assert!(left_nothing(&d));
}

#[test]
fn a_refused_baseos_update_is_deleted_with_its_reason() {
    let d = tempfile::tempdir().unwrap();
    let body = b"bosupd payload".to_vec();
    let release = bosupd_release(&body);
    let status = status_of(&[(NAME, "rejected: geometry does not fit")]);
    assert_eq!(
        install_baseos(d.path(), &body, &release, &status),
        UpdateEvent::Failed(UpdateFail::Rejected("geometry does not fit".into()))
    );
    assert!(left_nothing(&d));
}

#[test]
fn a_card_too_full_fails_before_downloading() {
    let d = tempfile::tempdir().unwrap();
    let release = bosupd_release(b"bosupd payload");
    let mut fetch_into =
        |_: &str, _: &mut dyn Write| -> Result<[u8; 32], UpdateFail> { panic!("downloaded") };
    let mut status = || None;
    assert_eq!(
        install_baseos_with(
            d.path(),
            release.size - 1,
            &mut fetch_into,
            &mut status,
            &release
        ),
        UpdateEvent::Failed(UpdateFail::Space)
    );
    assert!(left_nothing(&d));
}

#[test]
fn clean_baseos_removes_only_our_stale_files() {
    let d = tempfile::tempdir().unwrap();
    let gone = [
        "baseos-rgsp-1.3.0.bosupd",
        "baseos-rgsp-1.4.0.bosupd",
        "baseos-rgsp-1.5.0.bosupd.part",
    ];
    let kept = [
        "baseos-rgsp-1.5.0.bosupd",
        "baseos-rg35xxsp-1.3.0.bosupd",
        "notes.txt",
    ];
    for name in gone.iter().chain(&kept) {
        std::fs::write(d.path().join(name), b"x").unwrap();
    }
    clean_baseos(d.path(), "rgsp", "1.4.0");
    for name in gone {
        assert!(!d.path().join(name).exists(), "{name} kept");
    }
    for name in kept {
        assert!(d.path().join(name).exists(), "{name} deleted");
    }
}

fn ranged(
    body: &[u8],
    chunk: u64,
    short_at: Option<u64>,
) -> (
    Result<[u8; 32], UpdateFail>,
    Vec<u8>,
    Vec<(u64, u64)>,
    Vec<u64>,
) {
    let mut asked = Vec::new();
    let mut out = Vec::new();
    let mut seen = Vec::new();
    let result = slot::update::fetch_ranged(
        &mut |from, to| {
            asked.push((from, to));
            let end = match short_at {
                Some(at) if at == from => to,
                _ => to + 1,
            };
            Ok(
                Box::new(Cursor::new(body[from as usize..end as usize].to_vec()))
                    as Box<dyn std::io::Read>,
            )
        },
        body.len() as u64,
        chunk,
        &mut out,
        &mut |got| seen.push(got),
    );
    (result, out, asked, seen)
}

#[test]
fn a_ranged_download_asks_for_each_chunk_and_hashes_the_whole() {
    use sha2::Digest;
    let body: Vec<u8> = (0..10u8).collect();
    let (result, out, asked, seen) = ranged(&body, 4, None);
    assert_eq!(result, Ok(sha2::Sha256::digest(&body).into()));
    assert_eq!(out, body);
    assert_eq!(asked, [(0, 3), (4, 7), (8, 9)]);
    assert_eq!(seen.last(), Some(&10));
}

#[test]
fn a_short_range_fails_the_download() {
    let body: Vec<u8> = (0..10u8).collect();
    let (result, _, asked, _) = ranged(&body, 4, Some(4));
    assert_eq!(result, Err(UpdateFail::Download));
    assert_eq!(asked, [(0, 3), (4, 7)]);
}

#[test]
fn a_ticket_cancels_until_it_commits() {
    let t = Ticket::default();
    assert!(t.cancel());
    assert!(!t.commit());
    assert!(t.cancelled());
}

#[test]
fn a_committed_ticket_cannot_be_cancelled() {
    let t = Ticket::default();
    assert!(t.commit());
    assert!(!t.cancel());
    assert!(!t.cancelled());
}

#[test]
fn a_cancelled_ticket_stops_the_download_reader() {
    let t = Ticket::default();
    let mut r = Stoppable::new(Cursor::new(vec![7u8; 10]), t.clone());
    let mut buf = [0u8; 4];
    assert_eq!(r.read(&mut buf).unwrap(), 4);
    t.cancel();
    assert!(r.read(&mut buf).is_err());
}

#[test]
fn a_cancelled_install_reports_cancelled_not_its_failure() {
    let t = Ticket::default();
    t.cancel();
    assert_eq!(
        t.settle(UpdateEvent::Failed(UpdateFail::Download)),
        UpdateEvent::Cancelled
    );
    let t = Ticket::default();
    assert_eq!(
        t.settle(UpdateEvent::Failed(UpdateFail::Download)),
        UpdateEvent::Failed(UpdateFail::Download)
    );
}

const DS_DIGEST: &str = "aa8b27c0e8d3f1b2a4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b";

fn both_zips() -> String {
    format!(
        r#"{{
  "stable": {{
    "version": "1.11.0",
    "tag": "v1.11.0",
    "zip": "slot-v1.11.0-h700.zip",
    "size": 6919551,
    "sha256": "{DIGEST}",
    "rgdsplus": {{
      "zip": "slot-v1.11.0-rgdsplus.zip",
      "size": 8000000,
      "sha256": "{DS_DIGEST}"
    }}
  }}
}}"#
    )
}

#[test]
fn parse_release_gives_the_ds_plus_its_own_zip() {
    let r = parse_release(&both_zips(), Device::RgDsPlus).unwrap();
    assert_eq!(r.version, "1.11.0");
    assert_eq!(
        r.url,
        "https://github.com/slot-cfw/slot/releases/download/v1.11.0/slot-v1.11.0-rgdsplus.zip"
    );
    assert_eq!(r.size, 8000000);
    assert_eq!(&r.sha256[..2], &[0xaa, 0x8b]);
}

#[test]
fn parse_release_keeps_the_h700_zip_for_the_sps() {
    let r = parse_release(&both_zips(), Device::H700).unwrap();
    assert!(r.url.ends_with("/v1.11.0/slot-v1.11.0-h700.zip"), "{}", r.url);
    assert_eq!(r.size, 6919551);
}

#[test]
fn a_ds_plus_refuses_a_manifest_without_its_zip() {
    assert_eq!(parse_release(&v1_11(), Device::RgDsPlus), Err(UpdateFail::Bad));
}

#[test]
fn a_ds_plus_refuses_a_zip_named_for_another_device() {
    let json = both_zips().replace("slot-v1.11.0-rgdsplus.zip", "slot-v1.11.0-h700.zip");
    assert_eq!(parse_release(&json, Device::RgDsPlus), Err(UpdateFail::Bad));
}

#[test]
fn device_of_target_reads_rgdsplus() {
    assert_eq!(Device::of_target("rgdsplus"), Device::RgDsPlus);
    assert_eq!(Device::of_target("rg35xxsp"), Device::H700);
    assert_eq!(Device::of_target(""), Device::H700);
}

#[test]
fn a_check_on_the_ds_plus_offers_the_ds_plus_zip() {
    let json = both_zips();
    let mut fetch = |_: &str| Ok(json.clone().into_bytes());
    let event = check_with(
        u64::MAX / 2,
        &mut fetch,
        Running {
            slot: "1.10.0",
            baseos: Some(("rgdsplus", "0.0.1")),
        },
    );
    let UpdateEvent::Available(offers) = event else {
        panic!("no offer: {event:?}");
    };
    assert!(offers[0].url.ends_with("slot-v1.11.0-rgdsplus.zip"), "{}", offers[0].url);
}
