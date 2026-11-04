use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

pub const CART_SHELL_FILE: &str = "Config/cart_shell.txt";
pub const LABELS_SHELL_FILE: &str = "Labels/cart_shell.txt";

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Outline {
    Auto,
    Notched,
    Rounded,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ShellFinish {
    Solid,
    Clear,
    Glitter,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct ShellChoice {
    pub outline: Outline,
    pub colour: [u8; 3],
    pub finish: ShellFinish,
}

impl ShellChoice {
    pub fn parse(value: &str) -> Option<ShellChoice> {
        let mut words = value.split_whitespace();
        let outline = match words.next()? {
            "auto" => Outline::Auto,
            "notched" => Outline::Notched,
            "rounded" => Outline::Rounded,
            _ => return None,
        };
        let hex = words.next()?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let colour = [0, 2, 4].map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or(0));
        let finish = match words.next()? {
            "solid" => ShellFinish::Solid,
            "clear" => ShellFinish::Clear,
            "glitter" => ShellFinish::Glitter,
            _ => return None,
        };
        words.next().is_none().then_some(ShellChoice {
            outline,
            colour,
            finish,
        })
    }

    pub fn to_value(&self) -> String {
        let outline = match self.outline {
            Outline::Auto => "auto",
            Outline::Notched => "notched",
            Outline::Rounded => "rounded",
        };
        let finish = match self.finish {
            ShellFinish::Solid => "solid",
            ShellFinish::Clear => "clear",
            ShellFinish::Glitter => "glitter",
        };
        let [r, g, b] = self.colour;
        format!("{outline} {r:02x}{g:02x}{b:02x} {finish}")
    }
}

pub fn key(stem: &str) -> String {
    stem.nfc().collect()
}

pub fn choices(text: &str) -> HashMap<String, ShellChoice> {
    crate::ini::parse(text)
        .into_iter()
        .filter_map(|(stem, value)| Some((key(&stem), ShellChoice::parse(&value)?)))
        .collect()
}

pub fn layered(system: &str, labels: &str) -> HashMap<String, ShellChoice> {
    let mut out = choices(system);
    for (stem, value) in crate::ini::parse(labels) {
        if value.trim() == "auto" {
            out.remove(&key(&stem));
        } else if let Some(choice) = ShellChoice::parse(&value) {
            out.insert(key(&stem), choice);
        }
    }
    out
}

pub fn write_choice(
    root: &std::path::Path,
    stem: &str,
    choice: Option<&ShellChoice>,
) -> std::io::Result<()> {
    let path = root.join(LABELS_SHELL_FILE);
    let mut text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let want = key(stem);
    let others: Vec<String> = crate::ini::parse(&text)
        .into_keys()
        .filter(|k| key(k) == want && *k != want)
        .collect();
    for raw in others {
        text = crate::ini::set(&text, &raw, None)?;
    }
    let system = std::fs::read_to_string(root.join(CART_SHELL_FILE)).unwrap_or_default();
    let value = match choice {
        Some(c) => Some(c.to_value()),
        None => choices(&system)
            .contains_key(&want)
            .then(|| "auto".to_string()),
    };
    let out = crate::ini::set(&text, &want, value.as_deref())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    crate::atomic::atomic_write(&path, out.as_bytes())
}
