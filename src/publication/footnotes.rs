//! Bounded source-qualified Markdown notes retained before excerpt selection.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Options, Parser, Tag, TagEnd},
    std::collections::{BTreeMap, BTreeSet},
};

fn options() -> Options {
    crate::publication::markdown::options() | Options::ENABLE_FOOTNOTES
}

pub(crate) fn qualify(source: &str, body: &str) -> Result<String> {
    ensure!(
        body.len() <= 1024 * 1024,
        "footnote source exceeds 1 MiB"
    );
    if !body.contains("[^") {
        return Ok(body.into());
    }
    let mut protected = Vec::new(); // bounded by the 1 MiB source, at most 8192 literal regions.
    let mut edits = Vec::new(); // bounded: 1024 references and 128 definitions.
    let mut definitions = BTreeSet::new();
    let mut references = 0;
    for (event, range) in Parser::new_ext(body, options()).into_offset_iter() {
        let (label, range) = match event {
            Event::FootnoteReference(label) => {
                references += 1;
                ensure!(
                    references <= 1024,
                    "source exceeds 1024 footnote references"
                );
                (label, range.start + 2..range.end - 1)
            },
            Event::Start(Tag::FootnoteDefinition(label)) => {
                ensure!(
                    definitions.insert(label.to_string()),
                    "duplicate footnote definition: {label}"
                );
                ensure!(
                    definitions.len() <= 128,
                    "source exceeds 128 footnotes"
                );
                let end = body[range.start..].find("]:").ok_or_else(|| {
                    anyhow::anyhow!("invalid footnote definition")
                })?;
                (label, range.start + 2..range.start + end)
            },
            Event::Code(_) |
            Event::Start(Tag::CodeBlock(_)) |
            Event::Html(_) |
            Event::InlineHtml(_) |
            Event::Start(Tag::Link { .. }) |
            Event::Start(Tag::Image { .. }) => {
                protected.push(range);
                ensure!(
                    protected.len() <= 8192,
                    "footnote literal regions exceed budget"
                );
                continue;
            },
            _ => continue,
        };
        ensure!(
            edits.len() < 1152,
            "source exceeds footnote reference budget"
        );
        let qualified = if label.strip_prefix("ga-note-").is_some_and(|s| {
            s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
        }) {
            label.to_string()
        } else {
            format!(
                "ga-note-{}",
                crate::publication::dataset::identity("footnote", &[
                    source, &label
                ])
                .as_str()
                .rsplit(':')
                .next()
                .unwrap()
            )
        };
        edits.push((range, qualified));
    }
    let marker = regex::Regex::new(r"\[\^([^\]\r\n]{1,256})\]")?;
    for capture in marker.captures_iter(body) {
        let matched = capture.get(0).unwrap();
        let label = capture.get(1).unwrap();
        let escaped = body[..matched.start()]
            .bytes()
            .rev()
            .take_while(|b| *b == b'\\')
            .count() %
            2 ==
            1;
        if escaped ||
            protected
                .iter()
                .any(|range| range.contains(&matched.start()))
        {
            continue;
        }
        ensure!(
            edits.iter().any(|(range, _)| {
                range.start == label.start() && range.end == label.end()
            }),
            "unresolved footnote reference [{}] in {source}",
            &capture[1]
        );
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut output = String::new();
    let mut offset = 0;
    for (range, replacement) in edits {
        output.push_str(&body[offset..range.start]);
        output.push_str(&replacement);
        offset = range.end;
    }
    output.push_str(&body[offset..]);
    ensure!(
        output.len() <= 1024 * 1024,
        "qualified footnotes exceed source budget"
    );
    Ok(output)
}

/// Complete a selected excerpt from its retained source, excluding unused
/// notes.
pub(crate) fn complete(excerpt: &str, source: &str) -> Result<String> {
    ensure!(
        source.len() <= 1024 * 1024 && excerpt.len() <= 1024 * 1024,
        "footnote selection exceeds 1 MiB"
    );
    let definitions = definitions(source)?;
    if definitions.is_empty() {
        return Ok(excerpt.into());
    }
    let mut body = String::new();
    let mut offset = 0;
    for (event, range) in Parser::new_ext(excerpt, options()).into_offset_iter()
    {
        if matches!(event, Event::Start(Tag::FootnoteDefinition(_))) {
            body.push_str(&excerpt[offset..range.start]);
            offset = range.end;
        }
    }
    body.push_str(&excerpt[offset..]);
    let mut candidate = body.clone();
    for definition in definitions.values() {
        candidate.push_str("\n\n");
        candidate.push_str(definition);
    }
    ensure!(
        candidate.len() <= 2 * 1024 * 1024,
        "footnote selection context exceeds 2 MiB"
    );
    let mut seen = BTreeSet::new(); // bounded: 128 source definitions.
    let mut required = Vec::new(); // bounded: 128 source definitions, in first-reference order.
    let mut in_definition = false;
    for event in Parser::new_ext(&candidate, options()) {
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => in_definition = true,
            Event::End(TagEnd::FootnoteDefinition) => in_definition = false,
            Event::FootnoteReference(label) => {
                ensure!(!in_definition, "nested footnotes are unsupported");
                if seen.insert(label.to_string()) {
                    required.push(label.to_string());
                }
            },
            _ => {},
        }
    }
    for label in required {
        body.push_str("\n\n");
        body.push_str(&definitions[&label]);
    }
    ensure!(
        body.len() <= 1024 * 1024,
        "completed footnote excerpt exceeds 1 MiB"
    );
    Ok(body)
}

fn definitions(source: &str) -> Result<BTreeMap<String, String>> {
    let mut definitions = BTreeMap::new(); // bounded: 128 notes, each <=64 KiB.
    for (event, range) in Parser::new_ext(source, options()).into_offset_iter()
    {
        if let Event::Start(Tag::FootnoteDefinition(label)) = event {
            ensure!(range.len() <= 65536, "footnote exceeds 64 KiB");
            let definition = source[range].to_owned();
            let previous =
                definitions.insert(label.to_string(), definition.clone());
            ensure!(
                previous.as_ref().is_none_or(|old| old == &definition),
                "conflicting footnote definitions"
            );
            ensure!(
                definitions.len() <= 128,
                "source exceeds 128 footnotes"
            );
        }
    }
    Ok(definitions)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unresolved_notes_fail_but_literal_examples_remain_literal() {
        assert!(qualify("source.md", "A claim[^missing].").is_err());
        let literal = "`[^code]` and \\[^escaped]\n\n```md\n[^example]\n```";
        assert_eq!(qualify("source.md", literal).unwrap(), literal);
    }
    #[test]
    fn notes_follow_first_reference_order() {
        let body =
            "First[^z], then[^a].\n\n[^z]: First note.\n[^a]: Second note.\n";
        let output = complete(body, body).unwrap();
        assert!(
            output.find("First note").unwrap() <
                output.find("Second note").unwrap()
        );
    }
    #[test]
    fn source_notes_are_qualified_and_survive_heading_selection() {
        let source = "# Claim\n\nA claim[^one].\n\n# Notes\n\n[^one]: A \
                      **study**.\n\n[^unused]: Unselected.\n";
        let normalized = qualify("one.md", source).unwrap();
        assert_ne!(normalized, qualify("two.md", source).unwrap());
        let selected = normalized.split("# Notes").next().unwrap();
        let selected = complete(selected, &normalized).unwrap();
        assert!(selected.contains("A **study**"));
        assert!(!selected.contains("Unselected"));
        assert_eq!(
            Parser::new_ext(&selected, options())
                .filter(|e| matches!(e, Event::FootnoteReference(_)))
                .count(),
            1
        );
        assert_eq!(
            qualify("outer.md", &normalized).unwrap(),
            normalized
        );
    }
}
