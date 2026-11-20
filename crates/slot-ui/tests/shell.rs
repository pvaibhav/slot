use slot_store::scan;
use slot_ui::{
    cart_face, gb_plastics, gb_shell_of, gba_plastics, gba_shell_for, label_colour, label_text,
    lookup_order_is_exact_then_family_then_default, shell_for, shell_presets, table_keys, Finish,
    GbShell, DEFAULT_SHELL, DMG_SHELL, DUAL_MODE_SHELL, LABEL_WELL, LABEL_X, LABEL_Y,
};
use tempfile::TempDir;

fn tmp_root() -> TempDir {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in [
        "Games",
        "Games/GBA",
        "Labels",
        "Saves",
        "States",
        "System",
        "Config",
    ] {
        std::fs::create_dir(d.path().join(sub)).expect("create content dir");
    }
    d
}

fn write_rom_with_code(d: &TempDir, name: &str, title: &str, code: &str) {
    let mut rom = vec![0u8; 0x100];
    rom[0xa0..0xa0 + title.len()].copy_from_slice(title.as_bytes());
    rom[0xac..0xac + code.len()].copy_from_slice(code.as_bytes());
    std::fs::write(d.path().join("Games/GBA").join(name), rom).expect("write rom");
}

#[test]
fn an_unknown_game_gets_the_default_grey() {
    assert_eq!(gba_shell_for("ZZZZ").colour, DEFAULT_SHELL.colour);
    assert_eq!(gba_shell_for("").colour, DEFAULT_SHELL.colour);
    assert_eq!(gba_shell_for("AMTE").colour, DEFAULT_SHELL.colour);
}

#[test]
fn leafgreen_is_green_whatever_region_it_came_from() {
    for code in ["BPGE", "BPGJ", "BPGP", "BPGD"] {
        let s = gba_shell_for(code);
        assert_ne!(
            s.colour, DEFAULT_SHELL.colour,
            "{code} fell through to grey"
        );
        assert!(s.colour[1] > s.colour[0], "{code} is not green");
    }
}

#[test]
fn gba_video_carts_are_light_grey() {
    let v = gba_shell_for("MSKE");
    assert_ne!(
        v.colour, DEFAULT_SHELL.colour,
        "video fell through to the default grey"
    );
    assert!(
        v.colour.iter().all(|c| *c > 0xA0),
        "video shells are light grey, got {:?}",
        v.colour
    );
    assert_eq!(gba_shell_for("MPOE").colour, v.colour);
}

#[test]
fn an_exact_entry_outranks_the_family_letter() {
    assert_eq!(gba_shell_for("MSKE").colour, gba_shell_for("MSKJ").colour);
    assert!(lookup_order_is_exact_then_family_then_default());
}

#[test]
fn the_clear_carts_are_clear_and_the_rest_are_solid() {
    for code in table_keys() {
        let clear = code == "RZW" || ["AX", "BPE", "U3"].iter().any(|p| code.starts_with(p));
        let want = if clear {
            Finish::Translucent
        } else {
            Finish::Solid
        };
        assert_eq!(
            gba_shell_for(code).finish,
            want,
            "{code} has the wrong finish"
        );
    }
    assert_eq!(
        gba_shell_for("MSKE").finish,
        Finish::Solid,
        "the video family is not solid"
    );
    assert_eq!(
        gba_shell_for("ZZZZ").finish,
        Finish::Solid,
        "the default is not solid"
    );
}

#[test]
fn the_pokemon_shells_are_all_distinct() {
    let codes = ["AXVE", "AXPE", "BPEE", "BPRE", "BPGE"];
    let mut seen = Vec::new();
    for c in codes {
        let col = gba_shell_for(c).colour;
        assert!(
            !seen.contains(&col),
            "{c} shares a colour with another cart"
        );
        seen.push(col);
    }
}

#[test]
fn no_two_table_entries_share_a_key() {
    let mut keys: Vec<&str> = table_keys();
    keys.sort();
    let before = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), before, "two entries claim the same key");
    assert!(
        keys.iter().all(|k| k.len() == 3 || k.len() == 4),
        "keys are a region free prefix or one region's whole code"
    );
}

#[test]
fn a_region_that_shipped_other_plastic_gets_its_own() {
    let us = gba_shell_for("RZWE");
    let jp = gba_shell_for("RZWJ");
    assert_eq!(us.finish, Finish::Translucent);
    assert_eq!(jp.finish, Finish::Solid);
    assert!(jp.colour.iter().all(|c| *c > 0xd0), "Japan's is white");
    assert_ne!(us.colour, DEFAULT_SHELL.colour);
}

#[test]
fn boktai_is_clear_everywhere_and_drill_dozer_is_red() {
    for code in ["U3IE", "U3IP", "U3IJ", "U32E", "U33J"] {
        assert_eq!(gba_shell_for(code).finish, Finish::Translucent, "{code}");
    }
    let dozer = gba_shell_for("V49E");
    assert!(dozer.colour[0] > dozer.colour[2] + 40, "Drill Dozer is red");
    assert_eq!(dozer.colour, gba_shell_for("V49J").colour);
}

fn gb_shell(title: &[u8], code: &[u8], cgb: u8, japan: bool) -> slot_ui::Shell {
    let d = tempfile::tempdir().expect("tempdir");
    let games = d.path().join("Games/GB");
    std::fs::create_dir_all(&games).expect("games dir");
    let mut rom = vec![0u8; 0x150];
    rom[0x134..0x134 + title.len()].copy_from_slice(title);
    rom[0x13f..0x13f + code.len()].copy_from_slice(code);
    rom[0x143] = cgb;
    rom[0x14a] = u8::from(!japan);
    std::fs::write(games.join("Pak.gb"), rom).expect("rom");
    shell_for(&scan(d.path()).expect("scan")[0])
}

#[test]
fn red_and_blue_are_coloured_outside_japan_only() {
    let red = gb_shell(b"POKEMON RED", b"", 0x00, false);
    let blue = gb_shell(b"POKEMON BLUE", b"", 0x00, false);
    assert!(red.colour[0] > red.colour[2] + 60, "Red is red");
    assert!(blue.colour[2] > blue.colour[0] + 60, "Blue is blue");
    assert_eq!(
        gb_shell(b"POKEMON RED", b"", 0x00, true),
        DMG_SHELL,
        "Japan's Red is the grey pak"
    );
}

#[test]
fn gold_is_gold_except_in_japan() {
    let gold = gb_shell(b"POKEMON_GLD", b"AAUE", 0x80, false);
    assert_ne!(gold, DUAL_MODE_SHELL);
    assert_eq!(gold, gb_shell(b"POKEMON_GLD", b"AAUD", 0x80, false));
    assert_eq!(
        gb_shell(b"POKEMON_GLD", b"AAUJ", 0x80, true),
        DUAL_MODE_SHELL
    );
    assert_ne!(gb_shell(b"POKEMON_SLV", b"AAXE", 0x80, false), gold);
}

#[test]
fn crystal_is_aqua_under_both_its_codes() {
    let en = gb_shell(b"PM_CRYSTAL", b"BYTE", 0xc0, false);
    let jp = gb_shell(b"PM_CRYSTAL", b"BXTJ", 0xc0, true);
    assert_eq!(en, jp);
    assert_eq!(
        en.finish,
        Finish::Glitter,
        "Crystal's plastic has glitter in it"
    );
    assert!(en.colour[2] > en.colour[0], "Crystal is aqua");
}

#[test]
fn metal_gear_solid_is_black_not_clear() {
    assert_eq!(
        gb_shell(b"METALGEARGB", b"BMGE", 0xc0, false),
        DUAL_MODE_SHELL
    );
}

#[test]
fn pinball_is_yellow_in_japan_only() {
    let jp = gb_shell(b"POKEPINBALL", b"VPHJ", 0x80, true);
    assert!(
        jp.colour[0] > 0xc0 && jp.colour[2] < 0x60,
        "Japan's is yellow"
    );
    assert_eq!(
        gb_shell(b"POKEPINBALL", b"VPHE", 0x80, false),
        DUAL_MODE_SHELL
    );
}

#[test]
fn an_unlisted_pak_keeps_its_flags_plastic() {
    assert_eq!(gb_shell(b"TETRIS", b"", 0x00, false), DMG_SHELL);
    assert_eq!(gb_shell(b"ZELDA", b"AZLE", 0x80, false), DUAL_MODE_SHELL);
}

#[test]
fn a_shell_without_a_label_image_is_one_plastic() {
    let d = tmp_root();
    write_rom_with_code(&d, "Drill Dozer.gba", "DRILL DOZER", "V49E");
    let cart = &scan(d.path()).unwrap()[0];
    let f = slot_ui::cart_face_with(cart, Some(sheet(cart, label_colour(&label_text(cart)))));
    let px = |x: u32, y: u32| {
        let i = ((y * f.w + x) * 4) as usize;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    };
    let flat = px(32, 40);
    assert_eq!(
        flat,
        [0xa9, 0x51, 0x3b],
        "Drill Dozer is not its red plastic"
    );
    assert_eq!(
        px(LABEL_X, LABEL_Y),
        label_colour(&label_text(cart)),
        "the label well is not the generated label"
    );
    let mut letters = 0;
    for y in 0..LABEL_Y {
        for x in 0..f.w {
            let i = ((y * f.w + x) * 4) as usize;
            if f.rgba[i + 3] > 200
                && [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]] != [0xa9, 0x51, 0x3b]
            {
                letters += 1;
            }
        }
    }
    assert!(
        letters > 80,
        "the console name is missing from the shoulder: {letters} px"
    );
}

fn chosen(dir: &str, cgb: u8, code: &str, line: Option<&str>) -> (TempDir, slot_store::Cart) {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in [
        format!("Games/{dir}"),
        "System".to_string(),
        "Config".to_string(),
    ] {
        std::fs::create_dir_all(d.path().join(sub)).expect("dir");
    }
    let mut rom = vec![0u8; 0x150];
    rom[0x143] = cgb;
    rom[0xac..0xac + code.len()].copy_from_slice(code.as_bytes());
    let ext = dir.to_lowercase();
    std::fs::write(d.path().join(format!("Games/{dir}/Pak.{ext}")), rom).expect("rom");
    if let Some(line) = line {
        std::fs::write(
            d.path().join("Config/cart_shell.txt"),
            format!("Pak = {line}\n"),
        )
        .expect("ini");
    }
    let cart = scan(d.path()).expect("scan").remove(0);
    (d, cart)
}

#[test]
fn a_chosen_colour_and_finish_win() {
    let (_d, cart) = chosen("GB", 0x00, "", Some("auto 123456 glitter"));
    assert_eq!(shell_for(&cart).colour, [0x12, 0x34, 0x56]);
    assert_eq!(shell_for(&cart).finish, Finish::Glitter);
    let (_d, cart) = chosen("GBA", 0, "BPEE", Some("auto abcdef clear"));
    assert_eq!(shell_for(&cart).colour, [0xab, 0xcd, 0xef]);
    assert_eq!(shell_for(&cart).finish, Finish::Translucent);
}

#[test]
fn no_choice_changes_nothing() {
    for (dir, cgb, code) in [
        ("GB", 0x00, ""),
        ("GBC", 0xc0, ""),
        ("GBA", 0, "BPEE"),
        ("GBA", 0, "AMTE"),
    ] {
        let (_d, plain) = chosen(dir, cgb, code, None);
        let (_d2, garbled) = chosen(dir, cgb, code, Some("rounded"));
        assert_eq!(plain.shell, None);
        assert_eq!(shell_for(&plain), shell_for(&garbled), "{dir} {code}");
        assert_eq!(gb_shell_of(&plain), gb_shell_of(&garbled), "{dir} {code}");
    }
}

#[test]
fn a_chosen_outline_wins_on_a_game_boy_cart_and_not_on_gba() {
    let (_d, grey) = chosen("GB", 0x00, "", Some("rounded 9a978f solid"));
    assert_eq!(gb_shell_of(&grey), Some(GbShell::Rounded));
    let (_d, clear) = chosen("GBC", 0xc0, "", Some("notched 7c7a8a clear"));
    assert_eq!(gb_shell_of(&clear), Some(GbShell::Notched));
    let (_d, auto) = chosen("GBC", 0xc0, "", Some("auto 7c7a8a clear"));
    assert_eq!(gb_shell_of(&auto), Some(GbShell::Rounded));
    let (_d, gba) = chosen("GBA", 0, "AMTE", Some("rounded 112233 solid"));
    assert_eq!(gb_shell_of(&gba), None);
}

#[test]
fn a_rounded_choice_on_a_grey_pak_draws_the_rounded_outline() {
    let (_d, chosen_rounded) = chosen("GB", 0x00, "", Some("rounded 9a978f solid"));
    let (_d2, real_rounded) = chosen("GBC", 0xc0, "", Some("auto 9a978f solid"));
    let alpha = |cart: &slot_store::Cart| {
        let f = cart_face(cart);
        (0..f.w * f.h)
            .map(|i| f.rgba[(i * 4 + 3) as usize])
            .collect::<Vec<_>>()
    };
    assert_eq!(alpha(&chosen_rounded), alpha(&real_rounded));
}

#[test]
fn the_presets_are_the_tables_plastics_each_once() {
    let presets = shell_presets();
    assert_eq!(
        presets.len(),
        25,
        "a table row the presets name has gone missing"
    );
    let mut names: Vec<_> = presets.iter().map(|(n, _)| *n).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), presets.len(), "two presets share a name");
    let find = |n: &str| presets.iter().find(|(name, _)| *name == n).unwrap().1;
    assert_eq!(find("Game Boy grey"), DMG_SHELL);
    assert_eq!(find("Pokémon Emerald"), gba_shell_for("BPEE"));
}

fn greenish(face: &slot_ui::CartFace) -> usize {
    face.rgba
        .chunks_exact(4)
        .filter(|p| p[3] > 0 && p[1] as i32 > p[0] as i32 + 10 && p[1] as i32 > p[2] as i32 + 10)
        .count()
}

#[test]
fn a_clear_gba_cart_has_no_board_baked_into_its_face() {
    let (_d, clear) = chosen("GBA", 0, "BPEE", Some("auto d9dbd8 clear"));
    let (_d2, solid) = chosen("GBA", 0, "BPEE", Some("auto d9dbd8 solid"));
    assert_eq!(greenish(&cart_face(&solid)), 0);
    assert_eq!(
        greenish(&cart_face(&clear)),
        0,
        "a clear cart carries its own board, which would move with it"
    );
}

#[test]
fn clear_plastic_is_eighty_percent_alpha_and_the_label_box_stays_opaque() {
    let (_d, clear) = chosen("GBA", 0, "BPEE", Some("auto d9dbd8 clear"));
    let (_d2, solid) = chosen("GBA", 0, "BPEE", Some("auto d9dbd8 solid"));
    let paper = label_colour(&label_text(&clear));
    let (clear, solid) = (
        slot_ui::cart_face_with(&clear, Some(sheet(&clear, paper))),
        slot_ui::cart_face_with(&solid, Some(sheet(&solid, paper))),
    );
    let at = |f: &slot_ui::CartFace, x: u32, y: u32| {
        let i = ((y * f.w + x) * 4) as usize;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2], f.rgba[i + 3]]
    };

    let mut see_through = 0u32;
    for y in 0..clear.h {
        for x in 0..clear.w {
            let (c, s) = (at(&clear, x, y), at(&solid, x, y));
            if s[3] == 255 && c[3] == 204 {
                see_through += 1;
            } else {
                assert!(c[3] <= s[3], "the clear cart is more opaque at {x},{y}");
            }
        }
    }
    assert!(
        see_through > 10_000,
        "the plastic is not see-through: {see_through} px"
    );

    assert_eq!(at(&clear, 40, 50)[3], 204, "the plastic is not 80%");
    let well = at(&clear, LABEL_X, LABEL_Y);
    assert_eq!(well[..3], paper, "the label was washed: {:?}", &well[..3]);
    assert_eq!(well[3], 255, "the label is see-through");
}

#[test]
fn a_clear_gbc_pak_is_see_through() {
    let (_d, clear) = chosen("GBC", 0xc0, "", Some("auto ffffff clear"));
    let (_d2, solid) = chosen("GBC", 0xc0, "", Some("auto ffffff solid"));
    let (clear, solid) = (cart_face(&clear), cart_face(&solid));
    let at = |f: &slot_ui::CartFace| {
        let i = ((50 * f.w + 40) * 4) as usize;
        f.rgba[i + 3]
    };
    assert_eq!(at(&clear), 204, "the clear pak is not 80%");
    assert_eq!(at(&solid), 255, "the solid pak is see-through");
}

#[test]
fn firered_and_leafgreen_are_solid() {
    for code in ["BPRE", "BPGE"] {
        assert_eq!(
            gba_shell_for(code).finish,
            Finish::Solid,
            "{code} is not solid"
        );
    }
}

#[test]
fn each_table_plastic_is_the_neutral_shell_blended_with_its_colour() {
    let sample = |cart: &slot_store::Cart| {
        let f = cart_face(cart);
        let i = ((40 * f.w + 32) * 4) as usize;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    };
    for colour in gba_plastics() {
        let hex = format!("{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2]);
        let (_d, cart) = chosen("GBA", 0, "BPEE", Some(&format!("auto {hex} solid")));
        assert_eq!(sample(&cart), colour, "gba {hex} was tinted or substituted");
    }
    for colour in gb_plastics() {
        let hex = format!("{:02x}{:02x}{:02x}", colour[0], colour[1], colour[2]);
        let (_d, cart) = chosen("GB", 0x00, "", Some(&format!("auto {hex} solid")));
        let f = cart_face(&cart);
        let i = ((20 * f.w + 20) * 4) as usize;
        assert_eq!(
            [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]],
            colour,
            "gb {hex} was tinted or substituted"
        );
    }
}

#[test]
fn any_colour_is_blended_onto_the_one_neutral_shell() {
    let (_d, cart) = chosen("GBA", 0, "BPEE", Some("auto 123456 solid"));
    assert_eq!(shell_for(&cart).colour, [0x12, 0x34, 0x56]);
    let f = slot_ui::cart_face_with(&cart, Some(sheet(&cart, label_colour(&label_text(&cart)))));
    let at = |x: u32, y: u32| {
        let i = ((y * f.w + x) * 4) as usize;
        [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2]]
    };
    assert_eq!(
        at(32, 40),
        [0x12, 0x34, 0x56],
        "flat plastic is not the colour"
    );
    assert_eq!(
        at(LABEL_X, LABEL_Y),
        label_colour(&label_text(&cart)),
        "the label was tinted with the plastic"
    );
    let darker = (0..f.h)
        .flat_map(|y| (0..f.w).map(move |x| (x, y)))
        .map(|(x, y)| at(x, y))
        .filter(|p| p[0] < 0x12 && p[1] < 0x34 && p[2] < 0x56)
        .count();
    assert!(
        darker > 80,
        "the embossed console name did not survive the blend"
    );
}

#[test]
fn fifty_percent_grey_keeps_the_colour_and_either_side_moves_it() {
    let (_d, cart) = chosen("GBA", 0, "BPEE", Some("auto 606060 clear"));
    let f = cart_face(&cart);
    let (mut same, mut darker, mut lighter) = (0, 0, 0);
    for px in f.rgba.chunks_exact(4).filter(|p| p[3] > 0) {
        if px[..3] == LABEL_WELL {
            continue;
        }
        match px[0].cmp(&0x60) {
            std::cmp::Ordering::Equal => same += 1,
            std::cmp::Ordering::Less => darker += 1,
            std::cmp::Ordering::Greater => lighter += 1,
        }
    }
    assert!(
        same > 1_000,
        "flat plastic is not the set colour: {same} px"
    );
    assert!(
        darker > 500,
        "nothing darker than 50% grey darkened it: {darker} px"
    );
    assert!(
        lighter > 500,
        "nothing lighter than 50% grey lightened it: {lighter} px"
    );
}

#[test]
fn glitter_specks_the_see_through_plastic() {
    let glitter = pak_face(b"PM_CRYSTAL", b"BYTE", 0xc0, None);
    let clear = pak_face(b"PM_CRYSTAL", b"BYTE", 0xc0, Some("auto 86b9bf clear"));
    let mut flecks = 0usize;
    let mut same = 0usize;
    for (a, b) in glitter.rgba.chunks_exact(4).zip(clear.rgba.chunks_exact(4)) {
        if a[3] == 0 {
            continue;
        }
        if a[..3] == b[..3] {
            same += 1;
        } else {
            flecks += 1;
            assert!(
                a[0] >= b[0] && a[1] >= b[1] && a[2] >= b[2],
                "a glitter speck went darker: {:?} on {:?}",
                &a[..3],
                &b[..3]
            );
        }
    }
    assert!(flecks > 50, "glitter added no specks");
    assert!(same > flecks * 20, "glitter repainted the shell");
}

fn pak_face(title: &[u8], code: &[u8], cgb: u8, line: Option<&str>) -> slot_ui::CartFace {
    let d = tempfile::tempdir().expect("tempdir");
    for sub in ["Games/GBC", "System", "Config"] {
        std::fs::create_dir_all(d.path().join(sub)).expect("dir");
    }
    let mut rom = vec![0u8; 0x150];
    rom[0x134..0x134 + title.len()].copy_from_slice(title);
    rom[0x13f..0x13f + code.len()].copy_from_slice(code);
    rom[0x143] = cgb;
    std::fs::write(d.path().join("Games/GBC/Pak.gbc"), rom).expect("rom");
    if let Some(line) = line {
        std::fs::write(
            d.path().join("Config/cart_shell.txt"),
            format!("Pak = {line}\n"),
        )
        .expect("ini");
    }
    cart_face(&scan(d.path()).expect("scan").remove(0))
}

#[test]
fn a_clear_gba_cart_has_a_game_coloured_interior_under_its_board() {
    use slot_ui::{cart_interior, shows_interior, BOARD_H, BOARD_W, GB_CART_H, GB_CART_W};
    let (_d, white) = chosen("GBA", 0, "BPEE", Some("auto ffffff clear"));
    let (_d2, black) = chosen("GBA", 0, "BPEE", Some("auto 000000 clear"));
    let (_d3, solid) = chosen("GBA", 0, "BPEE", Some("auto ffffff solid"));
    let (_d4, pak) = chosen("GB", 0xc0, "", Some("auto ffffff clear"));
    assert!(shows_interior(&white));
    assert!(!shows_interior(&solid), "a solid cart shows no interior");
    assert!(shows_interior(&pak), "a clear pak has no interior");

    let (a, b) = (cart_interior(&white), cart_interior(&black));
    assert_eq!((a.w, a.h), (BOARD_W, BOARD_H));
    let mut differ = 0u32;
    let mut strongest = 0u8;
    for (p, q) in a.rgba.chunks_exact(4).zip(b.rgba.chunks_exact(4)) {
        assert_eq!(p[3], q[3], "the blend changed the interior's shape");
        differ += u32::from(p[..3] != q[..3]);
        strongest = strongest.max(p[3]);
    }
    assert_eq!(strongest, 204, "the interior is not 80% opaque");
    assert!(
        differ > 10_000,
        "the game's colour barely reaches the interior: {differ} px"
    );

    let (_d5, white_pak) = chosen("GB", 0xc0, "", Some("auto ffffff clear"));
    let (_d6, black_pak) = chosen("GB", 0xc0, "", Some("auto 000000 clear"));
    let (p, q) = (cart_interior(&white_pak), cart_interior(&black_pak));
    assert_eq!((p.w, p.h), (GB_CART_W, GB_CART_H));
    let differ = p
        .rgba
        .chunks_exact(4)
        .zip(q.rgba.chunks_exact(4))
        .filter(|(a, b)| a[..3] != b[..3])
        .count();
    assert!(
        differ > 10_000,
        "the pak's interior is not coloured: {differ} px"
    );
}

fn sheet(cart: &slot_store::Cart, rgb: [u8; 3]) -> Vec<u8> {
    let (w, h) = slot_ui::label_size(cart);
    (0..w * h)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
        .collect()
}
