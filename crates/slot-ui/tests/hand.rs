use slot_ui::hand::{hand_font, ink_for, marker_art, reveal};

#[test]
fn repeated_letters_cycle_through_three_variants() {
    let font = hand_font().expect("NoSmear loads");
    let (placed, _) = font.shape("MMMM", 40.0);
    let ids: Vec<u16> = placed.iter().map(|g| g.glyph).collect();
    assert_eq!(ids.len(), 4);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);
}

#[test]
fn a_word_cycles_through_all_five_shapes() {
    let font = hand_font().expect("NoSmear loads");
    let (placed, _) = font.shape("MMMMMM", 40.0);
    let ids: Vec<u16> = placed.iter().map(|g| g.glyph).collect();
    let mut first_five = ids[..5].to_vec();
    first_five.sort();
    first_five.dedup();
    assert_eq!(first_five.len(), 5, "{ids:?}");
    assert_eq!(ids[5], ids[0], "{ids:?}");
}

#[test]
fn first_letter_of_a_word_is_the_base_glyph() {
    let font = hand_font().expect("NoSmear loads");
    let (alone, _) = font.shape("M", 40.0);
    let (word, _) = font.shape("MM", 40.0);
    assert_eq!(alone[0].glyph, word[0].glyph);
}

#[test]
fn a_title_puts_ink_in_the_box() {
    let font = hand_font().expect("NoSmear loads");
    let cov = font.coverage(196, 86, &["Minish Cap".to_string()], 36.0);
    let inked = cov.iter().filter(|&&a| a > 128).count();
    assert!(inked > 500, "only {inked} inked pixels");
}

fn ink_sum(cov: &[u8]) -> u64 {
    cov.iter().map(|a| *a as u64).sum()
}

#[test]
fn every_inked_glyph_of_a_title_has_strokes() {
    let font = hand_font().expect("NoSmear loads");
    for word in "Bootleg 4 in 1 Minish Cap".split(' ') {
        let (placed, _) = font.shape(word, 40.0);
        for g in placed {
            assert!(
                font.has_strokes(g.glyph),
                "glyph {} of {word} has no strokes",
                g.glyph
            );
        }
    }
}

#[test]
fn missing_characters_still_write() {
    let font = hand_font().expect("NoSmear loads");
    let w = font.write("Pokémon ア", &[], "Pokemon", 264, 136, [40, 40, 40]);
    assert!(ink_sum(&w.ink) > 0);
    let frames = reveal(&w, 64);
    assert_eq!(frames.len(), 64);
    let last = ink_sum(&frames[63].0) as f64;
    assert!(last >= ink_sum(&w.ink) as f64 * 0.99, "last frame {last}");
}

#[test]
fn reveal_ends_with_the_whole_label() {
    let font = hand_font().expect("NoSmear loads");
    let w = font.write("Minish Cap", &[], "Minish Cap", 264, 136, [40, 120, 80]);
    let whole = ink_sum(&w.ink) as f64;
    let frames = reveal(&w, 64);
    assert!(ink_sum(&frames[0].0) as f64 <= whole * 0.10);
    assert!(ink_sum(&frames[63].0) as f64 >= whole * 0.99);
}

fn lum(c: [u8; 3]) -> f32 {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
    let (x, y) = (lum(a), lum(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

#[test]
fn the_ink_stands_out_on_every_shell() {
    for shell in [
        [0x30, 0x31, 0x36],
        [0xa8, 0xa6, 0xa0],
        [0xe6, 0xe0, 0xd2],
        [40, 150, 110],
        [200, 40, 50],
        [120, 80, 170],
        [230, 200, 40],
        [60, 110, 200],
        [250, 140, 30],
    ] {
        let ink = ink_for(shell);
        assert!(
            contrast(ink, shell) >= 4.0,
            "{ink:?} on {shell:?} is only {:.2}",
            contrast(ink, shell)
        );
    }
}

#[test]
fn the_marker_is_the_color_of_its_ink() {
    let mean = |rgba: &[u8]| {
        let (sum, n) = rgba
            .chunks_exact(4)
            .filter(|p| p[3] > 200)
            .fold((0.0, 0), |(s, n), p| (s + lum([p[0], p[1], p[2]]), n + 1));
        sum / n as f32
    };
    let light = marker_art([236, 236, 232]);
    let dark = marker_art([24, 24, 28]);
    assert!(
        mean(&light.cap) > mean(&dark.cap) + 0.3,
        "white cap {} vs black cap {}",
        mean(&light.cap),
        mean(&dark.cap)
    );
}

#[test]
fn the_tag_line_sits_clear_of_the_title() {
    let font = hand_font().expect("NoSmear loads");
    let tags = ["Translated".to_string()];
    let w = font.write("Mother 3", &tags, "Mother 3", 264, 136, [240, 238, 229]);
    let inked: Vec<bool> = (0..w.h)
        .map(|y| (0..w.w).any(|x| w.ink[(y * w.w + x) as usize] > 40))
        .collect();
    let top = inked.iter().position(|r| *r).unwrap();
    let bottom = inked.iter().rposition(|r| *r).unwrap();
    let gap = inked[top..=bottom].iter().filter(|r| !**r).count();
    assert!(gap >= 2, "only {gap} clear rows between title and tag");
}

fn starts(word: &str, seeds: std::ops::Range<u64>) -> Vec<u16> {
    let font = hand_font().expect("NoSmear loads");
    seeds
        .map(|seed| font.shape_from(word, 40.0, seed).0[0].glyph)
        .collect()
}

#[test]
fn a_word_starts_on_a_shape_its_seed_picks() {
    let mut firsts = starts("Mario", 0..40);
    assert_eq!(starts("Mario", 0..40), firsts);
    firsts.sort();
    firsts.dedup();
    assert_eq!(firsts.len(), 5, "{firsts:?}");
}

#[test]
fn a_seeded_word_draws_on_all_five_shapes() {
    let font = hand_font().expect("NoSmear loads");
    let mut seen: Vec<u16> = (0..10)
        .flat_map(|seed| font.shape_from("MMMMMMMMMMMM", 40.0, seed).0)
        .map(|g| g.glyph)
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 5, "{seen:?}");
}

#[test]
fn a_seeded_word_is_not_a_fixed_cycle() {
    let font = hand_font().expect("NoSmear loads");
    let broken = (0..20).any(|seed| {
        let ids: Vec<u16> = font
            .shape_from("MMMMMMMMMMMM", 40.0, seed)
            .0
            .iter()
            .map(|g| g.glyph)
            .collect();
        ids[5] != ids[0] || ids[6] != ids[1]
    });
    assert!(broken);
}

#[test]
fn a_doubled_letter_never_repeats_its_shape() {
    let font = hand_font().expect("NoSmear loads");
    for seed in 0..50 {
        let (placed, _) = font.shape_from("Shell", 40.0, seed);
        assert_ne!(placed[3].glyph, placed[4].glyph, "seed {seed}");
    }
}

#[test]
fn each_word_of_a_line_picks_its_own_start() {
    let font = hand_font().expect("NoSmear loads");
    let differs = (0..20).any(|seed| {
        let (placed, _) = font.shape_from("Mm Mm", 40.0, seed);
        placed[0].glyph != placed[3].glyph
    });
    assert!(differs);
}

#[test]
fn a_seeded_line_is_as_wide_as_its_glyphs() {
    let font = hand_font().expect("NoSmear loads");
    let widths: Vec<f32> = (0..10)
        .map(|seed| font.shape_from("Minish Cap", 40.0, seed).1)
        .collect();
    assert!(
        widths.iter().any(|w| (w - widths[0]).abs() > 0.5),
        "{widths:?}"
    );
}

#[test]
fn accented_letters_have_their_own_strokes() {
    let font = hand_font().expect("NoSmear loads");
    for word in "Pokémon Señor Über Façade naïve Ångström ÀÉÎÕÜ àéîõüÿ".split(' ')
    {
        let (placed, _) = font.shape_from(word, 40.0, 7);
        for g in placed {
            assert_ne!(g.glyph, 0, "{word} has a missing glyph");
            assert!(
                font.has_strokes(g.glyph),
                "glyph {} of {word} has no strokes",
                g.glyph
            );
        }
    }
}

#[test]
fn accents_on_capitals_clear_the_line_above() {
    let font = hand_font().expect("NoSmear loads");
    let w = font.write("Ça Ëtê Ñoël", &[], "Ça Ëtê Ñoël", 110, 260, [168, 166, 160]);
    let inked: Vec<bool> = (0..w.h)
        .map(|y| (0..w.w).any(|x| w.ink[(y * w.w + x) as usize] > 40))
        .collect();
    let bands = inked.windows(2).filter(|p| p[0] && !p[1]).count();
    assert!(bands >= 3, "lines run together: {bands} bands");
}

#[test]
fn the_pen_lingers_on_an_accent() {
    let font = hand_font().expect("NoSmear loads");
    let w = font.write(
        "Pokémon Recharged Yellow",
        &[],
        "Pokémon",
        196,
        86,
        [168, 166, 160],
    );
    let length = |s: &Vec<[f32; 2]>| {
        s.windows(2)
            .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
            .sum::<f32>()
    };
    let accent = w
        .path
        .iter()
        .min_by(|a, b| length(a).total_cmp(&length(b)))
        .expect("an accent stroke")
        .clone();
    let near = |p: [f32; 2]| {
        accent
            .iter()
            .any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < w.nib)
    };
    let frames = reveal(&w, 64);
    let on = frames.iter().filter(|(_, pen)| near(*pen)).count();
    assert!(on >= 2, "the pen spends {on} frames on the accent");
}

#[test]
fn the_pen_moves_through_the_air_between_strokes() {
    let font = hand_font().expect("NoSmear loads");
    let w = font.write(
        "Pokémon Recharged Yellow",
        &[],
        "Pokémon",
        196,
        86,
        [168, 166, 160],
    );
    let frames = reveal(&w, 64);
    let biggest = frames
        .windows(2)
        .map(|p| (p[1].1[0] - p[0].1[0]).hypot(p[1].1[1] - p[0].1[1]))
        .fold(0.0, f32::max);
    assert!(
        biggest < w.w as f32 * 0.4,
        "the pen jumps {biggest} px in one frame"
    );
}
