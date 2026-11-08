use slot_ui::reference_ground_covers;

#[test]
fn the_reference_ground_covers_panels_no_taller_or_wider_than_it() {
    assert!(reference_ground_covers((720, 480)));
    assert!(reference_ground_covers((640, 480)));
}

#[test]
fn bigger_panels_need_their_own_ground() {
    assert!(!reference_ground_covers((720, 720)));
    assert!(!reference_ground_covers((1024, 768)));
    assert!(!reference_ground_covers((960, 540)));
}
