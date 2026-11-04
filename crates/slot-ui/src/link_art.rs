use slot_gfx::{OUT_H, OUT_W};

use crate::art::render_svg;
use crate::bitmaps;
use crate::cart::CartFace;

pub const PORT_W: u32 = bitmaps::PORT_W;
pub const PORT_H: u32 = bitmaps::PORT_H;
pub const PORT_Y: f32 = (OUT_H - PORT_H) as f32;
pub const PLUG_W: u32 = 92;
pub const PLUG_H: u32 = 244;
pub const PLUG_TIP_X: f32 = 46.0;
pub const ADAPTER_W: u32 = bitmaps::ADAPTER_W;
pub const ADAPTER_H: u32 = bitmaps::ADAPTER_H;
/// Centre of the connector. It is the same width as the plug tip and sits on the screen centre.
pub const ADAPTER_BASE_X: f32 = 145.0;
/// Top of the connector. The connector is the bottom 31px, so a seated base puts the tip at `PORT_Y + 31`, the same depth as a plug.
pub const ADAPTER_BASE_Y: f32 = 149.0;
const CX: f32 = OUT_W as f32 / 2.0;
pub const ARCS: [(f32, f32, u32, u32); 3] = [
    (CX + 148.0, 324.0, 16, 56),
    (CX + 170.0, 308.0, 20, 88),
    (CX + 192.0, 292.0, 25, 120),
];
pub const CLICKS_X: f32 = CX - CLICKS_W as f32 / 2.0;
/// Just above the port lip. The port used to start at y=388 with the clicks at 360.
pub const CLICKS_Y: f32 = PORT_Y - 28.0;
pub const CLICKS_W: u32 = 180;
pub const CLICKS_H: u32 = 30;
pub const ARROW_W: u32 = 16;
pub const ARROW_H: u32 = 22;
pub const ARROW_LEFT_X: f32 = CX - 92.0;
pub const ARROW_RIGHT_X: f32 = CX + 76.0;
pub const ARROW_Y: f32 = 52.0;

const ARC_INK: &str = "#f6f4ef";

pub struct LinkArt {
    pub port: CartFace,
    pub port_light: CartFace,
    pub plug_host: CartFace,
    pub plug_join: CartFace,
    pub adapter: CartFace,
    pub arcs_right: [CartFace; 3],
    pub arcs_left: [CartFace; 3],
    pub clicks: CartFace,
    pub arrow_left: CartFace,
    pub arrow_right: CartFace,
}

pub fn link_art() -> LinkArt {
    let arcs_right = [arc_face(0), arc_face(1), arc_face(2)];
    let arcs_left = [
        mirror(&arcs_right[0]),
        mirror(&arcs_right[1]),
        mirror(&arcs_right[2]),
    ];
    LinkArt {
        port: bitmap_face(bitmaps::link_port(), PORT_W, PORT_H),
        port_light: bitmap_face(bitmaps::link_port_light(), PORT_W, PORT_H),
        plug_host: bitmap_face(bitmaps::link_plug_host(), PLUG_W, PLUG_H),
        plug_join: bitmap_face(bitmaps::link_plug_join(), PLUG_W, PLUG_H),
        adapter: bitmap_face(bitmaps::link_adapter(), ADAPTER_W, ADAPTER_H),
        arcs_right,
        arcs_left,
        clicks: svg_face(
            &format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {CLICKS_W} {CLICKS_H}"><g stroke="{ARC_INK}" stroke-width="4" stroke-linecap="round"><path d="M34 20 L20 4"/><path d="M146 20 L160 4"/><path d="M26 26 L4 26"/><path d="M154 26 L176 26"/></g></svg>"#
            ),
            CLICKS_W,
            CLICKS_H,
        ),
        arrow_left: svg_face(&arrow_svg("M14 2 L2 11 L14 20 Z"), ARROW_W, ARROW_H),
        arrow_right: svg_face(&arrow_svg("M2 2 L14 11 L2 20 Z"), ARROW_W, ARROW_H),
    }
}

fn bitmap_face(rgba: &[u8], w: u32, h: u32) -> CartFace {
    CartFace {
        rgba: rgba.to_vec(),
        w,
        h,
    }
}

fn svg_face(svg: &str, w: u32, h: u32) -> CartFace {
    let rgba = render_svg(svg, w, h).unwrap_or_else(|| vec![0; (w * h * 4) as usize]);
    CartFace { rgba, w, h }
}

fn arrow_svg(path: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {ARROW_W} {ARROW_H}"><path d="{path}" fill="{ARC_INK}"/></svg>"#
    )
}

fn arc_face(ring: usize) -> CartFace {
    let (_, _, w, h) = ARCS[ring];
    let bulge = [16.0, 24.0, 34.0][ring];
    let path = format!("M4 4 Q{} {} 4 {}", 4.0 + bulge, h as f32 / 2.0, h - 4);
    svg_face(
        &format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}"><path d="{path}" fill="none" stroke="{ARC_INK}" stroke-width="5" stroke-linecap="round"/></svg>"#
        ),
        w,
        h,
    )
}

fn mirror(face: &CartFace) -> CartFace {
    let mut rgba = vec![0u8; face.rgba.len()];
    for y in 0..face.h {
        for x in 0..face.w {
            let s = ((y * face.w + x) * 4) as usize;
            let d = ((y * face.w + (face.w - 1 - x)) * 4) as usize;
            rgba[d..d + 4].copy_from_slice(&face.rgba[s..s + 4]);
        }
    }
    CartFace {
        rgba,
        w: face.w,
        h: face.h,
    }
}
