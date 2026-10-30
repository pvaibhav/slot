use crate::shell::shell_presets;

const COLS: usize = 5;
const STEP: f32 = 54.0;
const GROUP_GAP: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chip {
    Current,
    Auto,
    Custom,
    Preset(usize),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub chip: Chip,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

fn chroma([r, g, b]: [u8; 3]) -> i32 {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    r.max(g).max(b) - r.min(g).min(b)
}

fn light([r, g, b]: [u8; 3]) -> f32 {
    0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
}

fn hue(rgb: [u8; 3]) -> f32 {
    let [r, g, b] = rgb.map(|c| c as f32);
    let max = r.max(g).max(b);
    let d = chroma(rgb) as f32;
    let h = if max == r {
        (g - b) / d
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    let deg = (h * 60.0 + 360.0).rem_euclid(360.0);
    if deg >= 350.0 {
        deg - 360.0
    } else {
        deg
    }
}

pub fn arranged() -> (Vec<usize>, Vec<usize>) {
    let presets = shell_presets();
    let color = |i: usize| presets[i].1.colour;
    let (mut neutrals, mut colors): (Vec<usize>, Vec<usize>) =
        (0..presets.len()).partition(|&i| chroma(color(i)) < 24);
    neutrals.sort_by(|&a, &b| light(color(b)).total_cmp(&light(color(a))));
    colors.sort_by(|&a, &b| hue(color(a)).total_cmp(&hue(color(b))));
    (neutrals, colors)
}

pub fn chip_grid(x0: f32, y0: f32) -> Vec<Placed> {
    let (neutrals, colors) = arranged();
    let preset = |list: Vec<usize>| list.into_iter().map(Chip::Preset).collect::<Vec<_>>();
    let groups = [
        vec![Chip::Current, Chip::Auto, Chip::Custom],
        preset(neutrals),
        preset(colors),
    ];
    let mut out = Vec::new();
    let mut y = y0;
    for (g, group) in groups.iter().enumerate() {
        if g > 0 {
            y += GROUP_GAP;
        }
        for row in group.chunks(COLS) {
            for (i, chip) in row.iter().enumerate() {
                out.push(Placed {
                    chip: *chip,
                    x: x0 + i as f32 * STEP,
                    y,
                });
            }
            y += STEP;
        }
    }
    out
}

pub fn step(grid: &[Placed], at: usize, dir: Dir) -> usize {
    let here = grid[at];
    let along = |row_y: f32| grid.iter().enumerate().filter(move |(_, p)| p.y == row_y);
    match dir {
        Dir::Left => along(here.y)
            .filter(|(_, p)| p.x < here.x)
            .max_by(|a, b| a.1.x.total_cmp(&b.1.x))
            .map_or(at, |(i, _)| i),
        Dir::Right => along(here.y)
            .filter(|(_, p)| p.x > here.x)
            .min_by(|a, b| a.1.x.total_cmp(&b.1.x))
            .map_or(at, |(i, _)| i),
        Dir::Up | Dir::Down => {
            let above = dir == Dir::Up;
            let row = grid
                .iter()
                .map(|p| p.y)
                .filter(|&y| if above { y < here.y } else { y > here.y })
                .fold(None, |best: Option<f32>, y| {
                    Some(match best {
                        Some(b) if above => b.max(y),
                        Some(b) => b.min(y),
                        None => y,
                    })
                });
            row.and_then(|y| {
                along(y).min_by(|a, b| {
                    (a.1.x - here.x).abs().total_cmp(&(b.1.x - here.x).abs())
                })
            })
            .map_or(at, |(i, _)| i)
        }
    }
}
