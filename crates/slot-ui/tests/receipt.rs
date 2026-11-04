use qrcode::{EcLevel, QrCode, Version};
use slot_ui::{receipt_blank, receipt_face, ReceiptText, UndoFace};

const URL: &str = "http://192.168.1.42/?pin=4821";

fn modules(url: &str) -> Vec<Vec<bool>> {
    let code = QrCode::with_version(url.as_bytes(), Version::Normal(3), EcLevel::M).unwrap();
    let w = code.width();
    code.to_colors()
        .chunks(w)
        .map(|r| r.iter().map(|c| *c == qrcode::Color::Dark).collect())
        .collect()
}

fn text(host: Option<&str>, ip: &str) -> ReceiptText {
    ReceiptText {
        host: host.map(str::to_string),
        ip: ip.to_string(),
        pin: "4821".to_string(),
    }
}

fn dark(f: &UndoFace, x: u32, y: u32) -> bool {
    let i = ((y * f.w + x) * 4) as usize;
    f.rgba[i + 3] > 200 && (f.rgba[i] as u32 + f.rgba[i + 1] as u32 + f.rgba[i + 2] as u32) < 3 * 140
}

fn ink_rows(f: &UndoFace) -> Vec<u32> {
    (0..f.h)
        .filter(|&y| (0..f.w).any(|x| dark(f, x, y)))
        .collect()
}

#[test]
fn face_is_paper_sized() {
    let f = receipt_face(&modules(URL), &text(None, "192.168.1.42"));
    assert_eq!((f.w, f.h), (226, 425));
    assert_eq!(f.rgba.len(), 226 * 425 * 4);
    assert_eq!(f.rgba[3], 0);
    assert_eq!(f.rgba[((200 * 226 + 113) * 4 + 3) as usize], 255);
    let bottom = ((424 * 226) * 4 + 3) as usize;
    assert_eq!(f.rgba[bottom], 0);
}

#[test]
fn same_input_gives_same_pixels() {
    let m = modules(URL);
    let t = text(Some("GAMEBIT.LOCAL"), "192.168.1.42");
    assert_eq!(receipt_face(&m, &t).rgba, receipt_face(&m, &t).rgba);
}

#[test]
fn qr_starts_forty_down() {
    let f = receipt_face(&modules(URL), &text(None, "192.168.1.42"));
    assert_eq!(ink_rows(&f)[0], 40);
}

#[test]
fn longest_address_stays_inside_the_margins() {
    let f = receipt_face(
        &modules(URL),
        &text(Some("GAMEBIT.LOCAL:2003"), "192.168.100.200:2003"),
    );
    for y in 0..f.h {
        for x in (0..10).chain(216..226) {
            assert!(!dark(&f, x, y), "ink at {x},{y}");
        }
    }
}

#[test]
fn no_host_omits_the_line() {
    let m = modules(URL);
    let with = receipt_face(&m, &text(Some("GAMEBIT.LOCAL"), "192.168.1.42"));
    let without = receipt_face(&m, &text(None, "192.168.1.42"));
    let last = |f: &UndoFace| ink_rows(f).into_iter().filter(|y| *y < 368).last().unwrap();
    assert!(last(&without) < last(&with));
}

#[test]
fn a_host_too_long_to_fit_is_left_off() {
    let m = modules(URL);
    let long = format!("{}.LOCAL:2003", "A".repeat(63));
    let with = receipt_face(&m, &text(Some(&long), "192.168.1.42"));
    let without = receipt_face(&m, &text(None, "192.168.1.42"));
    assert_eq!(with.rgba, without.rgba);
}

#[test]
fn padding_below_pin_is_blank() {
    let f = receipt_face(&modules(URL), &text(Some("GAMEBIT.LOCAL"), "192.168.1.42"));
    let last = *ink_rows(&f).last().unwrap();
    assert!(last < 407 - 10);
    for y in last + 1..407 {
        for x in 0..f.w {
            assert!(!dark(&f, x, y));
        }
    }
}

#[test]
fn blank_face_is_paper_sized_with_text() {
    let f = receipt_blank(&["PRINTING"]);
    assert_eq!((f.w, f.h), (226, 425));
    assert!(!ink_rows(&f).is_empty());
}

#[test]
fn qr_decodes_from_the_face() {
    let f = receipt_face(&modules(URL), &text(Some("GAMEBIT.LOCAL"), "192.168.1.42"));
    let mut img = rqrr::PreparedImage::prepare_from_greyscale(f.w as usize, f.h as usize, |x, y| {
        let i = (y * f.w as usize + x) * 4;
        let a = f.rgba[i + 3] as u32;
        let g = (f.rgba[i] as u32 * 30 + f.rgba[i + 1] as u32 * 59 + f.rgba[i + 2] as u32 * 11) / 100;
        ((g * a + 255 * (255 - a)) / 255) as u8
    });
    let grids = img.detect_grids();
    assert_eq!(grids.len(), 1);
    let (_, content) = grids[0].decode().unwrap();
    assert_eq!(content, URL);
}
