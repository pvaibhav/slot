use slot_input::Millis;
use slot_ui::{
    footer_ink, small_prompt_legends, text, Draw, UndoFace, ADAPTER_BASE_X, LINK_HOST_INK, PLUG_H,
    PLUG_TIP_X,
};

use crate::ble::Heard;
use crate::link_kind::LinkKind;
use crate::link_screen::Sprite;

pub const CARD_W: u32 = 424;
pub const CARD_H: u32 = 334;
const CARD_TOP: f32 = 50.0;
const DESIGN_H: f32 = 480.0;
const RADIUS: f32 = 14.0;
const BORDER: f32 = 2.0;
const FILL: [u8; 3] = [0x08, 0x07, 0x0c];
const EDGE: [u8; 3] = [0x2a, 0x27, 0x33];
const SCRIM: f32 = 0.62;
const PLUG_SHOWN_H: f32 = 140.0;
const LINE_ONE_PX: f32 = 22.0;
const LINE_TWO_PX: f32 = 26.0;
const LINE_MIN_PX: f32 = 14.0;
const LINE_MAX_W: f32 = 380.0;
const LINE_ONE_H: u32 = 32;
const LINE_TWO_H: u32 = 38;
const LINE_ONE_MID: f32 = 176.0;
const LINE_TWO_MID: f32 = 216.0;
const LEGEND_MID: f32 = 299.0;
const LEGEND_ROW_W: f32 = 340.0;
const WHITE: [u8; 3] = [0xff, 0xff, 0xff];
pub const NO_NAME: &str = "Player 1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InviteCard {
    pub heard: Heard,
    pub stem: String,
    pub title: String,
    pub shown_at: Millis,
}

impl InviteCard {
    pub fn nickname(&self) -> &str {
        match self.heard.nickname.as_deref() {
            Some(name) if !name.trim().is_empty() => name,
            _ => NO_NAME,
        }
    }

    pub fn session(&self) -> u16 {
        self.heard.advert.session
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InviteFaces {
    pub card: Sprite,
    pub line_one: Sprite,
    pub line_two: Sprite,
    pub not_now: Sprite,
    pub join: Sprite,
    pub art: Option<(Sprite, LinkKind)>,
}

fn empty() -> UndoFace {
    UndoFace {
        rgba: Vec::new(),
        w: 0,
        h: 0,
    }
}

fn line(text: &str, px: f32) -> Option<text::Layout> {
    let font = text::label_font()?;
    Some(text::fit(font, text, LINE_MAX_W, 1, px, LINE_MIN_PX))
}

pub fn line_one_face(nickname: &str) -> UndoFace {
    let (Some(font), Some(whole)) = (
        text::label_font(),
        line(&format!("{nickname} wants to play"), LINE_ONE_PX),
    ) else {
        return empty();
    };
    let width = |s: &str| text::line_width(font, s, whole.px, whole.tracking);
    let name = nickname.to_uppercase();
    let rest = " WANTS TO PLAY".to_string();
    let full = width(&format!("{name}{rest}"));
    let w = (full.ceil() as u32).clamp(1, LINE_MAX_W as u32);
    let h = LINE_ONE_H;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let pen = ((w as f32 - full) / 2.0).max(0.0);
    let part = |s: String| text::Layout {
        lines: vec![s],
        px: whole.px,
        tracking: whole.tracking,
    };
    let rest_pen = pen + width(&name) + whole.tracking;
    text::draw_left(&mut rgba, w, h, &part(name), pen, LINK_HOST_INK);
    text::draw_left(&mut rgba, w, h, &part(rest), rest_pen, WHITE);
    UndoFace { rgba, w, h }
}

pub fn line_two_face(title: &str) -> UndoFace {
    let (Some(font), Some(layout)) = (text::label_font(), line(title, LINE_TWO_PX)) else {
        return empty();
    };
    let ink = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = (ink.ceil() as u32).clamp(1, LINE_MAX_W as u32);
    let h = LINE_TWO_H;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    text::draw_centred(&mut rgba, w, h, &layout, WHITE);
    UndoFace { rgba, w, h }
}

pub fn legend_faces() -> [UndoFace; 2] {
    small_prompt_legends(footer_ink(false), "Not now", "Join")
}

fn rounded_distance(x: f32, y: f32, w: f32, h: f32, r: f32) -> f32 {
    let qx = (x - w / 2.0).abs() - (w / 2.0 - r);
    let qy = (y - h / 2.0).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

pub fn card_face() -> UndoFace {
    let (w, h) = (CARD_W, CARD_H);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let (fw, fh) = (w as f32, h as f32);
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let d = rounded_distance(px, py, fw, fh, RADIUS);
            let outer = (0.5 - d).clamp(0.0, 1.0);
            if outer <= 0.0 {
                continue;
            }
            let inner = (0.5 - (d + BORDER)).clamp(0.0, 1.0);
            let o = ((y * w + x) * 4) as usize;
            for c in 0..3 {
                let mixed = FILL[c] as f32 * inner + EDGE[c] as f32 * (1.0 - inner);
                rgba[o + c] = mixed.round() as u8;
            }
            rgba[o + 3] = (outer * 255.0).round() as u8;
        }
    }
    UndoFace { rgba, w, h }
}

pub fn card_origin(screen: (u32, u32)) -> (f32, f32) {
    let (sw, sh) = (screen.0 as f32, screen.1 as f32);
    let x = ((sw - CARD_W as f32) / 2.0).floor();
    let y = ((sh - DESIGN_H) / 2.0 + CARD_TOP).max(0.0).floor();
    (x, y)
}

fn tex(out: &mut Vec<Draw>, s: Sprite, x: f32, y: f32, w: f32, h: f32) {
    out.push(Draw::Tex {
        x: x.round(),
        y: y.round(),
        w,
        h,
        tex: s.tex,
        alpha: 1.0,
    });
}

fn centred_on(out: &mut Vec<Draw>, s: Sprite, mid_x: f32, mid_y: f32) {
    let (w, h) = (s.w as f32, s.h as f32);
    tex(out, s, mid_x - w / 2.0, mid_y - h / 2.0, w, h);
}

pub fn draw_invite_card(out: &mut Vec<Draw>, faces: &InviteFaces, screen: (u32, u32)) {
    out.push(Draw::Rect {
        x: 0.0,
        y: 0.0,
        w: screen.0 as f32,
        h: screen.1 as f32,
        colour: [0.0, 0.0, 0.0, SCRIM],
    });
    let (x, y) = card_origin(screen);
    let (cw, ch) = (faces.card.w as f32, faces.card.h as f32);
    tex(out, faces.card, x, y, cw, ch);
    let mid = x + cw / 2.0;
    if let Some((art, kind)) = faces.art {
        let scale = PLUG_SHOWN_H / PLUG_H as f32;
        let tip_x = match kind {
            LinkKind::Cable => PLUG_TIP_X,
            LinkKind::Wireless => ADAPTER_BASE_X,
        };
        let (w, h) = (
            (art.w as f32 * scale).round(),
            (art.h as f32 * scale).round(),
        );
        let tip_y = y + BORDER + PLUG_SHOWN_H;
        tex(out, art, mid - tip_x * scale, tip_y - h, w, h);
    }
    centred_on(out, faces.line_one, mid, y + LINE_ONE_MID);
    centred_on(out, faces.line_two, mid, y + LINE_TWO_MID);
    let row_left = mid - LEGEND_ROW_W / 2.0;
    let row_right = mid + LEGEND_ROW_W / 2.0;
    let legend_y = y + LEGEND_MID;
    let (b, a) = (faces.not_now, faces.join);
    tex(
        out,
        b,
        row_left,
        legend_y - b.h as f32 / 2.0,
        b.w as f32,
        b.h as f32,
    );
    tex(
        out,
        a,
        row_right - a.w as f32,
        legend_y - a.h as f32 / 2.0,
        a.w as f32,
        a.h as f32,
    );
}
