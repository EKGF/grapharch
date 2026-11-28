use super::{Icon, IconPresentation, IconShortcode, presentation::IconSize};

fn icon(shortcode: &str, presentation: IconPresentation) -> Icon {
    Icon::from_source(
        IconShortcode::try_new(shortcode).unwrap(),
        presentation,
    )
    .unwrap()
    .unwrap_or_else(|| panic!("{shortcode} is bundled"))
}

#[test]
fn every_set_resolves_and_round_trips_without_external_assets() {
    let large = IconPresentation { size: IconSize::Large, middle: true };
    for name in [
        "octicons-arrow-right-24",
        "material-cached",
        "material-account-group-outline",
        "fontawesome-brands-github",
        "fontawesome-regular-heart",
        "fontawesome-solid-house",
        "simple-rust",
    ] {
        let icon = icon(name, large);
        assert_eq!(
            Icon::parse_uri(&icon.uri()).unwrap(),
            Some(icon.clone())
        );
        let html = icon.html().unwrap();
        assert!(html.contains("aria-hidden=\"true\""));
        assert!(html.contains("fill=\"currentColor\""));
        assert!(html.contains("inline-icon-lg inline-icon-middle"));
        assert!(!html.contains("<script") && !html.contains("<title"));
        assert!(icon.svg().unwrap().starts_with("<svg "));
    }
}

#[test]
fn unknown_names_and_presentations_are_reported() {
    let unknown = IconShortcode::try_new("material-no-such-icon").unwrap();
    assert!(
        Icon::from_source(unknown, IconPresentation::default())
            .unwrap()
            .is_none()
    );
    assert!(IconShortcode::try_new("emoji-smile").is_err());
    assert!(IconShortcode::try_new("material-").is_err());
    assert!(
        Icon::parse_uri("urn:grapharch:icon:material-cached:huge").is_err()
    );
    assert!(!IconPresentation::default().apply_class("heart"));
}
