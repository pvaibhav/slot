#![cfg(feature = "nds")]

mod common;

use common::{tmp_root_with_carts, StubSnapshot};
use slot::app::{App, Phase};
use slot_gfx::{OUT_H, OUT_W};
use slot_store::{write_slot_state, Platform, SlotState};

#[test]
fn a_ds_picture_keeps_its_4_by_3_shape_on_the_rg_ds_plus() {
    let d = tmp_root_with_carts(&[]);
    std::fs::create_dir_all(d.path().join("Games/NDS")).unwrap();
    std::fs::write(d.path().join("Games/NDS/Kart.nds"), vec![0u8; 0x200]).unwrap();
    write_slot_state(
        d.path(),
        &SlotState {
            cart: Some("Kart".into()),
            cart_platform: Some(Platform::Nds),
            clock_set: true,
            ..SlotState::default()
        },
    )
    .unwrap();
    let mut a = App::boot(d.path());
    let panel = (1024u32, 768u32);
    a.set_panel(panel);
    a.set_snapshot(StubSnapshot::boxed());
    a.on_core_ready();
    for _ in 0..120 {
        a.update(1.0 / 60.0);
    }
    assert!(matches!(a.phase(), Phase::Playing { .. }), "{:?}", a.phase());
    assert_eq!(a.playing_platform(), Platform::Nds);
    let [_, _, w, h] = a.game_layout().rect;
    let shape = (w * panel.0 as f32 / OUT_W as f32) / (h * panel.1 as f32 / OUT_H as f32);
    assert!((shape - 4.0 / 3.0).abs() < 0.01, "the DS picture is {shape:.3}:1, not 4:3");
}
