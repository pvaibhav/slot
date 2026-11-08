use slot_input::Millis;
use slot_store::{Collection, Platform, Recent};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Choice {
    Collection(usize),
    New,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Layout {
    Single,
    Pair,
    Trio,
    Carousel,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Seat {
    pub shelf: usize,
    pub index: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Selecting {
        checked: Vec<Recent>,
        seat: Option<Seat>,
    },
    Choosing {
        games: Vec<Recent>,
        from: Seat,
        multi: bool,
        choices: Vec<Choice>,
        at: usize,
        since: Millis,
    },
    Removing {
        game: Recent,
        collection: usize,
        last: bool,
        since: Millis,
    },
    MovingGame {
        collection: usize,
        before: Vec<Recent>,
        from: usize,
        hold: Option<Hold>,
        done: usize,
        since: Millis,
    },
    MovingCollection {
        before: Vec<String>,
        row: Vec<String>,
        at: usize,
        hold: Option<Hold>,
        done: usize,
        since: Millis,
    },
}

impl Edit {
    pub fn selecting() -> Edit {
        Edit::Selecting {
            checked: Vec::new(),
            seat: None,
        }
    }

    pub fn toggle(&mut self, game: Recent) {
        let Edit::Selecting { checked, .. } = self else {
            return;
        };
        match checked.iter().position(|g| *g == game) {
            Some(at) => {
                checked.remove(at);
            }
            None => checked.push(game),
        }
    }

    pub fn is_checked(&self, platform: Platform, stem: &str) -> bool {
        match self {
            Edit::Selecting { checked, .. } => checked
                .iter()
                .any(|g| g.platform == platform && g.stem == stem),
            _ => false,
        }
    }
}

pub fn choices(collections: &[Collection], games: &[Recent]) -> Vec<Choice> {
    let mut out: Vec<Choice> = collections
        .iter()
        .enumerate()
        .filter(|(_, c)| !missing(c, games).is_empty())
        .map(|(i, _)| Choice::Collection(i))
        .collect();
    match out.len() {
        2 => out.insert(1, Choice::New),
        _ => out.push(Choice::New),
    }
    out
}

pub fn layout(n: usize) -> Layout {
    match n {
        0 | 1 => Layout::Single,
        2 => Layout::Pair,
        3 => Layout::Trio,
        _ => Layout::Carousel,
    }
}

pub fn opening_pick(choices: &[Choice]) -> usize {
    choices.iter().position(|c| *c == Choice::New).unwrap_or(0)
}

pub fn step_pick(at: usize, by: i32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    (at as i32 + by).rem_euclid(n as i32) as usize
}

pub fn missing(collection: &Collection, games: &[Recent]) -> Vec<Recent> {
    games
        .iter()
        .filter(|g| !collection.games.contains(g))
        .cloned()
        .collect()
}

pub fn reorder_with_hidden(order: &[String], visible: &[String]) -> Vec<String> {
    let mut leading = Vec::new();
    let mut after: Vec<(String, Vec<String>)> = Vec::new();
    let mut anchor: Option<String> = None;
    for name in order {
        if visible.contains(name) {
            anchor = Some(name.clone());
            continue;
        }
        match &anchor {
            None => leading.push(name.clone()),
            Some(a) => match after.iter_mut().find(|(k, _)| k == a) {
                Some((_, list)) => list.push(name.clone()),
                None => after.push((a.clone(), vec![name.clone()])),
            },
        }
    }
    let mut out = leading;
    for name in visible {
        out.push(name.clone());
        if let Some((_, list)) = after.iter().find(|(k, _)| k == name) {
            out.extend(list.iter().cloned());
        }
    }
    out
}

pub fn stack_games(collection: &Collection, shown: &[Recent]) -> Vec<Recent> {
    collection
        .games
        .iter()
        .filter(|g| shown.contains(g))
        .take(3)
        .cloned()
        .collect()
}

pub fn remove_stack(shelf: &[Recent], index: usize) -> Vec<Recent> {
    let rest: Vec<&Recent> = shelf
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, g)| g)
        .collect();
    let n = rest.len();
    if n == 0 {
        return Vec::new();
    }
    let front = index % n;
    let mut out = vec![rest[front].clone()];
    for at in [(front + n - 1) % n, (front + 1) % n] {
        if at != front && !out.contains(rest[at]) {
            out.push(rest[at].clone());
        }
    }
    out
}

pub const ENTER_SLIDE_MS: Millis = 220;
pub const ENTER_MOVE_MS: Millis = 360;
pub const ENTER_FADE_MS: Millis = 140;
pub const ADD_SLOT_MS: Millis = 220;
pub const KEYS_OUT_MS: Millis = 160;
pub const ADD_PAUSE_MS: Millis = 200;
pub const OUT_MS: Millis = 110;
pub const HOME_MS: Millis = 360;
pub const BACK_HOME_MS: Millis = 340;
pub const NEIGHBORS_IN_MS: Millis = 260;
pub const NEIGHBORS_LEAD_MS: Millis = 60;
pub const REMOVE_RISE_MS: Millis = 300;
pub const FAN_MS: Millis = 380;
pub const LAST_RISE_MS: Millis = 320;
pub const GATHER_MS: Millis = 380;
pub const SPOT_MS: Millis = 260;
pub const CAMERA_MS: Millis = 380;
pub const JIGGLE_DELAY_MS: Millis = 150;
pub const JIGGLE_RAMP_MS: Millis = 120;
pub const HOLD_JIGGLE_DEG: f32 = 2.2;
pub const HOLD_JIGGLE_HZ: f32 = 6.5;
pub const SOFT_JIGGLE_DEG: f32 = 1.6;
pub const SOFT_JIGGLE_MS: f32 = 420.0;
pub const NEIGHBOR_DIM: f32 = 0.75;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Hold {
    pub down: Millis,
    pub up: Option<Millis>,
    pub dir: i32,
}

pub fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

pub fn ease_out(u: f32) -> f32 {
    1.0 - (1.0 - clamp01(u)).powi(3)
}

pub fn ease_in(u: f32) -> f32 {
    clamp01(u).powi(3)
}

pub fn ease_in_out(u: f32) -> f32 {
    let u = clamp01(u);
    match u < 0.5 {
        true => 4.0 * u * u * u,
        false => 1.0 - (-2.0 * u + 2.0).powi(3) / 2.0,
    }
}

pub fn progress(now: Millis, from: Millis, ms: Millis) -> f32 {
    clamp01(now.saturating_sub(from) as f32 / ms.max(1) as f32)
}

fn ramp(since_press: Millis) -> f32 {
    clamp01(since_press.saturating_sub(JIGGLE_DELAY_MS) as f32 / JIGGLE_RAMP_MS as f32)
}

pub fn hold_jiggle(since_press: Millis) -> f32 {
    if since_press < JIGGLE_DELAY_MS {
        return 0.0;
    }
    let secs = (since_press - JIGGLE_DELAY_MS) as f32 / 1000.0;
    HOLD_JIGGLE_DEG * ramp(since_press) * (secs * HOLD_JIGGLE_HZ * std::f32::consts::TAU).sin()
}

pub fn soft_jiggle(t: Millis, phase: f32) -> f32 {
    SOFT_JIGGLE_DEG * ((t as f32 / SOFT_JIGGLE_MS + phase) * std::f32::consts::TAU).sin()
}

pub fn neighbor_dim(since_press: Millis) -> f32 {
    1.0 + (NEIGHBOR_DIM - 1.0) * ramp(since_press)
}

fn spots(held: Millis) -> f32 {
    let s = SPOT_MS as f32;
    let x = held as f32;
    match x < s {
        true => 0.5 * x * x / (s * s),
        false => 0.5 + (x - s) / s,
    }
}

pub fn travel(hold: &Hold, now: Millis, room: usize) -> f32 {
    let room = room as f32;
    let until = hold.up.map_or(now, |up| up.min(now));
    let moved = spots(until.saturating_sub(hold.down)).min(room);
    let Some(up) = hold.up.filter(|up| now >= *up) else {
        return moved;
    };
    let goal = (moved - 1e-4).ceil().max(1.0).min(room);
    let span = ((goal - moved) * SPOT_MS as f32 * 1.6).max(120.0);
    moved + (goal - moved) * ease_out((now - up) as f32 / span)
}

pub fn camera(lead: f32, released: bool, since_release: Millis) -> f32 {
    let held = lead.signum() * (lead.abs() - 1.0).max(0.0);
    match released {
        false => held,
        true => held + (lead - held) * ease_in_out(since_release as f32 / CAMERA_MS as f32),
    }
}

pub const PASS_DIP: f32 = 22.0;
pub const PASS_SHRINK: f32 = 0.86;

pub fn pass(frac: f32) -> (f32, f32, f32) {
    let frac = clamp01(frac);
    let arc = (frac * std::f32::consts::PI).sin();
    (frac, arc * PASS_DIP, 1.0 + (PASS_SHRINK - 1.0) * arc)
}

pub fn move_still(hold: Option<&Hold>, now: Millis) -> f32 {
    match hold {
        None => 1.0,
        Some(Hold { up: None, .. }) => 0.0,
        Some(Hold { up: Some(up), .. }) => progress(now, up + CAMERA_MS, JIGGLE_RAMP_MS),
    }
}

pub const FAN_DROP: f32 = 0.12;

pub fn fan_fade(f: f32) -> f32 {
    1.0 - ease_out(f / FAN_DROP)
}
