use slot_ui::palette::{arranged, chip_grid, step, Chip, Dir};
use slot_ui::shell_presets;

fn name(i: usize) -> &'static str {
    shell_presets()[i].0
}

#[test]
fn neutrals_run_white_to_black() {
    let (neutrals, _) = arranged();
    assert_eq!(name(neutrals[0]), "Made in Wario white");
    assert_eq!(name(*neutrals.last().unwrap()), "Game Boy black");
    assert_eq!(neutrals.len(), 9);
}

#[test]
fn colors_start_with_red() {
    let (_, colors) = arranged();
    assert_eq!(name(colors[0]), "Pokémon Red");
    assert_eq!(colors.len(), shell_presets().len() - 9);
}

#[test]
fn first_row_is_current_auto_custom() {
    let g = chip_grid(0.0, 0.0);
    assert_eq!(g[0].chip, Chip::Current);
    assert_eq!(g[1].chip, Chip::Auto);
    assert_eq!(g[2].chip, Chip::Custom);
    assert_eq!((g[1].x, g[2].x), (54.0, 108.0));
    assert!(matches!(g[3].chip, Chip::Preset(_)));
    assert_eq!(g[3].y, 68.0);
}

#[test]
fn groups_have_gaps() {
    let g = chip_grid(0.0, 0.0);
    let (neutrals, _) = arranged();
    let first_color = 3 + neutrals.len();
    let last_neutral = first_color - 1;
    assert_eq!(g[first_color].y - g[last_neutral].y, 68.0);
    assert_eq!(g[first_color].x, 0.0);
}

#[test]
fn up_from_the_second_row_reaches_the_first() {
    let g = chip_grid(0.0, 0.0);
    assert_eq!(step(&g, 4, Dir::Up), 1);
    assert_eq!(step(&g, 1, Dir::Down), 4);
    assert_eq!(step(&g, 1, Dir::Left), 0);
    assert_eq!(step(&g, 0, Dir::Left), 0);
    assert_eq!(step(&g, 0, Dir::Up), 0);
    assert_eq!(step(&g, 2, Dir::Right), 2);
    assert_eq!(g[3 + 5].x, 0.0, "five to a row");
}

#[test]
fn headings_keep_one_size_whatever_their_length() {
    let tall = |label: &str| {
        let face = slot_ui::heading_face(label);
        (0..face.h as usize)
            .filter(|y| (0..face.w as usize).any(|x| face.rgba[(y * face.w as usize + x) * 4 + 3] > 128))
            .count()
    };
    assert!(tall("Finish").abs_diff(tall("Label Printer")) <= 2);
}
