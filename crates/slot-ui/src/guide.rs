use std::collections::HashMap;
use std::sync::OnceLock;

use crate::battery::Gauge;
use fontdue::{Font, FontSettings, Metrics};
use slot_gfx::{Canvas, Draw, TexId};
use slot_store::GuideSpot;

use crate::bitmaps::ButtonLegend;
use crate::footer::Printed;
use crate::hud::PLATE_H;
use crate::plate::{
    blit, hint_quad, hint_row, hint_width, Hint, UndoFace, HINT_H, TITLE_H, TITLE_W,
};
use crate::polaroids::{draw_album_chrome, plate, AlbumChrome};
use crate::power_menu::{MENU_INK, MENU_PAD};
use crate::quick_menu::{button_face, button_pair, quick_text_face, up_down_pair};

const MONO_TTF: &[u8] = include_bytes!("../assets/DejaVuSansMono.ttf");

pub const GUIDE_HELP_TITLE: &str = "Guide Help";

pub const GUIDE_HINTS: [(&str, &str); 2] = [("Y", "Close"), ("MENU", "Help")];
pub const GUIDE_HELP_HINT: (&str, &str) = ("B", "Back");

pub const GUIDE_ZOOMS: [f32; 8] = [10.0, 12.0, 14.0, 16.0, 18.0, 21.0, 24.0, 28.0];
pub const GUIDE_COLUMNS: usize = 80;
pub const PAN_COLS: usize = 8;

const WAYS_OUT: usize = 1;
const HELP_ROW_GAP: u32 = 6;
const HELP_LABEL_GAP: u32 = 18;
const EDGE: f32 = 16.0;
const GAP: f32 = 4.0;
const INK: [u8; 3] = [232, 232, 232];
const BAR_W: f32 = 3.0;
const BAR_X: f32 = 7.0;
const BAR_MIN: f32 = 16.0;
const TRACK: [f32; 4] = [1.0, 1.0, 1.0, 0.12];
const THUMB: [f32; 4] = [1.0, 1.0, 1.0, 0.6];

pub fn mono_font() -> Option<&'static Font> {
    static FONT: OnceLock<Option<Font>> = OnceLock::new();
    FONT.get_or_init(|| Font::from_bytes(MONO_TTF, FontSettings::default()).ok())
        .as_ref()
}

pub fn page_box(canvas: Canvas) -> (f32, f32, u32, u32) {
    let (x, y) = (EDGE, PLATE_H + GAP);
    let w = canvas.w - 2.0 * EDGE;
    let h = canvas.h - 2.0 * (PLATE_H + GAP);
    (x, y, w.max(1.0) as u32, h.max(1.0) as u32)
}

pub fn cell(px: f32) -> (f32, f32) {
    let Some(font) = mono_font() else {
        return (px * 0.6, px * 1.2);
    };
    let adv = font.metrics('M', px).advance_width;
    let line = font
        .horizontal_line_metrics(px)
        .map_or(px * 1.2, |m| m.new_line_size.ceil());
    (adv, line)
}

pub fn fitting_zoom(canvas: Canvas) -> u8 {
    let (_, _, w, _) = page_box(canvas);
    GUIDE_ZOOMS
        .iter()
        .rposition(|&px| cell(px).0 * GUIDE_COLUMNS as f32 <= w as f32)
        .unwrap_or(0) as u8
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct GuideView {
    pub line: usize,
    pub col: usize,
    pub zoom: u8,
    pub wrap: bool,
    pub w: u32,
    pub h: u32,
}

#[derive(Copy, Clone)]
struct Row {
    line: u32,
    from: u32,
    to: u32,
    indent: u32,
}

fn wrap_line(text: &str, cols: usize) -> usize {
    let Some((cut, ch)) = text.char_indices().nth(cols) else {
        return text.len();
    };
    if ch == ' ' {
        return cut;
    }
    match text[..cut].rfind(' ') {
        Some(space) if !text[..space].trim_start().is_empty() => space,
        _ => cut,
    }
}

fn wrap_rows(lines: &[String], cols: usize) -> Vec<Row> {
    let mut rows = Vec::with_capacity(lines.len());
    for (line, text) in lines.iter().enumerate() {
        let hang = (text.len() - text.trim_start().len()).min(cols / 2);
        let mut indent = 0;
        let mut from = 0;
        loop {
            let rest = &text[from..];
            let to = from + wrap_line(rest, cols - indent);
            rows.push(Row {
                line: line as u32,
                from: from as u32,
                to: to as u32,
                indent: indent as u32,
            });
            indent = hang;
            let next = text[to..].trim_start();
            if next.is_empty() {
                break;
            }
            from = text.len() - next.len();
        }
    }
    rows
}

pub struct Guide {
    lines: Vec<String>,
    wrap: bool,
    wrapped: Vec<Row>,
    widest: usize,
    line: usize,
    col: usize,
    zoom: u8,
    page: Option<(TexId, GuideView)>,
    hint_faces: Vec<TexId>,
    help: bool,
    help_face: Option<(TexId, u32, u32)>,
    help_title: Option<TexId>,
}

impl Guide {
    pub fn new(lines: Vec<String>, spot: Option<GuideSpot>, canvas: Canvas) -> Self {
        let widest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let spot = spot.unwrap_or(GuideSpot {
            line: 0,
            col: 0,
            zoom: fitting_zoom(canvas),
            wrap: false,
        });
        let mut g = Guide {
            lines,
            wrap: spot.wrap,
            wrapped: Vec::new(),
            widest,
            line: spot.line as usize,
            col: spot.col as usize,
            zoom: spot.zoom.min(GUIDE_ZOOMS.len() as u8 - 1),
            page: None,
            hint_faces: Vec::new(),
            help: false,
            help_face: None,
            help_title: None,
        };
        if g.wrap {
            g.reflow(spot.line, spot.col, canvas);
        }
        g.clamp(canvas);
        g
    }

    pub fn spot(&self) -> GuideSpot {
        let (line, col) = self.anchor();
        GuideSpot {
            line,
            col,
            zoom: self.zoom,
            wrap: self.wrap,
        }
    }

    pub fn wrap(&self) -> bool {
        self.wrap
    }

    pub fn toggle_wrap(&mut self, canvas: Canvas) {
        let (line, col) = self.anchor();
        self.wrap = !self.wrap;
        if self.wrap {
            self.reflow(line, col, canvas);
        } else {
            self.line = line as usize;
            self.col = 0;
        }
        self.clamp(canvas);
    }

    fn anchor(&self) -> (u32, u32) {
        match self.wrapped.get(self.line).filter(|_| self.wrap) {
            Some(row) => (row.line, row.from),
            None => (self.line as u32, self.col as u32),
        }
    }

    fn reflow(&mut self, line: u32, from: u32, canvas: Canvas) {
        self.wrapped = wrap_rows(&self.lines, self.cols(canvas));
        self.col = 0;
        self.line = self
            .wrapped
            .partition_point(|r| (r.line, r.from) <= (line, from))
            .saturating_sub(1);
    }

    fn count(&self) -> usize {
        match self.wrap {
            true => self.wrapped.len(),
            false => self.lines.len(),
        }
    }

    pub fn visible_text(&self, row: usize) -> Option<&str> {
        self.row_text(self.line + row).map(|(_, text)| text)
    }

    pub fn visible_indent(&self, row: usize) -> Option<usize> {
        self.row_text(self.line + row).map(|(indent, _)| indent)
    }

    fn row_text(&self, at: usize) -> Option<(usize, &str)> {
        if !self.wrap {
            return self.lines.get(at).map(|l| (0, l.as_str()));
        }
        let row = self.wrapped.get(at)?;
        let text = &self.lines[row.line as usize][row.from as usize..row.to as usize];
        Some((row.indent as usize, text))
    }

    pub fn px(&self) -> f32 {
        GUIDE_ZOOMS[self.zoom as usize]
    }

    pub fn rows(&self, canvas: Canvas) -> usize {
        let (_, _, _, h) = page_box(canvas);
        ((h as f32 / cell(self.px()).1) as usize).max(1)
    }

    pub fn cols(&self, canvas: Canvas) -> usize {
        let (_, _, w, _) = page_box(canvas);
        ((w as f32 / cell(self.px()).0) as usize).max(1)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn view(&self, canvas: Canvas) -> GuideView {
        let (_, _, w, h) = page_box(canvas);
        GuideView {
            line: self.line,
            col: self.col,
            zoom: self.zoom,
            wrap: self.wrap,
            w,
            h,
        }
    }

    fn clamp(&mut self, canvas: Canvas) {
        let last_line = self.count().saturating_sub(self.rows(canvas));
        let last_col = match self.wrap {
            true => 0,
            false => self.widest.saturating_sub(self.cols(canvas)),
        };
        self.line = self.line.min(last_line);
        self.col = self.col.min(last_col);
    }

    pub fn scroll(&mut self, by: isize, canvas: Canvas) {
        self.line = self.line.saturating_add_signed(by);
        self.clamp(canvas);
    }

    pub fn page(&mut self, by: isize, canvas: Canvas) {
        let step = self.rows(canvas).saturating_sub(1).max(1) as isize;
        self.scroll(by * step, canvas);
    }

    pub fn pan(&mut self, by: isize, canvas: Canvas) {
        self.col = self.col.saturating_add_signed(by * PAN_COLS as isize);
        self.clamp(canvas);
    }

    pub fn zoom_by(&mut self, by: i8, canvas: Canvas) -> bool {
        let to = (self.zoom as i16 + by as i16).clamp(0, GUIDE_ZOOMS.len() as i16 - 1) as u8;
        if to == self.zoom {
            return false;
        }
        let (line, col) = self.anchor();
        self.zoom = to;
        if self.wrap {
            self.reflow(line, col, canvas);
        }
        self.clamp(canvas);
        true
    }

    pub fn set_hint_faces(&mut self, faces: Vec<TexId>) {
        self.hint_faces = faces;
    }

    pub fn set_help_face(&mut self, face: (TexId, u32, u32), title: TexId) {
        self.help_face = Some(face);
        self.help_title = Some(title);
    }

    pub fn set_page_face(&mut self, tex: TexId, view: GuideView) {
        self.page = Some((tex, view));
    }

    pub fn hints(&self) -> Vec<Hint> {
        hint_row(&GUIDE_HINTS)
    }

    pub fn help(&self) -> bool {
        self.help
    }

    pub fn toggle_help(&mut self) {
        self.help = !self.help;
    }

    pub fn render(&self, canvas: Canvas, glyphs: &mut GuideGlyphs) -> UndoFace {
        let view = self.view(canvas);
        let (w, h) = (view.w, view.h);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for px in rgba.chunks_exact_mut(4) {
            px[3] = 255;
        }
        let Some(font) = mono_font() else {
            return UndoFace { rgba, w, h };
        };
        let px = self.px();
        let (adv, line_h) = cell(px);
        let ascent = font.horizontal_line_metrics(px).map_or(px, |m| m.ascent);
        let cols = self.cols(canvas);
        for row in 0..self.rows(canvas) {
            let Some((indent, line)) = self.row_text(self.line + row) else {
                break;
            };
            let baseline = row as f32 * line_h + ascent;
            for (i, ch) in line.chars().skip(self.col).take(cols + 1).enumerate() {
                if ch == ' ' {
                    continue;
                }
                let (m, cov) = glyphs.get(font, ch, px);
                let x = ((i + indent) as f32 * adv).round() as i32 + m.xmin;
                let y = (baseline - (m.height as f32 + m.ymin as f32)).round() as i32;
                ink(&mut rgba, w, h, x, y, cov, m.width as u32, m.height as u32);
            }
        }
        UndoFace { rgba, w, h }
    }

    #[allow(clippy::too_many_arguments)]
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
        if self.help {
            return self.draw_help(canvas, out);
        }
        self.draw_page(canvas, out);
        let hints = self.hints();
        draw_album_chrome(
            canvas,
            AlbumChrome {
                title: None,
                hints: &hints,
                hint_faces: &self.hint_faces,
                ways_out: WAYS_OUT,
                dots: None,
            },
            gauge,
            bolt,
            clock,
            out,
        );
    }

    fn draw_page(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        let (x, y, _, _) = page_box(canvas);
        if let Some((tex, view)) = self.page {
            out.push(Draw::Tex {
                x,
                y,
                w: view.w as f32,
                h: view.h as f32,
                tex,
                alpha: 1.0,
            });
        }
        self.draw_bar(canvas, out);
    }

    fn draw_help(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        draw_help_screen(
            canvas,
            self.help_title,
            self.hint_faces.get(GUIDE_HINTS.len()).copied(),
            self.help_face,
            out,
        );
    }

    fn draw_bar(&self, canvas: Canvas, out: &mut Vec<Draw>) {
        let rows = self.rows(canvas);
        if self.count() <= rows {
            return;
        }
        let (_, y, _, h) = page_box(canvas);
        let (x, h) = (canvas.w - BAR_X - BAR_W, h as f32);
        out.push(Draw::Rect {
            x,
            y,
            w: BAR_W,
            h,
            colour: TRACK,
        });
        let n = self.count() as f32;
        let thumb = (h * rows as f32 / n).max(BAR_MIN);
        let last = (self.count() - rows) as f32;
        out.push(Draw::Rect {
            x,
            y: y + (h - thumb) * self.line as f32 / last,
            w: BAR_W,
            h: thumb,
            colour: THUMB,
        });
    }
}

pub(crate) fn draw_help_screen(
    canvas: Canvas,
    title: Option<TexId>,
    back: Option<TexId>,
    face: Option<(TexId, u32, u32)>,
    out: &mut Vec<Draw>,
) {
    out.push(plate(canvas, 0.0));
    if let Some(tex) = title {
        out.push(Draw::Tex {
            x: canvas.centred(TITLE_W as f32),
            y: (PLATE_H - TITLE_H as f32) / 2.0,
            w: TITLE_W as f32,
            h: TITLE_H as f32,
            tex,
            alpha: 1.0,
        });
    }
    let top = canvas.h - PLATE_H;
    out.push(plate(canvas, top));
    let (key, label) = GUIDE_HELP_HINT;
    out.push(hint_quad(
        EDGE,
        top + (PLATE_H - HINT_H as f32) / 2.0,
        hint_width(key, label) as f32,
        back,
    ));
    let Some((tex, w, h)) = face else {
        return;
    };
    out.push(Draw::Tex {
        x: canvas.centred(w as f32).floor(),
        y: ((canvas.h - h as f32) / 2.0).floor(),
        w: w as f32,
        h: h as f32,
        tex,
        alpha: 1.0,
    });
}

pub fn guide_help_face() -> UndoFace {
    help_rows_face(vec![
        (up_down_pair(), "Scroll"),
        (button_pair(ButtonLegend::L1, ButtonLegend::R1), "Page"),
        (button_pair(ButtonLegend::Left, ButtonLegend::Right), "Pan"),
        (button_pair(ButtonLegend::A, ButtonLegend::B), "Zoom"),
        (button_face(ButtonLegend::X), "Wrap"),
    ])
}

pub(crate) fn help_rows_face(rows: Vec<(UndoFace, &str)>) -> UndoFace {
    let rows: Vec<(UndoFace, UndoFace)> = rows
        .into_iter()
        .map(|(art, label)| (art, quick_text_face(label, MENU_INK)))
        .collect();
    let art_w = rows.iter().map(|(a, _)| a.w).max().unwrap_or(0);
    let label_w = rows.iter().map(|(_, l)| l.w - MENU_PAD).max().unwrap_or(0);
    let row_h = rows.iter().map(|(a, l)| a.h.max(l.h)).max().unwrap_or(0);
    let w = art_w + HELP_LABEL_GAP + label_w;
    let h = rows.len() as u32 * (row_h + HELP_ROW_GAP) - HELP_ROW_GAP;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for (i, (art, label)) in rows.iter().enumerate() {
        let top = i as u32 * (row_h + HELP_ROW_GAP);
        blit(
            &mut rgba,
            w,
            &art.rgba,
            art.w,
            art.h,
            art_w - art.w,
            top + (row_h - art.h) / 2,
        );
        let lx = art_w + HELP_LABEL_GAP - MENU_PAD;
        let cut = label.w.min(w - lx);
        let mut trimmed = Vec::with_capacity((cut * label.h * 4) as usize);
        for y in 0..label.h {
            let from = (y * label.w * 4) as usize;
            trimmed.extend_from_slice(&label.rgba[from..from + (cut * 4) as usize]);
        }
        blit(
            &mut rgba,
            w,
            &trimmed,
            cut,
            label.h,
            lx,
            top + (row_h - label.h) / 2,
        );
    }
    UndoFace { rgba, w, h }
}

#[derive(Default)]
pub struct GuideGlyphs {
    px: f32,
    cached: HashMap<char, (Metrics, Vec<u8>)>,
}

impl GuideGlyphs {
    fn get(&mut self, font: &Font, ch: char, px: f32) -> &(Metrics, Vec<u8>) {
        if self.px != px {
            self.px = px;
            self.cached.clear();
        }
        self.cached
            .entry(ch)
            .or_insert_with(|| font.rasterize(ch, px))
    }
}

#[allow(clippy::too_many_arguments)]
fn ink(dst: &mut [u8], dst_w: u32, dst_h: u32, x: i32, y: i32, cov: &[u8], gw: u32, gh: u32) {
    for gy in 0..gh {
        let dy = y + gy as i32;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for gx in 0..gw {
            let dx = x + gx as i32;
            let a = cov[(gy * gw + gx) as usize] as u32;
            if dx < 0 || dx >= dst_w as i32 || a == 0 {
                continue;
            }
            let i = (dy as u32 * dst_w + dx as u32) as usize * 4;
            for c in 0..3 {
                let under = dst[i + c] as u32;
                dst[i + c] = ((INK[c] as u32 * a + under * (255 - a) + 127) / 255) as u8;
            }
        }
    }
}
