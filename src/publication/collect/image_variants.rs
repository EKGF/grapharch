//! Normalize exact authored image color-scheme attributes into typed hints.
use {
    crate::publication::image_hints::{ColorScheme, ImageHints},
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag},
    regex::Regex,
};
pub(super) fn normalize(body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "image variant source exceeds 1 MiB"
    );
    let attribute =
        Regex::new(r"^[ \t]*\{[ \t]*\.(only-light|only-dark)[ \t]*\}")?;
    let mut output = String::new();
    let mut offset = 0;
    let mut count = 0;
    for (event, range) in
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
    {
        let Event::Start(Tag::Image { dest_url, title, .. }) = event else {
            continue
        };
        let Some(capture) = attribute.captures(&body[range.end..]) else {
            continue
        };
        count += 1;
        ensure!(
            count <= 128,
            "image variants exceed 128 per source"
        );
        let scheme = if &capture[1] == "only-light" {
            ColorScheme::Light
        } else {
            ColorScheme::Dark
        };
        let mut hints = ImageHints::parse(&title)?;
        ensure!(
            hints.color_scheme.is_none() || hints.color_scheme == Some(scheme),
            "conflicting authored image color schemes"
        );
        hints.color_scheme = Some(scheme);
        let raw = &body[range.clone()];
        let mut depth = 0;
        let mut escaped = false;
        let mut alt_end = None;
        for (index, ch) in raw.char_indices().skip(1) {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '[' {
                depth += 1;
            }
            if ch == ']' {
                depth -= 1;
                if depth == 0 {
                    alt_end = Some(index);
                    break;
                }
            }
        }
        let alt_end = alt_end.ok_or_else(|| {
            anyhow::anyhow!(
                "image variant requires a bracketed alternative text"
            )
        })?;
        ensure!(
            !dest_url.contains(['<', '>', '\n', '\r']),
            "image variant destination must be a bounded Markdown URL"
        );
        let title = hints.encoded()?.replace('\\', "\\\\").replace('"', "\\\"");
        output.push_str(&body[offset..range.start]);
        output.push_str(&format!(
            "{}](<{}> \"{}\")",
            &raw[..alt_end],
            dest_url,
            title
        ));
        offset = range.end + capture.get(0).unwrap().end();
    }
    output.push_str(&body[offset..]);
    Ok(output)
}
#[cfg(test)]
mod tests {
    use {
        super::normalize,
        crate::publication::image_hints::{ColorScheme, ImageHints},
        pulldown_cmark::{Event, Parser, Tag},
    };
    #[test]
    fn variants_preserve_alt_title_and_literal_examples() {
        let input = "![A **model**](model.svg \"Copyright \
                     title\"){.only-light}\n\n![Interactive](model-dark.svg \
                     \"grapharch:interactive-svg\"){.only-dark}";
        let normalized = normalize(input).unwrap();
        let hints = Parser::new(&normalized)
            .filter_map(|event| {
                match event {
                    Event::Start(Tag::Image { title, .. }) => {
                        Some(ImageHints::parse(&title).unwrap())
                    },
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(hints.len(), 2);
        assert_eq!(hints[0].title.as_deref(), Some("Copyright title"));
        assert_eq!(hints[0].color_scheme, Some(ColorScheme::Light));
        assert!(hints[1].interactive_svg);
        assert_eq!(hints[1].color_scheme, Some(ColorScheme::Dark));
        assert!(normalized.contains("A **model**"));
        let literal = format!("```markdown\n{input}\n```");
        assert_eq!(normalize(&literal).unwrap(), literal);
    }
}
