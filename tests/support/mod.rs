/// Specifications may execute only GraphArch's own sidebar interaction code.
/// Scripts originating in authored or collected source content remain
/// forbidden.
pub fn assert_specification_scripts(html: &str) {
    let document = scraper::Html::parse_document(html);
    let selector = scraper::Selector::parse("script").unwrap();
    let scripts: Vec<_> = document.select(&selector).collect();
    assert_eq!(
        scripts.len(),
        1,
        "only the renderer's trusted sidebar script is allowed"
    );
    assert_eq!(
        scripts[0].value().attrs().count(),
        0,
        "no external or source-supplied script attributes are allowed"
    );
    assert_eq!(
        scripts[0].text().collect::<String>().trim(),
        include_str!("../../src/publication/render/specification/sidebar.js")
            .trim()
    );
}
