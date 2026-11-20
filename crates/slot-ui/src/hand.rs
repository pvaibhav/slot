use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{Font, FontSettings};
use rustybuzz::{
    ttf_parser::{GlyphId, Tag},
    Face, Feature, UnicodeBuffer,
};

const HAND_TTF: &[u8] = include_bytes!("../assets/NoSmear.ttf");
const HAND_STROKES: &[u8] = include_bytes!("../assets/NoSmear.strokes.json");

const CAP: f32 = 0.70;
const LEADING: f32 = 1.02;
const CAP_ACCENT_ROOM: f32 = 0.3;
const ACCENT_ROOM: f32 = 0.15;
const DESCENT: f32 = 0.20;
const TAG_SHARE: f32 = 0.28;
const TAG_SCALE: f32 = 0.42;
const MIN_TAG_PX: f32 = 9.0;

type Strokes = Vec<Vec<[f32; 2]>>;

pub struct Hand {
    face: Face<'static>,
    raster: Font,
    strokes: HashMap<String, (f32, Strokes)>,
    alternates: HashMap<u16, Vec<u16>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub glyph: u16,
    pub x: f32,
    pub y: f32,
}

pub struct Written {
    pub w: u32,
    pub h: u32,
    pub ink: Vec<u8>,
    pub colour: [u8; 3],
    pub path: Vec<Vec<[f32; 2]>>,
    pub nib: f32,
}

fn load_strokes() -> HashMap<String, (f32, Strokes)> {
    let mut out = HashMap::new();
    let Ok(json) = serde_json::from_slice::<serde_json::Value>(HAND_STROKES) else {
        return out;
    };
    let Some(glyphs) = json.get("glyphs").and_then(|g| g.as_object()) else {
        return out;
    };
    for (name, g) in glyphs {
        let nib = g.get("nib").and_then(|n| n.as_f64()).unwrap_or(150.0) as f32;
        let strokes = g
            .get("strokes")
            .and_then(|s| s.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|s| s.as_array())
                    .map(|pts| {
                        pts.iter()
                            .filter_map(|p| {
                                let p = p.as_array()?;
                                Some([p.first()?.as_f64()? as f32, p.get(1)?.as_f64()? as f32])
                            })
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.insert(name.clone(), (nib, strokes));
    }
    out
}

fn load_alternates(face: &Face) -> HashMap<u16, Vec<u16>> {
    use rustybuzz::ttf_parser::gsub::SubstitutionSubtable;
    let mut out = HashMap::new();
    let Some(gsub) = face.tables().gsub else {
        return out;
    };
    let rand = Tag::from_bytes(b"rand");
    let features = (0..gsub.features.len()).filter_map(|i| gsub.features.get(i));
    for feature in features.filter(|f| f.tag == rand) {
        for index in feature.lookup_indices {
            let Some(lookup) = gsub.lookups.get(index) else {
                continue;
            };
            for sub in 0..lookup.subtables.len() {
                let Some(SubstitutionSubtable::Alternate(alt)) =
                    lookup.subtables.get::<SubstitutionSubtable>(sub)
                else {
                    continue;
                };
                for g in 0..face.number_of_glyphs() {
                    let Some(set) = alt
                        .coverage
                        .get(GlyphId(g))
                        .and_then(|i| alt.alternate_sets.get(i))
                    else {
                        continue;
                    };
                    let mut all = vec![g];
                    all.extend(set.alternates.into_iter().map(|a| a.0));
                    out.insert(g, all);
                }
            }
        }
    }
    out
}

fn picks(text: &str, seed: u64) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (n, word) in text.split_whitespace().enumerate() {
        let at = word.as_ptr() as usize - text.as_ptr() as usize;
        let key = seed ^ fnv(word) ^ (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut last: Option<(char, usize)> = None;
        for (j, (i, ch)) in word.char_indices().enumerate() {
            let roll = (noise(key, j as u32) * 10_000.0) as usize;
            let pick = match last {
                Some((prev, p)) if prev == ch && roll % 5 == p % 5 => roll + 1,
                _ => roll,
            };
            last = Some((ch, pick));
            out.push((at + i, pick));
        }
    }
    out
}

pub fn hand_font() -> Option<&'static Hand> {
    static FONT: OnceLock<Option<Hand>> = OnceLock::new();
    FONT.get_or_init(|| {
        let face = Face::from_slice(HAND_TTF, 0)?;
        Some(Hand {
            alternates: load_alternates(&face),
            raster: Font::from_bytes(HAND_TTF, FontSettings::default()).ok()?,
            strokes: load_strokes(),
            face,
        })
    })
    .as_ref()
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn leading(line: &str) -> f32 {
    let accented = line.chars().filter(|c| !c.is_ascii() && c.is_alphabetic());
    let room = accented.fold(0.0, |room: f32, c| {
        room.max(if c.is_uppercase() {
            CAP_ACCENT_ROOM
        } else {
            ACCENT_ROOM
        })
    });
    LEADING + room
}

fn gaps(lines: &[String]) -> f32 {
    lines.iter().skip(1).map(|l| leading(l)).sum()
}

fn noise(seed: u64, i: u32) -> f32 {
    let mut h = seed ^ (i as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= h >> 31;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 29;
    (h % 10_000) as f32 / 10_000.0
}

impl Hand {
    pub fn shape(&self, text: &str, px: f32) -> (Vec<Placed>, f32) {
        self.place(text, px, None)
    }

    pub fn shape_from(&self, text: &str, px: f32, seed: u64) -> (Vec<Placed>, f32) {
        self.place(text, px, Some(&picks(text, seed)))
    }

    fn place(&self, text: &str, px: f32, picks: Option<&[(usize, usize)]>) -> (Vec<Placed>, f32) {
        let off = |tag: &[u8; 4]| Feature::new(Tag::from_bytes(tag), 0, ..);
        let features = match picks {
            Some(_) => vec![off(b"rand"), off(b"calt")],
            None => vec![off(b"rand")],
        };
        let mut buf = UnicodeBuffer::new();
        buf.push_str(text);
        let out = rustybuzz::shape(&self.face, &features, buf);
        let scale = px / self.face.units_per_em() as f32;
        let mut pen = 0.0;
        let mut placed = Vec::with_capacity(out.len());
        for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
            let shaped = info.glyph_id as u16;
            let pick = picks.and_then(|p| {
                let at = p
                    .binary_search_by_key(&(info.cluster as usize), |(i, _)| *i)
                    .ok()?;
                let all = self.alternates.get(&shaped)?;
                Some(all[p[at].1 % all.len()])
            });
            let (glyph, advance) = match pick {
                Some(g) if g != shaped => (
                    g,
                    self.face
                        .glyph_hor_advance(GlyphId(g))
                        .map_or(pos.x_advance, |a| a as i32),
                ),
                _ => (shaped, pos.x_advance),
            };
            placed.push(Placed {
                glyph,
                x: pen + pos.x_offset as f32 * scale,
                y: pos.y_offset as f32 * scale,
            });
            pen += advance as f32 * scale;
        }
        (placed, pen)
    }

    pub fn line_width(&self, text: &str, px: f32) -> f32 {
        self.shape(text, px).1
    }

    pub fn line_height(&self, px: f32) -> f32 {
        self.raster
            .horizontal_line_metrics(px)
            .map_or(px, |m| m.new_line_size)
    }

    pub fn coverage(&self, dst_w: u32, dst_h: u32, lines: &[String], px: f32) -> Vec<u8> {
        let mut out = vec![0u8; (dst_w * dst_h) as usize];
        let Some(vm) = self.raster.horizontal_line_metrics(px) else {
            return out;
        };
        let line_h = vm.new_line_size;
        let mut baseline = (dst_h as f32 - line_h * lines.len() as f32) / 2.0 + vm.ascent;
        for line in lines {
            let (placed, width) = self.shape(line, px);
            let left = (dst_w as f32 - width) / 2.0;
            for g in placed {
                let (m, cov) = self.raster.rasterize_indexed(g.glyph, px);
                stamp(
                    &mut out,
                    dst_w,
                    dst_h,
                    (left + g.x + m.xmin as f32).round() as i32,
                    (baseline - g.y - (m.height as f32 + m.ymin as f32)).round() as i32,
                    &cov,
                    m.width as u32,
                    m.height as u32,
                );
            }
            baseline += line_h;
        }
        out
    }

    pub fn has_strokes(&self, glyph: u16) -> bool {
        self.glyph_strokes(glyph)
            .is_some_and(|(_, s)| !s.is_empty())
    }

    fn glyph_strokes(&self, glyph: u16) -> Option<&(f32, Strokes)> {
        let name = self.face.glyph_name(GlyphId(glyph))?;
        self.strokes.get(name)
    }

    fn fit(&self, title: &str, seed: u64, bw: f32, bh: f32) -> (Vec<String>, f32) {
        let words: Vec<&str> = title.split_whitespace().collect();
        if words.is_empty() {
            return (Vec::new(), 0.0);
        }
        let unit = |s: &str| self.shape_from(s, 100.0, seed).1 / 100.0;
        let mut best: (Vec<String>, f32) = (vec![words.join(" ")], 0.0);
        let n = words.len();
        let mut splits: Vec<Vec<usize>> = vec![vec![]];
        for a in 1..n {
            splits.push(vec![a]);
            for b in a + 1..n {
                splits.push(vec![a, b]);
            }
        }
        for cut in splits {
            let mut lines = Vec::new();
            let mut start = 0;
            for &c in cut.iter().chain(std::iter::once(&n)) {
                lines.push(words[start..c].join(" "));
                start = c;
            }
            let widest = lines.iter().map(|l| unit(l)).fold(0.0, f32::max);
            let block = CAP + gaps(&lines) + DESCENT;
            let px = (bw / widest.max(0.01)).min(bh / block);
            if px > best.1 * 1.04 {
                best = (lines, px);
            }
        }
        best
    }

    pub fn write(
        &self,
        title: &str,
        tags: &[String],
        key: &str,
        w: u32,
        h: u32,
        recess: [u8; 3],
    ) -> Written {
        let seed = fnv(key);
        let (fw, fh) = (w as f32, h as f32);
        let tilt = (noise(seed, 1) - 0.5) * 0.07;
        let (bx, by, bw, bh, slope) = (fw * 0.07, fh * 0.12, fw * 0.86, fh * 0.76, tilt * 0.6);
        let colour = ink_for(recess);
        let tag = tags.join(" ");
        let tag_room = if tag.is_empty() { 0.0 } else { TAG_SHARE };
        let (lines, px) = self.fit(title, seed, bw, bh * (1.0 - tag_room));
        let tag_px = px * TAG_SCALE;
        let tag_fits = !tag.is_empty()
            && self.shape_from(&tag, tag_px, seed).1 <= bw * 0.9
            && tag_px >= MIN_TAG_PX;
        let (lines, px) = if tag_fits {
            (lines, px)
        } else {
            self.fit(title, seed, bw, bh)
        };
        let mut ink = vec![0u8; (w * h) as usize];
        let mut path = Vec::new();
        let mut nib = 150.0 * px / 1000.0;
        let tag_h = if tag_fits {
            DESCENT * px + tag_px * CAP * 1.4
        } else {
            0.0
        };
        let block = CAP * px + gaps(&lines) * px + tag_h;
        let mut baseline = by + (bh - block - DESCENT * px) / 2.0 + CAP * px;
        let mid = fw / 2.0;
        let mut rows: Vec<(String, f32, f32, f32)> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i > 0 {
                    baseline += leading(l) * px;
                }
                let (_, width) = self.shape_from(l, px, seed);
                (l.clone(), px, bx + (bw - width) / 2.0, baseline)
            })
            .collect();
        if tag_fits {
            let width = self.shape_from(&tag, tag_px, seed).1;
            let base = baseline + tag_h;
            rows.push((tag, tag_px, bx + bw - width - bw * 0.04, base));
        }
        for (line, px, left, baseline) in rows {
            let scale = px / self.face.units_per_em() as f32;
            let (placed, _) = self.shape_from(&line, px, seed);
            for g in placed {
                let gx = left + g.x;
                let lift = (gx - mid) * slope;
                let base = baseline - g.y + lift;
                let (m, cov) = self.raster.rasterize_indexed(g.glyph, px);
                stamp(
                    &mut ink,
                    w,
                    h,
                    (gx + m.xmin as f32).round() as i32,
                    (base - (m.height as f32 + m.ymin as f32)).round() as i32,
                    &cov,
                    m.width as u32,
                    m.height as u32,
                );
                if let Some((n, strokes)) = self.glyph_strokes(g.glyph) {
                    nib = nib.max(n * scale);
                    for s in strokes {
                        path.push(
                            s.iter()
                                .map(|[x, y]| [gx + x * scale, base - y * scale])
                                .collect(),
                        );
                    }
                }
            }
        }
        Written {
            w,
            h,
            ink,
            colour,
            path,
            nib,
        }
    }
}

const INKS: [[u8; 3]; 2] = [[18, 18, 22], [250, 250, 246]];

pub fn ink_for(shell: [u8; 3]) -> [u8; 3] {
    let lum = |c: [u8; 3]| {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    };
    let contrast = |a: [u8; 3]| {
        let (x, y) = (lum(a), lum(shell));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    INKS.into_iter()
        .max_by(|a, b| contrast(*a).total_cmp(&contrast(*b)))
        .unwrap_or(INKS[0])
}

pub fn reveal(written: &Written, frames: u32) -> Vec<(Vec<u8>, [f32; 2])> {
    let (w, h) = (written.w, written.h);
    let r = (written.nib * 0.75).max(1.5);
    let step = (r / 3.0).max(0.5);
    let mut segs: Vec<(Vec<[f32; 2]>, bool)> = Vec::new();
    let mut prev: Option<[f32; 2]> = None;
    for stroke in &written.path {
        if let (Some(from), Some(&to)) = (prev, stroke.first()) {
            let mut air = Vec::new();
            trace(&mut air, from, to, step * AIR_SPEED);
            segs.push((air, false));
        }
        let mut pts = Vec::new();
        for pair in stroke.windows(2) {
            trace(&mut pts, pair[0], pair[1], step);
        }
        if let Some(last) = stroke.last() {
            pts.push(*last);
            prev = Some(*last);
        }
        segs.push((pts, true));
    }
    let total: usize = segs.iter().map(|(pts, _)| pts.len()).sum();
    let least = (total as f32 * MIN_STROKE_FRAMES / frames.max(1) as f32).ceil() as usize;
    let points: Vec<([f32; 2], bool)> = segs
        .into_iter()
        .flat_map(|(pts, ink)| {
            let pts = if ink && pts.len() < least {
                stretch(&pts, least)
            } else {
                pts
            };
            pts.into_iter().map(move |p| (p, ink))
        })
        .collect();
    let mut mask = vec![0u8; (w * h) as usize];
    let mut out = Vec::with_capacity(frames as usize);
    let mut done = 0usize;
    for f in 1..=frames {
        let upto = points.len() * f as usize / frames as usize;
        for (p, ink) in &points[done..upto] {
            if *ink {
                disc(&mut mask, w, h, *p, r);
            }
        }
        done = upto;
        let frame: Vec<u8> = if f == frames {
            written.ink.clone()
        } else {
            written
                .ink
                .iter()
                .zip(&mask)
                .map(|(i, m)| (*i as u32 * *m as u32 / 255) as u8)
                .collect()
        };
        let pen = points
            .get(upto.saturating_sub(1))
            .map_or([w as f32 / 2.0, h as f32 / 2.0], |(p, _)| *p);
        out.push((frame, pen));
    }
    out
}

const MIN_STROKE_FRAMES: f32 = 2.5;
const AIR_SPEED: f32 = 1.0;

fn trace(points: &mut Vec<[f32; 2]>, a: [f32; 2], b: [f32; 2], step: f32) {
    let len = (b[0] - a[0]).hypot(b[1] - a[1]);
    let n = (len / step.max(0.05)).ceil().max(1.0) as u32;
    for k in 0..n {
        let t = k as f32 / n as f32;
        points.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
    }
}

fn stretch(pts: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    if pts.len() < 2 {
        return vec![pts.first().copied().unwrap_or([0.0; 2]); n];
    }
    (0..n)
        .map(|k| {
            let at = k as f32 * (pts.len() - 1) as f32 / (n - 1).max(1) as f32;
            let i = (at as usize).min(pts.len() - 2);
            let t = at - i as f32;
            let (a, b) = (pts[i], pts[i + 1]);
            [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
        })
        .collect()
}

fn disc(mask: &mut [u8], w: u32, h: u32, c: [f32; 2], r: f32) {
    let (x0, x1) = (
        (c[0] - r - 1.0).max(0.0) as u32,
        ((c[0] + r + 1.0) as u32).min(w),
    );
    let (y0, y1) = (
        (c[1] - r - 1.0).max(0.0) as u32,
        ((c[1] + r + 1.0) as u32).min(h),
    );
    for y in y0..y1 {
        for x in x0..x1 {
            let d = (x as f32 + 0.5 - c[0]).hypot(y as f32 + 0.5 - c[1]);
            let a = ((r + 0.5 - d).clamp(0.0, 1.0) * 255.0) as u8;
            let i = (y * w + x) as usize;
            mask[i] = mask[i].max(a);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn stamp(dst: &mut [u8], dst_w: u32, dst_h: u32, x: i32, y: i32, cov: &[u8], gw: u32, gh: u32) {
    for gy in 0..gh {
        let dy = y + gy as i32;
        if dy < 0 || dy >= dst_h as i32 {
            continue;
        }
        for gx in 0..gw {
            let dx = x + gx as i32;
            if dx < 0 || dx >= dst_w as i32 {
                continue;
            }
            let i = (dy as u32 * dst_w + dx as u32) as usize;
            dst[i] = dst[i].max(cov[(gy * gw + gx) as usize]);
        }
    }
}

pub const MARKER_W: u32 = 14;
pub const MARKER_H: u32 = 140;
const MARKER_SCALE: u32 = 7;
pub const MARKER_CAP: [f32; 2] = [84.0, 139.0];

fn shade(ink: [u8; 3], k: f32, lift: f32) -> String {
    let c = ink.map(|v| {
        (v as f32 * k + (255.0 - v as f32) * lift)
            .round()
            .clamp(0.0, 255.0) as u8
    });
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

fn marker_svg(cap: bool, ink: [u8; 3]) -> String {
    let stops = [
        (0.0, 0.45, 0.0),
        (0.32, 1.0, 0.16),
        (0.5, 0.85, 0.0),
        (1.0, 0.3, 0.0),
    ]
    .map(|(at, k, lift)| {
        format!(
            r#"<stop offset="{at}" stop-color="{}"/>"#,
            shade(ink, k, lift)
        )
    })
    .concat();
    let (rim, clip, tip) = (
        shade(ink, 0.2, 0.0),
        shade(ink, 0.75, 0.06),
        shade(ink, 0.9, 0.0),
    );
    let defs = format!(
        r##"<defs>
<linearGradient id="ink" x1="0" x2="1" y1="0" y2="0">{stops}</linearGradient>
<linearGradient id="barrel" x1="0" x2="1" y1="0" y2="0">
<stop offset="0" stop-color="#8d8f93"/><stop offset="0.34" stop-color="#eeeff1"/><stop offset="0.55" stop-color="#c9cacd"/><stop offset="1" stop-color="#7c7e82"/>
</linearGradient>
</defs>"##
    );
    let body = if cap {
        format!(
            r##"<rect x="0.5" y="84" width="13" height="55" rx="3" fill="url(#ink)"/>
<rect x="1.5" y="136.5" width="11" height="3.5" rx="1.5" fill="{rim}"/>
<rect x="10.5" y="88" width="2" height="36" rx="1" fill="{clip}"/>"##
        )
    } else {
        format!(
            r##"<rect x="1.5" y="0.5" width="11" height="8" rx="3.5" fill="url(#ink)"/>
<rect x="1.5" y="5" width="11" height="103" fill="url(#barrel)"/>
<path d="M1.5 106 H12.5 V122 L10.5 130 H3.5 L1.5 122 Z" fill="url(#ink)"/>
<path d="M4.5 129.5 H9.5 L8.8 136 H5.2 Z" fill="#202024"/>
<path d="M5.6 135.5 H8.4 L7.6 139.6 Q7 140.2 6.4 139.6 Z" fill="{tip}"/>"##
        )
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {MARKER_W} {MARKER_H}" width="{}" height="{}">{defs}{body}</svg>"##,
        MARKER_W * MARKER_SCALE,
        MARKER_H * MARKER_SCALE
    )
}

pub fn marker_size() -> (u32, u32) {
    (MARKER_W * MARKER_SCALE, MARKER_H * MARKER_SCALE)
}

pub fn marker_sprite(cap: bool, ink: [u8; 3]) -> Vec<u8> {
    let (w, h) = marker_size();
    crate::art::render_svg(&marker_svg(cap, ink), w, h)
        .unwrap_or_else(|| vec![0; (w * h * 4) as usize])
}

pub const GLOW_PAD: u32 = 12;
const GLOW_RADIUS: usize = 16;

pub struct MarkerArt {
    pub body: Vec<u8>,
    pub cap: Vec<u8>,
    pub body_glow: Vec<u8>,
    pub cap_glow: Vec<u8>,
}

pub fn glow_size() -> (u32, u32) {
    let (w, h) = marker_size();
    let pad = GLOW_PAD * MARKER_SCALE;
    (w + 2 * pad, h + 2 * pad)
}

pub fn marker_art(ink: [u8; 3]) -> &'static MarkerArt {
    static ART: std::sync::Mutex<Vec<([u8; 3], &'static MarkerArt)>> =
        std::sync::Mutex::new(Vec::new());
    let made = || ART.lock().unwrap_or_else(|e| e.into_inner());
    let find = |list: &[([u8; 3], &'static MarkerArt)]| {
        list.iter().find(|(c, _)| *c == ink).map(|(_, art)| *art)
    };
    if let Some(art) = find(&made()) {
        return art;
    }
    let (body, cap) = (marker_sprite(false, ink), marker_sprite(true, ink));
    let built = MarkerArt {
        body_glow: glow(&body),
        cap_glow: glow(&cap),
        body,
        cap,
    };
    let mut list = made();
    if let Some(art) = find(&list) {
        return art;
    }
    let art: &'static MarkerArt = Box::leak(Box::new(built));
    list.push((ink, art));
    art
}

fn glow(sprite: &[u8]) -> Vec<u8> {
    let (w, h) = marker_size();
    let (gw, gh) = glow_size();
    let pad = (GLOW_PAD * MARKER_SCALE) as usize;
    let (w, h, gw, gh) = (w as usize, h as usize, gw as usize, gh as usize);
    let mut a = vec![0f32; gw * gh];
    for y in 0..h {
        for x in 0..w {
            a[(y + pad) * gw + x + pad] = sprite[(y * w + x) * 4 + 3] as f32;
        }
    }
    for _ in 0..3 {
        a = box_blur(&a, gw, gh, true);
        a = box_blur(&a, gw, gh, false);
    }
    a.iter()
        .flat_map(|v| [255, 255, 255, (v * 2.0).min(255.0) as u8])
        .collect()
}

fn box_blur(src: &[f32], w: usize, h: usize, across: bool) -> Vec<f32> {
    let r = GLOW_RADIUS;
    let n = (2 * r + 1) as f32;
    let (lines, len) = if across { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| if across { line * w + i } else { i * w + line };
    let mut out = vec![0f32; w * h];
    for line in 0..lines {
        let mut sum: f32 = (0..=r.min(len - 1)).map(|i| src[at(line, i)]).sum();
        for i in 0..len {
            out[at(line, i)] = sum / n;
            if i + r + 1 < len {
                sum += src[at(line, i + r + 1)];
            }
            if i >= r {
                sum -= src[at(line, i - r)];
            }
        }
    }
    out
}
