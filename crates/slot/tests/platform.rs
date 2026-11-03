mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use slot::app::Phase;
use slot::session::Session;
use slot_input::{Btn, RawEvent};
use slot_store::{core_for_platform, Core, Platform, StateRing};

fn seated_core(root: &Path, stem: &str) -> Core {
    core_for_platform(
        root,
        stem,
        Platform::Gb,
        slot_store::read_slot_state(root).gb_core,
    )
}

fn seat_and_autosave(root: &Path) {
    common::clocked(root);
    let mut s = Session::boot(root.to_path_buf());
    s.feed([RawEvent::Down(Btn::A)], 16);
    s.feed([RawEvent::Up(Btn::A)], 32);

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut now = 32;
    while !matches!(s.app().phase(), Phase::Playing { .. }) {
        assert!(Instant::now() < deadline, "the cart never seated");
        now += 16;
        s.feed([], now);
        s.update(1.0 / 60.0);
        std::thread::sleep(Duration::from_millis(1));
    }

    s.app_mut().tick_ms(300_000);
    s.app_mut().settle_saves();
}

#[test]
fn a_game_boy_carts_autosave_lands_under_gb_and_never_under_gba() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zelda"]);

    seat_and_autosave(d.path());

    assert!(
        StateRing::new(
            d.path(),
            Platform::Gb,
            seated_core(d.path(), "Tetris"),
            "Tetris"
        )
        .read_resume()
        .unwrap()
        .is_some(),
        "the Game Boy cart's autosave did not land under its own platform's directory"
    );
    assert!(
        StateRing::new(
            d.path(),
            Platform::Gba,
            seated_core(d.path(), "Tetris"),
            "Tetris"
        )
        .read_resume()
        .unwrap()
        .is_none(),
        "the Game Boy cart's autosave was filed as a GBA cart's, which is where a GBA game \
         of the same name keeps its own"
    );

    assert!(
        d.path().join("Saves/GB/Tetris.sav").is_file(),
        "the Game Boy cart's battery save did not land under its own platform's directory"
    );
    assert!(
        !d.path().join("Saves/GBA/Tetris.sav").exists(),
        "the Game Boy cart's battery save was written where a GBA game of the same name \
         keeps its own"
    );
}

#[test]
fn a_hand_organised_game_boy_card_scans_seats_saves_and_resumes() {
    let d = common::tmp_root_with_gb_carts(&["Tetris", "Zelda"]);

    seat_and_autosave(d.path());

    let resume = StateRing::new(
        d.path(),
        Platform::Gb,
        seated_core(d.path(), "Tetris"),
        "Tetris",
    )
    .read_resume()
    .unwrap();
    assert!(resume.is_some(), "nothing was written back to resume from");

    let again = slot::app::App::boot(d.path());
    let cart = again
        .seated_cart()
        .expect("the next boot came up with an empty slot");
    assert_eq!(
        cart.platform,
        Platform::Gb,
        "the card came back holding a cart for the wrong machine"
    );
    assert_eq!(cart.stem, "Tetris");
    assert!(
        cart.rom.ends_with("Games/GB/Tetris.gb"),
        "the rom the slot is holding is {:?}, which is not the one in the Game Boy folder",
        cart.rom
    );
}
