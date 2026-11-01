/// Normalize only the newline transformation required by HTML parsing.
pub fn html_newlines(source: &str) -> String {
    source.replace("\r\n", "\n").replace('\r', "\n")
}

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
        html_newlines(trusted).trim()
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

#[test]
fn displayed_code_preserves_whitespace_after_html_newline_normalization() {
    for newline in ["\n", "\r\n", "\r"] {
        let source = format!(
            "SELECT * WHERE {{{newline}  ?subject ?predicate ?object \
             .{newline}}}{newline}"
        );
        let document =
            scraper::Html::parse_fragment(&format!("<code>{source}</code>"));
        let displayed = document
            .select(&scraper::Selector::parse("code").unwrap())
            .next()
            .unwrap()
            .text()
            .collect::<String>();
        assert_eq!(displayed, html_newlines(&source));
        assert!(
            displayed.contains("\n  ?subject"),
            "indentation is significant"
        );
    }
}
