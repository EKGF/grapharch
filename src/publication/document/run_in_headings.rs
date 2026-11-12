//! A term's description is the content of that term's reference entry, not
//! document structure. Authored headings inside it become run-in headings: a
//! paragraph of strong text with the heading's inline Markdown. They create no
//! numbered section, outline or sidebar entry, or PDF bookmark, and no authored
//! text is dropped. A leading heading is kept too: whether it only repeats the
//! term's name cannot be decided without guessing ("Enterprise use case" is
//! not the label "Enterprise"); an author who finds it redundant removes it at
//! the source.
use {
    anyhow::{Result, ensure},
    pulldown_cmark::{Event, Parser, Tag, TagEnd},
    std::ops::Range,
};

/// One authored heading: its whole syntax span and its inline content span.
struct AuthoredHeading {
    span:   Range<usize>,
    inline: Option<Range<usize>>,
}

impl AuthoredHeading {
    // bounded: one description within the caller's 256 KiB annotation budget.
    fn all(body: &str) -> Vec<Self> {
        let mut headings = Vec::new();
        let mut current: Option<Self> = None;
        for (event, range) in
            Parser::new_ext(body, crate::publication::markdown::options())
                .into_offset_iter()
        {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    current = Some(Self { span: range, inline: None });
                },
                Event::End(TagEnd::Heading(_)) => {
                    headings.extend(current.take());
                },
                _ => {
                    if let Some(heading) = &mut current {
                        heading.inline = Some(match heading.inline.take() {
                            Some(inline) => {
                                inline.start.min(range.start)..
                                    inline.end.max(range.end)
                            },
                            None => range,
                        });
                    }
                },
            }
        }
        headings
    }

    fn run_in(&self, body: &str) -> Result<String> {
        let source = &body[self.span.clone()];
        let indent =
            source.len() - source.trim_start_matches([' ', '\t', '>']).len();
        let Some(inline) = &self.inline else {
            // An empty heading has no text to retain.
            return Ok(String::new())
        };
        let text = &body[inline.clone()];
        let delimiter = if !text.contains("**") {
            "**"
        } else {
            ensure!(
                !text.contains("__"),
                "markdownDescription heading cannot be presented as a run-in \
                 heading: it contains both ** and __ emphasis"
            );
            "__"
        };
        let mut paragraph = format!(
            "{}{delimiter}{text}{delimiter}\n",
            &source[..indent]
        );
        // A heading needs no blank line before the next block; a paragraph
        // does. Repeat the following line's container prefix to separate them.
        let rest = &body[self.span.end..];
        let next = rest.lines().next().unwrap_or_default();
        let prefix = &next
            [..next.len() - next.trim_start_matches([' ', '\t', '>']).len()];
        if !rest.is_empty() &&
            !next.trim_start_matches([' ', '\t', '>']).is_empty()
        {
            paragraph.push_str(prefix.trim_end());
            paragraph.push('\n');
        }
        Ok(paragraph)
    }
}

/// Present every authored heading of a term description as a run-in heading.
/// Parser offsets distinguish real headings from fenced code; all
/// non-heading bytes survive.
pub(crate) fn run_in(body: &str) -> Result<String> {
    let mut output = body.to_owned();
    for heading in AuthoredHeading::all(body).into_iter().rev() {
        // The replacement reproduces the span's own container prefix.
        output.replace_range(heading.span.clone(), &heading.run_in(body)?);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use {
        super::run_in,
        pulldown_cmark::{Event, Parser, Tag},
    };

    fn structure(input: &str) -> Vec<String> {
        Parser::new_ext(input, crate::publication::markdown::options())
            .filter_map(|event| {
                match event {
                    Event::Text(text) => Some(format!("text:{text}")),
                    Event::Start(Tag::Heading { .. }) => Some("heading".into()),
                    Event::Start(Tag::Strong) => Some("strong".into()),
                    Event::Start(Tag::Paragraph) => Some("paragraph".into()),
                    Event::Start(
                        tag @ (Tag::BlockQuote(_) | Tag::List(_) | Tag::Item),
                    ) => Some(format!("container:{tag:?}")),
                    _ => None,
                }
            })
            .collect()
    }

    #[test]
    fn headings_become_strong_paragraphs_and_code_stays_intact()
    -> anyhow::Result<()> {
        let source = "\n## Enterprise use case\nThe *scope* at the \
                      root.\n\n```md\n# Code, not a heading\n```\n\nTitle \
                      *with emphasis*\n=====================\n\n### \
                      Child\n\nBody.\n";
        let output = run_in(source)?;
        assert!(output.contains("**Enterprise use case**\n\nThe *scope*"));
        assert!(output.contains("```md\n# Code, not a heading\n```"));
        assert!(output.contains("**Title *with emphasis***"));
        assert!(output.contains("**Child**\n\nBody."));
        let events = structure(&output);
        assert!(
            !events.iter().any(|event| event == "heading"),
            "{output}"
        );
        assert_eq!(
            events.iter().filter(|event| *event == "strong").count(),
            3,
            "{output}"
        );
        Ok(())
    }

    #[test]
    fn run_in_headings_keep_quote_and_list_context() -> anyhow::Result<()> {
        for source in [
            "> ## Quoted heading\n>\n> Body.\n",
            "> ## Quoted heading\n> Body.\n",
            "- ## List heading\n\n  Body.\n",
            "> Setext heading\n> ==============\n",
        ] {
            let output = run_in(source)?;
            let events = structure(&output);
            assert!(!events.iter().any(|e| e == "heading"), "{output}");
            assert!(events.iter().any(|e| e == "strong"), "{output}");
            let containers = |events: &[String]| {
                events
                    .iter()
                    .filter(|e| {
                        e.starts_with("container:") || e.starts_with("text:")
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                containers(&structure(source)),
                containers(&events),
                "visible text or container context changed: {output}"
            );
        }
        Ok(())
    }

    #[test]
    fn headings_with_strong_text_use_the_alternative_delimiter()
    -> anyhow::Result<()> {
        let output = run_in("## A **bold** title\n\nBody.\n")?;
        assert!(
            output.starts_with("__A **bold** title__\n"),
            "{output}"
        );
        assert!(run_in("## A **bold** __mixed__ title\n").is_err());
        Ok(())
    }
}
