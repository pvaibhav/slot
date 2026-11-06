use slot_ui::{
    draw_slot_title, slot_name_y, title_line_face, word_face, Canvas, Draw, Printed, TexId, HINT_H,
    TITLE_LINE_H,
};

fn ink_rows(face: &slot_ui::UndoFace) -> usize {
    (0..face.h as usize)
        .filter(|y| {
            (0..face.w as usize).any(|x| face.rgba[(y * face.w as usize + x) * 4 + 3] > 128)
        })
        .count()
}

#[test]
fn a_title_is_set_larger_than_the_rack_name() {
    let title = title_line_face("POKEMON EMERALD VERSION");
    let rack = word_face("POKEMON EMERALD VERSION");
    assert_eq!(title.h, TITLE_LINE_H);
    assert!(
        ink_rows(&title) > ink_rows(&rack) + 4,
        "the title letters are {} rows tall, the rack name's {}",
        ink_rows(&title),
        ink_rows(&rack)
    );
}

#[test]
fn a_long_title_stays_inside_the_screen() {
    let face = title_line_face(&"MEGA MAN BATTLE NETWORK 6 CYBEAST GREGAR ".repeat(2));
    assert!(face.w <= 560, "the title is {} wide", face.w);
}

#[test]
fn the_title_sits_on_the_rack_names_baseline_and_grows_upward() {
    let canvas = Canvas { w: 640.0, h: 480.0 };
    let mut out = Vec::new();
    draw_slot_title(canvas, Printed::new(TexId::from_raw(7), 300), 1.0, &mut out);
    let Some(Draw::Tex { x, y, w, h, .. }) = out.first() else {
        panic!("no title drawn");
    };
    assert_eq!(*h, TITLE_LINE_H as f32);
    assert_eq!(y + h, slot_name_y(slot_ui::Canvas::FRAMED) + HINT_H as f32);
    assert_eq!(x + w / 2.0, 320.0);
}
