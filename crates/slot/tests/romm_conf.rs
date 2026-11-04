use slot::romm::{Protocol, RommConf, ROMM_CONF};

fn root() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn full() -> RommConf {
    RommConf {
        protocol: Protocol::Http,
        server: "nas.local".into(),
        port: Some(8080),
        self_signed: true,
        client_id: "abc123".into(),
        device_id: Some("dev-1".into()),
        token: Some("rmm_secret".into()),
    }
}

#[test]
fn round_trips_every_field() {
    let dir = root();
    full().save(dir.path()).unwrap();
    assert_eq!(RommConf::load(dir.path()), full());
}

#[test]
fn missing_file_is_a_default_conf() {
    let dir = root();
    let conf = RommConf::load(dir.path());
    assert_eq!(conf, RommConf::default());
    assert_eq!(conf.protocol, Protocol::Https);
    assert!(!conf.paired());
}

#[test]
fn origin_omits_a_blank_port() {
    let conf = RommConf {
        server: "romm.example.com".into(),
        ..RommConf::default()
    };
    assert_eq!(conf.origin(), "https://romm.example.com");
}

#[test]
fn origin_keeps_a_port() {
    let conf = RommConf {
        protocol: Protocol::Http,
        server: "nas.local".into(),
        port: Some(8080),
        ..RommConf::default()
    };
    assert_eq!(conf.origin(), "http://nas.local:8080");
}

#[test]
fn origin_strips_a_typed_scheme_slashes_and_path() {
    for typed in [
        "https://romm.example.com/",
        "HTTP://romm.example.com",
        "romm.example.com///",
        "  romm.example.com/library/roms ",
        "https://romm.example.com/?a=1",
    ] {
        let conf = RommConf {
            server: typed.into(),
            ..RommConf::default()
        };
        assert_eq!(conf.origin(), "https://romm.example.com", "{typed}");
    }
    let conf = RommConf {
        server: "http://nas.local/".into(),
        port: Some(8080),
        ..RommConf::default()
    };
    assert_eq!(conf.origin(), "https://nas.local:8080");
}

#[test]
fn unpair_keeps_the_connection_settings() {
    let mut conf = full();
    assert!(conf.paired());
    conf.unpair();
    assert!(!conf.paired());
    assert_eq!(conf.device_id, None);
    assert_eq!(conf.token, None);
    assert_eq!(conf.server, "nas.local");
    assert_eq!(conf.port, Some(8080));
    assert!(conf.self_signed);
    assert_eq!(conf.client_id, "abc123");
}

#[test]
fn client_id_stays_stable_once_written() {
    let dir = root();
    let conf = RommConf {
        server: "romm.example.com".into(),
        ..RommConf::default()
    };
    conf.save(dir.path()).unwrap();
    let first = RommConf::load(dir.path()).client_id;
    assert_eq!(first.len(), 32);
    assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
    conf.save(dir.path()).unwrap();
    assert_eq!(RommConf::load(dir.path()).client_id, first);
    let mut edited = RommConf::load(dir.path());
    edited.server = "other".into();
    edited.save(dir.path()).unwrap();
    assert_eq!(RommConf::load(dir.path()).client_id, first);
    assert!(dir.path().join(ROMM_CONF).exists());
}
