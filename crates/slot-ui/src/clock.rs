use slot_gfx::{Canvas, Draw, TexId};
use slot_store::{civil_from_days, days_from_civil, days_in_month, parse_stamp, Zone};

use crate::plate::UndoFace;
use crate::power_menu::{MENU_H, MENU_INK, MENU_PAD, MENU_PX};
use crate::quick_menu::{
    draw_menu_ground, quick_value_face, DIM_INK, LEGEND_Y, POINTER_BLEED, POINTER_GAP, QUICK_EDGE,
    QUICK_LABEL_X, QUICK_PITCH, QUICK_VISIBLE, TYPE_DROP,
};
use crate::text;

const DAY: i64 = 86_400;

const YEAR_MIN: i64 = 2000;
const YEAR_MAX: i64 = 2099;

const CARET_H: f32 = 3.0;
const CITY_PAGE: usize = QUICK_VISIBLE;
const TITLE_GAP: f32 = 24.0;
const SCROLL_INSET: f32 = 12.0;
const SCROLL_W: f32 = 4.0;

pub fn clock_label(stamp: &str) -> String {
    parse_stamp(stamp).map(hhmm).unwrap_or_default()
}

pub fn hhmm(secs: i64) -> String {
    time_text(secs, true)
}

pub fn time_text(secs: i64, clock_24h: bool) -> String {
    let rem = secs.rem_euclid(DAY);
    hour_minute(rem / 3600, rem / 60 % 60, clock_24h)
}

fn hour_minute(hour: i64, minute: i64, clock_24h: bool) -> String {
    match clock_24h {
        true => format!("{hour:02}:{minute:02}"),
        false => format!("{}:{minute:02} {}", twelve(hour), meridiem(hour)),
    }
}

fn twelve(hour: i64) -> i64 {
    match hour % 12 {
        0 => 12,
        h => h,
    }
}

fn meridiem(hour: i64) -> &'static str {
    if hour < 12 {
        "AM"
    } else {
        "PM"
    }
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

fn month_name(month: i64) -> &'static str {
    MONTHS
        .get((month - 1) as usize)
        .copied()
        .unwrap_or_default()
}

pub fn date_time_text(secs: i64, clock_24h: bool) -> String {
    let (_, month, day) = civil_from_days(secs.div_euclid(DAY));
    format!("{} {day} {}", month_name(month), time_text(secs, clock_24h))
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ClockRow {
    NetworkTime,
    Date,
    Time,
    Zone,
    Clock,
}

impl ClockRow {
    pub const ALL: [ClockRow; 5] = [
        ClockRow::NetworkTime,
        ClockRow::Date,
        ClockRow::Time,
        ClockRow::Zone,
        ClockRow::Clock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ClockRow::NetworkTime => "Set Automatically",
            ClockRow::Date => "Date",
            ClockRow::Time => "Time",
            ClockRow::Zone => "City",
            ClockRow::Clock => "Clock",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Part {
    Month,
    Day,
    Year,
    Hour,
    Minute,
    Meridiem,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ClockPicker {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    zone: Zone,
    clock_24h: bool,
    network_time: Option<bool>,
    row: usize,
    part: Option<usize>,
    choosing: Option<usize>,
}

impl ClockPicker {
    pub fn new(utc: i64, zone: Zone, clock_24h: bool) -> Self {
        let local = zone.local(utc);
        let (year, month, day) = civil_from_days(local.div_euclid(DAY));
        let rem = local.rem_euclid(DAY);
        ClockPicker {
            year: year.clamp(YEAR_MIN, YEAR_MAX),
            month,
            day,
            hour: rem / 3600,
            minute: rem / 60 % 60,
            zone,
            clock_24h,
            network_time: None,
            row: 0,
            part: None,
            choosing: None,
        }
    }

    pub fn with_network_time(mut self, network_time: Option<bool>) -> Self {
        self.network_time = network_time;
        self
    }

    pub fn from_ymd(year: i64, month: i64, day: i64) -> Self {
        ClockPicker {
            year: year.clamp(YEAR_MIN, YEAR_MAX),
            month,
            day,
            hour: 0,
            minute: 0,
            zone: Zone::default(),
            clock_24h: true,
            network_time: None,
            row: 0,
            part: None,
            choosing: None,
        }
    }

    pub fn secs(&self) -> i64 {
        let local = days_from_civil(self.year, self.month, self.day) * DAY
            + self.hour * 3600
            + self.minute * 60;
        self.zone.utc(local)
    }

    pub fn zone(&self) -> Zone {
        self.zone
    }

    pub fn clock_24h(&self) -> bool {
        self.clock_24h
    }

    pub fn month(&self) -> i64 {
        self.month
    }

    pub fn day(&self) -> i64 {
        self.day
    }

    pub fn network_time(&self) -> Option<bool> {
        self.network_time
    }

    pub fn rows(&self) -> &'static [ClockRow] {
        match self.network_time {
            None => &[
                ClockRow::Date,
                ClockRow::Time,
                ClockRow::Zone,
                ClockRow::Clock,
            ],
            Some(false) => &ClockRow::ALL,
            Some(true) => &[ClockRow::NetworkTime, ClockRow::Zone, ClockRow::Clock],
        }
    }

    pub fn row(&self) -> ClockRow {
        self.rows()[self.row]
    }

    fn flip_network_time(&mut self) {
        self.network_time = self.network_time.map(|on| !on);
    }

    pub fn part(&self) -> Option<Part> {
        self.part.and_then(|i| self.parts().get(i).copied())
    }

    fn parts(&self) -> &'static [Part] {
        match (self.row(), self.clock_24h) {
            (ClockRow::Date, _) => &[Part::Month, Part::Day, Part::Year],
            (ClockRow::Time, true) => &[Part::Hour, Part::Minute],
            (ClockRow::Time, false) => &[Part::Hour, Part::Minute, Part::Meridiem],
            _ => &[],
        }
    }

    pub fn highlighted(&self) -> Option<Zone> {
        self.choosing.map(|i| Zone::ALL[i])
    }

    pub fn up(&mut self) {
        let cities = Zone::ALL.len();
        match (self.choosing, self.part) {
            (Some(i), _) => self.choosing = Some((i + cities - 1) % cities),
            (None, Some(_)) => self.step(1),
            (None, None) => self.row = (self.row + self.rows().len() - 1) % self.rows().len(),
        }
    }

    pub fn down(&mut self) {
        match (self.choosing, self.part) {
            (Some(i), _) => self.choosing = Some((i + 1) % Zone::ALL.len()),
            (None, Some(_)) => self.step(-1),
            (None, None) => self.row = (self.row + 1) % self.rows().len(),
        }
    }

    pub fn left(&mut self) {
        match (self.choosing, self.part, self.row()) {
            (Some(i), _, _) => self.choosing = Some(i.saturating_sub(CITY_PAGE)),
            (None, Some(i), _) => self.part = Some(i.saturating_sub(1)),
            (None, None, ClockRow::Clock) => self.clock_24h = !self.clock_24h,
            (None, None, ClockRow::NetworkTime) => self.flip_network_time(),
            _ => {}
        }
    }

    pub fn right(&mut self) {
        match (self.choosing, self.part, self.row()) {
            (Some(i), _, _) => self.choosing = Some((i + CITY_PAGE).min(Zone::ALL.len() - 1)),
            (None, Some(i), _) => self.part = Some((i + 1).min(self.parts().len() - 1)),
            (None, None, ClockRow::Clock) => self.clock_24h = !self.clock_24h,
            (None, None, ClockRow::NetworkTime) => self.flip_network_time(),
            _ => {}
        }
    }

    pub fn a(&mut self) {
        if self.b() {
            return;
        }
        if self.row() == ClockRow::NetworkTime {
            return self.flip_network_time();
        }
        if self.row() == ClockRow::Zone {
            self.choosing = Zone::ALL.iter().position(|z| *z == self.zone);
            return;
        }
        self.part = match self.part {
            Some(_) => None,
            None => Some(0).filter(|_| self.row_opens()),
        };
    }

    pub fn b(&mut self) -> bool {
        match self.choosing.take() {
            Some(i) => {
                self.zone = Zone::ALL[i];
                true
            }
            None => false,
        }
    }

    pub fn editing(&self) -> bool {
        self.part.is_some()
    }

    fn row_opens(&self) -> bool {
        !self.parts().is_empty() || self.row() == ClockRow::Zone
    }

    fn window(&self) -> (usize, usize, usize) {
        match self.choosing {
            Some(i) => {
                let len = CITY_PAGE.min(Zone::ALL.len());
                let start = i.saturating_sub(len / 2).min(Zone::ALL.len() - len);
                (start, len, i - start)
            }
            None => (0, self.rows().len(), self.row),
        }
    }

    fn step(&mut self, by: i64) {
        match self.part() {
            Some(Part::Year) => self.year = wrap(self.year + by, YEAR_MIN, YEAR_MAX),
            Some(Part::Month) => self.month = wrap(self.month + by, 1, 12),
            Some(Part::Day) => {
                self.day = wrap(self.day + by, 1, days_in_month(self.year, self.month))
            }
            Some(Part::Hour) if self.clock_24h => self.hour = wrap(self.hour + by, 0, 23),
            Some(Part::Hour) => self.hour = wrap(self.hour % 12 + by, 0, 11) + self.hour / 12 * 12,
            Some(Part::Minute) => self.minute = wrap(self.minute + by, 0, 59),
            Some(Part::Meridiem) => self.hour = (self.hour + 12) % 24,
            None => {}
        }
        self.day = self.day.min(days_in_month(self.year, self.month));
    }

    fn pieces(&self, row: ClockRow) -> Vec<(Option<Part>, String)> {
        match row {
            ClockRow::NetworkTime => vec![(
                None,
                match self.network_time {
                    Some(true) => "On",
                    _ => "Off",
                }
                .into(),
            )],
            ClockRow::Date => vec![
                (Some(Part::Month), month_name(self.month).into()),
                (None, " ".into()),
                (Some(Part::Day), self.day.to_string()),
                (None, " ".into()),
                (Some(Part::Year), self.year.to_string()),
            ],
            ClockRow::Time => {
                let hour = match self.clock_24h {
                    true => format!("{:02}", self.hour),
                    false => twelve(self.hour).to_string(),
                };
                let mut v = vec![
                    (Some(Part::Hour), hour),
                    (None, ":".into()),
                    (Some(Part::Minute), format!("{:02}", self.minute)),
                ];
                if !self.clock_24h {
                    v.push((None, " ".into()));
                    v.push((Some(Part::Meridiem), meridiem(self.hour).into()));
                }
                v
            }
            ClockRow::Zone => vec![(None, self.zone.name().into())],
            ClockRow::Clock => vec![(
                None,
                match self.clock_24h {
                    true => "24-Hour",
                    false => "12-Hour",
                }
                .into(),
            )],
        }
    }

    pub fn value_text(&self, row: ClockRow) -> String {
        self.pieces(row).into_iter().map(|(_, s)| s).collect()
    }

    pub fn offset_text(&self) -> String {
        offset_text(self.zone.offset_min(self.secs()))
    }

    pub fn faces(&self) -> ClockFaceSet {
        let pair = |text: &str| [false, true].map(|lit| quick_value_face(text, lit));
        if self.choosing.is_some() {
            let (start, len, _) = self.window();
            let at = self.secs();
            let rows = Zone::ALL[start..start + len]
                .iter()
                .map(|z| [pair(z.name()), pair(&offset_text(z.offset_min(at)))])
                .collect();
            return ClockFaceSet { rows, caret: None };
        }
        let rows = self
            .rows()
            .iter()
            .map(|&r| [pair(r.label()), pair(&self.value_text(r))])
            .collect();
        ClockFaceSet {
            rows,
            caret: self.caret(),
        }
    }

    fn caret(&self) -> Option<(u32, u32)> {
        let part = self.part()?;
        let font = text::label_font()?;
        let layout = text::fit(font, "0", f32::INFINITY, 1, MENU_PX, MENU_PX);
        let width = |s: &str| text::line_width(font, &s.to_uppercase(), layout.px, layout.tracking);
        let mut before = String::new();
        for (p, s) in self.pieces(self.row()) {
            if p == Some(part) {
                let x = match before.is_empty() {
                    true => 0.0,
                    false => width(&before) + layout.tracking,
                };
                return Some((MENU_PAD + x.round() as u32, width(&s).round() as u32));
            }
            before.push_str(&s);
        }
        None
    }
}

fn offset_text(offset: i64) -> String {
    let sign = if offset < 0 { '-' } else { '+' };
    let mins = offset.abs();
    format!("UTC{sign}{:02}:{:02}", mins / 60, mins % 60)
}

pub struct ClockFaceSet {
    pub rows: Vec<[[UndoFace; 2]; 2]>,
    pub caret: Option<(u32, u32)>,
}

pub type Face = (TexId, u32, u32);

pub struct ClockFaces {
    pub rows: Vec<[[Face; 2]; 2]>,
    pub caret: Option<(u32, u32)>,
}

pub struct ClockScreen<'a> {
    pub picker: &'a ClockPicker,
    pub faces: Option<&'a ClockFaces>,
    pub title: Option<Face>,
    pub pointer: Option<Face>,
    pub ground: Option<(TexId, u32)>,
    pub canvas: Canvas,
}

impl ClockScreen<'_> {
    pub fn draw(&self, out: &mut Vec<Draw>) {
        draw_menu_ground(self.canvas, self.ground, out);
        let (right, pad) = (self.canvas.w - QUICK_EDGE, MENU_PAD as f32);
        let (start, len, selected) = self.picker.window();
        let top = (LEGEND_Y - QUICK_PITCH * len as f32) / 2.0;
        if let (Some((tex, w, h)), None) = (self.title, self.picker.choosing) {
            let y = top - TITLE_GAP - h as f32;
            push(out, tex, self.canvas.centred(w as f32), y, w, h);
        }
        if let Some(faces) = self.faces {
            for (i, [label, value]) in faces.rows.iter().enumerate() {
                let y = top + QUICK_PITCH * i as f32 + TYPE_DROP;
                let lit = i == selected;
                let (tex, w, h) = label[lit as usize];
                let x = QUICK_LABEL_X - pad;
                push(out, tex, x, y, w, h);
                if let (true, Some((tex, pw, ph))) = (lit, self.pointer) {
                    let px = x + pad - POINTER_GAP - pw as f32;
                    push(out, tex, px, y - POINTER_BLEED as f32, pw, ph);
                }
                let (tex, w, h) = value[lit as usize];
                let vx = right + pad - w as f32;
                push(out, tex, vx, y, w, h);
                if let (true, Some((cx, cw))) = (lit, faces.caret) {
                    let ink = MENU_INK.map(|c| f32::from(c) / 255.0);
                    out.push(Draw::Rect {
                        x: (vx + cx as f32).round(),
                        y: (y + MENU_H as f32).round(),
                        w: cw as f32,
                        h: CARET_H,
                        colour: [ink[0], ink[1], ink[2], 1.0],
                    });
                }
            }
        }
        if self.picker.choosing.is_some() {
            self.draw_scroll(top, start, len, out);
        }
    }

    fn draw_scroll(&self, y: f32, start: usize, len: usize, out: &mut Vec<Draw>) {
        let x = self.canvas.w - SCROLL_INSET - SCROLL_W;
        let h = QUICK_PITCH * len as f32;
        let all = Zone::ALL.len() as f32;
        let ink = DIM_INK.map(|c| f32::from(c) / 255.0);
        out.push(Draw::Rect {
            x,
            y,
            w: SCROLL_W,
            h,
            colour: [ink[0], ink[1], ink[2], 0.3],
        });
        let lit = MENU_INK.map(|c| f32::from(c) / 255.0);
        let along = (start + self.picker.window().2) as f32 / (all - 1.0);
        let thumb_h = h / all * len as f32;
        out.push(Draw::Rect {
            x,
            y: y + (h - thumb_h) * along,
            w: SCROLL_W,
            h: thumb_h,
            colour: [lit[0], lit[1], lit[2], 1.0],
        });
    }
}

fn push(out: &mut Vec<Draw>, tex: TexId, x: f32, y: f32, w: u32, h: u32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

fn wrap(v: i64, lo: i64, hi: i64) -> i64 {
    (v - lo).rem_euclid(hi - lo + 1) + lo
}
