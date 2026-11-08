use crate::plate::UndoFace;
use crate::text;

const HEADING_PX: f32 = 20.0;
const LAST_PX: f32 = 20.0;
const HEADING_INK: [u8; 3] = [0xc9, 0xc6, 0xbf];
const LAST_INK: [u8; 3] = [0xe6, 0xe3, 0xdd];
const PICKED_PX: f32 = 22.0;
const SIDE_PX: f32 = 15.0;
const PICKED_INK: [u8; 3] = [0xff, 0xff, 0xff];
const SIDE_INK: [u8; 3] = [0x9a, 0x97, 0x8f];

fn line(label: &str, px: f32, ink: [u8; 3]) -> UndoFace {
    let Some(font) = text::label_font() else {
        return UndoFace {
            rgba: Vec::new(),
            w: 0,
            h: 0,
        };
    };
    let layout = text::fit(font, &label.to_uppercase(), f32::INFINITY, 1, px, px);
    let set = layout
        .lines
        .iter()
        .map(|l| text::line_width(font, l, layout.px, layout.tracking))
        .fold(0.0, f32::max);
    let w = set.ceil() as u32 + 2;
    let h = (px * 1.5).ceil() as u32;
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    text::draw_centred(&mut rgba, w, h, &layout, ink);
    UndoFace { rgba, w, h }
}

pub fn heading_text_face(label: &str, large: bool) -> UndoFace {
    match large {
        true => line(label, LAST_PX, LAST_INK),
        false => line(label, HEADING_PX, HEADING_INK),
    }
}

pub fn stack_name_face(label: &str, picked: bool) -> UndoFace {
    match picked {
        true => line(label, PICKED_PX, PICKED_INK),
        false => line(label, SIDE_PX, SIDE_INK),
    }
}
