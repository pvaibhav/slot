use scraper::{Html, Selector};
use slot_store::{Cart, Platform};
use unicode_normalization::UnicodeNormalization;

use crate::{Error, Transport};
const BASE: &str = "https://gamesdb.launchbox-app.com";
const IMAGE: &str = "https://images.launchbox-app.com/";

pub(crate) fn allowed_url(url: &str) -> bool {
    url.starts_with(&format!("{BASE}/")) || url.starts_with(IMAGE)
}

fn unavailable(message: &str) -> Error {
    Error::Unavailable(message.into())
}

fn selector(s: &str) -> Selector {
    Selector::parse(s).expect("static CSS selector")
}

pub(crate) fn title(stem: &str) -> String {
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

pub(crate) fn normalize(value: &str) -> String {
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

fn encode(query: &str) -> String {
    let mut result = String::new();
    for b in query.bytes() {
        if b.is_ascii_alphanumeric() {
            result.push(b as char);
        } else {
            result.push_str(&format!("%{b:02X}"));
        }
    }
    result
}

/// The source's names for a cart's platform, its own first. Game Boy and Game Boy Color share
/// ROM extensions, so a cart may sit on either shelf.
fn platforms(platform: Platform) -> &'static [&'static str] {
    match platform {
        Platform::Gba => &["Nintendo Game Boy Advance"],
        Platform::Gb => &["Nintendo Game Boy", "Nintendo Game Boy Color"],
        Platform::Gbc => &["Nintendo Game Boy Color", "Nintendo Game Boy"],
    }
}

pub(crate) fn game_id(page: &str, wanted: &str, platform: Platform) -> Result<u64, Error> {
    let doc = Html::parse_document(page);
    for name in platforms(platform) {
        match ids(&doc, wanted, name).as_slice() {
            [] => continue,
            [id] => return Ok(*id),
            _ => break,
        }
    }
    Err(unavailable("no unique title match"))
}

fn ids(doc: &Html, wanted: &str, platform: &str) -> Vec<u64> {
    let mut found = Vec::new();
    for link in doc.select(&selector("a[href^='/games/details/']")) {
        let Some(heading) = link.select(&selector("h3")).next() else {
            continue;
        };
        let name = heading.text().collect::<String>();
        let listed = link
            .select(&selector("p"))
            .any(|p| p.text().collect::<String>().trim() == platform);
        if !listed || normalize(&name) != wanted {
            continue;
        }
        let href = link.value().attr("href").unwrap_or_default();
        if let Some(id) = href
            .strip_prefix("/games/details/")
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse::<u64>().ok())
        {
            if !found.contains(&id) {
                found.push(id);
            }
        }
    }
    found
}

fn known(title: &str) -> Option<u64> {
    Some(match title {
        "advance wars" => 2367,
        "aladdin" => 3215,
        "dexters laboratory deesaster strikes" => 26190,
        "lady sia" => 14182,
        "legend of zelda a link to the past and four swords" => 8376,
        "metal slug advance" => 10743,
        "metroid zero mission" => 3552,
        "super mario advance" => 2225,
        "tetris worlds" => 3827,
        "tom clancys rainbow six rogue spear" => 3412,
        "warioware inc mega microgames" => 3917,
        _ => return None,
    })
}

fn regions(cart: &Cart) -> &'static [&'static str] {
    let stem = cart.stem.to_lowercase();
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
    match cart.code.as_bytes().get(3) {
        Some(b'J') => &["Japan"],
        Some(b'P') | Some(b'D') | Some(b'F') | Some(b'I') | Some(b'S') => &["Europe"],
        _ => &["North America", "United States"],
    }
}

pub(crate) fn image_url(page: &str, cart: &Cart) -> Result<String, Error> {
    let doc = Html::parse_document(page);
    for region in regions(cart) {
        for link in doc.select(&selector("a[data-title][href]")) {
            let description = link.value().attr("data-title").unwrap_or_default();
            let url = link.value().attr("href").unwrap_or_default();
            if description.contains(" - Cart - Front Image")
                && !description.contains("Fanart")
                && description.ends_with(&format!("({region})"))
                && url.starts_with(IMAGE)
            {
                return Ok(url.to_owned());
            }
        }
    }
    Err(unavailable("no matching regional cartridge artwork"))
}

pub(crate) fn resolve(
    http: &mut impl Transport,
    cart: &Cart,
    canonical_title: Option<&str>,
) -> Result<String, Error> {
    let title = canonical_title
        .map(normalize)
        .unwrap_or_else(|| title(&cart.stem));
    if title.is_empty() {
        return Err(unavailable("empty game title"));
    }
    let id = match Some(&title)
        .filter(|_| cart.platform == Platform::Gba)
        .and_then(|t| known(t))
    {
        Some(id) => id,
        None => {
            let page = http.get(&format!("{BASE}/games/results?id={}", encode(&title)))?;
            game_id(&String::from_utf8_lossy(&page), &title, cart.platform)?
        }
    };
    let page = http.get(&format!("{BASE}/games/images/{id}"))?;
    image_url(&String::from_utf8_lossy(&page), cart)
}
