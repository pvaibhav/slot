use slot_store::{
    decode_guide, find_guide, read_guide, read_guide_spot, write_guide_spot, GuideSpot, Platform,
};
use tempfile::tempdir;

#[test]
fn a_guide_is_found_by_the_game_name_in_any_case() {
    let d = tempdir().unwrap();
    let dir = d.path().join("Guides/GBA");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Golden Sun.TXT"), "walkthrough").unwrap();
    assert_eq!(
        find_guide(d.path(), Platform::Gba, "golden sun"),
        Some(dir.join("Golden Sun.TXT"))
    );
    assert_eq!(find_guide(d.path(), Platform::Gb, "Golden Sun"), None);
    assert_eq!(find_guide(d.path(), Platform::Gba, "Golden"), None);
}

#[test]
fn a_hidden_guide_is_not_offered() {
    let d = tempdir().unwrap();
    let dir = d.path().join("Guides/GB");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".Tetris.txt"), "x").unwrap();
    assert_eq!(find_guide(d.path(), Platform::Gb, ".Tetris"), None);
}

#[test]
fn tabs_expand_to_eight_columns_and_line_ends_are_dropped() {
    let lines = decode_guide(b"\xEF\xBB\xBFab\tc\r\n\tx  \r\n\x0c\r\nend");
    assert_eq!(lines, ["ab      c", "        x", "", "end"]);
}

#[test]
fn a_latin1_guide_reads_as_latin1() {
    assert_eq!(decode_guide(b"Pok\xe9mon\n"), ["Pok\u{e9}mon"]);
    assert_eq!(decode_guide("Pok\u{e9}mon".as_bytes()), ["Pok\u{e9}mon"]);
}

#[test]
fn a_box_drawn_table_keeps_its_characters() {
    let lines = decode_guide("\u{250c}\u{2500}\u{2510}\n\u{2502}A\u{2502}".as_bytes());
    assert_eq!(lines[1].chars().count(), 3);
}

#[test]
fn read_guide_reads_the_file() {
    let d = tempdir().unwrap();
    let path = d.path().join("g.txt");
    std::fs::write(&path, "one\ntwo\n").unwrap();
    assert_eq!(read_guide(&path).unwrap(), ["one", "two"]);
}

#[test]
fn each_guide_keeps_its_own_spot() {
    let d = tempdir().unwrap();
    let a = d.path().join("Guides/GBA/Golden Sun.txt");
    let b = d.path().join("Guides/GB/Golden Sun.txt");
    assert_eq!(read_guide_spot(d.path(), &a), None);
    let spot = GuideSpot {
        line: 1200,
        col: 4,
        zoom: 2,
        wrap: false,
    };
    write_guide_spot(d.path(), &a, spot).unwrap();
    write_guide_spot(d.path(), &b, GuideSpot::default()).unwrap();
    assert_eq!(read_guide_spot(d.path(), &a), Some(spot));
    assert_eq!(read_guide_spot(d.path(), &b), Some(GuideSpot::default()));
    let moved = GuideSpot { line: 7, ..spot };
    write_guide_spot(d.path(), &a, moved).unwrap();
    assert_eq!(read_guide_spot(d.path(), &a), Some(moved));
    let text = std::fs::read_to_string(d.path().join("Config/guides.txt")).unwrap();
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn a_garbled_spot_starts_at_the_top() {
    let d = tempdir().unwrap();
    std::fs::create_dir_all(d.path().join("Config")).unwrap();
    std::fs::write(
        d.path().join("Config/guides.txt"),
        "GBA/x.txt = 12 nope 1\n",
    )
    .unwrap();
    assert_eq!(
        read_guide_spot(d.path(), &d.path().join("Guides/GBA/x.txt")),
        None
    );
}

#[test]
fn a_spot_keeps_wrap_and_an_older_line_reads_as_unwrapped() {
    let d = tempdir().unwrap();
    let guide = d.path().join("Guides/GBA/Golden Sun.txt");
    let spot = GuideSpot {
        line: 9,
        col: 240,
        zoom: 5,
        wrap: true,
    };
    write_guide_spot(d.path(), &guide, spot).unwrap();
    assert_eq!(read_guide_spot(d.path(), &guide), Some(spot));
    std::fs::write(
        d.path().join("Config/guides.txt"),
        "GBA/Golden Sun.txt=9 0 5\n",
    )
    .unwrap();
    assert_eq!(
        read_guide_spot(d.path(), &guide).map(|s| s.wrap),
        Some(false)
    );
}
