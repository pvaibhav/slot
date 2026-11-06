mod common;

use common::tmp_root;
use slot_store::{parse_stamp, read_slot_state, write_slot_state, SlotState, Zone};

fn at(stamp: &str) -> i64 {
    parse_stamp(stamp).expect("a stamp")
}

fn zone(name: &str) -> Zone {
    Zone::ALL
        .into_iter()
        .find(|z| z.name() == name)
        .unwrap_or_else(|| panic!("no {name} on the list"))
}

#[test]
fn new_york_moves_its_clocks_on_the_us_sundays() {
    let ny = zone("New York");
    assert_eq!(ny.offset_min(at("2026-01-15_12-00-00")), -300);
    assert_eq!(ny.offset_min(at("2026-03-08_06-59-59")), -300);
    assert_eq!(ny.offset_min(at("2026-03-08_07-00-00")), -240);
    assert_eq!(ny.offset_min(at("2026-11-01_05-59-59")), -240);
    assert_eq!(ny.offset_min(at("2026-11-01_06-00-00")), -300);
}

#[test]
fn london_moves_its_clocks_at_one_in_the_morning_utc_on_the_last_sundays() {
    let london = zone("London");
    assert_eq!(london.offset_min(at("2026-03-29_00-59-59")), 0);
    assert_eq!(london.offset_min(at("2026-03-29_01-00-00")), 60);
    assert_eq!(london.offset_min(at("2026-10-25_00-59-59")), 60);
    assert_eq!(london.offset_min(at("2026-10-25_01-00-00")), 0);
}

#[test]
fn sydney_has_its_summer_over_the_new_year() {
    let sydney = zone("Sydney");
    assert_eq!(sydney.offset_min(at("2026-01-15_00-00-00")), 660);
    assert_eq!(sydney.offset_min(at("2026-04-04_15-59-59")), 660);
    assert_eq!(sydney.offset_min(at("2026-04-04_16-00-00")), 600);
    assert_eq!(sydney.offset_min(at("2026-10-03_15-59-59")), 600);
    assert_eq!(sydney.offset_min(at("2026-10-03_16-00-00")), 660);
}

#[test]
fn auckland_starts_its_summer_on_the_last_sunday_of_september() {
    let auckland = zone("Auckland");
    assert_eq!(auckland.offset_min(at("2026-04-04_13-59-59")), 780);
    assert_eq!(auckland.offset_min(at("2026-04-04_14-00-00")), 720);
    assert_eq!(auckland.offset_min(at("2026-09-26_13-59-59")), 720);
    assert_eq!(auckland.offset_min(at("2026-09-26_14-00-00")), 780);
}

#[test]
fn a_city_without_summer_time_keeps_one_offset_all_year() {
    let tokyo = zone("Tokyo");
    assert_eq!(tokyo.offset_min(at("2026-01-15_00-00-00")), 540);
    assert_eq!(tokyo.offset_min(at("2026-07-15_00-00-00")), 540);
}

#[test]
fn a_local_time_turns_back_into_the_utc_it_came_from() {
    let ny = zone("New York");
    for stamp in ["2026-01-15_12-00-00", "2026-07-04_23-30-00"] {
        let utc = at(stamp);
        assert_eq!(ny.utc(ny.local(utc)), utc, "{stamp}");
    }
}

#[test]
fn the_cities_run_west_to_east_and_each_reads_back_from_its_id() {
    let winter = at("2026-01-15_00-00-00");
    let mut last = i64::MIN;
    for z in Zone::ALL {
        let std = z
            .offset_min(winter)
            .min(z.offset_min(at("2026-07-15_00-00-00")));
        assert!(std >= last, "{} is out of order", z.name());
        last = std;
        assert_eq!(Zone::parse(z.id()), Some(z), "{}", z.id());
    }
    assert!(Zone::ALL.len() >= 35, "only {} cities", Zone::ALL.len());
}

#[test]
fn east_and_west_walk_the_list_and_wrap() {
    let first = Zone::ALL[0];
    let last = Zone::ALL[Zone::ALL.len() - 1];
    assert_eq!(first.west(), last);
    assert_eq!(last.east(), first);
    assert_eq!(first.east(), Zone::ALL[1]);
}

#[test]
fn the_zone_and_the_hour_clock_are_saved() {
    let d = tmp_root();
    let s = SlotState {
        zone: zone("Sydney"),
        clock_24h: false,
        ..SlotState::default()
    };
    write_slot_state(d.path(), &s).unwrap();
    let back = read_slot_state(d.path());
    assert_eq!(back.zone, zone("Sydney"));
    assert!(!back.clock_24h);
}

#[test]
fn a_fresh_card_keeps_utc_on_a_24_hour_clock() {
    let s = SlotState::default();
    assert_eq!(s.zone.name(), "UTC");
    assert!(s.clock_24h);
}

#[test]
fn an_old_offset_becomes_a_city_that_shows_the_same_time_now() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    for offset in [-300, -240, 0, 60, 330, 600] {
        let d = tmp_root();
        std::fs::write(
            d.path().join("Config").join("slot.state"),
            format!("cart=\nbrightness=5\nblue_light=0\nvolume=60\nmuted=0\nclock_set=1\nutc_offset_min={offset}\n"),
        )
        .unwrap();
        let s = read_slot_state(d.path());
        assert!(s.clock_set, "{offset}: the old file was not read");
        assert_eq!(
            s.zone.offset_min(now),
            offset,
            "{offset}: became {}, which shows a different time",
            s.zone.name()
        );
    }
}

extern "C" {
    fn tzset();
}

fn libc_offset_min(utc: i64) -> i64 {
    let t = utc as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    tm.tm_gmtoff as i64 / 60
}

#[test]
fn libc_reads_every_city_the_way_slot_does() {
    let start = at("2026-01-01_00-00-00");
    let end = at("2028-01-01_00-00-00");
    for z in Zone::ALL {
        std::env::set_var("TZ", z.posix());
        unsafe { tzset() };
        let mut utc = start;
        while utc < end {
            assert_eq!(
                libc_offset_min(utc),
                z.offset_min(utc),
                "{} ({}) at {utc}",
                z.name(),
                z.posix()
            );
            utc += 900;
        }
    }
}

#[test]
fn new_york_is_five_hours_behind_in_winter_for_libc() {
    assert_eq!(zone("New York").posix(), "<-05>5<-04>,M3.2.0,M11.1.0");
}
