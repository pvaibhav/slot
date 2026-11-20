use std::time::Instant;

use slot_store::{Cart, Platform};

fn cart(platform: Platform, stem: &str, label: bool) -> Cart {
    Cart {
        platform,
        stem: stem.into(),
        rom: format!("Games/X/{stem}.rom").into(),
        label: label.then(|| "Labels/X/label.png".into()),
        title: stem.into(),
        code: String::new(),
        shell: None,
        tags: Vec::new(),
        sealed: false,
    }
}

fn ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> (f64, f64) {
    let start = Instant::now();
    std::hint::black_box(f());
    let first = ms(start);
    let mut best = f64::MAX;
    for _ in 0..runs {
        let start = Instant::now();
        std::hint::black_box(f());
        best = best.min(ms(start));
    }
    (first, best)
}

fn main() {
    let gba = cart(
        Platform::Gba,
        "Pokemon - Emerald Version (USA, Europe)",
        false,
    );
    let gb = cart(Platform::Gb, "Kirby's Pinball Land (USA, Europe)", false);
    let gbc = cart(
        Platform::Gbc,
        "Pokemon - Crystal Version (USA, Europe)",
        false,
    );
    let font = Instant::now();
    std::hint::black_box(slot_ui::hand::hand_font());
    println!("font load {:.1} ms", ms(font));
    let art = Instant::now();
    std::hint::black_box(slot_ui::hand::marker_art([18, 18, 22]));
    println!("marker art with glow {:.1} ms", ms(art));
    for (name, c) in [("gba", &gba), ("gb", &gb), ("gbc", &gbc)] {
        let (first, best) = time(5, || slot_ui::hand_reveal(c, 64));
        println!("hand_reveal {name}: first {first:.1} ms, best {best:.1} ms");
    }
    for (name, c) in [("gba", &gba), ("gb", &gb), ("gbc", &gbc)] {
        let (first, best) = time(5, || slot_ui::cart_face_with(c, None));
        println!("face handwritten {name}: first {first:.1} ms, best {best:.1} ms");
        let labelled = Cart {
            label: Some("Labels/X/label.png".into()),
            ..c.clone()
        };
        let (w, h) = slot_ui::label_size(&labelled);
        let art: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % 251) as u8, (i % 241) as u8, (i % 239) as u8, 255])
            .collect();
        let (first, best) = time(5, || slot_ui::cart_face_with(&labelled, Some(art.clone())));
        println!("face labelled {name}: first {first:.1} ms, best {best:.1} ms");
    }
}
