use std::path::Path;

use slot_gfx::{Draw, OUT_H, OUT_W};
use slot_store::{StateEntry, RING_MAX};
use slot_ui::{hint_face, hint_width, Gauge, Polaroids, Printed, DOT, HINT_H, LEGEND, PLATE_H};

fn entry(stamp: &str) -> StateEntry {
    StateEntry {
        stamp: stamp.to_string(),
        state: Path::new("/nonexistent").join(format!("{stamp}.state")),
        thumb: Path::new("/nonexistent").join(format!("{stamp}.png")),
    }
}

fn switcher_with(n: usize) -> Polaroids {
    Polaroids::new(
        (0..n)
            .map(|i| entry(&format!("2026-08-09_{:02}-00-00", 14 - i)))
            .collect(),
    )
}

fn draw(p: &Polaroids) -> Vec<Draw> {
    let mut out = Vec::new();
    p.draw(Gauge::default(), None, Printed::default(), &mut out);
    out
}

#[derive(Copy, Clone, Debug)]
struct Quad {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    alpha: f32,
}

fn quads(out: &[Draw]) -> Vec<Quad> {
    out.iter()
        .map(|d| match *d {
            Draw::Rect { x, y, w, h, colour } => Quad {
                x,
                y,
                w,
                h,
                alpha: colour[3],
            },
            Draw::Tex {
                x, y, w, h, alpha, ..
            } => Quad { x, y, w, h, alpha },
            Draw::Part {
                x, y, w, h, alpha, ..
            } => Quad { x, y, w, h, alpha },
            Draw::Turned {
                x, y, w, h, alpha, ..
            } => Quad { x, y, w, h, alpha },
            Draw::Game
            | Draw::Shot { .. }
            | Draw::Capture
            | Draw::Quad { .. }
            | Draw::Paper { .. } => Quad {
                x: 0.0,
                y: 0.0,
                w: OUT_W as f32,
                h: OUT_H as f32,
                alpha: 1.0,
            },
        })
        .collect()
}

fn dot_rects(out: &[Draw]) -> Vec<Quad> {
    quads(out)
        .into_iter()
        .filter(|q| q.w == DOT && q.h == DOT)
        .collect()
}

fn highlighted_dots(out: &[Draw]) -> Vec<Quad> {
    dot_rects(out)
        .into_iter()
        .filter(|q| q.alpha > 0.9)
        .collect()
}

fn hint_quads(out: &[Draw]) -> Vec<Quad> {
    quads(out)
        .into_iter()
        .filter(|q| q.h == HINT_H as f32)
        .collect()
}

fn has_hint(p: &Polaroids, out: &[Draw], key: &str) -> bool {
    p.hints().iter().any(|h| h.key == key) && hint_quads(out).len() == p.hints().len()
}

#[test]
fn there_is_one_dot_per_entry_and_exactly_one_is_selected() {
    let mut p = switcher_with(5);
    p.right();
    let (count, selected) = p.dots();
    assert_eq!(count, 5);
    assert_eq!(selected, 1);
    let out = draw(&p);
    assert_eq!(dot_rects(&out).len(), 5, "one dot per saved state");
    assert_eq!(
        highlighted_dots(&out).len(),
        1,
        "exactly one dot reads as current"
    );
}

#[test]
fn the_title_follows_the_selection() {
    let mut p = switcher_with(3);
    let first = p.title("2026-08-09_14-36-05", true);
    p.right();
    let second = p.title("2026-08-09_14-36-05", true);
    assert_ne!(first, second, "the title did not move with the selection");
}

#[test]
fn the_back_hint_names_its_button() {
    let p = switcher_with(2);
    let out = draw(&p);
    assert!(has_hint(&p, &out, "B"), "no back hint");
}

#[test]
fn the_undo_hint_names_its_button_and_only_shows_when_offered() {
    let mut p = switcher_with(2);
    let out = draw(&p);
    assert!(
        !has_hint(&p, &out, "X"),
        "undo offered when there is nothing to undo"
    );

    p.set_undo(Some("Undo save"));
    let out = draw(&p);
    assert!(
        has_hint(&p, &out, "X"),
        "the undo does not say what triggers it"
    );
}

#[test]
fn the_legend_runs_left_to_right_along_the_bottom_plate() {
    let mut p = switcher_with(2);
    p.set_undo(Some("Undo save"));
    let out = draw(&p);

    let hints = hint_quads(&out);
    assert_eq!(hints.len(), p.hints().len());
    for pair in hints.windows(2) {
        assert!(
            pair[0].x + pair[0].w <= pair[1].x,
            "a hint at {} runs into the one after it",
            pair[0].x
        );
    }
    for h in hints {
        assert!(
            h.y >= OUT_H as f32 - PLATE_H && h.y + h.h <= OUT_H as f32,
            "a hint at {} left the bottom plate",
            h.y
        );
    }
}

#[test]
fn a_full_ring_of_dots_stays_clear_of_the_legend() {
    let mut p = switcher_with(RING_MAX);
    p.set_undo(Some("Undo save"));
    let out = draw(&p);

    let hints = hint_quads(&out);
    let dots = dot_rects(&out);
    assert_eq!(dots.len(), RING_MAX);
    for d in dots {
        assert!(
            d.x >= 0.0 && d.x + d.w <= OUT_W as f32,
            "a dot at {} is off the plate",
            d.x
        );
        for h in &hints {
            assert!(
                d.x + d.w <= h.x || d.x >= h.x + h.w,
                "a dot at {} runs into the {} hint at {}",
                d.x,
                h.w,
                h.x
            );
        }
    }
}

#[test]
fn the_legend_names_every_action_on_screen() {
    let mut p = switcher_with(3);
    let keys: Vec<&str> = p.hints().iter().map(|h| h.key).collect();
    assert!(keys.contains(&"A"), "load is unlabelled");
    assert!(keys.contains(&"Y"), "delete is unlabelled");
    assert!(keys.contains(&"B"), "back is unlabelled");
    assert!(!keys.contains(&"X"), "undo shown with nothing to undo");
    p.set_undo(Some("Undo save"));
    assert!(p.hints().iter().any(|h| h.key == "X"));
}

#[test]
fn the_switcher_puts_the_ways_out_on_the_left() {
    let mut p = switcher_with(3);
    p.set_undo(Some("Undo save"));
    let out = draw(&p);
    let mid = OUT_W as f32 / 2.0;
    for key in ["B", "Y"] {
        let r = hint_rect(&out, key).unwrap_or_else(|| panic!("no {key} hint"));
        assert!(r.x + r.w < mid, "{key} is not on the left");
    }
    for key in ["A", "X"] {
        let r = hint_rect(&out, key).unwrap_or_else(|| panic!("no {key} hint"));
        assert!(r.x > mid, "{key} is not on the right");
    }
}

fn hint_rect(out: &[Draw], key: &str) -> Option<Quad> {
    let i = match key {
        "X" => LEGEND.len(),
        _ => LEGEND.iter().position(|(k, _)| *k == key)?,
    };
    hint_quads(out).get(i).copied()
}

#[test]
fn a_hint_is_tighter_than_it_was() {
    let f = hint_face("A", "Load");
    assert!(f.w < 128, "the hint is still the old fixed width: {}", f.w);
}

#[test]
fn the_full_legend_fits_across_the_bar() {
    let mut p = switcher_with(3);
    p.set_undo(Some("Undo save"));
    let total: u32 = p.hints().iter().map(|h| hint_face(h.key, &h.label).w).sum();
    assert!(
        total < OUT_W,
        "the legend is {total}px wide, wider than the screen"
    );
}

#[test]
fn a_hint_is_laid_out_at_the_width_it_rasterises_to() {
    for (key, label) in [("A", "Load"), ("X", "Undo an interminable action")] {
        assert_eq!(hint_width(key, label), hint_face(key, label).w);
    }
}
