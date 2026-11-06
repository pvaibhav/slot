use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use crate::app::shift_down;
use slot_gfx::{paper_flap_shows, Canvas, Compositor, Draw, GfxError, TexId, OUT_H, OUT_W};
use slot_input::Millis;
use slot_ui::{
    cover_bytes, default_background, ease, receipt_blank, receipt_face, ReceiptText,
    UndoFace,
};

const SLIDE: Millis = 260;
const RISE: Millis = 640;
const LAMP: Millis = 260;
const PRINT: Millis = 3200;
const HOLD: Millis = 350;
const RIP: Millis = 960;
const CARRY: Millis = 340;
const GAP: Millis = 140;
const RETURN: Millis = 560;
const CLEAR: Millis = 380;
const DARK: Millis = 60;
const DROP: Millis = 480;
const TEAR_INTO_RIP: Millis = 710;

const RISE_AT: Millis = SLIDE;
const LAMP_AT: Millis = RISE_AT + RISE;
const PRINT_AT: Millis = LAMP_AT + LAMP;
const HOLD_AT: Millis = PRINT_AT + PRINT;
const RIP_AT: Millis = HOLD_AT + HOLD;
const CARRY_AT: Millis = RIP_AT + RIP;
const GAP_AT: Millis = CARRY_AT + CARRY;
const RETURN_AT: Millis = GAP_AT + GAP;
const REST_AT: Millis = RETURN_AT + RETURN;

const STEPS: f32 = 62.0;
const OFF_Y: f32 = 490.0;
const SHEET_W: f32 = 226.0;
const SHEET_H: f32 = 425.0;
const CUT: f32 = 407.0;
const FEED_X: f32 = 105.0 + SHEET_W;
const FEED_Y: f32 = 302.0;
const REST_X: f32 = 442.0 + SHEET_W;
const REST_Y: f32 = 37.0 + SHEET_H;
const BELOW: f32 = 960.0;
const CARRY_DX: f32 = 170.0;
const CARRY_DY: f32 = 640.0;
const FOLD_FROM: f32 = -40.0;
const FOLD_TO: f32 = 500.0;
const FLAT: f32 = -60.0;
const WHOLE_CUT: f32 = 400.0;
const NO_CUT: f32 = -1.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Buzz,
    Tear,
    BuzzOff,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paper {
    pub pivot: (f32, f32),
    pub scale: f32,
    pub lift: f32,
    pub fold: f32,
    pub tear: f32,
    pub bow: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintFrame {
    pub slide: f32,
    pub printer_y: f32,
    pub lamp_on: bool,
    pub paper: Option<Paper>,
    pub paper_in_front: bool,
}

pub struct PrintScene {
    start: Millis,
    stop: Option<Millis>,
    buzzed: bool,
    torn: bool,
    hushed: bool,
    skipped: bool,
    ahead: Millis,
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn eout(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

fn ein(t: f32) -> f32 {
    t * t * t
}

fn ratio(a: Millis, b: Millis) -> f32 {
    a as f32 / b as f32
}

fn held(y: f32, lift: f32, fold: f32, tear: f32) -> Paper {
    Paper {
        pivot: (FEED_X, y),
        scale: 1.0,
        lift,
        fold,
        tear,
        bow: false,
    }
}

fn resting(y: f32, lift: f32) -> Paper {
    Paper {
        pivot: (REST_X, y),
        scale: 1.0,
        lift,
        fold: FLAT,
        tear: WHOLE_CUT,
        bow: true,
    }
}

fn ripping(a: Millis) -> Paper {
    let p = ratio(a, RIP);
    let fold = lerp(FOLD_FROM, FOLD_TO, p * p * (1.6 - 0.6 * p));
    let tear = ((fold - 0.78 * CUT) / 0.62).max(NO_CUT);
    held(FEED_Y, lerp(4.0, 10.0, p), fold, tear)
}

fn carried(a: Millis) -> Paper {
    let e = ratio(a, CARRY);
    let m = e * (0.55 + 0.45 * e);
    Paper {
        pivot: (FEED_X + CARRY_DX * m, FEED_Y + CARRY_DY * m),
        scale: 1.0 + 0.06 * e,
        lift: 10.0 + 6.0 * e,
        fold: FOLD_TO,
        tear: WHOLE_CUT,
        bow: false,
    }
}

fn opening(t: Millis) -> PrintFrame {
    let up = PrintFrame {
        slide: 0.0,
        printer_y: 0.0,
        lamp_on: true,
        paper: None,
        paper_in_front: false,
    };
    let front = |paper| PrintFrame {
        paper: Some(paper),
        paper_in_front: true,
        ..up
    };
    let back = |paper| PrintFrame {
        paper: Some(paper),
        ..up
    };
    match t {
        t if t < RISE_AT => PrintFrame {
            slide: 1.0 - ease(ratio(t, SLIDE)),
            printer_y: OFF_Y,
            lamp_on: false,
            ..up
        },
        t if t < LAMP_AT => PrintFrame {
            printer_y: OFF_Y * (1.0 - eout(ratio(t - RISE_AT, RISE))),
            lamp_on: false,
            ..up
        },
        t if t < PRINT_AT => PrintFrame {
            lamp_on: ratio(t - LAMP_AT, LAMP) > 0.45,
            ..up
        },
        t if t < HOLD_AT => {
            let step = (ratio(t - PRINT_AT, PRINT) * STEPS).floor();
            back(held(
                FEED_Y + SHEET_H - SHEET_H * step / STEPS,
                4.0,
                FLAT,
                NO_CUT,
            ))
        }
        t if t < RIP_AT => back(held(FEED_Y, 4.0, FLAT, NO_CUT)),
        t if t < CARRY_AT => back(ripping(t - RIP_AT)),
        t if t < GAP_AT => front(carried(t - CARRY_AT)),
        t if t < RETURN_AT => PrintFrame {
            paper_in_front: true,
            ..up
        },
        t if t < REST_AT => {
            let e = eout(ratio(t - RETURN_AT, RETURN));
            front(resting(lerp(BELOW, REST_Y, e), lerp(16.0, 9.0, e)))
        }
        _ => front(resting(REST_Y, 9.0)),
    }
}

fn clearing(base: &PrintFrame) -> Millis {
    match base.paper.is_some() || base.lamp_on {
        true => CLEAR + DARK,
        false => 0,
    }
}

fn closing_time(base: &PrintFrame) -> Millis {
    if base.slide > 0.0 {
        return ((1.0 - base.slide) * SLIDE as f32).ceil() as Millis;
    }
    clearing(base) + DROP + SLIDE
}

fn cleared(base: &PrintFrame, paper: Paper, a: f32) -> Paper {
    let y = paper.pivot.1;
    let to = match base.paper_in_front {
        true => BELOW,
        false => y + BELOW - REST_Y,
    };
    Paper {
        pivot: (paper.pivot.0, lerp(y, to, ein(a))),
        ..paper
    }
}

fn closing(base: &PrintFrame, a: Millis) -> PrintFrame {
    if base.slide > 0.0 {
        return PrintFrame {
            slide: (base.slide + ratio(a, SLIDE)).min(1.0),
            ..*base
        };
    }
    let clear = clearing(base);
    if clear > 0 && a < CLEAR {
        let p = ratio(a, CLEAR);
        return PrintFrame {
            paper: base.paper.map(|paper| cleared(base, paper, p)),
            ..*base
        };
    }
    let gone = PrintFrame {
        lamp_on: false,
        paper: None,
        ..*base
    };
    if a < clear {
        return gone;
    }
    let a = a - clear;
    if a < DROP {
        let y = base.printer_y;
        return PrintFrame {
            printer_y: lerp(y, OFF_Y, ein(ratio(a, DROP))),
            ..gone
        };
    }
    PrintFrame {
        slide: ease(ratio(a - DROP, SLIDE).min(1.0)),
        printer_y: OFF_Y,
        ..gone
    }
}

fn closes_from(stop: Millis) -> Millis {
    match (RIP_AT..GAP_AT).contains(&stop) {
        true => GAP_AT,
        false => stop,
    }
}

impl PrintScene {
    pub fn new(now: Millis) -> PrintScene {
        PrintScene {
            start: now,
            stop: None,
            buzzed: false,
            torn: false,
            hushed: false,
            skipped: false,
            ahead: 0,
        }
    }

    fn elapsed(&self, now: Millis) -> Millis {
        now.saturating_sub(self.start) + self.ahead
    }

    pub fn skip(&mut self, now: Millis) {
        let t = self.elapsed(now);
        if self.stop.is_some() || t >= REST_AT {
            return;
        }
        self.ahead += REST_AT - t;
        self.skipped = true;
        self.torn = true;
        if !self.buzzed {
            self.buzzed = true;
            self.hushed = true;
        }
    }

    pub fn stop(&mut self, now: Millis) {
        if self.stop.is_none() {
            self.stop = Some(self.elapsed(now));
        }
    }

    pub fn hold_print(&mut self, now: Millis) {
        if self.stop.is_none() && self.elapsed(now) >= PRINT_AT {
            self.start = now - (PRINT_AT - 1);
        }
    }

    pub fn frame(&self, now: Millis) -> PrintFrame {
        let t = self.elapsed(now);
        match self.stop.map(closes_from) {
            Some(from) if t >= from => closing(&opening(from), t - from),
            _ => opening(t),
        }
    }

    pub fn done(&self, now: Millis) -> bool {
        let t = self.elapsed(now);
        self.stop
            .map(closes_from)
            .is_some_and(|from| t >= from + closing_time(&opening(from)))
    }

    pub fn cues(&mut self, now: Millis) -> Vec<Cue> {
        let t = self.elapsed(now);
        let mut out = Vec::new();
        if !self.buzzed && self.stop.is_none() && t >= PRINT_AT {
            self.buzzed = true;
            out.push(Cue::Buzz);
        }
        let ripped = self.stop.is_none_or(|s| s >= RIP_AT);
        if !self.torn && ripped && t >= RIP_AT + TEAR_INTO_RIP {
            self.torn = true;
            out.push(Cue::Tear);
        }
        if self.buzzed && (self.stop.is_some() || self.skipped) && !self.hushed {
            self.hushed = true;
            out.push(Cue::BuzzOff);
        }
        out
    }
}

const PRINTER_PNG: &[u8] = include_bytes!("../../../assets/server/printer.png");
const FRONT_PNG: &[u8] = include_bytes!("../../../assets/server/printer_front.png");
const LAMP_R: f32 = 9.0;
const LAMP_XY: (f32, f32) = (355.0, 349.0);
const LAMP_OFF: [u8; 3] = [0x14, 0x16, 0x1b];
const SCENE_W: f32 = OUT_W as f32;

pub struct StageArt {
    printer: Vec<u8>,
    front: Vec<u8>,
    lamp: UndoFace,
}

pub struct PrintArt {
    paper: UndoFace,
}

pub enum PaperJob {
    Code {
        modules: Vec<Vec<bool>>,
        text: ReceiptText,
    },
    Blank(Vec<String>),
}

fn lamp_disc() -> UndoFace {
    let side = 2 * LAMP_R as u32 + 2;
    let mid = side as f32 / 2.0;
    let mut rgba = Vec::with_capacity((side * side * 4) as usize);
    for y in 0..side {
        for x in 0..side {
            let r = (x as f32 + 0.5 - mid).hypot(y as f32 + 0.5 - mid);
            let cover = (LAMP_R + 0.5 - r).clamp(0.0, 1.0);
            rgba.extend_from_slice(&LAMP_OFF);
            rgba.push((cover * 255.0) as u8);
        }
    }
    UndoFace {
        rgba,
        w: side,
        h: side,
    }
}

pub fn print_art(job: PaperJob) -> Option<PrintArt> {
    let paper = match job {
        PaperJob::Code { modules, text } => receipt_face(&modules, &text),
        PaperJob::Blank(lines) => {
            let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
            receipt_blank(&lines)
        }
    };
    Some(PrintArt { paper })
}

pub fn stage_art() -> Option<StageArt> {
    Some(StageArt {
        printer: cover_bytes(PRINTER_PNG, OUT_W, OUT_H)?,
        front: cover_bytes(FRONT_PNG, OUT_W, OUT_H)?,
        lamp: lamp_disc(),
    })
}

pub enum PrintArtState {
    Pending,
    Ready(PrintArt),
    Failed,
}

pub struct PrintArtWorker {
    built: Receiver<Option<PrintArt>>,
    taken: bool,
}

impl PrintArtWorker {
    pub fn spawn(job: PaperJob) -> PrintArtWorker {
        PrintArtWorker::spawn_with(move || print_art(job))
    }

    pub fn spawn_with(build: impl FnOnce() -> Option<PrintArt> + Send + 'static) -> PrintArtWorker {
        let (outbox, built) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("slot-print-art".into())
            .spawn(move || {
                let _ = outbox.send(build());
            });
        if let Err(e) = spawned {
            eprintln!("slot: print art: worker thread failed to start: {e}");
        }
        PrintArtWorker {
            built,
            taken: false,
        }
    }

    pub fn take(&mut self) -> PrintArtState {
        match self.built.try_recv() {
            Ok(Some(art)) => {
                self.taken = true;
                PrintArtState::Ready(art)
            }
            Err(TryRecvError::Empty) => PrintArtState::Pending,
            Err(TryRecvError::Disconnected) if self.taken => PrintArtState::Pending,
            Ok(None) | Err(TryRecvError::Disconnected) => PrintArtState::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrintTextures {
    wood: TexId,
    printer: TexId,
    front: TexId,
    paper: TexId,
    lamp: (TexId, u32),
}

impl PrintTextures {
    pub fn upload(c: &mut Compositor, art: &StageArt) -> Result<PrintTextures, GfxError> {
        c.prepare_paper()?;
        Ok(PrintTextures {
            wood: c.create_texture(OUT_W, OUT_H, default_background().unwrap_or_default()),
            printer: c.create_texture(OUT_W, OUT_H, &art.printer),
            front: c.create_texture(OUT_W, OUT_H, &art.front),
            paper: c.create_texture(1, 1, &[0; 4]),
            lamp: (
                c.create_texture(art.lamp.w, art.lamp.h, &art.lamp.rgba),
                art.lamp.w,
            ),
        })
    }

    pub fn paper(&self) -> TexId {
        self.paper
    }

    pub fn set_paper(&self, c: &mut Compositor, art: &PrintArt) {
        c.update_texture(self.paper, art.paper.w, art.paper.h, &art.paper.rgba);
    }
}

fn paper_draw(tex: TexId, p: &Paper, shift: f32, flap: bool) -> Draw {
    Draw::Paper {
        tex,
        pivot: [p.pivot.0 + shift, p.pivot.1],
        scale: p.scale,
        lift: p.lift,
        fold: p.fold,
        tear: p.tear,
        bow: p.bow,
        flap,
    }
}

pub fn print_draws(f: &PrintFrame, tex: &PrintTextures, canvas: Canvas, out: &mut Vec<Draw>) {
    let from = out.len();
    if canvas.h > OUT_H as f32 {
        let wide = canvas.w / canvas.h > SCENE_W / OUT_H as f32;
        let uv = match wide {
            true => {
                let shown = SCENE_W / OUT_H as f32 * canvas.h / canvas.w;
                [0.0, (1.0 - shown) / 2.0, 1.0, shown]
            }
            false => {
                let shown = canvas.w / canvas.h * OUT_H as f32 / SCENE_W;
                [(1.0 - shown) / 2.0, 0.0, shown, 1.0]
            }
        };
        out.push(Draw::Part {
            x: 0.0,
            y: 0.0,
            w: canvas.w,
            h: canvas.h,
            tex: tex.wood,
            alpha: 1.0,
            uv,
        });
    }
    let scene = out.len();
    scene_draws(f, tex, canvas, out);
    shift_down(&mut out[scene..], canvas.inset_y());
    shift_down(&mut out[from..], -f.slide * canvas.h);
}

fn scene_draws(f: &PrintFrame, tex: &PrintTextures, canvas: Canvas, out: &mut Vec<Draw>) {
    let shift = canvas.centred(SCENE_W);
    let (w, h) = (SCENE_W, OUT_H as f32);
    let layer = |tex, y| Draw::Tex {
        x: shift,
        y,
        w,
        h,
        tex,
        alpha: 1.0,
    };
    out.push(layer(tex.wood, 0.0));
    if f.slide > 0.0 {
        return;
    }
    out.push(layer(tex.printer, f.printer_y));
    if !f.lamp_on {
        let (lamp, side) = tex.lamp;
        let side = side as f32;
        out.push(Draw::Tex {
            x: shift + LAMP_XY.0 - side / 2.0,
            y: f.printer_y + LAMP_XY.1 - side / 2.0,
            w: side,
            h: side,
            tex: lamp,
            alpha: 1.0,
        });
    }
    let Some(paper) = &f.paper else {
        return;
    };
    out.push(paper_draw(tex.paper, paper, shift, false));
    if !f.paper_in_front {
        out.push(layer(tex.front, f.printer_y));
    }
    if paper_flap_shows(paper.fold) {
        out.push(paper_draw(tex.paper, paper, shift, true));
    }
}
