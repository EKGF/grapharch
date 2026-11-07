//! Safe authored SVG objects become ordinary retained Markdown images.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    regex::Regex,
    scraper::{Html, Selector},
};

pub(super) fn normalize(body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "SVG object source exceeds 1 MiB"
    );
    // bounded: protected ranges belong to one capped authored Markdown file.
    let mut protected = Vec::new();
    let mut block = None;
    for (event, range) in
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(_)) => block = Some(range.start),
            Event::End(TagEnd::CodeBlock) => {
                if let Some(start) = block.take() {
                    protected.push(start..range.end);
                }
            },
            Event::Code(_) => protected.push(range),
            _ => {},
        }
        ensure!(
            protected.len() <= 1024,
            "SVG object source exceeds code-range budget"
        );
    }
    let objects = Regex::new(r"(?is)<object\b[^>]*>.*?</object\s*>")?;
    let selector = Selector::parse("object").expect("constant object selector");
    let mut result = String::new();
    let mut offset = 0;
    let mut count = 0;
    for matched in objects.find_iter(body) {
        if protected.iter().any(|range| {
            range.start <= matched.start() && matched.start() < range.end
        }) {
            continue;
        }
        count += 1;
        ensure!(
            count <= 128,
            "Markdown source exceeds SVG object budget"
        );
        let parsed = Html::parse_fragment(matched.as_str());
        let object = parsed.select(&selector).next().expect("matched object");
        let attributes = object.value();
        ensure!(
            attributes.attrs().count() == 2 &&
                attributes.attr("type") == Some("image/svg+xml"),
            "authored objects support only SVG data/type attributes"
        );
        ensure!(
            object.children().all(|node| {
                node.value()
                    .as_text()
                    .is_some_and(|text| text.trim().is_empty())
            }),
            "authored SVG object cannot contain executable or fallback content"
        );
        let data = attributes
            .attr("data")
            .ok_or_else(|| anyhow::anyhow!("SVG object requires data"))?;
        ensure!(
            data.len() <= 4096 &&
                !data.contains(['<', '>', '\n', '\r', '\\']) &&
                !data.starts_with("//"),
            "SVG object requires a bounded local asset path"
        );
        let path = data.split('#').next().expect("split returns path");
        ensure!(
            !path.contains('?') &&
                path.ends_with(".svg") &&
                matches!(
                    url::Url::parse(data),
                    Err(url::ParseError::RelativeUrlWithoutBase)
                ),
            "SVG object requires a local .svg asset"
        );
        result.push_str(&body[offset..matched.start()]);
        result.push_str(&format!("![Diagram](<{data}>)"));
        offset = matched.end();
    }
    result.push_str(&body[offset..]);
    let figures = Regex::new(
        r"(?is)<figure(?:\s+markdown)?\s*>\s*(!\[Diagram\]\(<[^>]+>\))\s*</figure\s*>",
    )?;
    Ok(figures.replace_all(&result, "$1").into_owned())
}

#[cfg(test)]
mod tests {
    use super::normalize;
    #[test]
    fn svg_objects_keep_asset_identifiers_and_never_execute_embeds() {
        let source = "<figure markdown>\n<object \
                      data=\"/diagrams/out/example.svg#darkable\" \
                      type=\"image/svg+xml\"></object>\n</figure>";
        assert_eq!(
            normalize(source).unwrap(),
            "![Diagram](</diagrams/out/example.svg#darkable>)"
        );
        assert!(
            normalize(
                "<object data=\"https://example.org/a.svg\" \
                 type=\"image/svg+xml\"></object>"
            )
            .is_err()
        );
        assert!(
            normalize(
                "<object data=\"a.svg\" type=\"image/svg+xml\" \
                 onload=\"evil()\"></object>"
            )
            .is_err()
        );
        let example = "```html\n<object data=\"a.svg\" \
                       type=\"image/svg+xml\"></object>\n```";
        assert_eq!(normalize(example).unwrap(), example);
        let inline = "An example: `<object data=\"a.svg\" \
                      type=\"image/svg+xml\"></object>`.";
        assert_eq!(normalize(inline).unwrap(), inline);
    }
}
