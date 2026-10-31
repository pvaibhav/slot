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
    let at = select(&mut f, "Neighbor 5G");
    let screen = f.app().network_screen().unwrap();
    let count = screen.lines.len();
    let px = frame(&mut f, &mut c);
    dump(&px, "list");
    let canvas = f.app().canvas();
    let top = |i| net_row_top(count, at, i);
    assert!(pointer_at(&px, top(at)), "no pointer on the selected row");
    assert!(!pointer_at(&px, top(0)), "a pointer on an unselected row");
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
        let Some(n) = line.bars else {
            continue;
        };
        let [bx, by, _, _] = bars_rect(canvas, top(i));
        let outer = [bx + 8.0, by, 14.0, 2.0];
        let middle = [bx + 10.0, by + 6.0, 10.0, 2.0];
        assert_eq!(
            count_in(&px, outer, inked) > 0,
            n >= 3,
            "{} outer arc",
            line.label
        );
        assert_eq!(
            count_in(&px, middle, inked) > 0,
            n >= 2,
            "{} middle arc",
            line.label
        );
    }
    let value = [canvas.w - 250.0, top(0) + 8.0, 210.0, 36.0];
    assert!(
        count_in(&px, value, inked) > 50,
        "Home has no Connected value"
    );
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
    let title = [0.0, 150.0, canvas.w, 100.0];
    assert!(count_in(&px, title, bright) > 100, "no Forget prompt");
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
    open_networks(f);
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
