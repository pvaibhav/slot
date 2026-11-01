#![cfg(target_os = "macos")]

mod common;

use common::{clocked, tmp_root_with_carts};
use slot::frontend::Frontend;
use slot::wifi::{Cmd, Fake, Wifi};
use slot_gfx::{Compositor, HeadlessSurface, OUT_H, OUT_W};
use slot_input::{Action, Btn};
use slot_power::SimPlatform;
use slot_ui::{bars_rect, key_rect, lock_rect, net_row_top, Cap, Layout, QuickRow};
use tempfile::TempDir;

fn setup() -> Option<(TempDir, Frontend, Compositor, HeadlessSurface)> {
    let surface = HeadlessSurface::new().ok()?;
    let mut compositor = Compositor::new(&surface).ok()?;
    let d = tmp_root_with_carts(&["Emerald", "Fusion"]);
    clocked(d.path());
    std::fs::write(d.path().join("Config/wifi.conf"), b"network={}\n").unwrap();
    let mut f = Frontend::boot(Box::new(SimPlatform::at(d.path().to_path_buf())));
    f.upload_faces(&mut compositor);
    f.app_mut().set_wifi(Wifi::inline(Box::new(Fake::demo())));
    settle(&mut f);
    Some((d, f, compositor, surface))
}

fn settle(f: &mut Frontend) {
    for _ in 0..4 {
        let t = f.app().now();
        f.app_mut().tick_ms(t + 1_000);
    }
}

fn press(f: &mut Frontend, btn: Btn) {
    f.app_mut().apply(Action::GbaDown(btn));
    f.app_mut().apply(Action::GbaUp(btn));
}

fn dump(frame: &[u8], name: &str) {
    let Ok(dir) = std::env::var("SCRATCH_PNG_DIR") else {
        return;
    };
    let file = std::fs::File::create(format!("{dir}/network-{name}.png")).unwrap();
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), OUT_W, OUT_H);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().unwrap().write_image_data(frame).unwrap();
}

fn px(frame: &[u8], x: usize, y: usize) -> [u8; 3] {
    let o = (y * OUT_W as usize + x) * 4;
    [frame[o], frame[o + 1], frame[o + 2]]
}

fn bright(p: [u8; 3]) -> bool {
    p.iter().all(|c| *c > 0xe0)
}

fn inked(p: [u8; 3]) -> bool {
    p.iter().all(|c| *c > 0x80)
}

fn count_in(frame: &[u8], [x, y, w, h]: [f32; 4], test: fn([u8; 3]) -> bool) -> usize {
    let (x0, y0) = (x.round() as usize, y.round() as usize);
    let (x1, y1) = ((x + w).round() as usize, (y + h).round() as usize);
    (y0..y1)
        .flat_map(|yy| (x0..x1).map(move |xx| (xx, yy)))
        .filter(|&(xx, yy)| test(px(frame, xx, yy)))
        .count()
}

fn open_networks(f: &mut Frontend) {
    f.app_mut().apply(Action::QuickMenu);
    for row in [QuickRow::Network, QuickRow::Networks] {
        for _ in 0..12 {
            if f.app().quick_menu() == Some(row) {
                break;
            }
            press(f, Btn::Down);
        }
        assert_eq!(f.app().quick_menu(), Some(row));
        if row == QuickRow::Network {
            press(f, Btn::A);
        }
    }
    press(f, Btn::A);
    settle(f);
}

fn open_scan(f: &mut Frontend) {
    open_networks(f);
    select(f, slot::networks::SCAN_NETWORKS);
    press(f, Btn::A);
    settle(f);
}

fn frame(f: &mut Frontend, c: &mut Compositor) -> Vec<u8> {
    f.compose(c);
    f.compose(c);
    c.read_frame()
}

fn pointer_at(frame: &[u8], top: f32) -> bool {
    let rect = [10.0, top, 30.0, 52.0];
    count_in(frame, rect, bright) > 20
}

fn select(f: &mut Frontend, label: &str) -> usize {
    for _ in 0..20 {
        let s = f.app().network_screen().unwrap();
        if s.lines[s.selected].label == label {
            return s.selected;
        }
        press(f, Btn::Down);
    }
    panic!("no {label}");
}

#[test]
fn the_networks_list_renders() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().wifi_send(Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    settle(&mut f);
    open_networks(&mut f);
    let saved = f.app().network_screen().unwrap();
    assert_eq!(saved.lines.len(), 2);
    let px = frame(&mut f, &mut c);
    dump(&px, "saved");
    let canvas = f.app().canvas();
    let value = [canvas.w - 250.0, net_row_top(2, 0, 0) + 8.0, 210.0, 36.0];
    assert!(
        count_in(&px, value, inked) > 50,
        "Home has no Connected value"
    );
    select(&mut f, slot::networks::SCAN_NETWORKS);
    press(&mut f, Btn::A);
    settle(&mut f);
    let at = select(&mut f, "xfinitywifi");
    let screen = f.app().network_screen().unwrap();
    let count = screen.lines.len();
    let px = frame(&mut f, &mut c);
    dump(&px, "list");
    let canvas = f.app().canvas();
    let top = |i| net_row_top(count, at, i);
    assert!(pointer_at(&px, top(at)), "no pointer on the selected row");
    assert!(!pointer_at(&px, top(0)), "a pointer on an unselected row");
    let mut fills = Vec::new();
    for (i, line) in screen.lines.iter().enumerate() {
        if line.value.is_some() {
            continue;
        }
        let lock = count_in(&px, lock_rect(canvas, top(i)), inked);
        assert_eq!(lock > 40, line.lock, "{}: lock ink {lock}", line.label);
        let bars = count_in(&px, bars_rect(canvas, top(i)), inked);
        assert_eq!(
            bars > 10,
            line.bars.is_some(),
            "{}: bar ink {bars}",
            line.label
        );
        if let Some(n) = line.bars {
            fills.push((n, bars));
        }
    }
    for &(n, ink) in &fills {
        for &(m, more) in &fills {
            if m > n {
                assert!(more > ink, "{m} bars draw {more} ink, {n} bars draw {ink}");
            }
        }
    }
}

#[test]
fn a_network_page_renders() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    f.app_mut().wifi_send(Cmd::Join {
        ssid: "Home".into(),
        password: "password".into(),
        hidden: false,
    });
    settle(&mut f);
    open_networks(&mut f);
    select(&mut f, "Home");
    press(&mut f, Btn::A);
    settle(&mut f);
    let count = f.app().network_screen().unwrap().lines.len();
    let px = frame(&mut f, &mut c);
    dump(&px, "page");
    let canvas = f.app().canvas();
    assert!(pointer_at(&px, net_row_top(count, 0, 0)));
    for i in [0, 1, 3] {
        let top = net_row_top(count, 0, i);
        let value = [canvas.w - 250.0, top + 8.0, 210.0, 36.0];
        assert!(count_in(&px, value, inked) > 50, "row {i} has no value");
    }
    for _ in 0..4 {
        press(&mut f, Btn::Down);
    }
    press(&mut f, Btn::A);
    assert!(f.app().forget_prompt().is_some());
    let px = frame(&mut f, &mut c);
    dump(&px, "forget");
    let title = [0.0, 200.0, canvas.w, 80.0];
    assert!(count_in(&px, title, bright) > 100, "no Forget prompt");
    let below = [0.0, 280.0, canvas.w, 100.0];
    assert_eq!(count_in(&px, below, bright), 0, "a line under Forget");
    press(&mut f, Btn::A);
    assert_eq!(f.app().toast(), Some(slot_ui::Toast::Forgotten));
    let px = frame(&mut f, &mut c);
    dump(&px, "forgotten");
    let (x, y, w, h) = slot_ui::toast_rect(canvas, slot_ui::Toast::Forgotten);
    assert!(
        count_in(&px, [x, y, w, h], bright) > 100,
        "no Forgotten toast"
    );
}

#[test]
fn wifi_off_says_so_in_the_list() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    open_networks(&mut f);
    f.app_mut().set_wifi_off(true);
    let px = frame(&mut f, &mut c);
    dump(&px, "off");
    let band = [0.0, 150.0, f.app().canvas().w, 100.0];
    assert!(count_in(&px, band, bright) > 100, "no message");
}

fn tex_probe(c: &mut Compositor) -> usize {
    let id = c.create_texture(1, 1, &[0; 4]);
    let s = format!("{id:?}");
    s.trim_start_matches("TexId(")
        .trim_end_matches(')')
        .parse()
        .unwrap()
}

fn open_home_keyboard(f: &mut Frontend) {
    open_scan(f);
    select(f, "Home");
    press(f, Btn::A);
    assert!(f.app().keyboard().is_some());
}

#[test]
fn typing_reuses_the_keyboard_textures() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    open_home_keyboard(&mut f);
    for _ in 0..100 {
        f.compose(&mut c);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    let idle = tex_probe(&mut c);
    for _ in 0..24 {
        f.compose(&mut c);
    }
    let before = tex_probe(&mut c);
    assert_eq!(
        before,
        idle + 1,
        "idle frames made {} textures",
        before - idle - 1
    );
    for i in 0..24 {
        press(
            &mut f,
            [Btn::A, Btn::Right, Btn::Select, Btn::B, Btn::A][i % 5],
        );
        f.compose(&mut c);
    }
    let typed = tex_probe(&mut c);
    assert_eq!(
        typed,
        before + 1,
        "typing made {} textures",
        typed - before - 1
    );
    press(&mut f, Btn::Y);
    f.compose(&mut c);
    press(&mut f, Btn::A);
    let px = frame(&mut f, &mut c);
    let reopened = tex_probe(&mut c);
    assert_eq!(
        reopened,
        typed + 1,
        "reopening made {} textures",
        reopened - typed - 1
    );
    let canvas = f.app().canvas();
    let key = key_rect(canvas, Layout::General, 1, 0);
    assert!(count_in(&px, key, bright) > 50, "the keyboard did not draw");
}

#[test]
fn the_symbols_layer_draws_its_currency_keys() {
    let Some((_d, mut f, mut c, _s)) = setup() else {
        return;
    };
    open_home_keyboard(&mut f);
    press(&mut f, Btn::Down);
    press(&mut f, Btn::Down);
    press(&mut f, Btn::Left);
    press(&mut f, Btn::A);
    let kb = f.app().keyboard().unwrap().clone();
    assert!(kb.symbols());
    let px = frame(&mut f, &mut c);
    dump(&px, "symbols");
    let canvas = f.app().canvas();
    let font = slot_ui::text::label_font().unwrap();
    let mut missing = Vec::new();
    for (r, row) in kb.rows().iter().enumerate() {
        for (col, cap) in row.iter().enumerate() {
            let Cap::Char(_) = cap else {
                continue;
            };
            let label = kb.label(*cap);
            let ch = label.chars().next().unwrap();
            if font.lookup_glyph_index(ch) == 0 {
                missing.push(ch);
            }
            let ink = count_in(&px, key_rect(canvas, Layout::General, r, col), bright);
            if "\u{20ac}\u{a3}\u{a5}\u{a2}".contains(ch) {
                assert!(ink > 15, "{ch} drew {ink} px");
            }
        }
    }
    assert!(missing.is_empty(), "label.ttf lacks {missing:?}");
}

fn ink_span(frame: &[u8], top: f32) -> Option<(usize, usize, usize)> {
    let (y0, y1) = (top.max(0.0) as usize, (top + 52.0) as usize);
    let mut hits = (usize::MAX, 0, 0);
    for y in y0..y1 {
        for x in 0..OUT_W as usize {
            if bright(px(frame, x, y)) {
                hits = (hits.0.min(x), hits.1.max(x), hits.2 + 1);
            }
        }
    }
    (hits.2 > 0).then_some(hits)
}

#[test]
fn the_list_says_it_is_scanning_until_results_arrive_then_offers_other() {
    for panel in [(640, 480), (720, 480)] {
        let Some((_d, mut f, mut c, _s)) = setup() else {
            return;
        };
        f.app_mut().set_panel(panel);
        let canvas = f.app().canvas();
        f.app_mut().apply(Action::QuickMenu);
        for row in [QuickRow::Network, QuickRow::Networks] {
            while f.app().quick_menu() != Some(row) {
                press(&mut f, Btn::Down);
            }
            press(&mut f, Btn::A);
        }
        select(&mut f, slot::networks::SCAN_NETWORKS);
        press(&mut f, Btn::A);
        let s = f.app().network_screen().unwrap();
        assert_eq!(s.message, Some("Scanning for networks"));
        assert!(s.lines.is_empty(), "{panel:?}: rows before the scan");
        let px = frame(&mut f, &mut c);
        dump(&px, &format!("scanning-{}", panel.0));
        let top = net_row_top(1, 0, 0);
        let (left, right, n) = ink_span(&px, top).expect("no scanning message");
        assert!(n > 100, "{panel:?}: message too faint");
        let mid = (left + right) as f32 / 2.0;
        assert!(
            (mid - canvas.w / 2.0).abs() <= 4.0 && (right as f32) < canvas.w,
            "{panel:?}: message spans {left}..{right} on a {} canvas",
            canvas.w
        );
        settle(&mut f);
        let s = f.app().network_screen().unwrap();
        assert_eq!(s.message, None);
        assert!(s.lines.iter().any(|l| l.label == "Neighbor 5G"));
        assert_eq!(s.lines.last().unwrap().label, "Other Network");
        let px = frame(&mut f, &mut c);
        dump(&px, &format!("scanned-{}", panel.0));
        let count = s.lines.len();
        let below = net_row_top(count, s.selected, count.min(5));
        assert!(
            count >= 5 || ink_span(&px, below).is_none(),
            "{panel:?}: message still drawn"
        );
    }
}

#[test]
fn menu_on_the_keyboard_shows_a_full_screen_help() {
    for panel in [(640, 480), (720, 480)] {
        let Some((_d, mut f, mut c, _s)) = setup() else {
            return;
        };
        f.app_mut().set_panel(panel);
        open_home_keyboard(&mut f);
        let w = f.app().canvas().w;
        let px = frame(&mut f, &mut c);
        dump(&px, &format!("keyboard-{}", panel.0));
        let legend = [w - 140.0, 440.0, 140.0, 40.0];
        assert!(
            count_in(&px, legend, bright) > 50,
            "{panel:?}: no Help legend"
        );
        f.app_mut().apply(Action::QuickMenu);
        assert!(f.app().keyboard().unwrap().help());
        let px = frame(&mut f, &mut c);
        dump(&px, &format!("keyboard-help-{}", panel.0));
        let title = [w / 2.0 - 80.0, 0.0, 160.0, 40.0];
        assert!(count_in(&px, title, bright) > 50, "{panel:?}: no title");
        let back = [0.0, 440.0, 140.0, 40.0];
        assert!(count_in(&px, back, bright) > 50, "{panel:?}: no B Back");
        assert_eq!(count_in(&px, legend, bright), 0, "{panel:?}: legend drawn");
        let rows = [w / 2.0 - 150.0, 60.0, 300.0, 360.0];
        assert!(count_in(&px, rows, bright) > 300, "{panel:?}: no rows");
        press(&mut f, Btn::B);
        assert!(!f.app().keyboard().unwrap().help());
        assert_eq!(f.app().keyboard().unwrap().text(), "");
    }
}

#[test]
fn an_invalid_password_shows_one_line() {
    for panel in [(640, 480), (720, 480)] {
        let Some((_d, mut f, mut c, _s)) = setup() else {
            return;
        };
        f.app_mut().set_panel(panel);
        open_home_keyboard(&mut f);
        press(&mut f, Btn::Start);
        assert_eq!(
            f.app().update_text(),
            Some(("Invalid Password".to_string(), String::new()))
        );
        for _ in 0..10 {
            let t = f.app().now();
            f.app_mut().tick_ms(t + 100);
            f.compose(&mut c);
        }
        let px = frame(&mut f, &mut c);
        dump(&px, &format!("invalid-password-{}", panel.0));
        let w = f.app().canvas().w;
        let band = [0.0, 100.0, w, 300.0];
        assert!(count_in(&px, band, bright) > 200, "{panel:?}: no notice");
    }
}
