//! Shared interpretation of the source documentation annotation vocabulary.
use {
    crate::publication::view::RevisionView,
    anyhow::{Result, ensure},
    oxrdf::{Literal, NamedNode, Term},
    pulldown_cmark::{Event, Parser, Tag},
    std::collections::BTreeSet,
};

pub(crate) const MARKDOWN: &str =
    "https://ekgf.org/ontology/annotation#markdownDescription";

pub(super) fn selected(value: &Literal, language: &str) -> bool {
    value
        .language()
        .is_none_or(|tag| tag.eq_ignore_ascii_case(language))
}

pub(super) fn select(
    values: Vec<String>,
    subject: &NamedNode,
    language: &str,
) -> Result<Option<String>> {
    let values: BTreeSet<_> = values.into_iter().collect();
    ensure!(
        values.len() <= 1,
        "{subject} has multiple distinct markdownDescription values for \
         {language}"
    );
    let value = values.into_iter().next();
    if let Some(value) = &value {
        ensure!(
            !value.trim().is_empty(),
            "{subject} markdownDescription cannot be empty"
        );
        ensure!(
            value.len() <= 256 * 1024,
            "{subject} markdownDescription exceeds byte budget"
        );
    }
    Ok(value)
}

pub(crate) async fn read(
    view: &RevisionView,
    subject: &NamedNode,
    language: &str,
) -> Result<Option<String>> {
    let mut rows = view.objects_by(subject, &NamedNode::new(MARKDOWN)?).await?;
    let mut values = Vec::new();
    let mut count = 0;
    while let Some(row) = rows.next().await? {
        ensure!(
            count < 64,
            "{subject} exceeds markdownDescription annotation budget"
        );
        count += 1;
        let Term::Literal(value) = &row["value"] else {
            anyhow::bail!("{subject} markdownDescription must be a literal")
        };
        if selected(value, language) {
            values.push(value.value().to_owned());
        }
    }
    select(values, subject, language)
}

/// Nest authored headings beneath a generated resource title. Parser offsets
/// distinguish real headings from fenced code; all non-heading bytes survive.
pub(crate) fn nested(body: &str, minimum: usize) -> Result<String> {
    let headings: Vec<_> =
        Parser::new_ext(body, crate::publication::markdown::options())
            .into_offset_iter()
            .filter_map(|(event, range)| {
                match event {
                    Event::Start(Tag::Heading { level, .. }) => {
                        Some((range, level as usize))
                    },
                    _ => None,
                }
            })
            .collect();
    let Some(base) = headings.iter().map(|(_, level)| *level).min() else {
        return Ok(body.into())
    };
    let mut output = body.to_owned();
    for (range, level) in headings.into_iter().rev() {
        let level = minimum + level - base;
        ensure!(
            level <= 6,
            "markdownDescription headings exceed six levels when nested \
             beneath their resource"
        );
        let source = &body[range.clone()];
        let indent =
            source.len() - source.trim_start_matches([' ', '\t', '>']).len();
        let markers =
            source[indent..].bytes().take_while(|c| *c == b'#').count();
        if markers > 0 {
            output.replace_range(
                range.start + indent..range.start + indent + markers,
                &"#".repeat(level),
            );
        } else {
            // Setext headings have no opening marker to extend. Convert only
            // their syntactic heading span, retaining the inline Markdown.
            let source = source.trim_end_matches(['\n', '\r']);
            let (title, underline) =
                source.rsplit_once('\n').ok_or_else(|| {
                    anyhow::anyhow!("unsupported Markdown heading layout")
                })?;
            ensure!(
                underline
                    .trim_start_matches([' ', '\t', '>'])
                    .trim_end()
                    .chars()
                    .all(|c| matches!(c, '=' | '-')),
                "unsupported Markdown heading layout"
            );
            let title = title
                .lines()
                .map(|line| {
                    line.trim_start_matches([' ', '\t', '>']).trim_end()
                })
                .collect::<Vec<_>>()
                .join(" ");
            output.replace_range(
                range,
                &format!(
                    "{}{} {title}\n",
                    &source[..indent],
                    "#".repeat(level)
                ),
            );
        }
    }
    Ok(output)
}

#[test]
fn nested_headings_preserve_inline_text_and_fenced_code() -> Result<()> {
    let source = "Title *with emphasis*\n=====================\n\n```md\n# \
                  Code, not a heading\n```\n\n### Child\n";
    let nested = nested(source, 3)?;
    assert!(nested.starts_with("### Title *with emphasis*\n"));
    assert!(nested.contains("```md\n# Code, not a heading\n```"));
    assert!(nested.contains("##### Child"));
    Ok(())
}

#[test]
fn nested_headings_keep_quote_and_list_context() -> Result<()> {
    for source in [
        "> ## Quoted heading\n>\n> Body.\n",
        "- ## List heading\n\n  Body.\n",
        "> Setext heading\n> ==============\n",
    ] {
        let output = nested(source, 3)?;
        let levels: Vec<_> =
            Parser::new_ext(&output, crate::publication::markdown::options())
                .filter_map(|event| {
                    match event {
                        Event::Start(Tag::Heading { level, .. }) => {
                            Some(level as usize)
                        },
                        _ => None,
                    }
                })
                .collect();
        assert_eq!(levels, [3], "heading context lost: {output}");
        let structure = |input: &str| {
            Parser::new_ext(input, crate::publication::markdown::options())
                .filter_map(|event| {
                    match event {
                        Event::Text(text) => Some(format!("text:{text}")),
                        Event::Start(
                            tag @ (Tag::BlockQuote(_) |
                            Tag::List(_) |
                            Tag::Item),
                        ) => Some(format!("container:{tag:?}")),
                        _ => None,
                    }
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            structure(source),
            structure(&output),
            "visible text or container context changed"
        );
    }
    Ok(())
}
