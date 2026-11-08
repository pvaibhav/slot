//! LaunchBox's names for games and platforms, and its cart fronts ranked best first. Only
//! `task labels:index` uses them, to build the index that `Artwork.pack` is made from.
use slot_store::Platform;
use unicode_normalization::UnicodeNormalization;

pub fn title(stem: &str) -> String {
    let mut depth = 0u32;
    let clean: String = stem
        .chars()
        .filter(|c| match c {
            '(' | '[' => {
                depth += 1;
                false
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                false
            }
            _ => depth == 0,
        })
        .collect();
    normalize(&clean.replace(", The", ""))
}

pub fn normalize(value: &str) -> String {
    let mut text = String::new();
    for c in value
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
    {
        match c {
            '&' => text.push_str(" and "),
            '$' => text.push('s'),
            '\'' | '’' => {}
            c if c.is_alphanumeric() => text.extend(c.to_lowercase()),
            _ => text.push(' '),
        }
    }
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    text.strip_prefix("the ")
        .or_else(|| text.strip_prefix("disneys "))
        .or_else(|| text.strip_prefix("lara croft "))
        .unwrap_or(&text)
        .to_owned()
}

/// The source's names for a cart's platform, its own first. Game Boy and Game Boy Color share
/// ROM extensions, so a cart may sit on either shelf.
pub fn platforms(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Gba | Platform::Gbav => &["Nintendo Game Boy Advance"],
        Platform::Gb => &["Nintendo Game Boy", "Nintendo Game Boy Color"],
        Platform::Gbc => &["Nintendo Game Boy Color", "Nintendo Game Boy"],
        #[cfg(feature = "nds")]
        Platform::Nds => &[],
    }
}

fn regions(stem: &str, code: &str) -> &'static [&'static str] {
    let stem = stem.to_lowercase();
    // Filename regions take precedence over a possibly shared header code.
    let tags: Vec<_> = stem
        .split('(')
        .skip(1)
        .filter_map(|s| s.split_once(')').map(|p| p.0))
        .collect();
    for (token, regions) in [
        ("usa", &["North America", "United States"][..]),
        ("europe", &["Europe"][..]),
        ("japan", &["Japan"][..]),
        ("australia", &["Australia", "Oceania"][..]),
    ] {
        if tags
            .iter()
            .any(|tag| tag.split(',').any(|s| s.trim() == token))
        {
            return regions;
        }
    }
    match code.as_bytes().get(3) {
        Some(b'J') => &["Japan"],
        Some(b'P') | Some(b'D') | Some(b'F') | Some(b'I') | Some(b'S') => &["Europe"],
        _ => &["North America", "United States"],
    }
}

pub const PHOTO: &str = "Cart - Front";
pub const RENDER: &str = "Fanart - Cart - Front";

/// Cart fronts, best first and at most three: a photo of the cart from its own region, a fanart
/// render of that cart, then the same from any other region. A render is a scan of the real label
/// on a stock shell, and for Game Boy and Game Boy Color it is most of what LaunchBox has.
pub fn rank<T: Clone>(stem: &str, code: &str, images: &[(&str, &str, T)]) -> Vec<T> {
    let own = regions(stem, code);
    let mut ranked: Vec<_> = images
        .iter()
        .filter_map(|(kind, region, image)| {
            let render = match *kind {
                PHOTO => false,
                RENDER => true,
                _ => return None,
            };
            let away = !own.contains(region);
            Some((u8::from(away) * 2 + u8::from(render), image.clone()))
        })
        .collect();
    ranked.sort_by_key(|(tier, _)| *tier);
    ranked.into_iter().take(3).map(|(_, image)| image).collect()
}
