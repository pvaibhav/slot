use slot::editing::{
    choices, layout, missing, opening_pick, remove_stack, reorder_with_hidden, stack_games,
    step_pick, Choice, Edit, Layout,
};
use slot_store::{Collection, Platform, Recent};

fn gbc(stem: &str) -> Recent {
    Recent {
        platform: Platform::Gbc,
        stem: stem.into(),
    }
}

fn holding(name: &str, games: &[&str]) -> Collection {
    Collection {
        name: name.into(),
        games: games.iter().map(|g| gbc(g)).collect(),
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_collection_holding_the_game_is_not_offered() {
    let all = [holding("Pokemon", &["Crystal"]), holding("Puzzlers", &["Tetris"])];
    assert_eq!(
        choices(&all, &[gbc("Crystal")]),
        [Choice::Collection(1), Choice::New]
    );
}

#[test]
fn a_collection_holding_some_ticked_games_is_offered() {
    let all = [holding("Pokemon", &["Crystal"])];
    let ticked = [gbc("Crystal"), gbc("Picross"), gbc("Catrap")];
    assert_eq!(choices(&all, &ticked), [Choice::Collection(0), Choice::New]);
    assert_eq!(missing(&all[0], &ticked), [gbc("Picross"), gbc("Catrap")]);
}

#[test]
fn a_collection_holding_every_ticked_game_is_not_offered() {
    let all = [holding("Pokemon", &["Crystal", "Picross"])];
    assert_eq!(
        choices(&all, &[gbc("Picross"), gbc("Crystal")]),
        [Choice::New]
    );
}

#[test]
fn an_empty_collection_is_offered() {
    let all = [holding("Empty", &[])];
    assert_eq!(
        choices(&all, &[gbc("Crystal")]),
        [Choice::Collection(0), Choice::New]
    );
}

#[test]
fn layouts_follow_the_choice_count() {
    assert_eq!(layout(1), Layout::Single);
    assert_eq!(layout(2), Layout::Pair);
    assert_eq!(layout(3), Layout::Trio);
    assert_eq!(layout(4), Layout::Carousel);
    assert_eq!(layout(9), Layout::Carousel);
}

#[test]
fn the_screen_opens_on_new_collection() {
    let all = [holding("Pokemon", &[]), holding("Puzzlers", &[])];
    let offered = choices(&all, &[gbc("Crystal")]);
    let new = offered.iter().position(|c| *c == Choice::New).unwrap();
    assert_eq!(opening_pick(&offered), new);
}

#[test]
fn picking_wraps_both_ways() {
    assert_eq!(step_pick(0, -1, 4), 3);
    assert_eq!(step_pick(3, 1, 4), 0);
    assert_eq!(step_pick(1, 1, 4), 2);
}

#[test]
fn hidden_collections_keep_their_place_after_a_move() {
    let order = names(&["Racing", "Empty", "Pokemon", "Puzzlers"]);
    assert_eq!(
        reorder_with_hidden(&order, &names(&["Racing", "Puzzlers", "Pokemon"])),
        names(&["Racing", "Empty", "Puzzlers", "Pokemon"])
    );
    assert_eq!(
        reorder_with_hidden(&order, &names(&["Pokemon", "Racing", "Puzzlers"])),
        names(&["Pokemon", "Racing", "Empty", "Puzzlers"])
    );
}

#[test]
fn a_hidden_collection_at_the_front_stays_at_the_front() {
    let order = names(&["Empty", "Racing", "Pokemon"]);
    assert_eq!(
        reorder_with_hidden(&order, &names(&["Pokemon", "Racing"])),
        names(&["Empty", "Pokemon", "Racing"])
    );
}

#[test]
fn the_remove_stack_is_the_carts_around_the_gap() {
    let shelf = [gbc("Tetris"), gbc("Crystal"), gbc("Picross"), gbc("Metal Gear")];
    assert_eq!(
        remove_stack(&shelf, 1),
        [gbc("Picross"), gbc("Tetris"), gbc("Metal Gear")]
    );
    assert_eq!(
        remove_stack(&[gbc("Tetris"), gbc("Crystal")], 1),
        [gbc("Tetris")]
    );
    assert_eq!(
        remove_stack(&[gbc("Tetris"), gbc("Crystal"), gbc("Picross")], 2),
        [gbc("Tetris"), gbc("Crystal")]
    );
    assert!(remove_stack(&[gbc("Crystal")], 0).is_empty());
}

#[test]
fn a_stack_is_the_first_three_games_on_the_card() {
    let pokemon = holding("Pokemon", &["Gone", "Tetris", "Crystal", "Picross", "Metal Gear"]);
    let shown = [gbc("Metal Gear"), gbc("Picross"), gbc("Crystal"), gbc("Tetris")];
    assert_eq!(
        stack_games(&pokemon, &shown),
        [gbc("Tetris"), gbc("Crystal"), gbc("Picross")]
    );
}

#[test]
fn selecting_toggles_ticks() {
    let mut edit = Edit::selecting();
    edit.toggle(gbc("Crystal"));
    assert!(edit.is_checked(Platform::Gbc, "Crystal"));
    edit.toggle(gbc("Crystal"));
    assert!(!edit.is_checked(Platform::Gbc, "Crystal"));
}

mod motion {
    use slot::editing::{
        camera, fan_fade, hold_jiggle, move_still, neighbor_dim, pass, soft_jiggle, travel, Hold,
        SPOT_MS,
    };

    #[test]
    fn motion_hold_jiggle_waits_150_ms_then_reaches_2_2() {
        assert_eq!(hold_jiggle(0), 0.0);
        assert_eq!(hold_jiggle(149), 0.0);
        let peak = (270..600).map(|t| hold_jiggle(t).abs()).fold(0.0, f32::max);
        assert!(peak > 2.1 && peak <= 2.2, "{peak}");
        assert!((0..2000).all(|t| hold_jiggle(t).abs() <= 2.2));
    }

    #[test]
    fn motion_soft_jiggle_stays_within_1_6() {
        let peak = (0..2000).map(|t| soft_jiggle(t, 0.0).abs()).fold(0.0, f32::max);
        assert!(peak > 1.59 && peak <= 1.6, "{peak}");
        assert_ne!(soft_jiggle(100, 0.0), soft_jiggle(100, 0.45));
    }

    #[test]
    fn motion_the_passed_neighbour_crosses_under_the_held_one() {
        assert_eq!(pass(0.0), (0.0, 0.0, 1.0));
        let (shift, dip, scale) = pass(0.5);
        assert!((shift - 0.5).abs() < 1e-6);
        assert!(dip > 10.0);
        assert!(scale < 0.95);
        let (shift, dip, scale) = pass(1.0);
        assert!((shift - 1.0).abs() < 1e-6 && dip.abs() < 1e-3 && (scale - 1.0).abs() < 1e-3);
    }

    #[test]
    fn motion_the_held_one_is_still_while_it_moves() {
        assert_eq!(move_still(None, 500), 1.0);
        let held = Hold { down: 0, up: None, dir: 1 };
        assert_eq!(move_still(Some(&held), 500), 0.0);
        let let_go = Hold { down: 0, up: Some(300), dir: 1 };
        assert_eq!(move_still(Some(&let_go), 400), 0.0);
        assert_eq!(move_still(Some(&let_go), 3000), 1.0);
    }

    #[test]
    fn motion_the_card_left_out_of_the_fan_is_gone_a_quarter_of_the_way_in() {
        assert_eq!(fan_fade(0.0), 1.0);
        assert!(fan_fade(0.1) < 0.7);
        assert_eq!(fan_fade(0.25), 0.0);
        assert_eq!(fan_fade(1.0), 0.0);
    }

    #[test]
    fn motion_neighbors_dim_to_75_percent() {
        assert_eq!(neighbor_dim(100), 1.0);
        assert!((neighbor_dim(400) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn motion_a_tap_settles_on_exactly_one_spot() {
        let tap = Hold { down: 0, up: Some(140), dir: 1 };
        assert!(travel(&tap, 100, 10) < 1.0);
        assert_eq!(travel(&tap, 2000, 10), 1.0);
    }

    #[test]
    fn motion_a_hold_never_hitches() {
        let held = Hold { down: 0, up: None, dir: 1 };
        for t in (0..1000).step_by(10) {
            assert!(travel(&held, t + 50, 99) > travel(&held, t, 99), "stalled at {t}");
        }
        let a = travel(&held, 600, 99);
        let b = travel(&held, 600 + SPOT_MS, 99);
        assert!((b - a - 1.0).abs() < 1e-4, "{a} {b}");
    }

    #[test]
    fn motion_travel_is_capped_by_room() {
        let held = Hold { down: 0, up: None, dir: 1 };
        assert_eq!(travel(&held, 5000, 2), 2.0);
    }

    #[test]
    fn motion_the_camera_keeps_the_held_item_one_spot_out_then_catches_up() {
        assert_eq!(camera(0.6, false, 0), 0.0);
        assert!((camera(3.4, false, 0) - 2.4).abs() < 1e-6);
        assert!((camera(-3.0, false, 0) + 2.0).abs() < 1e-6);
        assert_eq!(camera(3.0, true, 2000), 3.0);
    }
}

#[test]
fn with_two_collections_new_collection_sits_in_the_middle() {
    let all = [holding("Pokemon", &[]), holding("Puzzlers", &[])];
    assert_eq!(
        choices(&all, &[gbc("Crystal")]),
        [Choice::Collection(0), Choice::New, Choice::Collection(1)]
    );
}
