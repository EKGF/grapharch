//! Interpret the authored objective identifier annotation, without raw HTML.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag},
};
pub(super) fn normalize(body: &str, prefix: Option<&str>) -> Result<String> {
    let span = regex::Regex::new(
        r#"^<span class="objective-badge(?: objective-badge-sm)?" data-letter="([A-Za-z0-9]{1,16})">$"#,
    )?;
    let mut changes = Vec::new(); // bounded: caller's Markdown source budget.
    let events: Vec<_> = Parser::new(body).into_offset_iter().collect();
    for pair in events.windows(2) {
        if let (Event::InlineHtml(open), Event::InlineHtml(close)) =
            (&pair[0].0, &pair[1].0) &&
            let Some(capture) = span.captures(open)
        {
            ensure!(
                close.as_ref() == "</span>",
                "identifier badge must be empty"
            );
            changes.push((
                pair[0].1.start..pair[1].1.end,
                format!("[](urn:grapharch:badge:{})", &capture[1]),
            ));
        }
    }
    let mut result = body.to_string();
    for (range, replacement) in changes.into_iter().rev() {
        result.replace_range(range, &replacement);
    }
    if let Some(prefix) = prefix {
        ensure!(
            !prefix.is_empty() &&
                prefix.len() <= 16 &&
                prefix.bytes().all(|b| b.is_ascii_alphanumeric()),
            "identifier badge must contain 1–16 ASCII letters or digits"
        );
        let heading = Parser::new(&result)
            .into_offset_iter()
            .find_map(|(event, range)| {
                matches!(
                    event,
                    Event::Start(Tag::Heading {
                        level: pulldown_cmark::HeadingLevel::H1,
                        ..
                    })
                )
                .then_some(range)
            })
            .ok_or_else(|| {
                anyhow::anyhow!("letter_prefix requires a level-one heading")
            })?;
        let text = &result[heading.clone()];
        ensure!(
            !text.contains("urn:grapharch:badge:"),
            "letter_prefix duplicates an explicit badge"
        );
        let offset = if text.starts_with('#') {
            text.find(' ')
                .ok_or_else(|| anyhow::anyhow!("invalid heading"))? +
                1
        } else {
            0
        };
        result.insert_str(
            heading.start + offset,
            &format!("[](urn:grapharch:badge:{prefix}) "),
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::normalize;
    #[test]
    fn identifiers_preserve_code_and_reject_invalid_prefixes() {
        let code =
            "`<span class=\"objective-badge\" data-letter=\"A\"></span>`";
        assert_eq!(normalize(code, None).unwrap(), code);
        assert!(normalize("# A", Some("<script>")).is_err());
        assert!(normalize("No heading", Some("A")).is_err());
        assert!(normalize("# [](urn:grapharch:badge:A) A", Some("A")).is_err());
        assert_eq!(
            normalize("# Heading", Some("A")).unwrap(),
            "# [](urn:grapharch:badge:A) Heading"
        );
    }
}
