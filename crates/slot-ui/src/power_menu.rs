use crate::plate::UndoFace;
use crate::text;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum PowerChoice {
    Restart,
    PowerOff,
    Repair,
}

impl PowerChoice {
    pub const ALL: [PowerChoice; 3] = [
        PowerChoice::Restart,
        PowerChoice::PowerOff,
        PowerChoice::Repair,
    ];

    pub fn index(self) -> usize {
        self as usize
    }
}

pub(crate) const MENU_PX: f32 = 30.0;
const MENU_MIN_PX: f32 = 18.0;
pub(crate) const MENU_H: u32 = 40;
pub const MENU_PAD: u32 = 18;
pub(crate) const MENU_INK: [u8; 3] = [0xf6, 0xf4, 0xef];

pub fn menu_face(label: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let ink = text::line_width(font, label, MENU_PX, 0.0).ceil() as u32;
    let w = ink + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * MENU_H * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, MENU_PX, MENU_MIN_PX);
    text::draw_centred(&mut rgba, w, MENU_H, &layout, MENU_INK);
    UndoFace { rgba, w, h: MENU_H }
}

pub fn heading_face(label: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let shown = label.to_uppercase();
    let ink = text::line_width(font, &shown, MENU_PX, text::tracking_for(MENU_PX)).ceil() as u32;
    let w = ink + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * MENU_H * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, MENU_PX, MENU_PX);
    text::draw_centred(&mut rgba, w, MENU_H, &layout, MENU_INK);
    UndoFace { rgba, w, h: MENU_H }
}

const DETAIL_PX: f32 = 20.0;
const DETAIL_H: u32 = 28;

pub fn menu_detail_face(label: &str) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let shown = label.to_uppercase();
    let ink =
        text::line_width(font, &shown, DETAIL_PX, text::tracking_for(DETAIL_PX)).ceil() as u32;
    let w = ink + 2 * MENU_PAD;
    let mut rgba = vec![0u8; (w * DETAIL_H * 4) as usize];
    let layout = text::fit(font, label, w as f32, 1, DETAIL_PX, DETAIL_PX);
    text::draw_centred(&mut rgba, w, DETAIL_H, &layout, MENU_INK);
    UndoFace {
        rgba,
        w,
        h: DETAIL_H,
    }
}
