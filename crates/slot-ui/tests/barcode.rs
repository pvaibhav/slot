use slot_ui::code39;

#[test]
fn a_hash_encodes_to_nine_elements_per_character_between_sentinels() {
    let run = code39("*9E11A10*").expect("hex and sentinels are in the alphabet");
    assert_eq!(run.len(), 9 * 9, "nine elements per character");
}

#[test]
fn every_character_is_three_wide_elements_of_nine() {
    let run = code39("*0123456789ABCDEF*").unwrap();
    for (n, c) in run.chunks_exact(9).enumerate() {
        assert_eq!(c.iter().filter(|w| **w).count(), 3, "character {n}");
    }
}

#[test]
fn the_wide_elements_fall_where_the_symbology_says() {
    let run = code39("*0F*").unwrap();
    let bars = |c: &[bool]| (0..9).step_by(2).filter(|i| c[*i]).count();
    let spaces = |c: &[bool]| (1..9).step_by(2).filter(|i| c[*i]).count();
    for (n, c) in run.chunks_exact(9).enumerate() {
        assert_eq!((bars(c), spaces(c)), (2, 1), "character {n}");
    }
}

#[test]
fn the_alphabet_covers_a_bare_url() {
    assert!(code39("*GITHUB.COM/SLOT-CFW/SLOT*").is_some());
    assert!(code39("*SLOT-CFW.FYI*").is_some());
    assert!(
        code39("*A B-C.D$E/F+G%H*").is_some(),
        "the symbols are all in"
    );
}

#[test]
fn lower_case_and_a_colon_are_refused_rather_than_dropped() {
    assert!(code39("*https*").is_none(), "there is no lower case");
    assert!(code39("*HTTPS://X*").is_none(), "there is no colon");
}
