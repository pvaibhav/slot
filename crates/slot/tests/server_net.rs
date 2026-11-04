use slot::server::{mdns_host, model_name};

#[test]
fn every_baseos_model_makes_a_name() {
    for (model, name) in [
        ("RG28XX", "rg28xx"),
        ("RG34XX", "rg34xx"),
        ("RG34XX SP", "rg34xxsp"),
        ("RG35XX Plus / RG35XX 2024", "rg35xxplus"),
        ("RG35XX H", "rg35xxh"),
        ("RG35XX Pro", "rg35xxpro"),
        ("RG35XX SP", "rg35xxsp"),
        ("RG40XX H", "rg40xxh"),
        ("RG40XX V", "rg40xxv"),
        ("RG CubeXX", "rgcubexx"),
        ("RG SP", "rgsp"),
    ] {
        assert_eq!(model_name(model).as_deref(), Some(name));
    }
    assert_eq!(model_name(""), None);
    assert_eq!(model_name("a.b"), None);
}

#[test]
fn the_system_host_name_is_shown_as_local() {
    assert_eq!(mdns_host("rgsp\n", None).as_deref(), Some("RGSP.LOCAL"));
    assert_eq!(
        mdns_host("MyHandheld", Some("hostname=MyHandheld\nmdns=true\n")).as_deref(),
        Some("MYHANDHELD.LOCAL")
    );
    assert_eq!(
        mdns_host("Studio.local", None).as_deref(),
        Some("STUDIO.LOCAL")
    );
}

#[test]
fn mdns_turned_off_in_baseos_shows_no_host() {
    assert_eq!(mdns_host("rgsp", Some("hostname=rgsp\nmdns=false\n")), None);
}

#[test]
fn an_empty_host_name_shows_no_host() {
    assert_eq!(mdns_host("", None), None);
}
