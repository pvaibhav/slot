use slot_ui::{Canvas, Draw, PairScreen, PairSlot, TexId, MOUTH_H, OUT_H, PAIR_QR};

const PAIR_X: f32 = 40.0;
const PAIR_GAP: f32 = 32.0;

fn tex_rects(out: &[Draw]) -> Vec<(usize, [f32; 4])> {
    out.iter()
        .filter_map(|d| match d {
            Draw::Tex {
                x, y, w, h, tex, ..
            } => Some((*tex, [*x, *y, *w, *h])),
            _ => None,
        })
        .map(|(tex, r)| (tex_index(tex), r))
        .collect()
}

fn tex_index(tex: TexId) -> usize {
    (0..64)
        .find(|i| TexId::from_raw(*i) == tex)
        .expect("unknown texture")
}

fn canvases() -> [Canvas; 2] {
    [
        Canvas::FRAMED,
        Canvas {
            w: 640.0,
            h: OUT_H as f32,
        },
    ]
}

#[test]
fn lines_without_a_qr_are_centred_on_the_canvas_both_ways() {
    for canvas in canvases() {
        let lines = [
            (PairSlot::Scan, (TexId::from_raw(1), 180, 34)),
            (PairSlot::Phone, (TexId::from_raw(2), 231, 34)),
        ];
        let mut out = Vec::new();
        PairScreen {
            qr: None,
            lines: &lines,
            ground: None,
            canvas,
        }
        .draw(&mut out);
        let rects = tex_rects(&out);
        assert_eq!(rects.len(), 2);
        for (_, [x, _, w, _]) in &rects {
            assert!(
                (x + w / 2.0 - canvas.w / 2.0).abs() <= 0.5,
                "off centre at {x}"
            );
        }
        let top = rects[0].1[1];
        let bottom = rects[1].1[1] + rects[1].1[3];
        let room = OUT_H as f32 - MOUTH_H;
        assert!(
            (top + bottom - room).abs() <= 1.0,
            "not centred above the footer"
        );
        assert_eq!(rects[1].1[1] - rects[0].1[1], 34.0);
    }
}

#[test]
fn a_single_line_without_a_qr_sits_in_the_middle() {
    for canvas in canvases() {
        let lines = [(PairSlot::Scan, (TexId::from_raw(1), 200, 34))];
        let mut out = Vec::new();
        PairScreen {
            qr: None,
            lines: &lines,
            ground: None,
            canvas,
        }
        .draw(&mut out);
        let [x, y, w, h] = tex_rects(&out)[0].1;
        assert!((x + w / 2.0 - canvas.w / 2.0).abs() <= 0.5);
        assert!((y + h / 2.0 - (OUT_H as f32 - MOUTH_H) / 2.0).abs() <= 1.0);
    }
}

#[test]
fn with_a_qr_the_lines_keep_the_right_hand_column() {
    for canvas in canvases() {
        let lines = [
            (PairSlot::Scan, (TexId::from_raw(1), 180, 34)),
            (PairSlot::Status, (TexId::from_raw(2), 100, 34)),
        ];
        let mut out = Vec::new();
        PairScreen {
            qr: Some((TexId::from_raw(3), PAIR_QR, PAIR_QR)),
            lines: &lines,
            ground: None,
            canvas,
        }
        .draw(&mut out);
        let rects = tex_rects(&out);
        let column = (PAIR_X + PAIR_QR as f32 + PAIR_GAP).round();
        assert_eq!(rects[0].1[..2], [PAIR_X, 50.0]);
        assert_eq!(rects[1].1[..2], [column, 58.0]);
        assert_eq!(rects[2].1[..2], [column, 316.0]);
    }
}
