use crate::stamp::{civil_from_days, days_from_civil, days_in_month};

const DAY: i64 = 86_400;
const HOUR: i64 = 3600;

#[derive(Copy, Clone)]
enum Summer {
    None,
    Us,
    Eu,
    Au,
    Nz,
}

struct City {
    name: &'static str,
    id: &'static str,
    std_min: i64,
    summer: Summer,
}

const fn city(name: &'static str, id: &'static str, std_min: i64, summer: Summer) -> City {
    City {
        name,
        id,
        std_min,
        summer,
    }
}

const CITIES: [City; 50] = [
    city("Pago Pago", "Pacific/Pago_Pago", -660, Summer::None),
    city("Honolulu", "Pacific/Honolulu", -600, Summer::None),
    city("Anchorage", "America/Anchorage", -540, Summer::Us),
    city("Los Angeles", "America/Los_Angeles", -480, Summer::Us),
    city("Denver", "America/Denver", -420, Summer::Us),
    city("Phoenix", "America/Phoenix", -420, Summer::None),
    city("Chicago", "America/Chicago", -360, Summer::Us),
    city("Mexico City", "America/Mexico_City", -360, Summer::None),
    city("New York", "America/New_York", -300, Summer::Us),
    city("Bogota", "America/Bogota", -300, Summer::None),
    city("Halifax", "America/Halifax", -240, Summer::Us),
    city("Caracas", "America/Caracas", -240, Summer::None),
    city("St. John's", "America/St_Johns", -210, Summer::Us),
    city("Sao Paulo", "America/Sao_Paulo", -180, Summer::None),
    city(
        "Buenos Aires",
        "America/Argentina/Buenos_Aires",
        -180,
        Summer::None,
    ),
    city("Azores", "Atlantic/Azores", -60, Summer::Eu),
    city("UTC", "Etc/UTC", 0, Summer::None),
    city("London", "Europe/London", 0, Summer::Eu),
    city("Reykjavik", "Atlantic/Reykjavik", 0, Summer::None),
    city("Paris", "Europe/Paris", 60, Summer::Eu),
    city("Berlin", "Europe/Berlin", 60, Summer::Eu),
    city("Lagos", "Africa/Lagos", 60, Summer::None),
    city("Athens", "Europe/Athens", 120, Summer::Eu),
    city("Johannesburg", "Africa/Johannesburg", 120, Summer::None),
    city("Istanbul", "Europe/Istanbul", 180, Summer::None),
    city("Moscow", "Europe/Moscow", 180, Summer::None),
    city("Nairobi", "Africa/Nairobi", 180, Summer::None),
    city("Tehran", "Asia/Tehran", 210, Summer::None),
    city("Dubai", "Asia/Dubai", 240, Summer::None),
    city("Kabul", "Asia/Kabul", 270, Summer::None),
    city("Karachi", "Asia/Karachi", 300, Summer::None),
    city("Delhi", "Asia/Kolkata", 330, Summer::None),
    city("Kathmandu", "Asia/Kathmandu", 345, Summer::None),
    city("Dhaka", "Asia/Dhaka", 360, Summer::None),
    city("Yangon", "Asia/Yangon", 390, Summer::None),
    city("Bangkok", "Asia/Bangkok", 420, Summer::None),
    city("Jakarta", "Asia/Jakarta", 420, Summer::None),
    city("Singapore", "Asia/Singapore", 480, Summer::None),
    city("Hong Kong", "Asia/Hong_Kong", 480, Summer::None),
    city("Beijing", "Asia/Shanghai", 480, Summer::None),
    city("Perth", "Australia/Perth", 480, Summer::None),
    city("Tokyo", "Asia/Tokyo", 540, Summer::None),
    city("Seoul", "Asia/Seoul", 540, Summer::None),
    city("Adelaide", "Australia/Adelaide", 570, Summer::Au),
    city("Darwin", "Australia/Darwin", 570, Summer::None),
    city("Sydney", "Australia/Sydney", 600, Summer::Au),
    city("Brisbane", "Australia/Brisbane", 600, Summer::None),
    city("Auckland", "Pacific/Auckland", 720, Summer::Nz),
    city("Tonga", "Pacific/Tongatapu", 780, Summer::None),
    city("Kiritimati", "Pacific/Kiritimati", 840, Summer::None),
];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Zone(usize);

impl Default for Zone {
    fn default() -> Self {
        Zone::parse("Etc/UTC").unwrap_or(Zone(0))
    }
}

impl Zone {
    pub const ALL: [Zone; CITIES.len()] = {
        let mut all = [Zone(0); CITIES.len()];
        let mut i = 0;
        while i < all.len() {
            all[i] = Zone(i);
            i += 1;
        }
        all
    };

    pub fn parse(id: &str) -> Option<Zone> {
        CITIES.iter().position(|c| c.id == id).map(Zone)
    }

    pub fn for_offset(offset_min: i64, utc: i64) -> Zone {
        Zone::ALL
            .into_iter()
            .min_by_key(|z| (z.offset_min(utc) - offset_min).abs())
            .unwrap_or_default()
    }

    pub fn name(self) -> &'static str {
        CITIES[self.0].name
    }

    pub fn id(self) -> &'static str {
        CITIES[self.0].id
    }

    pub fn east(self) -> Zone {
        Zone((self.0 + 1) % CITIES.len())
    }

    pub fn west(self) -> Zone {
        Zone((self.0 + CITIES.len() - 1) % CITIES.len())
    }

    pub fn offset_min(self, utc: i64) -> i64 {
        let c = &CITIES[self.0];
        let summer = match c.summer {
            Summer::None => false,
            Summer::Us => us_summer(utc, c.std_min),
            Summer::Eu => eu_summer(utc, c.std_min),
            Summer::Au => {
                let y = year(utc, c.std_min);
                let ends = sunday(y, 4, 1) * DAY + 2 * HOUR - c.std_min * 60;
                let starts = sunday(y, 10, 1) * DAY + 2 * HOUR - c.std_min * 60;
                utc < ends || utc >= starts
            }
            Summer::Nz => {
                let y = year(utc, c.std_min);
                let ends = sunday(y, 4, 1) * DAY + 2 * HOUR - c.std_min * 60;
                let starts = last_sunday(y, 9) * DAY + 2 * HOUR - c.std_min * 60;
                utc < ends || utc >= starts
            }
        };
        c.std_min + if summer { 60 } else { 0 }
    }

    pub fn posix(self) -> String {
        let c = &CITIES[self.0];
        let std = c.std_min;
        let summer = std + 60;
        let rules = match c.summer {
            Summer::None => return format!("{}{}", posix_name(std), posix_offset(std)),
            Summer::Us => ",M3.2.0,M11.1.0".to_string(),
            Summer::Eu => format!(
                ",M3.5.0/{},M10.5.0/{}",
                posix_clock(60 + std),
                posix_clock(60 + summer)
            ),
            Summer::Au => ",M10.1.0,M4.1.0/3".to_string(),
            Summer::Nz => ",M9.5.0,M4.1.0/3".to_string(),
        };
        format!(
            "{}{}{}{}",
            posix_name(std),
            posix_offset(std),
            posix_name(summer),
            rules
        )
    }

    pub fn local(self, utc: i64) -> i64 {
        utc + self.offset_min(utc) * 60
    }

    pub fn utc(self, local: i64) -> i64 {
        let guess = local - self.offset_min(local - CITIES[self.0].std_min * 60) * 60;
        local - self.offset_min(guess) * 60
    }
}

fn posix_name(min: i64) -> String {
    let sign = if min < 0 { '-' } else { '+' };
    let (h, m) = (min.abs() / 60, min.abs() % 60);
    if m == 0 {
        format!("<{sign}{h:02}>")
    } else {
        format!("<{sign}{h:02}{m:02}>")
    }
}

fn posix_offset(min: i64) -> String {
    let sign = if min > 0 { "-" } else { "" };
    let (h, m) = (min.abs() / 60, min.abs() % 60);
    if m == 0 {
        format!("{sign}{h}")
    } else {
        format!("{sign}{h}:{m:02}")
    }
}

fn posix_clock(min: i64) -> String {
    let (h, m) = (min / 60, min.rem_euclid(60));
    if m == 0 {
        format!("{h}")
    } else {
        format!("{h}:{m:02}")
    }
}

fn us_summer(utc: i64, std_min: i64) -> bool {
    let y = year(utc, std_min);
    let starts = sunday(y, 3, 2) * DAY + 2 * HOUR - std_min * 60;
    let ends = sunday(y, 11, 1) * DAY + 2 * HOUR - (std_min + 60) * 60;
    (starts..ends).contains(&utc)
}

fn eu_summer(utc: i64, std_min: i64) -> bool {
    let y = year(utc, std_min);
    let starts = last_sunday(y, 3) * DAY + HOUR;
    let ends = last_sunday(y, 10) * DAY + HOUR;
    (starts..ends).contains(&utc)
}

fn year(utc: i64, std_min: i64) -> i64 {
    civil_from_days((utc + std_min * 60).div_euclid(DAY)).0
}

fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

fn sunday(y: i64, m: i64, nth: i64) -> i64 {
    let first = days_from_civil(y, m, 1);
    first + (7 - weekday(first)) % 7 + 7 * (nth - 1)
}

fn last_sunday(y: i64, m: i64) -> i64 {
    let last = days_from_civil(y, m, days_in_month(y, m));
    last - weekday(last)
}
