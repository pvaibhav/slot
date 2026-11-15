use crate::bezel::{framed, screen_rect};
use crate::surface::{OUT_H, OUT_W};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sizing {
    Band,
    Integer,
    Fit,
    Panel,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GameLayout {
    pub panel: bool,
    pub rect: [f32; 4],
}

impl GameLayout {
    pub const BAND: GameLayout = GameLayout {
        panel: false,
        rect: [0.0, 0.0, OUT_W as f32, OUT_H as f32],
    };
}

const GBA: (u32, u32) = (240, 160);
const GB: (u32, u32) = (160, 144);

type Row = [[f32; 4]; 4];
type Entry = ((u32, u32), (u32, u32), Row);

const TABLE: [Entry; 4] = [
    ((720, 720), GBA, [[0.0, 120.0, 720.0, 480.0]; 4]),
    ((1024, 768), GBA, [[32.0, 64.0, 960.0, 640.0]; 4]),
    (
        (720, 720),
        GB,
        [
            [0.0, 120.0, 720.0, 480.0],
            [40.0, 72.0, 640.0, 576.0],
            [0.0, 36.0, 720.0, 648.0],
            [0.0, 0.0, 720.0, 720.0],
        ],
    ),
    (
        (1024, 768),
        GB,
        [
            [32.0, 64.0, 960.0, 640.0],
            [112.0, 24.0, 800.0, 720.0],
            [85.0, 0.0, 853.0, 768.0],
            [0.0, 0.0, 1024.0, 768.0],
        ],
    ),
];

pub fn table_placement(
    canvas: (u32, u32),
    picture: (u32, u32),
    sizing: Sizing,
) -> Option<[f32; 4]> {
    let column = match sizing {
        Sizing::Band => 0,
        Sizing::Integer => 1,
        Sizing::Fit => 2,
        Sizing::Panel => 3,
    };
    TABLE
        .iter()
        .find(|(c, p, _)| *c == canvas && *p == picture)
        .map(|(_, _, row)| row[column])
}

fn own_canvas(canvas: crate::layout::Canvas, picture: (u32, u32), sizing: Sizing) -> GameLayout {
    let (cw, ch) = (canvas.w, canvas.h);
    let (pw, ph) = (picture.0.max(1) as f32, picture.1.max(1) as f32);
    let (w, h) = match sizing {
        Sizing::Panel => (cw, ch),
        Sizing::Band | Sizing::Integer => {
            let k = (cw / pw).min(ch / ph).floor().max(1.0);
            (pw * k, ph * k)
        }
        Sizing::Fit => {
            let s = (cw / pw).min(ch / ph);
            ((pw * s).round(), (ph * s).round())
        }
    };
    let (x, y) = (((cw - w) / 2.0).floor(), ((ch - h) / 2.0).floor());
    let (sx, sy) = (OUT_W as f32 / cw, OUT_H as f32 / ch);
    GameLayout {
        panel: false,
        rect: [x * sx, y * sy, w * sx, h * sy],
    }
}

pub fn game_layout(window: (u32, u32), picture: (u32, u32), sizing: Sizing) -> GameLayout {
    let size = match crate::layout::fills(window) {
        true => crate::layout::panel_pixels(window),
        false => (OUT_W, OUT_H),
    };
    let canvas = crate::layout::Canvas {
        w: size.0 as f32,
        h: size.1 as f32,
    };
    if let Some([x, y, w, h]) = table_placement(size, picture, sizing) {
        let (sx, sy) = (OUT_W as f32 / canvas.w, OUT_H as f32 / canvas.h);
        return GameLayout {
            panel: false,
            rect: [x * sx, y * sy, w * sx, h * sy],
        };
    }
    let framed = framed(window);
    if !framed && size != (OUT_W, OUT_H) {
        return own_canvas(canvas, picture, sizing);
    }
    let panel = match framed {
        true => size,
        false => (OUT_W, OUT_H),
    };
    let band = match framed {
        true => (panel.0, screen_rect(panel).3 as u32),
        false => panel,
    };
    let (pw, ph) = (picture.0.max(1), picture.1.max(1));
    let size = match sizing {
        Sizing::Band => return GameLayout::BAND,
        Sizing::Panel => {
            return GameLayout {
                panel: framed,
                ..GameLayout::BAND
            }
        }
        Sizing::Integer => {
            let k = (panel.0 / pw).min(panel.1 / ph).max(1);
            (pw * k, ph * k)
        }
        Sizing::Fit => {
            let s = (panel.0 as f32 / pw as f32).min(panel.1 as f32 / ph as f32);
            (
                (pw as f32 * s).round() as u32,
                (ph as f32 * s).round() as u32,
            )
        }
    };
    let area = match size.0 <= band.0 && size.1 <= band.1 {
        true => band,
        false => panel,
    };
    let x = area.0.saturating_sub(size.0) / 2;
    let y = area.1.saturating_sub(size.1) / 2;
    let (sx, sy) = (OUT_W as f32 / area.0 as f32, OUT_H as f32 / area.1 as f32);
    GameLayout {
        panel: framed && area != band,
        rect: [
            x as f32 * sx,
            y as f32 * sy,
            size.0 as f32 * sx,
            size.1 as f32 * sy,
        ],
    }
}
