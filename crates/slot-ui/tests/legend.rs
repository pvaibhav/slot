use slot_ui::{
    core_button_legends, footer_ink, link_button_legends, quick_legend_faces, UndoFace, HINT_EDGE,
    MOUTH_H,
};

#[test]
fn footer_legends_keep_the_hint_on_the_inner_side() {
    let menu = quick_legend_faces(footer_ink(false));
    let light = quick_legend_faces(footer_ink(true));
    assert_eq!(hint_ink(&menu[0], true), footer_ink(false));
    assert_eq!(hint_ink(&light[0], true), footer_ink(true));
    assert_eq!(hint_ink(&light[2], false), footer_ink(true));
    assert!(menu.iter().all(|f| f.h == MOUTH_H as u32));
    assert!(menu[3].w > menu[0].w, "Resume is a longer hint than Back");
    assert!(
        hint_is_beside(&menu[0], true),
        "Back sits to the right of B"
    );
    assert!(
        hint_is_beside(&menu[2], false),
        "Open sits to the left of A"
    );
    assert!(
        hint_is_beside(&menu[4], false),
        "Restart sits to the left of X"
    );
    let link = link_button_legends([0xff, 0xff, 0xff]);
    assert_eq!(link.len(), 7);
    assert!(link.iter().all(|f| f.h == MOUTH_H as u32));
    assert!(
        hint_is_beside(&link[1], true),
        "Mode sits to the right of Select"
    );
    assert!(
        hint_is_beside(&link[2], false),
        "Swap sits to the left of the d-pad"
    );
    let core = core_button_legends(footer_ink(false));
    assert!(core.iter().all(|f| f.h == MOUTH_H as u32));
    assert!(
        hint_is_beside(&core[0], true),
        "Cancel sits to the right of B"
    );
    assert!(
        hint_is_beside(&core[1], true),
        "Swap sits to the right of the d-pad"
    );
    assert!(
        hint_is_beside(&core[2], false),
        "Choose sits to the left of A"
    );
}

fn hint_is_beside(face: &slot_ui::UndoFace, on_right: bool) -> bool {
    let (button, word) = match on_right {
        true => (0..40, face.w - HINT_EDGE - 24..face.w - HINT_EDGE),
        false => (face.w - HINT_EDGE - 40..face.w - HINT_EDGE, 0..24),
    };
    painted(face, button, 2..12)
        && !painted(face, word.clone(), 2..12)
        && painted(face, word, 22..36)
}

fn hint_ink(face: &UndoFace, on_right: bool) -> [u8; 3] {
    let xs = match on_right {
        true => face.w - HINT_EDGE - 24..face.w - HINT_EDGE,
        false => 0..24,
    };
    let mut best = (0u8, [0u8; 3]);
    for y in 22..36 {
        for x in xs.clone() {
            let i = ((y * face.w + x) * 4) as usize;
            if face.rgba[i + 3] > best.0 {
                best = (face.rgba[i + 3], face.rgba[i..i + 3].try_into().unwrap());
            }
        }
    }
    assert!(best.0 > 16, "the hint has no ink");
    best.1
}

fn painted(face: &slot_ui::UndoFace, xs: std::ops::Range<u32>, ys: std::ops::Range<u32>) -> bool {
    ys.flat_map(|y| xs.clone().map(move |x| (x, y)))
        .any(|(x, y)| face.rgba[((y * face.w + x) * 4 + 3) as usize] > 16)
}
