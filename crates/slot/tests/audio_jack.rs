use slot::audio::{follows_jack, Jack};

#[test]
fn starts_unplugged_and_unchanged() {
    let jack = Jack::default();
    assert!(!jack.plugged());
    assert!(!jack.take_change());
}

#[test]
fn plugging_in_reports_one_change() {
    let jack = Jack::default();
    jack.set(true);
    assert!(jack.plugged());
    assert!(jack.take_change());
    assert!(!jack.take_change());
}

#[test]
fn the_same_state_again_is_not_a_change() {
    let jack = Jack::default();
    jack.set(true);
    jack.take_change();
    jack.set(true);
    assert!(!jack.take_change());
}

#[test]
fn unplugging_reports_a_change() {
    let jack = Jack::default();
    jack.set(true);
    jack.take_change();
    jack.set(false);
    assert!(!jack.plugged());
    assert!(jack.take_change());
}

#[test]
fn only_dsos_reopens_audio_for_the_jack() {
    assert!(follows_jack(Some("1")));
    assert!(!follows_jack(None));
    assert!(!follows_jack(Some("0")));
    assert!(!follows_jack(Some("")));
}

#[test]
fn the_route_watch_sees_dsos_replace_its_audio_config() {
    use slot::audio::RouteWatch;
    let d = tempfile::tempdir().unwrap();
    let conf = d.path().join("asound.conf");
    std::fs::write(&conf, "speaker").unwrap();
    let paths = format!("/usr/share/alsa/alsa.conf:{}", conf.display());
    let mut watch = RouteWatch::new(Some(&paths));
    assert!(!watch.changed(), "the first look is not a change");
    assert!(!watch.changed());
    let next = d.path().join("asound.conf.new");
    std::fs::write(&next, "headphones").unwrap();
    std::fs::rename(&next, &conf).unwrap();
    assert!(watch.changed(), "a replaced config went unnoticed");
    assert!(!watch.changed());
}

#[test]
fn no_config_path_never_changes() {
    let mut watch = slot::audio::RouteWatch::new(None);
    assert!(!watch.changed());
}
