#![cfg(feature = "nds")]

use slot::core::{dylib_name, open_core};
use slot_store::{Blending, Core, Platform};

#[test]
fn the_ds_core_is_the_slot_ds_shim() {
    assert_eq!(
        dylib_name(Core::Dsperate),
        format!("slot_ds.{}", std::env::consts::DLL_EXTENSION)
    );
}

#[test]
fn a_missing_ds_shim_falls_back_to_a_ds_sized_mock() {
    let d = tempfile::tempdir().unwrap();
    std::env::remove_var("SLOT_CORE");
    let opened = open_core(
        d.path(),
        Core::Dsperate,
        Platform::Nds,
        "",
        false,
        Blending::Off,
        None,
        None,
    );
    if opened.named {
        eprintln!("skipped: a real slot_ds is installed beside the test binary");
        return;
    }
    assert_eq!(opened.core.video_size(), (256, 384));
    assert_eq!(opened.core.video_xrgb8888().len(), 256 * 384 * 4);
}
