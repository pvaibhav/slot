use crate::battery::Gauge;
use slot_gfx::{Canvas, Draw, TexId};
use slot_store::ShotEntry;

use crate::footer::Printed;
use crate::hud::PLATE_H;
use std::path::Path;

use crate::plate::{hint_row, Hint, HINT_H};
use crate::polaroids::{draw_album_chrome, AlbumChrome, Polaroids};

pub const PAN_KEY: &str = "PAN";

pub const SHOT_HINTS: [(&str, &str); 9] = [
    ("A", "View"),
    ("B", "Back"),
    ("Y", "Delete"),
    ("A", "Zoom"),
    (PAN_KEY, "Pan"),
    ("B", "Zoom Out"),
    ("SELECT", "Multi"),
    ("A", "Tick"),
    ("B", "Done"),
];

const GRID_LEGEND: [usize; 4] = [0, 1, 2, 6];
const PICK_LEGEND: [usize; 3] = [7, 2, 8];
const FULL_LEGEND: [usize; 3] = [3, 1, 2];
const ZOOM_LEGEND: [usize; 3] = [4, 3, 5];
const ZOOM_MAX_LEGEND: [usize; 2] = [4, 5];

pub const GRID_COLS: usize = 3;
pub const GRID_GAP: f32 = 16.0;
pub const MAX_ZOOM: u32 = 4;
pub const PAN_TAP: f32 = 24.0;

const EDGE: f32 = 16.0;
const OUTLINE: f32 = 3.0;
const OUTLINE_GAP: f32 = 2.0;
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TICK_INSET: f32 = 6.0;
const SPOT_W: f32 = 48.0;
const SPOT_H: f32 = 32.0;
const SPOT_BACK: [f32; 4] = [1.0, 1.0, 1.0, 0.25];
const SPOT_VIEW: [f32; 4] = [1.0, 1.0, 1.0, 0.9];

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShotMode {
    Grid,
    Full,
    Zoomed,
}

pub fn grid_cell(canvas: Canvas) -> (f32, f32) {
    let w = (canvas.w - GRID_GAP * (GRID_COLS as f32 + 1.0)) / GRID_COLS as f32;
    (w, w * 2.0 / 3.0)
}

pub fn grid_rows_shown(canvas: Canvas) -> usize {
    let (_, h) = grid_cell(canvas);
    let room = canvas.h - PLATE_H - (PLATE_H + GRID_GAP);
    (((room + GRID_GAP) / (h + GRID_GAP)).floor() as usize).max(1)
}

pub struct ShotViewer {
    pub entries: Vec<ShotEntry>,
    pub index: usize,
    top_row: usize,
    rows_shown: usize,
    full: bool,
    zoom: u32,
    centre: (f32, f32),
    pan_held: [bool; 4],
    pan_at: Option<u64>,
    face: Option<(TexId, u32, u32)>,
    thumbs: Vec<Option<(TexId, u32, u32)>>,
    ticked: Vec<bool>,
    selecting: bool,
    tick_face: Option<(TexId, u32, u32)>,
    count: Option<(TexId, u32)>,
    title: Option<TexId>,
    hint_faces: Vec<TexId>,
}

impl ShotViewer {
    pub fn new(entries: Vec<ShotEntry>) -> Self {
        ShotViewer {
            thumbs: vec![None; entries.len()],
            ticked: vec![false; entries.len()],
            selecting: false,
            tick_face: None,
            count: None,
            entries,
            index: 0,
            top_row: 0,
            rows_shown: 2,
            full: false,
            zoom: 1,
            centre: (0.0, 0.0),
            pan_held: [false; 4],
            pan_at: None,
            face: None,
            title: None,
            hint_faces: Vec::new(),
        }
    }

    pub fn fill(&mut self, entries: Vec<ShotEntry>) {
        *self = ShotViewer {
            face: None,
            title: self.title,
            hint_faces: std::mem::take(&mut self.hint_faces),
            tick_face: self.tick_face,
            ..ShotViewer::new(entries)
        };
    }

    pub fn mode(&self) -> ShotMode {
        match (self.full, self.zoom > 1) {
            (false, _) => ShotMode::Grid,
            (true, false) => ShotMode::Full,
            (true, true) => ShotMode::Zoomed,
        }
    }

    pub fn top_row(&self) -> usize {
        self.top_row
    }

    pub fn set_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.face = face;
        if let Some((_, w, h)) = face {
            if self.zoom == 1 {
                self.centre = (w as f32 / 2.0, h as f32 / 2.0);
            }
        }
    }

    pub fn set_thumb(&mut self, path: &Path, face: Option<(TexId, u32, u32)>) {
        if let Some(i) = self.entries.iter().position(|e| e.path == path) {
            self.thumbs[i] = face;
        }
    }

    pub fn thumb(&self, i: usize) -> Option<(TexId, u32, u32)> {
        self.thumbs.get(i).copied().flatten()
    }

    pub fn set_tick_face(&mut self, face: Option<(TexId, u32, u32)>) {
        self.tick_face = face;
    }

    pub fn selecting(&self) -> bool {
        self.selecting
    }

    pub fn toggle_selecting(&mut self) {
        if self.full {
            return;
        }
        self.selecting = !self.selecting;
        self.ticked.iter_mut().for_each(|t| *t = false);
    }

    pub fn tick_selected(&mut self) {
        if let (true, Some(t)) = (self.selecting, self.ticked.get_mut(self.index)) {
            *t = !*t;
        }
    }

    pub fn is_ticked(&self, i: usize) -> bool {
        self.ticked.get(i).copied().unwrap_or(false)
    }

    pub fn ticked_count(&self) -> usize {
        self.ticked.iter().filter(|&&t| t).count()
    }

    pub fn remove_ticked(&mut self, canvas: Canvas) -> Vec<ShotEntry> {
        let first = self.ticked.iter().position(|&t| t);
        let mut gone = Vec::new();
        let mut kept = Vec::new();
        let mut thumbs = Vec::new();
        for ((e, t), thumb) in self.entries.drain(..).zip(&self.ticked).zip(&self.thumbs) {
            match t {
                true => gone.push(e),
                false => {
                    kept.push(e);
                    thumbs.push(*thumb);
                }
            }
        }
        self.entries = kept;
        self.thumbs = thumbs;
        self.ticked = vec![false; self.entries.len()];
        self.selecting = false;
        if let Some(first) = first {
            self.index = first;
        }
        self.index = self.index.min(self.entries.len().saturating_sub(1));
        self.reset_view();
        self.rows_shown = grid_rows_shown(canvas);
        self.scroll();
        gone
    }

    pub fn thumb_window(&self, canvas: Canvas) -> std::ops::Range<usize> {
        let first = self.top_row.saturating_sub(1);
        let end = self.top_row + grid_rows_shown(canvas) + 2;
        let n = self.entries.len();
        (first * GRID_COLS).min(n)..(end * GRID_COLS).min(n)
    }

    pub fn thumb_order(&self, canvas: Canvas) -> Vec<usize> {
        let window = self.thumb_window(canvas);
        let n = self.entries.len();
        let shown = (self.top_row * GRID_COLS).min(n)
            ..((self.top_row + grid_rows_shown(canvas) + 1) * GRID_COLS).min(n);
        let mut order: Vec<usize> = shown.clone().collect();
        order.extend(window.filter(|i| !shown.contains(i)));
        order
    }

    pub fn set_count_face(&mut self, face: Option<(TexId, u32)>) {
        self.count = face;
    }

    pub fn has_face(&self) -> bool {
        self.face.is_some()
    }

    pub fn zoom(&self) -> u32 {
        self.zoom
    }

    pub fn zoom_in(&mut self, canvas: Canvas) {
        if !self.full || self.face.is_none() {
            return;
        }
        self.zoom = (self.zoom + 1).min(MAX_ZOOM);
        self.clamp_centre(canvas);
    }

    pub fn zoom_out(&mut self, canvas: Canvas) {
        self.zoom = self.zoom.saturating_sub(1).max(1);
        self.clamp_centre(canvas);
        if self.zoom == 1 {
            self.pan_held = [false; 4];
            self.pan_at = None;
        }
    }

    fn scale(&self, canvas: Canvas) -> Option<(f32, u32, u32)> {
        let (_, w, h) = self.face?;
        let fit = (canvas.w / w as f32).min(canvas.h / h as f32);
        Some((fit * self.zoom as f32, w, h))
    }

    fn clamp_centre(&mut self, canvas: Canvas) {
        let Some((s, w, h)) = self.scale(canvas) else {
            return;
        };
        let axis = |c: f32, len: f32, room: f32| {
            let half = room / (2.0 * s);
            match len * s <= room {
                true => len / 2.0,
                false => c.clamp(half, len - half),
            }
        };
        self.centre = (
            axis(self.centre.0, w as f32, canvas.w),
            axis(self.centre.1, h as f32, canvas.h),
        );
    }

    fn pan_by(&mut self, dx: f32, dy: f32, canvas: Canvas) {
        let Some((s, ..)) = self.scale(canvas) else {
            return;
        };
        self.centre.0 += dx / s;
        self.centre.1 += dy / s;
        self.clamp_centre(canvas);
    }

    pub fn pan_tap(&mut self, dx: i32, dy: i32, canvas: Canvas) {
        self.pan_by(dx as f32 * PAN_TAP, dy as f32 * PAN_TAP, canvas);
    }

    pub fn pan_hold(&mut self, dx: i32, dy: i32, held: bool) {
        let key = match (dx, dy) {
            (-1, 0) => 0,
            (1, 0) => 1,
            (0, -1) => 2,
            (0, 1) => 3,
            _ => return,
        };
        self.pan_held[key] = held;
    }

    fn pan_dir(&self) -> (i32, i32) {
        let [l, r, u, d] = self.pan_held.map(i32::from);
        (r - l, d - u)
    }

    pub fn tick(&mut self, now_ms: u64, canvas: Canvas) {
        let (dx, dy) = self.pan_dir();
        if (dx, dy) == (0, 0) {
            self.pan_at = None;
            return;
        }
        if let Some(at) = self.pan_at {
            let secs = now_ms.saturating_sub(at) as f32 / 1000.0;
            self.pan_by(
                dx as f32 * canvas.w * secs,
                dy as f32 * canvas.h * secs,
                canvas,
            );
        }
        self.pan_at = Some(now_ms);
    }

    pub fn shot_rect(&self, canvas: Canvas) -> Option<(f32, f32, f32, f32)> {
        let (s, w, h) = self.scale(canvas)?;
        let (sw, sh) = (w as f32 * s, h as f32 * s);
        Some((
            canvas.w / 2.0 - self.centre.0 * s,
            canvas.h / 2.0 - self.centre.1 * s,
            sw,
            sh,
        ))
    }

    pub fn view_frac(&self, canvas: Canvas) -> Option<(f32, f32, f32, f32)> {
        let (x, y, sw, sh) = self.shot_rect(canvas)?;
        let left = (-x).max(0.0) / sw;
        let top = (-y).max(0.0) / sh;
        let right = ((canvas.w - x) / sw).min(1.0);
        let bottom = ((canvas.h - y) / sh).min(1.0);
        Some((left, top, right - left, bottom - top))
    }

    pub fn set_title_face(&mut self, face: Option<TexId>) {
        self.title = face;
    }

    pub fn set_hint_faces(&mut self, faces: Vec<TexId>) {
        self.hint_faces = faces;
    }

    pub fn legend(&self) -> Vec<usize> {
        match self.mode() {
            ShotMode::Grid if self.selecting => PICK_LEGEND.to_vec(),
            ShotMode::Grid => GRID_LEGEND.to_vec(),
            ShotMode::Full => FULL_LEGEND.to_vec(),
            ShotMode::Zoomed if self.zoom >= MAX_ZOOM => ZOOM_MAX_LEGEND.to_vec(),
            ShotMode::Zoomed => ZOOM_LEGEND.to_vec(),
        }
    }

    pub fn hints(&self) -> Vec<Hint> {
        let pairs: Vec<(&'static str, &'static str)> =
            self.legend().iter().map(|&i| SHOT_HINTS[i]).collect();
        hint_row(&pairs)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn selected(&self) -> Option<&ShotEntry> {
        self.entries.get(self.index)
    }

    pub fn count_text(&self) -> String {
        if self.entries.is_empty() {
            return String::new();
        }
        if self.selecting {
            return format!("{} selected", self.ticked_count());
        }
        format!("{} of {}", self.index + 1, self.entries.len())
    }

    pub fn title(&self, now: &str, clock_24h: bool) -> String {
        match self.selected() {
            Some(e) => Polaroids::relative_time(&e.stamp, now, clock_24h),
            None => String::new(),
        }
    }

    pub fn grid_move(&mut self, dx: i32, dy: i32, canvas: Canvas) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        if dx != 0 {
            self.index = (self.index as i64 + dx as i64).clamp(0, n as i64 - 1) as usize;
        }
        if dy != 0 {
            let row = (self.index / GRID_COLS) as i64 + dy as i64;
            if row >= 0 && (row as usize) * GRID_COLS < n {
                let col = self.index % GRID_COLS;
                self.index = (row as usize * GRID_COLS + col).min(n - 1);
            }
        }
        self.rows_shown = grid_rows_shown(canvas);
        self.scroll();
    }

    fn scroll(&mut self) {
        let row = self.index / GRID_COLS;
        if row < self.top_row {
            self.top_row = row;
        }
        if row >= self.top_row + self.rows_shown {
            self.top_row = row + 1 - self.rows_shown;
        }
        let last = self.entries.len().saturating_sub(1) / GRID_COLS;
        self.top_row = self
            .top_row
            .min((last + 1).saturating_sub(self.rows_shown))
            .min(row);
    }

    pub fn open(&mut self) {
        if self.selected().is_some() {
            self.full = true;
            self.zoom = 1;
        }
    }

    pub fn to_grid(&mut self, canvas: Canvas) {
        self.full = false;
        self.reset_view();
        self.rows_shown = grid_rows_shown(canvas);
        self.scroll();
    }

    pub fn step(&mut self, dir: i32) {
        let n = self.entries.len();
        if n == 0 {
            return;
        }
        let to = (self.index as i64 + dir as i64).clamp(0, n as i64 - 1) as usize;
        if to != self.index {
            self.index = to;
            self.reset_view();
        }
    }

    fn reset_view(&mut self) {
        self.zoom = 1;
        self.face = None;
        self.pan_held = [false; 4];
        self.pan_at = None;
    }

    pub fn remove_selected(&mut self, canvas: Canvas) {
        if self.index >= self.entries.len() {
            return;
        }
        self.entries.remove(self.index);
        self.thumbs.remove(self.index);
        self.ticked.remove(self.index);
        self.reset_view();
        self.index = self.index.min(self.entries.len().saturating_sub(1));
        self.rows_shown = grid_rows_shown(canvas);
        self.scroll();
    }

    pub fn draw(
        &self,
        canvas: Canvas,
        gauge: Gauge,
        bolt: Option<TexId>,
        clock: Printed,
        out: &mut Vec<Draw>,
    ) {
        out.push(Draw::Rect {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            colour: [0.0, 0.0, 0.0, 1.0],
        });
        match self.mode() {
            ShotMode::Grid => self.draw_grid(canvas, out),
            _ => self.draw_shot(canvas, out),
        }
        let hints = self.hints();
        let faces: Vec<TexId> = self
            .legend()
            .iter()
            .filter_map(|&i| self.hint_faces.get(i).copied())
            .collect();
        draw_album_chrome(
            canvas,
            AlbumChrome {
                title: self.title,
                hints: &hints,
                hint_faces: &faces,
                ways_out: hints.len(),
                dots: None,
            },
            gauge,
            bolt,
            clock,
            out,
        );
        self.draw_corner(canvas, out);
    }

    fn draw_grid(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        let (cw, ch) = grid_cell(canvas);
        for (i, thumb) in self
            .thumbs
            .iter()
            .enumerate()
            .skip(self.top_row * GRID_COLS)
        {
            let (row, col) = (i / GRID_COLS - self.top_row, i % GRID_COLS);
            let x = GRID_GAP + col as f32 * (cw + GRID_GAP);
            let y = PLATE_H + GRID_GAP + row as f32 * (ch + GRID_GAP);
            if y >= canvas.h {
                break;
            }
            if let Some(face) = *thumb {
                fit_into(face, (x, y, cw, ch), out);
            }
            if let (true, Some((tex, tw, th))) = (self.is_ticked(i), self.tick_face) {
                out.push(Draw::Tex {
                    x: x + cw - TICK_INSET - tw as f32,
                    y: y + TICK_INSET,
                    w: tw as f32,
                    h: th as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            if i == self.index {
                outline(x, y, cw, ch, out);
            }
        }
    }

    fn draw_shot(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        match (self.face, self.shot_rect(canvas)) {
            (Some((tex, ..)), Some((x, y, w, h))) => out.push(Draw::Tex {
                x,
                y,
                w,
                h,
                tex,
                alpha: 1.0,
            }),
            _ => {
                if let Some(face) = self.thumb(self.index) {
                    fit_into(face, (0.0, 0.0, canvas.w, canvas.h), out);
                }
            }
        }
    }

    fn draw_corner(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        let plate = canvas.h - PLATE_H;
        if self.mode() != ShotMode::Zoomed {
            if let Some((tex, w)) = self.count {
                out.push(Draw::Tex {
                    x: canvas.w - EDGE - w as f32,
                    y: plate + (PLATE_H - HINT_H as f32) / 2.0,
                    w: w as f32,
                    h: HINT_H as f32,
                    tex,
                    alpha: 1.0,
                });
            }
            return;
        }
        let Some((fx, fy, fw, fh)) = self.view_frac(canvas) else {
            return;
        };
        let (x, y) = (canvas.w - EDGE - SPOT_W, plate + (PLATE_H - SPOT_H) / 2.0);
        out.push(Draw::Rect {
            x,
            y,
            w: SPOT_W,
            h: SPOT_H,
            colour: SPOT_BACK,
        });
        out.push(Draw::Rect {
            x: x + fx * SPOT_W,
            y: y + fy * SPOT_H,
            w: fw * SPOT_W,
            h: fh * SPOT_H,
            colour: SPOT_VIEW,
        });
    }
}

fn fit_into(
    (tex, w, h): (TexId, u32, u32),
    (x, y, cw, ch): (f32, f32, f32, f32),
    out: &mut Vec<Draw>,
) {
    let s = (cw / w as f32).min(ch / h as f32);
    let (sw, sh) = (w as f32 * s, h as f32 * s);
    out.push(Draw::Tex {
        x: x + (cw - sw) / 2.0,
        y: y + (ch - sh) / 2.0,
        w: sw,
        h: sh,
        tex,
        alpha: 1.0,
    });
}

fn outline(x: f32, y: f32, w: f32, h: f32, out: &mut Vec<Draw>) {
    let o = OUTLINE + OUTLINE_GAP;
    let rect = |x, y, w, h| Draw::Rect {
        x,
        y,
        w,
        h,
        colour: WHITE,
    };
    out.extend([
        rect(x - o, y - o, w + 2.0 * o, OUTLINE),
        rect(x - o, y + h + OUTLINE_GAP, w + 2.0 * o, OUTLINE),
        rect(x - o, y - OUTLINE_GAP, OUTLINE, h + 2.0 * OUTLINE_GAP),
        rect(
            x + w + OUTLINE_GAP,
            y - OUTLINE_GAP,
            OUTLINE,
            h + 2.0 * OUTLINE_GAP,
        ),
    ]);
}

pub fn thumbnail(rgba: &[u8], w: u32, h: u32, max_w: u32, max_h: u32) -> (Vec<u8>, u32, u32) {
    let scale = (max_w as f64 / w as f64)
        .min(max_h as f64 / h as f64)
        .min(1.0);
    let tw = ((w as f64 * scale + 1e-6).floor() as u32).max(1);
    let th = ((h as f64 * scale + 1e-6).floor() as u32).max(1);
    let span = |o: u32, t: u32, n: u32| {
        let a = (o as u64 * n as u64 / t as u64) as u32;
        let b = (((o + 1) as u64 * n as u64 / t as u64) as u32).clamp(a + 1, n);
        (a, b)
    };
    let mut out = Vec::with_capacity((tw * th * 4) as usize);
    for oy in 0..th {
        let (y0, y1) = span(oy, th, h);
        for ox in 0..tw {
            let (x0, x1) = span(ox, tw, w);
            let mut sum = [0u32; 4];
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = ((y * w + x) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += rgba[p + c] as u32;
                    }
                }
            }
            let n = (y1 - y0) * (x1 - x0);
            out.extend(sum.map(|v| ((v + n / 2) / n) as u8));
        }
    }
    (out, tw, th)
}
