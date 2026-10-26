//! Validation shared by image collection and rendering from restored graphs.
use {
    anyhow::{Result, bail, ensure},
    std::io::Cursor,
};
pub(crate) const MAX_IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) fn validate(bytes: &[u8], mime: &str) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_IMAGE_BYTES,
        "image exceeds 2 MiB budget or is empty"
    );
    match mime {
        "image/png" | "image/jpeg" => {
            let expected = if mime == "image/png" {
                image::ImageFormat::Png
            } else {
                image::ImageFormat::Jpeg
            };
            let reader = image::ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()?;
            ensure!(
                reader.format() == Some(expected),
                "image MIME does not match payload"
            );
            let (width, height) = reader.into_dimensions()?;
            ensure!(
                width > 0 &&
                    height > 0 &&
                    width <= 8192 &&
                    height <= 8192 &&
                    u64::from(width) * u64::from(height) <= 16_777_216,
                "image exceeds dimension budget"
            );
            let mut reader =
                image::ImageReader::with_format(Cursor::new(bytes), expected);
            let mut limits = image::Limits::default();
            limits.max_image_width = Some(8192);
            limits.max_image_height = Some(8192);
            limits.max_alloc = Some(64 * 1024 * 1024);
            reader.limits(limits);
            reader.decode()?;
        },
        "image/svg+xml" => {
            let source = std::str::from_utf8(bytes)?;
            let lower = source.to_ascii_lowercase();
            ensure!(
                !lower.contains("<!doctype") && !lower.contains("<!entity"),
                "SVG document types and entities are unsupported"
            );
            let document = scraper::Html::parse_fragment(source);
            let svg = scraper::Selector::parse("svg").unwrap();
            ensure!(
                document.select(&svg).count() == 1,
                "image must contain exactly one SVG root"
            );
            let mut count = 0;
            for node in document.tree.nodes() {
                let Some(element) = scraper::ElementRef::wrap(node) else {
                    continue
                };
                count += 1;
                ensure!(count <= 4096, "SVG exceeds element budget");
                let tag = element.value().name().to_ascii_lowercase();
                ensure!(
                    !matches!(
                        tag.as_str(),
                        "script" |
                            "foreignobject" |
                            "iframe" |
                            "object" |
                            "embed" |
                            "image" |
                            "use" |
                            "audio" |
                            "video" |
                            "animate" |
                            "animatetransform" |
                            "set"
                    ),
                    "unsupported active or external SVG element: {tag}"
                );
                for (name, value) in element.value().attrs() {
                    let name = name.to_ascii_lowercase();
                    if name == "xmlns" || name.starts_with("xmlns:") {
                        continue;
                    }
                    let compact = value
                        .to_ascii_lowercase()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>();
                    ensure!(
                        !name.starts_with("on"),
                        "SVG event handlers are unsupported"
                    );
                    if name == "href" || name.ends_with(":href") {
                        ensure!(
                            value.starts_with('#'),
                            "SVG external references are unsupported"
                        );
                    }
                    ensure!(
                        ![
                            "javascript:",
                            "http:",
                            "https:",
                            "file:",
                            "data:",
                            "@import",
                            "expression("
                        ]
                        .iter()
                        .any(|v| compact.contains(v)),
                        "SVG external references or scripts are unsupported"
                    );
                }
                if tag == "style" {
                    let css =
                        element.text().collect::<String>().to_ascii_lowercase();
                    ensure!(
                        !css.contains("@import") &&
                            !css.contains("http") &&
                            !css.contains("file:") &&
                            !css.contains("data:"),
                        "SVG external styles are unsupported"
                    );
                }
            }
        },
        _ => bail!("unsupported collected image MIME {mime}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    #[test]
    fn svg_namespace_is_not_an_external_resource() {
        assert!(validate(br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="#fff"/></svg>"##,"image/svg+xml").is_ok());
    }
    #[test]
    fn svg_active_content_and_external_resources_are_rejected() {
        for source in [
            "<svg><script>alert(1)</script></svg>",
            "<svg onload='a()'/>",
            "<svg><image href='https://example.org/a.png'/></svg>",
            "<!DOCTYPE svg><svg/>",
            "<svg><style>@import 'https://example.org/a.css';</style></svg>",
        ] {
            assert!(
                validate(source.as_bytes(), "image/svg+xml").is_err(),
                "{source}"
            );
        }
    }
}
