use std::collections::HashMap;

use serde_json::Value;
use slot_ui::house_label::Band;

#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub ground: [u8; 3],
    pub band: Option<Band>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub real: Option<String>,
    pub logo: Option<String>,
    pub boxed: Option<String>,
    pub look: Option<Look>,
}

pub struct ArtIndex {
    entries: Vec<Entry>,
    by_crc: HashMap<u32, usize>,
}

impl ArtIndex {
    pub fn parse(json: &str) -> Option<ArtIndex> {
        let Value::Object(map) = serde_json::from_str::<Value>(json).ok()? else {
            return None;
        };
        let mut index = ArtIndex {
            entries: Vec::new(),
            by_crc: HashMap::new(),
        };
        for (key, raw) in &map {
            let at = index.entries.len();
            index.entries.push(entry(raw));
            let mut crcs: Vec<u32> = crc_of(key).into_iter().collect();
            if let Some(list) = raw["crcs"].as_array() {
                crcs.extend(list.iter().filter_map(|c| c.as_str().and_then(crc_of)));
            }
            for crc in crcs {
                index.by_crc.insert(crc, at);
            }
        }
        Some(index)
    }

    pub fn get(&self, crc: u32) -> Option<&Entry> {
        self.by_crc.get(&crc).map(|&at| &self.entries[at])
    }
}

fn crc_of(key: &str) -> Option<u32> {
    match key.len() == 8 {
        true => u32::from_str_radix(key, 16).ok(),
        false => None,
    }
}

fn entry(raw: &Value) -> Entry {
    let path = |k: &str| raw[k].as_str().map(str::to_string);
    Entry {
        real: path("support-texture"),
        logo: path("wheel"),
        boxed: path("box-3D"),
        look: look(&raw["look"]),
    }
}

fn rgb(v: &Value) -> Option<[u8; 3]> {
    let s = v.as_str()?;
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let at = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([at(0)?, at(2)?, at(4)?])
}

fn look(raw: &Value) -> Option<Look> {
    if raw["sure"].as_bool() != Some(true) {
        return None;
    }
    let ground = rgb(&raw["ground"])?;
    let b = &raw["band"];
    if b.is_null() {
        return Some(Look { ground, band: None });
    }
    let size = b["size"].as_f64()? as f32;
    if !(size > 0.0 && size < 0.5) {
        return None;
    }
    let band = Band::parse(b["edge"].as_str()?, size, &rgb(&b["colour"])?)?;
    Some(Look {
        ground,
        band: Some(band),
    })
}
