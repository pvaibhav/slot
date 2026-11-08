use slot::art::index::ArtIndex;

#[test]
fn a_crc_key_finds_its_entry() {
    let index = ArtIndex::parse(
        r#"{"31CB6526":{"support-texture":"GB/support-texture/31CB6526.png","wheel":"GB/wheel/31CB6526.png","look":{"ground":"473d88","sure":false,"band":null}}}"#,
    )
    .unwrap();
    let entry = index.get(0x31CB6526).unwrap();
    assert_eq!(entry.real.as_deref(), Some("GB/support-texture/31CB6526.png"));
    assert_eq!(entry.logo.as_deref(), Some("GB/wheel/31CB6526.png"));
    assert!(entry.look.is_none());
}

#[test]
fn a_sure_look_keeps_ground_and_band() {
    let index = ArtIndex::parse(
        r#"{"00787832":{"wheel":"GBA/wheel/00787832.png","look":{"ground":"f9ca06","sure":true,"band":{"colour":"a67700","edge":"top","size":0.077}}}}"#,
    )
    .unwrap();
    let look = index.get(0x00787832).unwrap().look.as_ref().unwrap();
    assert_eq!(look.ground, [0xf9, 0xca, 0x06]);
    assert!(look.band.is_some());
}

#[test]
fn a_crc_inside_crcs_finds_the_extra() {
    let index = ArtIndex::parse(
        r#"{"gba-video-x":{"crcs":["1029FC8D","6443554B"],"wheel":"GBA/wheel/gba-video-x.png"}}"#,
    )
    .unwrap();
    let entry = index.get(0x6443554B).unwrap();
    assert_eq!(entry.logo.as_deref(), Some("GBA/wheel/gba-video-x.png"));
    assert!(index.get(0x1029FC8D).is_some());
}

#[test]
fn a_bad_band_drops_the_whole_look() {
    let index = ArtIndex::parse(
        r#"{"00787832":{"look":{"ground":"f9ca06","sure":true,"band":{"colour":"a67700","edge":"middle","size":0.077}}}}"#,
    )
    .unwrap();
    assert!(index.get(0x00787832).unwrap().look.is_none());
}

#[test]
fn junk_is_none() {
    assert!(ArtIndex::parse("<html>").is_none());
}

#[test]
fn an_entry_reads_its_box() {
    let index = ArtIndex::parse(r#"{"00038342":{"box-3D":"NDS/box-3D/00038342.png"}}"#).unwrap();
    assert_eq!(index.get(0x00038342).unwrap().boxed.as_deref(), Some("NDS/box-3D/00038342.png"));
}
