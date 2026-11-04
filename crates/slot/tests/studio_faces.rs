use slot::studio::LabelKind;
use slot::studio_faces::{build, chip_atlas, Cache, PreviewJob, StudioDone, StudioWorker, CHIP};
use slot_store::{Cart, Outline, Platform, ShellChoice, ShellFinish};
use slot_ui::shell_presets;

fn cart(shell: Option<ShellChoice>) -> Cart {
    Cart {
        platform: Platform::Gba,
        stem: "Probe".into(),
        rom: "Games/GBA/Probe.gba".into(),
        label: None,
        title: "PROBE".into(),
        code: String::new(),
        shell,
        sealed: false,
    }
}

fn red() -> Option<ShellChoice> {
    Some(ShellChoice {
        outline: Outline::Auto,
        colour: [0xc0, 0x10, 0x10],
        finish: ShellFinish::Solid,
    })
}

fn job(gen: u64, shell: Option<ShellChoice>) -> PreviewJob {
    PreviewJob {
        session: 1,
        gen,
        cart: cart(shell),
        ground: None,
        text: false,
        real: None,
        logo: None,
        wheel: None,
        existing: None,
        look: None,
    }
}

fn px(rgba: &[u8], w: u32, x: u32, y: u32) -> [u8; 3] {
    let i = ((y * w + x) * 4) as usize;
    [rgba[i], rgba[i + 1], rgba[i + 2]]
}

#[test]
fn a_preview_uses_the_edited_shell() {
    let mut cache = Cache::default();
    let plain = build(job(1, None), &mut cache);
    let tinted = build(job(2, red()), &mut cache);
    let title = |f: &slot::studio_faces::StudioFaces| {
        f.faces.iter().find(|(k, _)| *k == LabelKind::Title).unwrap().1.rgba.clone()
    };
    assert_ne!(px(&title(&plain), 360, 20, 100), px(&title(&tinted), 360, 20, 100));
}

#[test]
fn the_worker_answers_with_the_newest() {
    let worker = StudioWorker::spawn();
    for gen in 1..=5 {
        worker.ask(job(gen, None));
    }
    let mut last = 0;
    for _ in 0..400 {
        if let Some(StudioDone::Preview(done)) = worker.take() {
            last = done.gen;
            if last == 5 {
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(last, 5);
}

#[test]
fn chips_cover_every_preset() {
    let atlas = chip_atlas(&cart(None), None);
    let n = shell_presets().len() as u32 + 3;
    assert_eq!((atlas.w, atlas.h), (n * CHIP, 2 * CHIP));
    let centre = |cell: u32, row: u32| {
        let (x, y) = (cell * CHIP + CHIP / 2, row * CHIP + CHIP / 2);
        ((y * atlas.w + x) * 4 + 3) as usize
    };
    for cell in 0..n {
        assert_eq!(atlas.rgba[centre(cell, 1)], 255);
    }
    assert_eq!(atlas.rgba[3], 0, "chips are round");
}

#[test]
fn a_tinted_text_label_previews_its_color() {
    let mut j = job(1, None);
    j.ground = Some([9, 99, 199]);
    j.text = true;
    let tinted = build(j.clone(), &mut Cache::default());
    j.text = false;
    let plain = build(j, &mut Cache::default());
    let title = |f: &slot::studio_faces::StudioFaces| {
        f.faces.iter().find(|(k, _)| *k == slot::studio::LabelKind::Title).unwrap().1.rgba.clone()
    };
    assert_ne!(title(&tinted), title(&plain));
}

#[test]
fn a_text_color_stays_off_the_logo_sticker() {
    let d = tempfile::tempdir().unwrap();
    let logo = d.path().join("logo.png");
    let (w, h) = slot_ui::label_size(&cart(None));
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[200, 40, 40, 255].repeat((w * h) as usize)).unwrap();
    }
    std::fs::write(&logo, out).unwrap();
    let mut j = job(1, None);
    j.ground = Some([9, 99, 199]);
    j.text = true;
    j.logo = Some(logo);
    let faces = build(j, &mut Cache::default());
    assert!(faces.faces.iter().any(|(k, _)| *k == LabelKind::Logo));
}

#[test]
fn a_worn_logo_previews_automatic_from_the_wheel() {
    let d = tempfile::tempdir().unwrap();
    let wheel = d.path().join("wheel.png");
    std::fs::write(&wheel, include_bytes!("fixtures/solid_scan.jpg")).unwrap();
    let mut j = job(1, None);
    j.wheel = Some(wheel);
    let faces = build(j.clone(), &mut Cache::default());
    assert!(faces.faces.iter().any(|(k, _)| *k == LabelKind::Logo), "no Automatic preview");
    j.ground = Some([9, 99, 199]);
    let faces = build(j, &mut Cache::default());
    assert!(faces.faces.iter().any(|(k, _)| *k == LabelKind::Logo), "no JPEG wheel preview");
}
