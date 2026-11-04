use slot_store::{parse_stamp, Zone, FF_SPEEDS};
use slot_ui::{
    date_time_text, quick_label_face, quick_value_face, ClockPicker, ClockRow, QuickRow,
    QuickValue, UndoFace, MENU_PAD,
};

fn inked(f: &UndoFace, x: u32, y: u32) -> bool {
    f.rgba[((y * f.w + x) * 4 + 3) as usize] > 128
}

fn ink_columns(f: &UndoFace) -> (u32, u32) {
    let cols: Vec<u32> = (0..f.w)
        .filter(|&x| (0..f.h).any(|y| inked(f, x, y)))
        .collect();
    (
        *cols.first().expect("an empty face"),
        *cols.last().expect("an empty face"),
    )
}

#[test]
fn the_type_sits_exactly_menu_pad_in_from_both_sides_of_its_face() {
    for f in [
        quick_label_face(QuickRow::FastForwardSound, true),
        quick_label_face(QuickRow::Rumble, true),
        quick_value_face("Off", false),
        quick_value_face(QuickValue::Speed6.text(), true),
        quick_value_face("SEP 15 16:35", true),
    ] {
        let (first, last) = ink_columns(&f);
        let right = f.w - 1 - MENU_PAD;
        assert!(
            (MENU_PAD..MENU_PAD + 4).contains(&first),
            "ink starts at {first} of {}",
            f.w
        );
        assert!(
            (right - 4..=right).contains(&last),
            "ink ends at {last} of {}",
            f.w
        );
    }
}

#[test]
fn a_long_label_is_set_as_large_as_a_short_one() {
    let tall = |f: &UndoFace| {
        (0..f.h)
            .filter(|&y| (0..f.w).any(|x| inked(f, x, y)))
            .count()
    };
    let long = tall(&quick_label_face(QuickRow::FastForwardSound, true));
    let short = tall(&quick_label_face(QuickRow::Rumble, true));
    assert!(long + 1 >= short, "{long} rows of ink against {short}");
}

#[test]
fn the_fast_forward_row_offers_off_a_slow_speed_and_the_four_ceilings() {
    assert_eq!(FF_SPEEDS, [0, 6, 8, 12, 16, 24]);
    assert_eq!(
        FF_SPEEDS.map(QuickValue::speed),
        [
            Some(QuickValue::Off),
            Some(QuickValue::Speed1_5),
            Some(QuickValue::Speed2),
            Some(QuickValue::Speed3),
            Some(QuickValue::Speed4),
            Some(QuickValue::Speed6),
        ]
    );
    assert_eq!(QuickValue::Speed1_5.text(), "1.5×");
    for other in [1, 2, 3, 4, 5, 7, 9, 28, 255] {
        assert_eq!(
            QuickValue::speed(other),
            None,
            "{other} quarters is not a value the row has"
        );
    }
}

#[test]
fn the_rows_run_in_the_order_the_user_chose() {
    assert_eq!(
        QuickRow::MAIN.map(QuickRow::label),
        ["General", "Screen", "Gameplay", "Controls", "About"]
    );
    assert_eq!(
        QuickRow::GENERAL.map(QuickRow::label),
        [
            "Theme",
            "Wallpaper",
            "Wallpaper Tint",
            "Recents",
            "Game Titles",
            "Lid Power Off",
            "Date & Time"
        ]
    );
    assert_eq!(
        QuickRow::CONTROL_PAGES.map(QuickRow::label),
        ["Shelf", "Shortcuts", "In-Game"]
    );
    assert_eq!(
        QuickRow::SHELF_KEYS.map(QuickRow::label),
        ["Previous Shelf", "Next Shelf", "Favorite"]
    );
    assert_eq!(
        QuickRow::SCREEN.map(QuickRow::label),
        ["General", "Game Boy Advance", "Game Boy Color", "Game Boy"]
    );
    assert_eq!(
        QuickRow::SCREEN_GENERAL.map(QuickRow::label),
        ["Brightness", "Blue Light", "Saturation", "Contrast"]
    );
    assert_eq!(
        QuickRow::SCREEN_GBA.map(QuickRow::label),
        [
            "Shader",
            "Color Correction",
            "Frame Blending",
            "Bezel",
            "Scale"
        ]
    );
    assert_eq!(
        QuickRow::SCREEN_GBC.map(QuickRow::label),
        [
            "Shader",
            "Color Correction",
            "Frame Blending",
            "Bezel",
            "Scale",
            "Stretch"
        ]
    );
    assert_eq!(
        QuickRow::SCREEN_GB.map(QuickRow::label),
        [
            "Shader",
            "Model",
            "Color Correction",
            "Frame Blending",
            "Palette",
            "Bezel",
            "Scale",
            "Stretch"
        ]
    );
    assert_eq!(
        QuickRow::GAME.map(QuickRow::label),
        [
            "Fast Forward",
            "Fast Forward Sound",
            "Rewind",
            "Turbo Buttons",
            "Rumble",
            "Left Stick as D-Pad",
            "Load State on Insert",
            "Save State on Eject"
        ]
    );
    let opens: Vec<QuickRow> = QuickRow::ALL.into_iter().filter(|r| r.opens()).collect();
    assert_eq!(
        opens,
        [
            QuickRow::Screen,
            QuickRow::DateTime,
            QuickRow::About,
            QuickRow::Game,
            QuickRow::Resume,
            QuickRow::States,
            QuickRow::Screenshots,
            QuickRow::Settings,
            QuickRow::ExitGame,
            QuickRow::UndoReset,
            QuickRow::PlayScreen,
            QuickRow::PlayGameplay,
            QuickRow::ScreenGba,
            QuickRow::ScreenGbc,
            QuickRow::ScreenGb,
            QuickRow::General,
            QuickRow::PlayControls,
            QuickRow::Shortcuts,
            QuickRow::Controls,
            QuickRow::ControlsShelf,
            QuickRow::ControlsInGame,
            QuickRow::ScreenGeneral,
            QuickRow::UndoSave,
            QuickRow::UndoLoad,
            QuickRow::Cheats,
            QuickRow::Guide
        ]
    );
    for page in [
        &QuickRow::MAIN[..],
        &QuickRow::SCREEN[..],
        &QuickRow::SCREEN_GENERAL[..],
        &QuickRow::GAME[..],
        &QuickRow::GENERAL[..],
        &QuickRow::PLAY[..],
        &QuickRow::SETTINGS[..],
        &QuickRow::PLAY_SCREEN[..],
        &QuickRow::SCREEN_GBA[..],
        &QuickRow::SCREEN_GBC[..],
        &QuickRow::SCREEN_GB[..],
        &QuickRow::CHEAT_LIST[..],
    ] {
        for &row in page {
            assert_eq!(row.page(), page, "{row:?}");
        }
    }
}

#[test]
fn a_hidden_row_is_stepped_over_and_takes_no_place() {
    let hidden = [QuickRow::WallpaperTint];
    assert_eq!(QuickRow::Wallpaper.down(&hidden), QuickRow::Recents);
    assert_eq!(QuickRow::Recents.up(&hidden), QuickRow::Wallpaper);
    assert_eq!(QuickRow::Theme.up(&hidden), QuickRow::DateTime);
    assert_eq!(QuickRow::Recents.position(&hidden), 2);
    assert_eq!(QuickRow::Wallpaper.down(&[]), QuickRow::WallpaperTint);
    assert_eq!(QuickRow::Recents.position(&[]), 3);
    assert_eq!(
        slot_ui::quick_row_top(QuickRow::Recents, &hidden)
            - slot_ui::quick_row_top(QuickRow::Wallpaper, &hidden),
        slot_ui::QUICK_PITCH
    );
}

#[test]
fn the_values_read_as_the_menu_prints_them() {
    assert_eq!(
        QuickValue::ALL.map(QuickValue::text),
        [
            "1.5×",
            "2×",
            "3×",
            "4×",
            "6×",
            "On",
            "Off",
            "LCD3x",
            "Grid",
            "Dot",
            "Simpletex",
            "Shimmerless",
            "Authentic",
            "Game Boy",
            "Pocket",
            "Dark",
            "Light",
            "Show",
            "Hide",
            "25%",
            "45%",
            "65%",
            "1 min",
            "3 min",
            "5 min",
            "10 min",
            "15 min",
            "30 min",
            "0",
            "1",
            "2",
            "3",
            "4",
            "5",
            "6",
            "7",
            "8",
            "9",
            "SP Screen",
            "Integer",
            "Fit",
            "Classic",
            "Full Screen",
            "Mix",
            "Ghosting",
        ]
    );
    assert_eq!(QuickValue::flag(true), QuickValue::On);
    assert_eq!(QuickValue::flag(false), QuickValue::Off);
}

#[test]
fn the_date_and_time_read_as_a_month_a_day_and_the_carousels_clock() {
    let at = |stamp: &str, h24| date_time_text(parse_stamp(stamp).expect("a stamp"), h24);
    assert_eq!(at("2026-09-15_16-35-00", true), "SEP 15 16:35");
    assert_eq!(at("2027-01-05_04-07-59", true), "JAN 5 04:07");
    assert_eq!(at("2026-09-15_16-35-00", false), "SEP 15 4:35 PM");
}

#[test]
fn a_picker_for_a_set_clock_starts_at_the_local_time_in_its_city() {
    let utc = parse_stamp("2026-09-15_21-35-42").expect("a stamp");
    let ny = Zone::parse("America/New_York").expect("New York");
    let p = ClockPicker::new(utc, ny, true);
    assert_eq!(p.zone(), ny);
    assert_eq!(
        p.secs(),
        utc - 42,
        "the picker does not show the minute it was opened in"
    );
    assert_eq!(p.value_text(ClockRow::Date), "SEP 15 2026");
    assert_eq!(p.value_text(ClockRow::Time), "17:35");
}

#[test]
fn a_value_is_grey_until_its_row_is_in_hand() {
    let inkiest = |lit| {
        let f = quick_value_face("4×", lit);
        f.rgba
            .chunks(4)
            .max_by_key(|p| p[3])
            .map(|p| [p[0], p[1], p[2]])
            .expect("an empty face")
    };
    assert_eq!(inkiest(true), [0xf6, 0xf4, 0xef]);
    assert_eq!(inkiest(false), [0x9a, 0x9a, 0xa4]);
}

#[test]
fn the_pointer_is_a_crisp_pixel_arrow_in_the_menus_ink() {
    let p = slot_ui::quick_pointer_face();
    assert_eq!(p.w, 18);
    let opaque: Vec<&[u8]> = p.rgba.chunks(4).filter(|c| c[3] > 0).collect();
    assert!(!opaque.is_empty(), "an empty pointer");
    assert!(
        opaque.iter().all(|c| c == &[0xf6, 0xf4, 0xef, 0xff]),
        "the pointer is not one solid colour"
    );
    assert!(
        p.rgba.chunks(4).all(|c| c[3] == 0 || c[3] == 0xff),
        "the arrow was smoothed, not drawn in pixels"
    );
}

#[test]
fn a_label_is_grey_until_its_row_is_in_hand() {
    let inkiest = |lit| {
        let f = quick_label_face(QuickRow::Rumble, lit);
        f.rgba
            .chunks(4)
            .max_by_key(|p| p[3])
            .map(|p| [p[0], p[1], p[2]])
            .expect("an empty face")
    };
    assert_eq!(inkiest(true), [0xf6, 0xf4, 0xef]);
    assert_eq!(inkiest(false), [0x9a, 0x9a, 0xa4]);
}

#[test]
fn the_in_game_pages_hold_their_rows_in_order() {
    assert_eq!(
        QuickRow::PLAY,
        [
            QuickRow::Resume,
            QuickRow::UndoSave,
            QuickRow::UndoLoad,
            QuickRow::UndoReset,
            QuickRow::States,
            QuickRow::Screenshots,
            QuickRow::Cheats,
            QuickRow::Guide,
            QuickRow::Settings,
            QuickRow::ExitGame
        ]
    );
    assert_eq!(
        QuickRow::SETTINGS.map(QuickRow::label),
        ["Screen", "Gameplay", "Controls"]
    );
    assert_eq!(
        QuickRow::CONTROLS.map(QuickRow::label),
        [
            "Up", "Down", "Left", "Right", "A Button", "B Button", "Select", "Start", "L Button",
            "R Button", "Turbo A", "Turbo B"
        ]
    );
    assert_eq!(
        QuickRow::PLAY_SCREEN,
        [
            QuickRow::Shader,
            QuickRow::Model,
            QuickRow::Bezel,
            QuickRow::Scale,
            QuickRow::Stretch,
            QuickRow::Palette,
            QuickRow::ColourCorrection,
            QuickRow::Blending,
            QuickRow::PlayBrightness,
            QuickRow::PlayBlueLight,
            QuickRow::PlaySaturation,
            QuickRow::PlayContrast
        ]
    );
    for row in QuickRow::PLAY {
        assert_eq!(row.parent_in_game(), None, "{row:?}");
    }
}

#[test]
fn in_game_pages_climb_back_through_settings() {
    assert_eq!(QuickRow::Settings.child(), Some(QuickRow::PlayScreen));
    assert_eq!(QuickRow::PlayScreen.child(), Some(QuickRow::Shader));
    assert_eq!(
        QuickRow::PlayBrightness.parent_in_game(),
        Some(QuickRow::PlayScreen)
    );
    assert_eq!(QuickRow::PlayGameplay.child(), Some(QuickRow::FastForward));
    assert_eq!(
        QuickRow::Shader.parent_in_game(),
        Some(QuickRow::PlayScreen)
    );
    assert_eq!(
        QuickRow::PlayGameplay.parent_in_game(),
        Some(QuickRow::Settings)
    );
    assert_eq!(
        QuickRow::Rumble.parent_in_game(),
        Some(QuickRow::PlayGameplay)
    );
    assert_eq!(QuickRow::Rumble.parent(), Some(QuickRow::Game));
}

#[test]
fn in_game_rows_read_as_the_menu_shows_them() {
    let labels = [
        (QuickRow::Resume, "Resume"),
        (QuickRow::States, "States"),
        (QuickRow::Settings, "Settings"),
        (QuickRow::ExitGame, "Exit Game"),
        (QuickRow::PlayScreen, "Screen"),
        (QuickRow::PlayGameplay, "Gameplay"),
        (QuickRow::Shader, "Shader"),
        (QuickRow::ColourCorrection, "Color Correction"),
        (QuickRow::Blending, "Frame Blending"),
        (QuickRow::Palette, "Palette"),
        (QuickRow::PlayBrightness, "Brightness"),
        (QuickRow::PlayBlueLight, "Blue Light"),
    ];
    for (row, label) in labels {
        assert_eq!(row.label(), label);
    }
    for row in QuickRow::PLAY.into_iter().chain(QuickRow::SETTINGS) {
        assert!(row.centred(), "{row:?}");
    }
    for row in QuickRow::PLAY_SCREEN {
        assert!(!row.opens(), "{row:?}");
        assert!(!row.centred(), "{row:?}");
    }
    assert!(QuickRow::General.centred());
    assert!(!QuickRow::Rumble.centred());
}

#[test]
fn a_hidden_palette_and_bezel_leave_nine_screen_rows() {
    let hidden = [QuickRow::Palette, QuickRow::Bezel, QuickRow::Model];
    assert_eq!(
        QuickRow::Shader.shown(&hidden),
        vec![
            QuickRow::Shader,
            QuickRow::Scale,
            QuickRow::Stretch,
            QuickRow::ColourCorrection,
            QuickRow::Blending,
            QuickRow::PlayBrightness,
            QuickRow::PlayBlueLight,
            QuickRow::PlaySaturation,
            QuickRow::PlayContrast
        ]
    );
    assert_eq!(QuickRow::PlayContrast.down(&hidden), QuickRow::Shader);
}

#[test]
fn the_in_game_screen_panel_shows_three_rows_and_scrolls_the_rest() {
    assert_eq!(
        slot_ui::quick_window(QuickRow::Shader, &[QuickRow::Model]),
        (0, 3)
    );
    assert_eq!(
        slot_ui::quick_window(QuickRow::Bezel, &[QuickRow::Model]),
        (0, 3)
    );
    assert_eq!(
        slot_ui::quick_window(QuickRow::Scale, &[QuickRow::Model]),
        (1, 3)
    );
    assert_eq!(
        slot_ui::quick_window(QuickRow::Stretch, &[QuickRow::Model]),
        (2, 3)
    );
    assert_eq!(
        slot_ui::quick_window(QuickRow::PlayContrast, &[QuickRow::Model]),
        (8, 3)
    );
    let hidden = [QuickRow::Palette, QuickRow::Bezel, QuickRow::Model];
    assert_eq!(
        slot_ui::quick_window(QuickRow::PlayContrast, &hidden),
        (6, 3)
    );
}

#[test]
fn a_scroll_bar_shows_only_on_a_page_with_rows_out_of_sight() {
    let top =
        slot_ui::quick_scroll(QuickRow::Shader, &[QuickRow::Model]).expect("eleven rows in three");
    assert!(
        (top.thumb_y - top.y).abs() < 0.5,
        "the thumb is not at the top"
    );
    assert!(
        (top.thumb_h - top.h / 11.0).abs() < 0.5,
        "the thumb is not one row of eleven"
    );
    let end = slot_ui::quick_scroll(QuickRow::PlayContrast, &[QuickRow::Model])
        .expect("eleven rows in three");
    assert!(
        (end.thumb_y + end.thumb_h - (end.y + end.h)).abs() < 0.5,
        "the thumb is not at the bottom"
    );
    assert!(
        slot_ui::quick_scroll(QuickRow::Shader, &[QuickRow::Palette, QuickRow::Bezel]).is_some(),
        "nine rows in three still scroll"
    );
    assert_eq!(slot_ui::quick_scroll(QuickRow::Rumble, &[]), None);
    assert!(slot_ui::quick_scroll(QuickRow::GameUp, &[]).is_some());
}

#[test]
fn the_thumb_reaches_the_bottom_only_on_the_last_row() {
    let rows = QuickRow::CONTROLS;
    let mut last_y = f32::MIN;
    for (i, row) in rows.iter().enumerate() {
        let bar = slot_ui::quick_scroll(*row, &[]).expect("the controls page scrolls");
        let at_bottom = (bar.thumb_y + bar.thumb_h - (bar.y + bar.h)).abs() < 0.5;
        assert_eq!(at_bottom, i == rows.len() - 1, "{row:?} at {i}");
        assert!(bar.thumb_y > last_y, "{row:?}: the thumb did not move down");
        last_y = bar.thumb_y;
    }
}

#[test]
fn screen_opens_a_page_per_console_and_each_backs_out_to_its_console() {
    assert_eq!(QuickRow::Screen.child(), Some(QuickRow::ScreenGeneral));
    assert_eq!(QuickRow::ScreenGeneral.child(), Some(QuickRow::Brightness));
    assert!(QuickRow::ScreenGeneral.opens());
    assert!(QuickRow::ScreenGeneral.centred());
    assert!(!QuickRow::Brightness.centred());
    assert!(!QuickRow::Brightness.opens());
    for (console, page) in [
        (QuickRow::ScreenGeneral, &QuickRow::SCREEN_GENERAL[..]),
        (QuickRow::ScreenGba, &QuickRow::SCREEN_GBA[..]),
        (QuickRow::ScreenGbc, &QuickRow::SCREEN_GBC[..]),
        (QuickRow::ScreenGb, &QuickRow::SCREEN_GB[..]),
    ] {
        assert_eq!(console.parent(), Some(QuickRow::Screen));
        assert_eq!(console.child(), Some(page[0]));
        for &row in page {
            assert_eq!(row.parent(), Some(console), "{row:?}");
        }
    }
    assert!(QuickRow::ScreenGb.centred());
    assert!(!QuickRow::GbShader.centred());
}

#[test]
fn screen_rows_in_game_sit_in_a_panel_at_the_bottom() {
    let hidden = [QuickRow::Palette];
    let shader = slot_ui::quick_row_top(QuickRow::Shader, &hidden);
    let bezel = slot_ui::quick_row_top(QuickRow::Bezel, &hidden);
    assert_eq!(bezel - shader, slot_ui::QUICK_PITCH);
    assert!(
        shader > slot_ui::OUT_H as f32 / 2.0,
        "the panel starts at {shader}, covering more than three rows of the game"
    );
    assert!(slot_ui::quick_row_top(QuickRow::FastForward, &[]) < shader);
}

#[test]
fn the_wallpaper_tint_reads_as_how_much_it_darkens() {
    assert_eq!(
        [QuickValue::Low, QuickValue::Medium, QuickValue::High].map(QuickValue::text),
        ["25%", "45%", "65%"]
    );
}

#[test]
fn show_titles_sits_under_recents_on_the_general_page() {
    let at = |r: QuickRow| QuickRow::GENERAL.iter().position(|g| *g == r);
    assert_eq!(
        at(QuickRow::ShowTitles),
        at(QuickRow::Recents).map(|i| i + 1)
    );
    assert_eq!(QuickRow::ShowTitles.parent(), Some(QuickRow::General));
}
