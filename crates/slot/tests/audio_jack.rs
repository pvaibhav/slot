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
