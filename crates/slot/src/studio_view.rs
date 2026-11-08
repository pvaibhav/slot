use slot_store::{Outline, Platform, ShellChoice, ShellFinish};
use slot_ui::palette::{chip_grid, Chip};
use slot_ui::{shell_presets, GB_CART_H, GB_CART_W};

use crate::studio::{LabelKind, Row, Screen, Studio, Target};

const FOOTER_TOP: f32 = 422.0;
const CHIP_SIZE: f32 = 48.0;
const CHIP_STEP: f32 = 54.0;
const GRID_W: f32 = 4.0 * CHIP_STEP + CHIP_SIZE;
const GRID_TOP: f32 = 64.0;
const GRID_BOTTOM: f32 = 414.0;
const HEAD_H: f32 = 40.0;
const NAME_H: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    Heading,
    Detail,
    Digit,
    DigitOn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Cart,
    Text {
        text: String,
        style: Style,
        align: Align,
        x: f32,
        y: f32,
        alpha: f32,
        scale: f32,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    Chip {
        cell: u32,
        row: u32,
        x: f32,
        y: f32,
        selected: bool,
    },
    Dot {
        x: f32,
        y: f32,
        size: f32,
        alpha: f32,
    },
    Sheet,
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub items: Vec<Item>,
    pub closed_bar: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub margin: f32,
    pub edge: f32,
    pub cart_x: f32,
    pub cart_y: f32,
    pub cart_w: f32,
    pub cart_h: f32,
}

pub fn layout(canvas_w: f32, platform: Platform) -> Layout {
    let narrow = canvas_w < 700.0;
    let (w, ratio) = match platform {
        Platform::Gba | Platform::Gbav => (
            if narrow { 268.0 } else { slot_ui::CART_W as f32 },
            slot_ui::CART_H as f32 / slot_ui::CART_W as f32,
        ),
        #[cfg(feature = "nds")]
        Platform::Nds => (
            (if narrow { 268.0 } else { slot_ui::CART_W as f32 }) * slot_ui::DS_CART_W as f32
                / slot_ui::CART_W as f32,
            slot_ui::DS_CART_H as f32 / slot_ui::DS_CART_W as f32,
        ),
        Platform::Gb | Platform::Gbc => (
            if narrow { 248.0 } else { GB_CART_W as f32 },
            GB_CART_H as f32 / GB_CART_W as f32,
        ),
    };
    let inset = match platform {
        Platform::Gba | Platform::Gbav => 0.0,
        #[cfg(feature = "nds")]
        Platform::Nds => 0.0,
        Platform::Gb | Platform::Gbc => w * GB_ART_INSET / GB_CART_W as f32,
    };
    let h = w * ratio;
    let (sin, cos) = CART_TILT.to_radians().abs().sin_cos();
    let wide = (w - 2.0 * inset) * cos + h * sin;
    let sticker = match platform {
        Platform::Gba | Platform::Gbav => slot_ui::LABEL_W as f32 / slot_ui::CART_W as f32,
        #[cfg(feature = "nds")]
        Platform::Nds => slot_ui::DS_LABEL_W as f32 / slot_ui::DS_CART_W as f32,
        Platform::Gb | Platform::Gbc => slot_ui::GB_LABEL_W as f32 / GB_CART_W as f32,
    };
    let printer = w * sticker + 2.0 * (SLIP_PAD + PRINTER_LIP);
    let margin = ((canvas_w - wide - printer) / 3.0).floor().max(EDGE);
    Layout {
        margin,
        edge: margin + wide,
        cart_x: margin + (wide - w) / 2.0,
        cart_y: ((FOOTER_TOP - h) / 2.0).floor(),
        cart_w: w,
        cart_h: h,
    }
}

fn text(text: impl Into<String>, style: Style, align: Align, x: f32, y: f32, alpha: f32) -> Item {
    Item::Text {
        text: text.into(),
        style,
        align,
        x,
        y,
        alpha,
        scale: 1.0,
    }
}

fn preset_name(color: [u8; 3], finish: ShellFinish) -> Option<&'static str> {
    shell_presets().into_iter().find_map(|(name, s)| {
        let same = matches!(
            (s.finish, finish),
            (slot_ui::Finish::Solid, ShellFinish::Solid)
                | (slot_ui::Finish::Translucent, ShellFinish::Clear)
                | (slot_ui::Finish::Glitter, ShellFinish::Glitter)
        );
        (same && s.colour == color).then_some(name)
    })
}

fn shell_name(shell: Option<ShellChoice>) -> String {
    match shell {
        None => "Automatic".into(),
        Some(s) => preset_name(s.colour, s.finish).unwrap_or("Custom").into(),
    }
}

fn ground_name(ground: Option<[u8; 3]>) -> String {
    match ground {
        None => "Automatic".into(),
        Some(g) => shell_presets()
            .into_iter()
            .find(|(_, s)| s.colour == g)
            .map_or("Custom", |(n, _)| n)
            .into(),
    }
}

pub fn kind_name(kind: LabelKind) -> &'static str {
    match kind {
        LabelKind::Existing => "Existing",
        LabelKind::Real => "Real Label",
        LabelKind::Logo => "Logo Only",
        LabelKind::Title => "Title",
    }
}

fn row_name(row: Row) -> &'static str {
    match row {
        Row::Label => "Label Printer",
        Row::LabelColor => "Label Color",
        Row::Color => "Shell Color",
        Row::Finish => "Finish",
        Row::Shape => "Shape",
    }
}

fn row_choice(s: &Studio, row: Row) -> Option<(&'static str, usize)> {
    let shell = s.current_shell();
    match row {
        Row::Finish => Some(match shell.finish {
            ShellFinish::Solid => ("Solid", 0),
            ShellFinish::Clear => ("Clear", 1),
            ShellFinish::Glitter => ("Glitter", 2),
        }),
        Row::Shape => Some(match shell.outline {
            Outline::Auto => ("Automatic", 0),
            Outline::Notched => ("Notched", 1),
            Outline::Rounded => ("Rounded", 2),
        }),
        Row::Label | Row::LabelColor | Row::Color => None,
    }
}

fn list(s: &Studio, row: usize, left: f32, right: f32, out: &mut Vec<Item>) {
    const OPEN_H: f32 = 64.0;
    const INLINE_H: f32 = 100.0;
    const MAX_SCALE: f32 = 1.15;
    const LONGEST_NAME: f32 = 290.0;
    const DOT: f32 = 10.0;
    const DOT_STEP: f32 = 18.0;
    let mid = (left + right) / 2.0;
    let name_scale = ((right - left - 24.0) / LONGEST_NAME).min(MAX_SCALE);
    let rows = s.rows();
    let height = |r: &Row| if row_choice(s, *r).is_some() { INLINE_H } else { OPEN_H };
    let total: f32 = rows.iter().map(height).sum();
    let mut y = ((FOOTER_TOP - total) / 2.0).floor();
    for (i, r) in rows.iter().enumerate() {
        let alpha = if i == row { 1.0 } else { 0.6 };
        out.push(Item::Text {
            text: row_name(*r).into(),
            style: Style::Heading,
            align: Align::Center,
            x: mid,
            y: y + 8.0,
            alpha,
            scale: name_scale,
        });
        if let Some((value, at)) = row_choice(s, *r) {
            out.push(text(value, Style::Detail, Align::Center, mid, y + 46.0, alpha * 0.8));
            let left = mid - DOT / 2.0 - DOT_STEP;
            for dot in 0..3 {
                out.push(Item::Dot {
                    x: left + dot as f32 * DOT_STEP,
                    y: y + 78.0,
                    size: DOT,
                    alpha: if dot == at { 1.0 } else { 0.25 },
                });
            }
        }
        y += height(r);
    }
}

fn chip_label(s: &Studio, chip: Chip, target: Target) -> String {
    match (chip, target) {
        (Chip::Auto, _) => "Automatic".into(),
        (Chip::Custom, _) => "Custom".into(),
        (Chip::Current, Target::Shell) => shell_name(s.cart.shell),
        (Chip::Current, Target::Label) => ground_name(s.opened_ground),
        (Chip::Preset(i), _) => shell_presets()[i].0.into(),
    }
}

fn chips(s: &Studio, target: Target, at: usize, right: f32, mid: f32, l: &Layout, out: &mut Vec<Item>) {
    let probe = chip_grid(0.0, 0.0);
    let height = probe.last().map_or(0.0, |p| p.y) + CHIP_SIZE;
    let room = GRID_BOTTOM - GRID_TOP;
    let wanted = probe[at].y + CHIP_SIZE / 2.0 - room / 2.0;
    let scroll = wanted.clamp(0.0, (height - room).max(0.0));
    let top = 16.0;
    let x0 = right - GRID_W;
    let heading = match target {
        Target::Shell => "Shell Color",
        Target::Label => "Label Color",
    };
    out.push(text(heading, Style::Heading, Align::Center, x0 + GRID_W / 2.0, top, 1.0));
    let row = match target {
        Target::Shell => 0,
        Target::Label => 1,
    };
    let presets = shell_presets().len() as u32;
    for (i, p) in chip_grid(x0, GRID_TOP - scroll).iter().enumerate() {
        if p.y < GRID_TOP || p.y + CHIP_SIZE > GRID_BOTTOM {
            continue;
        }
        let cell = match p.chip {
            Chip::Preset(n) => n as u32,
            Chip::Current => presets,
            Chip::Auto => presets + 1,
            Chip::Custom => presets + 2,
        };
        out.push(Item::Chip {
            cell,
            row,
            x: p.x,
            y: p.y,
            selected: i == at,
        });
        if p.chip == Chip::Auto {
            out.push(Item::Text {
                text: "AUTO".into(),
                style: Style::Detail,
                align: Align::Center,
                x: p.x + CHIP_SIZE / 2.0,
                y: p.y + 14.0,
                alpha: 1.0,
                scale: 0.7,
            });
        }
        if p.chip == Chip::Custom {
            out.push(text("+", Style::Detail, Align::Center, p.x + CHIP_SIZE / 2.0, p.y + 10.0, 1.0));
        }
    }
    if height > room {
        let x = right + CHIP_EDGE - SCROLL_INSET - SCROLL_W;
        let thumb = room * room / height;
        let y = GRID_TOP + (room - thumb) * scroll / (height - room);
        out.push(Item::Rect { x, y: GRID_TOP, w: SCROLL_W, h: room, color: [1.0, 1.0, 1.0, 0.2] });
        out.push(Item::Rect { x, y, w: SCROLL_W, h: thumb, color: [1.0, 1.0, 1.0, 1.0] });
    }
    let name = chip_label(s, probe[at].chip, target);
    let (sin, cos) = CART_TILT.to_radians().abs().sin_cos();
    let bottom = l.cart_y + l.cart_h / 2.0 + (l.cart_w * sin + l.cart_h * cos) / 2.0;
    let gap = FOOTER_TOP - bottom;
    let scale = (gap / 32.0).clamp(0.72, 1.0);
    let name_y = (bottom + (gap - NAME_H * scale) / 2.0 - 3.0).floor();
    out.push(Item::Text { text: name, style: Style::Detail, align: Align::Center, x: mid, y: name_y, alpha: 1.0, scale });
}

fn custom(target: Target, rgb: [u8; 3], channel: usize, left: f32, right: f32, out: &mut Vec<Item>) {
    const ROW: f32 = 58.0;
    let mid = (left + right) / 2.0;
    let track = right - left - 96.0;
    let total = HEAD_H + 12.0 + 3.0 * ROW + 40.0;
    let top = ((FOOTER_TOP - total) / 2.0).floor();
    let heading = match target {
        Target::Shell => "Custom Shell",
        Target::Label => "Custom Label",
    };
    out.push(text(heading, Style::Heading, Align::Center, mid, top, 1.0));
    for c in 0..3 {
        let y = top + HEAD_H + 12.0 + c as f32 * ROW;
        let alpha = if c == channel { 1.0 } else { 0.6 };
        out.push(text(["R", "G", "B"][c], Style::Detail, Align::Left, left, y, alpha));
        for step in 0..30 {
            let mut color = rgb.map(|v| v as f32 / 255.0);
            color[c] = step as f32 / 29.0;
            out.push(Item::Rect {
                x: left + 40.0 + step as f32 * track / 30.0,
                y: y + 8.0,
                w: track / 30.0 + 0.5,
                h: 12.0,
                color: [color[0], color[1], color[2], 1.0],
            });
        }
        out.push(Item::Rect {
            x: left + 35.0 + rgb[c] as f32 / 255.0 * track,
            y: y + 2.0,
            w: 10.0,
            h: 24.0,
            color: [0.957, 0.945, 0.918, 1.0],
        });
        out.push(text(rgb[c].to_string(), Style::Detail, Align::Right, right, y, alpha));
    }
    let hex = format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]);
    let y = top + HEAD_H + 12.0 + 3.0 * ROW;
    out.push(text(hex, Style::Detail, Align::Center, mid, y, 1.0));
}

fn hex(target: Target, digits: [u8; 6], pos: usize, left: f32, right: f32, out: &mut Vec<Item>) {
    const BOX_H: f32 = 58.0;
    let step = ((right - left - 42.0) / 6.0).min(50.0);
    let box_w = step - 8.0;
    let total = HEAD_H + 16.0 + BOX_H;
    let top = ((FOOTER_TOP - total) / 2.0).floor();
    let x0 = right - (5.0 * step + box_w);
    let heading = match target {
        Target::Shell => "Shell Hex",
        Target::Label => "Label Hex",
    };
    let y = top + HEAD_H + 16.0;
    out.push(text(heading, Style::Heading, Align::Center, x0 - step / 2.0 + (6.0 * step + box_w) / 2.0, top, 1.0));
    out.push(text("#", Style::Heading, Align::Center, x0 - step + box_w / 2.0, y + 9.0, 1.0));
    for (i, d) in digits.iter().enumerate() {
        let x = x0 + i as f32 * step;
        let on = i == pos;
        out.push(Item::Rect {
            x,
            y,
            w: box_w,
            h: BOX_H,
            color: if on {
                [0.957, 0.945, 0.918, 1.0]
            } else {
                [0.0, 0.0, 0.0, 0.35]
            },
        });
        let style = if on { Style::DigitOn } else { Style::Digit };
        out.push(text(format!("{d:X}"), style, Align::Center, x + box_w / 2.0, y + 9.0, 1.0));
    }
}

pub fn view(s: &Studio, canvas_w: f32) -> View {
    let l = layout(canvas_w, s.cart.platform);
    let right = canvas_w - l.margin;
    let mid = l.cart_x + l.cart_w / 2.0;
    let mut items = vec![Item::Cart];
    let closed_bar = s.legends().len() > 2;
    match s.screen {
        Screen::List { row } => {
            list(s, row, l.edge + LIST_INSET, canvas_w - LIST_INSET, &mut items)
        }
        Screen::Sheet { .. } if s.sheet.is_empty() => {
            let line = match (s.art_failed, s.duplicates) {
                (true, _) => "No Connection",
                (false, true) => "No New Labels",
                (false, false) => "No Art Found",
            };
            let mid = (l.edge + right) / 2.0;
            items.push(text(line, Style::Detail, Align::Center, mid, 197.0, 1.0));
        }
        Screen::Sheet { .. } => items.push(Item::Sheet),
        Screen::Chips { target, at } => chips(s, target, at, canvas_w - CHIP_EDGE, mid, &l, &mut items),
        Screen::Custom { target, rgb, channel, .. } => custom(target, rgb, channel, l.edge + CUSTOM_GAP, right, &mut items),
        Screen::Hex { target, digits, pos, .. } => hex(target, digits, pos, l.edge, right, &mut items),
    }
    View { items, closed_bar }
}

pub const SLIP_PAD: f32 = 4.0;
pub const TRAY_TOP: f32 = 64.0;
const SLIP_GAP: f32 = 28.0;
const PRINTER_LIP: f32 = 10.0;
const EDGE: f32 = 20.0;
const LIST_INSET: f32 = 8.0;
const CUSTOM_GAP: f32 = 20.0;
const SCROLL_INSET: f32 = 8.0;
const CHIP_EDGE: f32 = 26.0;
const SCROLL_W: f32 = 4.0;
pub const CART_TILT: f32 = -7.0;
const GB_ART_INSET: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tray {
    pub x: f32,
    pub sticker_w: f32,
    pub sticker_h: f32,
    pub slip_w: f32,
    pub slip_h: f32,
    pub bottom: f32,
    pub printer_x: f32,
    pub printer_w: f32,
}

pub fn turned(rect: [f32; 4], centre: [f32; 2], deg: f32) -> [[f32; 2]; 4] {
    let [x, y, w, h] = rect;
    let (sin, cos) = deg.to_radians().sin_cos();
    [[x, y], [x + w, y], [x + w, y + h], [x, y + h]].map(|[px, py]| {
        let (dx, dy) = (px - centre[0], py - centre[1]);
        [centre[0] + dx * cos - dy * sin, centre[1] + dx * sin + dy * cos]
    })
}

pub fn tray(canvas_w: f32, cart: &slot_store::Cart) -> Tray {
    let l = layout(canvas_w, cart.platform);
    let well = well_of(cart);
    let reference = slot_gfx::Canvas {
        w: canvas_w,
        h: slot_gfx::OUT_H as f32,
    };
    let scale = l.cart_w / shelf_rect(reference, cart)[2];
    let (sticker_w, sticker_h) = (well[2] as f32 * scale, well[3] as f32 * scale);
    let (slip_w, slip_h) = (sticker_w + 2.0 * SLIP_PAD, sticker_h + 2.0 * SLIP_PAD);
    let printer_w = (slip_w + 2.0 * PRINTER_LIP).ceil();
    let printer_x = canvas_w - l.margin - printer_w;
    Tray {
        x: (printer_x + (printer_w - slip_w) / 2.0).floor(),
        sticker_w,
        sticker_h,
        slip_w,
        slip_h,
        bottom: (slip_h + SLIP_GAP).min(GRID_BOTTOM - TRAY_TOP - slip_h),
        printer_x,
        printer_w,
    }
}

pub fn well_of(cart: &slot_store::Cart) -> [u32; 4] {
    #[cfg(feature = "nds")]
    if cart.platform == Platform::Nds {
        return [slot_ui::DS_LABEL_X, slot_ui::DS_LABEL_Y, slot_ui::DS_LABEL_W, slot_ui::DS_LABEL_H];
    }
    match slot_ui::gb_shell_of(cart) {
        None => [slot_ui::LABEL_X, slot_ui::LABEL_Y, slot_ui::LABEL_W, slot_ui::LABEL_H],
        Some(slot_ui::GbShell::Notched) => [
            slot_ui::GB_LABEL_X,
            slot_ui::GB_LABEL_Y,
            slot_ui::GB_LABEL_W,
            slot_ui::GB_LABEL_H,
        ],
        Some(slot_ui::GbShell::Rounded) => [
            slot_ui::GBC_LABEL_X,
            slot_ui::GBC_LABEL_Y,
            slot_ui::GBC_LABEL_W,
            slot_ui::GBC_LABEL_H,
        ],
    }
}

pub fn shelf_rect(canvas: slot_gfx::Canvas, cart: &slot_store::Cart) -> [f32; 4] {
    let (w, h) = match cart.platform {
        Platform::Gba | Platform::Gbav => (slot_ui::CART_W as f32, slot_ui::CART_H as f32),
        #[cfg(feature = "nds")]
        Platform::Nds => (slot_ui::DS_CART_W as f32, slot_ui::DS_CART_H as f32),
        Platform::Gb | Platform::Gbc => (GB_CART_W as f32, GB_CART_H as f32),
    };
    [(canvas.w - w) / 2.0, slot_ui::rest_y(canvas, h), w, h]
}
