use slot_input::Btn;

use crate::bitmaps::ButtonLegend;
use crate::draw::{Canvas, Draw, TexId};
use crate::guide::{draw_help_screen, help_rows_face, GUIDE_HELP_HINT};
use crate::hud::footer_ink;
use crate::icon::{icon_coverage, Icon};
use crate::plate::{blit, hint_face, title_face, word_ink, UndoFace, CAP_GAP, HINT_EDGE};
use crate::power_menu::{heading_face, MENU_INK};
use crate::quick_menu::{button_face, button_pair, draw_menu_ground, DIM_INK};
use crate::slot_chrome::MOUTH_H;
use crate::text;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    General,
    Host,
    Numeric,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Special {
    Delete,
    Done,
    Shift,
    Symbols,
    Space,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Cap {
    Char(usize),
    Special(Special),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typed {
    Nothing,
    Changed,
    Help,
    Done(String),
    Cancelled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyboard {
    layout: Layout,
    title: String,
    subtitle: Option<String>,
    alert: bool,
    text: Vec<char>,
    cursor: usize,
    row: usize,
    col: usize,
    shift: bool,
    symbols: bool,
    help: bool,
    aim: Option<f32>,
}

use Cap::{Char as C, Special as S};
use Special::*;

const GENERAL: &[&[Cap]] = &[
    &[
        C(0),
        C(1),
        C(2),
        C(3),
        C(4),
        C(5),
        C(6),
        C(7),
        C(8),
        C(9),
        S(Delete),
    ],
    &[
        C(10),
        C(11),
        C(12),
        C(13),
        C(14),
        C(15),
        C(16),
        C(17),
        C(18),
        C(19),
    ],
    &[
        C(20),
        C(21),
        C(22),
        C(23),
        C(24),
        C(25),
        C(26),
        C(27),
        C(28),
        S(Done),
    ],
    &[
        S(Shift),
        C(29),
        C(30),
        C(31),
        C(32),
        C(33),
        C(34),
        C(35),
        S(Symbols),
    ],
    &[S(Space)],
];

const NUMERIC: &[&[Cap]] = &[
    &[C(6), C(7), C(8), S(Delete)],
    &[C(3), C(4), C(5), S(Done)],
    &[C(0), C(1), C(2)],
    &[C(9)],
];

const LOWER: &str = "1234567890qwertyuiopasdfghjklzxcvbnm";
const UPPER: &str = "1234567890QWERTYUIOPASDFGHJKLZXCVBNM";
const HOST_SHIFT: &str = ".-_:/~@+=,";
const SYMBOL: &str = "!@#$%^&*()`~[]\\|{};:'\"<>?/+=_,.-\u{20ac}\u{a3}\u{a5}\u{a2}";

fn layout_rows(layout: Layout) -> &'static [&'static [Cap]] {
    match layout {
        Layout::General | Layout::Host => GENERAL,
        Layout::Numeric => NUMERIC,
    }
}

fn nth(s: &str, i: usize) -> char {
    s.chars().nth(i).unwrap_or(' ')
}

impl Keyboard {
    pub fn new(layout: Layout, title: &str, subtitle: Option<&str>, text: &str) -> Keyboard {
        let text: Vec<char> = text.chars().collect();
        let row = if layout == Layout::Numeric { 0 } else { 1 };
        Keyboard {
            layout,
            title: title.to_string(),
            subtitle: subtitle.map(str::to_string),
            alert: false,
            cursor: text.len(),
            text,
            row,
            col: 0,
            shift: false,
            symbols: false,
            help: false,
            aim: None,
        }
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn subtitle(&self) -> Option<&str> {
        self.subtitle.as_deref()
    }

    pub fn alerted(self) -> Keyboard {
        Keyboard {
            alert: true,
            ..self
        }
    }

    pub fn refuse(&mut self, why: &str) {
        self.subtitle = Some(why.to_string());
        self.alert = true;
    }

    pub fn alert(&self) -> bool {
        self.alert
    }

    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn row(&self) -> usize {
        self.row
    }

    pub fn col(&self) -> usize {
        self.col
    }

    pub fn shift(&self) -> bool {
        self.shift
    }

    pub fn symbols(&self) -> bool {
        self.symbols
    }

    pub fn help(&self) -> bool {
        self.help
    }

    pub fn rows(&self) -> &'static [&'static [Cap]] {
        layout_rows(self.layout)
    }

    fn value(&self, i: usize) -> char {
        if self.layout == Layout::Numeric {
            nth(LOWER, i)
        } else if self.layout == Layout::Host && !self.symbols && self.shift && i < 10 {
            nth(HOST_SHIFT, i)
        } else if self.symbols || (self.shift && i < 10) {
            nth(SYMBOL, i)
        } else if self.shift {
            nth(UPPER, i)
        } else {
            nth(LOWER, i)
        }
    }

    pub fn label(&self, cap: Cap) -> String {
        match cap {
            Cap::Char(i) => self.value(i).to_string(),
            Cap::Special(s) => match s {
                Delete => "Delete",
                Done => "Done",
                Shift => "Shift",
                Symbols if self.symbols => "abc",
                Symbols => "#+=",
                Space => "Space",
            }
            .to_string(),
        }
    }

    fn insert(&mut self, c: char) -> Typed {
        self.text.insert(self.cursor, c);
        self.cursor += 1;
        Typed::Changed
    }

    fn backspace(&mut self) -> Typed {
        if self.cursor == 0 {
            return Typed::Nothing;
        }
        self.cursor -= 1;
        self.text.remove(self.cursor);
        Typed::Changed
    }

    fn done(&self) -> Typed {
        Typed::Done(self.text.iter().collect())
    }

    fn centre(&self, row: usize, col: usize) -> f32 {
        let [x, _, w, _] = key_rect(Canvas::of((720, 480)), self.layout, row, col);
        x + w / 2.0
    }

    fn nearest(&self, row: usize, x: f32) -> usize {
        (0..self.rows()[row].len())
            .min_by(|a, b| {
                let off = |c: usize| (self.centre(row, c) - x).abs();
                off(*a).total_cmp(&off(*b))
            })
            .unwrap_or(0)
    }

    fn navigate(&mut self, b: Btn) {
        let rows = self.rows();
        let n = rows.len();
        match b {
            Btn::Up | Btn::Down => {
                let x = self.aim.unwrap_or_else(|| self.centre(self.row, self.col));
                self.row = match b {
                    Btn::Up => (self.row + n - 1) % n,
                    _ => (self.row + 1) % n,
                };
                self.col = self.nearest(self.row, x);
                self.aim = Some(x);
            }
            Btn::Left => {
                let len = rows[self.row].len();
                self.col = (self.col + len - 1) % len;
                self.aim = None;
            }
            _ => {
                self.col = (self.col + 1) % rows[self.row].len();
                self.aim = None;
            }
        }
    }

    pub fn on_delete(&self) -> bool {
        self.rows()[self.row][self.col] == Cap::Special(Delete)
    }

    pub fn press(&mut self, b: Btn) -> Typed {
        if self.help {
            if !matches!(b, Btn::B | Btn::Menu) {
                return Typed::Nothing;
            }
            self.help = false;
            return Typed::Changed;
        }
        let general = self.layout != Layout::Numeric;
        match b {
            Btn::Up | Btn::Down | Btn::Left | Btn::Right => {
                self.navigate(b);
                Typed::Changed
            }
            Btn::A => match self.rows()[self.row][self.col] {
                Cap::Char(i) => self.insert(self.value(i)),
                Cap::Special(Delete) => self.backspace(),
                Cap::Special(Done) => self.done(),
                Cap::Special(Shift) => {
                    self.shift = !self.shift;
                    Typed::Changed
                }
                Cap::Special(Symbols) => {
                    self.symbols = !self.symbols;
                    Typed::Changed
                }
                Cap::Special(Space) => self.insert(' '),
            },
            Btn::B => self.backspace(),
            Btn::X if general => self.insert(' '),
            Btn::Select if general => {
                self.shift = !self.shift;
                Typed::Changed
            }
            Btn::Y => Typed::Cancelled,
            Btn::Start => self.done(),
            Btn::L1 if self.cursor > 0 => {
                self.cursor -= 1;
                Typed::Changed
            }
            Btn::R1 if self.cursor < self.text.len() => {
                self.cursor += 1;
                Typed::Changed
            }
            Btn::Menu => {
                self.help = true;
                Typed::Help
            }
            _ => Typed::Nothing,
        }
    }
}

pub type KeyTex = (TexId, u32, u32);

pub struct KeyboardFaces {
    pub title: UndoFace,
    pub subtitle: Option<UndoFace>,
    pub text: UndoFace,
    pub stops: Vec<f32>,
    pub caps: Vec<Vec<[UndoFace; 2]>>,
    pub help: UndoFace,
    pub help_title: UndoFace,
    pub help_back: UndoFace,
    pub legends: [UndoFace; 2],
}

pub struct KeyboardTex {
    pub title: KeyTex,
    pub subtitle: Option<KeyTex>,
    pub text: KeyTex,
    pub stops: Vec<f32>,
    pub caps: Vec<Vec<[KeyTex; 2]>>,
    pub help: KeyTex,
    pub help_title: KeyTex,
    pub help_back: KeyTex,
    pub legends: [KeyTex; 2],
}

impl KeyboardFaces {
    pub fn upload(&self, mut create: impl FnMut(&UndoFace) -> TexId) -> KeyboardTex {
        let mut tex = |f: &UndoFace| (create(f), f.w, f.h);
        KeyboardTex {
            title: tex(&self.title),
            subtitle: self.subtitle.as_ref().map(&mut tex),
            text: tex(&self.text),
            stops: self.stops.clone(),
            caps: self
                .caps
                .iter()
                .map(|row| row.iter().map(|[a, b]| [tex(a), tex(b)]).collect())
                .collect(),
            help: tex(&self.help),
            help_title: tex(&self.help_title),
            help_back: tex(&self.help_back),
            legends: [tex(&self.legends[0]), tex(&self.legends[1])],
        }
    }
}

const TITLE_Y: f32 = 20.0;
const SUBTITLE_Y: f32 = 60.0;
const SUBTITLE_PX: f32 = 16.0;
const ALERT_PX: f32 = 18.0;
const SUBTITLE_H: u32 = 24;
const FIELD_X: f32 = 60.0;
const FIELD_Y: f32 = 104.0;
const FIELD_PX: f32 = 30.0;
const FIELD_H: u32 = 40;
const CARET_W: f32 = 2.0;
const CARET_GAP: f32 = 2.0;
const CARET_INSET: f32 = 6.0;
const RULE_X: f32 = 52.0;
const RULE_Y: f32 = 149.0;
const RULE_H: f32 = 2.0;
const GRID_Y: f32 = 170.0;
const ROW_PITCH: f32 = 50.0;
const KEY_H: u32 = 44;
const KEY_W: u32 = 50;
const KEY_GAP: u32 = 6;
const KEY_MARGIN: f32 = 24.0;
const HELP_COLUMN_GAP: u32 = 40;
const ROW_KEYS: u32 = 12;
const SPACE_W: u32 = 300;
const KEY_R: f32 = 7.0;
const CAP_PX: f32 = 24.0;
const WORD_PX: f32 = 16.0;
const ICON_PX: f32 = 28.0;
const TILE: [u8; 4] = [0x12, 0x12, 0x14, 0xb8];
const CHOSEN: [u8; 4] = [0xf0, 0xf0, 0xee, 0xff];
const CHOSEN_INK: [u8; 3] = [0x18, 0x18, 0x1a];

fn key_w(canvas: Canvas) -> u32 {
    let room = canvas.w - 2.0 * KEY_MARGIN - ((ROW_KEYS - 1) * KEY_GAP) as f32;
    ((room / ROW_KEYS as f32).floor() as u32).min(KEY_W)
}

fn cap_width(cap: Cap, key_w: u32) -> u32 {
    match cap {
        Cap::Char(_) => key_w,
        Cap::Special(Space) => SPACE_W,
        Cap::Special(_) => 2 * key_w + KEY_GAP,
    }
}

fn block_width(layout: Layout, key_w: u32) -> f32 {
    layout_rows(layout)
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| cap_width(*c, key_w) + KEY_GAP)
                .sum::<u32>()
                - KEY_GAP
        })
        .max()
        .unwrap_or(0) as f32
}

fn indent(layout: Layout, row: usize, key_w: u32) -> f32 {
    let pitch = (key_w + KEY_GAP) as f32;
    match (layout, row) {
        (Layout::General | Layout::Host, 1) => pitch,
        (Layout::General | Layout::Host, 2 | 3) => pitch / 2.0,
        _ => 0.0,
    }
}

pub fn key_rect(canvas: Canvas, layout: Layout, row: usize, col: usize) -> [f32; 4] {
    let caps = layout_rows(layout)[row];
    let y = GRID_Y + canvas.inset_y() + row as f32 * ROW_PITCH;
    let kw = key_w(canvas);
    let w = cap_width(caps[col], kw) as f32;
    if caps[col] == Cap::Special(Space) {
        return [canvas.centred(w).round(), y, w, KEY_H as f32];
    }
    let left = canvas.centred(block_width(layout, kw)).round() + indent(layout, row, kw);
    let before: u32 = caps[..col]
        .iter()
        .map(|c| cap_width(*c, kw) + KEY_GAP)
        .sum();
    [left + before as f32, y, w, KEY_H as f32]
}

fn blank() -> UndoFace {
    UndoFace {
        rgba: Vec::new(),
        w: 0,
        h: 0,
    }
}

fn plain(line: &str, px: f32) -> text::Layout {
    text::Layout {
        lines: vec![line.to_string()],
        px,
        tracking: 0.0,
    }
}

const LINE_LEAD: f32 = 1.0;

fn line_face(line: &str, px: f32, h: u32, ink: [u8; 3]) -> (UndoFace, f32) {
    let Some(font) = text::label_font() else {
        return (blank(), 0.0);
    };
    let set = text::line_width(font, line, px, 0.0);
    let w = set.ceil() as u32 + 2;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    text::draw_left(&mut rgba, w, h, &plain(line, px), LINE_LEAD, ink);
    (UndoFace { rgba, w, h }, LINE_LEAD)
}

pub fn keyboard_text_face(kb: &Keyboard) -> (UndoFace, Vec<f32>) {
    let typed = kb.text();
    let (face, lead) = line_face(&typed, FIELD_PX, FIELD_H, MENU_INK);
    let Some(font) = text::label_font() else {
        return (face, vec![0.0]);
    };
    let mut stops = vec![lead];
    let mut pen = lead;
    for ch in typed.chars() {
        pen += font.metrics(ch, FIELD_PX).advance_width;
        stops.push(pen);
    }
    (face, stops)
}

fn subtitle_face(label: &str, alert: bool) -> UndoFace {
    let Some(font) = text::label_font() else {
        return blank();
    };
    let (px, ink) = match alert {
        true => (ALERT_PX, MENU_INK),
        false => (SUBTITLE_PX, DIM_INK),
    };
    let layout = text::fit(font, label, f32::INFINITY, 1, px, px);
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = set.ceil() as u32 + 2;
    let mut rgba = vec![0u8; (w * SUBTITLE_H * 4) as usize];
    text::draw_centred(&mut rgba, w, SUBTITLE_H, &layout, ink);
    UndoFace {
        rgba,
        w,
        h: SUBTITLE_H,
    }
}

fn tile(w: u32, h: u32, fill: [u8; 4]) -> Vec<u8> {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (fw, fh) = (w as f32, h as f32);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = px.clamp(KEY_R, fw - KEY_R);
            let cy = py.clamp(KEY_R, fh - KEY_R);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let cover = (KEY_R - d + 0.5).clamp(0.0, 1.0);
            let at = ((y * w + x) * 4) as usize;
            rgba[at..at + 3].copy_from_slice(&fill[..3]);
            rgba[at + 3] = (fill[3] as f32 * cover).round() as u8;
        }
    }
    rgba
}

fn ink_over(px: &mut [u8], a: u8, ink: [u8; 3]) {
    let a = a as u32;
    let under = px[3] as u32 * (255 - a) / 255;
    let out = a + under;
    if out == 0 {
        return;
    }
    for c in 0..3 {
        px[c] = ((ink[c] as u32 * a + px[c] as u32 * under + out / 2) / out) as u8;
    }
    px[3] = out as u8;
}

fn cap_icon(cap: Cap) -> Option<Icon> {
    match cap {
        Cap::Special(Delete) => Some(Icon::Backspace),
        Cap::Special(Done) => Some(Icon::Return),
        Cap::Special(Shift) => Some(Icon::Shift),
        Cap::Special(Space) => Some(Icon::Space),
        _ => None,
    }
}

fn cap_face(kb: &Keyboard, cap: Cap, chosen: bool, key_w: u32) -> UndoFace {
    let (w, h) = (cap_width(cap, key_w), KEY_H);
    let (fill, ink) = match chosen {
        true => (CHOSEN, CHOSEN_INK),
        false => (TILE, MENU_INK),
    };
    let mut rgba = tile(w, h, fill);
    match cap_icon(cap).and_then(|i| icon_coverage(i, ICON_PX)) {
        Some((cov, iw, ih)) => {
            let (x0, y0) = (w.saturating_sub(iw) / 2, h.saturating_sub(ih) / 2);
            for gy in 0..ih.min(h) {
                for gx in 0..iw.min(w) {
                    let a = cov[(gy * iw + gx) as usize];
                    if a > 0 {
                        let at = (((y0 + gy) * w + x0 + gx) * 4) as usize;
                        ink_over(&mut rgba[at..at + 4], a, ink);
                    }
                }
            }
        }
        None => {
            let label = kb.label(cap);
            let px = match label.chars().count() {
                1 => CAP_PX,
                _ => WORD_PX,
            };
            text::draw_centred(&mut rgba, w, h, &plain(&label, px), ink);
        }
    }
    UndoFace { rgba, w, h }
}

pub fn keyboard_cap_faces(kb: &Keyboard, canvas: Canvas) -> Vec<Vec<[UndoFace; 2]>> {
    let kw = key_w(canvas);
    kb.rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(|cap| [cap_face(kb, *cap, false, kw), cap_face(kb, *cap, true, kw)])
                .collect()
        })
        .collect()
}

fn legend(button: UndoFace, word: &str, button_first: bool) -> UndoFace {
    let word = word_ink(word, footer_ink(false));
    let w = button.w + CAP_GAP + word.w + HINT_EDGE;
    let h = button.h;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (button_x, word_x) = match button_first {
        true => (0, button.w + CAP_GAP),
        false => (word.w + CAP_GAP, 0),
    };
    blit(&mut rgba, w, &button.rgba, button.w, button.h, button_x, 0);
    let word_y = h.saturating_sub(word.h) / 2;
    blit(&mut rgba, w, &word.rgba, word.w, word.h, word_x, word_y);
    UndoFace { rgba, w, h }
}

pub const KEYBOARD_HELP_TITLE: &str = "Keyboard Help";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Shortcut {
    Delete,
    Space,
    Shift,
    Cursor,
    Cancel,
    Done,
}

impl Shortcut {
    pub fn label(self) -> &'static str {
        match self {
            Shortcut::Delete => "Delete",
            Shortcut::Space => "Space",
            Shortcut::Shift => "Shift",
            Shortcut::Cursor => "Cursor",
            Shortcut::Cancel => "Cancel",
            Shortcut::Done => "Done",
        }
    }

    fn art(self) -> UndoFace {
        match self {
            Shortcut::Delete => button_face(ButtonLegend::B),
            Shortcut::Space => button_face(ButtonLegend::X),
            Shortcut::Shift => button_face(ButtonLegend::Select),
            Shortcut::Cursor => button_pair(ButtonLegend::L1, ButtonLegend::R1),
            Shortcut::Cancel => button_face(ButtonLegend::Y),
            Shortcut::Done => button_face(ButtonLegend::Start),
        }
    }
}

pub fn keyboard_help_columns(layout: Layout) -> [Vec<Shortcut>; 2] {
    use Shortcut::*;
    match layout {
        Layout::General | Layout::Host => [vec![Delete, Space, Shift], vec![Cursor, Cancel, Done]],
        Layout::Numeric => [vec![Delete, Cursor], vec![Cancel, Done]],
    }
}

pub fn keyboard_help_face(layout: Layout) -> UndoFace {
    let [left, right] = keyboard_help_columns(layout)
        .map(|column| help_rows_face(column.into_iter().map(|s| (s.art(), s.label())).collect()));
    let w = left.w + HELP_COLUMN_GAP + right.w;
    let h = left.h.max(right.h);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    blit(&mut rgba, w, &left.rgba, left.w, left.h, 0, 0);
    let x = left.w + HELP_COLUMN_GAP;
    blit(&mut rgba, w, &right.rgba, right.w, right.h, x, 0);
    UndoFace { rgba, w, h }
}

pub fn keyboard_faces(kb: &Keyboard, canvas: Canvas) -> KeyboardFaces {
    let (text, stops) = keyboard_text_face(kb);
    KeyboardFaces {
        title: heading_face(kb.title()),
        subtitle: kb.subtitle().map(|s| subtitle_face(s, kb.alert())),
        text,
        stops,
        caps: keyboard_cap_faces(kb, canvas),
        help: keyboard_help_face(kb.layout()),
        help_title: title_face(KEYBOARD_HELP_TITLE),
        help_back: hint_face(GUIDE_HELP_HINT.0, GUIDE_HELP_HINT.1),
        legends: [
            legend(button_face(ButtonLegend::Y), "Cancel", true),
            legend(button_face(ButtonLegend::Menu), "Help", false),
        ],
    }
}

fn place(out: &mut Vec<Draw>, (tex, w, h): KeyTex, x: f32, y: f32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

fn colour(ink: [u8; 3]) -> [f32; 4] {
    [
        ink[0] as f32 / 255.0,
        ink[1] as f32 / 255.0,
        ink[2] as f32 / 255.0,
        1.0,
    ]
}

fn draw_field(canvas: Canvas, faces: &KeyboardTex, kb: &Keyboard, out: &mut Vec<Draw>) {
    let room = canvas.w - 2.0 * FIELD_X;
    let caret = faces
        .stops
        .get(kb.cursor())
        .or(faces.stops.last())
        .copied()
        .unwrap_or(0.0);
    let scroll = (caret + CARET_GAP + CARET_W - room).max(0.0).round();
    let (tex, tw, th) = faces.text;
    if tw > 0 {
        let shown = (tw as f32 - scroll).min(room);
        out.push(Draw::Part {
            x: FIELD_X,
            y: FIELD_Y + canvas.inset_y(),
            w: shown,
            h: th as f32,
            tex,
            alpha: 1.0,
            uv: [scroll / tw as f32, 0.0, shown / tw as f32, 1.0],
        });
    }
    out.push(Draw::Rect {
        x: (FIELD_X + caret + CARET_GAP - scroll).round(),
        y: FIELD_Y + canvas.inset_y() + CARET_INSET,
        w: CARET_W,
        h: FIELD_H as f32 - 2.0 * CARET_INSET,
        colour: colour(MENU_INK),
    });
    out.push(Draw::Rect {
        x: RULE_X,
        y: RULE_Y + canvas.inset_y(),
        w: canvas.w - 2.0 * RULE_X,
        h: RULE_H,
        colour: colour(DIM_INK),
    });
}

pub fn draw_keyboard(
    canvas: Canvas,
    ground: Option<crate::MenuGround>,
    faces: &KeyboardTex,
    kb: &Keyboard,
    out: &mut Vec<Draw>,
) {
    if kb.help() {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        let (title, back) = (faces.help_title.0, faces.help_back.0);
        return draw_help_screen(canvas, Some(title), Some(back), Some(faces.help), out);
    }
    draw_menu_ground(canvas, ground, out);
    place(
        out,
        faces.title,
        canvas.centred(faces.title.1 as f32),
        TITLE_Y + canvas.inset_y(),
    );
    if let Some(sub) = faces.subtitle {
        place(
            out,
            sub,
            canvas.centred(sub.1 as f32),
            SUBTITLE_Y + canvas.inset_y(),
        );
    }
    draw_field(canvas, faces, kb, out);
    for (r, row) in faces.caps.iter().enumerate() {
        for (c, pair) in row.iter().enumerate() {
            let chosen = (r, c) == (kb.row(), kb.col());
            let [x, y, _, _] = key_rect(canvas, kb.layout(), r, c);
            place(out, pair[chosen as usize], x, y);
        }
    }
}

pub fn draw_keyboard_legends(canvas: Canvas, faces: &KeyboardTex, out: &mut Vec<Draw>) {
    let y = canvas.h - MOUTH_H;
    let [(cancel, cw, _), (done, dw, _)] = faces.legends;
    for (tex, w, x) in [
        (cancel, cw, 0.0),
        (
            done,
            dw,
            (canvas.w - dw.saturating_sub(HINT_EDGE) as f32).round(),
        ),
    ] {
        out.push(Draw::Tex {
            x,
            y,
            w: w as f32,
            h: MOUTH_H,
            tex,
            alpha: 1.0,
        });
    }
}
