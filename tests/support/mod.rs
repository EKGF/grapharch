/// Specifications may execute only GraphArch's own sidebar interaction code.
/// Scripts originating in authored or collected source content remain
/// forbidden.
pub fn assert_specification_scripts(html: &str) {
    assert_scripts_against(
        html,
        include_str!("../../src/publication/render/specification/sidebar.js"),
    );
}

fn assert_scripts_against(html: &str, trusted: &str) {
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
        // HTML parsing normalizes CRLF and CR to LF before script execution.
        // Compare the same program after that specified transformation.
        trusted.replace("\r\n", "\n").replace('\r', "\n").trim()
    );
}

#[test]
fn trusted_script_comparison_respects_html_newline_normalization() {
    let windows_source = "const enabled = true;\r\nconsole.log(enabled);\r\n";
    assert_scripts_against(
        &format!("<script>{windows_source}</script>"),
        windows_source,
    );
}

#[test]
#[should_panic(
    expected = "only the renderer's trusted sidebar script is allowed"
)]
fn extra_script_is_rejected() {
    assert_scripts_against(
        "<script>trusted();</script><script>untrusted();</script>",
        "trusted();",
    );
}

#[test]
#[should_panic]
fn modified_script_is_rejected() {
    assert_scripts_against("<script>untrusted();</script>", "trusted();");
}
