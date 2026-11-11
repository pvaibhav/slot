mod common;

use common::{app_booting_at, app_booting_with_clock, tmp_root_with_carts, Clock};
use slot::app::{App, Phase};
use slot_input::{Action, Btn};
use slot_store::{parse_stamp, read_slot_state, write_slot_state, SlotState, Zone};
use slot_ui::{ClockPicker, ClockRow, Part, QuickRow};
use tempfile::TempDir;

#[test]
fn the_clock_is_asked_for_once_and_only_once() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let a = App::boot(d.path());
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "not asked on first launch"
    );

    let mut a = App::boot(d.path());
    a.confirm_clock();
    assert!(read_slot_state(d.path()).clock_set);
    let again = App::boot(d.path());
    assert!(
        !matches!(again.phase(), Phase::SetClock { .. }),
        "asked twice"
    );
}

#[test]
fn the_clock_comes_before_a_seated_cart() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Emerald".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let a = App::boot(d.path());
    assert!(matches!(a.phase(), Phase::SetClock { .. }));
}

#[test]
fn confirming_writes_the_picked_time_to_the_platform() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, clock) = app_booting_with_clock(d.path());
    a.apply(Action::GbaDown(Btn::Up));
    a.confirm_clock();
    assert_ne!(clock.get(), 0, "the platform clock was never set");
}

#[test]
fn the_picker_starts_from_the_clock_the_platform_already_has() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, clock) = app_booting_at(d.path(), 1_700_000_000);
    a.confirm_clock();
    assert_eq!(
        clock.get(),
        1_700_000_000,
        "confirming the clock as it stands moved it"
    );
}

fn at(stamp: &str) -> i64 {
    parse_stamp(stamp).expect("a stamp")
}

fn city(name: &str) -> Zone {
    Zone::ALL
        .into_iter()
        .find(|z| z.name() == name)
        .unwrap_or_else(|| panic!("no {name} on the list"))
}

fn on_row(c: &mut ClockPicker, row: ClockRow) {
    while c.row() != row {
        c.down();
    }
}

fn open(c: &mut ClockPicker, row: ClockRow, part: Part) {
    on_row(c, row);
    c.a();
    while c.part() != Some(part) {
        c.right();
    }
}

#[test]
fn the_rows_read_as_a_date_a_time_a_city_and_the_clock() {
    let c = ClockPicker::new(at("2026-10-27_18-05-00"), city("New York"), false);
    assert_eq!(c.value_text(ClockRow::Date), "OCT 27 2026");
    assert_eq!(c.value_text(ClockRow::Time), "2:05 PM");
    assert_eq!(c.value_text(ClockRow::Zone), "New York");
    assert_eq!(c.offset_text(), "UTC-04:00");
    assert_eq!(c.value_text(ClockRow::Clock), "12-Hour");
    let c = ClockPicker::new(at("2026-10-27_18-05-00"), city("New York"), true);
    assert_eq!(c.value_text(ClockRow::Time), "14:05");
    assert_eq!(c.value_text(ClockRow::Clock), "24-Hour");
}

#[test]
fn up_and_down_walk_the_rows_and_wrap() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    assert_eq!(c.row(), ClockRow::Date);
    c.up();
    assert_eq!(c.row(), ClockRow::Clock);
    c.down();
    c.down();
    assert_eq!(c.row(), ClockRow::Time);
}

#[test]
fn a_opens_the_date_and_left_and_right_walk_its_parts() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    c.a();
    assert_eq!(c.part(), Some(Part::Month));
    c.right();
    assert_eq!(c.part(), Some(Part::Day));
    c.right();
    c.right();
    assert_eq!(c.part(), Some(Part::Year), "walked off the end");
    c.up();
    assert_eq!(c.value_text(ClockRow::Date), "AUG 12 2027");
    c.left();
    c.left();
    c.left();
    assert_eq!(c.part(), Some(Part::Month), "walked off the start");
}

#[test]
fn a_closes_an_open_row_keeping_the_change() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    open(&mut c, ClockRow::Date, Part::Day);
    c.up();
    c.a();
    assert_eq!(c.part(), None);
    assert_eq!(c.value_text(ClockRow::Date), "AUG 13 2026");
}

#[test]
fn a_on_the_clock_opens_nothing() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    on_row(&mut c, ClockRow::Clock);
    c.a();
    assert_eq!(c.part(), None);
    assert!(!c.editing());
}

#[test]
fn a_on_the_city_opens_the_list_at_the_city_already_picked() {
    let mut c = ClockPicker::new(at("2026-08-12_12-00-00"), city("New York"), true);
    on_row(&mut c, ClockRow::Zone);
    c.a();
    assert_eq!(c.highlighted(), Some(city("New York")));
}

#[test]
fn up_and_down_move_one_city_and_left_and_right_skip_a_page() {
    let mut c = ClockPicker::new(at("2026-08-12_12-00-00"), city("New York"), true);
    on_row(&mut c, ClockRow::Zone);
    c.a();
    c.down();
    assert_eq!(c.highlighted(), Some(city("New York").east()));
    c.up();
    c.right();
    let page = Zone::ALL
        .iter()
        .position(|z| *z == city("New York"))
        .unwrap()
        + 6;
    assert_eq!(c.highlighted(), Some(Zone::ALL[page]));
    c.left();
    assert_eq!(c.highlighted(), Some(city("New York")));
}

#[test]
fn a_page_skip_stops_at_the_ends_of_the_list() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    on_row(&mut c, ClockRow::Zone);
    c.a();
    for _ in 0..20 {
        c.right();
    }
    assert_eq!(c.highlighted(), Some(Zone::ALL[Zone::ALL.len() - 1]));
    for _ in 0..20 {
        c.left();
    }
    assert_eq!(c.highlighted(), Some(Zone::ALL[0]));
}

#[test]
fn a_picks_the_highlighted_city_and_b_keeps_it_too() {
    for (key, name) in [("A", "a"), ("B", "b")] {
        let mut c = ClockPicker::new(at("2026-08-12_12-00-00"), city("New York"), true);
        on_row(&mut c, ClockRow::Zone);
        c.a();
        c.down();
        let handled = match name {
            "a" => {
                c.a();
                true
            }
            _ => c.b(),
        };
        assert!(handled, "{key} left the clock from the list");
        assert_eq!(c.highlighted(), None, "{key} did not close the list");
        assert_eq!(c.zone(), city("New York").east(), "{key} lost the pick");
        assert_eq!(c.row(), ClockRow::Zone);
    }
}

#[test]
fn b_away_from_the_list_is_left_to_the_screen() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    assert!(!c.b());
}

#[test]
fn left_and_right_on_the_closed_city_row_change_nothing() {
    let mut c = ClockPicker::new(at("2026-08-12_12-00-00"), city("New York"), true);
    on_row(&mut c, ClockRow::Zone);
    c.right();
    c.left();
    c.left();
    assert_eq!(c.zone(), city("New York"));
}

#[test]
fn left_and_right_on_the_clock_switch_between_12_and_24_hours() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    assert!(c.clock_24h());
    on_row(&mut c, ClockRow::Clock);
    c.right();
    assert!(!c.clock_24h());
    c.left();
    assert!(c.clock_24h());
}

#[test]
fn a_12_hour_time_has_an_am_and_pm_part() {
    let mut c = ClockPicker::new(at("2026-08-12_14-05-00"), Zone::default(), false);
    open(&mut c, ClockRow::Time, Part::Meridiem);
    c.up();
    assert_eq!(c.value_text(ClockRow::Time), "2:05 AM");
    c.left();
    c.left();
    assert_eq!(c.part(), Some(Part::Hour));
    for _ in 0..10 {
        c.up();
    }
    assert_eq!(c.value_text(ClockRow::Time), "12:05 AM");
    assert_eq!(c.secs(), at("2026-08-12_00-05-00"));
}

#[test]
fn a_24_hour_time_has_no_am_and_pm_part() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    open(&mut c, ClockRow::Time, Part::Minute);
    c.right();
    assert_eq!(c.part(), Some(Part::Minute));
}

#[test]
fn a_part_wraps_rather_than_running_off_the_end() {
    let mut c = ClockPicker::from_ymd(2026, 8, 12);
    open(&mut c, ClockRow::Date, Part::Month);
    for _ in 0..13 {
        c.up();
    }
    assert!(
        (1..=12).contains(&c.month()),
        "month {} left the calendar",
        c.month()
    );
}

#[test]
fn february_29_is_reachable_in_a_leap_year_and_not_otherwise() {
    let mut c = ClockPicker::from_ymd(2028, 2, 28);
    open(&mut c, ClockRow::Date, Part::Day);
    c.up();
    assert_eq!(c.day(), 29, "2028 is a leap year");
    let mut c = ClockPicker::from_ymd(2027, 2, 28);
    open(&mut c, ClockRow::Date, Part::Day);
    c.up();
    assert_eq!(c.day(), 1, "2027 has no 29th of February");
}

#[test]
fn a_day_the_new_month_does_not_have_is_pulled_back() {
    let mut c = ClockPicker::from_ymd(2026, 1, 31);
    open(&mut c, ClockRow::Date, Part::Month);
    c.up();
    assert_eq!((c.month(), c.day()), (2, 28));
}

#[test]
fn the_picker_hands_back_utc_for_the_time_typed_in_the_city() {
    let c = ClockPicker::new(at("2026-07-04_13-00-00"), city("New York"), true);
    assert_eq!(c.value_text(ClockRow::Time), "09:00");
    assert_eq!(c.secs(), at("2026-07-04_13-00-00"));
}

#[test]
fn the_offset_shown_is_the_one_on_the_date_typed() {
    let mut c = ClockPicker::new(at("2026-07-04_13-00-00"), city("New York"), true);
    assert_eq!(c.offset_text(), "UTC-04:00");
    open(&mut c, ClockRow::Date, Part::Month);
    for _ in 0..6 {
        c.down();
    }
    assert_eq!(c.value_text(ClockRow::Date), "JAN 4 2026");
    assert_eq!(c.offset_text(), "UTC-05:00");
    assert_eq!(c.secs(), at("2026-01-04_14-00-00"));
}

#[test]
fn confirming_saves_the_city_and_the_hour_clock_and_sets_the_platform_to_utc() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, clock) = app_booting_at(d.path(), 1_700_000_000);
    for _ in 0..2 {
        a.apply(Action::GbaDown(Btn::Down));
    }
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Right));
    a.confirm_clock();
    let s = read_slot_state(d.path());
    assert_eq!(s.zone, Zone::default().east());
    assert!(!s.clock_24h, "the 12-hour clock was not saved");
    assert_eq!(
        clock.get(),
        1_700_000_000 - s.zone.offset_min(1_700_000_000) * 60,
        "the platform was set to local rather than to utc"
    );
}

#[test]
fn the_wall_clock_is_local_to_the_city_and_follows_its_summer_time() {
    let d = tmp_root_with_carts(&["Emerald"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            zone: city("New York"),
            ..SlotState::default()
        },
    )
    .unwrap();
    let summer = at("2026-07-04_13-00-00");
    let (a, clock) = app_booting_at(d.path(), summer);
    assert_eq!(a.wall_secs(), summer - 4 * 3600);
    let winter = at("2026-12-04_13-00-00");
    clock.advance(winter - summer);
    assert_eq!(a.wall_secs(), winter - 5 * 3600);
}

#[test]
fn a_clock_that_never_got_set_is_asked_for_again() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (a, _clock) = app_booting_at(d.path(), 0);
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "a 1970 clock was taken at face value: {:?}",
        a.phase()
    );
}

#[test]
fn a_clock_that_looks_like_a_real_date_is_not_asked_for_again() {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let (a, _clock) = app_booting_at(d.path(), 1_786_568_000);
    assert!(
        !matches!(a.phase(), Phase::SetClock { .. }),
        "asked again for a clock that was already right"
    );
}

const AT: i64 = 1_786_568_022;
const OPEN_FOR: i64 = 100;

#[derive(Copy, Clone, Debug)]
enum Way {
    FirstBoot,
    QuickMenu,
}

fn clock_screen(way: Way) -> (TempDir, App, Clock) {
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    if let Way::QuickMenu = way {
        write_slot_state(
            d.path(),
            &SlotState {
                clock_set: true,
                zone: city("New York"),
                ..SlotState::default()
            },
        )
        .unwrap();
    }
    let (mut a, clock) = app_booting_at(d.path(), AT);
    if let Way::QuickMenu = way {
        a.apply(Action::QuickMenu);
        for _ in 0..QuickRow::System.position(&[]) {
            a.apply(Action::GbaDown(Btn::Down));
        }
        a.apply(Action::GbaDown(Btn::A));
        for _ in 0..QuickRow::DateTime.position(&a.hidden_rows()) {
            a.apply(Action::GbaDown(Btn::Down));
        }
        a.apply(Action::GbaDown(Btn::A));
    }
    assert!(
        matches!(a.phase(), Phase::SetClock { .. }),
        "{way:?} never reached the clock: {:?}",
        a.phase()
    );
    (d, a, clock)
}

fn to_the_city(a: &mut App) {
    while a.picker().expect("on the clock").row() != ClockRow::Zone {
        a.apply(Action::GbaDown(Btn::Down));
    }
}

#[test]
fn confirming_the_clock_untouched_leaves_it_where_it_is() {
    for way in [Way::FirstBoot, Way::QuickMenu] {
        let (_d, mut a, clock) = clock_screen(way);
        clock.advance(OPEN_FOR);
        a.apply(Action::GbaDown(Btn::B));
        assert_eq!(
            clock.get(),
            AT + OPEN_FOR,
            "{way:?}: an untouched confirm moved the clock by {} s",
            clock.get() - (AT + OPEN_FOR)
        );
    }
}

#[test]
fn changing_the_minute_moves_the_clock_by_exactly_that_minute() {
    for way in [Way::FirstBoot, Way::QuickMenu] {
        let (_d, mut a, clock) = clock_screen(way);
        a.apply(Action::GbaDown(Btn::Down));
        a.apply(Action::GbaDown(Btn::A));
        a.apply(Action::GbaDown(Btn::Right));
        assert_eq!(a.picker().expect("on the clock").part(), Some(Part::Minute));
        a.apply(Action::GbaDown(Btn::Up));
        a.apply(Action::GbaDown(Btn::A));
        clock.advance(OPEN_FOR);
        a.apply(Action::GbaDown(Btn::B));
        assert_eq!(
            clock.get(),
            AT + OPEN_FOR + 60,
            "{way:?}: a minute on moved the clock by {} s",
            clock.get() - (AT + OPEN_FOR)
        );
    }
}

#[test]
fn changing_only_the_city_moves_utc_by_exactly_the_change_in_offset() {
    for way in [Way::FirstBoot, Way::QuickMenu] {
        let (d, mut a, clock) = clock_screen(way);
        let before = a.picker().expect("on the clock").zone();
        to_the_city(&mut a);
        a.apply(Action::GbaDown(Btn::A));
        a.apply(Action::GbaDown(Btn::Down));
        a.apply(Action::GbaDown(Btn::A));
        let after = before.east();
        let moved = (before.offset_min(AT) - after.offset_min(AT)) * 60;
        assert_ne!(moved, 0, "{way:?}: the next city shows the same time");
        clock.advance(OPEN_FOR);
        a.apply(Action::GbaDown(Btn::B));
        assert_eq!(
            clock.get(),
            AT + OPEN_FOR + moved,
            "{way:?}: moving a city east moved the clock by {} s",
            clock.get() - (AT + OPEN_FOR)
        );
        assert_eq!(
            read_slot_state(d.path()).zone,
            after,
            "{way:?}: the city was not saved"
        );
    }
}

#[test]
fn network_time_is_not_offered_without_a_network() {
    let p = ClockPicker::new(1_700_000_000, Zone::default(), true);
    assert_eq!(p.network_time(), None);
    assert_eq!(p.rows(), &ClockRow::ALL[1..]);
}

#[test]
fn network_time_sits_on_top_when_offered() {
    let p = ClockPicker::new(1_700_000_000, Zone::default(), true).with_network_time(Some(false));
    assert_eq!(p.rows(), &ClockRow::ALL);
    assert_eq!(p.row(), ClockRow::NetworkTime);
    assert_eq!(p.value_text(ClockRow::NetworkTime), "Off");
}

#[test]
fn turning_network_time_on_hides_the_date_and_the_time() {
    let mut p =
        ClockPicker::new(1_700_000_000, Zone::default(), true).with_network_time(Some(false));
    p.a();
    assert_eq!(p.network_time(), Some(true));
    assert_eq!(p.value_text(ClockRow::NetworkTime), "On");
    assert_eq!(
        p.rows(),
        &[ClockRow::NetworkTime, ClockRow::Zone, ClockRow::Clock]
    );
    p.down();
    assert_eq!(p.row(), ClockRow::Zone);
}

#[test]
fn left_and_right_flip_network_time_too() {
    let mut p =
        ClockPicker::new(1_700_000_000, Zone::default(), true).with_network_time(Some(true));
    p.right();
    assert_eq!(p.network_time(), Some(false));
    p.left();
    assert_eq!(p.network_time(), Some(true));
}

#[test]
fn the_network_row_is_called_set_automatically() {
    assert_eq!(ClockRow::NetworkTime.label(), "Set Automatically");
}

fn libc_offset_min(utc: i64) -> i64 {
    let t = utc as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    tm.tm_gmtoff as i64 / 60
}

#[test]
fn the_cores_read_the_cart_clock_in_the_city_on_the_card() {
    std::env::set_var("TZ", "<+13>-13");
    let d = tmp_root_with_carts(&["Boktai"]);
    write_slot_state(
        d.path(),
        &SlotState {
            clock_set: true,
            zone: city("New York"),
            ..Default::default()
        },
    )
    .unwrap();
    let _a = App::boot(d.path());
    assert_eq!(libc_offset_min(at("2026-01-15_15-50-00")), -300);
    assert_eq!(libc_offset_min(at("2026-07-15_14-50-00")), -240);
}

#[test]
fn picking_a_new_city_moves_the_cart_clock_with_it() {
    std::env::set_var("TZ", "<+13>-13");
    let d = tmp_root_with_carts(&["Boktai"]);
    let (mut a, _clock) = app_booting_at(d.path(), 1_700_000_000);
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.apply(Action::GbaDown(Btn::Down));
    a.apply(Action::GbaDown(Btn::A));
    a.confirm_clock();
    let zone = read_slot_state(d.path()).zone;
    assert_ne!(zone, Zone::default());
    assert_eq!(libc_offset_min(1_700_000_000), zone.offset_min(1_700_000_000));
}
