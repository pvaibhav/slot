use slot_gfx::{Canvas, Draw, TexId};
use crate::battery::Gauge;
use slot_store::{parse_stamp, StateEntry};

use crate::art;
use crate::footer::{draw_corners, Printed};
use crate::hud::{PLATE, PLATE_H};
use crate::plate::{hint_quad, hint_row, hint_width, Hint, HINT_GAP, HINT_H, TITLE_H, TITLE_W};

pub const PHOTO_W: u32 = 240;
pub const PHOTO_H: u32 = 160;

const BLANK: [u8; 3] = [0x3a, 0x3a, 0x3e];

pub const DOT: f32 = 6.0;
const DOT_GAP: f32 = 10.0;
const DOT_DIM: f32 = 0.35;

const MARGIN: f32 = 16.0;

pub const LEGEND: [(&str, &str); 3] = [("B", "Back"), ("Y", "Delete"), ("A", "Load")];
const WAYS_OUT: usize = 2;
const UNDO_KEY: &str = "X";

pub struct PhotoFace {
    pub rgba: Vec<u8>,
    pub w: u32,
    pub h: u32,
}

pub fn photo_face(entry: &StateEntry) -> PhotoFace {
    let rgba = art::cover(&entry.thumb, PHOTO_W, PHOTO_H).unwrap_or_else(|| {
        std::iter::repeat_n(
            [BLANK[0], BLANK[1], BLANK[2], 255],
            (PHOTO_W * PHOTO_H) as usize,
        )
        .flatten()
        .collect()
    });
    PhotoFace {
        rgba,
        w: PHOTO_W,
        h: PHOTO_H,
    }
}

pub struct Polaroids {
    pub entries: Vec<StateEntry>,
    pub index: usize,
    faces: Vec<TexId>,
    title: Option<TexId>,
    hint_faces: Vec<TexId>,
    undo: Option<String>,
}

impl Polaroids {
    pub fn new(entries: Vec<StateEntry>) -> Self {
        Polaroids {
            entries,
            index: 0,
            faces: Vec::new(),
            title: None,
            hint_faces: Vec::new(),
            undo: None,
        }
    }

    pub fn set_faces(&mut self, faces: Vec<TexId>) {
        self.faces = faces;
    }

    pub fn set_title_face(&mut self, face: Option<TexId>) {
        self.title = face;
    }

    pub fn set_hint_faces(&mut self, faces: Vec<TexId>) {
        self.hint_faces = faces;
    }

    pub fn set_undo(&mut self, label: Option<&str>) {
        self.undo = label.map(str::to_string);
    }

    pub fn hints(&self) -> Vec<Hint> {
        let mut out = hint_row(&LEGEND);
        if let Some(label) = &self.undo {
            out.push(Hint {
                key: UNDO_KEY,
                label: label.clone(),
            });
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn selected(&self) -> Option<&StateEntry> {
        self.entries.get(self.index)
    }

    pub fn dots(&self) -> (usize, usize) {
        (self.entries.len(), self.index)
    }

    pub fn title(&self, now: &str, clock_24h: bool) -> String {
        match self.selected() {
            Some(e) => Self::relative_time(&e.stamp, now, clock_24h),
            None => String::new(),
        }
    }

    pub fn remove_selected(&mut self) {
        if self.index >= self.entries.len() {
            return;
        }
        self.entries.remove(self.index);
        if self.index < self.faces.len() {
            self.faces.remove(self.index);
        }
        self.index = self.index.min(self.entries.len().saturating_sub(1));
    }

    pub fn left(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.index = (self.index + 1).min(self.entries.len().saturating_sub(1));
    }

    pub fn draw(
        &self,
        gauge: Gauge,
        bolt: Option<TexId>,
        clock: Printed,
        out: &mut Vec<Draw>,
    ) {
        self.draw_photo(Canvas::FRAMED, out);
        self.draw_chrome(Canvas::FRAMED, gauge, bolt, clock, out);
    }

    pub fn draw_chrome(
        &self,
        canvas: Canvas,
        gauge: Gauge,
        bolt: Option<TexId>,
        clock: Printed,
        out: &mut Vec<Draw>,
    ) {
        let hints = self.hints();
        draw_album_chrome(
            canvas,
            AlbumChrome {
                title: self.title,
                hints: &hints,
                hint_faces: &self.hint_faces,
                ways_out: WAYS_OUT,
                dots: Some(self.dots()),
            },
            gauge,
            bolt,
            clock,
            out,
        );
    }

    pub fn draw_photo(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        let (w, h) = (canvas.w, canvas.h);
        out.push(match self.faces.get(self.index) {
            Some(tex) => Draw::Shot { tex: *tex },
            None => Draw::Rect {
                x: 0.0,
                y: 0.0,
                w,
                h,
                colour: [
                    BLANK[0] as f32 / 255.0,
                    BLANK[1] as f32 / 255.0,
                    BLANK[2] as f32 / 255.0,
                    1.0,
                ],
            },
        });
    }

    pub fn relative_time(stamp: &str, now: &str, clock_24h: bool) -> String {
        const RELATIVE_WINDOW: i64 = 12 * 3600;
        let seconds = stamp.get(..19).unwrap_or(stamp);
        let (Some(then), Some(parsed)) = (parse_stamp(seconds), parse_stamp(now)) else {
            return stamp.to_string();
        };
        let delta = parsed - then;
        if delta < 60 {
            return "just now".into();
        }
        if delta < 3600 {
            return format!("{} min ago", delta / 60);
        }
        if delta < RELATIVE_WINDOW {
            return format!("{} hr ago", delta / 3600);
        }
        format!(
            "{} {}",
            &stamp[..10],
            crate::clock::time_text(then, clock_24h)
        )
    }
}

pub(crate) fn plate(canvas: Canvas, y: f32) -> Draw {
    Draw::Rect {
        x: 0.0,
        y,
        w: canvas.w,
        h: PLATE_H,
        colour: PLATE,
    }
}

pub struct AlbumChrome<'a> {
    pub title: Option<TexId>,
    pub hints: &'a [Hint],
    pub hint_faces: &'a [TexId],
    pub ways_out: usize,
    pub dots: Option<(usize, usize)>,
}

pub fn draw_album_chrome(
    canvas: Canvas,
    chrome: AlbumChrome,
    gauge: Gauge,
    bolt: Option<TexId>,
    clock: Printed,
    out: &mut Vec<Draw>,
) {
    out.push(plate(canvas, 0.0));
    if let Some(tex) = chrome.title {
        out.push(Draw::Tex {
            x: canvas.centred(TITLE_W as f32),
            y: (PLATE_H - TITLE_H as f32) / 2.0,
            w: TITLE_W as f32,
            h: TITLE_H as f32,
            tex,
            alpha: 1.0,
        });
    }
    draw_corners(canvas, gauge, bolt, clock, out);

    let top = canvas.h - PLATE_H;
    out.push(plate(canvas, top));
    let y = top + (PLATE_H - HINT_H as f32) / 2.0;
    let widths: Vec<f32> = chrome
        .hints
        .iter()
        .map(|h| match h.key == crate::shot_viewer::PAN_KEY {
            true => crate::plate::pad_hint_width(&h.label) as f32,
            false => hint_width(h.key, &h.label) as f32,
        })
        .collect();
    let tail = &widths[chrome.ways_out.min(widths.len())..];
    let tail_w = tail.iter().sum::<f32>() + HINT_GAP * tail.len().saturating_sub(1) as f32;
    let mut x = MARGIN;
    for (i, w) in widths.iter().enumerate() {
        if i == chrome.ways_out {
            x = canvas.w - MARGIN - tail_w;
        }
        out.push(hint_quad(x, y, *w, chrome.hint_faces.get(i).copied()));
        x += w + HINT_GAP;
    }

    let Some((n, index)) = chrome.dots else {
        return;
    };
    let row = n as f32 * DOT + (n as f32 - 1.0).max(0.0) * DOT_GAP;
    let mut x = canvas.centred(row);
    for i in 0..n {
        out.push(Draw::Rect {
            x,
            y: top + (PLATE_H - DOT) / 2.0,
            w: DOT,
            h: DOT,
            colour: [1.0, 1.0, 1.0, if i == index { 1.0 } else { DOT_DIM }],
        });
        x += DOT + DOT_GAP;
    }
}
