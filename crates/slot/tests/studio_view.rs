use slot::studio::{Row, Screen, Studio, Target};
use slot::studio_view::{layout, view, Align, Item, Style};
use slot_input::Btn;
use slot_store::{Cart, Platform};

fn cart(platform: Platform) -> Cart {
    Cart {
        platform,
        stem: "Probe".into(),
        rom: "Games/GBA/Probe.gba".into(),
        label: Some("Labels/GBA/Probe.png".into()),
        title: "PROBE".into(),
        code: String::new(),
        shell: None,
    }
}

fn gba() -> Studio {
    Studio::new(cart(Platform::Gba), true)
}

fn texts(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn to_row(s: &mut Studio, row: Row) {
    while s.row() != Some(row) {
        s.press(Btn::Down);
    }
}

fn right_edge(items: &[Item]) -> f32 {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Text { x, align: Align::Right, .. } => Some(*x),
            Item::Chip { x, .. } => Some(*x + 48.0),
            Item::Rect { x, w, .. } if *w > 4.0 => Some(*x + *w),
            _ => None,
        })
        .fold(0.0, f32::max)
}

#[test]
fn the_list_names_its_rows_and_values() {
    let t = texts(&view(&gba(), 720.0).items);
    assert!(t.contains(&"Label Printer".to_string()));
    assert!(t.contains(&"Shell Color".to_string()));
    assert!(t.contains(&"Solid".to_string()));
}

#[test]
fn margins_match_on_both_screens() {
    for w in [720.0f32, 640.0] {
        let l = layout(w, Platform::Gba);
        let mut s = gba();
        to_row(&mut s, Row::Color);
        s.press(Btn::A);
        let edge = right_edge(&view(&s, w).items);
        assert!((w - edge - 26.0).abs() <= 1.0, "{w}: {edge}");
        assert!(l.cart_x + l.cart_w < w - 300.0 - 8.0, "{w}");
    }
}

#[test]
fn the_cart_is_centered_above_the_footer() {
    let v = view(&gba(), 720.0);
    let l = layout(720.0, Platform::Gba);
    assert!(((2.0 * l.cart_y + l.cart_h) - 422.0).abs() <= 1.0, "{} {}", l.cart_y, l.cart_h);
    assert!(v.items.iter().any(|i| matches!(i, Item::Cart)));
}

#[test]
fn chips_have_a_heading_and_the_name_under_the_cart() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    let t = texts(&view(&s, 720.0).items);
    assert!(t.contains(&"Shell Color".to_string()));
    assert!(t.contains(&"Automatic".to_string()), "{t:?}");
}

#[test]
fn custom_uses_the_closed_bar() {
    let mut s = gba();
    assert!(!view(&s, 720.0).closed_bar);
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(2);
    s.press(Btn::A);
    let v = view(&s, 720.0);
    assert!(v.closed_bar);
    assert!(texts(&v.items).contains(&"Custom Shell".to_string()));
}

#[test]
fn hex_has_six_digits_and_a_hash_with_no_arrows() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    s.chips_jump(2);
    s.press(Btn::A);
    s.press(Btn::Y);
    assert!(matches!(s.screen, Screen::Hex { .. }));
    let v = view(&s, 720.0);
    let t = texts(&v.items);
    assert!(t.contains(&"Shell Hex".to_string()));
    assert!(t.contains(&"#".to_string()));
    let digits = v.items.iter().filter(|i| matches!(i, Item::Text { style: Style::Digit | Style::DigitOn, .. })).count();
    assert_eq!(digits, 6);
}

#[test]
fn an_empty_sheet_says_why() {
    let mut s = Studio::new(Cart { label: None, ..cart(Platform::Gba) }, true);
    s.press(Btn::A);
    s.art_arrived(false, false);
    assert!(texts(&view(&s, 720.0).items).contains(&"No Art Found".to_string()));
    s.art_failed = true;
    assert!(texts(&view(&s, 720.0).items).contains(&"No Connection".to_string()));
}

#[test]
fn chips_scroll_to_keep_the_highlight_on_screen() {
    let mut s = gba();
    to_row(&mut s, Row::Color);
    s.press(Btn::A);
    let last = slot_ui::palette::chip_grid(0.0, 0.0).len() - 1;
    for at in [0, last] {
        s.chips_jump(at);
        let items = view(&s, 720.0).items;
        let chips: Vec<_> = items.iter().filter_map(|i| match i {
            Item::Chip { y, selected, .. } => Some((*y, *selected)),
            _ => None,
        }).collect();
        assert!(chips.iter().any(|c| c.1), "selected chip {at} is off screen");
        assert!(chips.iter().all(|c| c.0 >= 60.0 && c.0 + 48.0 <= 422.0), "{chips:?}");
    }
}

#[test]
fn the_label_row_never_reads_existing() {
    let t = texts(&view(&gba(), 720.0).items);
    assert!(!t.contains(&"Existing".to_string()), "{t:?}");
}

#[test]
fn the_printer_sits_on_the_cart_margin() {
    for w in [720.0, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let c = cart(platform);
            let t = slot::studio_view::tray(w, &c);
            let right = w - (t.printer_x + t.printer_w);
            assert_eq!(right, layout(w, platform).margin, "{w} {platform:?}");
            assert!(t.x > t.printer_x && t.x + t.slip_w < t.printer_x + t.printer_w);
            let l = layout(w, platform);
            assert!(t.printer_x > l.cart_x + l.cart_w, "{w} {platform:?}");
        }
    }
}

#[test]
fn the_tilted_cart_art_sits_on_the_margin() {
    use slot::studio_view::{turned, CART_TILT};
    for platform in [Platform::Gba, Platform::Gb, Platform::Gbc] {
        for w in [720.0, 640.0] {
            let l = layout(w, platform);
            let face = slot_ui::bare_face(&cart(platform));
            let (fw, fh) = (face.w as usize, face.h as usize);
            let (sx, sy) = (l.cart_w / fw as f32, l.cart_h / fh as f32);
            let mut seen = f32::MAX;
            for y in (0..fh).step_by(2) {
                for x in (0..fw).step_by(2) {
                    if face.rgba[(y * fw + x) * 4 + 3] > 128 {
                        let spot = [l.cart_x + x as f32 * sx, l.cart_y + y as f32 * sy, 1.0, 1.0];
                        let centre = [l.cart_x + l.cart_w / 2.0, l.cart_y + l.cart_h / 2.0];
                        seen = seen.min(turned(spot, centre, CART_TILT)[0][0]);
                    }
                }
            }
            assert!((seen - l.margin).abs() <= 5.0, "{platform:?} {w}: {seen} vs {}", l.margin);
        }
    }
}

#[test]
fn turning_keeps_the_pivot_and_the_size() {
    use slot::studio_view::turned;
    let c = turned([0.0, 0.0, 100.0, 50.0], [50.0, 25.0], 90.0);
    assert!((c[0][0] - 75.0).abs() < 0.01 && (c[0][1] + 25.0).abs() < 0.01, "{c:?}");
    assert!((c[2][0] - 25.0).abs() < 0.01 && (c[2][1] - 75.0).abs() < 0.01, "{c:?}");
}

#[test]
fn an_empty_sheet_of_duplicates_says_so() {
    let mut s = gba();
    to_row(&mut s, Row::Label);
    s.press(Btn::A);
    s.adopt_text([7, 8, 9]);
    s.duplicates = true;
    s.art_arrived(false, false);
    assert!(texts(&view(&s, 720.0).items).contains(&"No New Labels".to_string()));
}

#[test]
fn a_worn_label_with_nothing_new_shows_the_text_sticker() {
    let mut s = gba();
    to_row(&mut s, Row::Label);
    s.press(Btn::A);
    s.duplicates = true;
    s.art_arrived(false, false);
    let items = view(&s, 720.0).items;
    assert!(!texts(&items).contains(&"No New Labels".to_string()));
    assert!(items.iter().any(|i| matches!(i, Item::Sheet)));
}

#[test]
fn gb_stickers_pile_at_true_size_above_the_footer() {
    let c = cart(Platform::Gb);
    let t = slot::studio_view::tray(720.0, &c);
    let well = slot::studio_view::well_of(&c);
    assert_eq!(t.sticker_h, well[3] as f32);
    assert!(t.bottom < t.slip_h, "not overlapped");
    assert!(slot::studio_view::TRAY_TOP + t.bottom + t.slip_h <= 414.0);
}

#[test]
fn the_hex_row_clears_the_tilted_cart() {
    for w in [720.0f32, 640.0] {
        let l = layout(w, Platform::Gba);
        let mut s = gba();
        to_row(&mut s, Row::Color);
        s.press(Btn::A);
        s.screen = Screen::Hex { target: Target::Shell, digits: [0; 6], pos: 0, start: [0; 3] };
        let rects: Vec<f32> = view(&s, w).items.into_iter().filter_map(|i| match i {
            Item::Rect { x, .. } => Some(x),
            _ => None,
        }).collect();
        let left = rects[0] - (rects[1] - rects[0]);
        assert!(left >= l.edge, "{w}: {left} vs {}", l.edge);
    }
}

#[test]
fn gb_spacing_is_even_around_the_stickers() {
    for w in [720.0, 640.0] {
        for platform in [Platform::Gb, Platform::Gbc] {
            let l = layout(w, platform);
            let t = slot::studio_view::tray(w, &cart(platform));
            let gap = t.printer_x - l.edge;
            assert!((gap - l.margin).abs() <= 3.0, "{w} {platform:?}: {gap} vs {}", l.margin);
            assert!(l.margin > 20.0, "{w} {platform:?}");
        }
    }
}

#[test]
fn open_rows_are_names_and_inline_rows_carry_a_value_and_dots() {
    let s = Studio::new(cart(Platform::Gb), true);
    let items = view(&s, 720.0).items;
    let t = texts(&items);
    for name in ["Label Printer", "Shell Color", "Finish", "Shape", "Solid", "Automatic"] {
        assert!(t.contains(&name.to_string()), "{name}: {t:?}");
    }
    assert_eq!(t.len(), 6, "{t:?}");
    let dots = items.iter().filter(|i| matches!(i, Item::Dot { .. })).count();
    assert_eq!(dots, 6);
}

#[test]
fn the_list_centers_beside_the_cart() {
    let s = Studio::new(cart(Platform::Gb), true);
    let l = layout(720.0, Platform::Gb);
    let mid = (l.edge + 720.0) / 2.0;
    let items = view(&s, 720.0).items;
    let (mut top, mut low) = (f32::MAX, f32::MIN);
    for i in &items {
        match i {
            Item::Text { x, y, align, .. } => {
                assert_eq!(*align, Align::Center);
                assert_eq!(*x, mid);
                top = top.min(*y);
            }
            Item::Dot { y, size, .. } => low = low.max(y + size),
            _ => {}
        }
    }
    assert!(((top + low) / 2.0 - 211.0).abs() <= 10.0, "{top} {low}");
    let dots: Vec<f32> = items.iter().filter_map(|i| match i {
        Item::Dot { x, size, .. } => Some(x + size / 2.0),
        _ => None,
    }).collect();
    assert!((dots[1] - mid).abs() < 0.5, "{dots:?}");
}

#[test]
fn the_picked_row_is_marked_by_bright_text_alone() {
    let mut s = Studio::new(cart(Platform::Gb), true);
    s.press(Btn::Down);
    let items = view(&s, 720.0).items;
    assert!(!items.iter().any(|i| matches!(i, Item::Rect { .. })));
    let lit: Vec<&str> = items.iter().filter_map(|i| match i {
        Item::Text { text, alpha, .. } if *alpha == 1.0 => Some(text.as_str()),
        _ => None,
    }).collect();
    assert_eq!(lit.len(), 1, "{lit:?}");
}

#[test]
fn the_longest_name_fits_the_column() {
    for w in [720.0, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let s = Studio::new(cart(platform), true);
            let items = view(&s, w).items;
            let band = w - layout(w, platform).edge - 16.0;
            let scale = items.iter().find_map(|i| match i {
                Item::Text { text, scale, .. } if text == "Label Printer" => Some(*scale),
                _ => None,
            }).unwrap();
            let face = slot_ui::heading_face("Label Printer");
            assert!(face.w as f32 * scale <= band - 8.0, "{w} {platform:?}: {} in {band}", face.w as f32 * scale);
        }
    }
}

#[test]
fn the_color_screen_has_a_scrollbar_that_tracks_the_chip() {
    let bar = |at: usize| {
        let mut s = gba();
        to_row(&mut s, Row::Color);
        s.press(Btn::A);
        s.screen = Screen::Chips { target: Target::Shell, at };
        let rects: Vec<(f32, f32, f32)> = view(&s, 720.0).items.into_iter().filter_map(|i| match i {
            Item::Rect { x, y, h, .. } if x >= 700.0 => Some((x, y, h)),
            _ => None,
        }).collect();
        assert_eq!(rects.len(), 2, "track and thumb");
        assert_eq!(rects[0].0, 708.0);
        let chips = view(&s, 720.0).items.iter().filter_map(|i| match i {
            Item::Chip { x, .. } => Some(*x + 48.0),
            _ => None,
        }).fold(0.0, f32::max);
        assert!(rects[0].0 - chips >= 12.0, "{chips}");
        (rects[0], rects[1])
    };
    let (track, top) = bar(0);
    assert_eq!(top.1, track.1);
    assert!(top.2 < track.2);
    let last = slot_ui::palette::chip_grid(0.0, 0.0).len() - 1;
    let (track, end) = bar(last);
    assert!((end.1 + end.2 - (track.1 + track.2)).abs() < 0.5, "{end:?} {track:?}");
}

#[test]
fn the_chips_clear_the_tilted_cart() {
    for w in [720.0f32, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let l = layout(w, platform);
            let mut s = Studio::new(cart(platform), true);
            s.screen = Screen::Chips { target: Target::Shell, at: 0 };
            let left = view(&s, w).items.iter().filter_map(|i| match i {
                Item::Chip { x, .. } => Some(*x),
                _ => None,
            }).fold(f32::MAX, f32::min);
            assert!(left - l.edge >= 19.0, "{w} {platform:?}: {left} vs {}", l.edge);
        }
    }
}

#[test]
fn gba_stickers_get_room_between_them() {
    for w in [720.0, 640.0] {
        let t = slot::studio_view::tray(w, &cart(Platform::Gba));
        assert!(t.bottom - t.slip_h >= 24.0, "{w}: {} {}", t.bottom, t.slip_h);
        assert!(slot::studio_view::TRAY_TOP + t.bottom + t.slip_h <= 414.0);
    }
}

#[test]
fn custom_stays_between_the_cart_and_the_right_margin() {
    for w in [720.0f32, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let l = layout(w, platform);
            let mut s = Studio::new(cart(platform), true);
            s.screen = Screen::Custom { target: Target::Shell, rgb: [255, 0, 128], channel: 0, start: [255, 0, 128] };
            let items = view(&s, w).items;
            let mut low = f32::MAX;
            let mut high = f32::MIN;
            for i in &items {
                match i {
                    Item::Rect { x, w: width, .. } => {
                        low = low.min(*x);
                        high = high.max(*x + *width);
                    }
                    Item::Text { align: Align::Left, x, .. } => low = low.min(*x),
                    Item::Text { align: Align::Right, x, .. } => high = high.max(*x),
                    _ => {}
                }
            }
            assert!(low - l.edge >= 19.0, "{w} {platform:?}: {low} vs {}", l.edge);
            assert!((high - (w - l.margin)).abs() < 0.01, "{w} {platform:?}: {high}");
        }
    }
}

#[test]
fn hex_ends_on_the_right_margin() {
    for w in [720.0f32, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let l = layout(w, platform);
            let mut s = Studio::new(cart(platform), true);
            s.screen = Screen::Hex { target: Target::Shell, digits: [0; 6], pos: 0, start: [0; 3] };
            let high = view(&s, w).items.iter().filter_map(|i| match i {
                Item::Rect { x, w, .. } => Some(*x + *w),
                _ => None,
            }).fold(f32::MIN, f32::max);
            assert!((high - (w - l.margin)).abs() < 0.01, "{w} {platform:?}: {high}");
        }
    }
}

#[test]
fn the_current_label_chip_names_the_color_it_opened_with() {
    let mut s = gba();
    let (name, red) = slot_ui::shell_presets().into_iter().next().map(|(n, p)| (n, p.colour)).unwrap();
    s.adopt_logo(red);
    s.screen = Screen::Chips { target: Target::Label, at: 0 };
    assert!(texts(&view(&s, 720.0).items).contains(&name.to_string()));
}

#[test]
fn the_color_name_clears_the_footer_on_every_layout() {
    for w in [720.0f32, 640.0] {
        for platform in [Platform::Gba, Platform::Gb] {
            let mut s = Studio::new(cart(platform), true);
            s.screen = Screen::Chips { target: Target::Shell, at: 1 };
            let items = view(&s, w).items;
            let (y, scale) = items.iter().find_map(|i| match i {
                Item::Text { text, y, scale, .. } if text == "Automatic" => Some((*y, *scale)),
                _ => None,
            }).unwrap();
            assert!(y + 24.0 * scale <= 422.0, "{w} {platform:?}: {y} {scale}");
        }
    }
}
