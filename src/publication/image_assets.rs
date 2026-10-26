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
            // Tokenization rejects empty DTDs too; roxmltree permits them
            // despite allow_dtd=false. Tokens borrow the original
            // input; source bytes are never rewritten.
            for token in xmlparser::Tokenizer::from(source) {
                ensure!(
                    !matches!(
                        token?,
                        xmlparser::Token::DtdStart { .. } |
                            xmlparser::Token::EmptyDtd { .. } |
                            xmlparser::Token::EntityDeclaration { .. } |
                            xmlparser::Token::DtdEnd { .. } |
                            xmlparser::Token::ProcessingInstruction { .. }
                    ),
                    "SVG document types, custom entities and processing \
                     instructions are unsupported"
                );
            }
            let document = roxmltree::Document::parse_with_options(
                source,
                roxmltree::ParsingOptions {
                    allow_dtd:   false,
                    nodes_limit: 8192,
                },
            )?;
            let root = document.root_element();
            ensure!(
                root.tag_name().name() == "svg" &&
                    root.tag_name().namespace() ==
                        Some("http://www.w3.org/2000/svg"),
                "image must have an SVG namespace root"
            );
            let mut count = 0;
            for element in document.descendants() {
                ensure!(
                    !element.is_pi(),
                    "SVG processing instructions are unsupported"
                );
                if !element.is_element() {
                    continue;
                }
                count += 1;
                ensure!(count <= 4096, "SVG exceeds element budget");
                let tag = element.tag_name().name().to_ascii_lowercase();
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
                for attribute in element.attributes() {
                    let name = attribute.name().to_ascii_lowercase();
                    let value = attribute.value();
                    let compact = value
                        .to_ascii_lowercase()
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>();
                    ensure!(
                        !value.contains('\\') &&
                            compact
                                .split("url(")
                                .skip(1)
                                .all(|v| v.starts_with('#')),
                        "SVG escaped or external CSS URLs are unsupported"
                    );
                    ensure!(
                        !name.starts_with("on"),
                        "SVG event handlers are unsupported"
                    );
                    if name == "href" {
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
                    let css = element
                        .descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>()
                        .to_ascii_lowercase();
                    let compact = css
                        .chars()
                        .filter(|c| !c.is_whitespace())
                        .collect::<String>();
                    ensure!(
                        !css.contains('\\') &&
                            compact
                                .split("url(")
                                .skip(1)
                                .all(|v| v.starts_with('#')),
                        "SVG escaped or external CSS URLs are unsupported"
                    );
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
    fn svg_xml_declarations_and_namespace_prefixes_preserve_original_bytes() {
        let source=br##"<?xml version="1.0" encoding="UTF-8"?><!-- A comment can mention <?xml-stylesheet?> or <!DOCTYPE svg> without enabling either. --><s:svg xmlns:s="http://www.w3.org/2000/svg" width="20" height="10"><s:title>One &amp; two</s:title><s:rect width="20" height="10" fill="#fff"/></s:svg>"##;
        let before = source.to_vec();
        validate(source, "image/svg+xml").unwrap();
        assert_eq!(source.as_slice(), before);
    }
    #[test]
    fn svg_xml_parser_and_element_counts_are_bounded() {
        for children in
            ["<rect/>".repeat(4096), "<!-- comment -->".repeat(8192)]
        {
            let source = format!(
                "<svg xmlns='http://www.w3.org/2000/svg'>{children}</svg>"
            );
            assert!(validate(source.as_bytes(), "image/svg+xml").is_err());
        }
    }
    #[test]
    fn svg_requires_well_formed_xml_and_the_svg_root_namespace() {
        for source in ["<svg xmlns='http://www.w3.org/2000/svg'><rect></svg>","<svg xmlns='urn:not-svg'/>","<svg/>","<div/><svg xmlns='http://www.w3.org/2000/svg'/>","<svg xmlns='http://www.w3.org/2000/svg'>&undefined;</svg>","<?xml-stylesheet href='https://example.org/a.css'?><svg xmlns='http://www.w3.org/2000/svg'/>","<!DOCTYPE svg [<!ENTITY local 'test'>]><svg xmlns='http://www.w3.org/2000/svg'>&local;</svg>","<!DOCTYPE svg SYSTEM 'https://example.org/svg.dtd'><svg xmlns='http://www.w3.org/2000/svg'/>"] {
            assert!(validate(source.as_bytes(),"image/svg+xml").is_err(),"{source}");
        }
    }
    #[test]
    fn svg_active_content_and_external_resources_are_rejected() {
        for source in [
            "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
            "<svg xmlns='http://www.w3.org/2000/svg' xmlns:s='http://www.w3.org/2000/svg'><s:script>alert(1)</s:script></svg>",
            "<svg xmlns='http://www.w3.org/2000/svg' onload='a()'/>",
            "<svg xmlns='http://www.w3.org/2000/svg' xmlns:s='urn:custom' s:onload='a()'/>",
            "<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink'><a xlink:href='https://example.org/a'>x</a></svg>",
            "<svg xmlns='http://www.w3.org/2000/svg'><image href='https://example.org/a.png'/></svg>",
            "<!DOCTYPE svg><svg/>",
            "<!DOCTYPE svg><svg xmlns='http://www.w3.org/2000/svg'/>",
            "<svg xmlns='http://www.w3.org/2000/svg'><style>@import 'https://example.org/a.css';</style></svg>",
            "<svg xmlns='http://www.w3.org/2000/svg'><style>rect{fill:url(//example.org/a.svg)}</style></svg>",
            r"<svg xmlns='http://www.w3.org/2000/svg' style='fill:u\72l(//example.org/a.svg)'/>",
        ] {
            assert!(
                validate(source.as_bytes(), "image/svg+xml").is_err(),
                "{source}"
            );
        }
    }
}
