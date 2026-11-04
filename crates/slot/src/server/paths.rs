use std::path::{Path, PathBuf};

const HIDDEN: [&str; 2] = ["system", "cache"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub status: u16,
    pub message: String,
}

impl Refused {
    pub fn new(status: u16, message: impl Into<String>) -> Refused {
        Refused { status, message: message.into() }
    }
}

pub fn is_hidden(name: &str) -> bool {
    let trimmed = name.trim_end_matches(['.', ' ']);
    HIDDEN.iter().any(|h| trimmed.eq_ignore_ascii_case(h))
}

pub fn resolve(root: &Path, segments: &[String]) -> Result<PathBuf, Refused> {
    let Some(first) = segments.first() else {
        return Err(Refused::new(400, "path required"));
    };
    if is_hidden(first) {
        return Err(Refused::new(403, "forbidden"));
    }
    let mut out = root.to_path_buf();
    for s in segments {
        if s != "." && s != ".." && s.ends_with(['.', ' ']) {
            return Err(Refused::new(400, format!("{s:?} ends with a space or dot, which the card cannot store")));
        }
        if s.is_empty() || s == "." || s == ".." || s.contains(['/', '\\', '\0']) {
            return Err(Refused::new(403, "forbidden"));
        }
        out.push(s);
    }
    let base = root.canonicalize().map_err(|_| Refused::new(500, "card unavailable"))?;
    let mut probe = out.as_path();
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let anchor = loop {
        match probe.canonicalize() {
            Ok(p) => break p,
            Err(_) => {
                let (Some(name), Some(parent)) = (probe.file_name(), probe.parent()) else {
                    return Err(Refused::new(403, "forbidden"));
                };
                tail.push(name);
                probe = parent;
            }
        }
    };
    if !anchor.starts_with(&base) {
        return Err(Refused::new(403, "forbidden"));
    }
    let mut full = anchor;
    for name in tail.into_iter().rev() {
        full.push(name);
    }
    let hidden = full
        .strip_prefix(&base)
        .ok()
        .and_then(|rel| rel.components().next())
        .is_some_and(|c| is_hidden(&c.as_os_str().to_string_lossy()));
    if hidden {
        return Err(Refused::new(403, "forbidden"));
    }
    Ok(full)
}

pub fn card_name(name: &str) -> Result<(), Refused> {
    let bad = |why: &str| Err(Refused::new(400, format!("{name:?} {why}")));
    if name.is_empty() {
        return Err(Refused::new(400, "empty file name"));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return bad("ends with a space or dot, which the card cannot store");
    }
    if let Some((stem, _)) = name.rsplit_once('.') {
        if stem.ends_with(' ') {
            return bad("has a space before the extension, which the card cannot store");
        }
    }
    if name.chars().any(|c| c.is_control() || "\\/:*?\"<>|".contains(c)) {
        return bad("has a character the card cannot store");
    }
    Ok(())
}
