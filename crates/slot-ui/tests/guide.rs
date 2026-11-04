use slot_store::GuideSpot;
use slot_ui::{
    cell, fitting_zoom, page_box, Canvas, Guide, GuideGlyphs, GUIDE_COLUMNS, GUIDE_ZOOMS,
};

const WIDE: Canvas = Canvas { w: 720.0, h: 480.0 };
const SQUARE: Canvas = Canvas { w: 640.0, h: 480.0 };

fn lines(n: usize, width: usize) -> Vec<String> {
    (0..n)
        .map(|i| format!("{i:<width$}").replace(' ', "x"))
        .collect()
}

#[test]
fn a_new_guide_shows_eighty_columns_at_the_largest_size_that_fits() {
    for canvas in [WIDE, SQUARE] {
        let z = fitting_zoom(canvas) as usize;
        let g = Guide::new(lines(10, 80), None, canvas);
        assert!(g.cols(canvas) >= GUIDE_COLUMNS, "{canvas:?}");
        if let Some(&bigger) = GUIDE_ZOOMS.get(z + 1) {
            let (_, _, w, _) = page_box(canvas);
            assert!(cell(bigger).0 * GUIDE_COLUMNS as f32 > w as f32);
        }
    }
    assert!(fitting_zoom(WIDE) > fitting_zoom(SQUARE));
}

#[test]
fn scrolling_stops_at_the_last_full_page() {
    let mut g = Guide::new(lines(100, 10), None, WIDE);
    let rows = g.rows(WIDE);
    g.scroll(-5, WIDE);
    assert_eq!(g.spot().line, 0);
    g.scroll(1000, WIDE);
    assert_eq!(g.spot().line as usize, 100 - rows);
    g.page(-1, WIDE);
    assert_eq!(g.spot().line as usize, 100 - rows - (rows - 1));
}

#[test]
fn a_short_guide_does_not_scroll_or_pan() {
    let mut g = Guide::new(lines(3, 10), None, WIDE);
    g.scroll(1, WIDE);
    g.pan(1, WIDE);
    assert_eq!((g.spot().line, g.spot().col), (0, 0));
}

#[test]
fn panning_reaches_the_end_of_the_widest_line() {
    let mut g = Guide::new(lines(3, 200), None, SQUARE);
    let cols = g.cols(SQUARE);
    g.pan(100, SQUARE);
    assert_eq!(g.spot().col as usize, 200 - cols);
}

#[test]
fn zoom_stops_at_both_ends_and_keeps_the_view_in_range() {
    let mut g = Guide::new(lines(100, 90), None, WIDE);
    while g.zoom_by(-1, WIDE) {}
    assert_eq!(g.spot().zoom, 0);
    g.scroll(1000, WIDE);
    let top = g.spot().line;
    while g.zoom_by(1, WIDE) {}
    assert_eq!(g.spot().zoom as usize, GUIDE_ZOOMS.len() - 1);
    assert_eq!(g.spot().line, top, "zooming in moved the top line");
    g.scroll(1000, WIDE);
    while g.zoom_by(-1, WIDE) {}
    assert_eq!(g.spot().line as usize, 100 - g.rows(WIDE));
}

#[test]
fn a_saved_spot_out_of_range_is_pulled_back() {
    let spot = GuideSpot {
        line: 9999,
        col: 9999,
        zoom: 200,
        wrap: false,
    };
    let g = Guide::new(lines(50, 100), Some(spot), WIDE);
    assert_eq!(g.spot().zoom as usize, GUIDE_ZOOMS.len() - 1);
    assert_eq!(g.spot().line as usize, 50 - g.rows(WIDE));
    assert_eq!(g.spot().col as usize, 100 - g.cols(WIDE));
}

fn inked_rows(face: &slot_ui::UndoFace) -> Vec<bool> {
    (0..face.h as usize)
        .map(|y| {
            let row = &face.rgba[y * face.w as usize * 4..(y + 1) * face.w as usize * 4];
            row.chunks(4).any(|p| p[0] > 128)
        })
        .collect()
}

#[test]
fn the_page_inks_the_text_and_nothing_below_it() {
    let canvas = WIDE;
    let mut text = vec!["#".repeat(80); 2];
    text.push(String::new());
    text.push("#".repeat(80));
    let g = Guide::new(text, None, canvas);
    let face = g.render(canvas, &mut GuideGlyphs::default());
    let (_, _, w, h) = page_box(canvas);
    assert_eq!((face.w, face.h), (w, h));
    assert!(face.rgba.chunks(4).all(|p| p[3] == 255));
    let line_h = cell(g.px()).1 as usize;
    let rows = inked_rows(&face);
    let band = |r: usize| rows[r * line_h..(r + 1) * line_h].iter().any(|&b| b);
    assert!(band(0) && band(1) && band(3));
    assert!(!band(2), "the blank line has ink");
    assert!(!rows[4 * line_h..].iter().any(|&b| b), "ink below the text");
}

#[test]
fn the_page_follows_the_scroll() {
    let canvas = WIDE;
    let mut text = vec![String::new(); 60];
    text[1] = "#".repeat(10);
    let mut g = Guide::new(text, None, canvas);
    let mut glyphs = GuideGlyphs::default();
    let line_h = cell(g.px()).1 as usize;
    let before = inked_rows(&g.render(canvas, &mut glyphs));
    assert!(before[line_h..2 * line_h].iter().any(|&b| b));
    g.scroll(1, canvas);
    let after = inked_rows(&g.render(canvas, &mut glyphs));
    assert!(after[..line_h].iter().any(|&b| b));
    assert!(!after[line_h..].iter().any(|&b| b));
}

#[test]
fn the_legend_is_back_and_help_only() {
    let g = Guide::new(lines(3, 10), None, WIDE);
    let keys: Vec<(&str, String)> = g.hints().into_iter().map(|h| (h.key, h.label)).collect();
    assert_eq!(
        keys,
        [("Y", "Close".to_string()), ("MENU", "Help".to_string())]
    );
}

fn prose(words: usize) -> String {
    vec!["word"; words].join(" ")
}

#[test]
fn wrapping_fits_every_word_on_screen_and_stops_panning() {
    let mut g = Guide::new(vec![prose(200)], None, WIDE);
    g.zoom_by(7, WIDE);
    assert_eq!(g.view(WIDE).col, 0);
    g.toggle_wrap(WIDE);
    g.pan(1, WIDE);
    assert_eq!(g.view(WIDE).col, 0);
    let cols = g.cols(WIDE);
    let mut seen = 0;
    let mut rows = 0;
    while let Some(text) = g.visible_text(rows) {
        assert!(text.chars().count() <= cols, "{text:?}");
        assert!(!text.starts_with(' ') && !text.ends_with(' '), "{text:?}");
        seen += text.split_whitespace().count();
        assert!(text.split_whitespace().all(|w| w == "word"), "{text:?}");
        rows += 1;
    }
    assert_eq!(seen, 200);
}

#[test]
fn a_word_wider_than_the_screen_is_cut_and_blank_lines_stay() {
    let long = "x".repeat(300);
    let mut g = Guide::new(vec![long, String::new(), "end".into()], None, WIDE);
    g.toggle_wrap(WIDE);
    let cols = g.cols(WIDE);
    assert_eq!(g.visible_text(0).unwrap().len(), cols);
    let rows = 300usize.div_ceil(cols);
    assert_eq!(g.visible_text(rows), Some(""));
    assert_eq!(g.visible_text(rows + 1), Some("end"));
}

#[test]
fn indentation_on_a_first_row_is_kept() {
    let line = format!("    {}", prose(100));
    let mut g = Guide::new(vec![line], None, WIDE);
    g.toggle_wrap(WIDE);
    assert!(g.visible_text(0).unwrap().starts_with("    word"));
}

#[test]
fn toggling_and_zooming_keep_the_same_text_on_top() {
    let lines: Vec<String> = (0..100).map(|i| format!("{i} {}", prose(60))).collect();
    let mut g = Guide::new(lines, None, WIDE);
    g.scroll(40, WIDE);
    g.toggle_wrap(WIDE);
    assert!(g.visible_text(0).unwrap().starts_with("40 "));
    g.scroll(1, WIDE);
    let top = g.spot();
    assert_eq!(top.line, 40);
    assert!(top.col > 0);
    g.zoom_by(1, WIDE);
    let moved = g.spot();
    assert_eq!(moved.line, 40);
    assert!(moved.col <= top.col);
    g.toggle_wrap(WIDE);
    assert_eq!(g.spot().line, 40);
    assert!(!g.wrap());
}

#[test]
fn a_saved_wrapped_spot_reopens_on_the_same_row() {
    let lines: Vec<String> = (0..100).map(|i| format!("{i} {}", prose(60))).collect();
    let mut g = Guide::new(lines.clone(), None, WIDE);
    g.toggle_wrap(WIDE);
    g.scroll(200, WIDE);
    let spot = g.spot();
    let top = g.visible_text(0).unwrap().to_string();
    let back = Guide::new(lines, Some(spot), WIDE);
    assert!(back.wrap());
    assert_eq!(back.visible_text(0), Some(top.as_str()));
    assert_eq!(back.spot(), spot);
}

#[test]
fn a_spot_saved_before_wrapping_existed_stays_unwrapped() {
    let spot = GuideSpot {
        line: 3,
        col: 0,
        zoom: 2,
        wrap: false,
    };
    let g = Guide::new(lines(20, 200), Some(spot), WIDE);
    assert!(!g.wrap());
}

#[test]
fn wrapped_text_is_drawn_and_changes_the_view() {
    let mut g = Guide::new(vec![prose(400)], None, WIDE);
    let before = g.view(WIDE);
    let plain = inked_rows(&g.render(WIDE, &mut GuideGlyphs::default()));
    g.toggle_wrap(WIDE);
    assert_ne!(g.view(WIDE), before);
    let wrapped = inked_rows(&g.render(WIDE, &mut GuideGlyphs::default()));
    assert!(
        wrapped.iter().filter(|r| **r).count() > plain.iter().filter(|r| **r).count() * 4,
        "wrapped text should fill the page"
    );
}

#[test]
fn the_help_fits_between_the_plates() {
    let face = slot_ui::guide_help_face();
    assert!(
        face.h as f32 <= 480.0 - 2.0 * slot_ui::PLATE_H,
        "{}",
        face.h
    );
}

#[test]
fn wrapped_rows_keep_the_indent_of_their_line() {
    let line = format!("    {}", prose(100));
    let mut g = Guide::new(vec![line], None, WIDE);
    g.toggle_wrap(WIDE);
    let cols = g.cols(WIDE);
    assert_eq!(g.visible_indent(0), Some(0));
    assert!(g.visible_text(0).unwrap().starts_with("    word"));
    assert_eq!(g.visible_indent(1), Some(4));
    assert!(g.visible_text(1).unwrap().starts_with("word"));
    assert!(g.visible_text(1).unwrap().chars().count() + 4 <= cols);
}

#[test]
fn a_line_indented_past_half_the_screen_still_wraps_in_what_is_left() {
    let line = format!("{}{}", " ".repeat(70), prose(60));
    let mut g = Guide::new(vec![line], None, WIDE);
    g.toggle_wrap(WIDE);
    let cols = g.cols(WIDE);
    let mut row = 1;
    while let Some(text) = g.visible_text(row) {
        assert_eq!(g.visible_indent(row), Some(cols / 2));
        assert!(text.chars().count() + cols / 2 <= cols, "{text:?}");
        row += 1;
    }
}
